# 一覧と項目の対応を確かめる検査（TASK-16）の現状と前提

TASK-16（親 Issue #60）について、着手時点の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・件数・列の内容・鍵・項目 ID の具体例は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

TASK-16 は、private spec 内の 2 つの資料の対応を機械で確かめる検査を組むタスクである。
入力はポインタで示す。

- 一覧: `docs/spec/03-poc/wgpu-feature-inventory/inventory.csv`
- 項目: `docs/spec/04-behavior/` の項目ファイル群

## 前提タスクの状態

TASK-16 に前提タスクは無い（`docs/spec/05-tasks.md` の TASK-16。ポインタのみ）。
完了の宣言はオーナーの確認による。

## 着手時点の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入力の所在 | 2 つとも private の submodule の中にある。本リポに写しは無い | 上記のポインタ。リポ内検索で TASK-16 向けの文書・スクリプトは 0 件 |
| submodule の状態 | `docs/spec` は gitlink で `update = none`。既定の clone では空。取得は README「仕様」の明示手順 | `.gitmodules`、`git ls-tree HEAD docs/spec` |
| CI | `ci.yml` のどのジョブも submodule を取得しない。submodule を扱うのは `update-external.yml`（日次の追従 PR）だけ | `grep -n -i submodule .github/workflows/*.yml` |
| 帰結 | いまの public CI と既定の clone では、入力が手元に無いので検査を実行できない | 上の 2 行から |
| gitlink の追従 | 更新は `update-external.yml` の PR 経由だけ。本リポのブランチからは進めない | [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) |
| 既存の検査の流儀 | 実体は `scripts/*.sh`、`Makefile` は 1 行呼び出し、`scripts/help.sh` を同期。`scripts/deps-check.sh` は bash + `jq` + `python3 -I`（標準ライブラリのみ）で fail-closed。回帰テストは `scripts/test-deps-check.sh`（一時ディレクトリの fixture）。CI の `deny` ジョブが呼ぶ | `scripts/`、`Makefile`、`.github/workflows/ci.yml` |
| CI の決まり | ジョブを足すなら `ci-complete` の `needs` に足す。必須チェックの増減・改名は ruleset の更新（ユーザー判断事項）を伴う | [github-actions](../.claude/rules/github-actions.md) |
| 依存 | 外部依存は 0。追加は承認制 | `cargo metadata --format-version 1 --all-features --locked` |
| PoC の道具 | 持ち込まない。製品側の検査は新しく組む。入力は読むだけで書き換えない | TASK-6 の決定の記録（ポインタのみ）、[poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |

## pinned commit での試行

gitlink の指す commit の入力を一時ディレクトリへ取得し、使い捨ての `python3 -I` スクリプト（標準ライブラリの `csv` だけ）で突き合わせた。

- 前向き（一覧の行 → 項目）: 一覧の各行の鍵（先頭の列の値）が、項目ファイル（`README.md` を除く）の表の行に文字列として現れるかを見た。対応の無い行は出なかった。文字列一致による機械判定は実現できる見込みがある
- 同じ鍵が複数の項目の行に現れる行がある。受け持つ項目を 1 つに特定する規則は、文字列一致だけでは決まらない
- 逆向き（項目 → 一覧の行）: 項目の行に現れる、一覧の鍵と同じ形の文字列のうち、一覧に行の無いものが少数ある（関連のポインタ: TASK-552・TASK-1375）。厳密な一致にすると失敗する
- CSV は引用符付きのセルを含みうるので、行分割や `cut` ではなく CSV パーサが要る

CI では実行していない。取得物・生成物・スクリプトはコミットしていない（一時ファイルは削除済み）。

## 関連タスクの状態と引き継ぎ

事実のみを書く。向きは決めない。

| 関連 | Issue | 状態 | 引き継ぎ |
| ---- | ----- | ---- | -------- |
| TASK-17 | #65 | 未完了 | TASK-16 を前提に引く下流。TASK-16 が終わるまで着手できない |
| TASK-31 | #147 | 未完了 | 関連（ID のみ） |
| TASK-85 | #378 | 未完了 | 関連（ID のみ） |
| TASK-103 | #2451 | 未完了 | 関連（ID のみ） |
| TASK-147 | #1106 | 未完了 | 関連（ID のみ） |
| TASK-1313 | #1015 | 未完了 | TASK-16 の検査へ足す側。検査が先に在ることが前提 |
| TASK-552・TASK-916・TASK-2384・TASK-1375 | - | - | 関連（ID のみ） |
| TASK-6 | #35 | 済（クローズ） | PoC の道具を持ち込まないことのポインタ |

## #62 以降へ引き継ぐ実装の前提

- 実装は bash + `python3 -I`（標準ライブラリのみ）とし、依存は足さない
- 実体は `scripts/` に置き、`Makefile` は 1 行呼び出し、`scripts/help.sh` を同期する。fail-closed とする
- 回帰テストは合成の fixture（spec 由来でない値）で書く
- CI にジョブを足す場合は `ci-complete` の `needs` に足す。必須チェックの変更は ruleset の更新が要るため、ユーザー判断事項として報告する
- 入力は読むだけで書き換えない。spec 由来の出力を public のログ・コミットに残さない

## オーナー判断待ちの論点

既存の決定から一意に導けない。選択肢・影響・推奨は PR 本文に残す（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）。推奨は未承認で、確定扱いにしない。

| # | 論点 | 止まるもの |
| - | ---- | ---------- |
| Q1 | 検査を走らせる場所（本リポのローカル実行／本リポの CI で submodule を取得／spec リポ側の CI） | #62・#63 の置き場、#64 のテストの形 |
| Q2 | 成果物「対応の一覧」の置き場（項目 ID の一覧・索引は本リポに置けない規約との関係） | #63 がコミットしてよいもの |
| Q3 | 公開するスクリプトに spec 由来の語（列名・鍵の形・項目の行の書式）を定数で書いてよいか | #62 の作り |

## spec 側の判断待ち

「対応」の定義は、本リポから決められない（spec 側の判断待ち）。ポインタのみ。

- 前向きの判定規則（TASK-16）
- 逆向きの判定規則（関連: TASK-552・TASK-1375）

## 後続の分担

後続は本書の該当節へ追記する。

| 作業 | 担当 |
| ---- | ---- |
| 現状と前提の確認（本節群） | #61（1/4） |
| 実装 | #62（2/4）・#63（3/4） |
| テスト | #64（4/4） |

## 完了の条件の確かめ

1/4 では未実施。#64 で、本リポで観測できる事実により埋める。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。

## 確かめた日付

2026-10-11（base: a83bf42）
