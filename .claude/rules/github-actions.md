---
paths:
  - ".github/workflows/**"
description: >
  GitHub Actions ワークフローを編集する際の方針。runner 方針（public は GitHub ホステッド）、必須チェックと
  集約ジョブ、発火条件、ジョブ追加チェックリスト、所要時間の計測方法を規定する。
---

# GitHub Actions 編集ルール

## runner 方針

- 本リポは **public**。runner は GitHub ホステッド（`ubuntu-latest`）を使う
- 例外: `ci.yml` の `test-hooks` は macOS の BSD 系挙動と標準 bash 3.2 を検証するため `macos-latest` も matrix に含める
- 例外: `ai-review.yml` の `codex / review` は組織の reusable workflow（`Fandhe-AI/actions`）が codex 専用 self-hosted runner で動かす。
  fork PR では reusable workflow 側が fail-closed で実行を拒否する。public で self-hosted を使うと fork PR 経由で任意コードが実行されるため、
  新たに self-hosted を追加しない（`preflight` / `post_feedback` は ubuntu-latest）

## 必須チェックと集約ジョブ

- ruleset `main-protection` の required status checks: `ci-complete`・`codex / preflight`・`codex / review`・`codex / post_feedback`
- `ci.yml` の集約ジョブ `ci-complete` が各ジョブの結果を検証する。**ジョブを追加したら必ず `needs` にも追加する**
- 上記のジョブ名・ジョブ ID を変えるときは ruleset 側の更新が必要（ユーザー判断事項。名前を変えると pending のままマージ不能になる）

## 発火条件

- `push` は `main` のみ、`pull_request` は全ブランチ（二重実行を作らない）
- `concurrency` は PR 単位で古い実行を中断する。main への push は中断しない:
  `cancel-in-progress: ${{ github.event_name == 'pull_request' }}`
- 必須チェックを含む workflow に workflow 単位の `paths` / `paths-ignore` を付けない（スキップされると required が pending のままになる）。
  絞り込むなら、ジョブ内で変更検知し、`ci-complete` で skipped を許容する
- draft PR をスキップする場合は `ready_for_review` を `types` に含める
- `schedule` は日次（`update-external.yml`）。頻度を上げる場合は理由を記載する

## ジョブ・ワークフローを追加するとき

1. 新規ワークフローより既存ワークフローへのジョブ追加を優先する（ワークフロー数を増やさない）
2. `ci-complete` の `needs` へ追加する
3. チェック内容は `scripts/` と `make` の共通コマンドを呼ぶ。CI 独自にロジックを書かない（[makefile.md](./makefile.md)）
4. `lefthook` と重複させない（[git-hooks.md](./git-hooks.md)。重い検査は CI 側）
5. 独立したジョブに不要な `needs` を作らない。セットアップが支配的な小ジョブは統合する
6. `actions/checkout` は `persist-credentials: false`。既存と同じ SHA 固定 + バージョンコメントでピン止めする
7. ツールは版固定 + SHA256 検証で導入し、`permissions` は最小にする
8. `timeout-minutes` を設定する

## 計測方法

```bash
gh run list --workflow ci.yml --limit 30 --json databaseId,conclusion,createdAt,updatedAt,event
gh run view <id> --json jobs --jq '.jobs[] | "\(.name) \((.completedAt|fromdate)-(.startedAt|fromdate))s"'
```

## 棚卸し結果（2026-10-11）

| ワークフロー | ジョブ数 | 実測（直近 30 件） |
| ------------ | -------- | ------------------ |
| `ci.yml` | 6（matrix 含め 7 実行） | 成功は 17〜50 秒が中心。各ジョブは 4〜18 秒 |
| `ai-review.yml` | 3（reusable） | 成功は 約 60 秒 |
| `update-external.yml` | reusable | 日次 約 150 秒 |

ジョブがすでに数秒のため、これ以上の分割・統合による短縮余地は小さい。
