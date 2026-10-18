# 性能の判定の変動の幅の規則（TASK-1027）の所在と現状の事実

性能の測定の道具の現状について、本リポのコードと公開テストから説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1027。ポインタのみ）。spec の本文・値・規則は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
2/6（#6364）で「形の設計」の節を足した（決定の記録のポインタ・規則が乗る既存のコードの事実・守る本リポの規約）。
3/6（#6365）で「変動の幅の規則（3/6）」の節を足した（規則のモジュール `variation`。値は表に写さずコードを指す）。
4/6（#6366）で「測り直しと判定不能の扱い（4/6）」の節を足した（モジュール `remeasure`。値は表に写さずコードを指す）。
5/6（#6367）で「再現の試験（5/6）」の節を足した（モジュール `replay` とコマンド `variation-replay`。記録済みの行は本リポに置かない）。
6/6（#6368）で「完了の条件の確かめ（6/6）」の節を足した（公開の試験の実測と品質ゲート）。

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
| 測定の道具の構成 | `measure-perf` は `sample`・`stats`・`across`・`record`・`evidence`・`json`・`jsonread`・`runs`・`ratio`・`gate`・`variation`・`remeasure`・`replay` と、bin `across-runs`・`variation-replay` を持つ | `ls measure/crates/perf/src` |
| run ごとの統計 | `stats::RunStats` が run ごとの統計を出す | `measure/crates/perf/src/stats.rs` |
| run 間の変動帯 | 2 か所にある。`across::AcrossRuns` は f64 の `spread_abs`・`spread_ratio`、`runs::Band` は 1/4 単位の整数の `median`・`min`・`max`・`spread_abs` と千分の一単位の `spread_ratio_milli` | `measure/crates/perf/src/across.rs`・`measure/crates/perf/src/runs.rs` |
| 判定に使う変動帯 | どちらのモジュールも、どちらで判定するかは決めていない（冒頭のコメントにそう書いてある。2 件） | `git grep -n 'どちら' -- measure/crates/perf/src` |
| run 間の p95 | p95 は run ごとの統計と記録の書き出しにあり、run 間の集計（`across`・`runs`）には無い | `git grep -n p95 -- measure/crates/perf/src`（`stats.rs`・`record.rs` だけ） |
| 比の区分 | `ratio` の閾値は 1 個の固定。`RATIO_SCOPE_TASKS` は空で、範囲外は判定保留に倒す。PoC ごとに切り替わる設定の口は無い。変動の幅の設定は `variation` の閉じた定数表が持ち、外から差し替える口は無い | `measure/crates/perf/src/ratio.rs` |
| ラベルの語彙 | `record` の `Label` は Ok / Ng / Pending / NotRun / Indeterminate で、`IndeterminateReason` は 4 つ。綴りは `core` の `test_frame` に揃え、テストがソースと突き合わせる。変動の幅・測り直しに当たる理由は無い（4/6 でも変えていない）。`remeasure` は行・比較の単位の判定のラベルを別に持つ | `measure/crates/perf/src/record.rs` |
| PoC・測り直し・分解能の語 | `measure/` で当たるのは `variation.rs`・`remeasure.rs`・`replay.rs`・`tests/replay.rs` と `lib.rs` の `//!` だけ。測り直しの語は `remeasure.rs` と、その回数を持つ `variation.rs` の設定の表に現れる。時刻の分解能の語は `replay.rs`・`tests/replay.rs` だけで、入力の整数を出力へ通す欄の名前として現れる | `git grep -n -i -l -e poc -e remeasure -e resolution -e 分解能 -e 測り直し -- measure/` |
| 記録の本体 | 標本の数、CPU・GPU の標本、run ごとの統計を持つ。時刻の分解能の欄は無い（3/6〜5/6 は記録の形を変えていない。5/6 の再現の出力は分解能を入力から通すだけで、記録の本体には足していない） | `measure/crates/perf/src/record.rs` |
| 集計に入れない run | `not-run`・`indeterminate` の run は run 間の集計に入らない | `measure/crates/perf/src/runs.rs` |
| 門との関係 | `gate::GateOutcome::taken_run_ids` が、`runs::aggregate` へ渡す run を絞る | `measure/crates/perf/src/gate.rs` |
| 走る場所 | `make measure-verify`。GPU の実機は要らない | `scripts/measure-verify.sh` |
| 本書の対象の言及 | `docs/spec` を除く公開側の TASK-1027 の言及は、本書と、本書を指す CLAUDE.md・README.md の索引行と、`measure/crates/perf/src/variation.rs`・`measure/crates/perf/src/remeasure.rs`・`measure/crates/perf/src/replay.rs`・`measure/crates/perf/tests/replay.rs` の `//!` だけ | `git grep -n TASK-1027 -- . ':!docs/spec'` |

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

