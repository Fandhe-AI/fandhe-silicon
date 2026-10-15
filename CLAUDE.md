# CLAUDE.md

## Overview

fandhe-silicon は、fandhe-ai・vector-db・fandhe-3d で共用する独立した低レイヤーの CPU／GPU 基盤
（device・メモリ・実行・同期・能力問合せ・診断）の実装リポジトリ。Metal・Vulkan・CUDA を背後実装とし、
wgpu を依存関係から外せる状態を目指す。

- 開発基盤は [Fandhe-AI/template-dev](https://github.com/Fandhe-AI/template-dev) 由来（Makefile → `scripts/`・lefthook・CI）
- 実装言語は Rust（edition 2024・stable）。workspace は spec D-33 の 3 段 + 共通 crate に対応する 4 crate の骨格（ほぼ空の公開面・依存なし・全 crate `publish = false`。`core` だけ feature `test-support` の裏に検証用の道具 `parity` を持つ）
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
├── public-name-check.conf    # public-name-check の語の一覧（`--config` で渡す）
├── deny.toml                 # cargo-deny（ライセンス・ソース・wgpu 系の禁止。baseline-wgpu/deny.toml は禁止を外した別 workspace 用）
├── deps-approved.toml        # 依存の承認の記録（deps-check が検査。初期は 0 件）
├── Makefile                  # scripts/ を呼ぶだけの薄い入口（make help）（編集方針: .claude/rules/makefile.md）
├── lefthook.yml              # pre-commit / commit-msg（方針: .claude/rules/git-hooks.md）
├── skills-lock.json          # エージェントスキルの取得元・ハッシュ（update-external で日次追従）
├── .agents/skills/           # スキル実体（外部取得物）
├── .claude/
│   ├── agents/               # research / implement / testing / quality / docs
│   ├── rules/                # 委譲・コーディング・セキュリティ・spec 機密等の規約
│   ├── skills/               # .agents/skills/ への symlink
│   ├── workflows/
│   │   └── implement-issue-tree.js  # → ../skills/implement-issue-tree/scripts/ への symlink
│   └── settings.json         # SessionStart / PostToolUse hooks
├── measure/                  # 測定専用の別 workspace（TASK-6・TASK-27）。共有 crate `measure-perf`（`measure/crates/perf/`: 標本・run ごとの統計・run 間の集計・run ごとの記録の書き出し・記録を読んで run 間の集計を出すコマンド `across-runs`）。製品の workspace の外（ルート `exclude`）。検査は `make measure-verify`（CI の `deny` ジョブも呼ぶ）
├── baseline-wgpu/            # 比較基準の wgpu を置く別 workspace（基盤の依存グラフの外。`make deny` が cargo-deny〔`baseline-wgpu/deny.toml`〕と deps-check をかける。wgpu は `=30.0.1` で固定、アダプタ列挙の smoke bin のみ）
├── crates/
│   ├── core/                 # fandhe-silicon-core: 3 段で受け渡す共通型（外部依存なし。`test-support` の裏に `parity` と `test_frame`）
│   ├── contract/             # fandhe-silicon-contract: 下-1（backend-metal / vulkan / cuda feature。backend-dx12-compute は中身のない仮の置き場で既定では無効）
│   ├── exec/                 # fandhe-silicon-exec: 下-2（共通操作・代わりの実行）
│   └── upper/                # fandhe-silicon-upper: 上（wgpu 風の層。forbid(unsafe_code)）
├── .github/workflows/        # ci.yml / ai-review.yml / update-external.yml
├── docs/contract/            # 契約の明文のまとめと、形を決める作業の手順書（書式・更新の順は README）
├── docs/workspace-boundary.md  # 製品 workspace の境界の案内（決定の記録の正は private spec）
├── docs/wgpu-baseline-record.md  # 比較基準の wgpu の版の記録の書式（理由・経緯は private spec）
├── docs/baseline-wgpu.md     # 比較基準の wgpu の置き場の所在・製品 workspace を守る仕組み・別 workspace の配置・版の記録・確かめ方（TASK-7）
├── docs/parity-tool.md       # 結果の一致を確かめる共通の道具の所在・振る舞い・対応するテスト・閾値と範囲の扱い・確かめ方（TASK-24）
├── docs/pixel-parity-tool.md # 画素の一致を確かめる共通の道具の所在・振る舞い・対応するテスト・確かめ方（TASK-26）
├── docs/parity-non-finite.md # 有限でない値の規則の所在・振る舞い・対応するテスト・確かめ方（TASK-25）
├── docs/unsafe-policy.md     # unsafe の局所化・不変条件の書式・個数テストの枠の形・個数が変わるときの扱い（TASK-9）
├── docs/inventory-trace-check.md  # 一覧と項目の対応を確かめる検査の所在・検査のスクリプト・対応するテスト・確かめ方（TASK-16）
├── docs/implementation-scope.md  # 実装に含める範囲の決定の記録の所在と本リポの現状の事実（TASK-43）
├── docs/vulkan-operand-extraction.md  # Vulkan の成果物からオペランドの宣言を取り出す道具の所在と、本リポの現状の事実（TASK-98）
├── docs/dx12-compute-check.md  # DX12 の compute に関する点検の記録の所在と、本リポの現状の事実（TASK-236）
├── docs/ci-job-split.md      # CI の分け方の決定の記録の所在と、本リポの CI の現状の事実（TASK-18）
├── docs/ci-device-free-jobs.md  # 機材の要らないジョブの枠の所在と、本リポの現状の事実（TASK-112）
├── docs/test-frame.md       # 共通の試験の枠の所在と、本リポの現状の事実（TASK-23）
├── docs/public-name-check.md  # 公開 API の名前の検査の決定の記録の所在と、本リポの現状の事実（TASK-14・TASK-113）
├── docs/public-api-wgpu-check.md  # 公開 API に wgpu の型が無いことの検査の所在・振る舞い・対応するテスト・確かめ方（TASK-10）
├── docs/measurement-record.md  # 測定の記録の形式と置き場の決定の記録の所在と、本リポの現状の事実（TASK-28）
├── docs/measurement-tool.md  # 性能を測る共通の道具の所在・本リポの現状の事実・形の設計（決定の記録のポインタと守る規約）・現状の形（3/8〜7/8・9/11・10/11）・完了の条件の確かめ（8/8）（TASK-27）・回の run_ids との突き合わせと完了の条件の確かめ（TASK-2947）
├── docs/metal-run-procedure.md  # Metal の実機での試験の実行手順と、手順のスクリプト（metal-session.sh）の振る舞い・回帰テストとの対応（TASK-19）
├── docs/session-record.example.json  # 回ごとの記録のダミーの記入例（session-record-check の回帰テストが合格を確かめる）
├── docs/run-record.example.json  # run ごとの記録のダミーの記入例（書き出しの実物とバイト一致。perf-record-check の回帰テストが合格を確かめる）
├── docs/spec/                # private submodule（明示取得が必要）
└── scripts/                  # help / doctor / setup / check / verify / build-profiles / deny / measure-verify / deps-check / test-deps-check / inventory-trace-check / test-inventory-trace-check / session-record-check（make ターゲットなし）/ test-session-record-check / perf-record-check（make ターゲットなし）/ test-perf-record-check / record-summary（make ターゲットなし）/ test-record-summary / ci-frame-check / test-ci-frame-check / public-api-check / test-public-api-check / public-name-check / test-public-name-check / metal-session（make ターゲットなし）/ test-metal-session / lib.sh
    └── hooks/                # secret-scan.sh / commit-msg-check.sh / test-hooks.sh（フック自体の回帰テスト）
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
| `crates/core`・`crates/exec`・`crates/upper`・`baseline-wgpu/`・開発基盤（scripts / Makefile / CI） | core-builder |
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
| [spec-confidentiality.md](.claude/rules/spec-confidentiality.md) | private spec の機密保持（P0）・private spec 側の状態を公開側に書かない・公開 API の rustdoc に書いてよい振る舞いの範囲 |
| [dependency-policy.md](.claude/rules/dependency-policy.md) | 依存最小・wgpu 非依存・`=x.y.z` 固定・ユーザー承認制 |
| [japanese-style.md](.claude/rules/japanese-style.md) | 日本語出力スタイル |
| [conventional-commits.md](.claude/rules/conventional-commits.md) | Conventional Commits 規約（type / scope） |
| [code-comment-style.md](.claude/rules/code-comment-style.md) | コメント規約（役割・呼び出し文脈・SAFETY） |
| [out-of-scope-tracking.md](.claude/rules/out-of-scope-tracking.md) | スコープ外事項の Issue 追跡 |
| [makefile.md](.claude/rules/makefile.md) | Makefile は薄い入口・ターゲット増減の同期点・scripts 規約（`Makefile`・`scripts/*.sh` 編集時のみ読み込み） |
| [github-actions.md](.claude/rules/github-actions.md) | 既存ジョブへ検査を足すときの枠・runner 方針・必須チェック（ruleset 登録の 11 件）と集約ジョブ `ci-complete`・発火条件・ジョブ追加チェックリスト・キャッシュ方針（`.github/workflows/**` 編集時のみ読み込み） |
| [git-hooks.md](.claude/rules/git-hooks.md) | lefthook のフック方針（pre-commit 1 秒未満・重い検査や CI 重複は CI へ。`lefthook.yml`・`scripts/hooks/` 編集時のみ読み込み） |
| [contract-shaping-escalation.md](.claude/rules/contract-shaping-escalation.md) | 形を決めるタスクでのオーナー判断の伺い方（伺いは summary に残す）・spec 側への差し戻しの提案手順（spec 配下を書き換えない） |
| [poc-carry-over.md](.claude/rules/poc-carry-over.md) | PoC の隔離 workspace から持ち込むときの守り方（依存グラフに足場 0 件・記録を写さない・測定 → 製品の一方向。区分の正は private spec） |

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
- **品質ゲート**: `make check`（editorconfig-checker + shellcheck）・`make verify`（fmt --check / clippy -D warnings / test。`cargo test` の 1 回だけ試験の枠の記録を一時ディレクトリへ書かせ、集計の表を標準出力〔CI ではジョブのログ〕に出して消す）・`make deny`（cargo-deny + workspace の path が `crates/` の外を指さないことの検査 + `measure/`・`baseline-wgpu/` の cargo-deny と `baseline-wgpu/` の deps-check。jq が要る）・`make deps-check`（依存の `=x.y.z` 完全固定と承認の記録との一致の検査。CI の `deny` ジョブが `make deny` に続けて呼ぶ）。`scripts/deps-check.sh` を変更したら `make test-deps-check`（回帰テスト）も通す。CI の `check` / `verify` / `deny` ジョブも同じ `make` ターゲットを呼ぶ。`scripts/inventory-trace-check.sh` を変更したら `make test-inventory-trace-check`（合成 fixture の回帰テスト。CI の `deny` ジョブが呼ぶ）も通す。`make inventory-trace-check` は手元だけの検査で、`docs/spec` の取得が要り CI には組み込んでいない。`scripts/session-record-check.sh` を変更したら `make test-session-record-check`（合成 fixture の回帰テスト。CI の `deny` ジョブと `test-hooks` ジョブが呼ぶ）も通す。`scripts/perf-record-check.sh` を変更したら `make test-perf-record-check`（合成 fixture の回帰テスト。CI の `deny` ジョブが呼ぶ。`measure/crates/perf/tests/repeat.rs` が実物の出力を通す）も通す。`scripts/metal-session.sh` を変更したら `make test-metal-session`（偽のコマンドで macOS のコマンドを差し替える回帰テスト。CI の `deny` ジョブと `test-hooks` ジョブが呼ぶ）も通す。`scripts/record-summary.sh` または `scripts/verify.sh` を変更したら `make test-record-summary`（合成の部品と偽の cargo の回帰テスト。CI の `deny` ジョブが呼び、`test-hooks` ジョブの macOS では標準の bash 3.2 で直接実行する）も通す。`scripts/ci-frame-check.sh` または `.github/workflows/ci.yml` を変更したら `make ci-frame-check` と `make test-ci-frame-check`（合成 fixture の回帰テスト）も通す（どちらも CI の `deny` ジョブが呼ぶ）。`make public-api-check` と `make test-public-api-check` は CI の `deny` ジョブが呼ぶ。`scripts/public-api-check.sh` を変更したら `make test-public-api-check`（合成 JSON と一時 workspace の実物の rustdoc の JSON の回帰テスト）も通す。`make public-name-check` と `make test-public-name-check` は CI の `deny` ジョブが呼ぶ。`scripts/public-name-check.sh` を変更したら `make test-public-name-check`（ダミーの語の合成 JSON と、一時 workspace の実物の rustdoc の JSON の回帰テスト）も通す。`measure/` または `scripts/measure-verify.sh` を変更したら `make measure-verify`（測定専用の別 workspace の fmt / clippy / test / 依存の検査。CI の `deny` ジョブが呼ぶ）も通す。`scripts/hooks/` を変更したら `make test-hooks`（CI の `test-hooks` ジョブ）も通す
- **CI**: ジョブを追加したら `ci.yml` の `ci-complete` の `needs` に必ず追加する
- **EditorConfig**: 生成・編集したファイルは editorconfig-checker を通す（pre-commit で staged を検査）。`*.rs` は 4 スペース、staged の `*.rs` は pre-commit で `rustfmt --check`
- **セキュリティレビュー**: PR 作成前に OWASP Top 10・秘密情報・spec 漏えいを確認（[security](.claude/rules/security.md)）
- **ユーザー承認フロー**: implement-issue は計画承認後に実装。依存追加・Issue 起票・スコープ外対応はユーザー承認を経る

## hooks（settings.json）

| イベント | 内容 |
| -------- | ---- |
| SessionStart | 日本語・委譲・Conventional Commits（`--no-verify` 禁止）・spec 機密・依存固定・unsafe 規約・計画承認のリマインダーを表示 |
| PostToolUse（`Edit\|Write`） | 編集した `*.rs` を `rustfmt` で整形（edition は `Cargo.toml` の `[workspace.package]` から取得、無ければ 2024。`jq` / `rustfmt` / 対象ファイルが無ければ何もしない） |

個人設定は `.claude/settings.local.json`（git 管理外）に置く。
