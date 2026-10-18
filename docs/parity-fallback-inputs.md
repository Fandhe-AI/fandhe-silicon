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
| 一致の道具のファイル | `parity` は 14 個の `pub mod`（`binary16`・`case_inputs`・`cpu_reference`・`fallback_compare`・`fallback_inputs`・`fallback_reference`・`gemm_gate`・`gemm_inputs`・`gemm_population`・`pixel`・`special_values`・`tolerance`・`topk`・`topk_inputs`）で束ねる。比べ方の追加は配下にファイルを足して `pub mod` で束ねる流儀 | `ls crates/core/src/parity`、`grep -n 'pub mod' crates/core/src/parity.rs` |
| binary16 の変換 | 公開の関数は `f32_to_binary16_bits`・`binary16_bits_to_f32`・`round_slice_to_binary16`・`widen_binary16_slice`・`f64_to_binary16_bits`・`binary16_bits_to_f64` の 6 つ。f64 から 1 回で丸める入口と f64 へ広げる入口は 4/6 で足した | `grep -n 'pub fn' crates/core/src/parity/binary16.rs`、`crates/core/tests/parity_binary16.rs` |
| 決定的な乱数の生成器 | `gemm_inputs` の `SplitMix64`（`new`・`next_u64`・`skip`・`next_signed_unit_f32`）と `GEMM_INPUT_SEED` が `pub`。入力は固定の seed からその場で作り、ファイルには置かない | `crates/core/src/parity/gemm_inputs.rs`、[gemm-test-inputs](gemm-test-inputs.md) |
| 入力の作り方の流儀 | `case_inputs`・`topk_inputs` は式や定数からその場で入力を作る。入力のファイルは無い | `crates/core/src/parity/case_inputs.rs`、`crates/core/src/parity/topk_inputs.rs` |
| 和の参照と非正規化数 | `fallback_compare` の `reference_sum_f32`（`IntoIterator<Item = f32>` を受ける）・`SumReference`・`flush_subnormal_f32`・`F32SubnormalHandling` | `crates/core/src/parity/fallback_compare.rs` |
| 比べ方の入口 | `compare_bits_u32`・`compare_bits_f32`・`compare_relative_sum_f32`・`compare_order_distance_binary16` など。詳しくは [parity-fallback-compare](parity-fallback-compare.md) | `grep -n 'pub fn' crates/core/src/parity/fallback_compare.rs` |
| CPU の参照 | `cpu_reference` の `reference_gemm_f32` と `ReferenceError`。代わりの実行の参照は 4/6 で `fallback_reference` に足した | `crates/core/src/parity/cpu_reference.rs` |
| 統合テスト | `crates/core/tests` の `parity_*` は 19 本。`crates/core/Cargo.toml` の `required-features` は 33 件 | `ls crates/core/tests \| grep -c '^parity_'`、`grep -c required-features crates/core/Cargo.toml` |
| feature | `parity` は `test-support` の裏にある。`test-support = []` で、`contract` の `test-support` が `fandhe-silicon-core/test-support` へ転送する | `crates/core/src/lib.rs`、`crates/core/Cargo.toml`、`crates/contract/Cargo.toml` |
| `exec` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec` |
| 依存・unsafe | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。`deps-approved.toml` の承認は 0 件 | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`deps-approved.toml` |
| 走る場所 | `core` の `parity_*` は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-929 の言及は本書、`CLAUDE.md`、`README.md`、`crates/core/src/parity.rs`、`parity/binary16.rs`、`parity/fallback_inputs.rs`、`parity/fallback_reference.rs` と、試験 `parity_fallback_inputs.rs`・`parity_fallback_reference.rs`・`parity_fallback_shape.rs` にある | `git grep -n TASK-929 -- . ':!docs/spec'` |

## 所在（2/6）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | `crates/core/src/parity/fallback_inputs.rs`（`test-support` の裏。外部依存 0・`unsafe` なし。乱数は `gemm_inputs::SplitMix64` を使う） | `grep -n 'pub mod fallback_inputs' crates/core/src/parity.rs` |
| 公開名 | `FALLBACK_LENGTHS`・`FALLBACK_INPUT_SEED`・`FallbackInputKind`（`name`・`seed_for`）・`F16FmaInputs`・`generate_f16_fma_e1`・`generate_f16_fma_r`・`generate_f16_fma_e2`・`generate_f16_fma_s`・`E2_PAIRS`・`E2_LEN`・`S_VALUES`・`S_LEN` | `crates/core/src/parity/fallback_inputs.rs` |
| 試験 | `crates/core/tests/parity_fallback_inputs.rs`（3/6 の分を含めて 23 本） | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_inputs` |

## 振る舞い（2/6）

