# CUDA の背後実装の模擬のロストの後の操作の拒否と作り直し（TASK-188）の所在と本リポの現状

TASK-188 の作業のうち、CUDA の背後実装の模擬のロストの後の操作の拒否の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-188。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_cuda.rs` | 同左 |
| `lost_result` | `lost` と `generation` を受け、device に触らずに `Ok(())` か `DeviceLost { generation }` を返す | `git grep -n -F "fn lost_result(" -- crates/contract/src/backend_cuda.rs` |
| `check_not_lost` | `CudaDevice` のメソッド。`is_lost` の値と呼んだ時点の `generation()` を `lost_result` に渡す。`fault-injection` の無いビルドでは `is_lost` は常に偽 | `git grep -n -F -e "fn check_not_lost(" -e "fn is_lost(" -- crates/contract/src/backend_cuda.rs` |
| 模擬のロストの口 | `docs/cuda-simulated-device-lost.md` | 同左 |

## ロストの後の操作の拒否（1/3）

device に触らない試験（`crates/contract/src/backend_cuda.rs` の `lost_check_tests`）は次のとおり。

| 試験 | 内容 |
| ---- | ---- |
| `lost_result_passes_when_not_lost` | `lost` が偽なら複数の世代で `Ok(())` |
| `lost_result_carries_the_given_generation_when_lost` | `lost` が真なら複数の世代で、その世代を載せた `DeviceLost` |

`backend_cuda::device_tests::lost::a_handle_from_before_the_loss_can_still_be_freed_once` は、ロストの後のロスト前のハンドルの `upload` を `Err(DeviceLost { generation: d.generation() })` で照らす。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::lost_check_tests
```

2 件が通る。

## device の要る試験（2/3）

どの試験も CUDA の device に触るので `#[ignore]` を付け、通常の確認では走らず `ignored` になる。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。

| 試験 | 置き場 | 呼ぶ口 | 照らす値 |
| ---- | ------ | ------ | -------- |
| `two_losses_call_the_callback_once_and_move_the_generation_once` | `lost_invalidation_device_tests` | 具体型の device の `set_lost_callback` と `fault_hooks` | 通知の列が `[Unknown]` の 1 件。1 回目の後に世代が進み、2 回目の後は同じ値 |
| `the_listed_operations_issue_nothing_after_the_loss` | `lost_invalidation_device_tests` | 具体型の device | 並べた操作が `Err(DeviceLost { generation: generation() })`、`poll`・`poll_ex` は `Lost`。0 の長さ・表に無いハンドル・開いていない記録先・0 の要素・空でない `wait`・空の `submit_many` も同じ。2 回繰り返しても同じで、バッファの表の件数・次の識別子・投入済みの表の次の通し番号は変わらず、空の記録先の記録の数は 0 のまま。ロスト前のバッファの `free` は表から外れる |
| `a_wait_that_spans_the_loss_returns_device_lost` | `lost_invalidation_device_tests` | 具体型の device の `arm_gate`・`open_gate` | 門で止めた投入を待っている間に模擬のロストを起こし、門を開けると、待ちが `Err(DeviceLost { generation: generation() })` を返す。同じ印の `poll` は `Lost` |
| `a_new_request_after_the_loss_runs_a_dispatch_and_reads_back` | `lost_invalidation_device_tests` | `CudaDeviceProvider::request` の新しい device | 世代がロストの前後のどちらよりも大きい。確保・`upload`・コピーと dispatch を記録した記録先の投入・読み戻しが期待と一致。ロストした device の `allocate` は `DeviceLost` のまま |
| `the_listed_operations_are_lost_through_the_public_device` | `crates/contract/tests/backend_cuda_device_lost.rs` | 公開の `Device` の `memory()`・`queue()`・`compute()` | 並べた操作が `DeviceLost`、`poll`・`poll_ex` は `Lost`。dispatch を記録した記録先の `submit`・`submit_many` も `Unsupported` ではなく `DeviceLost` |
| `a_new_request_after_the_loss_round_trips_through_a_copy` | `crates/contract/tests/backend_cuda_device_lost.rs` | 公開の `Device` | 世代がロストした device の後の値より大きい。確保・`upload`・コピーを記録した記録先の `submit`・読み戻しが一致。ロストした device の `allocate` は `DeviceLost` のまま |

