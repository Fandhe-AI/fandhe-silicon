# 代わりの実行の結果の比べ方（TASK-928）の所在と本リポの現状

TASK-928 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-928。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-24 | `crates/core/src/parity/tolerance.rs`・`cpu_reference.rs` | [parity-tool](parity-tool.md) |
| TASK-25 | `crates/core/src/parity/special_values.rs` | [parity-non-finite](parity-non-finite.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 一致の道具のファイル | `parity` は 11 個の `pub mod`（`binary16`・`cpu_reference`・`fallback_compare`・`gemm_gate`・`gemm_inputs`・`gemm_population`・`pixel`・`special_values`・`tolerance`・`topk`・`topk_inputs`）で束ねる。比べ方の追加は配下にファイルを足して `pub mod` で束ねる流儀 | `ls crates/core/src/parity`、`grep -n 'pub mod' crates/core/src/parity.rs` |
| 有限でない値の比べ方 | `SpecialVerdict`・`judge_element_with_special_values`・`SpecialComparison`・`compare_f32_with_special_values`。入力は f32 で、両方が有限なら `tolerance::judge_element` へ委ねる | `crates/core/src/parity/special_values.rs`、`crates/core/tests/parity_special_values.rs` |
| 有限の比べ方 | `Tolerance`・`ElementVerdict`・`judge_element`・`Comparison`・`CompareError`・`compare_f32` | `crates/core/src/parity/tolerance.rs`、`crates/core/tests/parity_tolerance.rs` |
| binary16 の変換 | `f32_to_binary16_bits`・`binary16_bits_to_f32`・`round_slice_to_binary16`・`widen_binary16_slice`。ビット列は `u16` で持つ | `crates/core/src/parity/binary16.rs`、`crates/core/tests/parity_binary16.rs` |
| 統合テスト | `crates/core/tests` の `parity_*` は 15 本。`crates/core/Cargo.toml` の `required-features` は 29 件 | `ls crates/core/tests \| grep -c '^parity_'`、`grep -c required-features crates/core/Cargo.toml` |
| feature | `parity`・`test_frame`・`rerun` は `test-support` の裏にある。`test-support = []` で、`contract` の `test-support` が `fandhe-silicon-core/test-support` へ転送する | `crates/core/src/lib.rs`、`crates/core/Cargo.toml`、`crates/contract/Cargo.toml` |
| `exec` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格。`#![forbid(unsafe_code)]` | `git ls-files crates/exec` |
| 依存・unsafe | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。`deps-approved.toml` の承認は 0 件 | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`deps-approved.toml` |
| 走る場所 | `core` の `parity_*` は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-928 の言及は本書、`crates/core/src/parity.rs`、`crates/core/src/parity/fallback_compare.rs`、`crates/core/tests/parity_fallback_compare.rs`、`CLAUDE.md`、`README.md` にある | `git grep -n TASK-928 -- . ':!docs/spec'` |

## 所在（2/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | `crates/core/src/parity/fallback_compare.rs`（`test-support` の裏。標準ライブラリだけ、確保も panic もしない） | `grep -n 'pub mod' crates/core/src/parity.rs` |
| 公開名 | `OrderVerdict`・`judge_order_f32`・`judge_order_binary16`・`FallbackVerdict`・`judge_bits_u32`・`judge_bits_f32`・`FallbackComparison`・`compare_bits_u32`・`compare_bits_f32`。エラーは `tolerance::CompareError` を再利用する | `crates/core/src/parity/fallback_compare.rs` |
| 試験 | `crates/core/tests/parity_fallback_compare.rs`（3/8・4/8 の分を含めて 29 本） | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

## 振る舞い（2/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 順序の前段 | 有限でない値の 3 区分は `special_values::judge_element_with_special_values` の結果をそのまま写す。`special_values` は変えていない | `order_agrees_with_special_values_on_non_finite` |
| NaN | 両方が NaN なら `BothNan`（payload・符号を問わない）。片方だけなら `NonFiniteMismatch` | `order_nan_pairs_are_both_nan`・`order_single_nan_is_a_mismatch` |
| 無限大 | 同じ符号どうしだけ `SameInfinity`。逆の符号や有限との組は `NonFiniteMismatch` | `order_infinity_requires_the_same_sign` |
| 零 | 両方が零なら符号を問わず `BothZero`。零と最小の非正規化数は `Numeric` | `order_signed_zeros_are_both_zero` |
| 数値の条件へ回す組 | 上のどれにも当たらない有限どうしは `Numeric`。`is_decided` は `Numeric` 以外で真 | `order_finite_non_zero_pairs_go_to_numeric` |
| binary16 | `u16` のビット列を `binary16_bits_to_f32` で広げて同じ前段に通す。65504 と +Inf は前段で `NonFiniteMismatch` に決まる | `binary16_order_kinds`・`binary16_overflow_neighbour_is_decided_before_numeric`・`binary16_same_bits_never_mismatch_in_order` |
| ビット同一 | u32 は値が等しいときだけ `BitIdentical`。f32 は前段で決まらなかったときだけ `to_bits` を比べる。−0 と +0 は前段で `BothZero` になる | `bits_u32_identity`・`bits_f32_after_order` |
| 列の比べ | 長さ違いが先、次に長さ 0 で、検査のあとで添字を読む。集計は比べた数・不一致の数・最初の不一致の添字 | `compare_bits_errors_and_summary` |

## 所在（3/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/8 と同じ `fallback_compare.rs` に足す（新しいファイルは足さない） | `crates/core/src/parity/fallback_compare.rs` |
| 公開名 | `BINARY16_MAX_ORDER_DISTANCE`・`binary16_order_distance`・`judge_order_distance_binary16`・`compare_order_distance_binary16`・`count_bit_identical_binary16`。`FallbackVerdict` に `WithinOrderDistance`・`OrderDistanceExceeded` を足す | `grep -n 'pub ' crates/core/src/parity/fallback_compare.rs` |
| 試験 | 同じ `parity_fallback_compare.rs` に 9 本を足す | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

## 振る舞い（3/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 順序の差 | binary16 のビット列を符号つきの整数の鍵に写し、その差を取る。+0 と −0 は同じ鍵で、隣り合う値で 1。両端でもオーバーフローしない | `order_distance_mapping` |
| 一致の条件 | 前段で決まらない組は、差が `BINARY16_MAX_ORDER_DISTANCE`（1）以下なら `WithinOrderDistance`（一致）、超えれば `OrderDistanceExceeded`。ビット同一の組も `WithinOrderDistance` | `order_distance_neighbours`・`order_distance_same_bits_all_patterns` |
| 非正規化数・零をまたぐ組 | 非正規化数も 0 に落とさずこの条件で比べる。零をまたぐ組も鍵で数える | `order_distance_subnormals`・`order_distance_across_zero` |
| 前段が先 | 最大の有限値と +Inf のようにビット列が隣り合う組は、前段で `NonFiniteMismatch` に決まり、差の条件へ進まない | `order_distance_overflow_neighbour_is_not_within`・`order_distance_non_finite_follows_order` |
| 列の比べ | 長さ違いが先、次に長さ 0。集計の形は 2/8 と同じ | `compare_order_distance_errors_and_summary` |
| ビット同一の記録 | `count_bit_identical_binary16` はビット列が等しい要素の数を返す。判定には使わない | `count_bit_identical_records_only` |

## 所在（4/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/8 と同じ `fallback_compare.rs` に足す（新しいファイルは足さない） | `crates/core/src/parity/fallback_compare.rs` |
| 公開名 | `SUM_RELATIVE_BOUND`・`judge_relative_sum_f32`・`relative_sum_margin`・`compare_relative_sum_f32`。`FallbackVerdict` に `WithinRelativeSum`・`RelativeSumExceeded` を足す | `grep -n 'pub ' crates/core/src/parity/fallback_compare.rs` |
| 試験 | 同じ `parity_fallback_compare.rs` に 8 本を足す | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

## 振る舞い（4/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 前段が先 | 前段で決まる組はその区分に写し、このとき `abs_sum` は見ない | `relative_sum_order_first` |
| 一致の条件 | 前段で決まらない組は、誤差（f64）が `SUM_RELATIVE_BOUND`（1e-3）* `abs_sum` 以下なら `WithinRelativeSum`、超えれば `RelativeSumExceeded`。境目は含み、割り算は使わない | `relative_sum_boundary_is_inclusive`・`relative_sum_within_and_exceeded` |
| `abs_sum` が 0 | 誤差が正なら不一致、誤差 0 なら一致 | `relative_sum_zero_abs_sum` |
| `abs_sum` が使えない値 | 負・NaN・無限大は不一致に倒す | `relative_sum_invalid_abs_sum_fails_closed` |
| 余裕 | `relative_sum_margin` は誤差 / `abs_sum`。前段で一致は 0、前段で不一致は +Inf。記録だけに使い、判定は `judge_relative_sum_f32` だけが決める | `relative_sum_margin_values` |
| 列の比べ | 長さの検査の順は参照と実測、`abs_sums`、`margins`、長さ 0。エラーのとき `margins` は書き換えない | `compare_relative_sum_errors` |
| 余裕の全件記録 | 検査が通れば全添字の余裕を `margins` に書く。集計の形は 2/8 と同じ | `compare_relative_sum_summary_and_margins` |

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/8 現状と前提の確認 | #6033 |
| 2/8 | #6034 |
| 3/8 | #6035 |
| 4/8 | #6036 |
| 5/8 | #6037 |
| 6/8 | #6038 |
| 7/8 | #6039 |
| 8/8 | #6040 |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。後の段で足す振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [coding-rust](../.claude/rules/coding-rust.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[parity-element-rules](parity-element-rules.md)・[test-frame](test-frame.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
