# 描画先の大きさとバイト数の純粋関数（TASK-374）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-374、項目 `TEX-8`・`TEX-9`・`TEX-10`。

## 所在

- 実装: `crates/core/src/graphics/target_check.rs`
- 公開のパス: `fandhe_silicon_core::graphics::`（feature `graphics` の裏。`graphics` モジュールは [graphics-types.md](graphics-types.md)）

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `pixel_format_bytes` | `PixelFormat` | `u32` |
| `depth_format_bytes` | `DepthFormat` | `u32` |
| `offscreen_target_bytes` | 幅・高さ・1 画素のバイト数（いずれも `u32`） | `Result<u64, ContractError>` |
| `validate_offscreen_target_size` | 幅・高さ・上限（いずれも `u32`） | `Result<(), ContractError>` |

## 現状の振る舞い

- `pixel_format_bytes`: `Rgba8Unorm` は 4、`Bgra8Unorm` は 4、`Rgba32Float` は 16
- `depth_format_bytes`: `Depth32Float` は 4
- `offscreen_target_bytes`: 1 画素のバイト数 × 幅 × 高さを `u64` で求める。積が `u64` であふれるときは `ContractError::InvalidArgument`（文言は英語で `overflows u64` を含む）。0 や上限は見ない（0 を渡せば `Ok(0)`）
- いずれも背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）

## 対応するテスト

- doctest 4 本（各関数の `///`）
- `mod tests` 10 本（下の 2/4 の 5 本を含む）: `target_check_pixel_format_bytes`・`target_check_depth_format_bytes`・`target_check_bytes_1280x720`・`target_check_bytes_overflow_is_rejected`・`target_check_bytes_max_without_overflow`

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --lib target_check
cargo test -p fandhe-silicon-core --features graphics --doc target_check
make verify
```

## 入力の検査（2/4）

`validate_offscreen_target_size(width, height, max_texture_size)` は、幅か高さが 0 のとき、または幅か高さが上限を超えるときに `ContractError::InvalidArgument` を返し、上限ちょうどは通す。上限は呼び出し側が `u32` で渡す。

| 上限 | 幅 × 高さ | 返し |
| ---- | --------- | ---- |
| 16384 | 1280 × 720 | `Ok(())` |
| 16384 | 0 × 720、1280 × 0、0 × 0 | `InvalidArgument` |
| 16384 | 16384 × 16384 | `Ok(())` |
| 16384 | 16385 × 1、1 × 16385 | `InvalidArgument` |
| `u32::MAX` | `u32::MAX` × `u32::MAX` | `Ok(())` |
| 0 | 1 × 1 | `InvalidArgument` |

0 と上限超過がともに当たる入力でも返しは `InvalidArgument` で、文言は固定しない（試験は variant まで確かめる）。

- 試験: `target_check_size_zero_is_rejected`・`target_check_size_over_limit_is_rejected`・`target_check_size_limit_exact_passes`・`target_check_size_u32_max_limit_passes`・`target_check_size_zero_limit_rejects`、および関数の `///` の doctest
- 確かめ方: 上の `--lib target_check` と `--doc target_check` がそのまま拾う

## 境界の入力の試験（3/4）

試験ファイルは `crates/core/tests/graphics_target_check.rs`。外の crate から公開のパス `fandhe_silicon_core::graphics::` だけを使い、feature `graphics` の裏でだけ走る。`LimitsTable` の欄は TASK-157 のポインタ。

| 試験 | 確かめること |
| ---- | ------------ |
| `zero_width_or_height_is_rejected` | 上限 16384 で 0 × 720・1280 × 0・0 × 0 が `InvalidArgument` |
| `limit_exact_is_accepted` | 上限 16384 で 16384 × 16384・16384 × 1・1 × 16384 が `Ok(())` |
| `limit_plus_one_is_rejected` | 上限 16384 で 16385 × 1・1 × 16385・16385 × 16385 が `InvalidArgument` |
| `limit_from_limits_table` | `LimitsTable` の `max_texture_size` を 8 にして渡すと 8 × 8 が `Ok(())`、9 × 8・8 × 9 が `InvalidArgument` |
| `rgba32float_bytes_are_16_per_pixel` | `Rgba32Float` が 16、1280 × 720 が 14,745,600、16384 × 16384 が 4,294,967,296（`u64`） |
| `color_and_depth_bytes` | `Rgba8Unorm`・`Bgra8Unorm`・`Depth32Float` が 4、1280 × 720 が 3,686,400 |
| `byte_count_overflow_is_rejected` | `u32::MAX` × `u32::MAX` が 1 画素 16 バイトでも 4 バイトでも `InvalidArgument` |
| `rejection_is_repeatable` | 拒否の入力も受け入れる入力も、同じ引数で 2 回呼ぶと同じ結果 |
| `rejection_is_invalid_argument_only` | 拒否はどれも `InvalidArgument` |

