# 描画の公開の型と操作の trait（TASK-837）の所在と本リポの現状

TASK-837 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは DEV-63 のポインタだけにする。事実はリポジトリのルートで流したコマンドの出力から書く。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-837。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 現状の形（1/4）

### feature と転送

| 項目 | 事実 |
| ---- | ---- |
| core の feature | `crates/core/Cargo.toml:26` の `graphics = []` |
| 契約の crate の feature | `crates/contract/Cargo.toml:34` の `graphics = ["fandhe-silicon-core/graphics"]` |
| module | `crates/core/src/lib.rs` の `#[cfg(feature = "graphics")] pub mod graphics;`。feature を有効にしたときだけ公開される |

### 型

所在は `crates/core/src/graphics.rs`。

| 型 | 所在 | 内容 | derive | `#[non_exhaustive]` |
| -- | ---- | ---- | ------ | ------------------- |
| `PixelFormat` | `:27` | `Rgba8Unorm`・`Bgra8Unorm`・`Rgba32Float` | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` | あり |
| `DepthFormat` | `:48` | `Depth32Float` | 同上 | あり |
| `CompareFunction` | `:67` | `Less`・`LessEqual`・`Always` | 同上 | あり |
| `DepthConfig` | `:89` | 公開の欄 `format`・`clear_value`・`compare`・`write_enabled` | `Debug`・`Clone`・`Copy`・`PartialEq` | なし |
| `RenderEncoder` | `:132` | 非公開の欄 4 つ。`Clone`・`Copy` を持たない | `Debug` だけ | なし |

`RenderEncoder` の背後実装向けの口は `#[doc(hidden)]` の 6 つ。`__backend_new`・`__backend_new_rejected`（どちらも検査せず、safe）・`__backend_generation`・`__backend_id`・`__backend_device_id`・`__backend_rejected`。

描画先の handle は `crates/core/src/render_target.rs` の `RenderTargetHandle` を使う（TASK-375）。

### GraphicsOps

`crates/core/src/graphics.rs:202` の `pub trait GraphicsOps`。既定の本体は持たない。操作は 10 個。署名は rustdoc を見る。

`create_render_pipeline`・`create_render_pipeline_with_depth`・`create_offscreen_target`・`create_offscreen_target_with_depth`・`create_render_encoder`・`create_render_encoder_with_depth`・`draw`・`draw_ex`・`submit_render`・`copy_target_to_buffer`

### 対応するテスト

`crates/core/src/graphics.rs` の rustdoc の doctest:

| 位置 | 確かめること |
| ---- | ------------ |
| `PixelFormat` line 15（`compile_fail,E0004`） | 外の crate からワイルドカードの腕の無い `match` が拒まれる |
| `DepthFormat` line 38（`compile_fail,E0004`） | 同上 |
| `CompareFunction` line 55（`compile_fail,E0004`） | 同上 |
| `DepthConfig` line 78 | 公開の欄で構造体式を書ける |
| `RenderEncoder` line 105 | `__backend_new(5, 3, 1)` の 3 値と `__backend_rejected()` が `None`。`__backend_new_rejected` の値が `Some(&ContractError::DeviceMismatch)` で読める |
| `RenderEncoder` line 119（`compile_fail,E0451`） | 構造体式が拒まれる |
| `RenderEncoder` line 126（`compile_fail,E0599`） | `clone` が呼べない |
| `GraphicsOps` line 196 | `&dyn GraphicsOps` で書け、`submit_render` に記録先を 1 回渡せる |
| `GraphicsOps::submit_render` line 269（`compile_fail,E0382`） | 同じ記録先を 2 回渡すことが拒まれる |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features graphics --doc graphics
```

実行の出力は、通常の doctest が 3 件、`compile_fail` が 6 件で、いずれも `ok`。

## device の口と試験用の device（2/4）

### `Device::graphics()`

`crates/core/src/device.rs:141` の `fn graphics(&self) -> Option<&dyn GraphicsOps>`。feature `graphics` のビルドにだけある。既定の本体は `None`（確かめる試験は `crates/core/tests/device_trait_shape.rs` の `default_graphics_is_none_through_dyn_device`）。

### 試験用の device の答え

feature `test-support` と `graphics` を有効にしたとき、`StubDevice::graphics()` の答えは要求のプロファイルで変わる（所有形態によらない）。

| プロファイル | `graphics()` |
| ------------ | ------------ |
| `CpuOnly` | `None` |
| `ComputeOnly` | `None` |
| `Graphics` | `Some` |

`Some` が指すのは `crates/core/src/stub_device.rs` の非公開の型 `StubGraphics` で、`StubDevice::graphics()` 越しにだけ届く。10 操作の振る舞いは次のとおり。

| 操作 | 振る舞い |
| ---- | -------- |
| `create_render_pipeline`・`create_render_pipeline_with_depth` | 入力・配置・深度の形式を検査せず `Ok(PipelineHandle)` を返す。印の device の識別子は `generation()` と等しい |
| `create_offscreen_target` | 幅・高さ・形式を検査せず `Ok(RenderTargetHandle)` を返す |
| `create_offscreen_target_with_depth` | 色・深度の順に、識別子の異なる handle を 2 つ返す |
| `create_render_encoder`・`create_render_encoder_with_depth` | 描画先を検査せず記録先を返す |
| `draw`・`draw_ex`・`copy_target_to_buffer` | 何もせず `Unsupported` を返す |
| `submit_render` | `wait` が空でなければ `Unsupported`。記録先が拒否の値を持てばその値を返す。そうでなければ印を返し、`queue()` の `poll` で `Complete` になる |

通し番号が尽きたら、生成の操作は `BackendFailure`（記録先は拒否の値を持つ）を返す。panic しない。

### 対応するテスト

| ファイル | 試験 | 確かめること |
| -------- | ---- | ------------ |
| `crates/core/tests/stub_device_graphics.rs` | `cpu_only_*`・`compute_only_*`（所有形態ごとに 3 本ずつ、計 6 本） | `graphics()` が `None` |
| 同上 | `graphics_*_graphics_reaches_ops`（3 本） | `Some` で、描画先・パイプライン・記録先の device の識別子が `generation()` と等しく、投入の印が `Complete` になる |
| 同上 | `draw_draw_ex_and_copy_target_to_buffer_are_unsupported` | 3 操作が `Unsupported` |
| 同上 | `submit_render_with_wait_is_unsupported` | 待ちつきの投入が `Unsupported` |
| 同上 | `offscreen_target_with_depth_returns_distinct_ids` | 2 つの handle の識別子が異なる |
| 同上 | `submit_render_returns_error_held_by_encoder` | 記録先が持つ拒否の値がそのまま返る |
| `crates/core/tests/device_trait_shape.rs` | `default_graphics_is_none_through_dyn_device` | 上書きしない device で既定が `None` |
| `crates/core/src/stub_device.rs` の `mod tests` | `graphics_create_reports_exhausted_id_space` | 通し番号が尽きたら `BackendFailure` |
| 同上 | `render_encoder_carries_backend_failure_when_ids_are_exhausted` | 尽きたときの記録先が拒否の値を持ち、`submit_render` が同じ値を返す |

固定した値は `BackendKind::Vulkan` と、`Imported` の `NativeDeviceHandle::new(7u32)`。流れる組は `make verify`（`--all-features`）。

```text
cargo test -p fandhe-silicon-core --features test-support,graphics --test stub_device_graphics
cargo test -p fandhe-silicon-core --features graphics --test device_trait_shape
cargo test -p fandhe-silicon-core --features test-support,graphics --lib stub_device
```

## CI への組み込み（3/4）

描画の型と試験が流れる場所は次のとおり。新しいジョブ・ステップは足していない（`make build-profiles` は CI の `verify` ジョブと `verify-macos` ジョブが呼ぶ）。

| 場所 | 流れるもの | 確かめるコマンド |
| ---- | ---------- | ---------------- |
| `make verify`（`scripts/verify.sh` の `--all-features`） | core の graphics の doctest・`device_trait_shape`・`stub_device_graphics`・`stub_device` の `mod tests` を含む全部 | `make verify` |
| `make build-profiles` の core の `graphics` だけの組（`scripts/build-profiles.sh:93`〜`:94`） | clippy（`-D warnings`）と test。graphics の doctest と `device_trait_shape` の `default_graphics_is_none_through_dyn_device`（`stub_device_graphics` は `required-features` で外れる） | `cargo test -p fandhe-silicon-core --features graphics` |
| `make build-profiles` の core の `test-support,graphics` の組（`scripts/build-profiles.sh:95`〜`:96`） | clippy と test。上に加えて `stub_device_graphics` と `stub_device` の `mod tests` の graphics の試験 | `cargo test -p fandhe-silicon-core --features test-support,graphics` |
| `make build-profiles` の契約の crate の組（`scripts/build-profiles.sh:32`〜`:33`） | 契約の crate の `graphics` から core の `graphics` へ転送され、core の graphics module がビルドされる（core 自身の試験は流れない） | `cargo build -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics` |
| `make wasm-verify` の core の wasm32 型検査（`scripts/wasm-verify.sh:39`） | core の試験の crate の wasm32 の `cargo check`（実行はしない） | `cargo check -p fandhe-silicon-core --tests --all-features --locked --target wasm32-unknown-unknown` |
| `make public-api-check` の all-features の組（`scripts/public-api-check.sh:307`） | core の公開 API の文書化に graphics の項目が入る | `make public-api-check` |

## 型の試験（4/4）

外の crate から公開のパスだけで描画の型と `GraphicsOps` の形を確かめる統合テストを足した。`crates/core/tests/graphics_types.rs`（`crates/core/Cargo.toml` の `[[test]]` は `required-features = ["graphics"]`。`test-support` は要らない）。局所の型 `FixedGraphics` に 10 操作を実装し、固定の値を返す。

| ファイル | 試験 | 確かめること |
| -------- | ---- | ------------ |
| `crates/core/tests/graphics_types.rs` | `graphics_ops_is_implementable_and_object_safe` | 局所の型で 10 操作を実装でき、`Box<dyn GraphicsOps>` 越しに 10 操作を呼べる。戻り値が固定の値と一致し、`submit_render` が記録先を値で受けて投入の印を返す |
| 同上 | `value_types_have_listed_variants` | `PixelFormat` 3 値・`DepthFormat` 1 値・`CompareFunction` 3 値が、ワイルドカードの腕つきの `match` で想定の番号になる |
| 同上 | `depth_config_is_built_from_outside` | 構造体式で `DepthConfig` を作れ、コピーの後も元を使え、欄の違いで `PartialEq` が分かれる |
| 同上 | `render_encoder_is_static` | `RenderEncoder` が `'static` |

