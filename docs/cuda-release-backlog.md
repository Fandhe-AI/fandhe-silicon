# CUDA の背後実装の解放の保留の数・ゼロ化の完了待ち・回収の点（TASK-346）の所在と本リポの現状

TASK-346 の作業のうち、CUDA の背後実装の解放の保留の数（`pending_release_count`）・投入済みの表の件数・ゼロ化の完了を待つバッファと、保留したバッファを回収する呼び出しを照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-346。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `pending_release_count` | `crates/contract/src/backend_cuda.rs` の `impl MemoryOps for CudaDevice` の `pending_release_count` | `git grep -n "fn pending_release_count" -- crates/contract/src/backend_cuda.rs` |
| 解放の保留と門 | [cuda-fault-gate](cuda-fault-gate.md) | 同左 |
| 再利用の一覧とゼロ化 | [cuda-buffer-pool](cuda-buffer-pool.md) | 同左 |
| 試験 | `crates/contract/src/backend_cuda.rs` の `release_backlog_device_tests` | 下の「device の要る試験（1/3）」 |

## device の要る試験（1/3）

`crates/contract/src/backend_cuda.rs` の module `release_backlog_device_tests`（`#[cfg(all(test, feature = "fault-injection"))]`）。crate の中の投入済みの表と再利用の一覧の件数も読む。どの試験も本体を別のスレッドで流し、上限の時間で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `the_backlog_stays_within_nine_over_ten_thousand_rounds_and_ends_at_zero` | 確保・0 埋めの記録と投入・`free` を 10,000 回くり返す。各回の初めに、待っていない印が 8 つあれば、いちばん古い印を `poll` で `Complete` まで待つ。くり返しの後に残りの印を同じく待ち、最後の印にもう 1 回 `poll` を呼ぶ | 各回の後の `pending_release_count` は `Some` で 9 以下、投入済みの表の件数は 9 以下。最後の `poll` は `Complete` で、その後の `pending_release_count` は `Some(0)`、投入済みの表の件数は 0。2 つの数の最大を `max_pending_release_count=`・`max_submissions=` の 1 行で標準出力に出す。上限は 120 秒 |
| `buffers_waiting_for_zeroing_are_not_counted` | 門を仕込んで 0 埋めを投入し、そのバッファを `free` する。門を開けて別のバッファへ `upload` し、門を仕込んで空の投入をする。門を開けて空の投入を `Complete` まで待ち、`upload` の後に同じ長さ・用途で確保する | `free` の後は `Some(1)` で、ゼロ化の完了を待つバッファは 0。`upload` の後も `Some(1)`。空の投入の後は `Some(0)` で、ゼロ化の完了を待つバッファは 1、空の投入の `poll` は `Pending`、前の投入の `poll` は `Complete`。2 回目に門を仕込んでからここまでは 250 ms より早く終わる。確保の後は、ゼロ化の完了を待つバッファは 0 で、`Some(0)`。上限は 30 秒 |

| 項目 | 値 |
| ---- | -- |
| module | `release_backlog_device_tests` |
| `#[ignore]` | `requires a CUDA device` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib release_backlog_device_tests
```

2 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 解放の保留の数とゼロ化の完了待ちの device の要る試験を ignore 付きで足す | #4463 |