| 置き場 | cfg・feature |
| ------ | ------------ |
| `crates/contract/src/backend_cuda.rs` の `lost_invalidation_device_tests` | `test`・`native-blocking-wait`・`fault-injection` |
| `crates/contract/tests/backend_cuda_device_lost.rs` | `[[test]]` の `required-features` は `backend-cuda`・`native-blocking-wait`・`fault-injection` |

上の 2 本目・3 本目と統合の試験の 1 本目が照らす、ロストの後の値は次のとおり。`generation` は呼んだ時点の `generation()` の値。

| 操作 | ロストの後に返す値 | 2 本目で同じ値を照らす入力 |
| ---- | ------------------ | -------------------------- |
| `allocate` | `Err(DeviceLost { generation })` | 0 の長さ |
| `upload` | `Err(DeviceLost { generation })` | 表に無いハンドル |
| `download_blocking`・`download_async` | `Err(DeviceLost { generation })`（`download_async` は callback に渡す） | — |
| `copy_buffer_to_buffer`・`clear_buffer` | `Err(DeviceLost { generation })` | 開いていない記録先 |
| `dispatch`・`dispatch_ex`・`dispatch_checked` | `Err(DeviceLost { generation })`（`dispatch_checked` は宣言が合う入力） | 開いていない記録先と 0 の要素（`dispatch_ex`） |
| `submit`・`submit_many` | `Err(DeviceLost { generation })` | dispatch を記録した記録先、空でない `wait`（`submit`）、空の `submit_many` |
| `wait_blocking` | `Err(DeviceLost { generation })`（待っている間のロストを含む。3 本目） | — |
| `poll`・`poll_ex` | `Lost` | ロストの前の印（`poll`・`poll_ex`）と、今の世代の印（`poll`） |

通常の確認:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --lib backend_cuda::lost_invalidation_device_tests
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --test backend_cuda_device_lost
```

1 つ目は 4 件、2 つ目は 2 件が `ignored` になる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-188。ポインタのみ）。

時点のコミット: `55723d7c`。`docs/spec` の gitlink は `ac56f928301ffca0c15d857b56302916a3cce0fb`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `55723d7c` の `crates/contract/src/backend_cuda.rs` の `lost_check_tests`・`lost_invalidation_device_tests` と `crates/contract/tests/backend_cuda_device_lost.rs` に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n` で 9 本の名前を 1 つずつ引く | 出力 (2) |
| (3) | 口の所在 | `git grep -n -F` で `fn lost_result(`・`fn check_not_lost(`・`let result = self.check_not_lost().and(result);`・`if self.is_lost() {` を 1 つずつ引き、`self.check_not_lost()` の行の数を数える | 出力 (3) |
| (4) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 4 つの実行 | 出力 (4) |
| (5) | 依存の差と `[[test]]` の 1 節 | `2edc2327^ 55723d7c` の差と `make deps-check` | 出力 (5) |
| (6) | `backend_cuda_unsafe_count` の試験のファイルと core の差 | `2edc2327^ 55723d7c` の差 | 出力 (6) |
| (7) | CI | 2/3 のマージの後の main の push の run | 出力 (7) |

`2edc2327^` は 1/3 の squash `2edc2327` の親。(4) の `cargo test` と (5) の `make deps-check` は作業木で流した。

出力 (1):

```text
$ git show 55723d7c:crates/contract/src/backend_cuda.rs | awk '/^mod lost_check_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
2
$ git show 55723d7c:crates/contract/src/backend_cuda.rs | awk '/^mod lost_check_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
0
$ git show 55723d7c:crates/contract/src/backend_cuda.rs | awk '/^mod lost_invalidation_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
4
$ git show 55723d7c:crates/contract/src/backend_cuda.rs | awk '/^mod lost_invalidation_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
4
$ git show 55723d7c:crates/contract/tests/backend_cuda_device_lost.rs | grep -c '^[[:space:]]*#\[test\]'
2
$ git show 55723d7c:crates/contract/tests/backend_cuda_device_lost.rs | grep -c '^[[:space:]]*#\[ignore'
2
```

