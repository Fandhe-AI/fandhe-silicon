# 描画パスの状態の値の型と範囲の型と検査（TASK-838）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-838。

## 所在

- 実装: `crates/core/src/graphics/pass_state.rs`・`crates/core/src/graphics/draw_range.rs`
- 公開のパス: `fandhe_silicon_core::graphics::…`（feature `graphics` の裏。`graphics` モジュールは [graphics-types.md](graphics-types.md)、TASK-837）

## 描画パスの状態の値の型と範囲の型と検査（1/4）

### 型

| 型 | 欄・値 | derive |
| -- | ------ | ------ |
| `Viewport` | `x`・`y`・`width`・`height`・`min_depth`・`max_depth`（すべて `f32`、公開） | `Debug`・`Clone`・`Copy`・`PartialEq` |
| `ScissorRect` | `x`・`y`・`width`・`height`（すべて `u32`、公開） | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` |
| `IndexFormat` | `Uint16`・`Uint32`（`#[non_exhaustive]`。`Default` は `Uint32`） | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash`・`Default` |
| `DrawRange` | `first_vertex`・`vertex_count`・`first_instance`・`instance_count`（すべて `u32`、公開） | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` |
| `IndexedDrawRange` | `first_index`・`index_count`（`u32`）・`base_vertex`（`i32`）・`first_instance`・`instance_count`（`u32`）。公開 | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` |

`index_format_bytes` は `Uint16` で 2、`Uint32` で 4 を返す。

### 検査の関数

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `validate_draw_range(&DrawRange)` | `first_vertex + vertex_count` が `u32` に収まり、`first_instance + instance_count` も収まる（数 0 も同じ規則） | `Ok(())` |
| 同上 | どちらかの和があふれる | `Err(ContractError::InvalidArgument)` |
| `validate_indexed_draw_range(&IndexedDrawRange, IndexFormat, offset, bound_end)` | 2 つの和が収まり、`offset + (first_index + index_count) * インデックス 1 個のバイト数` が `bound_end` 以下（ちょうども通す） | `Ok(())` |
| 同上 | どちらかの和のあふれ、`u64` の足し算のあふれ、終わりが `bound_end` 超え | `Err(ContractError::InvalidArgument)` |

`base_vertex` の値と `offset` の倍数は結果に影響しない。計算は `checked_*` で行う。

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --lib graphics::
cargo test -p fandhe-silicon-core --features graphics --doc graphics
```

## 描画パスの状態を設定する操作と試験用の device の返し（2/4）

### 所在

- 操作: `crates/core/src/graphics.rs` の `GraphicsOps`（feature `graphics`）。
- 試験用の device: `crates/core/src/stub_device.rs` の `StubGraphics`。
- 外の crate の局所の実装例: `crates/core/tests/graphics_types.rs` の `FixedGraphics`（8 操作とも `Ok(())`）。

### 操作

どれも既定の本体を持たず、実装する側が 8 操作とも実装する。

| 操作 | 引数 | 戻り値 |
| ---- | ---- | ------ |
| `set_pipeline` | `&mut RenderEncoder`・`&PipelineHandle` | `Result<(), ContractError>` |
| `set_vertex_buffer` | `&mut RenderEncoder`・`slot: u32`・`&BufferHandle`・`offset: u64`・`size: Option<u64>` | 同上 |
| `set_index_buffer` | `&mut RenderEncoder`・`&BufferHandle`・`IndexFormat`・`offset: u64`・`size: Option<u64>` | 同上 |
| `set_immediates` | `&mut RenderEncoder`・`offset: u32`・`data: &[u8]` | 同上 |
| `set_viewport` | `&mut RenderEncoder`・`Viewport` | 同上 |
| `set_scissor_rect` | `&mut RenderEncoder`・`ScissorRect` | 同上 |
| `set_blend_constant` | `&mut RenderEncoder`・`[f32; 4]` | 同上 |
| `set_stencil_reference` | `&mut RenderEncoder`・`u32` | 同上 |

### 試験用の device の返し

試験用の device の振る舞い。記録先以外の引数は照らさず、状態も持たない。

| 入力 | `set_pipeline` | ほかの 7 操作 |
| ---- | -------------- | ------------- |
| 生きた記録先 | `Unsupported`（`op` は操作名） | 同左 |
| 拒否の値を持つ記録先 | その値（別 device の描画先から作ると `DeviceMismatch`） | 同左 |
| 別 device の記録先 | `DeviceMismatch` | 同左 |
| 破棄した描画先の記録先 | `InvalidArgument` | 同左 |
| 別 device のパイプライン | `DeviceMismatch` | 該当なし |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features test-support,graphics --lib stub_device::
cargo test -p fandhe-silicon-core --features graphics --test graphics_types
cargo test -p fandhe-silicon-core --features graphics --doc graphics
```

