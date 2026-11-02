# storage texture のアクセスの型（TASK-1944）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1944。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/storage_access.rs` |
| 公開のパス | `fandhe_silicon_core::texture::StorageTextureAccess` |
| feature の条件 | なし |

テクスチャの型の所在は [texture-types.md](texture-types.md)（TASK-371）。

## アクセスの型（1/4）

`StorageTextureAccess` は `#[non_exhaustive]` の列挙で、4 値を持つ。`ALL` は 4 値を下の順に並べた一覧。

| 値 | `writes()` | `reads()` |
| -- | ---------- | --------- |
| `WriteOnly` | 真 | 偽 |
| `ReadOnly` | 偽 | 真 |
| `ReadWrite` | 真 | 真 |
| `Atomic` | 真 | 真 |

確かめ方:

```text
cargo test -p fandhe-silicon-core --lib texture::storage_access
cargo test -p fandhe-silicon-core --doc StorageTextureAccess
```