出力 (2):

```text
$ git grep -n "fn lost_result_passes_when_not_lost(" 55723d7c -- crates/contract
55723d7c:crates/contract/src/backend_cuda.rs:10803:    fn lost_result_passes_when_not_lost() {
55723d7c:crates/contract/src/backend_metal.rs:14840:    fn lost_result_passes_when_not_lost() {
$ git grep -n "fn lost_result_carries_the_given_generation_when_lost(" 55723d7c -- crates/contract
55723d7c:crates/contract/src/backend_cuda.rs:10810:    fn lost_result_carries_the_given_generation_when_lost() {
55723d7c:crates/contract/src/backend_metal.rs:14847:    fn lost_result_carries_the_given_generation_when_lost() {
$ git grep -n "fn two_losses_call_the_callback_once_and_move_the_generation_once(" 55723d7c -- crates/contract
55723d7c:crates/contract/src/backend_cuda.rs:10919:    fn two_losses_call_the_callback_once_and_move_the_generation_once() {
55723d7c:crates/contract/src/backend_metal.rs:10420:        fn two_losses_call_the_callback_once_and_move_the_generation_once() {
$ git grep -n "fn the_listed_operations_issue_nothing_after_the_loss(" 55723d7c -- crates/contract
55723d7c:crates/contract/src/backend_cuda.rs:10941:    fn the_listed_operations_issue_nothing_after_the_loss() {
55723d7c:crates/contract/src/backend_metal.rs:10435:        fn the_listed_operations_issue_nothing_after_the_loss() {
$ git grep -n "fn a_wait_that_spans_the_loss_returns_device_lost(" 55723d7c -- crates/contract
55723d7c:crates/contract/src/backend_cuda.rs:11094:    fn a_wait_that_spans_the_loss_returns_device_lost() {
$ git grep -n "fn a_new_request_after_the_loss_runs_a_dispatch_and_reads_back(" 55723d7c -- crates/contract
55723d7c:crates/contract/src/backend_cuda.rs:11121:    fn a_new_request_after_the_loss_runs_a_dispatch_and_reads_back() {
55723d7c:crates/contract/src/backend_metal.rs:10564:        fn a_new_request_after_the_loss_runs_a_dispatch_and_reads_back() {
$ git grep -n "fn the_listed_operations_are_lost_through_the_public_device(" 55723d7c -- crates/contract
55723d7c:crates/contract/tests/backend_cuda_device_lost.rs:100:fn the_listed_operations_are_lost_through_the_public_device() {
55723d7c:crates/contract/tests/backend_metal_device_lost.rs:79:fn the_listed_operations_are_lost_through_the_public_device() {
$ git grep -n "fn a_new_request_after_the_loss_round_trips_through_a_copy(" 55723d7c -- crates/contract
55723d7c:crates/contract/tests/backend_cuda_device_lost.rs:171:fn a_new_request_after_the_loss_round_trips_through_a_copy() {
$ git grep -n "fn a_handle_from_before_the_loss_can_still_be_freed_once(" 55723d7c -- crates/contract
55723d7c:crates/contract/src/backend_cuda.rs:5665:        fn a_handle_from_before_the_loss_can_still_be_freed_once() {
55723d7c:crates/contract/src/backend_metal.rs:9295:            fn a_handle_from_before_the_loss_can_still_be_freed_once() {
```

出力 (3):

