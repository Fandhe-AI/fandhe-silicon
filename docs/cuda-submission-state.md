# CUDA の背後実装の投入状態と資源の寿命（TASK-347）の所在と本リポの現状

TASK-347 の作業のうち、CUDA の背後実装で、門で止めた投入に使ったバッファの解放の保留が待ちの `Timeout` と `poll_ex` の後も続くことと、dispatch を記録した記録先を投入せずに捨てたときの投入の回数と開いている記録先の数を照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-347。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_cuda.rs` | 同左 |
| 解放の保留の数 | [cuda-fault-gate](cuda-fault-gate.md) | 同左 |
| `wait_blocking` | [cuda-wait-blocking](cuda-wait-blocking.md) | 同左 |
| `poll`・`poll_ex` | [cuda-poll](cuda-poll.md) | 同左 |
| 記録先の集合と投入 | [cuda-command-submit](cuda-command-submit.md) | 同左 |
| dispatch の記録 | [cuda-dispatch-record](cuda-dispatch-record.md) | 同左 |

## 投入状態と資源の寿命の device の要る試験（1/2）

どの試験も CUDA の device に触るので `#[ignore]` を付け、通常の確認では走らず `ignored` になる。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。

統合の試験（公開の口の `Device::memory`・`Device::queue`・`Device::fault_hooks`）:

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_timeout_keeps_the_freed_source_of_a_gated_copy_held` | 門を仕込んでコピーを投入し、元を `free` してから、期限 100 ms の `wait_blocking`・`poll_ex`・`open_gate`・空の投入を呼ぶ | `free` の後の `pending_release_count` は `Some(1)`。`wait_blocking` は 100 ms 以上待って `Err(Timeout)` で、その後も `Some(1)`。`poll_ex` は `Pending` で、その後も `Some(1)`。門を開けると `poll_ex` が `Complete` になり、空の投入の後は `Some(0)` で、コピーの先は元の内容 |
| `sources_behind_a_gated_copy_stay_held_until_completion_is_observed` | 門を仕込んでコピーを投入し、門の無いコピーを続けて投入し、2 つの元を `free` してから、後ろの投入への期限 100 ms の `wait_blocking`・2 つの投入への `poll_ex`・`open_gate`・空の投入を呼ぶ | 2 つの `free` の後は `Some(2)`。2 つの投入の `poll_ex` は `wait_blocking` の `Err(Timeout)` の前も後も `Pending` で、`Some(2)` のまま。門を開けると 2 つとも `Complete` になり、空の投入の後は `Some(0)` で、2 つのコピーの先はそれぞれの元の内容 |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_submission_state.rs` |
| `required-features` | `backend-cuda`・`fault-injection`・`native-blocking-wait`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

lib の試験（`#[cfg(test)]` の数の口 `test_submitted_count`・`test_open_encoder_count` を使う）:

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing` | 4 語に定数を書く dispatch を記録した記録先を投入せずに捨て、空の投入を待ち、同じ dispatch を記録した別の記録先を投入して待つ | 開いている記録先の数は作成の前 0、記録の後 1、捨てた後 0。記録の後と捨てた後の空の投入の後は出力が 0 のまま。捨てた後の投入の回数は前と同じで、`pending_release_count` は `Some(0)`。別の記録先の投入の後は出力が定数になり、投入の回数は前より 2 多い |
| `a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held` | 門を仕込んで入力を出力に写す dispatch を投入し、入力を `free` してから、期限 100 ms の `wait_blocking`・`poll_ex`・`open_gate`・期限なしの `wait_blocking` を呼ぶ | `free` の後は `Some(1)`。`wait_blocking` は `Err(Timeout)` で、その後も `Some(1)`。`poll_ex` は `Pending` で、その後も `Some(1)`。門を開けて期限なしの `wait_blocking` が `Ok(())` を返した後は `Some(0)` で、出力は入力の写し |

| 置き場 | cfg・feature |
| ------ | ------------ |
| `crates/contract/src/backend_cuda.rs` の `submission_state_device_tests` | `test`・`native-blocking-wait`・`fault-injection` |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_submission_state
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --lib backend_cuda::submission_state_device_tests
```

どちらも 2 件が `ignored` になる。

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-347。ポインタのみ）。

