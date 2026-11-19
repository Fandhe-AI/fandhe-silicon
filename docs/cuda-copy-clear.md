# CUDA の背後実装のバッファのコピーとクリアの記録（TASK-624）の所在と本リポの現状

TASK-624 の作業のうち、CUDA の背後実装の `MemoryOps::copy_buffer_to_buffer`・`clear_buffer` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-624。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 記録の時点の照らし | `crates/contract/src/backend_cuda.rs` の `check_copy_record`・`check_clear_record` | 同ファイル |
| 投入の時点の組み立て | 同ファイルの `plan_transfers`・`CudaDevice::launch_transfers`・`CudaDevice::issue_event` | 同ファイル |
| 記録先の列 | 同ファイルの `OpenEncoders` | 同ファイル |
| 解放の保留 | 同ファイルの `PendingReleases`・`hold_or_release`・`CudaDevice::release_finished`・`impl MemoryOps for CudaDevice` の `free` | 同ファイル |
| 試験 | 同ファイルの `copy_clear_tests` | 下の「確かめ方」 |

## 記録と投入と解放の保留（1/3）

### 記録の時点

`copy_buffer_to_buffer`・`clear_buffer` の記録の時点の照らしと返す値は次のとおり。2〜9 は照らしの関数 `check_copy_record`・`check_clear_record` がこの順に照らす。照らしの関数は記録先を変えない。

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | この device で開いていない記録先（記録先の口 `OpenEncoders` の照らし） | `InvalidArgument` |
| 2 | 別の device のバッファ（元 → 先） | `DeviceMismatch` |
| 3 | ハンドルの世代が device の今の世代と違う | `InvalidArgument` |
| 4 | 表に無い・払い出しの世代がハンドルの世代と違う | `InvalidArgument` |
| 5 | ハンドルの大きさが表のバッファの長さと違う | `InvalidArgument` |
| 6 | 用途が足りない（コピーは元が `DOWNLOAD`・先が `UPLOAD`、クリアは `UPLOAD`） | `InvalidArgument`（操作の名前を前置） |
| 7 | 範囲がバッファの外・あふれる | `InvalidArgument` |
| 8 | offset と長さが 4 の倍数でない | `InvalidArgument` |
| 9 | コピーの元と先が同じバッファ | `InvalidArgument` |
| - | 上のどれにも当たらず長さが 0 | 何も記録せず `Ok(())` |
| - | 上のどれにも当たらない | 記録を足して `Ok(())` |

- 2・6・7 は、各行を元と先の両方に当ててから次の行へ進む
- 3・4・5 は、元の 3 行（世代・表・大きさ）を順に照らしてから、先の同じ 3 行を照らす

### 投入の時点と解放の保留の部品

device に触らない部品の振る舞いは次のとおり。

| 部品 | 振る舞い |
| ---- | -------- |
| `plan_transfers` | 記録の順を保ったまま、バッファの範囲を表の長さで照らして (始点, 終点) に直す |
| `plan_transfers` | 解放済み・世代違いのバッファは、範囲より先に `InvalidArgument` で拒む |
| `hold_or_release` | 最後に使った投入が終わっていない間だけ保留にし、無い・終わっているときはそのまま返す |
| `PendingReleases` | 終わったものだけを取り出し、残りは入れた順のまま残す |
| `OpenEncoders` | 開いている記録先にだけ記録を足し、取り出すと記録の順に返す |

device に触らない試験 `copy_clear_tests` の 17 本が、照らしと組み立てと保留の部品を確かめる。

