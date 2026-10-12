# 有限でない値の規則の追加（TASK-25）の現状と前提

TASK-25（親 Issue #109）について、着手時点（1/4、#110）の現状と前提、および 2/4（#111）で入れたコードの現状の形を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-25 を参照する（ポインタのみ）。
本書は現状の挙動だけを書き、あるべき規則との差は書かない。「着手時点の事実」は着手時点の記録として残し、書き換えない。
後続（2/4 の #111、3/4 の #112、4/4 の #113）は、本書の該当節へ追記した。3/4（#112）ではテストを足した（「テストの現状（3/4）」）。4/4（#113）では本リポで観測できる事実による確かめを記録した（「完了の条件の確かめ」）。
置き場・公開名・形は確定している。判断の記録と、完了の条件との対応づけの記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-25 の状態。ポインタのみ）。

## 前提タスクの状態

| 前提 | Issue | 状態 | 根拠 |
| ---- | ----- | ---- | ---- |
| TASK-24 | #100 | CLOSED（sub-issue #101〜#108 は全てクローズ） | `gh issue view`。道具の形は [parity-tool](parity-tool.md) の「現状の形」のとおり |

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
| TASK-24 | #100 | CLOSED | `parity` の置き場・feature は [parity-tool](parity-tool.md) の「現状の形」のとおり。TASK-25 の側で変えない |
| TASK-23 | #91 | OPEN | 本書では触れない |
| TASK-53 | #196 | OPEN | 本書では触れない |
| TASK-26 | #114 | OPEN（実行時点） | `parity` 配下に新しいファイルを足しうる並行のタスク（[pixel-parity-tool](pixel-parity-tool.md) の D9）。`parity.rs` の `pub mod` の行で衝突しうる |
| TASK-6 | #35 | CLOSED | 持ち込む・除くの区分は決定の記録（private spec）に従う。[poc-carry-over](../.claude/rules/poc-carry-over.md) を参照 |

## コードの現状の形（2/4）

