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

## 項目ごとの共通の試験（4/5）

外の crate の統合テスト 2 本で、コピーの指定・view の型・描画先の破棄の後の使用の値を公開のパスだけで確かめる。

| 試験のファイル | feature |
| -------------- | ------- |
| `crates/core/tests/texture_view_copy_spec.rs` | なし |
| `crates/core/tests/render_target_destroy.rs` | `test-support`・`graphics` |

`texture_view_copy_spec.rs`（8 本）:

| 試験の名前 | 入力 | 期待 |
| ---------- | ---- | ---- |
| `copy_spec_whole_points_to_mip0_origin0` | `TextureCopySpec::whole(&h)` | mip の段 0・原点 `(0, 0, 0)`・`texture` は `&h` を指す |
| `copy_spec_new_keeps_values_unchecked` | `new` に通常の値と `u32::MAX` | 渡した値をそのまま持つ・panic しない |
| `copy_spec_is_copy_with_borrowed_texture` | 指定を代入で写す | 写しの元も先も使え、同じ `&h` を指す |
| `copy_spec_fields_are_writable` | 欄へ代入 | 読み返して代入した値と一致する |
| `view_texture_returns_the_same_handle` | `TextureView::texture()` | 組み立てで渡した `&h` |
| `view_texture_outlives_view` | view を捨てた後の借用 | 元の handle を指し続ける |
| `two_views_borrow_one_texture` | 同じ handle から 2 つの view | どちらも `&h` を返し、識別子は組み立てで渡した値 |
| `view_backend_readers_round_trip` | `0`・`u64::MAX` を含む 3 つの値 | 組み立てで渡した値を返す |

`render_target_destroy.rs`（10 本。`backend` は `Vulkan`、`Profile::Graphics` の試験用の device）:

| 試験の名前 | 入力 | 期待 |
| ---------- | ---- | ---- |
| `destroyed_target_is_rejected_by_encoder` | 破棄した描画先 | 記録先が `InvalidArgument` を持つ・`draw`・`submit_render` が `InvalidArgument` |
| `never_created_target_is_invalid_argument` | 作っていない番号 | 記録先・`copy_target_to_buffer` が `InvalidArgument` |
| `other_device_target_is_mismatch_before_table` | 別 device の描画先（破棄の後を含む） | 記録先・`copy_target_to_buffer` が `DeviceMismatch` |
| `destroying_other_device_handle_keeps_own_target` | 別 device の識別子で同じ番号の破棄 | 自 device の描画先は拒否なしで、`submit_render` が通る |
| `destroying_unknown_handle_is_noop` | 作っていない番号の破棄 | panic せず、ほかの描画先は拒否なし |
| `recorded_then_destroyed_target_rejects_submit` | 記録先を作った後に描画先を破棄 | `submit_render` が `InvalidArgument` |
| `depth_pair_is_destroyed_one_by_one` | 深度の描画先だけ破棄 | 深度つきの記録先は `InvalidArgument` を持ち、色だけの記録先は拒否なし |
| `destroyed_ids_are_not_reused` | 破棄の後に作り直す | 識別子が違う |
| `rejection_is_repeatable` | 同じ入力を 2 回 | 2 回の値が等しい |
| `rejection_is_invalid_argument_only` | 破棄した描画先・作っていない番号 | 記録先・`draw`・`draw_ex`・`submit_render`・`copy_target_to_buffer` がすべて `InvalidArgument` |

確かめ方:

```text
cargo test -p fandhe-silicon-core --test texture_view_copy_spec
cargo test -p fandhe-silicon-core --features test-support,graphics --test render_target_destroy
```
