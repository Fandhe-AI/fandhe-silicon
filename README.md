# fandhe-silicon

fandhe-ai・vector-db・fandhe-3d で共用する、独立した低レイヤーの CPU／GPU 基盤
（device・メモリ・実行・同期・能力問合せ・診断）の実装リポジトリです。
Metal・Vulkan・CUDA を背後実装とし、wgpu を依存関係から外せる状態を目指します。

開発基盤は [Fandhe-AI/template-dev](https://github.com/Fandhe-AI/template-dev) から作成しています。
実装言語は Rust（edition 2024・stable）で、現在は workspace と crate の骨格（空の公開面・依存なし）のみです。

## 仕様（`docs/spec`）

仕様・フェーズ文書は private リポジトリ `Fandhe-AI/fandhe-silicon-spec` を `docs/spec` に
submodule として置いています（閲覧には権限が必要です）。

認証の無い環境での clone が失敗しないよう `.gitmodules` で `update = none` を指定しているため、
`git clone --recurse-submodules` や `git submodule update --init` では取得されません。
spec リポジトリへの読み取り権限がある環境で、次のように明示的に取得します。

```bash
git -c submodule.docs/spec.update=checkout submodule update --init docs/spec
```

参照の更新は `update-external.yml` が日次で PR を作成します（後述の「CI」）。

## ライセンス

MIT OR Apache-2.0 のデュアルライセンスです（[LICENSE-MIT](./LICENSE-MIT) / [LICENSE-APACHE](./LICENSE-APACHE)）。

## crate 構成

3 段（下-1・下-2・上）と、3 段で受け渡す型を置く共通 crate の 4 crate 構成です。
公開名・crates.io への公開は未定のため、全 crate を `publish = false` としています。

| パス | crate | 役割 |
|---|---|---|
| `crates/core/` | `fandhe-silicon-core` | 共通の型（device・メモリ・実行・同期・能力問合せ・診断）。外部 crate に依存しない |
| `crates/contract/` | `fandhe-silicon-contract` | 下-1: チップごとの薄い呼び出し。Metal・Vulkan・CUDA の背後実装を `backend-*` feature で持つ |
| `crates/exec/` | `fandhe-silicon-exec` | 下-2: 共通の操作と代わりの実行。`core` のみに依存する |
| `crates/upper/` | `fandhe-silicon-upper` | 上: wgpu 風の層。`#![forbid(unsafe_code)]` |

依存の向きは `contract → core` / `exec → core` / `upper → core, exec` です。
`unsafe` と FFI は `contract` の backend モジュールに閉じ込め、それ以外の crate は `#![forbid(unsafe_code)]` です。

`fandhe-silicon-contract` の feature:

| feature | 既定 | 内容 |
|---|---|---|
| `contract-core`・`cpu-isa`・`cpu-ops` | ✓ | CPU のみの構成 |
| `backend-metal`・`backend-vulkan`・`backend-cuda` | | 各 GPU の背後実装 |
| `graphics`・`ext-interop`・`native-blocking-wait` | | 描画・外部 API との相互運用・ブロッキング待機 |
| `test-support`・`fault-injection`・`artifact-inspection` | | テスト・診断専用（本番ビルドに含めない） |

wgpu 系（`wgpu`・`wgpu-core`・`wgpu-hal`・`wgpu-types`）と `naga` は `deny.toml` で依存グラフへの混入を禁止しています。

## 構成

| パス | 役割 |
|---|---|
| `crates/` | Rust の crate（前述の「crate 構成」） |
| `Cargo.toml`・`Cargo.lock` | workspace 定義（edition・ライセンス・lint を一元管理）とロックファイル |
| `rust-toolchain.toml` | ツールチェーンの単一真実源（stable + rustfmt / clippy） |
| `deny.toml` | cargo-deny の設定（アドバイザリ・ライセンス・取得元・wgpu 系の禁止） |
| `CLAUDE.md`・`.claude/agents/`・`.claude/rules/`・`.claude/settings.json` | Claude Code 用のリポジトリ案内・subagent 定義・規約・hooks |
| `.claude/workflows/` | `implement-issue-tree` の workflow（スキル内スクリプトへの symlink） |
| `.agents/skills/`・`.claude/skills/` | エージェント用スキル（`skills-lock.json` で管理する外部取得物。`.claude/skills/` は `.agents/skills/` への symlink） |
| `Makefile` | `scripts/` を呼ぶだけの薄い入口（`make help` で一覧） |
| `scripts/` | 処理の実体（`help` / `doctor` / `setup` / `check` / `verify` / `deny`）。Make なしでも直接実行できる |
| `scripts/hooks/` | lefthook から呼ばれる Git hooks の実体 |
| `lefthook.yml` | Git hooks 定義（pre-commit / commit-msg） |
| `.editorconfig`・`.editorconfig-checker.json` | 文字コード・改行・インデントの宣言と、その検査の除外設定（ライセンス本文は除外） |
| `.shellcheckrc` | shellcheck が `source` 先（`scripts/lib.sh`）を追って検査するための設定 |
| `.envrc`・`.env.example` | direnv による `.env` の読み込みと、その雛形 |
| `.mcp.json` | Claude Code のプロジェクト共有 MCP サーバー定義（ベースは空） |
| `docs/spec/` | 仕様・フェーズ文書（private の submodule。前述の「仕様」） |
| `.gitmodules` | submodule 定義（`docs/spec` を `update = none` で登録） |
| `LICENSE-MIT`・`LICENSE-APACHE` | ライセンス（前述の「ライセンス」） |
| `.github/workflows/` | CI・AI PR レビュー・外部ソース自動追従（後述の「CI」） |

## 必要なツール

| ツール | 用途 | 必須 |
|---|---|---|
| [GNU Make](https://www.gnu.org/software/make/) 3.81+ | `make` 入口 | ✓ |
| [rustup](https://rustup.rs/) | Rust ツールチェーン（`rust-toolchain.toml` に従い自動導入） | ✓ |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | 依存の監査（`make deny`） | 任意 |
| [lefthook](https://lefthook.dev/) | Git hooks | ✓ |
| [editorconfig-checker](https://github.com/editorconfig-checker/editorconfig-checker) | `.editorconfig` 準拠チェック | ✓ |
| [ShellCheck](https://www.shellcheck.net/) | シェルスクリプトの lint | ✓ |
| [direnv](https://direnv.net/) | `.env` の自動読み込み | 任意 |

macOS（Homebrew）の例:

```bash
brew install lefthook editorconfig-checker shellcheck direnv rustup
rustup-init          # 初回のみ。以降は rust-toolchain.toml の版が自動で使われる
cargo install --locked cargo-deny   # 任意
```

`make doctor` で導入状況を確認できます（読み取りのみで、何も導入・変更しません）。

## セットアップ

```bash
make doctor   # 必要なツールが揃っているか確認する
make setup    # Git hooks を有効化し、.env が無ければ .env.example から作成する
direnv allow  # direnv を使う場合のみ。.envrc の読み込みを許可する
```

`make setup` は何度実行しても安全です（既存の `.env` は上書きしません）。

## コマンド

| コマンド | 直接実行 | 内容 |
|---|---|---|
| `make help` | `scripts/help.sh` | 操作の一覧を表示する |
| `make doctor` | `scripts/doctor.sh` | 開発環境を診断する |
| `make setup` | `scripts/setup.sh` | Git hooks 有効化と `.env` 雛形の配置 |
| `make check` | `scripts/check.sh` | editorconfig-checker + shellcheck（ソースは変更しない） |
| `make verify` | `scripts/verify.sh` | `cargo fmt --check` + `cargo clippy`（`-D warnings`）+ `cargo test`（ソースは変更しない） |
| `make deny` | `scripts/deny.sh` | `cargo deny check`（advisories / bans / licenses / sources） |

整形は `cargo fmt --all` で行います（`make verify` は差分の検出のみ）。

## Git hooks

`make setup` 後、以下が自動実行されます。

- **pre-commit**
  - 簡易シークレット検知（`.env` 系ファイルの追加、トークン形式・秘密鍵・認証情報入り URL・ハードコード値）。
    保守的なヒューリスティックであり、網羅的なスキャナの代替ではありません
  - staged ファイルの editorconfig-checker
  - staged の `*.sh` に対する shellcheck
  - staged の `*.rs` に対する `rustfmt --check`
- **commit-msg**: [Conventional Commits](https://www.conventionalcommits.org/ja/) 形式の検証
  （`<type>[(<scope>)][!]: <要約>`、type は `feat` `fix` `docs` `style` `refactor` `perf` `test` `build` `ci` `chore` `revert`）

hooks に引っかかった場合は原因を修正してから再コミットしてください。`--no-verify` によるバイパスは行いません。

## CI

| ワークフロー | 内容 |
|---|---|
| `ci.yml` | `check`: ローカル・hooks と同じ `make check` を実行する（editorconfig-checker / shellcheck はバージョン固定 + SHA256 検証で導入）。`rust-ci`: Fandhe-AI/actions の `rust-base-ci` reusable workflow で fmt / clippy / test / cargo-deny を実行する（`make verify` / `make deny` と同じコマンド集合）。`pr-title`: PR タイトルを commit-msg フックと同じスクリプトで検証する（squash merge でコミット件名になるため）。`ci-complete`: 全ジョブ結果の集約 |
| `ai-review.yml` | Fandhe-AI/actions の ai-review（codex）による PR 自動レビュー。Actions variable `CODEX_HOME_DIR` が未設定の間は skip される |
| `update-external.yml` | エージェントスキル（`skills-lock.json`）と submodule（`.gitmodules`）の日次自動追従 PR。secrets は org の `SUBMODULE_PAT` を使う（`SKILLS_PAT` 未登録時は共通側が `SUBMODULE_PAT` へフォールバック）。作成する PR には `dependencies` / `automated` ラベルが付く |

- ruleset の required status checks には `ci-complete`（と ai-review の `codex / *`）を登録する（`rust-ci` の結果は `ci-complete` が集約する）。
  ruleset・マージ設定の導入は `setup-repo-guards` スキルの手順に従う
- CI にジョブを追加したら `ci-complete` の `needs` にも必ず追加する

## MCP サーバー

`.mcp.json` はベースとして空（`"mcpServers": {}`）にしてある。派生リポジトリで必要なサーバーを追加する。
API キー等は値を直接書かず `"${EXAMPLE_API_KEY:-}"` のように環境変数参照とし、実値は `.env`（direnv 経由）に置く。

## 環境変数

- 雛形は `.env.example`（キー名と空値・ダミー値のみ）、実値は `.env`（git 管理外）に置きます
- direnv を導入している場合、`.envrc` が `.env` をシェルへ読み込みます
- 個人用の追加設定は `.envrc.local`（git 管理外）に置けます

## 派生リポジトリでの拡張

1. **言語固有のコマンド**: `scripts/` に処理（例: `scripts/verify.sh`）を追加し、`Makefile` に 1 行で呼ぶターゲットと `## ` コメントを足し、`scripts/help.sh` の一覧も更新する（中身の無いターゲット `@true` 等は置かない）
2. **`make check` の拡張**: `scripts/check.sh` に fmt チェック・lint 等を追記する（自動修正は別ターゲットに分ける）
3. **hooks の追加**: `lefthook.yml` に job を追加し、実体が長くなる場合は `scripts/hooks/` へ切り出す
4. **EditorConfig**: 言語ごとのセクション（例: `[*.rs] indent_size = 4`）を `.editorconfig` に追加する
5. **`.gitignore`**: ビルド成果物（`/target`・`/dist`・`node_modules/` 等）を追加する
