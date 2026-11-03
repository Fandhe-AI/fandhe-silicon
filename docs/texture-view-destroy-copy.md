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

## 描画先の破棄の操作と試験用の device の描画先の表（3/5）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/graphics.rs`（trait の操作）・`crates/core/src/stub_device.rs`（試験用の device の表） |
| 公開のパス | `fandhe_silicon_core::graphics::GraphicsOps::destroy_offscreen_target` |
| feature の条件 | `graphics`（試験用の device は `test-support` と `graphics`） |

`GraphicsOps` の所在は [graphics-types.md](graphics-types.md)（TASK-837）、試験用の device は [stub-device.md](stub-device.md)（TASK-156）、
`RenderTargetHandle` は [render-target-handle.md](render-target-handle.md)（TASK-375）を参照する。

```rust
fn destroy_offscreen_target(&self, target: RenderTargetHandle);
```

handle を値で受けて消費し、`Result` は返さない。破棄の後に同じ handle を使う書き方はコンパイルで拒まれる（doctest の `compile_fail,E0382`）。

試験用の device は作った描画先の識別子の表と、記録先から使う描画先への対応を持つ。入力ごとの返しは次のとおり。

| 渡す描画先 | `create_render_encoder*` の記録先 | `draw`・`draw_ex` | `submit_render` | `copy_target_to_buffer` | `destroy_offscreen_target` |
| ---------- | --------------------------------- | ----------------- | --------------- | ----------------------- | -------------------------- |
| 作った描画先 | 拒否なし | `Unsupported` | 印を返す | `Unsupported` | 表から消す |
| 破棄した描画先・作っていない描画先 | `InvalidArgument` を持つ | `InvalidArgument` | `InvalidArgument` | `InvalidArgument` | 何もしない |
| 別 device の描画先 | `DeviceMismatch` を持つ | `DeviceMismatch` | `DeviceMismatch` | `DeviceMismatch` | 何もしない |

記録先を作った後に、その描画先を破棄してから `submit_render` に渡すと `InvalidArgument` を返す。
破棄した描画先の識別子は再利用しない。

確かめ方:

```text
cargo test -p fandhe-silicon-core --features test-support,graphics --lib stub_device
cargo test -p fandhe-silicon-core --features graphics --doc destroy_offscreen_target
cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_types
make doctest-error-code-check
```