## 外の crate からの試験（3/4）

公開のパスだけを使う統合テスト。

- `crates/core/tests/graphics_pass_types.rs`（feature `graphics`）
- `crates/core/tests/graphics_pass_stub.rs`（feature `test-support`・`graphics`。`backend` は `BackendKind::Vulkan`）

### 値の型・範囲の検査・操作の型（`graphics_pass_types`）

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `op_signatures_have_listed_types` | 8 操作を `dyn GraphicsOps` 越しに呼ぶクロージャを、型を書いた `fn` ポインタの変数へ入れる | コンパイルが通り、引数と戻り値の型が表のとおりに固定される |
| `viewport_is_built_from_outside` | 構造体式で 6 欄を与える | `Copy` で 2 回使え、同じ値は `==`、1 欄違いは `!=` |
| `scissor_rect_is_built_from_outside` | 構造体式で 4 欄を与える | 同上 |
| `index_format_has_default_and_bytes` | `IndexFormat::default()`・`index_format_bytes`・`_` の腕つきの `match` | 既定は `Uint32`、バイト数は 2 と 4、`match` がコンパイルできる |
| `draw_ranges_are_built_from_outside` | `DrawRange`・`IndexedDrawRange` を構造体式で作る（`base_vertex` は負の値） | 作れて `==` で比べられ、負の値を保つ |
| `draw_range_sum_overflow_is_rejected` | 頂点側・インスタンス側の和が `u32::MAX` に届く値と超える値 | 届く値は `Ok`、超える値は `InvalidArgument` |
| `indexed_draw_range_bounds` | 終わりと `bound_end` の一致・1 差、`u32` の和のあふれ、`u64` の足し算のあふれ、形式ごとの境、`base_vertex` の最小・0・最大 | 一致は `Ok`、1 超えとあふれは `InvalidArgument`、`base_vertex` で結果は変わらない |
| `rejection_is_repeatable` | 拒否になる同じ引数で 2 回呼ぶ | 戻り値が等しい |
| `rejection_is_invalid_argument_only` | 拒否になる入力の全部 | どれも `InvalidArgument` |