- 確かめ方: `cargo test -p fandhe-silicon-core --features graphics --test graphics_target_check`。`make verify` と `make wasm-verify` の型検査も拾う

## 完了の条件の確かめ（4/4）

4/4（#4576）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-374。項目 `TEX-8`・`TEX-9`・`TEX-10`）。GPU の実機なしで実行し、件数は実測。コードと試験は変えていない。

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 部品が共通の crate にある | `grep -n 'mod target_check\|pub use target_check' crates/core/src/graphics.rs`、`cargo check -p fandhe-silicon-core --features graphics` | `graphics.rs` の 327 行目と 328 行目、終了コード 0 |
| (2) | 0 の拒否（`TEX-9`） | `--test graphics_target_check` の `zero_width_or_height_is_rejected`、`--lib target_check` の `target_check_size_zero_is_rejected`・`target_check_size_zero_limit_rejects` | 3 本とも `ok` |
| (3) | 上限ちょうどを通し上限 + 1 を拒む（`TEX-8`） | `limit_exact_is_accepted`・`limit_plus_one_is_rejected`・`limit_from_limits_table`（`LimitsTable` は TASK-157 のポインタ）、`target_check_size_limit_exact_passes`・`target_check_size_over_limit_is_rejected`・`target_check_size_u32_max_limit_passes` | 6 本とも `ok` |
| (4) | バイト数の計算と `Rgba32Float`・深度の値（`TEX-10`） | `rgba32float_bytes_are_16_per_pixel`・`color_and_depth_bytes`、`target_check_pixel_format_bytes`・`target_check_depth_format_bytes`・`target_check_bytes_1280x720`、`--doc target_check` | 5 本とも `ok`、doctest 4 件 passed |
| (5) | あふれの拒否 | `byte_count_overflow_is_rejected`、`target_check_bytes_overflow_is_rejected`・`target_check_bytes_max_without_overflow`、`rejection_is_repeatable`・`rejection_is_invalid_argument_only` | 5 本とも `ok` |
| (6) | 背後実装を呼ばない | `grep -n '^use ' crates/core/src/graphics/target_check.rs`、`git diff --stat 4de70d8 3240ee9 -- crates/core/src/stub_device.rs crates/contract` | `use` は 2 行（`super::`・`crate::`）、差分 0 |
| (7) | 依存 0・unsafe 0 | `git diff --stat 4de70d8 3240ee9 -- Cargo.lock deps-approved.toml`、`git diff 4de70d8 3240ee9 -- crates/core/Cargo.toml`、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -c unsafe crates/core/src/graphics/target_check.rs`、`grep -n forbid crates/core/src/lib.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | `Cargo.lock`・`deps-approved.toml` は差分なし。`crates/core/Cargo.toml` の差分は `[[test]]` 1 項目（`graphics_target_check`、`required-features = ["graphics"]`）だけ。承認の記録 4 件、`unsafe` 0 件、33 行目に `#![forbid(unsafe_code)]`、core の外部依存 0 件 |
| (8) | 試験の件数と CI | `cargo test -p fandhe-silicon-core --features graphics` の `--test graphics_target_check`・`--lib target_check`・`--doc target_check`、`gh run list --branch main --workflow ci.yml` | 9 passed・10 passed・4 passed。CI は 1/4 `8edda34` の run 41346789572 が success、2/4 `eaeae4d` の run 41348216175 が success、3/4 `3240ee9` の run 41349529572 は記録時点で in_progress |
| (9) | 品質ゲート（本節を足した状態） | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | 6 つとも終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-374）。確かめた base は `3240ee92e817682a381196c4445a37d3d154b6e3`。

### 再実行の契機

- `crates/core/src/graphics/target_check.rs`・`crates/core/tests/graphics_target_check.rs` を変えたとき
- `PixelFormat`・`DepthFormat` に値が増えたとき
- 公開 API の検査（`scripts/public-api-check.sh` ほか）を変えたとき

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #4573 | 本頁の関数 |
| 2/4 | #4574 | `validate_offscreen_target_size`（入力の検査） |
| 3/4 | #4575 | `crates/core/tests/graphics_target_check.rs`（境界の入力の試験） |
| 4/4 | #4576 | 完了の条件の確かめ（本頁の 4/4 の節） |

関連: 描画先からバッファへの写しの書き込み先の検査は [copy-target-check.md](copy-target-check.md)。

## 今の値の補い（base 2f36d3d）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

記録の値は、完了の条件の確かめ（4/4）の表の値（過去の補いは無い）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 確かめ（4/4）の (1) | `graphics.rs` の 327 行目と 328 行目 → 1037 行目（`mod target_check;`）と 1038 行目（`pub use target_check::{`） | `grep -n 'mod target_check\|pub use target_check' crates/core/src/graphics.rs` |
