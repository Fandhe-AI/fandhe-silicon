# 書き込み用のビューの型

TASK-312 の 1/4〜4/4。private spec の本文は写さない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

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