時点のコミット: `d06c4fc9`。`docs/spec` の gitlink は `ac56f928301ffca0c15d857b56302916a3cce0fb`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `d06c4fc9` の統合の試験のファイルと `submission_state_device_tests` に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n` で 4 つの関数名を 1 つずつ引く | 出力 (2) |
| (3) | 口の所在 | `git grep -n -F` で 4 つの字句を 1 つずつ引く | 出力 (3) |
| (4) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 3 つの実行 | 出力 (4) |
| (5) | 依存と crates の差 | `d06c4fc9^ d06c4fc9` の差 | 出力 (5) |
| (6) | CI | 1/2 のマージの後の main の push の run | 出力 (6) |

`d06c4fc9^` は 1/2 の squash `d06c4fc9` の親。(4) の `cargo test` は作業木で流した。

出力 (1):

```text
$ git show d06c4fc9:crates/contract/tests/backend_cuda_submission_state.rs | grep -c '^[[:space:]]*#\[test\]'
2
$ git show d06c4fc9:crates/contract/tests/backend_cuda_submission_state.rs | grep -c '^[[:space:]]*#\[ignore'
2
$ git show d06c4fc9:crates/contract/src/backend_cuda.rs | awk '/^mod submission_state_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
2
$ git show d06c4fc9:crates/contract/src/backend_cuda.rs | awk '/^mod submission_state_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
2
```

出力 (2):

```text
$ git grep -n "fn a_timeout_keeps_the_freed_source_of_a_gated_copy_held(" d06c4fc9 -- crates/contract/tests/backend_cuda_submission_state.rs
d06c4fc9:crates/contract/tests/backend_cuda_submission_state.rs:135:fn a_timeout_keeps_the_freed_source_of_a_gated_copy_held() {
$ git grep -n "fn sources_behind_a_gated_copy_stay_held_until_completion_is_observed(" d06c4fc9 -- crates/contract/tests/backend_cuda_submission_state.rs
d06c4fc9:crates/contract/tests/backend_cuda_submission_state.rs:174:fn sources_behind_a_gated_copy_stay_held_until_completion_is_observed() {
$ git grep -n "fn dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing(" d06c4fc9 -- crates/contract/src/backend_cuda.rs
d06c4fc9:crates/contract/src/backend_cuda.rs:11537:    fn dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing() {
$ git grep -n "fn a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held(" d06c4fc9 -- crates/contract/src/backend_cuda.rs
d06c4fc9:crates/contract/src/backend_cuda.rs:11574:    fn a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held() {
```

出力 (3):

```text
$ git grep -n -F 'fn pending_release_count(&self) -> Option<u64> {' d06c4fc9 -- crates/contract/src/backend_cuda.rs
d06c4fc9:crates/contract/src/backend_cuda.rs:3590:    fn pending_release_count(&self) -> Option<u64> {
$ git grep -n -F 'const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);' d06c4fc9 -- crates/contract/src/backend_cuda.rs
d06c4fc9:crates/contract/src/backend_cuda.rs:1764:const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);
$ git grep -n -F '    fn test_submitted_count(&self) -> u64 {' d06c4fc9 -- crates/contract/src/backend_cuda.rs
d06c4fc9:crates/contract/src/backend_cuda.rs:2635:    fn test_submitted_count(&self) -> u64 {
$ git grep -n -F '    fn test_open_encoder_count(&self) -> usize {' d06c4fc9 -- crates/contract/src/backend_cuda.rs
d06c4fc9:crates/contract/src/backend_cuda.rs:2641:    fn test_open_encoder_count(&self) -> usize {
$ git grep -n 'fn poll_ex' d06c4fc9 -- crates/contract/src/backend_cuda.rs | wc -l
0
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_submission_state 2>&1 | grep -E '^(running|test result:)|\.\.\. ignored, requires a CUDA device$'
running 2 tests
test a_timeout_keeps_the_freed_source_of_a_gated_copy_held ... ignored, requires a CUDA device
test sources_behind_a_gated_copy_stay_held_until_completion_is_observed ... ignored, requires a CUDA device
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --lib backend_cuda::submission_state_device_tests 2>&1 | grep -E '^(running|test result:)|\.\.\. ignored, requires a CUDA device$'
running 2 tests
test backend_cuda::submission_state_device_tests::a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held ... ignored, requires a CUDA device
test backend_cuda::submission_state_device_tests::dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing ... ignored, requires a CUDA device
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 233 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

出力 (5):

```text
$ git diff --stat d06c4fc9^ d06c4fc9 -- Cargo.lock deps-approved.toml Cargo.toml crates/core crates/contract/tests/backend_cuda_unsafe_count.rs
$ git diff -U0 d06c4fc9^ d06c4fc9 -- crates/contract/Cargo.toml | grep '^[-+]' | grep -v -e '^+++ ' -e '^--- '
+[[test]]
+name = "backend_cuda_submission_state"
+required-features = ["backend-cuda", "fault-injection", "native-blocking-wait"]
+
$ git diff --name-only d06c4fc9^ d06c4fc9 -- crates
crates/contract/Cargo.toml
crates/contract/src/backend_cuda.rs
crates/contract/tests/backend_cuda_submission_state.rs
```

出力 (6):

```text
$ gh run list --branch main --event push --commit d06c4fc90d771de0e16647cc441b37ff22f45356 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44931362033
$ gh run view 44931362033 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
d06c4fc90d771de0e16647cc441b37ff22f45356
make verify (fmt + clippy + test): success
make verify + build-profiles (macos-latest): success
make deny (cargo-deny): success
$ gh run view 44931362033 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136928486672
$ gh run view 44931362033 --job 136928486672 --log | grep -oE 'test [a-z_:]*(a_timeout_keeps_the_freed_source_of_a_gated_copy_held|sources_behind_a_gated_copy_stay_held_until_completion_is_observed|dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing|a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held) \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      2 test a_timeout_keeps_the_freed_source_of_a_gated_copy_held ... ignored, requires a CUDA device
      2 test backend_cuda::submission_state_device_tests::a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held ... ignored, requires a CUDA device
      2 test backend_cuda::submission_state_device_tests::dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing ... ignored, requires a CUDA device
      2 test sources_behind_a_gated_copy_stay_held_until_completion_is_observed ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 投入状態と資源の寿命の device の要る試験を ignore 付きで足す | #4469 |
| 2/2 | 完了の条件の確かめ | #4470 |
