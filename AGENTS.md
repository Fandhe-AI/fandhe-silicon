# AGENTS.md

## 文書の位置づけ

本リポジトリで作業するすべての AI エージェント・開発者、および ai-review（provider:
codex）による PR 自動レビュー（`.github/workflows/ai-review.yml` wrapper → Fandhe-AI/actions
の ai-review reusable workflow）が共通で用いるレビュー観点集。codex は PR の **base コミット**の
本ファイルを読むため、本ファイルの変更はマージ後の次の PR から有効になる。

`crates/`・`deny.toml`・`make verify` / `make deny` 等の Rust 関連の資産は導入中のものを含む。
本ファイルは観点整理の正であり、個別基準の詳細は一次情報源（`CLAUDE.md`・`.claude/rules/`・
`README.md`）を正とする。内容が食い違う場合は一次情報源を正とし、本ファイルを追随させる。

## 優先度の定義

| 優先度 | 意味 | CI ゲート |
|--------|------|-----------|
| P0 | マージ不可。脆弱性・未定義動作・秘密情報 / private spec の漏えい・ガードレール迂回に直結 | ジョブ失敗 |
| P1 | 修正必須。設計原則・依存規約・CI 規約・運用規約への違反 | ジョブ失敗 |
| P2 | 修正推奨。可読性・保守性・テスト網羅・ドキュメントの改善 | 通過（コメントのみ） |

P0 と P1 はいずれも codex ジョブを失敗させる（マージ前に解消が必要）。P2 は指摘のみで通過する。

## 1. セキュリティ観点

`.claude/rules/security.md`・`.claude/rules/spec-confidentiality.md`・
`.claude/rules/dependency-policy.md` を正とする。

- **private spec の漏えい（P0）**: 本リポは public、仕様・フェーズ文書は private の
  `docs/spec` submodule にある。spec 本文の長文引用・ファイルコピー・非公開の内部判断の転記・
  spec の構成や内容を実質的に復元できる要約を、コード・コメント・ドキュメント・Issue・PR 本文・
  コミットメッセージへ持ち込むことは P0。許可されるのはタスク / ビヘイビア ID・spec 内パス・
  README で公開済みの範囲の 1〜2 行要約などのポインタ表記のみ。PR の base..head の全中間
  コミットメッセージも対象とする
- **シークレットの混入（P0）**: API キー・トークン・パスワード・秘密鍵・`.env` をコード・
  テスト・hooks・`.claude/settings.json`・`.mcp.json`・CI 設定へ含めない
  （`.mcp.json` は `"${VAR:-}"` の環境変数参照とする）
- **メモリ安全性（P0）**: safe API から未定義動作（解放後使用・二重解放・範囲外アクセス・
  データ競合・未同期のデバイスメモリ読み書き）へ到達できる経路は P0。FFI から受け取る
  ポインタ・長さ・ハンドルの検証省略、サイズ・オフセット計算の未検査オーバーフローを含む
- **unsafe の配置と根拠（P0/P1）**: `unsafe` は `crates/contract` の backend モジュール
  （FFI 境界）に限る。`core`・`exec`・`upper` の `#![forbid(unsafe_code)]` の除去・緩和は P0。
  `// SAFETY:`（`unsafe fn` は `/// # Safety`）の欠落・根拠不十分、workspace lints
  （`undocumented_unsafe_blocks`・`missing_safety_doc`・`unsafe_op_in_unsafe_fn`）の緩和は P1
- **依存とサプライチェーン（P0/P1）**: wgpu・wgpu-core・wgpu-hal・wgpu-types・naga の
  （推移的依存を含む）混入、`deny.toml` の `[bans] deny`・`[sources]` の緩和は P0。
  ユーザー承認の記録が無い依存追加・`=x.y.z` 完全固定でないバージョン指定・
  コピーレフトライセンスの許可追加・`[advisories] ignore` への理由なき追加は P1
- **CI の権限・トリガー（P0/P1）**: `pull_request_target` 等の fork PR へ secrets を露出する
  トリガー追加は P0。`permissions` の過剰付与、第三者 action の SHA 固定除去、
  CI で導入するツールのバージョン固定・SHA256 検証の省略は P1

## 2. アーキテクチャ・設計整合の観点

`CLAUDE.md`・`.claude/rules/coding-rust.md`・`README.md` を正とする。

- **crate の責務と依存の向き（P1）**: `core`（共通型）/ `contract`（下-1: チップごとの薄い
  呼び出し）/ `exec`（下-2: 共通操作・代わりの実行）/ `upper`（上: wgpu 風の層）。
  依存は contract → core / exec → core / upper → core, exec のみ。逆向き・循環の依存、
  `core` からチップ crate（Metal / Vulkan / CUDA バインディング）への依存は P1
