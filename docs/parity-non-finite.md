# 有限でない値の規則（TASK-25）の所在と現状の事実

有限でない値と符号付きの零を扱う要素ごとの比べ方について、本リポのコードと公開テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-25。ポインタのみ）。spec の本文・値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity/special_values.rs`。`parity.rs` に `pub mod special_values;` を持つ。同じ `parity` 配下の道具は [parity-tool](parity-tool.md) | `crates/core/src/parity.rs` |
| feature | `core` の `test-support` の裏。`contract` の `test-support` が転送する。どの crate の `default` にも入っていない | 各 `Cargo.toml` |
| 依存 | 外部依存 0。`unsafe` を使わない（`core` は `#![forbid(unsafe_code)]`）。`special_values.rs` の `use` は `super::tolerance` だけ | `Cargo.lock`、`crates/core/src/parity/special_values.rs` |
| 既存の比べ方との関係 | 有限どうしは `tolerance.rs` の `judge_element` へ委ねる。`judge_element`・`compare_f32`・`ElementVerdict`・`Comparison` とは別の型を持ち、これらは変えていない | `crates/core/src/parity/special_values.rs`、`agrees_with_compare_f32_on_all_finite_sequences` |
| 利用側 | `exec`・`upper` に `test-support` の転送は無い | 各 `Cargo.toml` |
| 走る場所 | `core` の `parity_special_values` は `make verify`（`--all-features`）で走る。`contract` の `test_support_special_values` は `make verify` に加え、`make build-profiles`（feature の組の対象は `fandhe-silicon-contract` だけ）の `test-support` を含む 4 組でも走る。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` の `PKG`・`NATIVE_SETS` |

## 振る舞い

公開名は `SpecialVerdict`・`judge_element_with_special_values`・`compare_f32_with_special_values`・`SpecialComparison`（getter は `compared`・`exceeded`・`non_finite_mismatch`・`first_mismatch`・`is_match`）。

- `SpecialVerdict` は 5 区分: `BothNan`・`SameInfinity`・`NonFiniteMismatch`・`Within`・`Exceeded`。`is_match` が真になるのは `BothNan`・`SameInfinity`・`Within`
- 参照が NaN なら、実測も NaN のときだけ一致（payload と符号ビットは比べない）。参照が無限大なら、同じ符号の無限大のときだけ一致。参照が有限で実測が有限でないときは不一致。列挙していない組は不一致
- 両方が有限なら `judge_element` へ委ねる
- `compare_f32_with_special_values` は、長さ違いを `CompareError::LengthMismatch`、長さ 0 を `CompareError::EmptyInput` で返す（長さ違いの検査が先）

### core

テストは `crates/core/tests/parity_special_values.rs`。入力はテストの中でその場で組む。

| 振る舞い | テスト |
| -------- | ------ |
| 同じビット列を参照と実測に渡すと必ず一致 | `same_bit_pattern_always_matches` |
| 値の種類（NaN・±無限大・±0・有限）の組ごとの区分が固定 | `verdict_matrix_over_value_kinds_is_fixed` |
| 非正規化数は有限として扱われる | `subnormals_are_treated_as_finite` |
| NaN のビット列の幅（signaling の形・符号・payload）によらず NaN として扱われる | `every_nan_bit_pattern_counts_as_nan` |
| 参照が零でない有限のとき、実測の零の符号は結果を変えない | `zero_actual_sign_does_not_change_the_verdict` |
| getter で読んだ絶対誤差の境界で、参照が零のときの内外が切り替わる | `readable_absolute_bound_is_applied_for_zero_reference` |
| 相対誤差は参照に対して測られる（向きが結果を変える） | `relative_error_is_measured_against_the_reference` |
| 有限だけの列では既存の `compare_f32` と集計が一致する | `agrees_with_compare_f32_on_all_finite_sequences` |
| 一致とみなす特殊値の組だけが `compare_f32` と異なる | `differs_from_compare_f32_only_on_matching_special_pairs` |
| 集計が要素ごとの判定と整合する | `summary_agrees_with_per_element_verdicts`・`single_element_sequences` |
| 最初の不一致が有限でない値の不一致になりうる | `first_mismatch_can_be_a_non_finite_mismatch` |
| CPU 参照の出力（有限でない値を含む入力から得たものを含む）を比べに渡せる | `cpu_reference_output_feeds_the_comparison` |

CPU 参照に有限でない入力を渡したときの出力の要素ごとの種類・零の符号は固定していない。入力の源としてだけ使っている。

### 別 crate からの確かめ

テストは `crates/contract/tests/test_support_special_values.rs`。

| 振る舞い | テスト |
| -------- | ------ |
| `contract` の `test-support` だけで道具に届く | `contract_test_support_enables_core_special_values_tool` |
| 背後実装の feature の組に依らず、区分と集計が同じ | `special_value_verdicts_are_fixed_under_any_backend_feature_set`・`special_value_sequence_summary_is_fixed_under_any_backend_feature_set` |
| 列のエラーが外から見える | `special_value_sequence_errors_are_reachable_from_outside_core` |

## 確かめ方

worktree のルートで実行する。

- `cargo test -p fandhe-silicon-core --features test-support`: 道具の統合テスト（`parity_special_values` を含む）と doctest
- `cargo test -p fandhe-silicon-contract --features test-support --test test_support_special_values`: 別 crate からの確かめ
- `make build-profiles`: `contract` の feature の組ごとのビルド・clippy・test（`test-support` を含む組で `test_support_special_values` が走る）
- `make verify`: `--all-features` の fmt・clippy・test
- `make deny`・`make deps-check`: 依存が増えていないこと
- `make check`: editorconfig-checker と shellcheck

## 関連

[parity-tool](parity-tool.md)・[pixel-parity-tool](pixel-parity-tool.md)・[parity-fallback-compare](parity-fallback-compare.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
