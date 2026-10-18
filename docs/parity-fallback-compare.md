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
| 試験 | `crates/core/tests/parity_fallback_compare.rs`（12 本） | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

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
