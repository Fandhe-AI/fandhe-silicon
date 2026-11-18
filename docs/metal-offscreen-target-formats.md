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

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-380。ポインタのみ）。

時点のコミット: `597f292e`。`docs/spec` の gitlink は `becfefdc2fb47fa610c6ecc914f3bf601910f5f3`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `offscreen_format_device_tests`・`offscreen_format_plan_tests` の `#[test]`・`#[ignore` の数は `awk` で module を切り出して `grep -c`。`crates/contract/tests/backend_metal_offscreen_formats.rs` の `grep -c '#\[test\]'`、`grep -c '^[[:space:]]*#\[ignore'` | `offscreen_format_device_tests` の `#[test]` は 2、`#[ignore` は 2。`offscreen_format_plan_tests` の `#[test]` は 1。ファイルの `#[test]` は 1、`#[ignore` は 1 |
| 2 | 受け手の試験の名前 | 下の (2) の `git grep` | 下の (2) の出力（4 行） |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-metal,graphics --lib offscreen_format_plan_tests` | `test result: ok. 1 passed; 0 failed` |
| 4 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,graphics -- -D warnings`、同じコマンドの `--features backend-metal,native-blocking-wait`、`--all-features` | 3 組とも終了コード 0 |
| 5 | CI の macOS のビルド | `gh run view 44365345054 --log --job 135017462641`（`make verify + build-profiles (macos-latest)`）の `backend_metal_offscreen_formats` の部分。run は `597f292e` の main の push | `Running tests/backend_metal_offscreen_formats.rs` の行は 2 か所。`make verify` の部分は `test result: ok. 0 passed; 0 failed; 1 ignored`。run の conclusion `success`、headSha `597f292ee979a7106af245993f5897dd742b7ae8` |
| 6 | 依存 0・unsafe 0 | `git diff --stat 597f292e^..HEAD -- Cargo.lock deps-approved.toml crates/core crates/contract/tests/backend_metal_unsafe_count.rs`、`make deps-check`、`cargo test -p fandhe-silicon-contract --test backend_metal_unsafe_count` | 1 つ目は出力なし、`make deps-check` は終了コード 0、`test result: ok. 1 passed; 0 failed` |
| 7 | CI | `gh run view 44365345054 --json conclusion,headSha,jobs` | `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`、headSha `597f292ee979a7106af245993f5897dd742b7ae8` |

(6) の `597f292e^` は、1/2 の 1 つ前のコミット。

(2) の出力（`git grep -n "fn plan_counts_four_and_sixteen_bytes_per_pixel\|fn bgra8unorm_target_is_bgra8_with_four_bytes_per_pixel\|fn rgba32float_target_is_rgba32float_with_sixteen_bytes_per_pixel\|fn graphics_device_creates_bgra8unorm_and_rgba32float_targets" -- crates/contract`）。

```text
crates/contract/src/backend_metal.rs:2605:    fn plan_counts_four_and_sixteen_bytes_per_pixel() {
crates/contract/src/backend_metal.rs:9619:        fn bgra8unorm_target_is_bgra8_with_four_bytes_per_pixel() {
crates/contract/src/backend_metal.rs:9626:        fn rgba32float_target_is_rgba32float_with_sixteen_bytes_per_pixel() {
crates/contract/tests/backend_metal_offscreen_formats.rs:17:fn graphics_device_creates_bgra8unorm_and_rgba32float_targets() {
```

(5) の出力（`make verify` の部分）。

```text
Running tests/backend_metal_offscreen_formats.rs (target/debug/build/fandhe-silicon-contract/dfa1df1c7d95369b/out/backend_metal_offscreen_formats-dfa1df1c7d95369b)

running 1 test
test graphics_device_creates_bgra8unorm_and_rgba32float_targets ... ignored, touches the Metal device; run only in a dedicated device session

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | `Bgra8Unorm` と `Rgba32Float` の描画先の作成の試験 | #4595 |
| 2/2 | 完了の条件の確かめ | #4596 |
