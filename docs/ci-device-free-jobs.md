# 機材の要らないジョブの枠（TASK-112）の現状・前提・形の設計

TASK-112（親 Issue #393）について、1/4（#3565）の現状と前提、形の設計を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
ジョブ・runner の表は複製せず [ci-job-split](./ci-job-split.md) を指す。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-112・TASK-18 と、そこから辿る決定の記録）。本リポには内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | `a7de5c2`（`update = none`。1/4 の着手時に、spec の main の先端と同じことを確かめた。確かめ方: `git ls-tree HEAD docs/spec` と `git ls-remote` による spec の main の先端の読み取り） |
| gitlink の更新 | `update-external.yml` の PR 経由。本作業では進めない |
| 完了の条件との対応づけの記録 | spec 側が正。本リポからは書き換えない |

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | PR #3（[test-frame](./test-frame.md) の同じ行と同じ根拠） |
| TASK-18（CI の分け方） | 済 | Issue #72 が CLOSED、PR #7048 がマージ済み。現状は [ci-job-split](./ci-job-split.md)。決定の内容は書かない（正は spec 側） |

## 着手時点の本リポの事実

ジョブの一覧・runner・呼ぶ make の表は [ci-job-split](./ci-job-split.md) を指す。ここには枠に関わる事実だけを置く。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| runner | `runs-on` は `ubuntu-latest` と `test-hooks` の matrix だけ。self-hosted の指定は 0 件 | `grep -n -E "runs-on\|self-hosted" .github/workflows/ci.yml` |
| 集約 | 集約以外のジョブ 5 つがすべて `ci-complete` の `needs` にある。`ci-complete` は success 以外を失敗にし、skipped を許すのは `pr-title` の `pull_request` 以外のイベントだけ | `sed -n '/^  ci-complete:/,$p' .github/workflows/ci.yml` |
| 失敗を握りつぶす書き方 | `continue-on-error` は 0 件、`\|\| true` は 0 件、`if: always()` は集約の `ci-complete` の 1 か所だけ | `grep -n -E "continue-on-error\|always\(\)" .github/workflows/ci.yml`、`grep -n -F "\|\| true" .github/workflows/ci.yml` |
| 条件つきの実行 | ジョブ単位は `pr-title`（`pull_request` のときだけ）と `ci-complete`（`always()`）。ステップ単位は `test-hooks` の macOS だけの追加のステップ 1 つ | `grep -n "if:" .github/workflows/ci.yml` |
| 既存ジョブへのステップ追加の先例 | `verify` が `make verify` に続けて `make build-profiles` を、`deny` が `make deny` に続けて `make deps-check`・`make test-deps-check`・`make test-inventory-trace-check`・`make test-session-record-check` を呼ぶ。check 名と `needs` は変えていない | `grep -n "run: make " .github/workflows/ci.yml` と各ジョブのコメント |
| スクリプトの土台 | `scripts/lib.sh` の `set -euo pipefail`・`require_cmd`（無ければ失敗）・`reject_args`（終了コード 2） | `scripts/lib.sh` |
| 回帰テストの先例 | `scripts/test-deps-check.sh`・`scripts/test-inventory-trace-check.sh`・`scripts/test-session-record-check.sh`（合成 fixture。終了コードと出力で判定） | 各スクリプトの先頭コメント |
| CI が呼ばない検査の先例 | `make inventory-trace-check`（入力が private 側にあるため） | `ci.yml` の `deny` のコメント、[makefile](../.claude/rules/makefile.md) |
| 枠の事実を機械で確かめる検査 | 0 件 | `ls scripts` |
| 必須チェック | 規約の記載は 11 件（ruleset の実物は本作業で見ていない） | [github-actions](../.claude/rules/github-actions.md) |

## 関連タスクの状態

状態は 2026-10-12 時点の `gh issue view` による。向きや関係の説明は書かない。

| 関連 | Issue | 状態 |
| ---- | ----- | ---- |
| TASK-10 | #363 | OPEN |
| TASK-11 | #364 | OPEN |
| TASK-13 | #366 | OPEN |
| TASK-15 | #367 | OPEN |
| TASK-22 | #368 | OPEN |
| TASK-113 | #394 | OPEN |
| TASK-18 | #72 | CLOSED |
| TASK-23 | #91 | OPEN |
| ID のみ | — | TASK-12・TASK-19 |

## 形の設計（1/4）

範囲は `docs/spec/05-tasks.md` の TASK-112 の内容の 2 文目。本節は本リポの `ci.yml`・`Makefile`・`scripts/`・規約から書く。

### 決定の記録に従う点（ポインタ）

| 点 | 扱い |
| -- | ---- |
| CI の分け方と実行の条件 | `docs/spec/05-tasks.md` の TASK-18 と、そこから辿る決定の記録に従う。本書には内容を書かない |
| 実機が要る試験の扱い | [ci-job-split](./ci-job-split.md) の「実機が要る試験の扱い」（ポインタ: TASK-18・TASK-19）。本節の対象の外 |
| 決定の記録から一意に決まらない点 | その点だけ止めて伺う（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）。推奨案を既定として確定しない |

### 枠の対象

- GitHub ホステッドの runner の上で完結する検査（入力は本リポの追跡ファイルと合成 fixture。ソースを変更しない）

### 分類（どのジョブに足すか）

検査は、要る道具をすでに用意しているジョブへ、ステップとして足す。