具体の値は公開テストが固定する。

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 長さの表 | 13 通りの長さを `FALLBACK_LENGTHS` に持つ | `lengths_table_is_fixed` |
| seed | 基の seed は GEMM の入力と同じ値。`FALLBACK_LENGTHS` の長さの範囲では、種類と長さが違えば seed が違う（任意の `usize` では保証しない） | `seed_base_matches_gemm_seed`・`seed_differs_across_kinds_and_lengths`・`kind_names_are_fixed` |
| `E1` | a・b・c は整数で、a×b+c が binary16 で正確に表せる範囲に入る。全長さで確かめる | `e1_values_are_integers_in_range` |
| `R` | 全長さで Inf・NaN を含まず、長さが n に揃う | `r_excludes_non_finite` |
| `E2` | a が外側、(b, c) の 16 組が内側の全数の列挙。乱数を使わない | `e2_enumeration`・`e2_pairs_are_distinct_and_hand_checked`・`e2_digest` |
| `S` | 15 値の全組の列挙。添字から a・b・c が決まる。乱数を使わない | `s_enumeration`・`s_values_decode_as_expected` |
| 決定性・空 | 同じ入力で同じ列を返す。n = 0 は空の列を返し拒否は入口の受け持ち | `generation_is_deterministic`・`empty_length_returns_empty` |
| 既知値 | 小さな長さの `E1`・`R` と `S` の sha256 を、本リポのコードが出す値として固定する | `known_digests` |

## 所在（3/6）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/6 と同じ `crates/core/src/parity/fallback_inputs.rs` と `crates/core/tests/parity_fallback_inputs.rs` に足した（`pub mod` の数・`parity_*` の本数・`required-features` の件数は不変） | `git grep -n 'ax_pair_allowed' -- crates` |
| 公開名 | `SUBGROUP_SUM_BLOCK`・`ATOMIC_ADD_OUTPUT_COUNTS`・`ax_pair_allowed`・`FallbackInputKind` の 7 変種と `seed_for_outputs`・`generate_subgroup_sum_u32_u`・`generate_subgroup_sum_f32_fx`・`generate_subgroup_sum_f32_fg`・`generate_subgroup_sum_f32_fs`・`generate_atomic_add_ax`・`generate_atomic_add_ag`・`generate_atomic_add_as` | `crates/core/src/parity/fallback_inputs.rs` |

## 振る舞い（3/6）

具体の値は公開テストが固定する。

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 表 | ブロックの大きさは 256、出力の数 m の表は {1, 64, 4096} | `outputs_table_and_block_are_fixed` |
| `AX` の組 | 256 × ⌈n ÷ m⌉ が 2^24 未満の組だけ真。m = 0 と桁あふれは偽。長さの表との組では m = 1 が 9、m = 64 が 12、m = 4096 が 13 長さ | `ax_pairs_are_computed` |
| `U` | u32 の乱数。長さが n に揃う | `u_is_uniform_over_high_bits` |
| `FX`・`AX` | −256〜256 の整数値の f32。使える組では出力の要素ごとの和の上限が 2^24 未満 | `fx_ax_are_integers_in_range` |
| `FG`・`AG` | 有限で、大きさは 2^−20 以上 2^20 以下。符号は両方出る。2 の冪の 40 区間すべてに値が出る。libm を使わず基本演算だけで作る | `fg_ag_magnitudes_in_range` |
| `FS` | 256 個のブロックごとに、ブロック番号 % 8 の型（NaN 1 つ・+Inf 1 つ・−Inf 1 つ・+Inf と −Inf・すべて −0.0・±0 の混在・すべて非正規化数・非正規化数と正規化数の交互）を置く。長さ 1 のクラスの規則も固定 | `fs_patterns_by_block` |
| `AS` | `i % m` が同じ要素の組ごとに、出力の要素番号 % 8 の型を置く。m = 0 は空の列 | `as_patterns_by_output_element`・`empty_length_returns_empty` |
| 有限の部分 | `FS`・`AS` の有限の値は大きさ 2^20 以下 | `fs_as_finite_parts_bound` |
| seed・決定性 | `AS` の seed は n と m で決まり、長さの表と m の表の範囲で一意。同じ入力で同じ列 | `seed_differs_across_kinds_and_lengths`・`generation_is_deterministic` |
| 既知値 | 小さな長さの 7 種の sha256 を、本リポのコードが出す値として固定する | `known_digests_subgroup_atomic` |

