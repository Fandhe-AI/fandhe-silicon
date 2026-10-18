# 結果の一致の試験の共通の入力と参照（TASK-977）の所在と本リポの現状

TASK-977 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-977、対象の項目は `docs/spec/04-behavior/backend-parity.md` の PARITY-4。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-24 | `crates/core/src/parity/cpu_reference.rs`・`tolerance.rs`・`topk.rs` | [parity-tool](parity-tool.md) |
| TASK-25 | `crates/core/src/parity/special_values.rs` | [parity-non-finite](parity-non-finite.md) |
| TASK-975 | `crates/core/tests/parity_element_rules.rs`・`parity_element_inputs.rs` | [parity-element-rules](parity-element-rules.md) |
| TASK-976 | `crates/core/tests/parity_signed_zero_topk.rs`・`crates/core/src/parity/topk_inputs.rs` | [parity-signed-zero-topk](parity-signed-zero-topk.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature | `parity`・`test_frame`・`rerun` は `core` の `test-support` の裏にある。`hash` は条件なしで公開される | `crates/core/src/lib.rs`、`crates/core/Cargo.toml` |
| 一致の道具のファイル | `crates/core/src/parity` に 12 ファイルある | `ls crates/core/src/parity` |
| 統合テスト | `crates/core/tests/parity_*.rs` は 16 本。`required-features` の宣言は 30 件 | `ls crates/core/tests/parity_*.rs`、`grep -c required-features crates/core/Cargo.toml` |
| GEMM の CPU 参照 | `reference_gemm_f32` は f32・行優先。各要素を `0.0` から `k` の昇順に `f32::mul_add` で積み上げる | `crates/core/src/parity/cpu_reference.rs` の rustdoc、`crates/core/tests/parity_cpu_reference.rs` |
| 要素ごとの比べ | `compare_f32`（`T1`）と `compare_f32_with_special_values`。数値は本書に持たない | `crates/core/src/parity/tolerance.rs`・`special_values.rs` |
| 要素ごと演算の CPU 参照 | 共通の関数は無い。Relu の参照は `case_inputs.rs` の中にだけある（3/4）。既存のテストは出力を直書きしている | `grep -n -i relu crates/core/src/parity/case_inputs.rs crates/core/tests/parity_element_rules.rs` |
| Top-k | `reference_topk_f32` と `compare_topk` がある | `grep -n '^pub fn' crates/core/src/parity/topk.rs` |
| Top-k の名前付きの入力 | `topk_inputs.rs` に `SIGNED_ZERO_INPUTS`・`PAD_POSITION_INPUTS`・`NON_FINITE_INPUTS` がある | `grep -n '^pub const' crates/core/src/parity/topk_inputs.rs` |
| 入力の作り方の先例 | `gemm_inputs.rs` は入力をファイルに置かず、その場で決定的に作る（固定 seed の `SplitMix64`）。大きな行は `#[ignore]` | [gemm-test-inputs](gemm-test-inputs.md) |
| 別 crate からの利用 | `contract` の `test-support` は `fandhe-silicon-core/test-support` へ転送する | `crates/contract/Cargo.toml`、`crates/contract/tests/test_support_forwarding.rs` |
| 背後実装との関係 | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。背後実装の crate に依存しない | `crates/core/Cargo.toml`、`crates/core/src/lib.rs` |
| wasm の照合の入力の形 | `wasm/node/cases.json` は `reference_source` の欄を持つ（形が在るという事実だけ） | `wasm/node/cases.json` |
| 走る場所 | `make verify`（`--all-features`）と `make build-profiles` の `test-support` を含む組。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |
| 本書の対象の言及 | 公開側の TASK-977 の言及は本書、本書へのリンクの行、`case_inputs` の rustdoc とそのテスト、`parity.rs` の入口の rustdoc | `git grep -n TASK-977 -- . ':!docs/spec'` |

## 17 ケースの入力（2/4）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/parity/case_inputs.rs`。feature `test-support` の裏。外部依存 0・`unsafe` なし | `grep -n 'pub mod case_inputs' crates/core/src/parity.rs` |
| ケース | `CaseId::ALL` は 17 件（GEMM の 13 件 `g-01`〜`g-13`、既存の 4 入力 `e-1`〜`e-4`）。識別名は試験の枠の `TestId` に通る文字だけ | `crates/core/tests/parity_case_inputs.rs` の `all_has_seventeen_unique_ids`・`ids_are_test_ids` |
| 作り方 | `CaseId::input()` が式からその場で決定的に作る。確保の失敗と次元の積の溢れは `ReferenceError` で返す | `inputs_are_fixed_by_sha256`（入力の f32 のビット列の sha256 を固定） |
| 形 | 演算の種類・m/k/n・入力の長さ・Top-k の k | `ops_and_shapes_are_fixed` |
| 出力の要素数 | `CaseInput::output_len()`。GEMM は `m*n`、Relu は入力の長さ、Top-k は `min(k, 有限のスコアの件数)`。合計の確かめは 3/4 の節 | `output_lens_are_fixed` |
| GEMM の設計の区分 | `GemmCase::design()`（NaN・±Inf・零・零でない有限）。`reference_gemm_f32` の出力の区分と一致する | `gemm_design_matches_reference_class` |
| 特殊値の置き方 | G-12 と E-1 の A の差は置き換えの位置だけ。E-2・E-4 の特殊値の位置 | `g12_differs_from_e1_only_in_replaced_rows`・`special_values_are_placed` |
| 走る場所 | `make verify`（`--all-features`）。GPU の実機は要らない | `cargo test -p fandhe-silicon-core --features test-support --test parity_case_inputs` |

## 参照の出力と追加の入力（3/4）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | 入力と同じ `case_inputs.rs`。新しいファイルは足さない | `grep -n 'pub fn reference\|pub fn total_output_len\|pub fn additional_topk_inputs' crates/core/src/parity/case_inputs.rs` |
| 参照の出力 | `CaseId::reference()`（`CaseInput::reference()` の糖衣）が `CaseOutput` を返す。GEMM は `reference_gemm_f32`、Top-k は `reference_topk_f32` をそのまま呼ぶ。失敗は `CaseReferenceError` | `gemm_reference_is_reference_gemm_f32`・`topk_reference_is_reference_topk_f32`・`case_reference_error_wraps_and_displays` |
| Relu の参照 | 正の値はそのまま、NaN・-Inf・負・±0.0 は `+0.0`。`f32::max` は零の符号が決まらないので使わない | `relu_reference_rules` |
| 要素数 | 参照の出力の長さは `output_len()` と一致する | `reference_len_matches_output_len` |
| 総数 | `total_output_len()` が 17 ケースの `output_len()` の和を `checked_add` で返す | `total_output_len_is_fixed` |
| 追加の入力 | `additional_topk_inputs()` は `topk_inputs::SIGNED_ZERO_INPUTS`（-0.0 を +0.0 より小さいスロット番号に置いた入力）を指す。`CaseId::ALL` と総数には入れない | `additional_inputs_put_negative_zero_first` |
| 参照の出力の固定 | 参照の出力の sha256 は固定しない（GEMM が新しく作る NaN のビット列は CPU の種類で違う）。要素の区分と既存の参照との一致で確かめる | `gemm_design_matches_reference_class` |
| 走る場所 | `make verify`（`--all-features`）。GPU の実機は要らない | `cargo test -p fandhe-silicon-core --features test-support --test parity_case_inputs` |

次の段（4/4）に残すもの: 別の crate から入力と参照の出力を読み込めることの確かめと、完了の条件の確かめ。

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/4 現状と前提の確認 | #6227 |
| 2/4 実装 | #6228 |
| 3/4 実装 | #6229 |
| 4/4 テスト | #6230 |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形は受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 追加の入力は、新設する前に既存の名前付きの入力で足りるかを確かめる
- spec への進み具合の記録は spec 側の作業で、本リポからは書き換えない（private spec 側の記録に従う。ポインタのみ）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[parity-element-rules](parity-element-rules.md)・[parity-signed-zero-topk](parity-signed-zero-topk.md)・[gemm-test-inputs](gemm-test-inputs.md)・[test-frame](test-frame.md)
