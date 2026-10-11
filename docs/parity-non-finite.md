# 有限でない値の規則の追加（TASK-25）の現状と前提

TASK-25（親 Issue #109）について、着手時点（1/4、#110）の現状と前提、および 2/4（#111）で入れたコードの現状の形を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-25 を参照する（ポインタのみ）。
本書は現状の挙動だけを書き、あるべき規則との差は書かない。「着手時点の事実」は着手時点の記録として残し、書き換えない。
後続（2/4 の #111、3/4 の #112、4/4 の #113）は、本書の該当節へ追記する。

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
| テスト | `[[test]]` 1 件（`parity_special_values`、`required-features = ["test-support"]`） | `crates/core/Cargo.toml` |

## オーナー判断待ち（未承認の仮置き）

[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) に従い、推奨を既定として確定しない。回答があるまで、下の仮置きは確定扱いにしない。

| 論点 | 仮置き | 選択肢（列挙のみ） | 止まるもの |
| ---- | ------ | ------------------ | ---------- |
| 1. 置き場 | `parity/special_values.rs` に新ファイル | `tolerance.rs` を変える／別の置き場 | TASK-25 の 3/4・4/4（#112・#113） |
| 2. 公開名 | `special_values` 系の名前 | 別名にする | 同上 |
| 3. 判定の区分 | 5 区分の別の型。`ElementVerdict` は変えない | `ElementVerdict` を細分化する（Q11 の経路） | 同上 |
| 4. 既存の集計との関係 | `Comparison` と別の集計の型 | `Comparison` に項目を足す（Q5 の続き） | 同上 |
| 5. 前提 | [parity-tool](parity-tool.md) の Q1・Q3・Q11 は未回答のまま着手 | 回答を待つ | TASK-24 の確定 |

## 形の論点（列挙のみ）

推奨は書かない。回答は無く、確定扱いにしない。規則の足し方・判定の区分・命名は 2/4 で仮置きした（次節「コードの現状の形」）。残るのは新しいテストの置き方・入力の作り方・`build-profiles` の組（3/4 の #112）。

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
| 3/4 | #112 | テスト |
| 4/4 | #113 | 確かめ |

親 #109 の「進み具合を spec に記録する」は spec リポ側の作業で、本リポからは書き換えない。

確かめた日付: 2026-10-11（base: 68145f4）