## 所在（4/6）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | `crates/core/src/parity/fallback_reference.rs`（`test-support` の裏。外部依存 0・`unsafe` なし。和の計算は `fallback_compare::reference_sum_f32` を呼び、新しく書かない）。f64 の変換は `binary16.rs` に足した | `grep -n 'pub mod fallback_reference' crates/core/src/parity.rs` |
| 公開名 | `reference_f16_fma`・`reference_f16_fma_slice`・`subgroup_sum_output_len`・`subgroup_sum_block_range`・`reference_subgroup_sum_u32`・`reference_subgroup_sum_f32`・`reference_atomic_add_f32`・`F32SumReferences`（`values`・`abs_sums`・`len`・`is_empty`）。`binary16` に `f64_to_binary16_bits`・`binary16_bits_to_f64` | `crates/core/src/parity/fallback_reference.rs` |
| 試験 | `crates/core/tests/parity_fallback_reference.rs`（15 本） | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_reference` |

## 振る舞い（4/6）

具体の値は公開テストが固定する。

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| f64 から binary16 | f32 を経由せず最近接偶数で 1 回だけ丸める。全 65,536 のビット列が往復で戻る（NaN は NaN のまま）。f32 で表せる値では `f32_to_binary16_bits` と一致する | `f64_conversion_round_trips_every_binary16`・`f64_conversion_matches_f32_conversion_on_f32_values`・`f64_conversion_rounds_once` |
| 境界の値 | 零の符号を保つ。f64 の非正規化数は零へ。2^-25 ちょうどは 0、超えれば最小の非正規化数。65520 以上は無限大 | `f64_conversion_special_values` |
| `f16-fma` | 3 つを f64 に広げ `f64::mul_add` で融合して計算し、binary16 へ 1 回だけ丸める。結果が NaN のときは `0x7e00` に揃える | `f16_fma_hand_checked` |
| `E1` | 全長さで、整数の正確な計算を binary16 に丸めた値とビットが一致する | `f16_fma_e1_matches_exact_integer_arithmetic` |
| 列の関数 | `b`・`c` の長さが `a` と違えば `LengthMismatch`。空は空の列。要素ごとの関数と一致する | `f16_fma_slice_checks_lengths_and_empty`・`f16_fma_slice_matches_element_function` |
| ブロックの範囲 | 出力の数は ⌈n ÷ 256⌉。ブロックは重ならず 0..n を覆う。範囲外の j は `None` | `block_ranges_cover_input` |
| `subgroup-sum` の u32 | ブロックごとの和を 2^32 を法として求める | `subgroup_sum_u32_wraps_and_matches_blocks` |
| `subgroup-sum` の f32 | ブロックごとの値と Σ\|x\| が `reference_sum_f32` をブロックに直接かけた結果と一致する（`Preserve`・`FlushToZero`） | `subgroup_sum_f32_matches_reference_sum_per_block` |
| `atomic-add` | 出力は長さ m。要素 j は `skip(j).step_by(m)` の和と一致し、足される入力が無い要素は +0.0・Σ\|x\| 0。m = 0 は空の結果 | `atomic_add_matches_strided_sums`・`atomic_add_small_hand_example_and_zero_outputs` |
| 決定性 | 同じ入力で同じ出力 | `references_are_deterministic` |
| 既知値 | 参照の出力の sha256 を、本リポのコードが出す値として固定する（f32 の出力は NaN を 1 つの値に揃えて数える） | `known_digests` |

## 所在（5/6）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | `crates/core/tests/parity_fallback_shape.rs`（試験だけ。製品のコードは変えない。参照の実装とは別に、規則をそのまま書いた散布の計算を試験の中に持つ） | `grep -n 'parity_fallback_shape' crates/core/Cargo.toml` |
| 試験 | 8 本。`required-features = ["test-support"]` | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_shape` |

## 振る舞い（5/6）

具体の値は公開テストが固定する。`Preserve`・`FlushToZero` の両方で形が同じことを確かめる。

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| `subgroup-sum` の出力の長さ | n = 0 は 0、1〜256 は 1、257〜512 は 2、513 は 3。33・65 でも 1 で、ブロックの大きさ 256 は 32・64 に依らない。u32・f32 の参照の出力の長さも同じ | `subgroup_sum_output_len_hand_checked` |
| `subgroup-sum` の足し先 | 入力の添字 p の 1 つだけが 1 なら、出力は p ÷ 256 の要素だけが 1 で他は 0 | `subgroup_sum_one_hot_marks_block` |
| `subgroup-sum` の手計算 | `x[i] = i + 1` の n = 1・255・256・257・513 の出力を、リテラルの値で固定する | `subgroup_sum_arithmetic_series_hand_checked` |
| `atomic-add` の手計算 | `x = [1, 2, 3, 4, 5]` の m = 1・2・3・5・7 の出力を、リテラルの値で固定する | `atomic_add_small_hand_examples` |
| `atomic-add` の長さと初期値 | 出力の長さは m。足される入力が無い要素は +0.0 のビット列。すべて -0.0 の入力でも出力は +0.0。n が m より小さい・等しい・大きい場合を確かめる | `atomic_add_length_is_m_and_starts_from_positive_zero` |
| `atomic-add` の足し先 | 入力の添字 p の 1 つだけが 1 なら、出力は p mod m の要素だけが 1 で他は 0 | `atomic_add_one_hot_marks_slot` |
| 生成器の入力との突き合わせ（`subgroup-sum`） | 全 13 通りの長さで出力の長さが ⌈n ÷ 256⌉。値は n が 65,539 以下の入力で、`s[i / 256] += x[i]` の散布の計算とビットが一致する（長い入力は長さだけ確かめる） | `subgroup_sum_matches_scatter_for_generated_inputs` |
| 生成器の入力との突き合わせ（`atomic-add`） | 全 13 通りの長さ・m の 3 通りで出力の長さが m。値は n が 65,539 以下の入力で、`o[i mod m] += x[i]` の散布の計算とビットが一致する（長い入力は長さだけ確かめる） | `atomic_add_matches_scatter_for_generated_inputs` |

