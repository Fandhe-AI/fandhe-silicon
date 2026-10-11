# 有限でない値の規則の追加（TASK-25）の現状と前提

TASK-25（親 Issue #109）について、着手時点（1/4、#110）の現状と前提、および 2/4（#111）で入れたコードの現状の形を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-25 を参照する（ポインタのみ）。
本書は現状の挙動だけを書き、あるべき規則との差は書かない。「着手時点の事実」は着手時点の記録として残し、書き換えない。
後続（2/4 の #111、3/4 の #112、4/4 の #113）は、本書の該当節へ追記する。3/4（#112）ではテストを足した（「テストの現状（3/4）」）。

## 前提タスクの状態

| 前提 | Issue | 状態 | 根拠 |
| ---- | ----- | ---- | ---- |
| TASK-24 | #100 | CLOSED（sub-issue #101〜#108 は全てクローズ） | `gh issue view`。[parity-tool](parity-tool.md) は完了を宣言していない |

- [parity-tool](parity-tool.md) の Q1〜Q11 は未回答。置き場・feature・形は未承認の仮置き
- TASK-24 が確定するまで、TASK-25 の完了は宣言しない。TASK-24 を満たされたものとして扱わない

## 着手時点の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | member 4（`crates/` 配下）。パッケージも 4 で、外部依存は 0 | `cargo metadata --format-version 1 --all-features --locked` |
| 道具の置き場 | `crates/core/src/parity.rs` と `parity/`（`cpu_reference.rs`・`tolerance.rs`・`topk.rs`） | `crates/core/src/parity.rs` |
| feature | `core` の `test-support` の裏。`contract` の `test-support` が転送する。`default` には入っていない | 各 `Cargo.toml`、`crates/core/src/lib.rs` |
| 要素ごとの比べ | `judge_element` は `Within`・`Exceeded`・`NonFinite` の 3 区分を返す。`ElementVerdict` は `#[non_exhaustive]` でない | `crates/core/src/parity/tolerance.rs` |
| 要素ごとの比べでの有限でない値 | 参照か実測のどちらかが有限でなければ `NonFinite`。両方が NaN・両方が同じ向きの無限大でも `NonFinite` | 公開テスト `non_finite_values_are_flagged`（`crates/core/tests/parity_tolerance.rs`） |
| 要素ごとの比べでの符号付きの零 | 参照が零のとき、実測の零は符号によらず `Within` | 公開テスト `zero_reference_uses_absolute_only`（同ファイル） |
| `Comparison` の集計 | getter は `compared`・`exceeded`・`non_finite`・`first_mismatch`・`is_match` | `crates/core/src/parity/tolerance.rs` |
| Top-k の参照と比べ | 参照は有限でない値を含めない。比べは符号付きの零の差を不一致にし、有限でない値も同じビット比較の規則で扱う | 公開テスト `non_finite_scores_never_appear`・`signed_zero_difference_is_caught`・`non_finite_entries_follow_the_same_bitwise_rule`（`crates/core/tests/parity_topk.rs`） |
| `reference_gemm_f32` に有限でない入力を渡したとき | 公開テストなし。振る舞いは約束しない | `crates/core/tests/parity_cpu_reference.rs` |
| 有限でない値の規則だけを扱う専用のモジュール・テスト・文書 | 0 件（上記は既存のテスト内の一部） | リポ内検索 |
| テストの置き方 | `core` の `[[test]]` 3 件（`parity_cpu_reference`・`parity_tolerance`・`parity_topk`）。いずれも `required-features = ["test-support"]` | `crates/core/Cargo.toml` |
| `build-profiles` | `NATIVE_SETS` に `test-support` を含む組が 4 組ある（`test-support`・`backend-metal,test-support`・`backend-vulkan,test-support`・`backend-cuda,test-support`）。`make verify` は `--all-features` で検査する | `scripts/build-profiles.sh` |
| `docs/spec` | gitlink で `update = none`。`ci.yml` は取得しない | `.gitmodules`、`.github/workflows/ci.yml` |

