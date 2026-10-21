# スタブの実装の契約テスト（TASK-137）の所在と本リポの現状

TASK-137 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-137。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 確かめ方 | 文書 |
| ---------------- | ---------- | -------- | ---- |
| TASK-5 | `crates/contract/Cargo.toml` の `[features]`、`crates/contract/src/lib.rs` の `cfg`、`scripts/build-profiles.sh` | 親の Issue の状態が CLOSED（`gh issue view 30 --json state`） | [contract-feature-check](contract-feature-check.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | 同上（`gh issue view 91 --json state`） | [test-frame](test-frame.md) |
| TASK-136 | `crates/core/src/device.rs`・`capability.rs` | 同上（`gh issue view 411 --json state`） | [contract-device-types](contract-device-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 既にある型 | device の要求・能力問合せの型は `crates/core` にある。定義と確かめは [contract-device-types](contract-device-types.md) を参照し、ここでは繰り返さない | `crates/core/src/device.rs`、`crates/core/src/capability.rs` |
| 公開のトレイト | `crates/`・`wasm/` の `pub trait` は 4 件だけ（`Capabilities`、`FaultHooks`〔`fault-injection` の裏〕、`MemoryOps`〔`crates/core/src/memory/ops.rs`〕、`Check`〔`test-support` の裏〕） | `git grep -nE 'pub trait' -- crates wasm` |
| device まわりの口 | device のトレイト、device を返す入口、投入の列のトレイト、compute の口、試験用のスタブの device に当たる物は `crates/`・`wasm/` に無い（`pub trait` は上の 4 件だけ）。メモリの操作のトレイト `MemoryOps` とバッファ型（`BufferHandle`・`BufferUsage`）は `crates/core/src/memory/` に既にある。名前に `Device` を含む型は要求と生ハンドルの型、`Stub` を含む型は故障の差し込み用の `FaultStub` だけ。`crates/core/src/memory.rs` はそれらの再公開と引数検査の純粋関数（`validate_allocation_size`・`validate_buffer_range`）を持つ | `git grep -nE 'pub (trait\|struct) [A-Za-z]*(Device\|Memory\|Queue\|Stub)' -- crates wasm` |
| 故障の差し込み口 | `FaultHooks` はある。それを返す公開のアクセサを持つ device のトレイトは無い（`crates/core/tests/fault_parts_composition.rs` の同名メソッドは試験内の局所トレイト） | `crates/core/src/fault.rs`、`crates/core/tests/fault_parts_composition.rs` |
| feature の裏のアイテム | `graphics`・`artifact-inspection` は `Cargo.toml` に宣言済みだが、これらで切られたアイテムは 0 件 | `git grep -n 'feature = "graphics"' -- crates`、`git grep -n 'feature = "artifact-inspection"' -- crates` |
| 組ごとのビルド検査 | `scripts/build-profiles.sh` の `NATIVE_SETS`（`graphics` を含む組と含まない組がある）と `WASM_SETS`。`make build-profiles`（CI の `verify` ジョブ） | `grep -n 'NATIVE_SETS\|WASM_SETS' scripts/build-profiles.sh` |
| コンパイル失敗の既存の仕組み | feature の有無でコンパイルが落ちることを確かめる枠が `crates/contract/src/cfg_gate_doctest.rs` にある | [feature-gate-compile-check](feature-gate-compile-check.md) |
| 走る場所 | `make verify`（`--all-features`）と `make build-profiles`。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |
| 依存の承認 | 外部 crate は足さない。`deps-approved.toml` の記録の範囲に収める | `grep -c '^\[\[' deps-approved.toml` |
| 本書の対象の言及 | TASK-137 の言及は本書、`CLAUDE.md`、`README.md` だけ | `git grep -nE 'TASK-137([^0-9]\|$)' -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/6 | #3659 | 現状と前提の確認（本書） |
| 2/6 | #3660 | 形の設計 |
| 3/6 | #3661 | 実装 |
| 4/6 | #3662 | テスト |
| 5/6 | #3663 | CI への組み込み |
| 6/6 | #3664 | 確かめ |

## 本リポでの反映の方針

本書には現状の事実だけを置く。具体の形（型・名前・置き場・試験の名前）は受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す。
置き場と形は TASK-137 の記録に従う（ポインタのみ）。`core` に足す場合は外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ。

守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)。

## 関連

[contract-device-types](contract-device-types.md)・[contract-feature-check](contract-feature-check.md)・[feature-gate-compile-check](feature-gate-compile-check.md)・[contract-fault-injection](contract-fault-injection.md)・[test-frame](test-frame.md)・[docs/contract/README.md](contract/README.md)