| 試験 |
| ---- |
| `copy_passes_and_records_the_operands` |
| `clear_passes_and_records_the_operand` |
| `copy_rejects_each_row` |
| `clear_rejects_each_row` |
| `copy_reports_the_earlier_row_when_two_adjacent_rows_apply` |
| `clear_reports_the_earlier_row_when_two_adjacent_rows_apply` |
| `u64_overflow_is_rejected` |
| `zero_length_records_nothing_unless_an_earlier_row_applies` |
| `usage_rejections_name_the_operation_and_the_missing_usage` |
| `plan_keeps_the_record_order_and_converts_the_ranges` |
| `plan_rejects_a_freed_or_stale_buffer_before_any_range` |
| `jobs_name_the_buffers_they_touch` |
| `open_encoders_record_only_to_open_ids_and_claim_returns_the_records_in_order` |
| `pair_mut_borrows_two_distinct_entries` |
| `hold_or_release_holds_only_while_the_last_submission_is_unfinished` |
| `pending_releases_take_finished_keeps_the_unfinished_in_order` |
| `a_sequence_stays_unfinished_until_reclaim_removes_it` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib copy_clear_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count
```

## device の要る試験（2/3）

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_recorded_copy_takes_effect_only_after_submit` | `copy_buffer_to_buffer` を記録して `submit` し、`poll` と読み戻しを呼ぶ | 投入の前の読み戻しは元の値（全 0）、完了の後は CPU の参照と全バイト一致、元は変わらない |
| `a_recorded_clear_zeroes_only_the_range` | `clear_buffer` を記録して `submit` する | 投入の前は元の値、完了の後は範囲だけ 0 にした CPU の参照と全バイト一致 |
| `records_take_effect_in_record_order` | 1 つの記録先に a→b のコピー、b のクリア、b→c のコピーを記録して `submit` する | b と c が CPU の参照と一致 |
| `submit_many_runs_each_encoder_in_the_given_order` | 2 つの記録先を `submit_many` で投入する | c が CPU の参照と一致 |
| `a_rejected_record_leaves_the_encoder_usable` | 用途・範囲・整列・同じバッファのコピーと、用途・範囲・整列のクリアを記録する | 7 つとも `InvalidArgument`。続く正しいコピーの投入が効き、拒まれた先は全 0 のまま |
| `a_zero_length_record_changes_nothing` | 長さ 0 のコピーとクリアを記録して `submit` する | どちらも `Ok(())`。投入の後も元と先は変わらない |
| `freeing_a_recorded_buffer_before_submit_rejects_the_submit` | 記録の後に元を `free` して `submit` する | `InvalidArgument`。`test_submitted_count()` は変わらず、次の空の投入の `sequence` は前の数 + 1、先は全 0 のまま |
| `freeing_a_buffer_right_after_submit_keeps_the_result` | 投入の直後に元を `free` する | 完了の後に先が CPU の参照と一致。同じ長さの次の確保の読み戻しは全 0 |
| `buffers_freed_while_their_copy_is_gated_are_held_until_completion` | 門を仕込んでコピーを投入し、バッファを `free` していき、門を開ける | 解放の保留の数が、コピーに使っていないバッファの `free` で 0、元の `free` で 1、先の `free` で 2。完了の後に `Complete` が続き、数は 0 |
| `held_buffers_are_released_at_each_recovery_point` | 門で止めたコピーの元を `free` し、門を開けてから `allocate`・`submit`・`submit_many`・`poll` を 1 つずつ呼ぶ | 保留の数は呼ぶ前に 1、呼んだ後に 0 |

| 項目 | 値 |
| ---- | -- |
| module | `copy_clear_device_tests`（`crates/contract/src/backend_cuda.rs`）。試験 8 本 |
| `cfg` | `#[cfg(all(test, feature = "native-blocking-wait"))]` |
| module | `copy_clear_gate_device_tests`（同ファイル）。試験 2 本 |
| `cfg` | `#[cfg(all(test, feature = "fault-injection"))]` |
| `#[ignore]` | どちらも `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --lib copy_clear_device_tests
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib copy_clear_gate_device_tests
```

1 本目で 8 件、2 本目で 2 件が `ignored` と数えられる。

### 振る舞いと照らす試験

| 振る舞い | 照らす試験 |
| -------- | ---------- |
| 記録の時点では stream に何も積まず、投入の前の読み戻しは元の値のまま | `a_recorded_copy_takes_effect_only_after_submit`・`a_recorded_clear_zeroes_only_the_range` |
| 1 つの記録先の記録は記録の順に効く | `records_take_effect_in_record_order` |
| `submit_many` は渡した記録先の順に効く | `submit_many_runs_each_encoder_in_the_given_order` |
| 記録した後に元を `free` すると `submit` は `InvalidArgument` で、投入の数と次の通し番号は変わらず、先は変わらない | `freeing_a_recorded_buffer_before_submit_rejects_the_submit` |
| 投入の直後に元を `free` しても結果は保たれ、同じ長さの次の確保の読み戻しは全 0 | `freeing_a_buffer_right_after_submit_keeps_the_result` |
| コピーに使っていないバッファの `free` は保留されず、コピーに使ったバッファの `free` は投入の完了まで保留される | `buffers_freed_while_their_copy_is_gated_are_held_until_completion` |
| 保留したバッファは `allocate`・`submit`・`submit_many`・`poll` のどれを呼んでも、投入が終わっていれば解放される | `held_buffers_are_released_at_each_recovery_point` |

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-624。ポインタのみ）。

