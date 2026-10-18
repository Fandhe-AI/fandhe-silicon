# 性能の値を採る前の門の規則（TASK-1026）の所在と現状の事実

性能の測定の道具と、結果の一致を確かめる道具の現状について、本リポのコードと公開テストから説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1026。ポインタのみ）。spec の本文・値・規則は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

前提タスクに当たる本リポの物は次のとおり。

| 前提 | 本リポの物 |
| ---- | ---------- |
| TASK-24 | `crates/core/src/parity/`（`cpu_reference.rs`・`tolerance.rs`・`topk.rs`・`topk_inputs.rs`）（[parity-tool](parity-tool.md)） |
| TASK-25 | `crates/core/src/parity/special_values.rs`（[parity-non-finite](parity-non-finite.md)） |
| TASK-26 | `crates/core/src/parity/pixel.rs`（[pixel-parity-tool](pixel-parity-tool.md)） |
| TASK-27 | 別 workspace `measure/` の `measure-perf`（`measure/crates/perf/`）（[measurement-tool](measurement-tool.md)） |

submodule（`docs/spec`）が指している版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 比べの結果の型 | 比べの結果の型が `is_match` を持つ（6 件） | `git grep -n 'fn is_match' -- crates/core/src/parity` |
| 判定への写し | `test_frame` の `Verdict::{Match, Mismatch}` へ、比べの結果からの `impl From<&_> for Verdict` が 5 件ある | `git grep -n 'impl From' -- crates/core/src/test_frame/outcome.rs` |
| feature | `parity`・`test_frame` は `core` の feature `test-support` の裏だけで公開される | `grep -n -A2 '\[features\]' crates/core/Cargo.toml` |
| 測定の道具の構成 | `measure-perf` は `sample`・`stats`・`across`・`record`・`evidence`・`json`・`jsonread`・`runs`・`ratio`・`gate` と、bin `across-runs` を持つ | `ls measure/crates/perf/src` |
| 測定の道具と `core` | `measure-perf` の lib は `core` に依存しない。門は一致・不一致を独自の列挙で受ける。`core`（feature `test-support`）は dev-dependency だけで、試験が使う | `git grep -n -e fandhe-silicon-core -e fandhe_silicon_core -- measure/crates/perf`（`src/` は 0 件。`Cargo.toml` の `[dev-dependencies]` と `tests/gate.rs` にある） |
| path 依存の検査 | `measure-verify.sh` は `--allow-path-root` で `crates/` 配下への path だけを免除して `deps-check.sh` を呼ぶ | `grep -n 'allow-path-root' scripts/measure-verify.sh`（1 件） |
| 記録の語彙 | `record` に `Label`・`NotRunReason`・`IndeterminateReason` がある。綴りは `test_frame` に揃え、テストがソースと突き合わせる | `measure/crates/perf/src/record.rs` |
| 集計に入れない run | `not-run`・`indeterminate` の run は run 間の集計に入らない | `measure/crates/perf/src/runs.rs` |
| 走る場所 | `measure/` の試験は `make measure-verify`、`core` の `parity_*`・`test_frame_*` は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/measure-verify.sh`、`scripts/verify.sh` |
| 本書の対象の言及 | `docs/spec` を除く公開側の TASK-1026 の言及は、本書と `measure/crates/perf/src/gate.rs` などにある | `git grep -n TASK-1026 -- . ':!docs/spec'` |

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/4 現状と前提の確認 | #6359 |
| 2/4 門の規則を測定の道具に足す | #6360 |
| 3/4 門を通らない入力・通る入力の試験 | #6361 |
| 4/4 完了の条件の確かめ | #6362 |

## 門の規則（2/4）

| 項目 | 事実 |
| ---- | ---- |
| 所在 | `measure/crates/perf/src/gate.rs`（`measure_perf::gate`）。純粋な関数 `apply` と、1 行の JSON を作る `render` |
| 入力 | run ごとに、腕・行・run の ID と照合の結果（結果の一致の道具または画素の一致の道具の一致・不一致。結果が無いときは `None`） |
| 出力 | run ごとの採否と、採らない理由の綴り（`result-parity-mismatch`・`pixel-parity-mismatch`・`result-missing`・`pair-not-taken`）、採った・採らなかった run と組の件数、理由ごとの件数 |
| 規則 | 明示の一致の run だけ採る。同じ腕 × 行の組に採らない run が 1 件でもあれば、その組の一致の run も採らない。入力が 0 件、または run の ID の重複はエラー |
| `core` との関係 | lib（`src/`）は `core` に依存しない。`core` の比べの結果から `GateVerdict` への写しは呼び手が行い、試験（`tests/gate.rs`）がその呼び手になる |
| 記録の形 | `record` の `RunRecord` の JSON は変えていない。呼び手が `GateOutcome::taken_run_ids` で `runs::aggregate` へ渡す run を絞る |
| 確かめ方 | `make measure-verify`（`gate.rs` の単体試験を含む） |

## 試験（3/4）

`measure/crates/perf/tests/gate.rs`。`core` の実物の比べの道具（`compare_f32`・`compare_topk`・`compare_f32_with_special_values`・`compare_pixels`・`compare_pixels_exact`）の結果を `GateVerdict` へ写して門に通す。比べがエラーのときは結果なし（`None`）に写す。GPU の実機・ファイル・ネットワークは使わない。

| 試験の関数 | 確かめる振る舞い |
| ---------- | ---------------- |
| `result_parity_exact_input_is_taken` | 参照と同じ列は一致で、採る |
| `result_parity_perturbed_input_is_not_taken_and_blocks_pair` | 1 要素に誤差を足した列は不一致で `result-parity-mismatch`。同じ組の一致の run は `pair-not-taken` |
| `topk_identical_taken_and_bit_flip_or_swap_not_taken` | Top-k は同一なら採り、スコアの 1 ビット違い・順序の入れ替えは `result-parity-mismatch` |
| `non_finite_in_finite_reference_position_is_not_taken` | 参照が有限の位置の NaN は不一致で採らない。同じ列は採る |
| `pixel_parity_same_image_taken_and_changed_pixel_not_taken` | 同じ画像は採り、1 画素のチャネル差が上限を超えると `pixel-parity-mismatch` |
| `pixel_parity_exact_rejects_difference_of_one` | 厳密な比べは差 1 でも不一致で採らない |
| `compare_error_maps_to_missing_and_is_not_taken` | 長さ違い・大きさ違いは `result-missing` で採らない |
| `mixed_inputs_leave_reasons_and_counts` | 混ぜた入力で、run ごとの採否・件数・理由ごとの件数・`taken_run_ids`・`render` の欄が期待どおり |
| `all_match_inputs_are_all_taken` | すべて一致の入力は全件採り、理由の件数は 0 |

確かめ方: `make measure-verify`。

## 完了の条件の確かめ（4/4）

4/4（#6362）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-1026）。
GPU の実機なしで、末尾の base で実行した値。件数は実測。

| 成果物 | 本書の節 | 試験 |
| ------ | -------- | ---- |
| 門の規則 | 門の規則（2/4） | `measure/crates/perf/src/gate.rs` の単体試験 |
| 門を通らない入力・通る入力の試験 | 試験（3/4） | `measure/crates/perf/tests/gate.rs` |

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 結合試験が通る | `cargo test --manifest-path measure/Cargo.toml --workspace --all-features --locked --test gate` | 9 passed; 0 failed; 0 ignored |
| A2 | 単体試験が通る | 同じ形に `--lib gate::` | 6 passed; 0 failed; 0 ignored |
| A3 | 不一致の入力では性能値を採らず、理由と件数が残る | A1 のうち `result_parity_perturbed_input_is_not_taken_and_blocks_pair`・`topk_identical_taken_and_bit_flip_or_swap_not_taken`・`non_finite_in_finite_reference_position_is_not_taken`・`pixel_parity_same_image_taken_and_changed_pixel_not_taken`・`pixel_parity_exact_rejects_difference_of_one`・`compare_error_maps_to_missing_and_is_not_taken`・`mixed_inputs_leave_reasons_and_counts` | すべて ok |
| A4 | 一致の入力では性能値を採る | A1 のうち `result_parity_exact_input_is_taken`・`all_match_inputs_are_all_taken`（と A3 の各試験の一致の側） | すべて ok |
| A5 | 採った run の ID と 1 行の JSON が出る | `mixed_inputs_leave_reasons_and_counts` が `taken_run_ids` と `render` の欄を確かめる | ok |
| A6 | lib が `core` に依存しない | 「本リポの現状」の表の「測定の道具と `core`」の行の grep | `src/` は 0 件。`Cargo.toml` の `[dev-dependencies]` と `tests/gate.rs` にだけある |
| A7 | path 依存の免除 | `grep -c 'allow-path-root' scripts/measure-verify.sh` | 1 件 |
| A8 | base の CI | base の push の CI run を `gh run view` で確認 | 完了・success（`ci-complete` も success） |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make measure-verify` | すべて終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1026）。

### 再実行の契機

- `measure/crates/perf/src/gate.rs` を変えたとき
- `measure/crates/perf/tests/gate.rs` を変えたとき
- `crates/core/src/parity/` の `tolerance.rs`・`topk.rs`・`special_values.rs`・`pixel.rs` を変えたとき
- `measure/crates/perf/Cargo.toml` の dev-dependency を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `9440832`、`docs/spec` の gitlink は `8664148`。

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。後の段で足す振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 関連

[parity-tool](parity-tool.md)・[pixel-parity-tool](pixel-parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)・[measurement-tool](measurement-tool.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
