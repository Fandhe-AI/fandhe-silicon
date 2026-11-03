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

## 入れ子の絞りと解放の後の型検査（2/5）

`BufferSlice::slice(offset, size)` は範囲の先頭からの相対で絞り、検査は親の大きさに対して `validate_buffer_range`（TASK-309）で行う。返す範囲も元のハンドルを同じ寿命で借り、`buffer()` は元のハンドルを返す。

| 親の範囲 | 相対の `offset` / `size` | 返し |
| -------- | ------------------------ | ---- |
| `[4, 12)` | 2 / 2 | `Ok`（元の先頭からの開始位置 6・大きさ 2） |
| `[4, 12)` | 2 / 7 | `Err(ContractError::InvalidArgument)`（親の 8 を超える） |
| `[4, 12)` | 8 / 0 | `Ok`（開始位置 12・大きさ 0） |
| `[4, 12)` | 9 / 0 | `Err(ContractError::InvalidArgument)` |
| 3 段の入れ子 | 各段の相対の値 | 開始位置は各段の和 |

元のハンドルを `MemoryOps::free` に渡すときの型検査は doctest で確かめる。

| doctest | 期待 |
| ------- | ---- |
| 範囲が生きている間の `free` | `compile_fail,E0505` |
| `free` の後の `slice` | `compile_fail,E0382` |
| 範囲を使い終えた後の `free` | 通る |

確かめ方:

```sh
cargo test -p fandhe-silicon-core --lib memory::slice
cargo test -p fandhe-silicon-core --doc memory::slice
make doctest-error-code-check
```
