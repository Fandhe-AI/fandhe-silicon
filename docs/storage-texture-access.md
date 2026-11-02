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

## 束縛の用途とシェーダーとの照合（2/4）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/storage_access.rs` |
| 公開のパス | `fandhe_silicon_core::texture::validate_storage_texture_usage`・`fandhe_silicon_core::texture::validate_storage_texture_shader_access` |
| feature の条件 | なし |

`validate_storage_texture_usage(declared, access)` は、`STORAGE_BINDING` を要求し、`Atomic` のときは `STORAGE_ATOMIC` も要求する。足りなければ `InvalidArgument`。過申告は通り、空の用途はどのアクセスでも拒否する。

| `access` | `STORAGE_BINDING` のみ | 両方 | `STORAGE_ATOMIC` のみ | 空 |
| -------- | ---------------------- | ---- | --------------------- | -- |
| `WriteOnly`・`ReadOnly`・`ReadWrite` | Ok | Ok | `InvalidArgument` | `InvalidArgument` |
| `Atomic` | `InvalidArgument` | Ok | `InvalidArgument` | `InvalidArgument` |

`validate_storage_texture_shader_access(layout_format, layout_access, shader_format, shader_access)` は、形式の完全一致を先に見て、次にアクセスを見る。不一致は `InvalidArgument` で、文言は `C2:` で始まる。

| 配置 / シェーダー | `WriteOnly` | `ReadOnly` | `ReadWrite` | `Atomic` |
| ---------------- | ----------- | ---------- | ----------- | -------- |
| `WriteOnly` | Ok | 拒否 | 拒否 | 拒否 |
| `ReadOnly` | 拒否 | Ok | 拒否 | 拒否 |
| `ReadWrite` | Ok | 拒否 | Ok | 拒否 |
| `Atomic` | 拒否 | 拒否 | 拒否 | Ok |

形式が違えば（sRGB の対を含む）アクセスに依らず `InvalidArgument`。

確かめ方:

```text
cargo test -p fandhe-silicon-core --lib texture::storage_access
cargo test -p fandhe-silicon-core --doc validate_storage_texture
```
