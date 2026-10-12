# 画素の一致を確かめる共通の道具（TASK-26）の所在と現状の事実

画素の一致を確かめる共通の道具について、本リポのコードと公開テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-26。ポインタのみ）。spec の本文・閾値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity/pixel.rs`。`parity.rs` に `pub mod pixel;` を持つ。同じ `parity` 配下の道具は [parity-tool](parity-tool.md) | `crates/core/src/parity.rs` |
| feature | `core` の `test-support` の裏。`contract` の `test-support` が転送する。どの crate の `default` にも入っていない | 各 `Cargo.toml` |
| 依存 | 外部依存 0。`unsafe` を使わない（`core` は `#![forbid(unsafe_code)]`）。`pixel.rs` の `use` は `std::fmt` だけ | `Cargo.lock`、`crates/core/src/parity/pixel.rs` |
| 型 | 入力・閾値の設定・集計・エラーは `pixel.rs` の中の専用の型。`tolerance.rs` の型は使わない | `crates/core/src/parity/pixel.rs` |
| 入力 | メモリ上の列を受ける口だけを持つ。画像ファイルの読み込み・復号はしない。`baseline-wgpu/`（ルート `Cargo.toml` の `exclude` の別 workspace）を参照しない | `crates/core/src/parity/pixel.rs`、ルート `Cargo.toml` |
| 利用側 | `exec`・`upper` に `test-support` の転送は無い | 各 `Cargo.toml` |
| 走る場所 | `make verify` と、`make build-profiles` の `test-support` を含む 4 組（`crates/contract/tests/test_support_pixel.rs` 経由）。GPU の実機は要らない | `scripts/build-profiles.sh` の `NATIVE_SETS` |

## 振る舞い

### 入力の受け口

テストは `crates/core/tests/parity_pixel.rs`。

| 公開アイテム | 振る舞い | テスト |
| ------------ | -------- | ------ |
| `CHANNELS_PER_PIXEL` | 1 画素あたりの要素数（4。要素型は `u8`）を外から読める | `channel_count_is_readable` |
| `PixelView::new` | 正しい長さの列で作れ、`width()`・`height()`・`data()` が渡した値を返す。行の詰め物は受けない | `valid_input_exposes_what_was_passed`、doctest |
| `PixelView` | 構造体の式で直接作れない | `compile_fail` の doctest |
| `PixelInputError::ZeroSize` | 幅か高さが 0 のとき（列が空でなくても） | `zero_width_or_height_is_a_zero_size_error` |
| `PixelInputError::SizeOverflow` | 幅 × 高さ、またはチャネル数を掛けた積が溢れるとき。panic しない | `every_size_product_is_checked_for_overflow` |
| `PixelInputError::LengthMismatch` | 列の長さが求めた要素数と違うとき（期待値と実際の長さを持つ） | `length_mismatch_reports_expected_and_actual` |
| `Display` | 全 variant で空でない ASCII | `error_display_is_non_empty_ascii` |

### 比べる関数と閾値の設定

テストは `crates/core/tests/parity_pixel.rs`。

| 公開アイテム | 振る舞い | テスト |
| ------------ | -------- | ------ |
| `T3` / `PixelTolerance` | 閾値の設定。getter（`near_channel_diff()`・`max_channel_diff()`・`min_near_numerator()`・`min_near_denominator()`）で値を読め、外から任意の値を作れない（整数だけで持つ） | `t3_setting_is_readable`、`compile_fail` の doctest |
| `compare_pixels_exact` | 閾値を取らない。4 要素がすべて等しい画素だけが一致。比べた数・違った画素数・最初の添字を返す | `identical_images_match_on_both_comparisons`、`exact_comparison_counts_mismatched_pixels`、`exact_comparison_sees_an_alpha_only_difference`、`exact_comparison_is_symmetric` |
| `compare_pixels` | 閾値を引数で受ける。画素の差は 4 要素（アルファを含む）の差の最大で、向きは対称。「内」の割合の下限と全体の上限の両方を、境界を含めて判定する | `pixel_difference_is_the_largest_channel_difference`、`readable_bounds_are_the_ones_applied`、`ratio_boundary_is_inclusive`、`max_difference_alone_fails_the_match`、`comparison_is_symmetric`、`first_index_is_the_earliest_pixel_in_row_major_order`、`difference_within_near_bound_matches_with_tolerance_but_not_exactly`、`ratio_is_applied_to_images_smaller_than_the_denominator` |
| 割合の比較 | 整数の掛け合わせ（`u128`）で行い、溢れない | `extreme_values_do_not_overflow` |
| `PixelCompareError::SizeMismatch` | 幅か高さが違えば、要素数が同じでもエラー（両方の大きさを持つ）。不一致として数えない | `size_mismatch_is_an_error_even_with_equal_length` |
| `Display` | 空でない ASCII | `compare_error_display_is_non_empty_ascii` |

閾値の数値は本書に持たない。定義は `crates/core/src/parity/pixel.rs` の `T3` の 1 か所で、値・規則を変える変更の扱いは [parity-tool](parity-tool.md) の「値・規則を変える変更の扱い」と同じ。閾値を当てる範囲は本書では決めない（private spec 側の TASK-53 を参照。ポインタのみ）。

### 別 crate からの確かめ

テストは `crates/contract/tests/test_support_pixel.rs`。

| 振る舞い | テスト |
| -------- | ------ |
| `contract` の `test-support` だけで、`core` の画素の道具が使え、同じ画像どうしが一致する | `contract_test_support_enables_core_pixel_tool` |
| `core` の外の crate から `T3` の getter が読め、値どうしの関係が成り立つ | `pixel_threshold_setting_is_readable_from_outside_core` |
| 固定の小さな画像で、2 つの比べが背後実装の feature の組に依らず固定の結果になる（`make build-profiles` の各組で走る） | `pixel_comparisons_give_fixed_results_under_any_backend_feature_set` |
| 大きさ 0 の入力と大きさの不一致が、外から到達できるエラーになる | `pixel_input_and_size_errors_are_reachable_from_outside_core` |

## 確かめ方

worktree のルートで実行する。

- `cargo test -p fandhe-silicon-core --features test-support`: 画素の道具の統合テスト（`parity_pixel`）と doctest
- `cargo test -p fandhe-silicon-contract --features test-support --test test_support_pixel`: 別 crate からの確かめ
- `make build-profiles`: `test-support` を含む組を含む feature の組ごとのビルド・clippy・test
- `make verify`: `--all-features` の fmt・clippy・test
- `make deny`・`make deps-check`: 依存が増えていないこと
- `make check`: editorconfig-checker と shellcheck

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)・[baseline-wgpu](baseline-wgpu.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