```text
$ git grep -n -F "fn lost_result(" 55723d7c -- crates/contract/src/backend_cuda.rs
55723d7c:crates/contract/src/backend_cuda.rs:2042:fn lost_result(lost: bool, generation: u64) -> Result<(), ContractError> {
$ git grep -n -F "fn check_not_lost(" 55723d7c -- crates/contract/src/backend_cuda.rs
55723d7c:crates/contract/src/backend_cuda.rs:2568:    fn check_not_lost(&self) -> Result<(), ContractError> {
$ git grep -n -F "let result = self.check_not_lost().and(result);" 55723d7c -- crates/contract/src/backend_cuda.rs
55723d7c:crates/contract/src/backend_cuda.rs:3829:        let result = self.check_not_lost().and(result);
$ git grep -n -F "if self.is_lost() {" 55723d7c -- crates/contract/src/backend_cuda.rs
55723d7c:crates/contract/src/backend_cuda.rs:3744:        if self.is_lost() {
$ git grep -n -F "self.check_not_lost()" 55723d7c -- crates/contract/src/backend_cuda.rs | wc -l
9
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::lost_check_tests 2>&1 | grep -E '^(running|test result:)'
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 149 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --lib backend_cuda::lost_invalidation_device_tests 2>&1 | grep -E '^(running|test result:)'
running 4 tests
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 227 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --test backend_cuda_device_lost 2>&1 | grep -E '^(running|test result:)'
running 2 tests
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

出力 (5):

```text
$ git diff --stat 2edc2327^ 55723d7c -- Cargo.lock deps-approved.toml Cargo.toml
$ git diff -U0 2edc2327^ 55723d7c -- crates/contract/Cargo.toml | grep '^[-+]' | grep -v -e '^+++ ' -e '^--- '
+[[test]]
+name = "backend_cuda_device_lost"
+required-features = ["backend-cuda", "native-blocking-wait", "fault-injection"]
+
$ make deps-check; echo exit=$?
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
exit=0
```

出力 (6):

```text
$ git diff --stat 2edc2327^ 55723d7c -- crates/contract/tests/backend_cuda_unsafe_count.rs crates/core
```

出力 (7):

```text
$ gh run list --branch main --event push --commit 55723d7c27fd3c991f43827183759ab458ab2b09 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44888466640
$ gh run view 44888466640 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
55723d7c27fd3c991f43827183759ab458ab2b09
make verify + build-profiles (macos-latest): success
make deny (cargo-deny): success
make verify (fmt + clippy + test): success
$ gh run view 44888466640 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136798832873
$ gh run view 44888466640 --job 136798832873 --log | grep -oE 'test [a-z_:]*(lost_result_passes_when_not_lost|lost_result_carries_the_given_generation_when_lost|two_losses_call_the_callback_once_and_move_the_generation_once|the_listed_operations_issue_nothing_after_the_loss|a_wait_that_spans_the_loss_returns_device_lost|a_new_request_after_the_loss_runs_a_dispatch_and_reads_back|the_listed_operations_are_lost_through_the_public_device|a_new_request_after_the_loss_round_trips_through_a_copy) \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      2 test a_new_request_after_the_loss_round_trips_through_a_copy ... ignored, requires a CUDA device
      6 test backend_cuda::lost_check_tests::lost_result_carries_the_given_generation_when_lost ... ok
      6 test backend_cuda::lost_check_tests::lost_result_passes_when_not_lost ... ok
      2 test backend_cuda::lost_invalidation_device_tests::a_new_request_after_the_loss_runs_a_dispatch_and_reads_back ... ignored, requires a CUDA device
      2 test backend_cuda::lost_invalidation_device_tests::a_wait_that_spans_the_loss_returns_device_lost ... ignored, requires a CUDA device
      2 test backend_cuda::lost_invalidation_device_tests::the_listed_operations_issue_nothing_after_the_loss ... ignored, requires a CUDA device
      2 test backend_cuda::lost_invalidation_device_tests::two_losses_call_the_callback_once_and_move_the_generation_once ... ignored, requires a CUDA device
      8 test backend_metal::lost_check_tests::lost_result_carries_the_given_generation_when_lost ... ok
      8 test backend_metal::lost_check_tests::lost_result_passes_when_not_lost ... ok
      2 test the_listed_operations_are_lost_through_the_public_device ... ignored, requires a CUDA device
