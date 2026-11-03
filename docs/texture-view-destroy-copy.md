# テクスチャのコピーの指定などの所在（TASK-372）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-372。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/copy_spec.rs` |
| 公開のパス | `fandhe_silicon_core::texture::TextureOrigin`・`fandhe_silicon_core::texture::TextureCopySpec` |
| feature の条件 | なし |

テクスチャの型の所在は [texture-types.md](texture-types.md)（TASK-371）。

## コピーの転送元・転送先の指定の型（1/5）

`TextureOrigin` は `x`・`y`・`z` を `u32` で持つ。`Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` を実装する。

`TextureCopySpec<T>` は `texture: T`・`mip_level: u32`・`origin: TextureOrigin` を持つ。
`#[non_exhaustive]` で、`Debug`・`Clone`・`Copy` を実装する（`Clone`・`Copy` は `T` が実装するとき）。
外の crate は構造体式で組めず、次の 2 つの口を使う。どちらも検査せず、失敗せず、panic しない。

| 口 | `texture` | `mip_level` | `origin` |
| -- | --------- | ----------- | -------- |
| `whole(texture)` | 渡した値 | 0 | `(0, 0, 0)` |
| `new(texture, mip_level, origin)` | 渡した値 | 渡した値 | 渡した値 |

確かめ方:

```text
cargo test -p fandhe-silicon-core --lib texture::copy_spec
cargo test -p fandhe-silicon-core --doc TextureCopySpec
cargo test -p fandhe-silicon-core --doc TextureOrigin
```
