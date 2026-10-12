---
paths:
  - ".github/workflows/**"
description: >
  GitHub Actions ワークフローを編集する際の方針。runner 方針（public は GitHub ホステッド）、必須チェックと
  集約ジョブ、発火条件、ジョブ追加チェックリスト、キャッシュ方針（保存は main のみ・キー設計・容量）、
  所要時間の計測方法を規定する。
---

# GitHub Actions 編集ルール

## runner 方針

- 本リポは **public**。runner は GitHub ホステッド（`ubuntu-latest`）を使う
- 例外: `ci.yml` の `test-hooks` は macOS の BSD 系挙動と標準 bash 3.2 を検証するため `macos-latest` も matrix に含める
- 例外: `ai-review.yml` の `codex / review` は組織の reusable workflow（`Fandhe-AI/actions`）が codex 専用 self-hosted runner で動かす。
  fork PR では reusable workflow 側が fail-closed で実行を拒否する。public で self-hosted を使うと fork PR 経由で任意コードが実行されるため、
  新たに self-hosted を追加しない（`preflight` / `post_feedback` は ubuntu-latest）

## 必須チェックと集約ジョブ

- ruleset `main-protection` の required status checks（11 件）:
  - `ci.yml`: `ci-complete`・`make check (editorconfig-checker + shellcheck)`・`make verify (fmt + clippy + test)`・
    `make deny (cargo-deny)`（`make deps-check` / `make test-deps-check` / `make test-inventory-trace-check` / `make test-session-record-check` も含む。check 名は不変）・`PR title (Conventional Commits)`・`hook script tests (ubuntu-latest)`・`hook script tests (macos-latest)`
  - `ai-review.yml`: `codex / preflight`・`codex / review`・`codex / post_feedback`
  - 外部 App: `Cursor Bugbot`（cursor App に束縛。それ以外は GitHub Actions の integration に束縛）
- 個別ジョブまで登録するのは、implement-issue-tree の自動マージ前ゲート（G0）が PR 上の全 check を必須チェックに含めることを要求するため。
  PR で生成される check を増やす・減らす・改名するときは、ruleset 側も同時に更新する（ユーザー判断事項。
  登録漏れは自動マージの停止、登録した check が生成されないと pending のままマージ不能になる）
- `ci.yml` の集約ジョブ `ci-complete` が各ジョブの結果を検証する。**ジョブを追加したら必ず `needs` にも追加する**
- matrix を変えると check 名（`hook script tests (<os>)` 等）が変わる。push だけで生成される check は ruleset に登録しない

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
9. キャッシュを使う場合は「キャッシュ方針」に従う（保存は main への push のみ。確認結果を PR 本文に書く）

## キャッシュ方針

現状（2026-10-11）: `ci.yml` は GitHub Actions のキャッシュを使っていない。リポジトリのキャッシュは 0 件・0 バイト
（`gh cache list` と `actions/cache/usage` で確認。以前は PR #3 の実行が `refs/pull/3/merge` に保存した 3 件・約 17MB が
残っていたが、すでに無い）。キャッシュを追加する場合は本節に従う。

### 前提（容量と課金）

- リポジトリあたりの上限は 10GB（Org 設定も 10GB）。超えると最終アクセスの古い順に追い出され、7 日間アクセスの無い
  キャッシュは削除される
- Org の Actions 予算は $0（超過時は停止）のため、10GB を超える上限は使えない。上げると 10GB を超えた時点で
  キャッシュが読み取り専用になる。上限・予算の変更はユーザー判断

### 入れる前に効果を測る

- 本リポは GitHub ホステッド runner で毎回まっさらな VM から始まるため、キャッシュが効く余地はある。ただし現状の
  各ジョブは 4〜18 秒で、復元にかかる時間の方が長くなり得る。足す前後でジョブログの復元秒数・`Compiling` の件数
  （依存の再ビルド数）を比べ、効果を確かめてから入れる
- self-hosted runner ではツールチェーンや `CARGO_TARGET_DIR` 等がジョブをまたいで残り、キャッシュが無意味になる
  ことがある（他リポの実例: mirror-ui は pnpm キャッシュの復元に 1 ジョブ平均 75 秒かけていたが `downloaded 0` で
  効果ゼロだったため撤去。team-hub は `Swatinem/rust-cache` が `./target` を保存対象にしていたが runner の
  `CARGO_TARGET_DIR` が別の場所で registry しか保存されておらず、後処理が共有 `CARGO_HOME` の crate を消していた
  ため撤去）。runner の種類を変えるときも測り直す

### 保存は main への push だけ

- PR から保存したキャッシュは `refs/pull/<N>/merge` スコープに入り、その PR からしか読めないのに上限を共有して
  main のキャッシュを追い出す（以前残っていた PR #3 の 3 件がこれにあたる）
- `actions/cache` は `actions/cache/restore`（全イベント）と `actions/cache/save` に分け、save には
  `if: github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.<id>.outputs.cache-hit != 'true'` を付け、
  `key: ${{ steps.<id>.outputs.cache-primary-key }}` で保存する（`<id>` は restore ステップの `id`）
- `Swatinem/rust-cache` を使うなら `save-if: ${{ github.ref == 'refs/heads/main' }}` を渡す
- Fandhe-AI/actions の `rust-base-ci.yml` を `cache: true` で呼ぶなら `cache-save: ${{ github.ref == 'refs/heads/main' }}`
  も渡す（共通側の `cache-save` の既定は true で、PR からも保存する）
