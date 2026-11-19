# CUDA の背後実装のハンドルの検証と `free` の無視（TASK-343）の所在と本リポの現状

TASK-343 の作業のうち、CUDA の背後実装の `upload`・`download_blocking`・`download_async`・`free` に、表に無い識別子・偽造・解放の後・世代違い・別の device のバッファのハンドルを渡す試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-343。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `upload`・`download_blocking`・`download_async` のハンドルの照らし | `crates/contract/src/backend_cuda.rs` の `impl CudaDevice` の `range` | `git grep -n "fn range(" -- crates/contract/src/backend_cuda.rs` |
| `free` | `crates/contract/src/backend_cuda.rs` の `impl MemoryOps for CudaDevice` の `free` と、関数 `remove_issued` | `git grep -n "fn free(" -- crates/contract/src/backend_cuda.rs` と `git grep -n "fn remove_issued" -- crates/contract/src/backend_cuda.rs` |
| ハンドルを作る口 | `crates/core/src/memory/buffer.rs` の `BufferHandle::__backend_new` | `git grep -n "fn __backend_new" -- crates/core/src/memory/buffer.rs` |
| 試験 | `crates/contract/tests/backend_cuda_handle_check.rs` と、下の表の既存の試験 | 下の「device の要る試験（1/2）」 |

## device の要る試験（1/2）

公開の口（`Device::memory`）だけを呼ぶ統合の試験。ハンドルは `BufferHandle::__backend_new` で作る。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `unknown_or_forged_handles_are_rejected_by_upload_and_both_downloads` | 自 device のバッファのハンドルから識別子を 0 と `u64::MAX` にしたもの・世代を 1 つ進めたもの・device の識別子を差し替えたものと、別に確保したバッファの `free` の前に写したハンドルを作り、`upload`・`download_blocking`・`download_async` に渡す | device の識別子を差し替えたものは 3 つの口とも `DeviceMismatch`、ほかの 4 通りは 3 つの口とも `InvalidArgument`。`download_async` の callback は 1 回だけ呼ばれる。自 device のバッファの中身は変わらない |
| `another_devices_buffer_is_rejected_by_download_async_and_the_same_id_stays_intact` | 2 つの device で 1 つずつ確保し、1 つ目の device のハンドルを 2 つ目の device の `download_async` に渡す | 2 つのハンドルは device の識別子が違い、識別子が同じ。callback は 1 回だけ `DeviceMismatch` を受ける。2 つ目の device のバッファの中身は変わらない |
| `freeing_unknown_or_forged_handles_is_ignored_and_keeps_own_buffers` | 1 本目と同じ 5 通りのハンドルを `free` に渡す | 自 device の 2 つのバッファは中身を保ち、その後の `upload`・`download_blocking` も通る |

同じ口と入力を照らす既存の試験:

| 試験 | 所在 | 照らす値 |
| ---- | ---- | -------- |
| `two_devices_do_not_accept_each_others_buffers` | `crates/contract/tests/backend_cuda_device.rs` | 2 つの device の最初のバッファは識別子が同じ。別の device のハンドルの `upload`・`download_blocking` は `DeviceMismatch` で、自 device の同じ識別子のバッファの中身は変わらない |
| `upload_to_a_freed_handle_is_rejected_as_invalid_argument` | 同上 | `free` の前に写したハンドルの `upload`・`download_blocking` は `InvalidArgument` |
| `freeing_another_devices_handle_keeps_the_buffer_with_the_same_id` | 同上 | 別の device のハンドルを `free` に渡した後も、自 device の同じ識別子のバッファの中身は変わらず、払い出し元の device のバッファも読める |
| `a_handle_that_differs_only_in_generation_is_rejected_and_ignored_by_free` | 同上 | 世代だけが違うハンドルの `upload` は `InvalidArgument`。そのハンドルを `free` に渡した後も、元のハンドルの中身が読める |
| `remove_issued_matches_the_issue_generation_not_the_current_one`・`remove_issued_ignores_forged_ids_and_an_empty_table` | `crates/contract/src/backend_cuda.rs` の `mod tests`（device に触らない） | 表に無い識別子・払い出しの世代が違う識別子・空の表では何も外さず、ほかの項目を残す。一致した項目は 1 回だけ外す |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_handle_check.rs` |
| `required-features` | `backend-cuda`・`native-blocking-wait`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --test backend_cuda_handle_check
```

3 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | ハンドルの検証と free の無視の device の要る試験を ignore 付きで足す | #4451 |