```

## device の要る試験の強め（4/5）

上の「device の要る試験（2/3）」の節の表と文は記録時点の値で、書き換えていない。4/5 の後の `a_wait_that_spans_the_loss_returns_device_lost` の形は次のとおり。本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。

| 試験 | 置き場 | 呼ぶ口 | 照らす値 |
| ---- | ------ | ------ | -------- |
| `a_wait_that_spans_the_loss_returns_device_lost` | `lost_invalidation_device_tests` | 具体型の device の `arm_gate`・`submit`・`wait_blocking`・`fault_hooks`・`open_gate`・`poll` と、投入済みの表にある完了の印の持ち手の数を読む補助 `event_holders` | 門を仕込んだ空の投入の後の持ち手の数が `Some(1)`。別のスレッドで `wait_blocking` を呼び、持ち手の数が `Some(2)` になるまで待ちが終わらない。門を仕込む前からの経過が、模擬のロストの前に `GATE_EXPIRY / 2` より短い。模擬のロストの後に門を開けると、待ちが `Err(DeviceLost { generation: generation() })` を返す。同じ印の `poll` は `Lost` |

`crates/contract/src/backend_cuda.rs` の module の `//!` は、`download_blocking`・`wait_blocking` が feature `native-blocking-wait` も有効なビルドにあると書く。feature `native-blocking-wait` の無いビルドでは、`crates/contract/src/cfg_gate_doctest.rs` の `DownloadBlockingGate`・`WaitBlockingGate` の doctest が `compile_fail` になる。

確かめのコマンドは上の「device の要る試験（2/3）」の節の通常の確認と同じで、1 つ目は 4 件、2 つ目は 2 件が `ignored` と数えられる。

## 完了の条件の確かめ（5/5）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-188。ポインタのみ）。

時点のコミット: `a833fe26`。`docs/spec` の gitlink は `ac56f928301ffca0c15d857b56302916a3cce0fb`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `a833fe26` の `crates/contract/src/backend_cuda.rs` の `lost_invalidation_device_tests` と `crates/contract/tests/backend_cuda_device_lost.rs` に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験と補助の名前 | `git grep -n` で `a_wait_that_spans_the_loss_returns_device_lost`・`event_holders` を 1 つずつ引く | 出力 (2) |
| (3) | 口の所在 | `git grep -n -F` で 4 つの字句を 1 つずつ引く | 出力 (3) |
| (4) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 4 つの実行 | 出力 (4) |
| (5) | 句を受ける doctest | `cargo test -p fandhe-silicon-contract --doc Blocking` の 2 つの実行 | 出力 (5) |
| (6) | 依存と crates の差 | `a833fe26^ a833fe26` の差 | 出力 (6) |
| (7) | CI | 4/5 のマージの後の main の push の run | 出力 (7) |

`a833fe26^` は 4/5 の squash `a833fe26` の親。(4)・(5) の `cargo test` は作業木で流した。

出力 (1):

```text
$ git show a833fe26:crates/contract/src/backend_cuda.rs | awk '/^mod lost_invalidation_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
4
$ git show a833fe26:crates/contract/src/backend_cuda.rs | awk '/^mod lost_invalidation_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
4
$ git show a833fe26:crates/contract/tests/backend_cuda_device_lost.rs | grep -c '^[[:space:]]*#\[test\]'
2
$ git show a833fe26:crates/contract/tests/backend_cuda_device_lost.rs | grep -c '^[[:space:]]*#\[ignore'
2
```

出力 (2):

```text
$ git grep -n "fn a_wait_that_spans_the_loss_returns_device_lost(" a833fe26 -- crates/contract
a833fe26:crates/contract/src/backend_cuda.rs:11103:    fn a_wait_that_spans_the_loss_returns_device_lost() {
$ git grep -n "fn event_holders(" a833fe26 -- crates/contract
a833fe26:crates/contract/src/backend_cuda.rs:10915:    fn event_holders(d: &CudaDevice, sequence: u64) -> Option<usize> {
```

出力 (3):