時点のコミット: `0b62fdc3`。`docs/spec` の gitlink は `bfc545a1462bd69953ff876a1ff231348d3352f8`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `copy_clear_tests`・`copy_clear_device_tests`・`copy_clear_gate_device_tests` の module の中の `#[test]`・`#[ignore` の行の数（awk と `grep -c`） | `copy_clear_tests` は `#[test]` が 17、`#[ignore` が 0。`copy_clear_device_tests` は `#[test]` が 8、`#[ignore` が 8。`copy_clear_gate_device_tests` は `#[test]` が 2、`#[ignore` が 2 |
| 2 | 受け手の試験の名前 | 試験の名前ごとの `git grep -n "fn <名前>" -- crates/contract/src/backend_cuda.rs`、既存の module は `git grep -n "^mod <名前> {" -- crates/contract/src/backend_cuda.rs`、統合の試験は `git ls-files` | 下の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib copy_clear_tests`、`--features backend-cuda,native-blocking-wait --lib copy_clear_device_tests`、`--features backend-cuda,fault-injection --lib copy_clear_gate_device_tests`、`--features backend-cuda --test backend_cuda_unsafe_count` | 下の出力 |
| 4 | 依存 0 | `git diff --stat 5b71dda9..HEAD -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 5 | unsafe の数 | `git diff --stat 5b71dda9..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs`、`git show <rev>:crates/contract/src/backend_cuda.rs \| grep -c unsafe`、`cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count` | 下の出力 |
| 6 | 変えない本体 | `git diff --numstat 5b71dda9..HEAD -- crates/contract/src/backend_cuda.rs` の 2 列目 | 追加 1646、削除 72 |
| 7 | 積む順 | `let sequence = subs.issue(\|\| {` の後の `attach_with`・`launch_transfers`・`record_event(None)` を含む行を出す awk | 下の出力 |
| 8 | CI | `gh run view 44471705850 --json conclusion,headSha,jobs`（`0b62fdc3` の main の push の run）、`verify` のジョブのログの `copy_clear_device_tests::`・`copy_clear_gate_device_tests::` の行 | conclusion `success`、headSha `0b62fdc318c7883bfb37cc73eb7b7e9262a93e96`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`。ログの行は下の出力 |

(4)・(5)・(6) の `5b71dda9` は、1/3 の 1 つ前のコミット。

(2) の出力。

```text
$ git grep -n "fn a_recorded_copy_takes_effect_only_after_submit" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7173:    fn a_recorded_copy_takes_effect_only_after_submit() {
$ git grep -n "fn a_recorded_clear_zeroes_only_the_range" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7194:    fn a_recorded_clear_zeroes_only_the_range() {
$ git grep -n "fn records_take_effect_in_record_order" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7209:    fn records_take_effect_in_record_order() {
$ git grep -n "fn submit_many_runs_each_encoder_in_the_given_order" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7229:    fn submit_many_runs_each_encoder_in_the_given_order() {
$ git grep -n "fn plan_keeps_the_record_order_and_converts_the_ranges" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6880:    fn plan_keeps_the_record_order_and_converts_the_ranges() {
$ git grep -n "fn open_encoders_record_only_to_open_ids_and_claim_returns_the_records_in_order" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6967:    fn open_encoders_record_only_to_open_ids_and_claim_returns_the_records_in_order() {
$ git grep -n "fn copy_rejects_each_row" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6619:    fn copy_rejects_each_row() {
$ git grep -n "fn clear_rejects_each_row" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6678:    fn clear_rejects_each_row() {
$ git grep -n "fn copy_reports_the_earlier_row_when_two_adjacent_rows_apply" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6706:    fn copy_reports_the_earlier_row_when_two_adjacent_rows_apply() {
$ git grep -n "fn clear_reports_the_earlier_row_when_two_adjacent_rows_apply" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6770:    fn clear_reports_the_earlier_row_when_two_adjacent_rows_apply() {
$ git grep -n "fn usage_rejections_name_the_operation_and_the_missing_usage" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6837:    fn usage_rejections_name_the_operation_and_the_missing_usage() {
$ git grep -n "fn a_rejected_record_leaves_the_encoder_usable" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7255:    fn a_rejected_record_leaves_the_encoder_usable() {
$ git grep -n "fn u64_overflow_is_rejected" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6797:    fn u64_overflow_is_rejected() {
$ git grep -n "fn plan_rejects_a_freed_or_stale_buffer_before_any_range" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6908:    fn plan_rejects_a_freed_or_stale_buffer_before_any_range() {
$ git grep -n "fn zero_length_records_nothing_unless_an_earlier_row_applies" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6810:    fn zero_length_records_nothing_unless_an_earlier_row_applies() {
$ git grep -n "fn a_zero_length_record_changes_nothing" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7294:    fn a_zero_length_record_changes_nothing() {
$ git grep -n "fn freeing_a_recorded_buffer_before_submit_rejects_the_submit" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7311:    fn freeing_a_recorded_buffer_before_submit_rejects_the_submit() {
$ git grep -n "fn buffers_freed_while_their_copy_is_gated_are_held_until_completion" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7401:    fn buffers_freed_while_their_copy_is_gated_are_held_until_completion() {
$ git grep -n "fn hold_or_release_holds_only_while_the_last_submission_is_unfinished" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7029:    fn hold_or_release_holds_only_while_the_last_submission_is_unfinished() {
$ git grep -n "fn pending_releases_take_finished_keeps_the_unfinished_in_order" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7049:    fn pending_releases_take_finished_keeps_the_unfinished_in_order() {
$ git grep -n "fn a_sequence_stays_unfinished_until_reclaim_removes_it" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7066:    fn a_sequence_stays_unfinished_until_reclaim_removes_it() {
$ git grep -n "fn freeing_a_buffer_right_after_submit_keeps_the_result" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7334:    fn freeing_a_buffer_right_after_submit_keeps_the_result() {
$ git grep -n "fn held_buffers_are_released_at_each_recovery_point" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7429:    fn held_buffers_are_released_at_each_recovery_point() {
$ git grep -n "fn pair_mut_borrows_two_distinct_entries" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:7014:    fn pair_mut_borrows_two_distinct_entries() {
$ git grep -n "fn backend_cuda_unsafe_count" -- crates/contract/tests/backend_cuda_unsafe_count.rs
crates/contract/tests/backend_cuda_unsafe_count.rs:22:fn backend_cuda_unsafe_count_matches_expected() {
$ git grep -n "^mod tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:3068:mod tests {
$ git grep -n "^mod device_tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:4531:mod device_tests {
$ git grep -n "^mod teardown_tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:5537:mod teardown_tests {
$ git grep -n "^mod teardown_device_tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:5683:mod teardown_device_tests {
$ git grep -n "^mod on_complete_device_tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:5924:mod on_complete_device_tests {
$ git grep -n "^mod gate_tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6109:mod gate_tests {
$ git grep -n "^mod gate_device_tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6245:mod gate_device_tests {
$ git grep -n "^mod on_complete_gate_device_tests {" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:6371:mod on_complete_gate_device_tests {
$ git ls-files crates/contract/tests/backend_cuda_submit.rs
crates/contract/tests/backend_cuda_submit.rs
```

(3) の出力。

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib copy_clear_tests
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --lib copy_clear_device_tests
running 8 tests
test result: ok. 0 passed; 0 failed; 8 ignored; 0 measured; 131 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib copy_clear_gate_device_tests
running 2 tests
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 152 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(5) の出力。

```text
$ git diff --stat 5b71dda9..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs
$ git show 5b71dda9:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show HEAD:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(7) の出力。

```text
$ awk '/let sequence = subs.issue\(\|\| \{/{f=1} f&&/attach_with|launch_transfers|record_event\(None\)/{print} f&&/^        \}\)\?;/{exit}' crates/contract/src/backend_cuda.rs
            self.gate.attach_with(|latch| {
            self.launch_transfers(map, jobs)?;
                    .record_event(None)
```

(8) の出力。

```text
$ gh run view 44471705850 --job 135382504656 --log | grep -E 'backend_cuda::copy_clear_(gate_)?device_tests::' | sed 's/^.*Z //' | sort | uniq -c
      2 test backend_cuda::copy_clear_device_tests::a_recorded_clear_zeroes_only_the_range ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_device_tests::a_recorded_copy_takes_effect_only_after_submit ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_device_tests::a_rejected_record_leaves_the_encoder_usable ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_device_tests::a_zero_length_record_changes_nothing ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_device_tests::freeing_a_buffer_right_after_submit_keeps_the_result ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_device_tests::freeing_a_recorded_buffer_before_submit_rejects_the_submit ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_device_tests::records_take_effect_in_record_order ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_device_tests::submit_many_runs_each_encoder_in_the_given_order ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_gate_device_tests::buffers_freed_while_their_copy_is_gated_are_held_until_completion ... ignored, requires a CUDA device
      2 test backend_cuda::copy_clear_gate_device_tests::held_buffers_are_released_at_each_recovery_point ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | バッファのコピーとクリアを記録して投入で積み、使用中のバッファの解放を保留する | #5250 |
| 2/3 | バッファのコピーとクリアの device の要る試験を ignore 付きで足す | #5251 |
| 3/3 | 完了の条件の確かめ | #5252 |

## 今の値の補い（base 25769d1）

記録時点の表と既存の節は書き換えない。以下は base `25769d1` に対するこの変更の後の今の値。所在は [cuda-wait-blocking](cuda-wait-blocking.md) の 1/3 と 2/3 の節（TASK-606）。

| 表の行 | 前の値 → 今の値 | 照らす試験 |
| ------ | --------------- | ---------- |
| 「device の要る試験（2/3）」と「振る舞いと照らす試験」の、保留したバッファを解放する呼び出しの列挙（`allocate`・`submit`・`submit_many`・`poll`） | その 4 つ → feature `native-blocking-wait` のビルドでは `wait_blocking` も、待った投入が終わっていれば保留したバッファを解放する | `the_wait_releases_held_buffers_and_calls_registered_callbacks`（device の要る試験） |
