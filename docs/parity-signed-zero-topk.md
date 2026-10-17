# ±0 の扱いと Top-k の規則の確かめ（TASK-976）の所在と現状の事実

±0 と Top-k の比べについて、本リポのコードと公開テストから説明できる現状を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-976、対象ビヘイビアは `docs/spec/04-behavior/backend-parity.md` の PARITY-4・PARITY-6。ポインタのみ）。spec の本文・値・規則は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

前提タスクに当たる本リポの物は次のとおり。

| 前提 | 本リポの物 |
| ---- | ---------- |
| TASK-24 | `crates/core/src/parity/topk.rs`・`tolerance.rs`（[parity-tool](parity-tool.md)） |
| TASK-25 | `crates/core/src/parity/special_values.rs`（[parity-non-finite](parity-non-finite.md)） |
| TASK-23 | `crates/core/src/test_frame`（[test-frame](test-frame.md)） |

submodule（`docs/spec`）が指している版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| Top-k の道具の公開アイテム | `TopKEntry`・`TopKReferenceError`・`reference_topk_f32`・`TopKComparison`・`compare_topk` | `grep -n -E '^pub (fn\|struct\|enum)' crates/core/src/parity/topk.rs` |
| 参照の並べ方 | スコアは `f32::total_cmp` の降順、同点はスロット番号の昇順。有限でない値（NaN・±Inf）は候補から外す | `non_finite_scores_never_appear`（`crates/core/tests/parity_topk.rs`） |
| 参照が返す件数 | `min(k, 有限の件数)` 件。足りない分を埋める値は持たない | `fewer_finite_than_k_returns_only_finite_count`、`zero_k_and_empty_scores_give_empty_ok` |
| 比べの規則 | 位置ごとにスロット番号とスコアのビット列が等しいときだけ一致。閾値の引数は無い | `signed_zero_difference_is_caught`、`difference_within_t1_is_still_a_topk_mismatch` |
| 長さ違い・長さ 0 | 長さ違いは `CompareError::LengthMismatch`、長さ 0 は `EmptyInput` | `length_mismatch_and_empty_are_errors` |
| ±0 を含む入力（参照の側） | 入力 `[-0.0, 0.0]` で、参照は +0.0（スロット 1）を -0.0（スロット 0）より前に置く。既にある分 | `positive_zero_precedes_negative_zero` |
| 有限でないスコアのエントリの比べ | `TopKEntry::new(u32::MAX, f32::NEG_INFINITY)` の組も同じビット規則で比べる | `non_finite_entries_follow_the_same_bitwise_rule` |
| 要素ごとの比べ方での ±0 | 符号付きの零どうしは一致として扱う（Top-k のビット比較とは別の規則） | `signed_zeros_match_each_other`、`zero_reference_switches_at_absolute_boundary`、`verdict_matrix_over_value_kinds_is_fixed`（`parity_special_values.rs`）、`zero_reference_uses_absolute_only`（`parity_tolerance.rs`） |
| 試験の枠とのつながり | `impl From<&TopKComparison> for Verdict` がある | `grep -n 'TopKComparison' crates/core/src/test_frame/outcome.rs` |
| 別 crate からの利用 | `contract` の試験が `compare_topk`・`reference_topk_f32` を呼ぶ | `grep -n -E 'compare_topk\|reference_topk_f32' crates/contract/tests/test_support_forwarding.rs` |
| feature・依存・`unsafe` | `core` の `test-support` の裏。外部依存 0。`core` は `#![forbid(unsafe_code)]` | `grep -n -A2 '\[features\]' crates/core/Cargo.toml`、`grep -n forbid crates/core/src/lib.rs` |
| 走る場所 | `core` の `parity_topk` は `make verify`（`--all-features`）で走る。`contract` の `test_support_*` は `make build-profiles` の `test-support` を含む組でも走る。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。後の段で足すテストと入力は、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 値・規則を変える変更の扱いは [parity-tool](parity-tool.md) に従う
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
