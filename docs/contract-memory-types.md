# バッファ・メモリ操作の公開の型（TASK-308）の所在と本リポの現状

TASK-308 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-308。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-4 | workspace と 4 crate の骨格（ルート `Cargo.toml` の `members`） | [workspace-boundary](workspace-boundary.md) |
| TASK-10 | `scripts/public-api-check.sh`（`make public-api-check`。CI の `deny` ジョブが呼ぶ） | [public-api-wgpu-check](public-api-wgpu-check.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-136 | `crates/core/src/capability.rs` | [contract-device-types](contract-device-types.md) |
| TASK-269 | `crates/core/src/error.rs` | [contract-error-types](contract-error-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| メモリの置き場 | `crates/core/src/memory.rs` はあるが、中身はバッファの引数検査の純粋関数だけ。持ち主は TASK-309（[buffer-range-check](buffer-range-check.md)） | `cat crates/core/src/memory.rs` |
| 足りない型 | バッファの用途・ハンドル・メモリ操作のトレイトに当たる公開の型は `crates/`・`wasm/`・`measure/`・`baseline-wgpu/` に無い | `git grep -n -E 'pub (trait\|struct\|enum) [A-Za-z]*(Buffer\|Memory)' -- crates wasm measure baseline-wgpu`（0 件） |
| 本書の対象の言及 | TASK-308 の言及は本書、`CLAUDE.md`、`README.md` のほかは `crates/core/src/execution/dispatch_check.rs` のポインタ（背後実装へ渡す束縛の型の持ち分）だけ | `git grep -n TASK-308 -- . ':!docs/spec' ':!.agents'` |
| エラー側の型 | `ContractError`（`InvalidArgument` を含む）は `crates/core/src/error.rs` にある。TASK-269 の持ち分 | `grep -n 'InvalidArgument' crates/core/src/error.rs` |
| device 側 | device の識別子・世代を表す専用の型は無い（`DeviceLost` にフィールドがあるだけ）。`limits()` も無い（TASK-157 の持ち分） | `grep -n 'limits' crates/core/src/capability.rs` |
| `contract` の公開面 | 公開モジュールは feature の裏の `backend_*` と `doc(hidden)` の `cfg_gate_doctest` だけ。契約の共通の公開型は無い | `grep -n 'pub mod' crates/contract/src/lib.rs` |
| `compile_fail` の既存の形 | `crates/contract/src/cfg_gate_doctest.rs`・`crates/core/src/parity/tolerance.rs`・`pixel.rs` に前例がある。コメントのとおり stable の rustdoc は失敗の理由を照合しないので、通る例を並べて import の正しさを担保している | `git grep -ln compile_fail -- crates` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。feature は `test-support`・`fault-injection`。wasm32 でもビルドされる | `crates/core/Cargo.toml`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| `exec`・`upper` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec crates/upper` |
| 公開 API の検査 | `scripts/public-api-check.sh` は workspace の lib crate を全部、2 組（production / all-features）で走査する。どの crate に型を足しても対象に入る | `scripts/public-api-check.sh` |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `make verify` は workspace の全 feature で試験を流すので doctest も走る。GPU の実機は要らない | `scripts/verify.sh` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #4283 | 現状と前提の確認（本書） |
| 2/6 | #4284 | 実装 |
| 3/6 | #4285 | テスト |
| 4/6 | #4286 | テスト |
| 5/6 | #4287 | テスト |
| 6/6 | #4288 | 確かめ |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・値・メソッド・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場（`core` か `contract` か）と形は、TASK-308 の決定の記録に従う（関係: TASK-297・TASK-418。ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- `docs/contract/` の項目ファイルは、対応するコードと公開テストを入れる段で作る（[docs/contract/README.md](contract/README.md)）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

- [contract-device-types](contract-device-types.md)
- [contract-error-types](contract-error-types.md)
- [buffer-range-check](buffer-range-check.md)
- [dispatch-operand-check](dispatch-operand-check.md)
- [public-api-wgpu-check](public-api-wgpu-check.md)
- [test-frame](test-frame.md)
- [workspace-boundary](workspace-boundary.md)
- [docs/contract/README.md](contract/README.md)

確かめた base は `eede585`、`docs/spec` の gitlink は `f603cba` を指す。
