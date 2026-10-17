# T-1 の要素ごとの規則の確かめ（TASK-975）の所在と本リポの現状

TASK-975 の確かめを進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-975。ポインタのみ）。内容・理由・経緯は本書に置かない。
対象の項目は TASK-975 の記録に従う（ポインタのみ）。`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-24 | `crates/core/src/parity/tolerance.rs`・`cpu_reference.rs`・`topk.rs` | [parity-tool](parity-tool.md) |
| TASK-25 | `crates/core/src/parity/special_values.rs` | [parity-non-finite](parity-non-finite.md) |
| TASK-23 | `crates/core/src/test_frame` | [test-frame](test-frame.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験用の部品の feature | `core` の `lib.rs` で `parity`・`test_frame`・`rerun` が `#[cfg(feature = "test-support")]` の下にある。`hash` は条件なし。`[features]` は `test-support = []` | `crates/core/src/lib.rs`、`crates/core/Cargo.toml` |
| 一致の道具のファイル | `crates/core/src/parity` に 5 ファイル（`cpu_reference`・`pixel`・`special_values`・`tolerance`・`topk`） | `ls crates/core/src/parity` |
| 一致の道具の統合テスト | `core` は `crates/core/tests/parity_*.rs` の 7 本。`contract` は `test_support_forwarding.rs`・`test_support_pixel.rs`・`test_support_special_values.rs` | `ls crates/core/tests crates/contract/tests` |
| `required-features` の件数 | 20 | `grep -c required-features crates/core/Cargo.toml` |
| 依存 | `core` の外部依存は 0。`#![forbid(unsafe_code)]` | `crates/core/Cargo.toml`、`crates/core/src/lib.rs` |
| 走る場所 | `make verify`（`--all-features`）と、`make build-profiles` の `test-support` を含む組。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |

## (1) 要素ごとの規則の確かめ（TASK-975 の 2/6）

対応する項目は PARITY-1（ポインタのみ）。テストは `crates/core/tests/parity_element_rules.rs`。閾値の数値は本書に持たず、テストは `T1` の getter から導く。

| 振る舞い | テスト |
| -------- | ------ |
| 比較元の CPU 参照は融合積和で積み上げる。融合なしの手順で作った列は外れ、融合ありの値の列は一致する | `reference_is_built_with_fused_multiply_add` |
| 一致は「相対誤差が内、または絶対誤差が内」。相対だけ内・絶対だけ内・両方内は内、両方外は外 | `match_requires_relative_or_absolute_bound` |
| 最大値どうしの比べでは内に入る列でも、要素ごとに外れる要素があれば不一致。件数と最初の添字は要素ごとの判定と合う | `max_value_comparison_would_pass_but_elementwise_fails` |
| 長い列の 1 要素だけの外れ（先頭・中間・末尾）で不一致。平均なら内に収まる入力 | `single_outlier_in_a_long_sequence_is_a_mismatch` |
| 有限の列では `compare_f32_with_special_values` も同じ件数・最初の添字・一致の判定を返す | `finite_rule_is_the_same_through_special_values_comparator` |
| 要素ごと演算の出力に見立てた列（直書き）でも判定は要素ごとで、1 外れで不一致 | `elementwise_like_output_is_judged_per_element` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support --test parity_element_rules`

## (2)(3) 有限でない値の規則の確かめ（TASK-975 の 3/6）

対応する項目は PARITY-2・PARITY-3（ポインタのみ）。テストは `crates/core/tests/parity_element_rules.rs`。

| 振る舞い | テスト |
| -------- | ------ |
| 参照が NaN のとき、payload だけ・符号ビットだけが違う NaN は一致。既存の `compare_f32` では有限でない値として数えられる | `nan_reference_matches_nan_with_payload_only_difference` |
| CPU 参照が出した NaN は、別の payload の NaN と一致 | `nan_from_cpu_reference_matches_nan_with_another_payload` |
| 片側だけ NaN の組（有限・±Inf との組、両方向）は不一致。件数と最初の添字は要素ごとの判定と合う | `only_one_side_nan_is_a_mismatch` |
| payload だけ違う NaN の一致があっても、別の添字の外れ（NaN 片側・有限の外れ）で列全体が不一致 | `payload_only_nan_match_does_not_hide_other_mismatch` |
| 参照が ±Inf のとき、同じ符号の Inf だけ一致。NaN・符号違いの Inf・有限は不一致 | `infinite_reference_matches_only_same_sign_infinity` |
| 参照が有限で実測が NaN・±Inf なら不一致。`f32::MAX` に対する Inf も不一致 | `finite_reference_against_nan_or_infinity_is_a_mismatch` |
| CPU 参照が出した Inf を、符号反転・NaN・有限へ置き換えた要素の件数と最初の添字が合う | `infinity_from_cpu_reference_is_judged_per_element` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support --test parity_element_rules`

## (4) 参照が零の要素の規則の確かめ（TASK-975 の 4/6）

対応する項目は PARITY-5（ポインタのみ）。テストは `crates/core/tests/parity_element_rules.rs`。閾値の数値は本書に持たず、テストは `T1` の getter から導く。

| 振る舞い | テスト |
| -------- | ------ |
| 参照が零のとき、実測の零は符号を問わず一致。CPU 参照の零の符号は固定しない | `zero_from_cpu_reference_matches_zero_of_either_sign` |
| 参照が零のとき、絶対誤差が上限未満の零でない有限（正負・非正規化数の最小値を含む）は一致 | `zero_reference_with_tiny_finite_inside_absolute_bound_matches` |
| 参照が零のとき、絶対誤差が上限以上の有限は外れ（厳密な `<`）。上限そのものは `f32` で表せないため、上限を挟む隣接 `f32` 値の内外を確かめる。件数と最初の添字は要素ごとの判定と合う | `zero_reference_with_finite_outside_absolute_bound_is_a_mismatch` |
| 同じ大きさの誤差が、零でない参照には相対誤差で内、零の参照には外（相対誤差は零の参照に効かない） | `relative_error_is_not_used_for_zero_reference` |
| 零の参照が並ぶ列でも、NaN・Inf の実測は有限でない値の不一致として数える | `zero_reference_rule_does_not_hide_non_finite_mismatch` |
| 要素ごと演算の出力に見立てた列（直書き）の零の位置も、絶対誤差の上限だけで判定される | `elementwise_like_zeros_are_judged_by_absolute_bound` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support --test parity_element_rules`

## 既にある関連の部品

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。形の具体は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 値・規則を変える変更の扱いは [parity-tool](parity-tool.md) の「値・規則を変える変更の扱い」に従う
- 守る規約: [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[coding-rust](../.claude/rules/coding-rust.md)・[dependency-policy](../.claude/rules/dependency-policy.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)
