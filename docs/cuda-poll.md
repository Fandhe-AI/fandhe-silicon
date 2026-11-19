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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | poll・poll_ex の device の要る試験を ignore 付きで足す | #5192 |
