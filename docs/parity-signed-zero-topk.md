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

## (1) の確かめ（TASK-976 の 2/6）

対応する項目は PARITY-4（ポインタのみ）。テストは `crates/core/tests/parity_signed_zero_topk.rs`。比べの側に絞り、参照の並び自体は既存の `positive_zero_precedes_negative_zero` が持つ。

| 振る舞い | テスト |
| -------- | ------ |
| GEMM の参照が -0.0 のとき、実測の +0.0 と要素ごとの比べで一致する | `gemm_reference_negative_zero_matches_positive_zero_actual` |
| GEMM の参照が +0.0 のとき、実測の -0.0 と要素ごとの比べで一致する | `gemm_reference_positive_zero_matches_negative_zero_actual` |
| 要素ごと演算風の列で零の符号だけの差は一致、零でない外れ値は不一致で位置が出る | `elementwise_like_output_treats_signed_zeros_as_equal` |
| IEEE の順に並べた Top-k は全順序の参照と不一致、全順序どおりなら一致する | `ieee_ordered_topk_is_a_mismatch_against_total_order_reference` |
| 長い Top-k の中で、零の 2 件の食い違いだけが不一致として位置つきで出る | `signed_zero_mismatch_is_localized_within_longer_topk` |
| 同じ ±0 の組が、要素ごとでは一致、Top-k では不一致になる | `same_signed_zero_pair_matches_elementwise_but_not_in_topk` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support --test parity_signed_zero_topk`（`make verify` でも走る）

## (2) の確かめ（TASK-976 の 3/6）

対応する項目は PARITY-6（ポインタのみ）。テストは同じ `crates/core/tests/parity_signed_zero_topk.rs`。入力は手書きの小さな列で、各テストは先に参照の並びを手計算の期待と照らしてから比べる。

| 振る舞い | テスト |
| -------- | ------ |
| 同点 2 件のスロットだけを入れ替えると、その 2 位置が不一致になる | `tie_pair_with_swapped_slots_is_a_mismatch` |
| 有限の件数が k に満たないとき、参照と実測を同じ値で k 件にそろえれば一致する（埋める値は複数の候補で確かめる） | `padded_reference_matches_identically_padded_actual` |
| 埋める位置に実の 1 件がある実測は、その位置で不一致になる | `real_entry_in_pad_position_is_a_mismatch` |
| 埋める値のスコアのビット列だけが違う（NaN の payload・±0）と不一致になる | `pad_with_different_bits_is_a_mismatch` |
| 埋める値を実の件より前に置くと、ずれた位置が不一致として数えられる | `pad_placed_before_real_entries_is_a_mismatch` |
| そろえていない参照と k 件の実測は `CompareError::LengthMismatch` になる | `unpadded_reference_against_k_length_actual_is_length_mismatch` |
| 有限でないスコアは参照に入らず、それを選んだ実測は位置 0 で不一致になる | `non_finite_scores_are_absent_and_selecting_them_is_a_mismatch` |
| 同点・有限でない値・±0 を混ぜた入力で、k が有限の件数より小さい場合と大きい場合の両方が位置ごとに比べられる | `mixed_input_with_ties_non_finite_and_signed_zeros` |
| テスト内の独立した選び方の結果が、同じスコア列への参照と一致する | `independent_selection_over_same_scores_equals_reference` |

- 比べる相手は同じスコア列から作った `reference_topk_f32` の出力。全量 readback の出力どうしの比べはこの試験の対象外（PARITY-13 のポインタのみ）
- 本リポは埋める値を定めない。テストは候補の値で、どれでも同じビット規則で比べられることを確かめる
- 参照は k 件にそろえない。長さをそろえるのは呼び出し側（テストでは補助関数 `pad_to_k`）

確かめ方: `cargo test -p fandhe-silicon-core --features test-support --test parity_signed_zero_topk`（`make verify` でも走る）

## 追加の入力の確かめ（TASK-976 の 4/6）

対応する項目は PARITY-4・PARITY-6（ポインタのみ）。入力は `crates/core/src/parity/topk_inputs.rs` に名前付きの定数として置き、テストは同じ `crates/core/tests/parity_signed_zero_topk.rs`。期待の並びは規則から手で計算した値で、テストが `reference_topk_f32` と照らす。

| 定数 | 入力が持つ形 | テスト |
| ---- | ------------ | ------ |
| `SIGNED_ZERO_INPUTS` | -0.0 のスロットが +0.0 のスロットより小さい。期待では +0.0 が先に来る（k の境で -0.0 が外れる入力を含む） | `signed_zero_inputs_mismatch_when_zeros_are_ieee_ordered` |
| `PAD_POSITION_INPUTS` | k が有限の件数より大きく、末尾に埋める位置ができる（全位置が埋める位置の入力を含む） | `pad_position_inputs_compare_after_padding_with_any_candidate` |
| `NON_FINITE_INPUTS` | NaN（payload・符号違い）・+Inf・-Inf を含み、期待には入らない | `non_finite_inputs_mismatch_when_a_non_finite_score_is_selected` |
| 全入力 | 期待が参照の出力と一致し、件数が `min(k, 有限の件数)`、種ごとの性質を持つ。独立した選び方とも一致する | `every_additional_input_reproduces_its_expected_order`、`additional_inputs_have_the_kind_they_claim`、`independent_selection_agrees_on_every_additional_input` |

- 公開アイテム: `TopKInput`・`SIGNED_ZERO_INPUTS`・`PAD_POSITION_INPUTS`・`NON_FINITE_INPUTS`（確かめ方: `grep -n -E '^pub (const\|struct)' crates/core/src/parity/topk_inputs.rs`）
- 埋める値は定めない。埋める位置の入力は「k が有限の件数より大きい」ことで表し、値は使う側が渡す
- feature `test-support` の裏だけに公開され、別の crate の試験からも同じ入力を使える

確かめ方: `cargo test -p fandhe-silicon-core --features test-support --test parity_signed_zero_topk`（`make verify` でも走る）

## 完了の条件の確かめ（5/6）

5/6（#6225）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-976）。
結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 本書の試験ファイル全体 | `cargo test -p fandhe-silicon-core --features test-support --test parity_signed_zero_topk` | `21 passed; 0 failed; 0 ignored` |
| A2 | 既存の Top-k の試験 | 同 `--test parity_topk` | `15 passed; 0 failed; 0 ignored` |
| A3 | (1) の期待（2/6） | A1 の出力で 2/6 の節の 6 本が `ok` | 6 本とも `ok` |
| A4 | (2) の期待（3/6・4/6） | A1 の出力で 3/6 の節の 9 本と 4/6 の節の 6 本が `ok` | 15 本とも `ok` |
| A5 | 比べる相手の明記 | `grep -n 'PARITY-13' crates/core/tests/parity_signed_zero_topk.rs docs/parity-signed-zero-topk.md` | 試験ファイルの `//!` と (2) の節の両方に在る。同じスコア列への参照と比べる `independent_selection_over_same_scores_equals_reference`・`independent_selection_agrees_on_every_additional_input` は `ok` |
| A6 | 別 crate からの利用 | `make build-profiles` | 終了コード 0 |
| A7 | main の CI | 両試験ファイルを含む最新の完了した push run の `make verify (fmt + clippy + test)` ジョブの step | `make verify` の conclusion は success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0。`make verify` の出力に両試験ファイルの実行が出る |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-976）。

### 再実行の契機

- `crates/core/src/parity/topk.rs`・`topk_inputs.rs`・`tolerance.rs`・`special_values.rs` を変えたとき
- `crates/core/tests/parity_signed_zero_topk.rs`・`crates/core/tests/parity_topk.rs` を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `cff173c`、`docs/spec` の gitlink は `6e9fc5d`。

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。後の段で足すテストと入力は、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 値・規則を変える変更の扱いは [parity-tool](parity-tool.md) に従う
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
