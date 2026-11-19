# CUDA の背後実装の `wait_blocking`（TASK-606）の所在と本リポの現状

TASK-606 の作業のうち、CUDA の背後実装の `CommandQueue::wait_blocking` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-606。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/contract/src/backend_cuda.rs` の `impl CommandQueue for CudaDevice` の `wait_blocking`（feature `native-blocking-wait` の裏） | `git grep -n "fn wait_blocking" -- crates/contract/src/backend_cuda.rs` |
| 待つ前の照らし | 同ファイルの `check_wait_token`・`Submissions::wait_step`・`WaitStep` | 同ファイル |
| 待ち | 同ファイルの `CudaDevice::wait_issued`・`wait_until_limit` | 同ファイル |
| 待ちの失敗の印 | 同ファイルの `Submissions::mark_wait_failure`（欄 `wait_failures`） | 同ファイル |
| 試験 | 同ファイルの `wait_blocking_tests`（`#[cfg(test)]`） | 下の「確かめ方」 |

## 待つ前の照らしと待ちの部品（1/3）

### 待つ前の照らしの部品

`check_wait_token` は次の順に照らし、最初に当たったものを返す。

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | ロスト済みを表す引数が真 | `DeviceLost`（`generation` は渡した device の世代） |
| 2 | 別の device の印 | `DeviceMismatch` |
| 3 | 印の世代が渡した device の世代と違う | `DeviceLost`（`generation` は印の世代） |
| 4 | ほか | `Ok(())` |

`Submissions::wait_step` は投入済みの表を変えずに、番号から次の一歩を返す。

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 払い出していない番号（0 と、最後に払い出した番号より大きいもの） | `Unissued { last }`（`last` は最後に払い出した番号、無ければ 0） |
| 2 | 待ちの失敗の印のある番号 | `Failed`（印の値） |
| 3 | 失敗した投入 | `Failed`（`BackendFailure`、`op` は `wait_blocking`） |
| 4 | 投入済みの表にある | `Wait` |
| 5 | ほか（回収済み） | `Done` |

`unissued_wait_error` は `InvalidArgument` を返し、文字列に `wait_blocking` と要求した番号と最後の番号を載せる。

### 期限つきの待ち

`wait_until_limit` は完了の確かめが `Pending` の間、眠りを挟んで確かめ直し、経過が期限に届くと `Timeout` を返す。
1 回目の確かめは期限によらず行い、期限 0 では眠らない。挟む眠りは残りの時間を超えない。

### 待ちの失敗の印

`mark_wait_failure` は番号に最初の失敗の値を付け、後の値で上書きしない。印の付いた番号は、`state_of` が `Lost` を返し、
投入済みの表から外れても `is_unfinished` が真を返す。

### 照らす試験