## 関連タスクの状態と引き継ぎ

事実のみを書く。向きは決めない。

| 関連 | Issue | 状態 | 引き継ぎ |
| ---- | ----- | ---- | -------- |
| TASK-24 | #100 | CLOSED（完了は宣言されていない） | `parity` の置き場・feature は未承認の仮置き。TASK-25 の側で確定扱いにしない |
| TASK-23 | #91 | OPEN | 本書では触れない |
| TASK-53 | #196 | OPEN | 本書では触れない |
| TASK-26 | #114 | OPEN | `parity` 配下に新しいファイルを足しうる並行のタスク（[pixel-parity-tool](pixel-parity-tool.md) の D9）。`parity.rs` の `pub mod` の行で衝突しうる |
| TASK-6 | #35 | CLOSED | 持ち込む・除くの区分は決定の記録（private spec）に従う。[poc-carry-over](../.claude/rules/poc-carry-over.md) を参照 |

## コードの現状の形（2/4・未承認の仮置き）

本リポのコードと公開テストが示す事実のみ。確定扱いにしない。

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity/special_values.rs`。`parity.rs` に `pub mod special_values;` を 1 行足した。`test-support` の裏 | `crates/core/src/parity.rs` |
| 公開名 | `SpecialVerdict`・`judge_element_with_special_values`・`compare_f32_with_special_values` と、集計の型（getter は `compared`・`exceeded`・`non_finite_mismatch`・`first_mismatch`・`is_match`） | `crates/core/src/parity/special_values.rs` |
| 判定の区分 | `SpecialVerdict` は 5 区分（`BothNan`・`SameInfinity`・`NonFiniteMismatch`・`Within`・`Exceeded`） | 同上 |
| 有限どうし | `judge_element` へ委ねる。既存の `judge_element`・`compare_f32` は変えていない | 同上、公開テスト（`crates/core/tests/parity_special_values.rs`） |
| テスト | `core` の `[[test]]` 1 件（`parity_special_values`、`required-features = ["test-support"]`）。3/4 で `contract` の `[[test]]` 1 件（`test_support_special_values`、同 feature）を足した | 各 `Cargo.toml` |
| 走る場所（3/4） | `make verify`（`--all-features`）と、`make build-profiles` の `test-support` を含む 4 組。`build-profiles` の組は増やしていない | `scripts/build-profiles.sh` |

## テストの現状（3/4・未承認の仮置き）

本リポのコードと公開テストが示す事実のみ。`src` は変えていない。入力はテストの中でその場で組んだもので、既存の 12 件の期待値は変えていない。

### core（`crates/core/tests/parity_special_values.rs`）

| 観測できる振る舞い | 確かめ方（テスト名） |
| ------------------ | -------------------- |
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

CPU 参照に有限でない入力を渡したときの出力の要素ごとの種類・零の符号は固定していない（着手時点の事実のとおり、振る舞いは約束されていない）。入力の源としてだけ使っている。

### 別 crate からの確かめ（`crates/contract/tests/test_support_special_values.rs`）

| 観測できる振る舞い | 確かめ方（テスト名） |
| ------------------ | -------------------- |
| `contract` の `test-support` だけで道具に届く | `contract_test_support_enables_core_special_values_tool` |
| 背後実装の feature の組に依らず、区分と集計が同じ | `special_value_verdicts_are_fixed_under_any_backend_feature_set`・`special_value_sequence_summary_is_fixed_under_any_backend_feature_set` |
| 列のエラーが外から見える | `special_value_sequence_errors_are_reachable_from_outside_core` |

`exec`・`upper` への `test-support` の転送は足していない（使うコードが無い）。

## オーナー判断待ち（未承認の仮置き）

[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) に従い、推奨を既定として確定しない。回答があるまで、下の仮置きは確定扱いにしない。

2/4 の PR（#7025）はマージ済みで、仮置きは未承認のまま main にある（マージ済みは履歴の事実であり、承認ではない）。下の判断への回答は本リポでは確認できていない。承認はまだ得ていない。本 PR（3/4）も未承認の仮置きを含むため、オーナーの判断が出るまでマージしない。判断が出た後に、承認の記録と、実装・文書を整合させる。

| 論点 | 仮置き | 選択肢（列挙のみ） | 止まるもの |
| ---- | ------ | ------------------ | ---------- |
| 1. 置き場 | `parity/special_values.rs` に新ファイル | `tolerance.rs` を変える／別の置き場 | TASK-25 の 3/4・4/4（#112・#113） |
| 2. 公開名 | `special_values` 系の名前 | 別名にする | 同上 |
| 3. 判定の区分 | 5 区分の別の型。`ElementVerdict` は変えない | `ElementVerdict` を細分化する（Q11 の経路） | 同上 |
| 4. 既存の集計との関係 | `Comparison` と別の集計の型 | `Comparison` に項目を足す（Q5 の続き） | 同上 |
| 5. 前提 | [parity-tool](parity-tool.md) の Q1・Q3・Q11 は未回答のまま着手 | 回答を待つ | TASK-24 の確定 |
| 6. テストの入力の作り方（3/4） | 判定の区分ごとにその場で組んだ入力で固定する。PoC の入力は写さない | (a) 仮置きのまま、完了の条件との対応づけはオーナーが確かめる (b) PoC と同じ入力での確かめを、TASK-6 の決定の記録に沿う置き場・方法で別に行う | #113 での完了の条件との対応づけ、TASK-25 の完了の宣言 |
| 7. CPU 参照に有限でない入力を渡す使い方（3/4） | 入力の源としてだけ使い、出力の要素ごとの種類は固定しない | (a) 仮置きのまま (b) 出力の種類も公開テストで固定する（TASK-24 の側の範囲に触れる） (c) CPU 参照を通さない | TASK-24 の CPU 参照の範囲（[parity-tool](parity-tool.md) の Q4） |

## 形の論点（列挙のみ）

推奨は書かない。回答は無く、確定扱いにしない。規則の足し方・判定の区分・命名は 2/4 で、新しいテストの置き方・入力の作り方・`build-profiles` の組は 3/4 で仮置きした（前節「コードの現状の形」「テストの現状（3/4）」）。

- 規則の足し方の置き場: `parity/` に新しいファイルと `pub mod` 1 行か、`tolerance.rs` を変えるか
- `judge_element` の区分の扱い: 既存の区分を細かく分けるか、別の関数・型を足すか。既存の公開テストの期待値が変わる場合は [parity-tool](parity-tool.md) の「値・規則を変える変更の扱い（案）」と Q11 に従う
- `Comparison` の集計の項目（[parity-tool](parity-tool.md) の Q5 の続き）
- 要素ごとの比べと Top-k の比べで、符号付きの零の現状の扱いが異なる点をどう扱うか
- 新しいテストの置き方（`[[test]]` と `required-features`、`build-profiles` の組）
- テストの入力の作り方（PoC の入力・記録は複製しない。[poc-carry-over](../.claude/rules/poc-carry-over.md)、TASK-6）
- Q1・Q3・Q11 が未回答のまま #111 を進めるか
- 並行する TASK-26 との衝突の避け方

## 後続の分担

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 2/4 | #111 | 実装。済（未承認の仮置き） |
| 3/4 | #112 | テスト。済（未承認の仮置き） |
| 4/4 | #113 | 確かめ |

#112 の判断:

- `build-profiles` の組は増やしていない（既存の `test-support` を含む組で新しいテストが走る）
- `contract` 側のテストは新しいファイルにした
- `exec`・`upper` の利用口は足していない
- `src` は変えていない

親 #109 の「進み具合を spec に記録する」は spec リポ側の作業で、本リポからは書き換えない。

確かめた日付: 2026-10-11（base: 83ad4f0）
