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

## 項目ごとの受理と拒否の入力の試験（3/4）

置き場は `crates/core/tests/texture_storage_access.rs`（feature なし。公開のパスだけを使う外の crate の試験）。照合は形式 `R32Float` でそろえて呼ぶ。拒否は `InvalidArgument` の種類を照らし、照合の拒否は文言の先頭の `C2:` までを照らす。

| 試験の名前 | 入力 | 期待 |
| ---------- | ---- | ---- |
| `access_values` | `ALL` | 4 値を宣言の順に並べた一覧。`writes`・`reads` は `WriteOnly` が真・偽、`ReadOnly` が偽・真、`ReadWrite`・`Atomic` が真・真 |
| `write_only_binding_usage`・`read_only_binding_usage`・`read_write_binding_usage` | `STORAGE_BINDING` のみ・過申告 / `TEXTURE_BINDING` のみ・空 | Ok / `InvalidArgument` |
| `atomic_binding_usage` | `STORAGE_BINDING` と `STORAGE_ATOMIC` の両方・過申告 / `STORAGE_BINDING` のみ・`STORAGE_ATOMIC` のみ・空 | Ok / `InvalidArgument` |
| `read_only_layout_rejects_writing_shader` | 配置 `ReadOnly`、シェーダー 4 値 | `ReadOnly` だけ Ok、残りは `C2:` の `InvalidArgument` |
| `read_write_layout_shader_access` | 配置 `ReadWrite`、シェーダー 4 値 | `ReadWrite`・`WriteOnly` は Ok、`ReadOnly`・`Atomic` は `C2:` の `InvalidArgument` |
| `write_only_layout_shader_access` | 配置 `WriteOnly`、シェーダー 4 値 | `WriteOnly` だけ Ok、残りは `C2:` の `InvalidArgument` |
| `atomic_layout_shader_access` | 配置 `Atomic`、シェーダー 4 値 | `Atomic` だけ Ok、残りは `C2:` の `InvalidArgument` |
| `shader_format_must_match` | 4 アクセスそれぞれで配置とシェーダーに同じアクセス、形式は `R32Float` と `R32Uint`・`Rgba8Unorm` と `Rgba8UnormSrgb`（逆向きも） | `C2:` の `InvalidArgument` |
| `rejection_is_repeatable` | 拒否・受理の入力を同じ引数で 2 回 | 2 回とも同じ結果 |
| `rejection_is_invalid_argument_only` | 用途と照合の拒否の入力 | どれも `InvalidArgument` |

確かめ方:

```text
cargo test -p fandhe-silicon-core --test texture_storage_access
```
