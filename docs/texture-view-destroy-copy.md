# テクスチャのコピーの指定・view の型・描画先の破棄の操作・テクスチャと view の破棄の操作の所在（TASK-372）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-372。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/copy_spec.rs` |
| 公開のパス | `fandhe_silicon_core::texture::TextureOrigin`・`fandhe_silicon_core::texture::TextureCopySpec` |
| feature の条件 | なし |

テクスチャの型の所在は [texture-types.md](texture-types.md)（TASK-371）。

所在の補い（`TextureOps` の破棄の操作と試験用の device）: 上の表の外に、次のファイルがある。詳しくは末尾の「テクスチャと view の破棄の操作」。

| 項目 | 内容 |
| ---- | ---- |
| `TextureOps` の破棄の 2 操作 | `crates/core/src/texture/ops.rs` の `destroy_texture`・`destroy_view` |
| 試験用の device の表 | `crates/core/src/stub_device.rs` の `StubTextures` |
| 外の crate の試験 | `crates/core/tests/texture_destroy.rs`（feature `test-support`） |

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

今の補い: 記録先を作った後にその描画先を破棄してから `draw`・`draw_ex` に渡すと `InvalidArgument` を返す。
別 device で作った記録先を `draw`・`draw_ex`・`submit_render` に渡すと `DeviceMismatch` を返す。
外の crate の試験は `crates/core/tests/render_target_destroy.rs` の `recorded_then_destroyed_target_rejects_draw` と `other_device_encoder_is_mismatch_for_draw_and_submit`（#7855）。

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

## 完了の条件の確かめ（5/5）

5/5（#4567）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-372）。GPU の実機なしの Linux で実行し、件数は実測。コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 型と操作が共通の crate に公開されている | `grep -n 'mod copy_spec\|mod view\|pub use copy_spec\|pub use view' crates/core/src/texture.rs`、`grep -n 'fn destroy_offscreen_target' crates/core/src/graphics.rs`、`cargo check -p fandhe-silicon-core`（feature なし）と `--features graphics` | `texture.rs` の 492・493・495・496 行目、`graphics.rs` の 293 行目。`cargo check` は 2 つとも終了コード 0 |
| (2) | コピーの指定の型 | `cargo test -p fandhe-silicon-core --test texture_view_copy_spec` の `copy_spec_*` | 4 本とも `ok` |
| (3) | view の型と元のテクスチャの問い合わせ | 同じ試験の `view_*`・`two_views_*`、`cargo test -p fandhe-silicon-core --doc TextureView`、`make doctest-error-code-check` | 試験 4 本とも `ok`（試験全体は 8 passed）。doctest は通常 2 passed・`compile_fail` 3 passed。`make doctest-error-code-check` は終了コード 0 |
| (4) | 描画先の破棄の後・作っていない描画先の使用の値 | `cargo test -p fandhe-silicon-core --features test-support,graphics --test render_target_destroy`、`cargo test -p fandhe-silicon-core --features graphics --doc destroy_offscreen_target` | 10 本とも `ok`（10 passed）。doctest は 1 passed |
| (5) | 試験用の device で確かめていること | `grep -n -A2 'render_target_destroy' crates/core/Cargo.toml`、`grep -n 'all-features' scripts/verify.sh` | `[[test]]` の `required-features = ["test-support", "graphics"]`。`verify.sh` の 23・39・62 行目が `cargo test --workspace --all-features`（60 行目は clippy の `--all-features`） |
| (6) | 依存 0・unsafe 0 | 1/5〜4/5 の 4 コミット（3343dba・a9c082a・c3f387d・e46aa2e）それぞれで `Cargo.lock`・`deps-approved.toml`・`crates/contract`・`crates/core/src/texture/ops.rs`・`crates/core/src/device.rs`・`crates/core/Cargo.toml` の差分を見る、`grep -c unsafe crates/core/src/texture/copy_spec.rs crates/core/src/texture/view.rs`、`grep -n forbid crates/core/src/lib.rs` | 4 コミットとも `Cargo.lock`・`deps-approved.toml`・`crates/contract`・`ops.rs`・`device.rs` の差分は 0。`crates/core/Cargo.toml` は e46aa2e の 4 行の追加（`[[test]]` の 1 項目）だけ。`unsafe` は 2 ファイルとも 0 件。`lib.rs` の 38 行目に `#![forbid(unsafe_code)]` |
| (7) | CI で流れる場所と件数 | `make verify`、`make wasm-verify`、`make profile-test-count-check`、`grep -n 'run: make' .github/workflows/ci.yml` | 3 つとも終了コード 0。`make verify` の出力で `render_target_destroy` は 10 passed、`texture_view_copy_spec` は 8 passed。`profile-test-count-check` の `record:` は CpuOnly が passed=6 graphics=0、ComputeOnly が passed=6 graphics=0、Graphics が passed=9 graphics=3。`ci.yml` の `make verify` は 177・498 行目、`make wasm-verify` は 275・459 行目、`make profile-test-count-check` は 186・504 行目 |

