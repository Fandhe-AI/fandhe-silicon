# Metal の背後実装の `Bgra8Unorm`・`Rgba32Float` の描画先の作成の試験（TASK-380）の所在と本リポの現状

TASK-380 の作業のうち、`Bgra8Unorm`・`Rgba32Float` の描画先の作成の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-380。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| device に触らない試験の module | `crates/contract/src/backend_metal.rs` の `imp` の外の `offscreen_format_plan_tests`。cfg は `all(test, feature = "graphics")` | 同左 |
| device の要る試験の module | 同じファイルの `imp` の中の `offscreen_format_device_tests`。cfg は `all(test, feature = "graphics")` | 同左 |
| 公開の口の試験のファイル | `crates/contract/tests/backend_metal_offscreen_formats.rs` | 同左 |
| `[[test]]` の feature | `backend-metal`・`graphics` | `crates/contract/Cargo.toml` |
| cfg | 公開の口の試験のファイルは `target_os = "macos"`。macOS 以外では 0 件 | 下の「確かめ方」 |
| 描画先の作成の口 | `docs/metal-offscreen-target.md`（TASK-377） | 同左 |
| 1 画素のバイト数の関数 | `docs/offscreen-target-check.md`（TASK-374） | 同左 |

## `Bgra8Unorm`・`Rgba32Float` の描画先の作成の試験（1/2）

3 か所の試験を足した。device の要る 3 本は `#[ignore]` が付く。

| 試験 | 呼ぶ口 | 入力 | 照らす値 |
| ---- | ------ | ---- | -------- |
| `plan_counts_four_and_sixteen_bytes_per_pixel`（lib。device に触らない） | `offscreen_target_plan` | `Bgra8Unorm`・`Rgba32Float` の各々で 1×1 と 1280×720、上限 16384、深度なし | `Ok((幅 × 高さ × バイト数, None))`。バイト数は `Bgra8Unorm` が 4、`Rgba32Float` が 16 |
| `bgra8unorm_target_is_bgra8_with_four_bytes_per_pixel`（lib。device の要る試験） | `create_offscreen_target`・`target_shape`・`offscreen_target_plan` | `Graphics` の device、`Bgra8Unorm`、1×1 と 1280×720 | 作成が `Ok`、`target_shape` が `BGRA8Unorm` の形（`Private`・`RenderTarget \| ShaderRead`・`Type2D`・mip 1・sample 1）、計画が `Ok((幅 × 高さ × 4, None))` |
| `rgba32float_target_is_rgba32float_with_sixteen_bytes_per_pixel`（lib。device の要る試験） | 同上 | `Graphics` の device、`Rgba32Float`、1×1 と 1280×720 | 作成が `Ok`、`target_shape` が `RGBA32Float` の形（ほかの欄は同じ）、計画が `Ok((幅 × 高さ × 16, None))` |
| `graphics_device_creates_bgra8unorm_and_rgba32float_targets`（公開の口。device の要る試験） | `MetalDeviceProvider::request`（`Metal`・`Graphics`・`ProductIndependent`）の `Device::graphics()` の `create_offscreen_target` | 2 形式の各々で 1280×720 | 作成が `Ok`、handle の `__backend_device_id()`・`__backend_generation()` が同じ device で確保したバッファの値と等しい |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,graphics --lib offscreen_format_plan_tests
cargo test -p fandhe-silicon-contract --features backend-metal,graphics --lib offscreen_format_device_tests
cargo test -p fandhe-silicon-contract --features backend-metal,graphics --test backend_metal_offscreen_formats
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,graphics -- -D warnings
```

- 1 本目は 1 件が通る。
- 2 本目の device の要る試験と 3 本目は、macOS 以外では 0 件。macOS では `ignored` になる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | `Bgra8Unorm` と `Rgba32Float` の描画先の作成の試験 | #4595 |