## 完了の条件の確かめ（6/6）

6/6（#6046）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-929）。
結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。

| 成果物 | 本書の節 | 試験 |
| ------ | -------- | ---- |
| 入力の生成器 | 所在（2/6）・振る舞い（2/6）・所在（3/6）・振る舞い（3/6） | `parity_fallback_inputs` |
| CPU の参照 | 所在（4/6）・振る舞い（4/6） | `parity_fallback_reference` |
| 出力の形の規則の試験 | 所在（5/6）・振る舞い（5/6） | `parity_fallback_shape` |

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 入力の生成器の試験 | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_inputs` | 23 passed; 0 failed; 0 ignored。所在（2/6）の件数と一致 |
| A2 | 長さ・m の表と `AX` の組 | A1 の出力 | `lengths_table_is_fixed`・`outputs_table_and_block_are_fixed`・`ax_pairs_are_computed` が ok |
| A3 | CPU の参照の試験 | 同 `--test parity_fallback_reference` | 15 passed; 0 failed; 0 ignored |
| A4 | 1 回だけの丸めと境界の値 | A3 の出力 | `f64_conversion_round_trips_every_binary16`・`f64_conversion_rounds_once`・`f64_conversion_special_values`・`f16_fma_hand_checked` が ok |
| A5 | 出力の形の規則（手計算の小さな入力） | 同 `--test parity_fallback_shape` | 8 passed; 0 failed; 0 ignored。`subgroup_sum_arithmetic_series_hand_checked`・`atomic_add_small_hand_examples` を含む |
| A6 | binary16 の既存の変換の回帰 | 同 `--test parity_binary16` | 7 passed; 0 failed |
| A7 | feature なしで外れること | `cargo test -p fandhe-silicon-core`（feature なし）・`make build-profiles` | どちらも終了コード 0 |
| C1 | 本体の範囲 | `fallback_inputs.rs`・`fallback_reference.rs` を `unsafe`・`std::fs`・`println!`・`env::var`・`process::Command` で grep。`crates/contract`・`crates/exec`・`crates/upper` の `src` を `fallback_inputs`・`fallback_reference` で grep | 前者は 1 件（`fallback_reference.rs` 冒頭のコメントで「`unsafe` を持たない」と述べる行。コードではない）、後者は 0 件 |
| C2 | 依存 | `deps-approved.toml` の承認の件数、`crates/core/Cargo.toml` の依存 | 承認 0 件、`core` の外部依存 0 |
| D | 品質ゲート | `make verify` | 終了コード 0。出力に 3 本の試験ファイルの実行が出る |

完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

### 再実行の契機

- `crates/core/src/parity/fallback_inputs.rs`・`fallback_reference.rs`・`binary16.rs`・`fallback_compare.rs`・`gemm_inputs.rs` を変えたとき
- `crates/core/tests/parity_fallback_inputs.rs`・`parity_fallback_reference.rs`・`parity_fallback_shape.rs` を変えたとき
- CI の `verify`・`deny` ジョブの手順を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `1c97bf3`、`docs/spec` の gitlink は `8664148`。

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/6 | #6041 |
| 2/6 | #6042（`f16-fma` の入力の生成器と長さ n の表。本書の「所在（2/6）」「振る舞い（2/6）」） |
| 3/6 | #6043（`subgroup-sum`・`atomic-add` の入力の生成器と出力の数 m の表。本書の「所在（3/6）」「振る舞い（3/6）」） |
| 4/6 | #6044（CPU の参照と f64 から binary16 への変換。本書の「所在（4/6）」「振る舞い（4/6）」） |
| 5/6 | #6045（出力の形の規則の試験。本書の「所在（5/6）」「振る舞い（5/6）」） |
| 6/6 | #6046（完了の条件の確かめ。本書の「完了の条件の確かめ（6/6）」） |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く
- 具体の形は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[parity-fallback-compare](parity-fallback-compare.md)・[parity-case-inputs](parity-case-inputs.md)・[gemm-test-inputs](gemm-test-inputs.md)・[test-frame](test-frame.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
