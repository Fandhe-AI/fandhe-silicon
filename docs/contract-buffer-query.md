# バッファの大きさ・用途の問合せ（TASK-310）の所在と本リポの現状

TASK-310 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-310・TASK-297。ポインタのみ）。内容・理由・経緯は本書に置かない。
前提の型は [contract-memory-types](contract-memory-types.md)（TASK-308）に置いてある。

## 現状の形（2/6）

置き場は `crates/core/src/memory/buffer.rs`（外部依存 0・`forbid(unsafe_code)` のまま）。

| 項目 | 事実 |
| ---- | ---- |
| `BufferHandle` のフィールド | 非公開の 5 つ（世代・識別子・device の識別子・確保した大きさ・用途） |
| `__backend_new` | `(generation, id, device_id, byte_len, usage)` の 5 引数。`byte_len`・`usage` は `allocate` に渡された値を渡す |
| `size(&self) -> u64` | `__backend_new` に渡した大きさをそのまま返す（MEM-44 のポインタ） |
| `usage(&self) -> BufferUsage` | `__backend_new` に渡した用途をそのまま返す（MEM-45 のポインタ） |

- どちらも `Result` を返さず、背後実装を呼ばず、panic の経路も無い。`size()` は範囲の検査の正にしない
- 対応するテスト: `crates/core/tests/memory_buffer_query.rs`（大きさの往復・用途の往復・ほかのフィールドが変わらないこと・共有参照越しの呼び出し）。既存の `memory_types.rs`・`memory_usage_values.rs`・`memory_pending_release.rs` は 5 引数に追従
- 確かめ方: `cargo test -p fandhe-silicon-core --test memory_buffer_query`（`--all-features` 付きでも流す）

## 現状の形（3/6）

2/6 から変えていないもの: `BufferHandle` の形と `__backend_new` の 5 引数。足したのは `MemoryOps::allocate` の rustdoc の契約と、そのテストだけ。

| 項目 | 事実 |
| ---- | ---- |
| `MemoryOps::allocate` の契約 | 返すハンドルは受けた `byte_len`・`usage` を `__backend_new` にそのまま渡して作る。呼び出し側は `size()`・`usage()` で同じ値を読める。`size()` は範囲の検査の正にしない |

- 対応するテスト: `crates/core/tests/memory_buffer_query_dyn.rs`
    - `allocate_through_dyn_returns_given_size`: `&dyn MemoryOps` 越しの `allocate` の大きさが `size()` で読める
    - `allocate_through_dyn_returns_given_usage`: 同じく用途が `usage()` で読める
    - `allocate_through_generic_matches_dyn`: ジェネリクス越しでも同じ値になる
    - `size_usage_survive_until_free`: `upload` の後も値が変わらず、`free` に値で渡せる
    - `mismatch_detects_wrong_pass_through`: 渡す値をずらした実装を照合の関数が検出する（陽性対照）
- 確かめ方: `cargo test -p fandhe-silicon-core --test memory_buffer_query_dyn`（`--all-features` 付きでも流す）
- 残る段: 4/6〜6/6（5/6 は下の節）

## 方針の整理（5/6）

この段でコード・公開 API は変えていない。範囲の正は private spec 側の記録にある（`docs/spec/05-tasks.md` の TASK-310・TASK-297。ポインタのみ）。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 用途を読んで操作を拒む処理 | 製品の `src/` に無い（`.usage()`・`.size()` の呼び出し元は試験だけ） | `git grep -n -E '\.usage\(\)\|\.size\(\)' -- crates/core/src crates/contract/src crates/exec/src crates/upper/src` |
| 背後実装の `impl MemoryOps` | `crates/contract`・`exec`・`upper` に無い | `git grep -n 'impl MemoryOps' -- crates/contract crates/exec crates/upper` |
| `import_memory` | 本リポに無い | `git grep -n import_memory -- crates wasm measure` |
| `docs/contract/` の項目ファイル | 無い（`README.md`・`TEMPLATE.md` だけ） | `ls docs/contract` |

方針:

- この TASK が `core` で受け持つのは、`size`・`usage` の 2 つの関数、`allocate` の rustdoc の契約、試験用の実装による試験だけ
- 用途と操作の対応の検査は、この TASK の段では足さない。扱いは private spec 側の記録に従う（ポインタのみ。MEM-109、TASK-1779・TASK-1796）
- 背後実装の `allocate` が `byte_len`・`usage` をハンドルへ渡すことの確かめは、この TASK の試験の範囲の外。扱いは private spec 側の記録に従う（ポインタのみ。TASK-314・TASK-327・TASK-340）。この TASK の試験は、試験用の実装で契約を確かめるところまで
- `docs/contract/` の項目ファイルは、この TASK の段では作らない。背後実装ごとの提供・非提供を裏づける公開テストが本リポに無く、[docs/contract/README.md](contract/README.md) の「テストの無い約束は書かない」に当たる。作る時期は private spec 側の記録に従う（ポインタのみ）
- 公開する名前の `size`・`usage` は `make public-name-check` の対象に入っていて、通っている

6/6 で完了の条件（MEM-3・MEM-44・MEM-45。ポインタのみ）を確かめるときに流すもの。合否の判定は 6/6 で行い、この段では書かない。

| 試験ファイル | 指す項目 |
| ------------ | -------- |
| `crates/core/tests/memory_buffer_query.rs` | MEM-44・MEM-45 |
| `crates/core/tests/memory_buffer_query_dyn.rs` | MEM-44・MEM-45 |

- コマンド: `cargo test -p fandhe-silicon-core --test memory_buffer_query --test memory_buffer_query_dyn --all-features` と `make verify`
- 残る段: 6/6
- 確かめた base は `930f319`、`docs/spec` の gitlink は `0a802d7` を指す。