3/6（#6365）で入った物は次節「変動の幅の規則（3/6）」、4/6（#6366）で入った物は「測り直しと判定不能の扱い（4/6）」を参照。
5/6（#6367）で入った物は「再現の試験（5/6）」を参照。6/6（#6368）で入った物は「完了の条件の確かめ（6/6）」を参照。

## 変動の幅の規則（3/6）

規則のモジュール `variation` を足した。記録の JSON と、`runs`・`across`・`ratio`・`gate` の振る舞いは変えていない。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `measure/crates/perf/src/variation.rs`（`measure-perf` の `variation`） | `ls measure/crates/perf/src` |
| 入力 | 腕ごとの run の値の列（1/4 ns 単位の整数）。1 run の値の統計は呼び手が選ぶ（設定が `RunStatistic` で示す） | `variation::VariationSetting` |
| 出力 | 腕ごとの「幅の規則に当たったか」と、当たった腕の名前（昇順）。PoC-11 は「差が幅の中か」を別の関数で返す | `variation::check_row`・`variation::check_difference` |
| 判定の形 | `Criterion` の 4 つ。幅が係数 × run 間中央値を超える（等しいときは超えない）、その下限つき、最大 ÷ 最小が閾値以上（等しいときは当たる）、差が幅以下（等しいときは当たる） | `variation::Criterion` |
| 数え方 | 中央値は 2 倍して持ち、丸めない。比べるのは `u128` の checked 演算で、`f64` の割り算は使わない | `variation.rs` の単体試験 |
| 設定の置き場 | `variation::VariationRule::setting` の定数表の 1 か所。値は本書に写さずコードを指す。外から差し替える公開の口は無い | `measure/crates/perf/src/variation.rs` |
| 失敗の扱い | run の数の不一致・空・腕の名前の重複・最小が 0・判定の形に合わない呼び出し・溢れは `Err`。GPU の値が無い run には呼び手が規則を当てない | `variation::VariationError` |
| 記録の形 | 変えていない。`record`・`runs`・`docs/run-record.example.json`・`scripts/` に差分は無い | `git diff origin/main -- measure/crates/perf/src/record.rs measure/crates/perf/src/runs.rs docs/run-record.example.json scripts/` |
| 試験 | 境目（ちょうど・下・上）、偶数個の中央値、下限が効く場合と係数が効く場合、`Err` の各場合、溢れ。値・腕の名前は合成 | `make measure-verify` |

## 測り直しと判定不能の扱い（4/6）

