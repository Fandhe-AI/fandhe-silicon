# Conventional Commits 規約

## 形式

```text
<type>[(<scope>)][!]: <日本語の要約>

<本文（任意・日本語）>
```

`scripts/hooks/commit-msg-check.sh` が commit-msg フック（lefthook）と CI の `pr-title` ジョブで検証する
（squash merge では PR タイトルがそのままコミット件名になる）。

## type

`commit-msg-check.sh` が許容する 11 種に限る。

| type | 用途 |
| ---- | ---- |
| feat | 機能追加 |
| fix | バグ修正 |
| docs | ドキュメントのみの変更 |
| style | 挙動に影響しない整形 |
| refactor | 挙動を変えないコード整理 |
| perf | 性能改善 |
| test | テストの追加・修正 |
| build | ビルド・依存関係の変更 |
| ci | CI 設定の変更 |
| chore | 上記以外の雑務 |
| revert | 取り消し |

## scope

英小文字・数字・ハイフンのみ（`[a-z0-9-]+`）。複数領域にまたがる場合は省略可。

| scope | 対象 |
| ----- | ---- |
| core | `crates/core`（共通型） |
| contract | `crates/contract`（下-1。backend 横断の変更） |
| metal / vulkan / cuda | `crates/contract` 内の各 backend 実装 |
| exec | `crates/exec`（下-2） |
| upper | `crates/upper`（上） |
| deps | `Cargo.toml` の依存・`Cargo.lock`・`deny.toml` |
| spec | `docs/spec` submodule 参照の更新 |
| skills | `.claude/skills`・`.agents/skills`・`skills-lock.json` |
| claude | `.claude/` の agents・rules・settings・`CLAUDE.md` |
| ci / scripts | `.github/workflows/`・`scripts/` |

※ crate 構成を変えたら本表も更新する。

## breaking change

- 破壊的変更は `!` を付け（例: `feat(core)!: ...`）、本文に `BREAKING CHANGE:` を記載する

## 禁止事項

- `git commit --no-verify` の使用（pre-commit / commit-msg フックを必ず通す）
- 複数の関心事を 1 コミットに混在させること（type が 2 つ以上必要なら分割する）
- スコープ外の変更の混入（[out-of-scope-tracking](./out-of-scope-tracking.md)）
- private spec の本文をコミットメッセージへ転記すること（[spec-confidentiality](./spec-confidentiality.md)）
