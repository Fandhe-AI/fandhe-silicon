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
| `build-profiles` | feature の組ごとの build / clippy / test と wasm32 の check。ソースを変更しない。wasm32 の target 未導入は失敗（skip しない） |
| `deny` | cargo-deny の検査 + workspace の path が `crates/` の外を指さないことの検査（TASK-6） |
| `deps-check` | 依存の版指定が `=x.y.z` の完全固定かと承認の記録（`deps-approved.toml`）との一致の検査（cargo metadata + jq）。ソースを変更しない・ネットワーク不要。ツール未導入は失敗、違反は終了コード 1 |
| `test-deps-check` | `scripts/deps-check.sh` の回帰テスト。一時ディレクトリの fixture だけを使い、リポジトリのファイルを変更しない・ネットワーク不要。ツール未導入は失敗（skip しない） |
| `test-inventory-trace-check` | `scripts/inventory-trace-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要・`docs/spec` の取得は不要。判定は終了コード + 標準出力 + 標準エラーの部分一致 + 違反の件数。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `test-session-record-check` | `scripts/session-record-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ。検査本体は引数（記録のパス）が要るため make ターゲットを持たない（漏れではない。直接実行する） |
| `ci-frame-check` | `.github/workflows/ci.yml`（`--workflow <path>` で差し替え可）が CI の枠の事実を保っているかの検査（集約ジョブの存在と `needs` の網羅・`runs-on` の許可ラベル・失敗の握りつぶしの綴りが無いこと・`always()` の場所）。入力は読むだけ・ネットワーク不要。終了コード 0 合格・1 違反・2 誤用または前提の不足。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `test-ci-frame-check` | `scripts/ci-frame-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `inventory-trace-check` | 一覧の鍵が項目ファイルに現れるかの検査（既定はファイル単位。項目の行〔表のデータ行〕への帰属は直接実行で `--item-rows` を付けたときだけ。TASK-16 のポインタ。bash + python3）。`--list` で合格のときだけ対応の一覧を標準出力へ出す（保存しない）。入力は読むだけ・ネットワーク不要。終了コード 0 合格・1 違反・2 誤用または入力なし（`--item-id-column` が解決できない場合を含む）（入力が無ければ成功扱いにしない）。CI は呼ばない |
| `test-hooks` | `scripts/hooks/` の回帰テスト（bash + git のみ） |
| `doctor` | 診断のみ。何も導入しない |
| `setup` | Git hooks 有効化と `.env` 作成。再実行可（冪等） |

CI（`.github/workflows/ci.yml`）はこれらのターゲットを呼ぶ。名前・引数・終了コードを変えるときは CI も同じ PR で更新する。
