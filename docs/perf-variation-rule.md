# 性能の判定の変動の幅の規則（TASK-1027）の所在と現状の事実

性能の測定の道具の現状について、本リポのコードと公開テストから説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1027。ポインタのみ）。spec の本文・値・規則は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
2/6（#6364）で「形の設計」の節を足した（決定の記録のポインタ・規則が乗る既存のコードの事実・守る本リポの規約）。

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
| 本書の対象の言及 | `docs/spec` を除く公開側の TASK-1027 の言及は、本書と、本書を指す CLAUDE.md・README.md の索引行だけ | `git grep -n TASK-1027 -- . ':!docs/spec'` |

## 形の設計（2/6）

本段の範囲は `docs/spec/05-tasks.md` の TASK-1027 の内容の前半。
具体（置き場の名前・型・欄・設定の値・規則の形・出力の先）は 3/6（#6365）以降のコードと公開テストが示し、本節には先に書かない。
本節に書くのは、決定の記録のポインタ、規則が乗る既存のコードの事実、どの形でも守る本リポの規約。

### 決定の記録に従う点

- 規則と設定は、`docs/spec/05-tasks.md` の TASK-1027 と `docs/spec/04-behavior/performance.md` から辿る決定の記録に従う（ポインタのみ）。本書には内容を書かない
- 関連の文書: [measurement-tool](measurement-tool.md)（TASK-27）・[perf-gate-rule](perf-gate-rule.md)（TASK-1026）・[measurement-record](measurement-record.md)（TASK-28）
- 決定の記録から一意に決まらない点は [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路で扱う

### 規則が乗る既存のコード・検査の事実

上の「本リポの現状」の表と重なる事実は、その表を参照する。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| run 間の集計のコマンド | `across-runs` は標準出力に 1 行の JSON を出し、ファイルを作らない | `measure/crates/perf/src/lib.rs` の `//!` |
| run ごとの記録の形の検査 | `scripts/perf-record-check.sh`。make ターゲットは無く、回帰テストは `make test-perf-record-check` | ファイル冒頭のコメント |
| 記録の例 | `docs/run-record.example.json` は書き出しの実物とバイトで一致する | `measure/crates/perf/tests/repeat.rs` |
| 語彙の突き合わせ | `record` の `Label`・`IndeterminateReason` の綴りは、試験が `crates/core` の `test_frame` のソースと突き合わせる | `measure/crates/perf/tests/record.rs` |
| 記録のパスを引数で受ける照合の前例 | `scripts/rerun-compare.sh`。make ターゲットを持たない | ファイル冒頭のコメント |
| 検査の入口 | `make measure-verify`。CI の `deny` ジョブが呼ぶ | `scripts/measure-verify.sh` |

### どの形でも守る本リポの規約

| 項目 | 規約 | 所在 |
| ---- | ---- | ---- |
| 製品の workspace | 製品の crate は測定の道具に依存しない。向きは「測定 → 製品の公開 API」の一方向 | [poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |
| 依存 | 標準ライブラリだけで書く。依存が要る場合は承認の経路を通し、`=x.y.z` で固定する | [dependency-policy](../.claude/rules/dependency-policy.md) |
| unsafe | `measure/` は `unsafe_code = "forbid"` | `measure/Cargo.toml` |
| 失敗の扱い | ライブラリは `Result` を返し panic しない。不正な入力・未知の入力は `Err` にするか、判定を付けない側に倒す | [coding-rust](../.claude/rules/coding-rust.md) |
| 値の置き場 | 記録済みの run の値は公開側に置かない。公開側の試験と CI は、形が同じ合成値の fixture で行う | 親 #935 |
| 記録済みの行との照合 | 記録のパスを引数で受ける形にし、公開側の試験・CI には組み込まない | `scripts/rerun-compare.sh`、親 #935 |
| spec との対応 | ID のポインタだけ | [spec-confidentiality](../.claude/rules/spec-confidentiality.md) |
| 検査 | `make measure-verify`。記録の形を変える場合は `make test-perf-record-check` も通す | `Makefile` |

### 3/6 以降で扱うこと

3/6（#6365）・4/6（#6366）の実装、5/6（#6367）の試験、6/6（#6368）の確かめ。
各段の PR のコードと公開テストが具体を示し、その PR で本書に節を足す。

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

- 本書には現状の事実と、決定の記録のポインタ・守る規約だけを置く。後の段で足す振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 関連

[measurement-tool](measurement-tool.md)・[perf-gate-rule](perf-gate-rule.md)・[measurement-record](measurement-record.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
