# 機材の要らない CI のジョブ（TASK-112）の現状の形（2/4）

TASK-112（親 Issue #393）の 2/4（#3566）で、本リポの `ci.yml` のコードから確かめられる「機材の要らない検査と、それを走らせるジョブの対応」を記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
ジョブ・runner の表は複製せず [ci-job-split](./ci-job-split.md) を指す。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（ポインタ: `docs/spec/05-tasks.md` の TASK-112・TASK-18）。本リポには決定の内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | 確かめ方: `git ls-tree HEAD docs/spec`（`update = none`。更新は `update-external.yml` の PR 経由で、本作業では進めない） |
| 完了の条件との対応づけの記録 | spec 側が正。本リポからは書き換えない |

## 前提タスク

| 前提 | 参照 |
| ---- | ---- |
| TASK-4（workspace と crate の骨格） | `docs/spec/05-tasks.md` のポインタ。進み具合は spec 側が正 |
| TASK-18（CI の分け方） | 現状は [ci-job-split](./ci-job-split.md)。進み具合は spec 側が正 |

## 現状の形（2/4）

`ci.yml` が呼ぶ検査の対応。ジョブはすべて GitHub ホステッドの runner で、実機・GPU を使うジョブは 0 件（確かめ方: `grep -n -E "runs-on|self-hosted" .github/workflows/ci.yml`、`grep -n -i gpu .github/workflows/ci.yml`）。

| 検査 | ジョブ ID | 呼ぶ make ターゲット | 実体 |
| ---- | --------- | -------------------- | ---- |
| 整形・シェルの静的検査 | `check` | `make check` | `scripts/check.sh` |
| フックの回帰テスト | `test-hooks` | `make test-hooks` | `scripts/hooks/test-hooks.sh` |
| fmt・clippy・workspace の試験 | `verify` | `make verify` | `scripts/verify.sh` |
| feature の組ごとの build・clippy・test と wasm32 の `cargo check` | `verify` | `make build-profiles` | `scripts/build-profiles.sh` |
| 依存（ライセンス・ソース・禁止対象）と workspace の path の検査 | `deny` | `make deny` | `scripts/deny.sh` |
| 依存の `=x.y.z` 固定と承認の記録の一致 | `deny` | `make deps-check` | `scripts/deps-check.sh` |
| 上の検査の回帰テスト | `deny` | `make test-deps-check` | `scripts/test-deps-check.sh` |
| 一覧と項目の対応の検査の回帰テスト（合成 fixture） | `deny` | `make test-inventory-trace-check` | `scripts/test-inventory-trace-check.sh` |
| 回ごとの記録の形の検査の回帰テスト（合成 fixture） | `deny` | `make test-session-record-check` | `scripts/test-session-record-check.sh` |
| `ci.yml` が枠の事実を保っているかの検査 | `deny` | `make ci-frame-check` | `scripts/ci-frame-check.sh` |
| 上の検査の回帰テスト（合成 fixture） | `deny` | `make test-ci-frame-check` | `scripts/test-ci-frame-check.sh` |
| PR タイトルの形式 | `pr-title` | なし（直接呼ぶ） | `scripts/hooks/commit-msg-check.sh` |
| 上のジョブの結果の集約 | `ci-complete` | なし | `ci.yml` に直接書かれた判定 |

確かめ方: `grep -n "run: make " .github/workflows/ci.yml`

### CI が呼ばない make ターゲット

| ターゲット | 理由（コード内の記述による） |
| ---------- | ---------------------------- |
| `help`・`doctor`・`setup` | 開発者向けの案内・環境確認・導入。検査ではない |
| `inventory-trace-check` | 入力が private 側にあり、本リポの CI には組み込まない（`ci.yml` の `deny` ジョブのコメント） |

`scripts/session-record-check.sh` 本体は引数（検査する記録）が要るため make ターゲットを持たない（同スクリプトの先頭コメント）。

### まだ無いもの