- **backend 固有型の漏出（P1）**: `upper`・`exec`・`core` の公開面に backend 固有の型・
  ハンドルを出すことは P1。backend の選択は `contract` の feature（`backend-*`）/ `cfg` で行う
- **feature の規約（P1）**: 既定 feature は `contract-core`・`cpu-isa`・`cpu-ops` のみ。
  `backend-*`・`graphics`・`ext-interop`・`native-blocking-wait` を既定に入れる変更、
  テスト・診断専用 feature（`test-support`・`fault-injection`・`artifact-inspection`）を
  本番経路で必須にする変更は P1。対象外プラットフォームでビルドが壊れる変更も P1
- **エラー処理（P1）**: ライブラリコードの `unwrap` / `expect` / panic 経路（テストと
  不変条件が自明な箇所を除く）、backend のエラーコードの握りつぶし、能力が無い場合の
  暗黙のフォールバックは P1。明示的なエラーまたは能力問合せの結果として返す
- **コマンド契約（P1）**: 処理の実体は `scripts/`、`Makefile` は 1 行の呼び出しのみ。
  ローカル（`make check` / `make verify` / `make deny`）・Git hooks・CI が同じコマンドを
  共有する。CI に独自のチェック定義を持ち込む・`Makefile` に処理を直書きする・
  中身の無いターゲット（`@true` 等）を置く・ターゲット変更時に `scripts/help.sh` を
  更新しないことは P1
- **CI 集約（P1）**: ジョブを追加したら `ci.yml` の `ci-complete` の `needs` に追加する。
  `ci-complete` の fail-closed 判定（skipped の許容は条件付きジョブの明示リストのみ）を
  緩める変更は P1
- **テストの弱体化禁止（P1）**: テストの削除・理由なき `#[ignore]`・アサーションの弱体化で
  CI を通すことは P1。実機 GPU が必要なテストは理由付きの feature / `#[ignore]` で分離する

## 3. 再利用・アセット化の観点

本リポは fandhe-ai・vector-db・fandhe-3d が共用する基盤であり、変更は全上位へ波及する。

- **上位から見た API の安定性（P1/P2）**: `upper` の公開 API の破壊的変更は
  Conventional Commits の `!` と `BREAKING CHANGE:` を必須とする（欠落は P1）。
  公開 API の doc comment（`///`）欠落、crate 入口の `//!` 欠落は P2
- **汎用実装の分離（P2）**: backend 非依存のロジックは `core` / `exec` に置き、特定 backend の
  前提を共通層へ持ち込まない。backend 間で重複する処理は共通化を検討する
- **ハードコード回避（P2）**: デバイス名・パス・閾値・上限値のロジックへの直書きを避け、
  能力問合せ・定数・設定へ集約する
- **ドキュメント整備（P2）**: crate・feature・コマンド・依存の変更は `README.md`・
  `CLAUDE.md`・`.claude/rules/` の該当箇所への追随とセットで行う。コメントは
  `.claude/rules/code-comment-style.md` に従い、役割・呼び出し文脈・同期点・所有権を
  ファイル単体で読める形で残す

## 4. リポジトリ固有の観点

- **spec 正本の不可侵（P1）**: `docs/spec`（private submodule）の実体の書き換えは禁止。
  仕様変更は spec リポジトリ側で行う（submodule ポインタの前進自体は通常の更新）
- **外部取得物の直接編集（P1）**: `.agents/skills/`・`.claude/skills/` は `skills-lock.json` で
  管理する外部取得物であり、直接編集しない（更新は update-external の追従 PR か固定版の
  `skills` CLI で行う）
- **コミット・承認フロー（P1/P2）**: 日本語 Conventional Commits（`scripts/hooks/commit-msg-check.sh`
  が許容する type のみ）。`--no-verify` によるフック回避の痕跡は P1。依存の追加・更新、
  スコープ外の変更の混入は P1（別 Issue・別 PR で扱う）

## Fandhe-AI/actions の参照方式（@latest）

2026-10-09・オーナー判断。`Fandhe-AI/actions`（組織内の上流リポジトリ）への `uses:` 参照は、
組織の現行標準として可変タグ `@latest` を使う（兄弟リポジトリ fandhe-db / rust-ai-library と同一。
`latest` は上流の `move-latest-tag.yml` が main への push ごとに付け替える）。本書の
「SHA 固定」に関する観点は**第三者 action にのみ**適用し、`Fandhe-AI/actions` への
`@latest` 参照・SHA pin の除去を指摘しない。
