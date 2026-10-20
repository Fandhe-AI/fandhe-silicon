# 契約のエラー・完了状態の型（TASK-269）の所在と本リポの現状

TASK-269 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-269。対象の項目は `docs/spec/04-behavior/error-format.md` の ERR-8・ERR-18・ERR-53・ERR-54。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-4 | workspace と 4 crate の骨格（ルート `Cargo.toml` の `members`） | [workspace-boundary](workspace-boundary.md) |
| TASK-22 | `wasm/`（glue の cdylib）と `scripts/wasm-node-check.sh` ほか | [wasm32-node-foundation](wasm32-node-foundation.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-264・TASK-266・TASK-267・TASK-268・TASK-306 | —（本リポに対応する物は無い。private spec 側の記録に従う〔ポインタのみ〕） | — |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 対象の型の有無 | TASK-269 が定義する型は `crates/` にも `wasm/` にも無い。エラーや理由を表す既存の公開 enum は、各機能の内側の暫定・個別の型で、契約の共通の型として置かれたものではない | `git grep -n -E 'pub enum [A-Za-z]*(Error\|Reason)' -- crates wasm` |
| 既存のエラー風の型（実行まわり） | `core` の `execution` に `OperandCheckError`（`#[non_exhaustive]`。rustdoc に暫定の型で契約のエラー型ではないとある。`Display`・`Error` は実装しない） | `crates/core/src/execution/dispatch_check.rs` |
| 既存のエラー風の型（成果物キャッシュ） | `core` の `artifact_cache` に `RejectReason`・`LoadOutcome`（`#[non_exhaustive]`） | `crates/core/src/artifact_cache.rs` |
| 既存のエラー風の型（背後実装） | `contract` の `backend_vulkan::spirv_operands` に `SpirvExtractError`（feature `backend-vulkan` の裏。`#[non_exhaustive]`。`fmt::Display` と `std::error::Error` を実装） | `crates/contract/src/backend_vulkan/spirv_operands.rs` |
| 既存のエラー風の型（試験の道具） | `parity`・`test_frame`・`rerun`（`test-support` の裏）にも個別のエラー型がある。契約の型ではなく検証用の道具の内側 | `git grep -n -E 'pub enum [A-Za-z]*(Error\|Reason)' -- crates/core/src` |
| 拡張性の試験の既存の形 | 別の crate（統合テスト）から `#![deny(unreachable_patterns)]` と `_` の腕で `#[non_exhaustive]` を固定する形がある | `crates/core/tests/execution_type_coverage.rs` |
| `core` の形 | 公開モジュールは `artifact_cache`・`execution`・`hash`（条件なし）と、`test-support` の裏の `parity`・`test_frame`・`rerun`。feature は `test-support` だけ。外部依存 0。`#![forbid(unsafe_code)]` | `crates/core/src/lib.rs`、`crates/core/Cargo.toml` |
| 依存の向き | contract → core、exec → core、upper → core・exec。`wasm/crates/glue` が `contract` に path で依存する | 各 `Cargo.toml`、`wasm/crates/glue/Cargo.toml` |
| wasm32 での `core` | `scripts/build-profiles.sh` が wasm32 向けのビルドを流し、`wasm/` の `make wasm-verify` も `contract` 経由で `core` を wasm32 でビルドする。`core` に足すものは std だけで wasm32 でも通る必要がある | `scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| `exec`・`upper` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec crates/upper` |
| 走る場所 | `core` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-269 の言及は本書、`CLAUDE.md`、`README.md`（と本書への関連のリンク）だけ | `git grep -n TASK-269 -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/8 | #4040 | 現状と前提の確認（本書） |
| 2/8〜3/8 | #4041・#4042 | 実装 |
| 4/8〜7/8 | #4043〜#4046 | テスト |
| 8/8 | #4047 | 確かめ |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・値・フィールド・名前・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場と形は TASK-269 の決定の記録に従う（ポインタのみ）
- `core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[dispatch-operand-check](dispatch-operand-check.md)・[artifact-cache-check](artifact-cache-check.md)・[test-frame](test-frame.md)・[wasm32-node-foundation](wasm32-node-foundation.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