| ジョブ | 用意しているもの | 足せるステップ |
| ------ | ---------------- | -------------- |
| `check` | 版固定の editorconfig-checker・shellcheck（Rust の toolchain は入れていない） | 書式・シェルの静的検査 |
| `verify` | `rust-toolchain.toml` の toolchain | workspace のビルド・lint・試験、feature の組・target ごとの確かめ |
| `deny` | toolchain、版固定の cargo-deny、runner 同梱の jq・python3 | 依存グラフ・manifest を読む検査、検査スクリプト自体の回帰テスト |
| `test-hooks` | bash + git（ubuntu・macOS） | `scripts/hooks/` の回帰テストだけ |
| `pr-title` | `pull_request` のときだけ | PR の題の検査だけ |

- 当てはまるジョブが無い（別の OS・別の toolchain・別の時間の上限が要る）ときだけ、新しいジョブを検討する。その場合は「オーナー判断待ち」の J1 に当たる

### 実行場所

- `runs-on` は既存のラベルのまま（[github-actions](../.claude/rules/github-actions.md) の「runner 方針」）。ラベル・matrix を変えると check 名が変わるので、J1 に当たる

### 失敗したときの扱い

| 層 | 守ること | 根拠 |
| -- | -------- | ---- |
| スクリプト | 違反で非 0 終了。ツール未導入・入力なし・誤用を成功にしない | `scripts/lib.sh`、[makefile](../.claude/rules/makefile.md) の契約の表 |
| make | 1 行の呼び出しで終了コードをそのまま返す。中身の無いターゲットを置かない | [makefile](../.claude/rules/makefile.md) |
| ステップ | `continue-on-error`・`\|\| true`・`if: always()` を付けない。条件で黙って飛ばさない。ジョブごと飛ばす条件を足すなら、`ci-complete` で明示的に許容する（`pr-title` の先例） | `ci.yml` の現状 |
| ジョブ | `ci-complete` の `needs` にあるジョブへ足す | `ci.yml`、[github-actions](../.claude/rules/github-actions.md) |
| 集約 | `ci-complete` が success 以外を失敗にする。skipped の許容を増やすときは理由を PR 本文に書く | `ci.yml` の `ci-complete` |
| 失敗することの確かめ | 検査ごとに合成 fixture の回帰テストを置き、違反の入力で非 0 終了と文言を確かめる。実物の違反をコミットしない。期待値を緩めない | `scripts/test-*-check.sh` の先例、[unsafe-policy](./unsafe-policy.md) の同じ考え方 |
| 必須チェック | 登録は本リポの PR では変えない | [github-actions](../.claude/rules/github-actions.md) の「必須チェックと集約ジョブ」 |

### 検査を足すときの手順

1. 実体を `scripts/<name>.sh` に置く（[makefile](../.claude/rules/makefile.md) の「新規スクリプトの規約」）
2. `make` のターゲットを足し、同期点（`.PHONY`・`## `・`scripts/help.sh`・契約の表・`README.md`・`CLAUDE.md`）を同じ PR で直す
3. 回帰テスト（`scripts/test-<name>.sh` と `make test-<name>`）を足す
4. 「分類」の表で選んだ既存ジョブへ、検査と回帰テストのステップを足す。`ci.yml` 先頭のコメントの列挙と、規約の必須チェックの括弧書きを合わせる
5. ツールを足すなら版固定 + SHA256 検証（[github-actions](../.claude/rules/github-actions.md) のジョブ追加の 7）。外部依存の追加は承認フロー（[dependency-policy](../.claude/rules/dependency-policy.md)）
6. `lefthook` と重複させない（[git-hooks](../.claude/rules/git-hooks.md)）
7. PR の題・ブランチ名など PR 由来の文字列を `run` へ直接展開しない（`pr-title` の先例）

## 後続への引き継ぎ

- サンプルの検査が満たす性質: 中身のある検査であること／bash と runner 同梱の道具だけで動くこと／合成 fixture の回帰テストを伴うこと／関連タスクが受け持つ検査を先取りしないこと／check 名を増やさないこと
- 候補（確定しない。採否と具体は後段のコードと公開テストが示す）: `ci.yml` 自身が上の「着手時点の本リポの事実」の行（集約以外の全ジョブが `needs` にある、self-hosted と `continue-on-error` が無い、など）を保っていることを確かめる検査
- 後段で `ci.yml`・`Makefile`・`scripts/` を変えたら、本書に「現状の形」の節を足す
- 完了の条件の確かめは後段が、本リポで観測できる事実で埋める（条件の本文は転記しない）

## オーナー判断待ち

承認は得ていない。推奨案を既定として確定していない。

| # | 論点 | 止まるもの |
| - | ---- | ---------- |
| J1 | PR で生成される check を増やす・変える必要が出たときの ruleset の更新（本段の形は増やさないので、いまは生じない） | 新しいジョブ・ラベル・matrix を要る検査 |
| J2 | 規約に、実機が要る試験と CI の関係についての規範の文を足すか（ポインタ: `docs/spec/05-tasks.md` の TASK-18） | 無し（本段は足していない） |
| J3 | 完了の条件の確かめに、回帰テストに加えて実際に失敗する CI の実行の観測が要るか | 4/4 |

## 分担

| Issue | 段 |
| ----- | -- |
| #3565 | 1/4 形の設計 |
| #3566 | 2/4 実装 |
| #3567 | 3/4 CI への組み込み |
| #3568 | 4/4 確かめ |

各 Issue の状態は書かない。確かめ方は `gh issue view <番号>`。親 #393 の進み具合の記録は spec リポ側の作業。

## 確かめた日付

2026-10-12（1/4。base: origin/main の `ca396fd`、gitlink: `a7de5c2`）

## 関連

- [ci-job-split](./ci-job-split.md)
- [test-frame](./test-frame.md)
- [unsafe-policy](./unsafe-policy.md)
- [github-actions](../.claude/rules/github-actions.md)
- [makefile](../.claude/rules/makefile.md)
- [git-hooks](../.claude/rules/git-hooks.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
