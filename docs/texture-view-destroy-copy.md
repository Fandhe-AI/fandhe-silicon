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

## view の型と元のテクスチャの問い合わせ（2/5）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/view.rs` |
| 公開のパス | `fandhe_silicon_core::texture::TextureView` |
| feature の条件 | なし |

`TextureView<'t>` は元の handle の借用と、世代・識別子・device の識別子を非公開の欄で持つ。
`Debug` だけを実装し、`Clone`・`Copy` は実装しない。

| 口 | 返す値 |
| -- | ------ |
| `texture()` | 組み立てで渡した handle の借用（寿命 `'t`。`Result` ではない） |
| `__backend_generation()`・`__backend_id()`・`__backend_device_id()` | 組み立てで渡した値 |

コンパイルで拒まれるもの（doctest の `compile_fail`）:

| 書き方 | エラーコード |
| ------ | ------------ |
| `.clone()` | E0599 |
| 外の crate からの構造体式 | E0451 |
| view が残る間に元の handle を動かす | E0505 |

確かめ方:

```text
cargo test -p fandhe-silicon-core --lib texture::view
cargo test -p fandhe-silicon-core --doc TextureView
make doctest-error-code-check
```
