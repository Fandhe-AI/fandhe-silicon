# CUDA の背後実装の `poll`・`poll_ex`（TASK-605）の所在と本リポの現状

TASK-605 の作業のうち、CUDA の背後実装の `CommandQueue::poll`・`CommandQueue::poll_ex` を公開の口から照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-605。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `poll` | `crates/contract/src/backend_cuda.rs` の `impl CommandQueue for CudaDevice` の `poll` | `git grep -n "fn poll(" -- crates/contract/src/backend_cuda.rs` |
| `poll_ex` の既定の本体 | `crates/core/src/execution/submit.rs` の `CommandQueue::poll_ex` | `git grep -n "fn poll_ex" -- crates/core/src/execution/submit.rs` |
| 門 | [cuda-fault-gate](cuda-fault-gate.md) | 同左 |
| 模擬のロスト | [cuda-simulated-device-lost](cuda-simulated-device-lost.md) | 同左 |
| 試験 | `crates/contract/tests/backend_cuda_poll.rs` | 下の「device の要る試験（1/2）」 |

## device の要る試験（1/2）

公開の口（`Device::queue`・`Device::fault_hooks`）だけを呼ぶ統合の試験。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_gated_submission_polls_pending_without_blocking` | 門を仕込んで空の投入をし、`poll`・`poll_ex` を呼び、門を開けて `poll` を繰り返す | `poll` は `Pending`、`poll_ex` は `Pending` で、2 つの呼び出しは 250 ms より早く戻る。門を開けた後の `poll` は `Complete` |
| `a_finished_submission_polls_complete_through_both_calls` | 空の投入を 2 つする。1 つ目は `poll`、2 つ目は `poll_ex` を `Complete` まで繰り返し、もう一方を呼ぶ | 1 つ目の `poll_ex` は `Complete`、2 つ目の `poll` は `Complete` |
| `a_reclaimed_token_polls_complete_while_a_later_one_is_gated` | 1 つ目の投入を `poll` で `Complete` まで待ち、門を仕込んで 2 つ目を投入し、1 つ目に `poll`・`poll_ex` を呼ぶ。門を開けて 2 つ目を `Complete` まで待ち、1 つ目にもう一度呼ぶ | 2 つ目の `poll` は `Pending`。1 つ目はどちらの時点でも `poll` が `Complete`、`poll_ex` が `Complete` |
| `tokens_from_before_a_simulated_loss_poll_lost` | 完了した印と門で止めた印に `poll`・`poll_ex` を呼び、`simulate_device_lost(DeviceLostReason::Unknown)` の後にもう一度呼び、門を開けて門で止めた印にもう一度呼ぶ | ロストの前は、完了した印が `Complete`・`Complete`、門で止めた印が `Pending`・`Pending`。ロストの後は 2 つの印とも `Lost`・`Lost`。門を開けた後も門で止めた印は `Lost`・`Lost` |
| `unissued_sequences_poll_lost` | 完了した印の番号を 0・最後の番号 + 1・`u64::MAX` に替えた印に `poll`・`poll_ex` を呼ぶ | 3 通りとも `poll` は `Lost`、`poll_ex` は `Lost`。元の印は `Complete`・`Complete` |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_poll.rs` |
| `required-features` | `backend-cuda`・`fault-injection`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_poll
```

5 件が `ignored` と数えられる。

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-605。ポインタのみ）。

時点のコミット: `1fc50468`。`docs/spec` の gitlink は `bfc545a1462bd69953ff876a1ff231348d3352f8`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `crates/contract/tests/backend_cuda_poll.rs` の行頭の `#[test]`・`#[ignore` を `grep -c` で数える | `#[test]` 5・`#[ignore` 5 |
| (2) | 受け手の試験の名前 | 5 本の関数を 1 つずつ `git grep -n "fn <名前>" -- crates` で引き、`git ls-files` で試験のファイルを引く | 下の出力 (2) |
| (3) | `poll_ex` の本体の所在 | `git grep -n "fn poll_ex"` を `crates/contract/src/backend_cuda.rs` と `crates/core/src/execution/submit.rs` に当てる | 下の出力 (3) |
| (4) | device に触らない試験 | `--test backend_cuda_poll`（`backend-cuda,fault-injection`）・core の `--test poll_ex_default`・`--test backend_cuda_unsafe_count` を流す | 下の出力 (4) |
| (5) | 依存 0 と `[[test]]` の 1 節 | `85c5b226` から HEAD までの `Cargo.lock`・`deps-approved.toml`・`Cargo.toml` の差分、`crates/contract/Cargo.toml` の差分の行、`make deps-check` | 下の出力 (5) |
| (6) | unsafe の数と本体の差 | `85c5b226` から HEAD までの `backend_cuda_unsafe_count.rs`・`crates/contract/src`・`crates/core` の差分と、`backend_cuda.rs` の `unsafe` の行数 | 下の出力 (6) |
| (7) | CI | 時点のコミットの main の push の run のジョブの conclusion と headSha、`verify` のログの 5 本の `ignored` の行 | 下の出力 (7) |

