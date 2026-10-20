# 故障の差し込みの部品（TASK-271）の所在と本リポの現状

TASK-271 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-271。ポインタのみ）。内容・理由・経緯は本書に置かない。
持ち込みの区分は TASK-6 の決定の記録に従う（ポインタのみ）。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-269 | `crates/core/src/error.rs`・`capability.rs` と `crates/core/tests/error_*.rs`・`completion_state_closed.rs` | [contract-error-types](contract-error-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `fault-injection` の宣言 | `crates/contract/Cargo.toml` に `fault-injection = []` だけ（含意なし・ほかの crate への転送なし）。`crates/core` の feature は `test-support` だけ。テスト・診断専用の feature を `core` へ転送する既存の形は `test-support = ["fandhe-silicon-core/test-support"]` | `sed -n '/\[features\]/,/^$/p' crates/contract/Cargo.toml crates/core/Cargo.toml` |
| `fault-injection` を囲む既存の検査 | `crates/contract/tests/feature_default_scope.rs` の `TEST_ONLY`（`default` と本番の組に入らない）、`scripts/upper-layer-check.sh` の `FORBIDDEN_FEATURES`（測定の workspace から有効にさせない）、公開 API の検査の本番の組から除く扱い（[public-api-contract-check](public-api-contract-check.md)）、[contract-feature-check](contract-feature-check.md) | `git grep -n fault-injection -- crates scripts docs ':!docs/spec'` |
| feature の裏の中身 | `fault-injection` の `cfg` で隠したコードは `crates/`・`wasm/` に 0 件 | `git grep -n 'feature = "fault-injection"' -- crates wasm` |
| 共通の型 | TASK-269 のエラー・完了状態・理由の型と、TASK-136 の能力問合せの型は `crates/core` に条件なしで公開済み | [contract-error-types](contract-error-types.md)・[contract-device-types](contract-device-types.md) |
| 公開のトレイト | `crates/`・`wasm/` にある `pub trait` は、`core` の `Capabilities` と、`test-support` の裏の `rerun` の `Check` の 2 件 | `git grep -n -E 'pub trait ' -- crates wasm` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。wasm32 でもビルドされる。`test-support` の裏のモジュールは `parity`・`test_frame`・`rerun` | `crates/core/src/lib.rs`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| `contract` の形 | `#![deny(unsafe_code)]`。`unsafe` は `backend_*` だけに置く | `crates/contract/src/lib.rs`、[unsafe-policy](unsafe-policy.md) |
| workspace | member は `crates/` 配下の 4 つ。`make deny` が `crates/` の外を指す path を拒む | ルート `Cargo.toml`、[workspace-boundary](workspace-boundary.md) |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `core`・`contract` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-271 の言及は本書、`CLAUDE.md`、`README.md` だけ | `git grep -n TASK-271 -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/8 | #4050 | 現状と前提の確認（本書） |
| 2/8 | #4051 | 形の設計 |
| 3/8〜6/8 | #4052〜#4055 | 実装 |
| 7/8 | #4056 | テスト |
| 8/8 | #4057 | 手順書の作成 |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・口・名前・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場と形は TASK-271 の決定の記録に従う（ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ。テスト・診断専用の feature の裏に置き、`default` と本番の組に入れない
- `docs/contract/` の項目ファイルは、対応するコードと公開テストを入れる段で作る（[docs/contract/README.md](contract/README.md)）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

- [contract-error-types](contract-error-types.md)
- [contract-device-types](contract-device-types.md)
- [test-frame](test-frame.md)
- [contract-feature-check](contract-feature-check.md)
- [public-api-contract-check](public-api-contract-check.md)
- [upper-layer-check](upper-layer-check.md)
- [workspace-boundary](workspace-boundary.md)
- [docs/contract/README.md](contract/README.md)
