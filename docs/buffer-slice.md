# バッファの部分範囲の型

TASK-311 の 1/5〜5/5。private spec の本文は写さない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 物 | 場所 |
| -- | ---- |
| 型と作る口・読む口 | `crates/core/src/memory/slice.rs` |
| 公開のパス | `fandhe_silicon_core::memory::BufferSlice` |
| feature | 条件なし |

## 部分範囲の型と作る口（1/5）

`BufferSlice<'a>` は元の `BufferHandle` を共有で借りる値で、`Debug`・`Clone`・`Copy` を持つ。欄は非公開で、下の 2 つの口からだけ作れる。範囲の検査は `validate_buffer_range`（TASK-309）をそのまま使い、新しいエラーの種類は足さない。

| 口 | 入力 | 返し |
| -- | ---- | ---- |
| `BufferHandle::slice(offset, size)` | 終端 `offset + size` が `size()` 以下（確保長ちょうど・長さ 0 を含む） | `Ok(BufferSlice)` |
| 同上 | 終端が `size()` を超える、または `u64` であふれる | `Err(ContractError::InvalidArgument)`（文言は `validate_buffer_range` のまま） |
| `BufferHandle::as_entire_slice()` | なし | 開始位置 0・大きさ `size()` の `BufferSlice`（失敗しない） |
| `BufferSlice::buffer()` | なし | 範囲を作った元のハンドルへの参照 |
| `BufferSlice::offset()` | なし | 元のバッファの先頭からのバイト数 |
| `BufferSlice::size()` | なし | 範囲のバイト数 |

読む口はどれも `Result` を返さない。

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib memory::slice
cargo test -p fandhe-silicon-core --doc memory::slice
```
