# CUDA の背後実装の試験用の門（TASK-345）の所在と本リポの現状

TASK-345 の作業のうち、所在と本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-345。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `GateState`・`GateLatch`・`gate_host_fn`・`CudaDevice::launch_gate`・`impl FaultHooks for CudaDevice` | 同ファイル |
| 試験 | 同ファイルの `gate_tests` | 下の「確かめ方」 |

## 門の仕込みと開け（1/3）

| 操作 | 置き場 | 動き |
| ---- | ------ | ---- |
| `arm_gate` | `impl FaultHooks for CudaDevice` | 門を仕込む。仕込み済みなら何もしない。常に `Ok(())` |
| `open_gate` | 同上 | 仕込んだ門と積んだ門を全部開ける。何度呼んでも `Ok(())` |
| 投入 | `CudaDevice::issue_event` | 仕込んだ門があれば event の前に門を待つ host の関数を積む。積めなければ `BackendFailure`（`op` は `launch_host_function`）で、通し番号を消費せず仕込みを残す。門を積んだ後に event の記録が失敗したら、門は積んだまま残る |
| 門の待ちの上限 | `GATE_EXPIRY` | 500 ms。過ぎると門が閉じたままでも host の関数が戻り、stream が先へ進む |
| `Drop`・`Device::destroy` | `CudaDevice` | 同期の前に門を全部開ける |
| `free` の中の stream の記録 | `MemoryOps::free` | 門を積まない |

feature `fault-injection` が無いビルドには、門の欄も門を積む処理もない。`impl FaultHooks for CudaDevice` と `fault_hooks()` もそのビルドには無い。

device に触らない試験 `gate_tests` の 8 本が、門の部品を照らす。

| 試験 | 照らす値 |
| ---- | -------- |
| `attaching_without_an_armed_gate_launches_nothing` | 仕込みが無ければ積む手段は 0 回で、積んだ門は 0 |
| `arming_twice_launches_one_gate_for_the_next_submission_only` | 2 回仕込んでも積む手段は 1 回で、仕込みは消え、積んだ門は 1 つで閉じている |
| `opening_before_the_submission_clears_the_armed_gate` | 投入の前に開けると積む手段は 0 回で、仕込みは消える |
| `a_failed_launch_keeps_the_armed_gate` | 積めなかったときは仕込みが残り、次の投入が同じ門を積み、開けるとその門も開く |
| `opening_opens_every_issued_gate_and_is_idempotent` | 積んだ 2 つが開いて一覧が空になり、もう一度開けても開いたまま |
| `an_opened_gate_is_dropped_from_the_issued_list_on_the_next_attach` | 開いた門は次に積むときに一覧から外れる |
| `waiting_on_a_closed_gate_returns_false_after_the_limit` | 閉じた門の待ちは上限の後に偽を返す |
| `waiting_returns_true_once_another_thread_opens_the_gate` | 別のスレッドが開けると待ちが真を返す |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib gate_tests
cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
```

## device の要る試験（2/3）

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_gated_submission_stays_pending_until_the_gate_opens` | 門を仕込んで空の投入をし、`poll` と `open_gate` を呼ぶ | 開けるまで `Pending`、開けると上限の半分より早く `Complete`、もう一度開けても `Ok(())` |
| `a_later_submission_waits_behind_the_gate` | 門を仕込んで空の投入を 2 つする | 2 つ目も開けるまで `Pending`、開けると 2 つとも `Complete` |
| `a_closed_gate_expires_and_the_submission_completes` | 門を仕込んで空の投入をし、開けない | 先に `Pending`、上限が過ぎると `Complete` |
| `opening_before_the_submission_leaves_it_ungated` | 門を仕込んで開けてから空の投入をする | 上限の半分より早く `Complete` |
| `dropping_with_a_closed_gate_returns_before_the_expiry` | 閉じた門を積んだまま `drop` する | 上限の半分より早く戻る |
| `destroying_with_a_closed_gate_returns_before_the_expiry` | 閉じた門を積んだまま `destroy` する | 上限の半分より早く戻る |

