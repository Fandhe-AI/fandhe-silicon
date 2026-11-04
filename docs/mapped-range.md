# 書き込み用のビューの型

TASK-312 の 1/4〜4/4。private spec の本文は写さない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 物 | 場所 |
| -- | ---- |
| 型と長さ・空の判定 | `crates/core/src/memory/mapped_range.rs` |
| 公開のパス | `fandhe_silicon_core::memory::MappedRangeMut` |
| feature | 条件なし |

## 書き込み用のビューの型と長さ・空の判定（1/4）

作る口は背後実装が使う `#[doc(hidden)]` の口で、借りたバイト列をそのまま包む（検査も変換もしない）。

| 口 | 入力 | 返し |
| -- | ---- | ---- |
| `len` | なし | ビューのバイト数（`u64`）。失敗しない |
| `is_empty` | なし | 長さが 0 のとき真 |
| `Debug` | なし | `MappedRangeMut { len: N }`（長さだけ） |

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib memory::mapped_range
cargo test -p fandhe-silicon-core --doc mapped_range
```

## 範囲の絞り（2/4）

`slice(offset, len)` はビューの先頭からの相対で範囲を絞り、元のビューを排他で借りる。
検査は親の長さに対して `validate_buffer_range`（TASK-309）で行い、`Err` はその値のまま返す。

| 入力（16 バイトのビュー） | 返し |
| ------------------------ | ---- |
| `slice(0, 16)` | `Ok`（長さ 16） |
| `slice(16, 0)` | `Ok`（空） |
| `slice(8, 9)`・`slice(17, 0)` | `Err(ContractError::InvalidArgument)` |
| `slice(u64::MAX, 1)` | `Err(ContractError::InvalidArgument)` |
| 絞ったビューをさらに絞る | 絞ったビューの先頭からの相対で、その長さに照らす |

| doctest | 結果 |
| ------- | ---- |
| 絞ったビューが生きている間に同じビューから次を絞る | `compile_fail,E0499` |
| 使い切ってから次を絞る | 通る |

```sh
cargo test -p fandhe-silicon-core --lib memory::mapped_range
RUSTC_BOOTSTRAP=1 cargo test -p fandhe-silicon-core --doc memory::mapped_range
make doctest-error-code-check
```

## 境界の入力と絞りの試験（3/4）

`crates/core/tests/memory_mapped_range.rs`（feature の条件なし）が、公開のパスだけで境界の入力を確かめる。
拒否の文言は比べず、`InvalidArgument` と `validate_buffer_range`（TASK-309）との `==` で比べる。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `empty_view_len_and_is_empty` | 長さ 0 の配列と空の `Vec` | `len() == 0`・`is_empty()`。`slice(0, 0)` は `Ok`、`slice(0, 1)`・`slice(1, 0)` は `InvalidArgument` |
| `len_and_is_empty_follow_backing_length` | 長さ 1・3・4・8・255・4096 | `len()` が長さ、`is_empty()` が偽 |
| `slice_up_to_exact_length_is_accepted` | 長さ n で `slice(0, n)`・`slice(k, n - k)`・`slice(n, 0)` | 長さが n・n−k・0 |
| `slice_beyond_length_is_rejected` | `slice(0, n + 1)`・`slice(n, 1)`・`slice(n + 1, 0)`・`slice(1, n)` | `InvalidArgument` |
| `slice_overflow_is_rejected` | `u64::MAX` を含む offset・len | `InvalidArgument` |
| `slice_matches_validate_buffer_range` | 長さ 0・1・7・16 と offset・len の格子 | `validate_buffer_range` と同じ結果 |
| `nested_slice_is_checked_against_parent` | 絞ったビューの絞り、4 段の入れ子 | 各段が直前の段の長さに照らされる |
| `parent_len_is_unchanged_after_slice` | 絞った後・拒否された後 | 親の `len()`・`is_empty()` が変わらない |
| `rejection_is_repeatable` | 同じ引数を 2 回 | 拒否は同じ `Err`、受理は同じ長さ |
| `rejection_is_invalid_argument_only` | 拒否の入力の全部 | `InvalidArgument` だけ |

```sh
cargo test -p fandhe-silicon-core --test memory_mapped_range
```

## 完了の条件の確かめ（4/4）

本リポで観測できる事実だけを記録する（ポインタ: `docs/spec/05-tasks.md` の TASK-312）。GPU の機材なしの Linux で実行し、件数は実測。コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 型が共通の crate に公開されている | `grep -n 'mod mapped_range\|pub use mapped_range' crates/core/src/memory.rs`、`cargo check -p fandhe-silicon-core`（feature なし） | 14 行目 `mod mapped_range;`、22 行目 `pub use mapped_range::MappedRangeMut;`。check の終了コード 0 |
| (2) | 長さ | `cargo test -p fandhe-silicon-core --lib memory::mapped_range`、`cargo test -p fandhe-silicon-core --test memory_mapped_range` | `zero_length_array_is_empty`・`zero_length_vec_is_empty`・`len_follows_backing_length`・`len_and_is_empty_follow_backing_length`・`empty_view_len_and_is_empty` が `ok` |
| (3) | 空の判定 | (2) と同じ試験 | (2) の 5 件が `ok` |
| (4) | 長さ 0 のビュー・長さちょうど・長さ超過・`u64` のあふれ | 同上 | `slice_boundaries`・`slice_of_empty_view`・`slice_up_to_exact_length_is_accepted`・`slice_beyond_length_is_rejected`・`slice_overflow_is_rejected`・`empty_view_len_and_is_empty` が `ok` |
| (5) | 絞った範囲が元のビューの範囲に収まる | 同上 | `slice_stays_inside_parent`・`nested_slice_is_checked_against_parent` が `ok` |
| (6) | 範囲の外で panic せず `InvalidArgument` | 同上 | `rejection_is_invalid_argument_only`・`slice_matches_validate_buffer_range`（`validate_buffer_range` は TASK-309）が `ok` |
| (7) | 元のビューを排他で借りる | `RUSTC_BOOTSTRAP=1 cargo test -p fandhe-silicon-core --doc memory::mapped_range`、`make doctest-error-code-check` | 通る doctest 4 件 `ok`、`compile_fail,E0499` の 1 件 `ok`。`make doctest-error-code-check` の終了コード 0 |
| (8) | 背後実装を呼ばない | `grep -n '^use ' crates/core/src/memory/mapped_range.rs`、3 つのコミット（`460a18e`・`377a723`・`41c21b1`）それぞれで `git show --stat --format= <sha> -- crates/contract crates/core/src/stub_device.rs crates/core/src/memory/ops.rs crates/core/src/memory/buffer.rs` | `use` は `core::fmt`・`core::mem`・`super::`・`crate::error` の 4 行。3 コミットとも差分 0 行 |
| (9) | 依存 0・unsafe 0 | 同じ 3 コミットで `git show --stat --format= <sha> -- Cargo.lock deps-approved.toml crates/core/Cargo.toml`、`grep -c unsafe crates/core/src/memory/mapped_range.rs`、`grep -n forbid crates/core/src/lib.rs` | 3 コミットとも差分 0 行。`unsafe` は 0 件。39 行目 `#![forbid(unsafe_code)]` |
| (10) | CI で流れる場所と件数 | `make verify`、`make wasm-verify`、`make doctest-error-code-check` | `make verify` の終了コード 0（`--lib memory::mapped_range` 6 passed、`--test memory_mapped_range` 10 passed を含む）。`make wasm-verify` の終了コード 0。`make doctest-error-code-check` の終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-312）。確かめた base は `41c21b146cab8e0f36e2230ad2c9329affc364f2`。
