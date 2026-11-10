# Rust コーディング規約

## ツールチェーン

- `rust-toolchain.toml`（stable・rustfmt・clippy）を単一真実源とする。edition は 2024（`Cargo.toml` の `[workspace.package]`）
- `make verify`（`cargo fmt --check`・`cargo clippy --workspace --all-targets --all-features -- -D warnings`・`cargo test`）を通してからコミットする
- lint は `[workspace.lints]` で一元管理し、各 crate は `[lints] workspace = true` とする
- 言語固有のコマンドは `scripts/` に実体を置き、`Makefile` からは 1 行で呼ぶ（README「派生リポジトリでの拡張」）
- ルート `Cargo.toml` の `[profile.dev.package."*"]`（依存クレートの debuginfo を `line-tables-only` にする）を維持する。外せるのはデバッガで依存の変数・型を追う必要が恒常的に出たときだけ。`[profile]` は workspace ルートにだけ書ける（メンバー側は無視される）
- `[profile]` を変えるときは CI キャッシュへの影響を確認する（キーが捕捉せず旧成果物に exact hit し続ける。[github-actions](./github-actions.md)「キーの設計」）
- ローカルの `target/` は古い成果物で肥大化する（worktree ごとにも増える）。`cargo install cargo-sweep` のうえ `cargo sweep --time 14`（14 日以上使われていない成果物を削除）で掃除する

## 設計

- crate 構成（spec D-33）: `core`（共通型）/ `contract`（下-1: チップごとの薄い呼び出し）/ `exec`（下-2: 共通操作・代わりの実行）/ `upper`（上: wgpu 風の層）、加えて wasm32 向けの cdylib `wasm-abi`（依存は contract だけ）
- 依存の向きは contract → core / exec → core / upper → core, exec を守る（spec BUILD-54〜58）。core はチップ crate に依存しない
- backend の選択は `contract` の cargo feature（`backend-metal` 等）/ `cfg` で行い、対象外プラットフォームでもビルドが壊れないようにする。プロファイルは feature で切り替え、crate を分けない
- 上位（fandhe-ai・vector-db・fandhe-3d）へは backend 固有型を漏らさない。wgpu を依存から外せる状態を維持する
- 依存は最小限。追加・更新は必ずユーザー承認＋ `=x.y.z` 完全固定（[dependency-policy](./dependency-policy.md)）

## unsafe と FFI

- `unsafe` は `contract` の backend モジュール（FFI 境界）と、`cpu_ops` の aarch64 の NEON の module（組み込み関数を呼ぶ橋）に閉じ込め、公開 API は safe なラッパーとして提供する。別 workspace の `baseline-wgpu/crates/`・`measure/crates/` の比較の腕の crate（TASK-1008〜1011 のポインタ）は、下位の API（Metal・MPS・wgpu の実験的な機能）を呼ぶ橋に限って `unsafe` を持てる。その crate は workspace の `forbid(unsafe_code)` を継がず、自前の `[lints.rust]`・`[lints.clippy]` で `unsafe_code = "deny"` とし、`unsafe_op_in_unsafe_fn`・`clippy::undocumented_unsafe_blocks`・`clippy::missing_safety_doc` の deny と、その workspace のほかの lints も明記して保ち、`src/` の中の `#[allow(unsafe_code)]` は下位の API を呼ぶ 1 つの module（または関数）にだけ付ける。比較の腕の crate では、実行時に受けたカーネル・シェーダーを走らせる入口を、橋の module の中の `unsafe fn`（`/// # Safety` に呼び出し側の条件）として公開してよい。それを呼ぶ `#[ignore]` 付きの device の試験ファイルに限り、ファイルの先頭に `#![allow(unsafe_code)]` を置ける。置く PR とその範囲を変える PR は [docs/unsafe-policy.md](../../docs/unsafe-policy.md) の小節「承認」の PR ごとの承認を要し、包括の承認の対象にしない。`core`・`exec`・`upper`・`wasm-abi` は `#![forbid(unsafe_code)]`
- すべての `unsafe` ブロック・`unsafe fn` に `// SAFETY:`（`unsafe fn` は `/// # Safety`）で、成立させている不変条件と根拠を書く（clippy `undocumented_unsafe_blocks` / `missing_safety_doc` を deny で強制）
- `unsafe extern` ブロックにも直前に `// SAFETY:` を書き、宣言の署名が外部の関数の C の宣言と合う根拠とリンクの根拠を書く。呼び出しの前提は呼ぶ側のブロックの `// SAFETY:` が担う。lint の対象の外なのでレビューで確かめる（書式は [docs/unsafe-policy.md](../../docs/unsafe-policy.md) の「不変条件の書式」）
- FFI から受け取るポインタ・長さ・ハンドルは null / 範囲 / 生存期間を検証してから使う
- GPU リソース（バッファ・コマンドキュー・イベント等）の所有権と解放責務を型で表現し、二重解放・解放後使用を作らない
- ホスト・デバイス間の同期点を明示し、未同期のメモリ読み書きを safe API から到達可能にしない

## エラーハンドリング

- ライブラリコードでは `Result` を返し、panic させない（`unwrap` / `expect` はテストと不変条件が自明な箇所に限る）
- backend のエラーコード（`MTLCommandBufferStatus`・`VkResult`・`cudaError_t` 等）は握りつぶさず、共通のエラー型へ写像する
- 能力が無い・未対応の場合は暗黙のフォールバックをせず、明示的なエラーまたは能力問合せの結果として返す。ビルド時の `cfg` で版を選び、どのビルドでも同じ口が計算する形（SIMD の命令が無いビルドの版が scalar と同じ計算をするなど）は、暗黙のフォールバックに当たらない

## テスト

- 挙動は `docs/spec` のタスク / ビヘイビア ID（ポインタ表記）に対応づけてテストする
- 実機 GPU が必要なテストは feature / `#[ignore]` 理由付きで分離し、CI（GitHub ホステッド）で回る範囲を明示する
- テストの skip・ignore・アサーション弱体化で CI を通さない

## コメント

- [code-comment-style](./code-comment-style.md) に従う（Rust は `///` / `//!` のドキュメンテーションコメント）