| 検査 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 公開 API の検査に当たるスクリプト・make ターゲット | 0 件 | `ls scripts`、`grep -n PHONY Makefile`。受け持ちは別タスク（ポインタ: TASK-10・TASK-13・TASK-15・TASK-113。現状は [public-name-check](./public-name-check.md)） |

## 現状の形（3/4）

`ci.yml` 自身を入力にして枠の事実を確かめる検査 1 つと、その回帰テストを、既存の `deny` ジョブのステップとして実行する。

| 項目 | 内容 |
| ---- | ---- |
| 検査の入口 | `make ci-frame-check`（実体: `scripts/ci-frame-check.sh`。`--workflow <path>` で入力を差し替えられる） |
| 終了コード | 0 合格 / 1 違反 / 2 誤用・前提の不足（python3 が無い場合を含む。成功扱いにしない） |
| 回帰テスト | `make test-ci-frame-check`（合成 fixture と実物の写しだけを使い、リポジトリのファイルを変更しない） |
| 載せたジョブ | `deny`（ステップを 2 つ足しただけで、ジョブ ID・check 名・`needs`・`runs-on` は変えていない） |

検査の規則（コードの記述による。定義は `scripts/ci-frame-check.sh` の先頭コメントが正）:

| 規則 | 内容 |
| ---- | ---- |
| F1 | 集約ジョブ `ci-complete` があり、ジョブ単位の `if` が `always()` である |
| F2 | `ci-complete` の `needs` が、それ以外の全ジョブ ID と過不足なく一致する |
| F3 | 各ジョブの `runs-on` が許可ラベル（`ubuntu-latest`・`macos-latest`）か、リテラルの並びの matrix 参照である |
| F4 | `continue-on-error` のキーがどこにも無い |
| F5 | コメント行以外に `|| true`・`|| :` の綴りが無い |
| F6 | `always()` が F1 の 1 か所以外に現れない |

読めない形（タブのインデント・アンカー・対応外の書き方・ジョブ ID の重複・`jobs` の欠落）は違反にする。

検査しないもの: `ci-complete` の判定式の中身、`ci.yml` 以外の workflow、ruleset の登録、ステップ単位の `if`、F5 の綴り以外の握りつぶし。

確かめ方: `make ci-frame-check`、`make test-ci-frame-check`、`grep -n "run: make " .github/workflows/ci.yml`

4/4（#3568）への引き継ぎ: `ci-complete` の判定式の回帰テストと、実際に失敗する CI の実行の観測の要否は、本段では扱っていない。

## 本段で変えたもの・変えなかったもの

- 変えたもの: 本書の追加と、文書の索引（`README.md`・`CLAUDE.md`）・[ci-job-split](./ci-job-split.md) からのリンク
- 変えなかったもの: `.github/workflows/ci.yml`（ジョブ・check 名・`ci-complete` の `needs`）、`Makefile`、`scripts/`、`.claude/rules/github-actions.md`
- 上の照合で、機材の要らない既存の make ターゲットで CI が呼んでいないものは、上の 2 件の除外を除き見つからなかった

## 3/4・4/4 への引き継ぎ

- 触る可能性のあるファイル: `.github/workflows/ci.yml`、本書、[ci-job-split](./ci-job-split.md)
- 守る規約: [github-actions](../.claude/rules/github-actions.md)（check 名の増減・改名は ruleset の更新が要る。ジョブを足したら `ci-complete` の `needs` に足す）、[makefile](../.claude/rules/makefile.md)（中身のないターゲットを足さない）
- 観測した事実: `ci-complete` の判定は `ci.yml` に直接書かれた式で、これ自体の回帰テストは無い

## 分担

| Issue | 段 |
| ----- | -- |
| #3565 | 1/4 |
| #3566 | 2/4（本書） |
| #3567 | 3/4 |
| #3568 | 4/4 |

各 Issue の状態は書かない。確かめ方は `gh issue view <番号>`。

## 関連

- [ci-job-split](./ci-job-split.md)
- [github-actions](../.claude/rules/github-actions.md)
- [makefile](../.claude/rules/makefile.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