```text
$ git grep -n -F 'WaitStep::Wait => subs.pending.get(&token.sequence).cloned(),' a833fe26 -- crates/contract/src/backend_cuda.rs
a833fe26:crates/contract/src/backend_cuda.rs:3816:                WaitStep::Wait => subs.pending.get(&token.sequence).cloned(),
$ git grep -n -F 'let result = self.check_not_lost().and(result);' a833fe26 -- crates/contract/src/backend_cuda.rs
a833fe26:crates/contract/src/backend_cuda.rs:3829:        let result = self.check_not_lost().and(result);
$ git grep -n -F 'check_wait_token(self.is_lost(), &token, self.id, self.current_generation())?;' a833fe26 -- crates/contract/src/backend_cuda.rs
a833fe26:crates/contract/src/backend_cuda.rs:3810:        check_wait_token(self.is_lost(), &token, self.id, self.current_generation())?;
$ git grep -n -F 'も有効なビルドにある（TASK-188）。' a833fe26 -- crates/contract/src/backend_cuda.rs
a833fe26:crates/contract/src/backend_cuda.rs:76://! `DeviceLost` を返す。`download_blocking`・`wait_blocking` は feature `native-blocking-wait` も有効なビルドにある（TASK-188）。
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --lib backend_cuda::lost_invalidation_device_tests 2>&1 | grep -E '^(running|test result:)|\.\.\. ignored, requires a CUDA device$'
running 4 tests
test backend_cuda::lost_invalidation_device_tests::a_new_request_after_the_loss_runs_a_dispatch_and_reads_back ... ignored, requires a CUDA device
test backend_cuda::lost_invalidation_device_tests::a_wait_that_spans_the_loss_returns_device_lost ... ignored, requires a CUDA device
test backend_cuda::lost_invalidation_device_tests::the_listed_operations_issue_nothing_after_the_loss ... ignored, requires a CUDA device
test backend_cuda::lost_invalidation_device_tests::two_losses_call_the_callback_once_and_move_the_generation_once ... ignored, requires a CUDA device
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 227 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --test backend_cuda_device_lost 2>&1 | grep -E '^(running|test result:)'
running 2 tests
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::wait_blocking_tests 2>&1 | grep -E '^(running|test result:)'
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 144 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (5):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --doc Blocking 2>&1 | grep -E 'cfg_gate_doctest::(DownloadBlockingGate|WaitBlockingGate)|^test result:'
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::WaitBlockingGate (line 43) - compile fail ... ok
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::DownloadBlockingGate (line 31) - compile fail ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.03s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --doc Blocking 2>&1 | grep -E 'cfg_gate_doctest::(DownloadBlockingGate|WaitBlockingGate)|^test result:'
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::WaitBlockingGate (line 43) ... ok
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::DownloadBlockingGate (line 31) ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s
```

出力 (6):

```text
$ git diff --stat a833fe26^ a833fe26 -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml crates/core crates/contract/tests
$ git diff --name-only a833fe26^ a833fe26 -- crates
crates/contract/src/backend_cuda.rs
```

出力 (7):

```text
$ gh run list --branch main --event push --commit a833fe261eeaf82f5f37a59429a9e588f76eb4f1 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44897887185
$ gh run view 44897887185 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
a833fe261eeaf82f5f37a59429a9e588f76eb4f1
make deny (cargo-deny): success
make verify + build-profiles (macos-latest): success
make verify (fmt + clippy + test): success
$ gh run view 44897887185 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136827333610
$ gh run view 44897887185 --job 136827333610 --log | grep -oE 'test [a-z_:]*(a_wait_that_spans_the_loss_returns_device_lost|lost_is_checked_before_the_owner_and_the_generation) \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      2 test backend_cuda::lost_invalidation_device_tests::a_wait_that_spans_the_loss_returns_device_lost ... ignored, requires a CUDA device
      6 test backend_cuda::wait_blocking_tests::lost_is_checked_before_the_owner_and_the_generation ... ok
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 模擬のロストの後の操作の拒否を実装する | #3834 |
| 2/3 | device の要る試験を足す | #3835 |
| 3/3 | 完了の条件の確かめ | #3836 |
| 4/5 | device の要る試験を強める | #8436 |
| 5/5 | 完了の条件の確かめ | #8437 |