- `actions/setup-node` の `cache: pnpm` など、`setup-*` 系 action の一体型キャッシュは PR でも保存する。使うなら
  保存を main に限れる restore / save の分離に置き換える

### キーの設計

- キーは中身のハッシュ（lockfile 等）にする。`github.sha`・`github.run_id` など毎回変わる値は入れない。入れる場合は
  `restore-keys` の前方一致で直近を拾う設計にし、保存は main に限る
- `hashFiles('**/Cargo.lock')` のような再帰 glob は使わず、ルートの `Cargo.lock` を指定する。本リポにも
  `.agents/skills/make/samples/projects/` 配下にスキルのサンプル（`Cargo.lock` 3 件・`pnpm-lock.yaml` 2 件）があり、
  再帰 glob はこれを拾う。依存が変わらないのにスキル更新でキーが変わり全キャッシュを作り直す事故が fandhe-browser・
  fandhe-frontend・fandhe-db・fandhe-container・template-frontend-react_router で起きた
- `target/` などビルド成果物をキャッシュするなら、キーにツールチェーンの版（`rustc -vV` の `release`・
  `commit-hash`）を入れる。本リポの `rust-toolchain.toml` は `channel = "stable"` のため、このファイルのハッシュでは
  版の更新を検知できない。版を入れないと toolchain 更新後も古い成果物に exact hit し続け、exact hit では再保存
  されないため依存を毎回全再ビルドする（fandhe-db で依存 135 crate を毎回再ビルドしていた）
- `restore-keys` で前方一致を許し、キーが変わっても cold にならないようにする
- キーが捕捉しない変更（`Cargo.toml` の `[profile]`、`RUSTFLAGS`・`CARGO_PROFILE_*` 等の env、`.cargo/config.toml`、
  ビルドコマンドやフラグ）で保存内容が変わるときは、キーに版セグメント（`-v2-` 等。無ければ追加する）を入れて上げ、
  `restore-keys` も同時に上げる。上げないと旧内容に exact hit したまま依存を毎回再ビルドし、exact hit なので
  再保存もされない（他リポで `[profile.dev.package."*"]` を足したときに起きた。依存のビルドスクリプト・proc-macro の
  debuginfo が変わり `-C metadata` が連鎖して変わる）。キーが `Cargo.toml` を含んでいても、旧世代の部分復元で新旧の
  成果物が混ざった太い保存になるため上げる
- `Swatinem/rust-cache` は workspace メンバーの `Cargo.toml` しかハッシュしない（`cargo metadata --no-deps` の
  packages の manifest のみ）。本リポのルートは仮想 manifest（`[package]` なし）のため、ルートの `[profile]` の変更は
  キーに入らない。変えるときは `prefix-key` に版セグメントを付けて上げる
- Fandhe-AI/actions の `rust-base-ci.yml` を `cache: true` で呼ぶ場合、呼び出し側からキーは変えられない。上記の
  変更を入れた PR の **マージ後に** main スコープの `<prefix>-<OS>-{clippy,test}-v4-*` を `gh cache delete` する
  （マージ前に消すと main の push で旧内容が保存し直される）

### サイズを抑える

- 1 世代の合計が 10GB に余裕をもって収まるようにする。キャッシュの種類を増やすときは 1 件のサイズ × 種類 × OS
  （matrix）を見積もる
- `target/` を保存するなら、保存前に workspace メンバー（`crates/*`）の成果物を prune し、`CARGO_INCREMENTAL=0`、
  CI 限定で `CARGO_PROFILE_DEV_DEBUG=line-tables-only` 等で縮める（依存側はルート `Cargo.toml` の
  `[profile.dev.package."*"]` で既に行番号のみ。メンバーまで行番号のみにするのは env だけ）
- 上限に達しても、毎回読まれる main の最新キャッシュは最後まで残る（LRU）。キーの切り替えで古い世代が追い出されても
  新しい世代が保存済みならミスにならない。この状態を保てていれば上限を上げる必要はない

### 計測

```bash
gh api repos/Fandhe-AI/fandhe-silicon/actions/cache/usage                      # 件数・合計サイズ
gh cache list -R Fandhe-AI/fandhe-silicon -L 100                                # key・サイズ・最終アクセス
gh cache list -R Fandhe-AI/fandhe-silicon -L 100 --json id,key,ref,sizeInBytes  # ref（main か PR か）
```

- ジョブログの `Cache restored from key:`（primary key と一致なら exact、それ以外は partial）・`Cache not found`・
  `Cache saved with key:` で hit / miss と保存を確かめる
- 不要なキャッシュは `gh cache delete <id>` で消せるが、共有状態なので対象（key・ref）を確認してから消す

### PR のチェックリスト

ワークフローにキャッシュを追加・変更する PR では、次を確認したことを PR 本文に書く。

- [ ] 効果を実測した（復元秒数・依存の再ビルド数。効果が無ければ入れない・残さない）
- [ ] 保存は main への push に限っている
- [ ] キーはルートの lockfile のハッシュで、成果物を含むならツールチェーンの版を含み、`restore-keys` がある
- [ ] 1 世代の合計サイズを見積もり、10GB に余裕をもって収まる

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