| 項目 | 値 |
| ---- | -- |
| module | `gate_device_tests`（`crates/contract/src/backend_cuda.rs`） |
| `cfg` | `#[cfg(all(test, feature = "fault-injection"))]` |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib gate_device_tests
```

6 件が `ignored` と数えられる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-345。ポインタのみ）。

時点のコミット: `8ae38674`。`docs/spec` の gitlink は `bfc545a1462bd69953ff876a1ff231348d3352f8`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `gate_device_tests` と `gate_tests` の module の中の `#[test]`・`#[ignore` の行の数（awk と `grep -c`） | `gate_device_tests` は `#[test]` が 6、`#[ignore` が 6。`gate_tests` は `#[test]` が 8 |
| 2 | 受け手の試験の名前 | 試験の名前ごとの `git grep -n "fn <名前>" -- crates`、module は `git grep -n "mod <名前> {" -- crates` | 下の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib gate_tests`、同 `--lib gate_device_tests`、`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 下の出力 |
| 4 | 依存 0 | `git diff --stat 7053b216..HEAD -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 5 | 個数の表 | `git diff 7053b216..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs`、`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 下の出力 |
| 6 | 変えない本体 | `git diff --numstat 7053b216..HEAD -- crates/contract/src/backend_cuda.rs` の 2 列目 | 追加 458、削除 2 |
| 7 | CI | `gh run view 44431952063 --json conclusion,headSha,jobs`（`8ae38674` の main の push の run）、`verify` のジョブのログの `gate_device_tests` の行 | conclusion `success`、headSha `8ae386746ca4914e9c15a748ce1c3f12e4d07df0`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`。ログの行は下の出力 |

(4)・(5)・(6) の `7053b216` は、1/3 の 1 つ前のコミット。

(2) の出力。

```text
$ git grep -n "fn a_gated_submission_stays_pending_until_the_gate_opens" -- crates
crates/contract/src/backend_cuda.rs:5687:    fn a_gated_submission_stays_pending_until_the_gate_opens() {
$ git grep -n "fn a_later_submission_waits_behind_the_gate" -- crates
crates/contract/src/backend_cuda.rs:5703:    fn a_later_submission_waits_behind_the_gate() {
$ git grep -n "fn arming_twice_launches_one_gate_for_the_next_submission_only" -- crates
crates/contract/src/backend_cuda.rs:5533:    fn arming_twice_launches_one_gate_for_the_next_submission_only() {
$ git grep -n "fn attaching_without_an_armed_gate_launches_nothing" -- crates
crates/contract/src/backend_cuda.rs:5523:    fn attaching_without_an_armed_gate_launches_nothing() {
$ git grep -n "fn opening_opens_every_issued_gate_and_is_idempotent" -- crates
crates/contract/src/backend_cuda.rs:5588:    fn opening_opens_every_issued_gate_and_is_idempotent() {
$ git grep -n "fn backend_cuda_unsafe_count" -- crates
crates/contract/tests/backend_cuda_unsafe_count.rs:22:fn backend_cuda_unsafe_count_matches_expected() {
$ git grep -n "fn dropping_with_a_closed_gate_returns_before_the_expiry" -- crates
crates/contract/src/backend_cuda.rs:5743:    fn dropping_with_a_closed_gate_returns_before_the_expiry() {
$ git grep -n "fn destroying_with_a_closed_gate_returns_before_the_expiry" -- crates
crates/contract/src/backend_cuda.rs:5755:    fn destroying_with_a_closed_gate_returns_before_the_expiry() {
$ git grep -n "fn waiting_on_a_closed_gate_returns_false_after_the_limit" -- crates
crates/contract/src/backend_cuda.rs:5618:    fn waiting_on_a_closed_gate_returns_false_after_the_limit() {
$ git grep -n "fn a_closed_gate_expires_and_the_submission_completes" -- crates
crates/contract/src/backend_cuda.rs:5718:    fn a_closed_gate_expires_and_the_submission_completes() {
$ git grep -n "fn a_failed_launch_keeps_the_armed_gate" -- crates
crates/contract/src/backend_cuda.rs:5560:    fn a_failed_launch_keeps_the_armed_gate() {
$ git grep -n "fn opening_before_the_submission_clears_the_armed_gate" -- crates
crates/contract/src/backend_cuda.rs:5548:    fn opening_before_the_submission_clears_the_armed_gate() {
$ git grep -n "fn opening_before_the_submission_leaves_it_ungated" -- crates
crates/contract/src/backend_cuda.rs:5730:    fn opening_before_the_submission_leaves_it_ungated() {
$ git grep -n "fn fault_hooks_are_present_with_default_bodies" -- crates
crates/contract/tests/backend_cuda_ext_interop.rs:99:fn fault_hooks_are_present_with_default_bodies() {
$ git grep -n "mod teardown_tests {" -- crates
crates/contract/src/backend_cuda.rs:4933:mod teardown_tests {
crates/contract/src/backend_metal.rs:14615:mod teardown_tests {
$ git grep -n "mod teardown_device_tests {" -- crates
crates/contract/src/backend_cuda.rs:5079:mod teardown_device_tests {
crates/contract/src/backend_metal.rs:9749:    mod teardown_device_tests {
$ git grep -n "mod device_tests {" -- crates
crates/contract/src/backend_cuda.rs:3927:mod device_tests {
crates/contract/src/backend_metal.rs:6006:    mod device_tests {
$ git grep -n "mod on_complete_device_tests {" -- crates
crates/contract/src/backend_cuda.rs:5320:mod on_complete_device_tests {
$ git grep -n "mod gate_tests {" -- crates
crates/contract/src/backend_cuda.rs:5505:mod gate_tests {
$ git grep -n "mod gate_device_tests {" -- crates
crates/contract/src/backend_cuda.rs:5641:mod gate_device_tests {
$ git ls-files crates/contract/tests/backend_cuda_submit.rs
crates/contract/tests/backend_cuda_submit.rs
```

(3) の出力。

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib gate_tests
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 125 filtered out; finished in 0.02s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib gate_device_tests
running 6 tests
test result: ok. 0 passed; 0 failed; 6 ignored; 0 measured; 127 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(5) の出力。

```text
$ git diff 7053b216..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs
-        block: 2,
+        block: 4,
+        func: 1,
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(7) の出力。

```text
$ gh run view 44431952063 --job 135251249191 --log | grep 'gate_device_tests' | sed 's/^.*Z //' | sort | uniq -c
      2 test backend_cuda::gate_device_tests::a_closed_gate_expires_and_the_submission_completes ... ignored, requires a CUDA device
      2 test backend_cuda::gate_device_tests::a_gated_submission_stays_pending_until_the_gate_opens ... ignored, requires a CUDA device
      2 test backend_cuda::gate_device_tests::a_later_submission_waits_behind_the_gate ... ignored, requires a CUDA device
      2 test backend_cuda::gate_device_tests::destroying_with_a_closed_gate_returns_before_the_expiry ... ignored, requires a CUDA device
      2 test backend_cuda::gate_device_tests::dropping_with_a_closed_gate_returns_before_the_expiry ... ignored, requires a CUDA device
      2 test backend_cuda::gate_device_tests::opening_before_the_submission_leaves_it_ungated ... ignored, requires a CUDA device
```

## 解放の保留の数（4/6）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `impl MemoryOps for CudaDevice` の `pending_release_count`。既定の `None` を上書きする | `git grep -n "fn pending_release_count" -- crates/contract/src/backend_cuda.rs` |
| 試験 | `pending_release_count_tests`（`crates/contract/src/backend_cuda.rs`。`#[cfg(test)]`） | 下のコマンド |

device に触らない試験 1 本が、解放を保留するバッファの一覧の数え方を照らす。

| 試験 | 照らす値 |
| ---- | -------- |
| `each_held_buffer_counts_once_even_for_the_same_submission` | 同じ投入の番号で積んだ 2 つと別の番号の 1 つで数が 3。終わった番号の 2 つを外すと 1 |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib pending_release_count_tests
```

## 解放の保留の数の device の要る試験（5/6）

公開の口（`Device::memory`・`Device::queue`・`Device::fault_hooks`）だけを呼ぶ統合の試験。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_freed_copy_source_is_counted_until_the_gated_copy_completes` | 門を仕込んでコピーを投入し、コピーに使っていないバッファとコピーの元を `free` し、門を開けて `Complete` まで `poll` し、空の投入をしてから、コピーの先を `download_blocking` で読む | 数は始めに `Some(0)`、使っていないバッファの `free` の後も `Some(0)`、元の `free` の後に `Some(1)`。門を開ける前の `poll` は `Pending` で数は `Some(1)` のまま。`Complete` の後の空の投入の後に `Some(0)`。読んだ内容は元に書いた内容と同じ |
| `reading_the_count_does_not_release_held_buffers` | 門で止めたコピーの元を `free` し、門を開け、別のバッファへの `upload` の後に数を 2 回読み、`poll` を呼ぶ | 2 回とも `Some(1)`。`poll` が `Complete` を返した後に `Some(0)` |
| `freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission` | 門を仕込んでから使っていないバッファを `free` し、別のバッファへ `upload` し、空の投入をして `poll` と `open_gate` を呼ぶ | `upload` は 250 ms より早く戻る。投入の後の `poll` は `Pending`。門を開けると `Complete` |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_pending_release.rs` |
| `required-features` | `backend-cuda`・`fault-injection`・`native-blocking-wait`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_pending_release
```

3 件が `ignored` と数えられる。

## 完了の条件の確かめ（6/6）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-345。ポインタのみ）。

時点のコミット: `32a1f32a`。`docs/spec` の gitlink は `bfc545a1462bd69953ff876a1ff231348d3352f8`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `pending_release_count_tests` の module と `backend_cuda_pending_release.rs` の `#[test]`・`#[ignore` の行頭の属性の行の数（awk と `grep -c '^#\[ignore'`。行頭を限らないと `//!` のコメントの言及まで当たって 4 になる） | `pending_release_count_tests` は `#[test]` が 1、`#[ignore` が 0。`backend_cuda_pending_release.rs` は `#[test]` が 3、`#[ignore` が 3 |
| 2 | 受け手の試験の名前 | 試験の名前ごとの `git grep -n "fn <名前>" -- crates`、ファイルは `git ls-files` | 下の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib pending_release_count_tests`、`cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_pending_release`、`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 下の出力 |
| 4 | 依存 0 と `[[test]]` の 1 節 | `git diff --stat 050a99cc..HEAD -- Cargo.lock deps-approved.toml Cargo.toml`、`git diff -U0 050a99cc..HEAD -- crates/contract/Cargo.toml` の追加・削除の行、`make deps-check` | 前の diff は出力なし。`crates/contract/Cargo.toml` は追加 4 行（`[[test]]`・`name`・`required-features`・空行）で削除 0。`make deps-check` は終了コード 0 |
| 5 | unsafe の数 | `git diff --stat 050a99cc..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs`、`050a99cc` と `HEAD` の `crates/contract/src/backend_cuda.rs` の `grep -c unsafe`、`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 下の出力 |
| 6 | 変えない本体 | `git diff --numstat 050a99cc..HEAD -- crates/contract/src/backend_cuda.rs` の 2 列目 | 追加 32、削除 0 |
| 7 | CI | `gh run view 44494391417 --json conclusion,headSha,jobs`（`32a1f32a` の main の push の run）、`verify` のジョブのログの 3 つの統合の試験の行 | conclusion `success`、headSha `32a1f32ac8536ea256eff0d3878259402df6f2b3`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`。ログの行は下の出力 |
| 8 | 1/3 の表の行と照らす試験 | `free` の中の stream の記録の行を照らす試験の名前、`launch_host_function` の失敗の写しと `self.gate.attach_with` の呼び出しの `git grep -n` | 下の出力。`launch_host_function` の失敗の写し 2 行と `attach_with` の呼び出し 1 行を照らす試験は、`gate_tests`・`gate_device_tests`・`pending_release_count_tests`・`backend_cuda_pending_release` に無い |

(4)・(5)・(6) の `050a99cc` は、4/6 の 1 つ前のコミット。

(2) の出力。

```text
$ git grep -n "fn each_held_buffer_counts_once_even_for_the_same_submission" -- crates
crates/contract/src/backend_cuda.rs:7491:    fn each_held_buffer_counts_once_even_for_the_same_submission() {
$ git grep -n "fn a_freed_copy_source_is_counted_until_the_gated_copy_completes" -- crates
crates/contract/tests/backend_cuda_pending_release.rs:96:fn a_freed_copy_source_is_counted_until_the_gated_copy_completes() {
$ git grep -n "fn reading_the_count_does_not_release_held_buffers" -- crates
crates/contract/tests/backend_cuda_pending_release.rs:130:fn reading_the_count_does_not_release_held_buffers() {
$ git grep -n "fn freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission" -- crates
crates/contract/tests/backend_cuda_pending_release.rs:154:fn freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission() {
$ git ls-files crates/contract/tests/backend_cuda_pending_release.rs
crates/contract/tests/backend_cuda_pending_release.rs
```

(3) の出力。

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib pending_release_count_tests
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 117 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_pending_release
running 3 tests
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(5) の出力。

```text
$ git diff --stat 050a99cc..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs
$ git show 050a99cc:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show HEAD:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(7) の出力。

```text
$ gh run view 44494391417 --job 135457771791 --log | grep -e 'a_freed_copy_source_is_counted_until_the_gated_copy_completes' -e 'reading_the_count_does_not_release_held_buffers' -e 'freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission' | grep 'ignored, requires a CUDA device' | sed 's/^.*Z //' | sort | uniq -c
      2 test a_freed_copy_source_is_counted_until_the_gated_copy_completes ... ignored, requires a CUDA device
      2 test freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission ... ignored, requires a CUDA device
      2 test reading_the_count_does_not_release_held_buffers ... ignored, requires a CUDA device
```

(8) の出力。

```text
$ git grep -n "fn freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission" -- crates
crates/contract/tests/backend_cuda_pending_release.rs:154:fn freeing_a_buffer_leaves_the_armed_gate_for_the_next_submission() {
$ git grep -n 'backend_failure("launch_host_function"' -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:2294:            .map_err(|e| backend_failure("launch_host_function", e))?;
crates/contract/src/backend_cuda.rs:2307:        .map_err(|e| backend_failure("launch_host_function", e))
$ git grep -n 'self.gate.attach_with' -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:2109:            self.gate.attach_with(|latch| {
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 試験用の門の仕込みと開け | #4455 |
| 2/3 | 門で止めた投入の device の要る試験 | #4456 |
| 3/3 | 完了の条件の確かめ | #4457 |
| 4/6 | 解放を保留しているバッファの数を CUDA の device で返す | #4458 |
| 5/6 | 解放の保留の数の device の要る試験を ignore 付きで足す | #4459 |
| 6/6 | 完了の条件の確かめ | #4460 |