| 試験 | 照らす値 |
| ---- | -------- |
| `lost_is_checked_before_the_owner_and_the_generation` | ロスト済みは別の device の印にも今の世代の `DeviceLost`。ロストでなければ別の device の印は `DeviceMismatch`、世代違いは印の世代の `DeviceLost`、合う印は `Ok(())` |
| `unissued_sequences_are_rejected_without_changing_the_table` | 0・最後の番号 + 1・`u64::MAX` は `Unissued { last: 4 }`、照らした後も表の欄は同じ。何も払い出していない表の 1 は `Unissued { last: 0 }` |
| `the_unissued_error_names_the_operation_and_both_sequences` | `InvalidArgument` の文字列に `wait_blocking` と要求した番号と最後の番号がある |
| `issued_sequences_map_to_done_wait_or_failed` | 回収済みは `Done`、表に残る番号は `Wait`、失敗した投入は `BackendFailure`（`op` は `wait_blocking`、`backend` は `Cuda`） |
| `a_marked_sequence_keeps_its_first_error_and_stays_unfinished` | 2 回印を付けても最初の値。失敗した投入に印を付けると印の値。`state_of` は `Lost`。表から外しても `is_unfinished` は真 |
| `the_limited_wait_stops_on_a_terminal_state_or_at_the_limit` | 期限 500 µs で `Pending` が続くと `Timeout`。眠りの合計は 500 µs、確かめは 4 回。3 回目に `Complete` を返すと `Ok(Complete)` |
| `a_zero_limit_probes_once_and_never_sleeps` | 期限 0 は `Lost` を返す確かめに `Ok(Lost)`、`Pending` に `Timeout`。どちらも眠らない |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib wait_blocking_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count
```

## device の要る試験（2/3）

公開の口（`Device::memory`・`Device::queue`・`Device::fault_hooks`・`Device::generation`）だけを呼ぶ統合の試験。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_gated_copy_times_out_and_completes_after_the_gate_opens` | 門を仕込んでコピーを投入し、期限 100 ms で待ち、門を開けて `poll` を繰り返す | 待ちは `Err(Timeout)`、その後の `poll` は `Pending`。門を開けた後の `poll` は `Complete`、コピーの先は元の内容 |
| `a_limited_wait_returns_timeout_at_the_limit` | 門を仕込んで空の投入をし、期限 0・期限 50 ms で待ち、門を開けて上限なし・期限 0・期限 50 ms で待つ | 期限 0 は `Err(Timeout)` で 250 ms より早く戻る。期限 50 ms は `Err(Timeout)` で、戻るまでの時間は 50 ms 以上。門を開けた後は 3 つとも `Ok(())` |
| `an_unlimited_wait_returns_after_the_copy_and_the_clear` | 1 つの記録先にコピーとクリアを記録して投入し、上限なしで待つ | `Ok(())`。`poll` は `Complete`。コピーの先は元の内容、クリアした先は範囲だけ 0 |
| `the_wait_releases_held_buffers_and_calls_registered_callbacks` | 門で止めたコピーの元を `free` し、`on_complete` を登録し、門を開けずに上限なしで待ち、その後に門を開ける | 待つ前の解放の保留の数は `Some(1)`、callback は呼ばれていない。待った後は `Ok(())`、数は `Some(0)`、callback は `Complete` で呼ばれている |
| `a_reclaimed_submission_returns_ok` | 2 つ投入して待ち、1 つ目に上限なし・期限 0 で待ち直す | どれも `Ok(())` |
| `an_unissued_sequence_is_rejected_without_waiting` | 門で止めた投入の後、番号 0・最後の番号 + 1・`u64::MAX` の印に、上限なし・期限 0・期限 10 秒で待つ | 9 通りとも `Err(InvalidArgument(_))` で 250 ms より早く戻る。その後の `poll` は `Pending` |
| `a_token_of_another_device_is_a_mismatch` | 1 つ目の device の印を 2 つ目の device で、上限なし・期限 0 で待つ | どちらも `Err(DeviceMismatch)`。1 つ目の device では `Ok(())` |
| `a_lost_device_returns_device_lost_before_the_owner_check` | 模擬のロストの後、ロストの前の印と別の device の印に期限 50 ms で待つ | どちらも `Err(DeviceLost { generation })` で、`generation` は device の今の世代。別の device ではその印に `Ok(())` |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_wait_blocking.rs` |
| `required-features` | `backend-cuda`・`fault-injection`・`native-blocking-wait`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_wait_blocking
```

8 件が `ignored` と数えられる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-606。ポインタのみ）。