(5)・(6) の `85c5b226` は、1/2 の 1 つ前のコミット。

出力 (2):

```text
$ git grep -n "fn a_gated_submission_polls_pending_without_blocking" -- crates
crates/contract/tests/backend_cuda_poll.rs:121:fn a_gated_submission_polls_pending_without_blocking() {
$ git grep -n "fn a_finished_submission_polls_complete_through_both_calls" -- crates
crates/contract/tests/backend_cuda_poll.rs:144:fn a_finished_submission_polls_complete_through_both_calls() {
$ git grep -n "fn a_reclaimed_token_polls_complete_while_a_later_one_is_gated" -- crates
crates/contract/tests/backend_cuda_poll.rs:162:fn a_reclaimed_token_polls_complete_while_a_later_one_is_gated() {
$ git grep -n "fn tokens_from_before_a_simulated_loss_poll_lost" -- crates
crates/contract/tests/backend_cuda_poll.rs:184:fn tokens_from_before_a_simulated_loss_poll_lost() {
$ git grep -n "fn unissued_sequences_poll_lost" -- crates
crates/contract/tests/backend_cuda_poll.rs:217:fn unissued_sequences_poll_lost() {
$ git ls-files crates/contract/tests/backend_cuda_poll.rs
crates/contract/tests/backend_cuda_poll.rs
```

出力 (3):

```text
$ git grep -n "fn poll_ex" -- crates/contract/src/backend_cuda.rs | wc -l
0
$ git grep -n "fn poll_ex" -- crates/core/src/execution/submit.rs
crates/core/src/execution/submit.rs:339:    fn poll_ex(&self, token: SubmitToken) -> CompletionStateEx {
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_poll
running 5 tests
test result: ok. 0 passed; 0 failed; 5 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-core --test poll_ex_default
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (5):

```text
$ git diff --stat 85c5b226..HEAD -- Cargo.lock deps-approved.toml Cargo.toml
$ git diff -U0 85c5b226..HEAD -- crates/contract/Cargo.toml | grep '^[-+]' | grep -v -e '^+++ ' -e '^--- '
+[[test]]
+name = "backend_cuda_poll"
+required-features = ["backend-cuda", "fault-injection"]
+
$ make deps-check
終了コード 0
```

出力 (6):

```text
$ git diff --stat 85c5b226..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs crates/contract/src crates/core
$ git show 85c5b226:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show HEAD:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
```

出力 (7):

```text
$ gh run view 44592557580 --json conclusion,headSha,jobs
conclusion: success
headSha: 1fc504680d26d423e96e367508a17ad01d84dca9
make verify (fmt + clippy + test): success
make verify + build-profiles (macos-latest): success
make deny (cargo-deny): success
$ gh run view 44592557580 --job <verify のジョブ> --log | grep -E 'test <5 本の名前> \.\.\. ignored, requires a CUDA device' | sort | uniq -c
      2 test a_finished_submission_polls_complete_through_both_calls ... ignored, requires a CUDA device
      2 test a_gated_submission_polls_pending_without_blocking ... ignored, requires a CUDA device
      2 test a_reclaimed_token_polls_complete_while_a_later_one_is_gated ... ignored, requires a CUDA device
      2 test tokens_from_before_a_simulated_loss_poll_lost ... ignored, requires a CUDA device
      2 test unissued_sequences_poll_lost ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | poll・poll_ex の device の要る試験を ignore 付きで足す | #5192 |
| 2/2 | 完了の条件の確かめ | #5193 |
