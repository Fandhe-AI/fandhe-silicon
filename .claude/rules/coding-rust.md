# Rust コーディング規約

## ツールチェーン

- `rust-toolchain.toml`（stable・rustfmt・clippy）を単一真実源とする。edition は 2024（`Cargo.toml` の `[workspace.package]`）
- `make verify`（`cargo fmt --check`・`cargo clippy --workspace --all-targets --all-features -- -D warnings`・`cargo test`）を通してからコミットする
- lint は `[workspace.lints]` で一元管理し、各 crate は `[lints] workspace = true` とする
- 言語固有のコマンドは `scripts/` に実体を置き、`Makefile` からは 1 行で呼ぶ（README「派生リポジトリでの拡張」）

## 設計

- crate 構成（spec D-33）: `core`（共通型）/ `contract`（下-1: チップごとの薄い呼び出し）/ `exec`（下-2: 共通操作・代わりの実行）/ `upper`（上: wgpu 風の層）
- 依存の向きは contract → core / exec → core / upper → core, exec を守る（spec BUILD-54〜58）。core はチップ crate に依存しない
- backend の選択は `contract` の cargo feature（`backend-metal` 等）/ `cfg` で行い、対象外プラットフォームでもビルドが壊れないようにする。プロファイルは feature で切り替え、crate を分けない
- 上位（fandhe-ai・vector-db・fandhe-3d）へは backend 固有型を漏らさない。wgpu を依存から外せる状態を維持する
- 依存は最小限。追加・更新は必ずユーザー承認＋ `=x.y.z` 完全固定（[dependency-policy](./dependency-policy.md)）

## unsafe と FFI

- `unsafe` は `contract` の backend モジュール（FFI 境界）に閉じ込め、公開 API は safe なラッパーとして提供する。`core`・`exec`・`upper` は `#![forbid(unsafe_code)]`
- すべての `unsafe` ブロック・`unsafe fn` に `// SAFETY:`（`unsafe fn` は `/// # Safety`）で、成立させている不変条件と根拠を書く（clippy `undocumented_unsafe_blocks` / `missing_safety_doc` を deny で強制）
- FFI から受け取るポインタ・長さ・ハンドルは null / 範囲 / 生存期間を検証してから使う
- GPU リソース（バッファ・コマンドキュー・イベント等）の所有権と解放責務を型で表現し、二重解放・解放後使用を作らない
- ホスト・デバイス間の同期点を明示し、未同期のメモリ読み書きを safe API から到達可能にしない

## エラーハンドリング

- ライブラリコードでは `Result` を返し、panic させない（`unwrap` / `expect` はテストと不変条件が自明な箇所に限る）
- backend のエラーコード（`MTLCommandBufferStatus`・`VkResult`・`cudaError_t` 等）は握りつぶさず、共通のエラー型へ写像する
- 能力が無い・未対応の場合は暗黙のフォールバックをせず、明示的なエラーまたは能力問合せの結果として返す

## テスト

- 挙動は `docs/spec` のタスク / ビヘイビア ID（ポインタ表記）に対応づけてテストする
- 実機 GPU が必要なテストは feature / `#[ignore]` 理由付きで分離し、CI（GitHub ホステッド）で回る範囲を明示する
- テストの skip・ignore・アサーション弱体化で CI を通さない

## コメント

- [code-comment-style](./code-comment-style.md) に従う（Rust は `///` / `//!` のドキュメンテーションコメント）