モジュール `remeasure` を足し、`variation` の設定の表に測り直しの回数と判定不能の範囲を足した。記録の JSON と、`runs`・`across`・`ratio`・`gate` の振る舞いは変えていない。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `measure/crates/perf/src/remeasure.rs`（`measure-perf` の `remeasure`） | `ls measure/crates/perf/src` |
| 入力 | 測った回ごとの腕の run の値の列（1/4 ns 単位の整数）と、行の中の比較の一覧（識別子・比べる腕・相手〔腕か定数〕・比の上限）。比の上限は呼び手が渡す | `remeasure::judge_row` |
| 出力 | 測り直しが要る（当たった腕、または比較の名前の昇順）か、判定が付いた（判定に使った回・腕ごとの run 間中央値・比較ごとのラベル）か | `remeasure::RowJudgement` |
| 測り直し | 規則に当たった行だけを測り直す。回数の上限は設定の表が持ち、それを超える回は渡せない。当たりが無い行への測り直しは `Err` | `variation::VariationSetting::remeasures`・`remeasure::RemeasureError` |
| 判定不能の範囲 | 測り直した後も当たりが残ったときの範囲は設定の表が PoC ごとに持つ。行の全部・当たった腕が関わる比較・差が幅の中の比較の 3 種 | `variation::IndeterminateScope`・`variation::VariationRule::setting` |
| 判定の値 | run 間の中央値。2 倍で持ち丸めない。測り直したときは測り直した回の値だけで判定し、前の回の値は混ぜない | `variation::ArmSpread::median_x2`・`remeasure.rs` の単体試験 |
| 境目 | 比の上限以内（等しいときを含む）なら OK、超えれば NG。`u128` の checked 演算で行い、`f64` は使わない | `remeasure.rs` の単体試験 |
| ラベル | `ok`・`ng`・`indeterminate`（理由は `variation-exceeded`）。行・比較の単位の判定で、`record::Label` は変えていない | `remeasure::JudgedLabel` |
| 設定の置き場 | 回数と範囲は `variation::VariationRule::setting` の 1 か所。値は本書に写さずコードを指す | `measure/crates/perf/src/variation.rs` |
| 失敗の扱い | 回の数・不要な測り直し・腕の集合の不一致・未知の腕・自分との比較・比較の重複や 0 件・上限の分母 0・run の数の不一致・溢れは `Err`。判定できないものは判定不能にし、暗黙に OK に倒さない | `remeasure::RemeasureError` |
| 記録の形 | 変えていない。`record`・`runs`・`docs/run-record.example.json`・`scripts/` に差分は無い | `git diff origin/main -- measure/crates/perf/src/record.rs measure/crates/perf/src/runs.rs docs/run-record.example.json scripts/` |
| 試験 | 判定の境目、定数の相手、測り直しの要否、測り直した回の値だけでの判定、範囲ごとの判定不能、`Err` の各場合、渡す順への非依存、溢れ。値・腕の名前は合成 | `make measure-verify` |

## 再現の試験（5/6）

記録済みの run 間の値を入力にして、行ごとのラベルを再現する道具を足した。記録済みの行のファイルは本リポに置かない。公開側の試験と CI は、形が同じ合成値の入力で行う。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `measure/crates/perf/src/replay.rs`（`replay`）とコマンド `measure/crates/perf/src/bin/variation-replay.rs`。make ターゲットは無く、`scripts/rerun-compare.sh` と同じく手元でパスを引数に渡して流す。CI には入れない | `ls measure/crates/perf/src measure/crates/perf/src/bin` |
| 入力 | JSON 1 ファイル。規則の名前、時刻の分解能（CPU と、あれば GPU。整数）、行の配列。行は識別子、測った回（1 回目、あれば測り直しの 2 回目）ごとの腕と run の値（1/4 ns 単位の整数）、比較の一覧、期待のラベル（比較ごと、または測り直しが要ること）を持つ。run の値は呼び手が `VariationSetting::statistic` に従って作る。PoC の生の標本からの変換は本リポの外の手元作業で、道具はしない | `replay::parse_input` |
| 読み方 | `jsonread` の厳格な読み込み。未知のキー・小数・使えない名前・重複・件数の上限超えは違反。件数の上限は `replay` の定数 | `replay::MAX_ROWS` ほか |
| 判定 | 行ごとに `remeasure::judge_row` を呼ぶ。規則と判定は足さず、判定側の `Err` は入力の違反にする | `replay::replay` |
| 期待との突き合わせ | 比較ごとにラベルを比べ、行ごと・全体の一致を出す。期待の比較の識別子の集合が行の比較と合わないときは違反。判定が付くはずの行に測り直しの期待、または逆は不一致 | `replay::replay` |
| 出力 | 標準出力に 1 ファイル 1 行の JSON。規則の名前、分解能の通し、行（識別子の昇順）ごとの結果と比較（識別子の昇順）ごとのラベル・理由・期待・一致、一致した行の数。ファイルは作らない | `measure/crates/perf/tests/replay.rs` |
| 時刻の分解能 | 入力の整数をそのまま出力へ通す。判定には使わず、設定の表にも書かない | `measure/crates/perf/tests/replay.rs` の `resolution_passes_through` |
| 終了コード | 0 すべて一致 / 1 不一致（JSON は出す）または入力の違反（JSON は出さない）/ 2 誤用（引数なし・未知のオプション・読めない）。`rerun-compare.sh` の 0 / 1 / 2 と同じ向き | `measure/crates/perf/tests/replay.rs` の `command_exit_codes`・`command_misuse_is_two` |
| 標準エラー | 固定の文言と引数・行の番号だけ。入力の値とパスは出さない | `command_stderr_has_no_values_or_paths` |
| 記録の形 | 変えていない。`record`・`runs`・`variation`・`remeasure`・`docs/run-record.example.json`・`scripts/` に差分は無い | `git diff origin/main -- measure/crates/perf/src/record.rs measure/crates/perf/src/runs.rs measure/crates/perf/src/variation.rs measure/crates/perf/src/remeasure.rs docs/run-record.example.json scripts/` |
| 試験 | 全規則での判定・測り直しの要否・判定不能・測り直した回の値だけでの判定、判定不能の範囲の 3 種、境目（ちょうど・直上）、判定と判定不能と NG の混在、期待を変えた入力の不一致、渡す順への非依存、分解能の通し、違反と誤用の終了コード。値・腕・行の名前はすべて合成 | `make measure-verify` |

