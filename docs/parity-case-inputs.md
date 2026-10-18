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
| 一致の道具のファイル | `crates/core/src/parity` に 11 ファイルある | `ls crates/core/src/parity` |
| 統合テスト | `crates/core/tests/parity_*.rs` は 15 本。`required-features` の宣言は 29 件 | `ls crates/core/tests/parity_*.rs`、`grep -c required-features crates/core/Cargo.toml` |
| GEMM の CPU 参照 | `reference_gemm_f32` は f32・行優先。各要素を `0.0` から `k` の昇順に `f32::mul_add` で積み上げる | `crates/core/src/parity/cpu_reference.rs` の rustdoc、`crates/core/tests/parity_cpu_reference.rs` |
| 要素ごとの比べ | `compare_f32`（`T1`）と `compare_f32_with_special_values`。数値は本書に持たない | `crates/core/src/parity/tolerance.rs`・`special_values.rs` |
| 要素ごと演算の CPU 参照 | `core` には無い。既存のテストは出力を直書きしている | `grep -n -i relu crates/core/tests/parity_element_rules.rs crates/core/tests/parity_signed_zero_topk.rs` |
| Top-k | `reference_topk_f32` と `compare_topk` がある | `grep -n '^pub fn' crates/core/src/parity/topk.rs` |
| Top-k の名前付きの入力 | `topk_inputs.rs` に `SIGNED_ZERO_INPUTS`・`PAD_POSITION_INPUTS`・`NON_FINITE_INPUTS` がある | `grep -n '^pub const' crates/core/src/parity/topk_inputs.rs` |
| 入力の作り方の先例 | `gemm_inputs.rs` は入力をファイルに置かず、その場で決定的に作る（固定 seed の `SplitMix64`）。大きな行は `#[ignore]` | [gemm-test-inputs](gemm-test-inputs.md) |
| 別 crate からの利用 | `contract` の `test-support` は `fandhe-silicon-core/test-support` へ転送する | `crates/contract/Cargo.toml`、`crates/contract/tests/test_support_forwarding.rs` |
| 背後実装との関係 | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。背後実装の crate に依存しない | `crates/core/Cargo.toml`、`crates/core/src/lib.rs` |
| wasm の照合の入力の形 | `wasm/node/cases.json` は `reference_source` の欄を持つ（形が在るという事実だけ） | `wasm/node/cases.json` |
| 走る場所 | `make verify`（`--all-features`）と `make build-profiles` の `test-support` を含む組。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |
| 本書の対象の言及 | 公開側の TASK-977 の言及は本書と、本書へのリンクの行だけ | `git grep -n TASK-977 -- . ':!docs/spec'` |

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
