# 代わりの実行の試験の入力と CPU の参照（TASK-929）の所在と本リポの現状

TASK-929 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-929。対象の項目は `docs/spec/04-behavior/fallback-execution.md` の FB-43・FB-44。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-24 | `crates/core/src/parity/tolerance.rs`・`cpu_reference.rs` | [parity-tool](parity-tool.md) |
| TASK-25 | `crates/core/src/parity/special_values.rs` | [parity-non-finite](parity-non-finite.md) |
| TASK-928 | `crates/core/src/parity/fallback_compare.rs` | [parity-fallback-compare](parity-fallback-compare.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 一致の道具のファイル | `parity` は 13 個の `pub mod`（`binary16`・`case_inputs`・`cpu_reference`・`fallback_compare`・`fallback_inputs`・`gemm_gate`・`gemm_inputs`・`gemm_population`・`pixel`・`special_values`・`tolerance`・`topk`・`topk_inputs`）で束ねる。比べ方の追加は配下にファイルを足して `pub mod` で束ねる流儀 | `ls crates/core/src/parity`、`grep -n 'pub mod' crates/core/src/parity.rs` |
| binary16 の変換 | 公開の関数は `f32_to_binary16_bits`・`binary16_bits_to_f32`・`round_slice_to_binary16`・`widen_binary16_slice` の 4 つ。丸める入口は f32 だけで、f64 から変換する入口は無い | `grep -n 'pub fn' crates/core/src/parity/binary16.rs`、`crates/core/tests/parity_binary16.rs` |
| 決定的な乱数の生成器 | `gemm_inputs` の `SplitMix64`（`new`・`next_u64`・`skip`・`next_signed_unit_f32`）と `GEMM_INPUT_SEED` が `pub`。入力は固定の seed からその場で作り、ファイルには置かない | `crates/core/src/parity/gemm_inputs.rs`、[gemm-test-inputs](gemm-test-inputs.md) |
| 入力の作り方の流儀 | `case_inputs`・`topk_inputs` は式や定数からその場で入力を作る。入力のファイルは無い | `crates/core/src/parity/case_inputs.rs`、`crates/core/src/parity/topk_inputs.rs` |
| 和の参照と非正規化数 | `fallback_compare` の `reference_sum_f32`（`IntoIterator<Item = f32>` を受ける）・`SumReference`・`flush_subnormal_f32`・`F32SubnormalHandling` | `crates/core/src/parity/fallback_compare.rs` |
| 比べ方の入口 | `compare_bits_u32`・`compare_bits_f32`・`compare_relative_sum_f32`・`compare_order_distance_binary16` など。詳しくは [parity-fallback-compare](parity-fallback-compare.md) | `grep -n 'pub fn' crates/core/src/parity/fallback_compare.rs` |
| CPU の参照 | `cpu_reference` の `reference_gemm_f32` と `ReferenceError` | `crates/core/src/parity/cpu_reference.rs` |
| 統合テスト | `crates/core/tests` の `parity_*` は 17 本。`crates/core/Cargo.toml` の `required-features` は 31 件 | `ls crates/core/tests \| grep -c '^parity_'`、`grep -c required-features crates/core/Cargo.toml` |
| feature | `parity` は `test-support` の裏にある。`test-support = []` で、`contract` の `test-support` が `fandhe-silicon-core/test-support` へ転送する | `crates/core/src/lib.rs`、`crates/core/Cargo.toml`、`crates/contract/Cargo.toml` |
| `exec` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec` |
| 依存・unsafe | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。`deps-approved.toml` の承認は 0 件 | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`deps-approved.toml` |
| 走る場所 | `core` の `parity_*` は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-929 の言及は本書、`CLAUDE.md`、`README.md` にある | `git grep -n TASK-929 -- . ':!docs/spec'` |

## 所在（2/6）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | `crates/core/src/parity/fallback_inputs.rs`（`test-support` の裏。外部依存 0・`unsafe` なし。乱数は `gemm_inputs::SplitMix64` を使う） | `grep -n 'pub mod fallback_inputs' crates/core/src/parity.rs` |
| 公開名 | `FALLBACK_LENGTHS`・`FALLBACK_INPUT_SEED`・`FallbackInputKind`（`name`・`seed_for`）・`F16FmaInputs`・`generate_f16_fma_e1`・`generate_f16_fma_r`・`generate_f16_fma_e2`・`generate_f16_fma_s`・`E2_PAIRS`・`E2_LEN`・`S_VALUES`・`S_LEN` | `crates/core/src/parity/fallback_inputs.rs` |
| 試験 | `crates/core/tests/parity_fallback_inputs.rs`（14 本） | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_inputs` |

## 振る舞い（2/6）

具体の値は公開テストが固定する。

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 長さの表 | 13 通りの長さを `FALLBACK_LENGTHS` に持つ | `lengths_table_is_fixed` |
| seed | 基の seed は GEMM の入力と同じ値。種類と長さが違えば seed が違う | `seed_base_matches_gemm_seed`・`seed_differs_across_kinds_and_lengths`・`kind_names_are_fixed` |
| `E1` | a・b・c は整数で、a×b+c が binary16 で正確に表せる範囲に入る。全長さで確かめる | `e1_values_are_integers_in_range` |
| `R` | 全長さで Inf・NaN を含まず、長さが n に揃う | `r_excludes_non_finite` |
| `E2` | a が外側、(b, c) の 16 組が内側の全数の列挙。乱数を使わない | `e2_enumeration`・`e2_pairs_are_distinct_and_hand_checked`・`e2_digest` |
| `S` | 15 値の全組の列挙。添字から a・b・c が決まる。乱数を使わない | `s_enumeration`・`s_values_decode_as_expected` |
| 決定性・空 | 同じ入力で同じ列を返す。n = 0 は空の列を返し拒否は入口の受け持ち | `generation_is_deterministic`・`empty_length_returns_empty` |
| 既知値 | 小さな長さの `E1`・`R` と `S` の sha256 を、本リポのコードが出す値として固定する | `known_digests` |

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/6 | #6041 |
| 2/6 | #6042（`f16-fma` の入力の生成器と長さ n の表。本書の「所在（2/6）」「振る舞い（2/6）」） |
| 3/6 | #6043（`subgroup-sum`・`atomic-add` の入力と m の表を足す） |
| 4/6 | #6044（CPU の参照と、f64 から binary16 へ丸める入口を足す） |
| 5/6 | #6045（出力の形の規則の試験） |
| 6/6 | #6046 |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く
- 具体の形は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[parity-fallback-compare](parity-fallback-compare.md)・[parity-case-inputs](parity-case-inputs.md)・[gemm-test-inputs](gemm-test-inputs.md)・[test-frame](test-frame.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
