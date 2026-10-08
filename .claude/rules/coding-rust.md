# Rust コーディング規約

## ツールチェーン

- Rust 導入時に `rust-toolchain.toml`（stable・rustfmt・clippy）を追加し、単一真実源とする
- `cargo fmt`・`cargo clippy --workspace --all-targets -- -D warnings` を通してからコミットする
- `.editorconfig` に `[*.rs] indent_size = 4` を追加し、rustfmt と初期値を一致させる
- 言語固有のコマンドは `scripts/` に実体を置き、`Makefile` からは 1 行で呼ぶ（README「派生リポジトリでの拡張」）

## 設計

- backend 非依存の共通 API（device・メモリ・実行・同期・能力問合せ・診断）と、Metal / Vulkan / CUDA の背後実装を分離する
- backend の選択は cargo feature / `cfg` で行い、対象外プラットフォームでもビルドが壊れないようにする
- 上位（fandhe-ai・vector-db・fandhe-3d）へは backend 固有型を漏らさない。wgpu を依存から外せる状態を維持する
- 依存は最小限。追加・更新は必ずユーザー承認＋ `=x.y.z` 完全固定（[dependency-policy](./dependency-policy.md)）

## unsafe と FFI

- `unsafe` は backend 実装（FFI 境界）に閉じ込め、公開 API は safe なラッパーとして提供する
- すべての `unsafe` ブロック・`unsafe fn` に `// SAFETY:`（`unsafe fn` は `/// # Safety`）で、成立させている不変条件と根拠を書く
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
