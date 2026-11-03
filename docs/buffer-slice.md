# バッファの部分範囲の型

TASK-311 の 1/5〜5/5。private spec の本文は写さない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 物 | 場所 |
| -- | ---- |
| 型と作る口・読む口 | `crates/core/src/memory/slice.rs` |
| 公開のパス | `fandhe_silicon_core::memory::BufferSlice` |
| feature | 条件なし |

## 部分範囲の型と作る口（1/5）

`BufferSlice<'a>` は元の `BufferHandle` を共有で借りる値で、`Debug`・`Clone`・`Copy` を持つ。欄は非公開で、下の 2 つの口からだけ作れる。範囲の検査は `validate_buffer_range`（TASK-309）をそのまま使い、新しいエラーの種類は足さない。

| 口 | 入力 | 返し |
| -- | ---- | ---- |
| `BufferHandle::slice(offset, size)` | 終端 `offset + size` が `size()` 以下（確保長ちょうど・長さ 0 を含む） | `Ok(BufferSlice)` |
| 同上 | 終端が `size()` を超える、または `u64` であふれる | `Err(ContractError::InvalidArgument)`（文言は `validate_buffer_range` のまま） |
| `BufferHandle::as_entire_slice()` | なし | 開始位置 0・大きさ `size()` の `BufferSlice`（失敗しない） |
| `BufferSlice::buffer()` | なし | 範囲を作った元のハンドルへの参照 |
| `BufferSlice::offset()` | なし | 元のバッファの先頭からのバイト数 |
| `BufferSlice::size()` | なし | 範囲のバイト数 |

読む口はどれも `Result` を返さない。

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib memory::slice
cargo test -p fandhe-silicon-core --doc memory::slice
```

## 入れ子の絞りと解放の後の型検査（2/5）

`BufferSlice::slice(offset, size)` は範囲の先頭からの相対で絞り、検査は親の大きさに対して `validate_buffer_range`（TASK-309）で行う。返す範囲も元のハンドルを同じ寿命で借り、`buffer()` は元のハンドルを返す。

| 親の範囲 | 相対の `offset` / `size` | 返し |
| -------- | ------------------------ | ---- |
| `[4, 12)` | 2 / 2 | `Ok`（元の先頭からの開始位置 6・大きさ 2） |
| `[4, 12)` | 2 / 7 | `Err(ContractError::InvalidArgument)`（親の 8 を超える） |
| `[4, 12)` | 8 / 0 | `Ok`（開始位置 12・大きさ 0） |
| `[4, 12)` | 9 / 0 | `Err(ContractError::InvalidArgument)` |
| 3 段の入れ子 | 各段の相対の値 | 開始位置は各段の和 |

元のハンドルを `MemoryOps::free` に渡すときの型検査は doctest で確かめる。

| doctest | 期待 |
| ------- | ---- |
| 範囲が生きている間の `free` | `compile_fail,E0505` |
| `free` の後の `slice` | `compile_fail,E0382` |
| 範囲を使い終えた後の `free` | 通る |

確かめ方:

```sh
cargo test -p fandhe-silicon-core --lib memory::slice
cargo test -p fandhe-silicon-core --doc memory::slice
make doctest-error-code-check
```

## 境界の入力の試験（3/5）

`crates/core/tests/memory_buffer_slice.rs`（feature の条件なし）が、`BufferHandle::slice` の境界の入力を外の crate から公開のパスで確かめる。拒否の文言の字句は比べず、種類（`InvalidArgument`）と `validate_buffer_range`（TASK-309）との `==` で確かめる。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `slice_end_at_allocation_length_is_accepted` | 大きさ 16 で `(0, 16)`・`(8, 8)`・`(16, 0)` | `Ok`。`offset()`・`size()` は渡した値 |
| `slice_past_allocation_length_is_rejected` | 大きさ 16 で `(0, 17)`・`(8, 9)`・`(17, 0)` | `InvalidArgument` |
| `slice_end_overflow_is_rejected` | 大きさ `u64::MAX` で `(u64::MAX, 1)`・`(1, u64::MAX)` | `InvalidArgument` |
| 同上 | 大きさ `u64::MAX` で `(u64::MAX, 0)`・`(0, u64::MAX)` | `Ok` |
| `zero_length_slices` | 大きさ 16 で `(0, 0)`・`(16, 0)`。大きさ 0 で `(0, 0)`・`(1, 0)` | 前 3 つは `Ok` で `size() == 0`。`(1, 0)` は `InvalidArgument` |
| `slice_matches_validate_buffer_range` | 大きさ・`offset`・`size` の全組 | `slice` の結果と `validate_buffer_range` の結果が一致 |
| `slice_accessors_return_inputs` | `slice(4, 8)` | `buffer()` は元のハンドルと同じ参照。`offset()` は 4、`size()` は 8 |
| `slice_rejection_is_invalid_argument_only` | 上の拒否の入力 | `InvalidArgument` だけ |

確かめ方:

```sh
cargo test -p fandhe-silicon-core --test memory_buffer_slice
```

## 入れ子と全体の範囲の試験（4/5）

同じ `crates/core/tests/memory_buffer_slice.rs` が、入れ子の範囲と全体の範囲 `as_entire_slice` を外の crate から公開のパスで確かめる。拒否の文言の字句は比べず、`==`・`matches!` だけで比べる。`validate_buffer_range`（TASK-309）は入れ子の検査の突き合わせにだけ引く。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `nested_slice_is_relative_to_parent` | 大きさ 16 の親 `(4, 8)` から `(2, 2)` | `offset()` は 6、`size()` は 2。`buffer()` は元のハンドルと同じ参照 |
| `nested_slice_is_checked_against_parent_size` | 同じ親から `(2, 7)`・`(8, 0)`・`(9, 0)`・`(u64::MAX, 1)` | `(8, 0)` は `Ok`（`offset()` は 12）。ほかは `InvalidArgument` |
| `nested_slice_matches_validate_buffer_range` | 親 3 通りと `offset`・`size` の全組 | 親の大きさでの `validate_buffer_range` と一致し、開始位置は親の開始位置との和 |
| `multi_level_nesting_offsets_accumulate` | 大きさ 32 から 4 段の入れ子 | 各段の `offset()` は元の先頭からの和、`size()` は渡した値、`buffer()` は元のハンドル |
| `entire_slice_covers_whole_buffer` | 大きさ 0・1・16・`u64::MAX` | `offset()` は 0、`size()` はバッファの大きさ。そこからの絞りは `BufferHandle::slice` と一致 |
| `slices_are_copy_and_coexist` | 同じハンドルの 2 範囲と入れ子、コピー | 同時に持て、値が変わらない |
| `slice_is_repeatable` | 受理と拒否の入力を 2 回ずつ | 同じ結果 |

確かめ方:

```sh
cargo test -p fandhe-silicon-core --test memory_buffer_slice
```
