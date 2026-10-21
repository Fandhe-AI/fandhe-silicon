# バッファの大きさ・用途の問合せ（TASK-310）の所在と本リポの現状

TASK-310 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-310。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。
対象の項目は `04-behavior/` の MEM-3・MEM-44・MEM-45 のポインタだけを書く。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-297 | 本リポに物は無い。private spec 側の記録に従う（ポインタのみ） | なし |
| TASK-308 | `crates/core/src/memory/buffer.rs`（`BufferUsage`・`BufferHandle`）・`crates/core/src/memory/ops.rs`（`MemoryOps`） | [contract-memory-types](contract-memory-types.md) |
| TASK-309 | `crates/core/src/memory/buffer_check.rs`（`validate_buffer_range`・`validate_allocation_size`） | [buffer-range-check](buffer-range-check.md) |

## 本リポの現状（着手前の 1/6 の時点の記録）

本節の表は、着手前（1/6）の時点の事実の記録で、その後の段の実装で変わった（例: `BufferHandle` は大きさ・用途を持ち `size`・`usage` を返す）。表の値は書き換えず残す。いまの実態は [contract-buffer-query](contract-buffer-query.md) と `crates/core/src/memory/buffer.rs`・`crates/core/tests/` の実物で確かめる。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `BufferHandle` の公開面 | フィールドは非公開の 3 つ（世代・識別子・device の識別子）。公開の関数は `doc(hidden)` の `__backend_new`・`__backend_generation`・`__backend_id`・`__backend_device_id` の 4 つだけ。derive は `Debug` のみ。大きさ・用途を返す公開の口は無い | `grep -n 'pub fn\|derive' crates/core/src/memory/buffer.rs` |
| `BufferUsage` の公開面 | 定数 7 つ、`empty`・`is_empty`・`bits`・`contains`、`BitOr`・`BitOrAssign`。derive は `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash` | 同上 |
| `MemoryOps::allocate` | `allocate(&self, byte_len: u64, usage: BufferUsage) -> Result<BufferHandle, ContractError>` | `grep -n 'fn allocate' crates/core/src/memory/ops.rs` |
| `__backend_new` の呼び出し元 | `crates/core/tests/` の `memory_types.rs`（2 か所）・`memory_pending_release.rs`（2 か所）・`memory_usage_values.rs`（1 か所）だけ | `git grep -n __backend_new -- crates wasm measure baseline-wgpu` |
| `MemoryOps` の実装 | 実装は上の試験ファイルの中だけ（4 つ）。`crates/contract`・`exec`・`upper`・`wasm-abi`・`wasm/`・`measure/`・`baseline-wgpu/` に `impl MemoryOps` は無い。`contract` の参照は `doc(hidden)` の `memory_free_doctest.rs` だけ | `git grep -n 'impl MemoryOps' -- crates wasm measure baseline-wgpu`・`git grep -n -E 'BufferUsage\|BufferHandle\|MemoryOps' -- crates/contract crates/exec crates/upper crates/wasm-abi wasm measure baseline-wgpu` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。feature は `test-support`・`fault-injection`・`native-blocking-wait`。wasm32 でもビルドされる | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| `docs/contract/` | 項目ファイルは無い（`README.md`・`TEMPLATE.md`・`procedure.md` だけ） | `ls docs/contract` |
| 公開 API の検査 | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check` は workspace の lib crate を全部走査する。`core` に口を足せば対象に入る | `scripts/public-api-check.sh`、`scripts/public-name-check.sh` |
| 走る場所 | `make verify` は全 feature で試験と doctest を流す。GPU の実機は要らない | `scripts/verify.sh` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #4293 | 現状と前提の確認（本書） |
| 2/6 | #4294 | 実装 |
| 3/6 | #4295 | 実装 |
| 4/6 | #4296 | テスト |
| 5/6 | #4297 | 方針の整理 |
| 6/6 | #4298 | 確かめ（[contract-buffer-query](contract-buffer-query.md) の「完了の条件の確かめ（6/6）」） |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（置き場・名前・返す値・ハンドルの中身）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 形は TASK-310 の決定の記録に従う（関係: TASK-297・TASK-308。ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- 公開する名前は `make public-name-check` を通す
- 範囲の外は private spec 側の記録に従う（ポインタのみ。MEM-109、TASK-1779・TASK-1796・TASK-314）
- `docs/contract/` の項目ファイルは、対応するコードと公開テストを入れる段で作る（[docs/contract/README.md](contract/README.md)）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

- [contract-memory-types](contract-memory-types.md)
- [buffer-range-check](buffer-range-check.md)
- [public-api-wgpu-check](public-api-wgpu-check.md)
- [public-name-check](public-name-check.md)
- [test-frame](test-frame.md)
- [workspace-boundary](workspace-boundary.md)
- [docs/contract/README.md](contract/README.md)

1/6 の時点で確かめた base は `e1623ee`、`docs/spec` の gitlink は `0a802d7` を指していた（いまの gitlink は上の `git ls-tree origin/main docs/spec` で確かめる）。
