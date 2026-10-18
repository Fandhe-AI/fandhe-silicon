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
| 測定の道具と `core` | `measure/` は `core` に依存しない。門は一致・不一致を独自の列挙で受ける | `git grep -n -e fandhe-silicon-core -e fandhe_silicon_core -- measure/`（0 件） |
| path 依存の検査 | `measure-verify.sh` は `--allow-path-root` を渡さずに `deps-check.sh` を呼ぶ | `grep -n 'allow-path-root' scripts/measure-verify.sh`（0 件） |
| 記録の語彙 | `record` に `Label`・`NotRunReason`・`IndeterminateReason` がある。綴りは `test_frame` に揃え、テストがソースと突き合わせる | `measure/crates/perf/src/record.rs` |
| 集計に入れない run | `not-run`・`indeterminate` の run は run 間の集計に入らない | `measure/crates/perf/src/runs.rs` |
| 走る場所 | `measure/` の試験は `make measure-verify`、`core` の `parity_*`・`test_frame_*` は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/measure-verify.sh`、`scripts/verify.sh` |
| 本書の対象の言及 | `docs/spec` を除く公開側の TASK-1026 の言及は、本書と `measure/crates/perf/src/gate.rs` などにある | `git grep -n TASK-1026 -- . ':!docs/spec'` |

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/4 現状と前提の確認 | #6359 |
| 2/4 門の規則を測定の道具に足す | #6360 |
| 3/4 | #6361 |
| 4/4 | #6362 |

## 門の規則（2/4）

| 項目 | 事実 |
| ---- | ---- |
| 所在 | `measure/crates/perf/src/gate.rs`（`measure_perf::gate`）。純粋な関数 `apply` と、1 行の JSON を作る `render` |
| 入力 | run ごとに、腕・行・run の ID と照合の結果（結果の一致の道具または画素の一致の道具の一致・不一致。結果が無いときは `None`） |
| 出力 | run ごとの採否と、採らない理由の綴り（`result-parity-mismatch`・`pixel-parity-mismatch`・`result-missing`・`pair-not-taken`）、採った・採らなかった run と組の件数、理由ごとの件数 |
| 規則 | 明示の一致の run だけ採る。同じ腕 × 行の組に採らない run が 1 件でもあれば、その組の一致の run も採らない。入力が 0 件、または run の ID の重複はエラー |
| `core` との関係 | `core` に依存しない（依存を足していない）。`core` の比べの結果から `GateVerdict` への写しは呼び手が行う |
| 記録の形 | `record` の `RunRecord` の JSON は変えていない。呼び手が `GateOutcome::taken_run_ids` で `runs::aggregate` へ渡す run を絞る |
| 確かめ方 | `make measure-verify`（`gate.rs` の単体試験を含む） |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。後の段で足す振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 関連

[parity-tool](parity-tool.md)・[pixel-parity-tool](pixel-parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)・[measurement-tool](measurement-tool.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