本リポのコードと公開テストが示す事実のみ。

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity/special_values.rs`。`parity.rs` に `pub mod special_values;` を 1 行足した。`test-support` の裏 | `crates/core/src/parity.rs` |
| 公開名 | `SpecialVerdict`・`judge_element_with_special_values`・`compare_f32_with_special_values` と、既存の `Comparison` とは別の集計の型 `SpecialComparison`（getter は `compared`・`exceeded`・`non_finite_mismatch`・`first_mismatch`・`is_match`） | `crates/core/src/parity/special_values.rs` |
| 判定の区分 | `SpecialVerdict` は 5 区分（`BothNan`・`SameInfinity`・`NonFiniteMismatch`・`Within`・`Exceeded`）。既存の `ElementVerdict` とは別の型で、`ElementVerdict` は変えていない | 同上 |
| 有限どうし | `judge_element` へ委ねる。既存の `judge_element`・`compare_f32` は変えていない | 同上、公開テスト（`crates/core/tests/parity_special_values.rs`） |
| テスト | `core` の `[[test]]` 1 件（`parity_special_values`、`required-features = ["test-support"]`）。3/4 で `contract` の `[[test]]` 1 件（`test_support_special_values`、同 feature）を足した | 各 `Cargo.toml` |
| 走る場所（3/4） | `make verify`（`--all-features`）と、`make build-profiles` の `test-support` を含む 4 組。`build-profiles` の組は増やしていない | `scripts/build-profiles.sh` |

## テストの現状（3/4）

本リポのコードと公開テストが示す事実のみ。`src` は変えていない。入力はテストの中でその場で組んだもので（PoC の入力・記録は写していない。[poc-carry-over](../.claude/rules/poc-carry-over.md)）、既存の 12 件の期待値は変えていない。

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

## 後続の分担

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 2/4 | #111 | 実装。済 |
| 3/4 | #112 | テスト。済 |
| 4/4 | #113 | 確かめ。済（「完了の条件の確かめ」に記録） |

#112 の判断:

- `build-profiles` の組は増やしていない（既存の `test-support` を含む組で新しいテストが走る）
- `contract` 側のテストは新しいファイルにした
- `exec`・`upper` の利用口は足していない
- `src` は変えていない

親 #109 の「進み具合を spec に記録する」は spec リポ側の作業で、本リポからは書き換えない。

## 完了の条件の確かめ（4/4）

本リポで観測できる事実だけを書く。コマンドは worktree のルートで実行した。spec の完了の条件の本文は転記しない（`docs/spec/05-tasks.md` の TASK-25 を参照。ポインタのみ）。

| 確かめる項目 | 方法 | 結果 |
| ------------ | ---- | ---- |
| 有限でない値の規則の道具に背後実装の feature での分岐が無い | `crates/core/src/parity.rs` と `parity/` を `backend`・`cfg(` で grep | `cfg(` は 0 件。`backend` は `special_values.rs` の doc コメントのポインタ行の 1 行だけ |
| `unsafe` を使っていない | `crates/core/src` を `unsafe` で grep | `lib.rs` の `#![forbid(unsafe_code)]` の 1 行だけ |
| 外部依存 0 のまま | `Cargo.lock` の `[[package]]` の数、`make deps-check` | 4（workspace の member のみ）、終了コード 0 |
| 道具が workspace の外に依存しない | `special_values.rs` の `use`、`make deny` | `use` は `super::tolerance` の 1 行だけ、`make deny` は終了コード 0 |
| 製品の経路から参照していない | `crates/exec`・`crates/upper`・`crates/contract/src` を `parity`・`special_values` で grep | 0 件 |
| `test-support` が `default` に入っていない | 各 `Cargo.toml` の `[features]` | `core` に `default` なし。`contract` の `default` は `contract-core`・`cpu-isa`・`cpu-ops` |
| 道具のテストが通る | `cargo test -p fandhe-silicon-core --features test-support` | `parity_special_values` 25 件、`parity_tolerance` 14 件、`parity_topk` 15 件、`parity_cpu_reference` 8 件、`parity_pixel` 22 件、doctest 3 件。失敗 0 |
| 既存の比べ方を変えていない | 既存の `parity_*` が期待値を変えずに通ること、`agrees_with_compare_f32_on_all_finite_sequences`・`differs_from_compare_f32_only_on_matching_special_pairs` | 上のとおり全て通過 |
| 値の種類の組ごとの区分・零の符号・NaN のビット列の幅の扱いが固定 | `verdict_matrix_over_value_kinds_is_fixed`・`zero_actual_sign_does_not_change_the_verdict`・`every_nan_bit_pattern_counts_as_nan`・`subnormals_are_treated_as_finite` | 通過 |
| 集計が要素ごとの判定と整合する | `summary_agrees_with_per_element_verdicts`・`single_element_sequences`・`first_mismatch_can_be_a_non_finite_mismatch` | 通過 |
| `contract` の `test-support` だけで `core` の外から届く | `make build-profiles` の `test-support` を含む 4 組で `contract_test_support_enables_core_special_values_tool` | 4 組とも通過（`test_support_special_values` は各組 4 件） |
| 背後実装の feature の組に依らず区分と集計が同じ | 同 4 組で `special_value_verdicts_are_fixed_under_any_backend_feature_set`・`special_value_sequence_summary_is_fixed_under_any_backend_feature_set` | 4 組とも通過 |
| 列のエラーが外から到達できる | 同 4 組で `special_value_sequence_errors_are_reachable_from_outside_core` | 4 組とも通過 |
| GPU の実機なしで走る | 上のテストを実機なしで実行 | 実機なしで通過。CI の結果は未検証（PR 作成前） |
| 品質ゲート | `make check`・`make verify`・`make build-profiles`・`make deny`・`make deps-check` | 全て終了コード 0 |
| spec の完了の条件との照合 | 記録の正は private spec 側（TASK-25 の状態。ポインタのみ） | 本書では行っていない |
| CPU 参照に有限でない入力を渡したときの出力の種類・適用の範囲（TASK-53）・TASK-23 の枠との接続 | 「テストの現状（3/4）」と [parity-tool](parity-tool.md) のまま | 該当なし（出力の種類は固定していない。範囲と枠は TASK-53（#196）・TASK-23（#91）に委ねる。本段では触れない） |

- 判定: 本リポで観測できる範囲では、上の項目を確かめられた。完了の条件との対応づけの記録の正は private spec 側（TASK-25 の状態）
- 再実行の契機: `parity` の置き場・feature の変更、区分・比べの規則の変更、TASK-53（#196）の範囲の決定、TASK-23（#91）の枠の追加、`scripts/build-profiles.sh` の組の変更

確かめた日付: 2026-10-11（base: ee6a825）
