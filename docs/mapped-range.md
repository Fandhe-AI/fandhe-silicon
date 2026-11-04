# 書き込み用のビューの型

TASK-312 の 1/4〜2/4。private spec の本文は写さない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 物 | 場所 |
| -- | ---- |
| 型と長さ・空の判定 | `crates/core/src/memory/mapped_range.rs` |
| 公開のパス | `fandhe_silicon_core::memory::MappedRangeMut` |
| feature | 条件なし |

## 書き込み用のビューの型と長さ・空の判定（1/4）

作る口は背後実装が使う `#[doc(hidden)]` の口で、借りたバイト列をそのまま包む（検査も変換もしない）。

| 口 | 入力 | 返し |
| -- | ---- | ---- |
| `len` | なし | ビューのバイト数（`u64`）。失敗しない |
| `is_empty` | なし | 長さが 0 のとき真 |
| `Debug` | なし | `MappedRangeMut { len: N }`（長さだけ） |

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib memory::mapped_range
cargo test -p fandhe-silicon-core --doc mapped_range
```

## 範囲の絞り（2/4）

`slice(offset, len)` はビューの先頭からの相対で範囲を絞り、元のビューを排他で借りる。
検査は親の長さに対して `validate_buffer_range`（TASK-309）で行い、`Err` はその値のまま返す。

| 入力（16 バイトのビュー） | 返し |
| ------------------------ | ---- |
| `slice(0, 16)` | `Ok`（長さ 16） |
| `slice(16, 0)` | `Ok`（空） |
| `slice(8, 9)`・`slice(17, 0)` | `Err(ContractError::InvalidArgument)` |
| `slice(u64::MAX, 1)` | `Err(ContractError::InvalidArgument)` |
| 絞ったビューをさらに絞る | 絞ったビューの先頭からの相対で、その長さに照らす |

| doctest | 結果 |
| ------- | ---- |
| 絞ったビューが生きている間に同じビューから次を絞る | `compile_fail,E0499` |
| 使い切ってから次を絞る | 通る |

```sh
cargo test -p fandhe-silicon-core --lib memory::mapped_range
RUSTC_BOOTSTRAP=1 cargo test -p fandhe-silicon-core --doc memory::mapped_range
make doctest-error-code-check
```