```text
cargo test -p fandhe-silicon-core --features graphics --test graphics_types
```

流れる組（各組のログに `Running tests/graphics_types.rs` が出ることを確かめた）:

| 場所 | 事実 |
| ---- | ---- |
| `make verify`（`--all-features`） | 流れる |
| `make build-profiles` の core の `graphics` だけの組 | 流れる |
| `make build-profiles` の core の `test-support,graphics` の組 | 流れる |
| `make wasm-verify` の core の `--all-features --tests` の wasm32 型検査 | 型検査に乗る（実行はしない） |
| feature なしの `cargo test -p fandhe-silicon-core` | 流れない（`Running tests/graphics_types` が出ない） |

## 完了の条件の確かめ（4/4）

TASK-837 の 4/4（#5820）として、本リポで観測できる事実だけを記録する。GPU の実機のない Linux の隔離した作業ツリー（base `2caa325`）で流した出力による。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 署名どおりの型がビルドできる | `make verify`・`make build-profiles` の終了コード。`grep -n 'fn ' crates/core/src/graphics.rs` | 終了コードは両方 0。`__backend_*` を除く `fn` の行のうち、`GraphicsOps` の 10 操作が `:204`〜`:283` に並ぶ（残りは doctest の `fn f`） |
| 2 | `RenderEncoder`・`RenderTargetHandle` が公開の欄を持たない | `cargo test -p fandhe-silicon-core --features graphics --doc graphics`、`cargo test -p fandhe-silicon-core --doc render_target` | `graphics` は 9 件（通常 3・`compile_fail` 6）が `ok`。`RenderEncoder` の `line 119`（E0451）・`line 126`（E0599）は `compile fail ... ok`。`render_target` の `compile_fail` 2 件（`line 21`・`line 28`）も `ok` |
| 3 | `submit_render` が `RenderEncoder` を消費する | 同じ `--doc graphics` | `GraphicsOps::submit_render (line 269) - compile fail ... ok` |
| 4 | 公開 API に wgpu の型が 0 件（TASK-10） | `make public-api-check` | 終了コード 0。`documents=10 public_items_total=10388 wgpu_type_refs_total=0 wgpu_other_refs_total=0` |
| 5 | ほかの公開 API の検査 | `make public-name-check`・`make public-api-webgpu-check`・`make public-api-sdk-check`・`make core-deps-check` | 終了コードはすべて 0。`name_hits_total=0`・`webgpu_names_total=0`・`sdk_type_refs_total=0`・`core-deps-check` は `core_features=5 forwarded=5 forward_violations=0` |
| 6 | `Device::graphics()` と試験用の device（DEV-59・DEV-60・DEV-63） | `--features test-support,graphics --test stub_device_graphics`、`--features graphics --test device_trait_shape`、`--features test-support,graphics --lib stub_device` | 13 件・7 件・7 件がすべて `ok`。`stub_device_graphics` の試験名は 2/4 の表と一致、`default_graphics_is_none_through_dyn_device` と `graphics_create_reports_exhausted_id_space` も `ok` |
| 7 | 型の試験 | `cargo test -p fandhe-silicon-core --features graphics --test graphics_types` | `4 passed`（上の表の 4 本） |
| 8 | CI で流れる場所 | 3/4 の節の表。`grep -n 'run: make' .github/workflows/ci.yml`。`gh run list --branch main --workflow ci.yml` | 3/4 の表の呼び出し（`make verify`・`make build-profiles`・`make deny`・`make wasm-verify`・`make public-api-check` など）が `ci.yml` に並ぶ。run は 1/4 `b990f42`（41308803435）・2/4 `a5f399a`（41310450220）・3/4 `2caa325`（41312795055）がすべて success（completed） |
| 9 | 依存 0・unsafe の追加 0 | `git diff --stat b3e7d28 HEAD -- Cargo.lock deps-approved.toml`、`git diff --stat b3e7d28 HEAD -- crates/core/Cargo.toml crates/contract/Cargo.toml`、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n unsafe_code crates/core/src/lib.rs`、`grep -c unsafe crates/core/src/graphics.rs` | `Cargo.lock`・`deps-approved.toml` の差分は空。2 つの `Cargo.toml` の差分は 8 行追加・1 行削除（core の `graphics` と `[[test]]` の項目、契約の crate の転送の行）。承認の記録は 4 件。`lib.rs:31` は `#![forbid(unsafe_code)]`。`graphics.rs` の `unsafe` は 0 |
| 10 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | 終了コードはすべて 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-837）。確かめた base は `2caa325`。

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #5817 | 現状の形 |
| 2/4 | #5818 | device の口と試験用の device |
| 3/4 | #5819 | CI への組み込み |
| 4/4 | #5820 | 型の試験と完了の条件の確かめ |
