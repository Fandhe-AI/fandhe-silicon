# 性能の判定の変動の幅の規則（TASK-1027）の所在と現状の事実

性能の測定の道具の現状について、本リポのコードと公開テストから説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1027。ポインタのみ）。spec の本文・値・規則は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

前提タスクに当たる本リポの物は次のとおり。

| 前提 | 本リポの物 |
| ---- | ---------- |
| TASK-27 | 別 workspace `measure/` の `measure-perf`（`measure/crates/perf/`）（[measurement-tool](measurement-tool.md)） |
| TASK-1026 | `measure/crates/perf/src/gate.rs`・`measure/crates/perf/tests/gate.rs`（[perf-gate-rule](perf-gate-rule.md)） |

submodule（`docs/spec`）が指している版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 測定の道具の構成 | `measure-perf` は `sample`・`stats`・`across`・`record`・`evidence`・`json`・`jsonread`・`runs`・`ratio`・`gate` と、bin `across-runs` を持つ | `ls measure/crates/perf/src` |
| run ごとの統計 | `stats::RunStats` が run ごとの統計を出す | `measure/crates/perf/src/stats.rs` |
| run 間の変動帯 | 2 か所にある。`across::AcrossRuns` は f64 の `spread_abs`・`spread_ratio`、`runs::Band` は 1/4 単位の整数の `median`・`min`・`max`・`spread_abs` と千分の一単位の `spread_ratio_milli` | `measure/crates/perf/src/across.rs`・`measure/crates/perf/src/runs.rs` |
| 判定に使う変動帯 | どちらのモジュールも、どちらで判定するかは決めていない（冒頭のコメントにそう書いてある。2 件） | `git grep -n 'どちら' -- measure/crates/perf/src` |
| run 間の p95 | p95 は run ごとの統計と記録の書き出しにあり、run 間の集計（`across`・`runs`）には無い | `git grep -n p95 -- measure/crates/perf/src`（`stats.rs`・`record.rs` だけ） |
| 比の区分 | `ratio` の閾値は 1 個の固定。`RATIO_SCOPE_TASKS` は空で、範囲外は判定保留に倒す。PoC ごとに切り替わる設定の口は無い | `measure/crates/perf/src/ratio.rs` |
| ラベルの語彙 | `record` の `Label` は Ok / Ng / Pending / NotRun / Indeterminate で、`IndeterminateReason` は 4 つ。綴りは `core` の `test_frame` に揃え、テストがソースと突き合わせる。変動の幅・測り直しに当たる理由は無い | `measure/crates/perf/src/record.rs` |
| PoC・測り直し・分解能の語 | `measure/` には無い（0 件） | `git grep -n -i -e poc -e remeasure -e resolution -e 分解能 -e 測り直 -- measure/` |
| 記録の本体 | 標本の数、CPU・GPU の標本、run ごとの統計を持つ。時刻の分解能の欄は無い | `measure/crates/perf/src/record.rs` |
| 集計に入れない run | `not-run`・`indeterminate` の run は run 間の集計に入らない | `measure/crates/perf/src/runs.rs` |
| 門との関係 | `gate::GateOutcome::taken_run_ids` が、`runs::aggregate` へ渡す run を絞る | `measure/crates/perf/src/gate.rs` |
| 走る場所 | `make measure-verify`。GPU の実機は要らない | `scripts/measure-verify.sh` |
| 本書の対象の言及 | `docs/spec` を除く公開側の TASK-1027 の言及は、本書だけ | `git grep -n TASK-1027 -- . ':!docs/spec'` |

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/6 現状と前提の確認 | #6363 |
| 2/6 形の設計 | #6364 |
| 3/6 実装 | #6365 |
| 4/6 実装 | #6366 |
| 5/6 テスト | #6367 |
| 6/6 確かめ | #6368 |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。後の段で足す振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 関連

[measurement-tool](measurement-tool.md)・[perf-gate-rule](perf-gate-rule.md)・[measurement-record](measurement-record.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
