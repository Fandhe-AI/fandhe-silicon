---
paths:
  - "Makefile"
  - "scripts/*.sh"
description: >
  Makefile と scripts/*.sh（scripts/hooks/ を除く）を編集する際の方針。Makefile は scripts/ を呼ぶだけの
  薄い入口とし、処理の定義は scripts/ に置く。ターゲットを増減するときの同期点と、各ターゲットの契約を規定する。
---

# Makefile / scripts 編集ルール

`scripts/hooks/**` と `lefthook.yml` は [git-hooks.md](./git-hooks.md) の担当。本ファイルは扱わない。
詳細な設計指針は make スキル（`.claude/skills/make`）を参照する。

## 基本方針

- Makefile は**薄い入口**。レシピは `@scripts/<name>.sh` か、言語ネイティブの 1 行コマンドのみ
- 複数行・条件分岐・`for`・環境依存の処理は `scripts/` の bash スクリプトへ置く
- 中身の無いターゲット（`@true` 等）は置かない（呼び出し元が終了コードを信用してしまう）
- GNU Make 3.81（macOS 標準）互換。`.ONESHELL`・`.RECIPEPREFIX` 等の 3.82+ 専用機能は使わない
- `.DEFAULT_GOAL := help` を維持する。一覧は各ターゲット行の `## ` コメントから生成される

## ターゲットを追加・改名・削除するとき

1. `.PHONY` を更新する
2. `## ` コメントを書く（無いターゲットは `make help` に出ない）
3. `scripts/help.sh`（Make 非依存の一覧）を同じ変更で同期する
4. 呼び出し元を洗い出して更新する: `grep -rn "make " .github .claude docs README.md CLAUDE.md AGENTS.md lefthook.yml scripts`
5. `make help` と `make -n <target>` で確認する

## 新規スクリプトの規約

- 先頭に `#!/usr/bin/env bash` と `set -euo pipefail`、`scripts/lib.sh` を `source` する
- 引数を取らないなら `reject_args`、必須コマンドは `require_cmd` を使う
- 実行権限を付与し（`chmod +x`）、`shellcheck` を通す（`make check` が `scripts/**/*.sh` を検査する）
- エラー出力は英語（既存の `lib.sh` に合わせる）。人向けのコメントは日本語

## 既存ターゲットの契約

| ターゲット | 契約 |
| ---------- | ---- |
| `check` | editorconfig-checker + shellcheck。ソースを変更しない。ツール未導入は成功扱いにせず失敗 |
| `verify` | `cargo fmt --check` + clippy（`-D warnings`）+ test。ソースを変更しない（整形は別途 `cargo fmt --all`） |
| `deny` | cargo-deny の検査のみ |
| `test-hooks` | `scripts/hooks/` の回帰テスト（bash + git のみ） |
| `doctor` | 診断のみ。何も導入しない |
| `setup` | Git hooks 有効化と `.env` 作成。再実行可（冪等） |

CI（`.github/workflows/ci.yml`）はこれらのターゲットを呼ぶ。名前・引数・終了コードを変えるときは CI も同じ PR で更新する。
