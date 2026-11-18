# Metal の背後実装の `upload`・`download_*` の用途の検査（TASK-316）の所在と本リポの現状

TASK-316 の作業のうち、Metal の背後実装の `MemoryOps::upload`・`download_blocking`・`download_async` が行う用途の検査の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-316。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の 1 ファイル | `crates/contract/src/backend_metal.rs` |
| 純粋な関数 | `usage_access_range`（`imp` の外） | `grep -n 'fn usage_access_range' crates/contract/src/backend_metal.rs` |
| 記録する欄 | `BufferEntry` の `usage`（`allocate` が受けた用途を入れる） | 同ファイルの `struct BufferEntry` |
| cfg | feature `backend-metal`。device に触る部分（`imp`）は macOS だけ | `crates/contract/src/lib.rs` |
| 試験 | 同ファイルの `usage_role_check_tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-metal --lib usage_role_check_tests` |
| device の取得・確保 | [metal-device-request.md](./metal-device-request.md)（TASK-158） | 同頁 |
| 用途と操作の対応 | [buffer-usage-check.md](./buffer-usage-check.md)（TASK-1796） | 同頁 |

## `upload`・`download_*` の用途の検査（1/3）

操作ごとの役割と、確保のときの用途に要る値:

| 操作 | 役割 | 要る用途 |
| ---- | ---- | -------- |
| `upload` | `UploadDestination` | `UPLOAD` |
| `download_blocking`・`download_async` | `ReadbackSource` | `DOWNLOAD` |

照らす順:

| 順 | 照らすもの | 返す値 |
| -- | ---------- | ------ |
| 1 | 別の device のハンドル | `DeviceMismatch` |
| 2 | 表にあるか | `InvalidArgument` |
| 3 | 払い出しの世代 | `InvalidArgument` |
| 4 | 記録した用途が役割を満たすか | `InvalidArgument` |
| 5 | 最後に使った投入が終わっているか | `InvalidArgument` |
| 6 | 範囲（`host_range`） | `InvalidArgument` |

- 用途の拒否の文字列は `<操作の名前>: buffer usage lacks <値> required for <役割>` の形。
- 判定に使うのは確保のときに記録した用途で、ハンドルの `usage()` は使わない。
- 長さ 0 の `upload` と読み出しも同じ順で照らす。
- `download_async` も同じ検査で、結果はコールバックに 1 回渡す。
- `free` のゼロ化は用途の検査を通さない。

device に触らない試験:

| 試験 | 確かめること |
| ---- | ------------ |
| `a_usage_without_the_required_value_is_rejected_with_op_and_value_names` | 足りない用途が 2 つの役割とも拒まれ、文字列に操作の名前と値の名前が入ること |
| `an_empty_usage_is_rejected_even_for_zero_length` | 空の用途が長さ 0 でも拒まれること |
| `the_full_usage_passes_both_roles` | `STORAGE \| UPLOAD \| DOWNLOAD` が 2 つの役割とも通ること |
| `a_usage_rejection_wins_over_a_range_rejection` | 用途が足りず範囲も外れる入力が用途の拒否で返ること |
| `a_usage_rejection_wins_over_an_in_use_rejection` | 用途が足りず使用中の入力が用途の拒否で返ること |
| `in_use_is_rejected_when_the_usage_passes` | 用途が足り使用中の入力が使用中の拒否で返ること |
| `an_out_of_range_input_is_rejected_by_the_range_check` | 用途が足り使用中でなく範囲の外の入力が `host_range` と同じ拒否で返ること |
| `a_passing_input_returns_the_host_range` | 全部通る入力が `host_range` と同じ値を返すこと |

確保の用途を `STORAGE | UPLOAD | DOWNLOAD` に替えた。期待値は変えていない。替えた試験・補助:

- `imp::device_tests::a_fresh_allocation_reads_back_all_zero`
- `imp::device_tests::reused_buffers_never_expose_previous_contents_over_1000_rounds`
- `imp::device_tests::dispatch` の補助 `buffer_of`・`zeroed`
- `dispatch_through_the_public_device_is_recorded_and_submit_is_unsupported`（`crates/contract/tests/backend_metal_device.rs`）

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-metal --lib usage_role_check_tests
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait -- -D warnings
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal -- -D warnings
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --all-features -- -D warnings
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | upload・download から用途の検査を呼ぶ | #4323 |