今の件数の補い: (4) の `render_target_destroy` は、上の 2 本が加わって 12 passed（`cargo test -p fandhe-silicon-core --features test-support,graphics --test render_target_destroy`）。

doctest の件数の補い: (4) の「doctest は 1 passed」は通常の 1 本の件数で、`compile_fail` の 1 本（`destroy_offscreen_target` の 285 行目）は別に数える。`cargo test -p fandhe-silicon-core --features graphics --doc destroy_offscreen_target` の今の値は、通常 1 passed・`compile_fail` 1 passed。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-372）。確かめた base は `e46aa2ee8c6f1e24ac67b064a6bff6b60d640b52`。

確かめ方:

```text
cargo check -p fandhe-silicon-core
cargo check -p fandhe-silicon-core --features graphics
cargo test -p fandhe-silicon-core --test texture_view_copy_spec
cargo test -p fandhe-silicon-core --features test-support,graphics --test render_target_destroy
cargo test -p fandhe-silicon-core --doc TextureView
make doctest-error-code-check
make verify
make wasm-verify
make profile-test-count-check
```

## テクスチャと view の破棄の操作

所在: `crates/core/src/texture/ops.rs`（trait `TextureOps`）、`crates/core/src/stub_device.rs`（試験用の device の `StubTextures`）、`crates/core/tests/texture_destroy.rs`（外の crate の試験）。ポインタ: `docs/spec/05-tasks.md` の TASK-372。

`TextureOps` は `create_texture` のほかに 2 つの操作を持つ。どちらも既定の本体は無く、実装する側は両方を実装する。

| 操作 | 署名 | 戻り |
| ---- | ---- | ---- |
| `destroy_texture` | `fn destroy_texture(&self, texture: TextureHandle)` | 無し（`Result` を返さない） |
| `destroy_view` | `fn destroy_view<'t>(&self, view: TextureView<'t>)` | 無し（`Result` を返さない） |

handle と view は値で受けて消費する。コンパイルで拒まれる使い方は doctest の `compile_fail` で確かめる。

| 使い方 | doctest | error コード |
| ------ | ------- | ------------ |
| `destroy_texture` の後に同じ handle を使う | `destroy_texture` の `compile_fail` | E0382 |
| 同じ handle を 2 回 `destroy_texture` する | `destroy_texture` の `compile_fail` | E0382 |
| view が残る間に元の handle を `destroy_texture` する | `destroy_texture` の `compile_fail` | E0505 |
| `destroy_view` の後に同じ view を使う | `destroy_view` の `compile_fail` | E0382 |

通る例は `&dyn TextureOps` 越しの `destroy_texture` と、`destroy_view` の後の `destroy_texture`。

試験用の device の `StubTextures` は、作ったテクスチャの識別子の表を持つ。入力ごとの動きは次のとおり。

| 入力 | 表 | 次に作る識別子 |
| ---- | -- | -------------- |
| 作ったテクスチャの `destroy_texture` | その識別子を除く | 破棄した識別子と違う |
| 作っていない識別子の handle の `destroy_texture` | 変わらない | 続きの番号 |
| 別 device の handle の `destroy_texture` | 自分の項目は残る | 続きの番号 |
| view の `destroy_view` | 変わらない（元の識別子も残る） | 続きの番号 |

試験:

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `stub_device` の `destroy_texture_removes_own_entry` | 作って破棄する | 表から消える |
| 同 `destroy_texture_of_other_device_keeps_own_entry` | 同じ番号で device の識別子だけ違う handle を破棄する | 自分の項目が残る |
| 同 `destroy_unknown_texture_keeps_entries` | 作っていない番号の handle を破棄する | 表が変わらない |
| 同 `destroy_view_keeps_texture_entry` | 作ったテクスチャの view を破棄する | 元の識別子が残る |
| 同 `destroyed_texture_id_is_not_reused` | 破棄の後に作る | 識別子が前と違う |
| `texture_destroy` の `destroyed_texture_id_is_not_reused` | `ComputeOnly`・`Graphics` の両方で作る・破棄する・作る | 識別子が前と違う |
| 同 `destroying_unknown_texture_keeps_numbering` | 作っていない番号の handle を破棄する | panic せず、次の識別子は前の続き |
| 同 `destroying_other_device_texture_is_noop` | 2 つの device の handle を取り違えて破棄する | panic せず、どちらの device も続けて作れる |
| 同 `destroy_view_then_texture_through_dyn` | `&dyn TextureOps` 越しに `destroy_view`、`destroy_texture` | panic しない |

確かめ方:

```text
cargo test -p fandhe-silicon-core --doc texture::ops
cargo test -p fandhe-silicon-core --features test-support --lib stub_device
cargo test -p fandhe-silicon-core --features test-support --test texture_destroy
cargo test -p fandhe-silicon-core --all-features
make doctest-error-code-check
make verify
```

今の値の補い（base 1890c9a）: `TextureOps` の操作は今は 4 つ（`create_texture`・`destroy_texture`・`destroy_view`・`create_sampler`）で、どれも既定の本体は無い。`create_sampler` の所在は [device-textures.md](device-textures.md)（TASK-150）。
