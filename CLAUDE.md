# CLAUDE.md

## Overview

fandhe-silicon は、fandhe-ai・vector-db・fandhe-3d で共用する独立した低レイヤーの CPU／GPU 基盤
（device・メモリ・実行・同期・能力問合せ・診断）の実装リポジトリ。Metal・Vulkan・CUDA を背後実装とし、
wgpu を依存関係から外せる状態を目指す。

- 開発基盤は [Fandhe-AI/template-dev](https://github.com/Fandhe-AI/template-dev) 由来（Makefile → `scripts/`・lefthook・CI）
- 実装言語は Rust（edition 2024・stable）。workspace は spec D-33 の 3 段 + 共通 crate に対応する 4 crate の骨格（空の公開面・依存なし・全 crate `publish = false`）
- 本リポは **public**、仕様・フェーズ文書は **private** の `Fandhe-AI/fandhe-silicon-spec`（`docs/spec` submodule。`update = none`）
- ライセンスは MIT OR Apache-2.0

## Repository Structure

```text
fandhe-silicon/
├── CLAUDE.md
├── README.md
├── Cargo.toml                # workspace（edition・lints を一元管理）
├── Cargo.lock
├── rust-toolchain.toml       # stable + rustfmt / clippy
├── deny.toml                 # cargo-deny（ライセンス・ソース・wgpu 系の禁止）
├── Makefile                  # scripts/ を呼ぶだけの薄い入口（make help）
├── lefthook.yml              # pre-commit / commit-msg
├── skills-lock.json          # エージェントスキルの取得元・ハッシュ（update-external で日次追従）
├── .agents/skills/           # スキル実体（外部取得物）
├── .claude/
│   ├── agents/               # research / implement / testing / quality / docs
│   ├── rules/                # 委譲・コーディング・セキュリティ・spec 機密等の規約
│   ├── skills/               # .agents/skills/ への symlink
│   ├── workflows/
│   │   └── implement-issue-tree.js  # → ../skills/implement-issue-tree/scripts/ への symlink
│   └── settings.json         # SessionStart / PostToolUse hooks
├── crates/
│   ├── core/                 # fandhe-silicon-core: 3 段で受け渡す共通型（外部依存なし）
│   ├── contract/             # fandhe-silicon-contract: 下-1（backend-metal / vulkan / cuda feature）
│   ├── exec/                 # fandhe-silicon-exec: 下-2（共通操作・代わりの実行）
│   └── upper/                # fandhe-silicon-upper: 上（wgpu 風の層。forbid(unsafe_code)）
├── .github/workflows/        # ci.yml / ai-review.yml / update-external.yml
├── docs/spec/                # private submodule（明示取得が必要）
└── scripts/                  # help / doctor / setup / check / verify / deny / lib.sh
    └── hooks/                # secret-scan.sh / commit-msg-check.sh
```

依存の向きは contract → core / exec → core / upper → core, exec（spec BUILD-54〜58）。
公開名・下-1 のチップ別分割は未決のため、構成を変えたら本ツリーと委譲表を更新する。

## 委譲方針（必読）

main セッションは計画・委譲・レビュー・統合に徹し、ファイルの大量読み込み・横断検索・
外部仕様調査・コード作成は subagent へ委譲してコンテキスト消費を抑える。
詳細は [delegation.md](.claude/rules/delegation.md)（調査・設計）と
[delegation-impl.md](.claude/rules/delegation-impl.md)（作成・編集）を参照。

### パスベース切り替え表（要約）

| 対象 | 委譲先 |
| ---- | ------ |
| コード・構成の横断調査、`docs/spec` 参照（ポインタ表記） | explorer |
| Metal / Vulkan / CUDA / wgpu・依存候補の外部調査 | reference-researcher |
| `crates/core`・`crates/exec`・`crates/upper`・開発基盤（scripts / Makefile / CI） | core-builder |
| `crates/contract`（Metal / Vulkan / CUDA backend・FFI・CPU ISA） | backend-builder |
| テスト / 静的検査の実行と失敗解析 | test-runner |
| ベンチマーク・性能回帰 | bench-runner |
| レビュー / セキュリティ監査 | reviewer / security-auditor |
| lint 集計 / ドキュメント更新 | linter / docs-writer |

### model 配分

| 用途 | model |
| ---- | ----- |
| 複雑な横断判断・アーキテクチャ設計 | opus または fable（fable は特に大規模設計・横断判断の最上位 tier） |
| 調査・生成・実装・レビュー | sonnet |
| 機械的集計・lint・ドキュメント更新 | haiku |

## Sub-agents

| カテゴリ | subagent_type | model | 役割 |
| -------- | ------------- | ----- | ---- |
| research | explorer | sonnet | コードベース横断調査（読み取り専用） |
| research | reference-researcher | sonnet | 外部仕様・ライブラリ調査（読み取り専用） |
| implement | core-builder | sonnet | core / exec / upper・開発基盤の実装 |
| implement | backend-builder | sonnet | contract（Metal / Vulkan / CUDA backend・FFI・CPU ISA）の実装 |
| testing | test-runner | sonnet | `make verify` / `make deny` / `make check` と失敗解析 |
| testing | bench-runner | sonnet | ベンチマーク計測・性能回帰検出 |
| quality | reviewer | sonnet | P0/P1/P2 観点のコードレビュー |
| quality | security-auditor | sonnet | unsafe / FFI・秘密情報・spec 漏えい・サプライチェーン監査 |
| quality | linter | haiku | lint・整形の機械的確認 |
| docs | docs-writer | haiku | README・CLAUDE.md 等の更新 |

定義は `.claude/agents/<category>/<name>.md`。

## Rules

| ファイル | 内容 |
| -------- | ---- |
| [delegation.md](.claude/rules/delegation.md) | 調査・設計フェーズの委譲原則・パスベース切り替え・model 配分 |
| [delegation-impl.md](.claude/rules/delegation-impl.md) | 作成・編集フェーズの委譲マッピング・実装フロー |
| [coding-rust.md](.claude/rules/coding-rust.md) | Rust 規約（設計・unsafe / FFI・エラー・テスト） |
| [security.md](.claude/rules/security.md) | 秘密情報・メモリ安全性・OWASP Top 10 観点 |
| [spec-confidentiality.md](.claude/rules/spec-confidentiality.md) | private spec の機密保持（P0） |
| [dependency-policy.md](.claude/rules/dependency-policy.md) | 依存最小・wgpu 非依存・`=x.y.z` 固定・ユーザー承認制 |
| [japanese-style.md](.claude/rules/japanese-style.md) | 日本語出力スタイル |
| [conventional-commits.md](.claude/rules/conventional-commits.md) | Conventional Commits 規約（type / scope） |
| [code-comment-style.md](.claude/rules/code-comment-style.md) | コメント規約（役割・呼び出し文脈・SAFETY） |
| [out-of-scope-tracking.md](.claude/rules/out-of-scope-tracking.md) | スコープ外事項の Issue 追跡 |

## Current Skills

`skills-lock.json` で管理し、`update-external.yml` が日次で追従 PR を作成する。
手動で更新する場合は `skills` CLI をバージョン固定で実行する（`npx --yes "skills@<X.Y.Z>" add ...`。
未固定の `npx skills add` は使わない。固定版は `init-claude` / `update-claude` スキルの「skills CLI のバージョン固定と更新手順」節に合わせる）。

### Fandhe-AI/agent-cli-skills（ワークフロー）

| スキル | 用途 |
| ------ | ---- |
| create-commit | Conventional Commits 形式のコミット作成 |
| create-pr | PR 作成（OWASP チェック込み） |
| create-issue / create-issue-tree / update-issue-tree | Issue・Issue ツリーの作成と棚卸し |
| create-plan | 実装計画の作成（`_/local-plans/`） |
| implement-issue / implement-issue-tree | Issue 単体実装 / ツリー並列実装 |
| implement-review / implement-review-pr | ローカル diff / PR のレビュー |
| comment-code | code-comment-style に従ったコメント補強 |
| update-docs | CLAUDE.md の更新 |
| init-claude / update-claude | `.claude/` 体系の初期化 / 差分充実 |
| setup-repo-guards | CI ガード・branch protection 導入 |
| contribute-skill | スキル改修の upstream 貢献 |

### Fandhe-AI/agent-reference-skills（リファレンス）

| スキル | 用途 |
| ------ | ---- |
| apple-silicon / apple-graphics | Metal・MSL・MPS / Core Animation 等 |
| nvidia-cuda / dgx-spark | CUDA・PTX・CUTLASS / DGX Spark |
| windows-graphics-media | Direct3D・DXGI 等 |
| rust | Rust 言語・Cargo |
| fandhe-ai / fandhe-db | 上位ライブラリ（利用側）のリファレンス |
| make / lefthook / editorconfig / commitlint | 開発基盤ツール |
| github-docs | GitHub API・Actions |
| anthropic-claude-code / openai-codex | エージェント CLI |

## Conventions

- **日本語**: やりとり・報告・コメント・コミット本文は日本語（[japanese-style](.claude/rules/japanese-style.md)）
- **Conventional Commits**: `scripts/hooks/commit-msg-check.sh` が commit-msg フックと CI（PR タイトル）で検証。`--no-verify` によるバイパス禁止
- **private spec**: `docs/spec` の本文をコード・ドキュメント・Issue・PR・コミットへ転記しない。参照は ID・パスのポインタ表記（[spec-confidentiality](.claude/rules/spec-confidentiality.md)）
- **依存**: 追加・更新はユーザー承認＋ `=x.y.z` 完全固定。wgpu 系を導入しない（[dependency-policy](.claude/rules/dependency-policy.md)）
- **unsafe / FFI**: backend に閉じ込め、`// SAFETY:` 必須、公開 API は safe（[coding-rust](.claude/rules/coding-rust.md)）
- **コマンド契約**: 処理の実体は `scripts/`、`Makefile` は 1 行呼び出しのみ。ターゲット変更時は `scripts/help.sh` も更新。ローカル・hooks・CI は同じ `make check` を共有する
- **品質ゲート**: `make check`（editorconfig-checker + shellcheck）・`make verify`（fmt --check / clippy -D warnings / test）・`make deny`（cargo-deny）。CI の `check` / `verify` / `deny` ジョブも同じ `make` ターゲットを呼ぶ
- **CI**: ジョブを追加したら `ci.yml` の `ci-complete` の `needs` に必ず追加する
- **EditorConfig**: 生成・編集したファイルは editorconfig-checker を通す（pre-commit で staged を検査）。`*.rs` は 4 スペース、staged の `*.rs` は pre-commit で `rustfmt --check`
- **セキュリティレビュー**: PR 作成前に OWASP Top 10・秘密情報・spec 漏えいを確認（[security](.claude/rules/security.md)）
- **Cursor Bugbot**: PR 作成直後と修正 push 後に `gh pr comment <N> --body '@cursor review'` を投稿する（[delegation-impl](.claude/rules/delegation-impl.md) の実装フロー 6）
- **ユーザー承認フロー**: implement-issue は計画承認後に実装。依存追加・Issue 起票・スコープ外対応はユーザー承認を経る

## hooks（settings.json）

| イベント | 内容 |
| -------- | ---- |
| SessionStart | 日本語・委譲・Conventional Commits（`--no-verify` 禁止）・spec 機密・依存固定・unsafe 規約・計画承認のリマインダーを表示 |
| PostToolUse（`Edit\|Write`） | 編集した `*.rs` を `rustfmt` で整形（edition は `Cargo.toml` の `[workspace.package]` から取得、無ければ 2024。`jq` / `rustfmt` / 対象ファイルが無ければ何もしない） |

個人設定は `.claude/settings.local.json`（git 管理外）に置く。
