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
| 一致の道具のファイル | `parity` は 10 個の `pub mod`（`binary16`・`cpu_reference`・`gemm_gate`・`gemm_inputs`・`gemm_population`・`pixel`・`special_values`・`tolerance`・`topk`・`topk_inputs`）で束ねる。比べ方の追加は配下にファイルを足して `pub mod` で束ねる流儀 | `ls crates/core/src/parity`、`grep -n 'pub mod' crates/core/src/parity.rs` |
| 有限でない値の比べ方 | `SpecialVerdict`・`judge_element_with_special_values`・`SpecialComparison`・`compare_f32_with_special_values`。入力は f32 で、両方が有限なら `tolerance::judge_element` へ委ねる | `crates/core/src/parity/special_values.rs`、`crates/core/tests/parity_special_values.rs` |
| 有限の比べ方 | `Tolerance`・`ElementVerdict`・`judge_element`・`Comparison`・`CompareError`・`compare_f32` | `crates/core/src/parity/tolerance.rs`、`crates/core/tests/parity_tolerance.rs` |
| binary16 の変換 | `f32_to_binary16_bits`・`binary16_bits_to_f32`・`round_slice_to_binary16`・`widen_binary16_slice`。ビット列は `u16` で持つ | `crates/core/src/parity/binary16.rs`、`crates/core/tests/parity_binary16.rs` |
| 統合テスト | `crates/core/tests` の `parity_*` は 14 本。`crates/core/Cargo.toml` の `required-features` は 28 件 | `ls crates/core/tests \| grep -c '^parity_'`、`grep -c required-features crates/core/Cargo.toml` |
| feature | `parity`・`test_frame`・`rerun` は `test-support` の裏にある。`test-support = []` で、`contract` の `test-support` が `fandhe-silicon-core/test-support` へ転送する | `crates/core/src/lib.rs`、`crates/core/Cargo.toml`、`crates/contract/Cargo.toml` |
| `exec` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格。`#![forbid(unsafe_code)]` | `git ls-files crates/exec` |
| 依存・unsafe | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。`deps-approved.toml` の承認は 0 件 | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`deps-approved.toml` |
| 走る場所 | `core` の `parity_*` は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | 公開側（`docs/spec` を除く）に TASK-928 の言及は本書以前に無い | `git grep -n TASK-928 -- . ':!docs/spec'` |

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