### 試験用の device の返し（`graphics_pass_stub`）

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `pass_state_ops_are_unsupported_on_live_encoder` | 同じ device の描画先の記録先・同じ device のパイプライン | 8 操作とも `Unsupported`、`op` は操作名と一致 |
| `pass_state_ops_return_encoder_rejection` | 別 device の描画先から作った記録先 | 8 操作とも `DeviceMismatch` |
| `pass_state_ops_reject_destroyed_target` | 描画先を破棄した後の記録先 | 8 操作とも `InvalidArgument` |
| `set_pipeline_rejects_other_device_pipeline` | 生きた記録先・別 device のパイプライン | `DeviceMismatch` |
| `pass_state_ops_do_not_consume_encoder` | 8 操作の後に同じ記録先を `submit_render` へ値で渡す | コンパイルが通り、panic しない |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --test graphics_pass_types
cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_pass_stub
```

## 完了の条件の確かめ（4/4）

4/4（#5824）の時点で本リポで観測できた事実だけを記録する（ポインタ: `docs/spec/05-tasks.md` の TASK-838）。GPU の実機なしで実行した。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 型と操作が共通の crate の `graphics` に公開されている | `grep -n 'mod pass_state\|mod draw_range\|pub use pass_state\|pub use draw_range' crates/core/src/graphics.rs`、`grep -n 'fn set_' crates/core/src/graphics.rs`、`cargo check -p fandhe-silicon-core --features graphics` | `graphics.rs:433` の `mod pass_state;`、`:434` の `pub use pass_state::{IndexFormat, ScissorRect, Viewport, index_format_bytes};`、`:436` の `mod draw_range;`、`:437` の `pub use draw_range::{`。`fn set_` は 328・335・345・355・372・379・386・393 行目の 8 行。`cargo check` は終了コード 0 |
| (2) | 8 操作の引数の型・戻り値の型 | `cargo test -p fandhe-silicon-core --features graphics --test graphics_pass_types` | `op_signatures_have_listed_types` が ok |
| (3) | 値の型の欄と外の crate からの構造体式、`IndexFormat` の `#[non_exhaustive]` と既定値 | 同じ試験、`cargo test -p fandhe-silicon-core --features graphics --doc pass_state`、`make doctest-error-code-check` | `viewport_is_built_from_outside`・`scissor_rect_is_built_from_outside`・`index_format_has_default_and_bytes`・`draw_ranges_are_built_from_outside` が ok。doctest は通常が 4 passed、`compile_fail,E0004`（`pass_state.rs:60`）が 1 passed。`make doctest-error-code-check` は終了コード 0 |
| (4) | 範囲の検査の純粋関数の境の入力 | 同じ試験 | `draw_range_sum_overflow_is_rejected`・`indexed_draw_range_bounds`・`rejection_is_repeatable`・`rejection_is_invalid_argument_only` が ok。`graphics_pass_types` 全体は 9 passed |
| (5) | 記録先が寿命の型引数を持たず、値で投入する | `cargo test -p fandhe-silicon-core --features graphics --test graphics_types render_encoder_is_static`、`cargo test -p fandhe-silicon-core --features graphics --doc submit_render` | `render_encoder_is_static` が ok（1 passed）。`--doc submit_render` は通常が 0 passed、`compile_fail,E0382`（`graphics.rs:401`）が 1 passed。`graphics.rs` の `compile_fail,E0382` は 287・401 行目の 2 つ |
| (6) | 足した操作が既定の本体を持たない | `sed -n '328,400p' crates/core/src/graphics.rs`、`grep -n 'impl GraphicsOps for' crates/core/src/stub_device.rs crates/core/tests/graphics_types.rs` | 8 宣言はいずれも `;` で終わり、本体の `{` を持たない（範囲内の `{` で終わる行は doc コメントの例の 1 行だけ）。`impl` は `stub_device.rs:781` と `graphics_types.rs:21`。`fn set_` は前者の `impl` に 8 本（874・886・901・916・929・941・953・965 行目）、後者に 8 本 |
| (7) | 試験用の device の返し | `cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_pass_stub` | `pass_state_ops_are_unsupported_on_live_encoder`・`pass_state_ops_return_encoder_rejection`・`pass_state_ops_reject_destroyed_target`・`set_pipeline_rejects_other_device_pipeline`・`pass_state_ops_do_not_consume_encoder` が ok（5 passed） |
| (8) | 公開 API に wgpu の型が 0 件 | `make public-api-check`・`make public-api-webgpu-check`・`make public-api-sdk-check`・`make public-name-check` | 終了コードはいずれも 0。`record:` の行はいずれも `commit=f7578eb2e072123ffe3eace8defb308a6cd6fdbf`。`public-api-check` は `documents=10 public_items_total=14977 wgpu_type_refs_total=0 wgpu_other_refs_total=0`、`public-api-webgpu-check` は `documents=2 public_items_total=3814 webgpu_names_total=0 submit_token_ops_total=11 submit_token_violations_total=0`、`public-api-sdk-check` は `documents=10 public_items_total=12936 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0`、`public-name-check` は `documents=10 public_items_total=12936 name_hits_total=0` |
| (9) | 依存 0・unsafe 0 | コミットごとの `git show --stat <sha> -- Cargo.lock deps-approved.toml crates/contract`（`28f41f7`・`a050b8b`・`f7578eb`）、`git show f7578eb -- crates/core/Cargo.toml`、`grep -c unsafe crates/core/src/graphics/pass_state.rs crates/core/src/graphics/draw_range.rs`、`grep -n forbid crates/core/src/lib.rs` | 3 コミットとも出力 0 行。`Cargo.toml` の差分は `f7578eb` の `[[test]]` 2 項目（`graphics_pass_types`、`graphics_pass_stub`。+8 行）だけ。`unsafe` は 2 ファイルとも 0 件。`lib.rs:39` に `#![forbid(unsafe_code)]` |
| (10) | CI で流れる場所 | `make verify`（`scripts/verify.sh` は `cargo test --workspace --all-features`）、`make wasm-verify`、`make build-profiles` | `make verify` の `cargo test` の中で `graphics_pass_types` が 9 passed、`graphics_pass_stub` が 5 passed、`graphics_types` が 4 passed。`make verify`・`make wasm-verify`・`make build-profiles` は終了コード 0 |

品質ゲートの終了コード: `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` はいずれも 0。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-838）。確かめた base は `f7578eb2e072123ffe3eace8defb308a6cd6fdbf`。

### 今の値の補い（base 8deff9f）

上の「完了の条件の確かめ（4/4）」の表は記録の時点の値で、書き換えない。今の値は次のとおり。

| 表の行 | 記録の時点の値 | 今の値 |
| ------ | -------------- | ------ |
| (1) | `graphics.rs:433`・`:434`・`:436`・`:437` | `:684` が `mod pass_state;`、`:685` が `pub use pass_state::{...}`、`:687` が `mod draw_range;`、`:688` が `pub use draw_range::{`。`fn set_` は 579・586・596・606・623・630・637・644 行目の 8 行 |
| (3) | doctest は通常が 4 passed、`compile_fail,E0004`（`pass_state.rs:60`）が 1 passed | 通常が 4 passed、`compile_fail` が 3 passed（`compile_fail,E0004` は `pass_state.rs:76`、`compile_fail,E0277` は `pass_state.rs:18`・`:51`） |
| (5) | `compile_fail,E0382`（`graphics.rs:401`）が 1 passed。`graphics.rs` の `compile_fail,E0382` は 287・401 行目の 2 つ | `compile_fail,E0382`（`graphics.rs:652`）が 1 passed。`graphics.rs` の `compile_fail,E0382` は 538・652 行目の 2 つ |
| (6) | `impl` は `stub_device.rs:781` と `graphics_types.rs:21`。`fn set_` は前者に 8 本（874・886・901・916・929・941・953・965 行目） | 変わらない。`graphics_types.rs` の `fn set_` は 99・107・118・129・138・146・154・162 行目の 8 本 |
| (7)・(10) | `graphics_pass_stub` は 5 passed | 6 passed（`pass_state_ops_reject_encoder_of_another_device` が加わる） |

数えたコマンドは `grep -n 'mod pass_state\|mod draw_range\|pub use pass_state\|pub use draw_range' crates/core/src/graphics.rs`、`grep -n 'fn set_' crates/core/src/graphics.rs`、`grep -n 'compile_fail,E0' crates/core/src/graphics.rs crates/core/src/graphics/pass_state.rs`、上の「確かめ方（4/4）」の `--doc pass_state`・`--doc submit_render`・`--test graphics_pass_stub`。

### 確かめ方（4/4）

```sh
grep -n 'mod pass_state\|mod draw_range\|pub use pass_state\|pub use draw_range' crates/core/src/graphics.rs
grep -n 'fn set_' crates/core/src/graphics.rs
cargo check -p fandhe-silicon-core --features graphics
cargo test -p fandhe-silicon-core --features graphics --test graphics_pass_types
cargo test -p fandhe-silicon-core --features graphics --doc pass_state
cargo test -p fandhe-silicon-core --features graphics --test graphics_types render_encoder_is_static
cargo test -p fandhe-silicon-core --features graphics --doc submit_render
cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_pass_stub
make doctest-error-code-check
make public-api-check
make public-api-webgpu-check
make public-api-sdk-check
make public-name-check
make verify
make wasm-verify
make build-profiles
```

## 今の値の補い（base 2f36d3d）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

記録の値は、直前の「今の値の補い（base 8deff9f）」の値。

| 表の行 | 記録の値 → 今の値 | 測ったコマンド |
| ------ | ----------------- | -------------- |
| (1) | `:684` が `mod pass_state;`、`:685` が `pub use pass_state::{...}`、`:687` が `mod draw_range;`、`:688` が `pub use draw_range::{` → `:1046`・`:1047`・`:1049`・`:1050` | `grep -n 'mod pass_state\|mod draw_range\|pub use pass_state\|pub use draw_range' crates/core/src/graphics.rs` |
| (1) | `fn set_` は 579・586・596・606・623・630・637・644 行目の 8 行 → 897・904・914・924・941・948・955・962 行目の 8 行（`graphics.rs` の rustdoc の例の中にも `fn set_` の行があるので、`grep -n 'fn set_'` の出力にはこの 8 行のほか 264 行目から 803 行目の例の行が混ざる） | `awk '/pub trait GraphicsOps/{f=1} f && /^    fn set_/{print NR}' crates/core/src/graphics.rs` |
| (5) | `compile_fail,E0382`（`graphics.rs:652`）が 1 passed。`graphics.rs` の `compile_fail,E0382` は 538・652 行目の 2 つ → `graphics.rs:1014` が 1 passed。2 つは 856・1014 行目 | `grep -n 'compile_fail,E0382' crates/core/src/graphics.rs`、`cargo test -p fandhe-silicon-core --features graphics --doc submit_render` |
| (6) | `impl` は `stub_device.rs:781`。`stub_device.rs` の `fn set_` は 8 本（874・886・901・916・929・941・953・965 行目）→ `impl` は `:782`。8 本は 875・887・902・917・930・942・954・966 行目。`graphics_types.rs:21` と `graphics_types.rs` の `fn set_` の 8 本（99・107・118・129・138・146・154・162 行目）は変わらない | `grep -n 'impl GraphicsOps for' crates/core/src/stub_device.rs`、`awk '/impl GraphicsOps for StubGraphics/{f=1;next} f && /^}/{exit} f && /^    fn set_/{print NR}' crates/core/src/stub_device.rs`、`grep -n 'fn set_' crates/core/tests/graphics_types.rs` |
| (7)・(10) | `graphics_pass_stub` は 6 passed → 6 passed。`graphics_pass_types` は 9 passed のまま | `cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_pass_stub`、`... --test graphics_pass_types` |
