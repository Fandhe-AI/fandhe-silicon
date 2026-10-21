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

## 完了の条件の確かめ（6/6）

6/6（#4298）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-310）。GPU の実機なしで実行し、件数は実測。本体のコードと試験は変えていない。

### 成果物と置き場の対応

| 成果物 | 置き場 |
| ------ | ------ |
| 大きさ・用途の問合せ（`size()`・`usage()`）と `allocate` の契約 | `crates/core/src/memory/buffer.rs`・`ops.rs`（上の 2/6・3/6 の節） |
| 往復の試験 | `crates/core/tests/memory_buffer_query.rs`・`memory_buffer_query_dyn.rs` |
| 用途の組み合わせの網羅の試験 | `crates/core/tests/memory_buffer_usage_combinations.rs`（上の 4/6 の節） |
| 7 値の土台の試験と `free` の doctest | `crates/core/tests/memory_usage_values.rs`・`crates/contract/src/memory_free_doctest.rs`（[contract-memory-types](contract-memory-types.md)） |

### 確かめ

| 項目 | 方法 | 結果 |
| ---- | ---- | ---- |
| 4 本の試験 | `cargo test -p fandhe-silicon-core` に `--test` で 4 本（`memory_buffer_query`・`memory_buffer_query_dyn`・`memory_buffer_usage_combinations`・`memory_usage_values`）を指定 | 4・5・4・6 件が passed、failed・ignored は 0（終了コード 0） |
| 同じ（`--features native-blocking-wait`） | 同上 | 同じ件数で終了コード 0 |
| 同じ（`--all-features`） | 同上 | 同じ件数で終了コード 0 |
| feature なしで走ること | `crates/core/Cargo.toml` に上の 4 本の `[[test]]` 宣言が無い | 0 件 |
| `free` の doctest | `cargo test -p fandhe-silicon-contract --no-default-features --features test-support --doc memory_free_doctest` | 4 件が passed（正常 1・compile_fail 3） |
| 外部依存と unsafe | `core` の `#![forbid(unsafe_code)]`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | forbid あり、外部依存 0 |
| wasm32 | 4 本それぞれ `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --test <名前>` | 終了コード 0 |
| 検査 | `make build-profiles`・`make wasm-verify`・`make public-api-check`・`make public-name-check`・`make public-api-sdk-check` | どれも終了コード 0 |
| 背後実装 | `impl MemoryOps` を `crates`・`wasm`・`measure`・`baseline-wgpu` で検索 | 試験の中の試験用の実装だけで、背後実装は無い |
| base の CI | `ci.yml` の main の push の run | 直前の `930f319` の run（39534529883）は completed・success。`8ac9997` の run（39535613659）は記録時点で in_progress |

- 対象外（本リポにコードが無い、または別タスクの範囲）: 背後実装の `allocate` が値をハンドルへ渡すことの確かめ、用途と操作の対応の検査、`import_memory`。いずれも private spec 側の記録に従う（ポインタのみ）

### 再実行の契機

- `buffer.rs`・`ops.rs` の `size`・`usage`・`__backend_new`・`allocate` の契約を変えたとき
- 上の 4 本の試験か `memory_free_doctest.rs` を変えたとき
- `BufferUsage` の値が増えたとき
- 背後実装の `MemoryOps` が入ったとき
- `docs/spec` の gitlink が進んだとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-310）。確かめた base は `8ac999765331c3006db8daffa9ea9d8d140f3fa3`、`docs/spec` の gitlink は `0a802d7081610a36991a94b1ef220075d0616a28` を指す。