使い方: `cargo run --manifest-path measure/Cargo.toml --bin variation-replay -- <入力の JSON> [<入力の JSON> ...]`。
記録済みの行を入力にした再現の扱いは「完了の条件の確かめ（6/6）」を参照。

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/6 現状と前提の確認 | #6363 |
| 2/6 形の設計 | #6364 |
| 3/6 実装 | #6365 |
| 4/6 実装 | #6366 |
| 5/6 テスト | #6367 |
| 6/6 確かめ | #6368 |

## 完了の条件の確かめ（6/6）

6/6（#6368）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-1027）。
GPU の実機なしで、末尾の base で実行した値。件数は実測。

| 成果物 | 本書の節 | 試験 |
| ------ | -------- | ---- |
| `variation` | 変動の幅の規則（3/6） | `measure/crates/perf/src/variation.rs` の単体試験 |
| `remeasure` | 測り直しと判定不能の扱い（4/6） | `measure/crates/perf/src/remeasure.rs` の単体試験 |
| `replay` と `variation-replay` | 再現の試験（5/6） | `measure/crates/perf/tests/replay.rs` |

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 変動の幅の規則の単体試験が通る | `cargo test --manifest-path measure/Cargo.toml --workspace --all-features --locked --lib variation::` | 15 passed; 0 failed; 0 ignored |
| A2 | 測り直しの単体試験が通る | 同じ形に `--lib remeasure::` | 13 passed; 0 failed; 0 ignored |
| A3 | 再現の結合試験が通る | 同じ形に `--test replay` | 18 passed; 0 failed; 0 ignored |
| A4 | 規則・判定不能の範囲・測り直し・不一致の検出・終了コードが合成値で確かめられている | A3 のうち `scopes_are_all_exercised`・`first_attempt_without_trigger_settles`・`first_attempt_with_trigger_needs_remeasure`・`trigger_remaining_after_remeasure_is_indeterminate_per_scope`・`remeasured_values_alone_decide`・`changed_expected_label_is_a_mismatch_with_output`・`expected_needs_remeasure_but_settled_is_a_mismatch`・`resolution_passes_through`・`command_exit_codes`・`command_misuse_is_two`・`command_stderr_has_no_values_or_paths` | すべて ok |
| B1 | 記録の形が変わっていない | base との差分: `record.rs`・`runs.rs`・`docs/run-record.example.json`・`scripts/`・`Cargo.lock`・`deps-approved.toml`・`measure/deps-approved.toml`（この PR の変更後） | 差分なし |
| B2 | 依存の向きが「測定 → 製品」の一方向 | `grep -rl measure-perf crates` | 0 件 |
| E1 | base の CI | base の直前の push の CI run（`22a5127`）を `gh run view` で確認。base（`1c97bf3`）の run は確認時点で実行中 | 完了・success（`ci-complete` も success） |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make measure-verify` | すべて終了コード 0 |

記録済みの行を入力にした再現は公開側の試験・CI に含めない。完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1027）。

### 再実行の契機

- `measure/crates/perf/src/variation.rs`・`remeasure.rs`・`replay.rs`・`bin/variation-replay.rs` を変えたとき
- `measure/crates/perf/tests/replay.rs` を変えたとき
- `record.rs`・`jsonread.rs` を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `1c97bf3`、`docs/spec` の gitlink は `8664148`。

## 本リポでの反映の方針

- 本書には現状の事実と、決定の記録のポインタ・守る規約だけを置く。後の段で足す振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 関連

[measurement-tool](measurement-tool.md)・[perf-gate-rule](perf-gate-rule.md)・[measurement-record](measurement-record.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