時点のコミット: `f8594be1`。`docs/spec` の gitlink は `bfc545a1462bd69953ff876a1ff231348d3352f8`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `wait_blocking_tests` の module と `backend_cuda_wait_blocking.rs` の `#[test]`・`#[ignore` の行頭の属性の行の数（awk と `grep -c '^[[:space:]]*#\[test\]'`・`grep -c '^[[:space:]]*#\[ignore'`） | `wait_blocking_tests` は `#[test]` が 7、`#[ignore` が 0。`backend_cuda_wait_blocking.rs` は `#[test]` が 8、`#[ignore` が 8 |
| 2 | 受け手の試験の名前 | 試験の名前ごとの `git grep -n "fn <名前>" -- crates`、ファイルは `git ls-files` | 下の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib wait_blocking_tests`、`cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_wait_blocking`、`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` の `running` と `test result:` の行 | 下の出力 |
| 4 | 依存 0 と `[[test]]` の 1 節 | `git diff --stat a9e8400f..HEAD -- Cargo.lock deps-approved.toml Cargo.toml`、`git diff -U0 a9e8400f..HEAD -- crates/contract/Cargo.toml` の追加・削除の行、`make deps-check` | 前の diff は出力なし。`crates/contract/Cargo.toml` は追加 4 行（`[[test]]`・`name`・`required-features`・空行）で削除 0。`make deps-check` は終了コード 0 |
| 5 | unsafe の数 | `git diff --stat a9e8400f..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs`、`a9e8400f` と `HEAD` の `crates/contract/src/backend_cuda.rs` の `grep -c unsafe`、`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 下の出力 |
| 6 | 変えた本体の行 | `git diff --numstat a9e8400f..HEAD -- crates/contract/src/backend_cuda.rs` の 1 列目・2 列目 | 追加 420、削除 13 |
| 7 | CI | `gh run view 44524938088 --json conclusion,headSha,jobs`（`f8594be1` の main の push の run）、`verify` のジョブのログの 8 本の統合の試験の行 | conclusion `success`、headSha `f8594be1aa9cb4ae49ca73d44f33ec34198cf2fb`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`。ログの行は下の出力 |

(4)・(5)・(6) の `a9e8400f` は、1/3 の 1 つ前のコミット。

(2) の出力。

```text
$ git grep -n "fn lost_is_checked_before_the_owner_and_the_generation" -- crates
crates/contract/src/backend_cuda.rs:7746:    fn lost_is_checked_before_the_owner_and_the_generation() {
$ git grep -n "fn unissued_sequences_are_rejected_without_changing_the_table" -- crates
crates/contract/src/backend_cuda.rs:7765:    fn unissued_sequences_are_rejected_without_changing_the_table() {
$ git grep -n "fn the_unissued_error_names_the_operation_and_both_sequences" -- crates
crates/contract/src/backend_cuda.rs:7792:    fn the_unissued_error_names_the_operation_and_both_sequences() {
$ git grep -n "fn issued_sequences_map_to_done_wait_or_failed" -- crates
crates/contract/src/backend_cuda.rs:7803:    fn issued_sequences_map_to_done_wait_or_failed() {
$ git grep -n "fn a_marked_sequence_keeps_its_first_error_and_stays_unfinished" -- crates
crates/contract/src/backend_cuda.rs:7822:    fn a_marked_sequence_keeps_its_first_error_and_stays_unfinished() {
$ git grep -n "fn the_limited_wait_stops_on_a_terminal_state_or_at_the_limit" -- crates
crates/contract/src/backend_cuda.rs:7840:    fn the_limited_wait_stops_on_a_terminal_state_or_at_the_limit() {
$ git grep -n "fn a_zero_limit_probes_once_and_never_sleeps" -- crates
crates/contract/src/backend_cuda.rs:7888:    fn a_zero_limit_probes_once_and_never_sleeps() {
$ git grep -n "fn a_gated_copy_times_out_and_completes_after_the_gate_opens" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:134:fn a_gated_copy_times_out_and_completes_after_the_gate_opens() {
$ git grep -n "fn a_limited_wait_returns_timeout_at_the_limit" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:163:fn a_limited_wait_returns_timeout_at_the_limit() {
$ git grep -n "fn an_unlimited_wait_returns_after_the_copy_and_the_clear" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:199:fn an_unlimited_wait_returns_after_the_copy_and_the_clear() {
$ git grep -n "fn the_wait_releases_held_buffers_and_calls_registered_callbacks" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:237:fn the_wait_releases_held_buffers_and_calls_registered_callbacks() {
$ git grep -n "fn a_reclaimed_submission_returns_ok" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:268:fn a_reclaimed_submission_returns_ok() {
$ git grep -n "fn an_unissued_sequence_is_rejected_without_waiting" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:289:fn an_unissued_sequence_is_rejected_without_waiting() {
$ git grep -n "fn a_token_of_another_device_is_a_mismatch" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:317:fn a_token_of_another_device_is_a_mismatch() {
$ git grep -n "fn a_lost_device_returns_device_lost_before_the_owner_check" -- crates
crates/contract/tests/backend_cuda_wait_blocking.rs:336:fn a_lost_device_returns_device_lost_before_the_owner_check() {
$ git ls-files crates/contract/tests/backend_cuda_wait_blocking.rs
crates/contract/tests/backend_cuda_wait_blocking.rs
```

(3) の出力。

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib wait_blocking_tests
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 118 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_wait_blocking
running 8 tests
test result: ok. 0 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(5) の出力。

```text
$ git diff --stat a9e8400f..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs
$ git show a9e8400f:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show HEAD:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(7) の出力。

```text
$ gh run view 44524938088 --job 135562881216 --log | grep -E 'test (a_gated_copy_times_out_and_completes_after_the_gate_opens|a_limited_wait_returns_timeout_at_the_limit|an_unlimited_wait_returns_after_the_copy_and_the_clear|the_wait_releases_held_buffers_and_calls_registered_callbacks|a_reclaimed_submission_returns_ok|an_unissued_sequence_is_rejected_without_waiting|a_token_of_another_device_is_a_mismatch|a_lost_device_returns_device_lost_before_the_owner_check) \.\.\. ignored, requires a CUDA device' | sed 's/^.*Z //' | sort | uniq -c
      2 test a_gated_copy_times_out_and_completes_after_the_gate_opens ... ignored, requires a CUDA device
      2 test a_limited_wait_returns_timeout_at_the_limit ... ignored, requires a CUDA device
      2 test a_lost_device_returns_device_lost_before_the_owner_check ... ignored, requires a CUDA device
      2 test a_reclaimed_submission_returns_ok ... ignored, requires a CUDA device
      2 test a_token_of_another_device_is_a_mismatch ... ignored, requires a CUDA device
      2 test an_unissued_sequence_is_rejected_without_waiting ... ignored, requires a CUDA device
      2 test an_unlimited_wait_returns_after_the_copy_and_the_clear ... ignored, requires a CUDA device
      2 test the_wait_releases_held_buffers_and_calls_registered_callbacks ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | CUDA の device の wait_blocking を event の同期と期限つきの確かめで実装する | #5198 |
| 2/3 | wait_blocking の device の要る試験を ignore 付きで足す | #5199 |
| 3/3 | 完了の条件の確かめ | #5200 |
