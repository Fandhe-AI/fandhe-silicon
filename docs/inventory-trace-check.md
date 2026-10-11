# 一覧と項目の対応を確かめる検査（TASK-16）の現状と前提

TASK-16（親 Issue #60）について、着手時点の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・件数・列の内容・鍵・項目 ID の具体例は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

TASK-16 は、private spec 内の 2 つの資料の対応を機械で確かめる検査を組むタスクである。
入力はポインタで示す。

- 一覧: `docs/spec/03-poc/wgpu-feature-inventory/inventory.csv`
- 項目: `docs/spec/04-behavior/` の項目ファイル群

## 前提タスクの状態

前提タスクの有無は `docs/spec/05-tasks.md` の TASK-16 を参照する（ポインタのみ）。
完了の宣言はオーナーの確認による。

## 着手時点の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入力の所在 | 2 つとも private の submodule の中にある。本リポに写しは無い | 上記のポインタ |
| submodule の状態 | `docs/spec` は gitlink で `update = none`。既定の clone では空。取得は README「仕様」の明示手順 | `.gitmodules`、`git ls-tree HEAD docs/spec` |
| CI | `ci.yml` のどのジョブも submodule を取得しない。submodule を扱うのは `update-external.yml`（日次の追従 PR）だけ | `grep -n -i submodule .github/workflows/*.yml` |
| 帰結 | いまの public CI と既定の clone では、入力が手元に無いので検査を実行できない | 上の 2 行から |
| gitlink の追従 | 更新は `update-external.yml` の PR 経由だけ。本リポのブランチからは進めない | [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) |
| 既存の検査の流儀 | 実体は `scripts/*.sh`、`Makefile` は 1 行呼び出し、`scripts/help.sh` を同期。`scripts/deps-check.sh` は bash + `jq` + `python3 -I`（標準ライブラリのみ）で fail-closed。回帰テストは `scripts/test-deps-check.sh`（一時ディレクトリの fixture）。CI の `deny` ジョブが呼ぶ | `scripts/`、`Makefile`、`.github/workflows/ci.yml` |
| CI の決まり | ジョブを足すなら `ci-complete` の `needs` に足す。必須チェックの増減・改名は ruleset の更新（ユーザー判断事項）を伴う | [github-actions](../.claude/rules/github-actions.md) |
| 依存 | 外部依存は 0。追加は承認制 | `cargo metadata --format-version 1 --all-features --locked` |
| PoC の道具 | 持ち込まない。製品側の検査は新しく組む。入力は読むだけで書き換えない | TASK-6 の決定の記録（ポインタのみ）、[poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |

## pinned commit での試行

gitlink の指す commit の入力を一時ディレクトリへ取得し、使い捨てのスクリプトで突き合わせる試行をした。
試行の結果は spec の構造・中身に触れるため、本書には書かない。対応の定義に関わる所見は、spec 側の判断待ちとして
ユーザーへ報告する（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）。

- CSV は引用符付きのセルを含みうるので、行分割や `cut` ではなく CSV パーサが要る（一般的な CSV の性質）

CI では実行していない。取得物・生成物・スクリプトはコミットしていない（一時ファイルは削除済み）。

## 検査のスクリプト（#62）

入口は `make inventory-trace-check`（実体は `scripts/inventory-trace-check.sh`。bash + `python3 -I`、標準ライブラリのみ）。
入力は読むだけで、ファイルを作らず、ネットワークを使わない。CI には組み込んでいない（Q1 が未回答のため。いまの CI は submodule を取得しない）。

引数（直接実行で渡す。`make` 経由は既定のパス）:

| 引数 | 既定 | 意味 |
| ---- | ---- | ---- |
| `--csv <path>` | 上記ポインタの一覧 | 先頭のレコードを見出しとして扱う |
| `--items-dir <dir>` | 上記ポインタの項目のディレクトリ | 直下の `*.md` だけを読む（再帰しない） |
| `--key-column <name>` | 先頭の列 | 鍵の列を見出しの名前で指す。名前はスクリプトに書かない |
| `--exclude <basename>` | なし（繰り返し可） | 探索から外すファイル名。`README.md` は常に外す |
| `--show-keys` | 無効 | 違反の行に鍵の文字列も出す。出力は spec の内容なので、PR・Issue・CI のログ・コミットに貼らない |

終了コード:

| コード | 意味 |
| ------ | ---- |
| 0 | 合格。すべての行の鍵が、少なくとも 1 つの項目ファイルに現れた |
| 1 | 違反（fail-closed）。現れない鍵・鍵が空・鍵が重複・列の数の不一致・データの行が 0・探索対象が 0・CSV や項目ファイルを読めない |
| 2 | 誤用・前提の不足（検査を終えていない）。未知の引数・入力が無い（既定の clone など）・`python3` が無い・`--key-column` や `--exclude` が解決できない・予期しない内部エラー |

前提の検査を先に行い、欠けていれば内容を読まずに 2 で終える。入力が無いときに 0 を返さない。

判定の規則（**未承認の仮置き**。前向きだけ。「対応」の定義は spec 側の判断待ち）:

1. CSV を `csv` モジュールで解析し、鍵の列の値を各行の鍵とする
2. 鍵が「現れる」とは、項目ファイルの本文にその文字列があり、直前と直後の文字がどちらも「全鍵に現れる文字の集合」に入らないこと（別の鍵の一部は数えない境界。正規表現は使わない）
3. すべての鍵が、探索対象の少なくとも 1 つに現れれば合格

規則は `occurs()` の 1 関数に閉じ込めてあり、決まったらその関数だけを差し替える。

既知の限界:

- 前向きだけ。逆向きは未実装（spec 側の判断待ち）
- 粒度は「項目ファイルのどれかに現れる」まで。どの項目が受け持つかは見ない（#63 へ引き継ぐ）
- 鍵の直後・直前に、鍵の文字集合に入る文字が句読点として続く書き方は「現れた」に数えない（厳しい側に倒れる）
- CI では走らない。既定の clone では入力が無く、2 で終わる
- 回帰テストはまだ無い（#64）

Q1〜Q3 は未回答のままで、#62 はどの案も確定させていない（CI に組み込まない・spec 由来の語を書かない・一覧を出力しない）。

## 関連タスクの状態と引き継ぎ

事実のみを書く。向きは決めない。

| 関連 | Issue | 状態 | 引き継ぎ |
| ---- | ----- | ---- | -------- |
| TASK-17 | #65 | 未完了 | 関連（ID のみ） |
| TASK-31 | #147 | 未完了 | 関連（ID のみ） |
| TASK-85 | #378 | 未完了 | 関連（ID のみ） |
| TASK-103 | #2451 | 未完了 | 関連（ID のみ） |
| TASK-147 | #1106 | 未完了 | 関連（ID のみ） |
| TASK-1313 | #1015 | 未完了 | 関連（ID のみ） |
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
| 実装 | #62（2/4。検査のスクリプト。済）・#63（3/4） |
| テスト | #64（4/4） |

## 完了の条件の確かめ

1/4 では未実施。#64 で、本リポで観測できる事実により埋める。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。

## 確かめた日付

2026-10-11（base: 2ab7b91。#62 の追記）
