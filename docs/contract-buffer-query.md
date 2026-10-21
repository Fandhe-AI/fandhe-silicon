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

## 試験（4/6）

`crates/core` の `src/` は変えず、試験ファイルを 1 つ足した。公開 API の変化は無い。

| 確かめること | 内容 |
| ------------ | ---- |
| 全 128 通り | 空を含む 7 値の全組み合わせで、`&dyn MemoryOps` 越しの `allocate` に渡した用途と、返ったハンドルの `usage()` が等しい（`bits()` の一致・7 定数それぞれの `contains` を含む） |
| 大きさ | 同じハンドルの `size()` が `allocate` に渡した `byte_len` と等しい |
| 7 値を 1 つずつ | 失敗時に定数名が分かる形で、`usage()` が渡した定数と等しい |
| 共有参照 | 同じハンドルの `usage()` を繰り返し呼んでも同じ値 |
| 陽性対照 | 渡された用途に 1 ビット足す試験用の実装を、照合が検出する（正しい実装では食い違いが空） |

- 試験用の実装はファイルの中だけにあり、背後実装（Metal / Vulkan / CUDA）には依らない。背後実装の `allocate` が用途をハンドルへ渡すことの確かめは、この試験の範囲の外で、private spec 側の記録に従う（ポインタのみ）
- 対応するテスト: `crates/core/tests/memory_buffer_usage_combinations.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --test memory_buffer_usage_combinations`（`--all-features` 付きでも流す）
- 残る段: 5/6〜6/6

確かめた base は `930f319`、`docs/spec` の gitlink は `0a802d7081610a36991a94b1ef220075d0616a28` を指す。
