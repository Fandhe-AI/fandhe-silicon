# 問合せの結果の配置と問合せの組・解決先の検査（TASK-1877）

private spec の本文は転記しない。以下は本リポのコードと確かめ方のコマンドから書いた事実である。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1877・`04-behavior/` の CMD-127〜CMD-130。ポインタのみ）。

## 所在

- ファイル: `crates/core/src/execution/query_check.rs`
- 公開のパス: `fandhe_silicon_core::execution::{PipelineStatisticsFlags, QUERY_SIZE, QUERY_SET_MAX_QUERIES, QUERY_RESOLVE_BUFFER_ALIGNMENT, validate_query_set_count, validate_query_resolve}`
- feature の条件なし。外部依存 0。`forbid(unsafe_code)` のまま

| 名前 | 値・内容 |
| ---- | -------- |
| `PipelineStatisticsFlags::VERTEX_SHADER_INVOCATIONS` | ビット `1 << 0` |
| `PipelineStatisticsFlags::CLIPPER_INVOCATIONS` | ビット `1 << 1` |
| `PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT` | ビット `1 << 2` |
| `PipelineStatisticsFlags::FRAGMENT_SHADER_INVOCATIONS` | ビット `1 << 3` |
| `PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS` | ビット `1 << 4` |
| `QUERY_SIZE` | `8`（問合せの結果 1 つのバイト数） |
| `QUERY_SET_MAX_QUERIES` | `4096`（`u32`。問合せの組の個数の上限） |
| `QUERY_RESOLVE_BUFFER_ALIGNMENT` | `256`（`u64`。解決先の offset が倍数であるべき値） |
| `validate_query_set_count(count: u32) -> Result<(), ContractError>` | 個数が 1〜4096 かの検査 |
| `validate_query_resolve(set_count, stride, first_query, query_count, dst_size, dst_offset) -> Result<u64, ContractError>` | 整列・範囲・長さを照らし、書くバイト数を返す |

外から作れるのは定数と `|` だけで、生のビットから作る口は無い。

## 現状の形（1/5）

`count` は立てたフラグの数、`stride` は `count × QUERY_SIZE`、`position(flag)` は立てたフラグのうち小さいビットから数えた 0 始まりの順。

| 入力 | `count` | `stride` | `position` |
| ---- | ------- | -------- | ---------- |
| 空 | 0 | 0 | 常に `None` |
| 1 個 | 1 | 8 | そのフラグは `Some(0)` |
| `CLIPPER_PRIMITIVES_OUT \| COMPUTE_SHADER_INVOCATIONS` | 2 | 16 | 前者 `Some(0)`・後者 `Some(1)`・含まないフラグ `None` |
| 5 つ全部 | 5 | 40 | 定数の宣言順に `Some(0)`〜`Some(4)` |

`position` の引数が 0 個または 2 個以上のビットを持つとき、また含まれないフラグのときは `None`。

## 現状の形（2/5）

どちらも引数の値だけを見る純粋関数で、拒否はすべて `ContractError::InvalidArgument`。

`validate_query_set_count`

| `count` | 返し |
| ------- | ---- |
| 0 | `Err(InvalidArgument)` |
| 1 | `Ok(())` |
| 4096 | `Ok(())` |
| 4097 | `Err(InvalidArgument)` |
| `u32::MAX` | `Err(InvalidArgument)` |

`validate_query_resolve`（引数は `set_count, stride, first_query, query_count, dst_size, dst_offset` の順）

| 入力 | 返し |
| ---- | ---- |
| `(4096, 16, 0, 2, 512, 256)` | `Ok(32)` |
| `(4096, 16, 0, 2, 288, 256)`（終端が `dst_size` ちょうど） | `Ok(32)` |
| `(4, 8, 2, 2, 8192, 0)`（範囲が `set_count` ちょうど） | `Ok(16)` |
| `(4, 8, 0, 0, 256, 256)`（個数 0） | `Ok(0)` |
| `dst_offset` が 256 の倍数でない（255） | `Err(InvalidArgument)` |
| `stride` が 0 | `Err(InvalidArgument)` |
| `stride` が 8 の倍数でない（12） | `Err(InvalidArgument)` |
| `first_query + query_count` が `set_count` を超える | `Err(InvalidArgument)` |
| `query_count × stride` が `u64` に収まらない | `Err(InvalidArgument)` |
| 書き込みの終端が `u64` に収まらない（offset `u64::MAX - 255`・40 件） | `Err(InvalidArgument)` |
| 書き込みの終端が `dst_size` を超える（287） | `Err(InvalidArgument)` |

## 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| doctest（`PipelineStatisticsFlags`） | 例の 2 個の組の `count`・`stride`・`position` |
| `five_constants_are_distinct_single_bits` | 5 定数が互いに違う 1 ビット |
| `empty_has_zero_count_and_stride` | 空の `count`・`stride` が 0 |
| `all_flags_have_count_five_and_stride_forty` | 全部の和の `count` 5・`stride` 40・位置 0〜4 |
| `two_flags_report_count_stride_and_positions` | 例の組の値 |
| `position_is_none_for_empty_multi_bit_or_absent_flag` | `None` の 3 通り |
| doctest（`validate_query_set_count`・`validate_query_resolve`） | 個数 0・1・4096・4097、`Ok(32)`、offset 255、長さ 287 |
| `set_count_accepts_1_and_4096_rejects_0_and_4097` | 個数の受理と拒否 |
| `resolve_returns_bytes_for_valid_input` | `Ok` の 4 通り |
| `resolve_rejects_each_single_fault` | 単独の誤りごとの拒否と文言の先頭の語 |

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib query_check
cargo test -p fandhe-silicon-core --doc PipelineStatisticsFlags
cargo test -p fandhe-silicon-core --doc validate_query
```

## 結果の配置と個数の上限の試験（3/5）

`crates/core/src/` は変えず、試験のファイル `crates/core/tests/query_layout.rs`（feature 不要）を 1 つ足した。公開 API は変わらない。期待は試験の中で独立に数える。

| テスト | 項目のポインタ | 入力 | 期待 |
| ------ | -------------- | ---- | ---- |
| `layout_all_31_combinations` | CMD-127 | 空でない 31 通りの組 | `count` が立てた数、`stride` が数 × 8、位置が小さいビットからの順で `0..count` を隙間なく埋め、立てていないフラグは `None` |
| `layout_example_two_flags` | CMD-127 | `CLIPPER_PRIMITIVES_OUT` と `COMPUTE_SHADER_INVOCATIONS` の組 | `stride` 16、位置 0・1、バイトの位置 0・8 |
| `layout_query_i_starts_at_offset_plus_i_stride` | CMD-127 | stride 16、offset 256、3 件、`dst_size` 304 | `Ok(48)`、先頭が 256・272・288、最後の終端が 304 |
| `position_rejects_non_single_flag` | CMD-127 | 空・2 ビットの和・含まないフラグ | `None` |
| `query_size_and_constants` | CMD-127〜CMD-130 | 定数 | 8・4096・256 |
| `set_count_bounds` | CMD-128 | 1〜4096 の全件、0、4097 以上の刻み、`u32::MAX` | 1〜4096 は `Ok(())`、ほかは `Err(InvalidArgument)` |
| `empty_flags_have_zero_stride` | CMD-127 | 空の組とその stride の解決 | `count`・`stride` が 0、`Err(InvalidArgument)`（stride の語） |
| `rejection_is_invalid_argument_only` | CMD-128 | 個数 0・4097・`u32::MAX`、空の stride | `InvalidArgument` だけ |

```sh
cargo test -p fandhe-silicon-core --test query_layout
cargo test -p fandhe-silicon-core --all-features
cargo check -p fandhe-silicon-core --tests --target wasm32-unknown-unknown
```

## 解決先の整列・範囲・長さの試験（4/5）

`crates/core/src/` は変えず、試験のファイル `crates/core/tests/query_resolve.rs`（feature 不要）を 1 つ足した。公開 API は変わらない。期待は試験の中で `u128` で独立に数える。

| テスト | 項目のポインタ | 入力 | 期待 |
| ------ | -------------- | ---- | ---- |
| `resolve_offset_alignment` | CMD-130 | `dst_offset` 0・256・512、1・8・255・257、`u64::MAX`、`u64::MAX - 255` | 256 の倍数は `Ok`、ほかは `InvalidArgument`（offset の語）。`u64::MAX - 255` は終端のあふれ・超過を区別して拒否 |
| `resolve_boundaries` | CMD-129 | 長さちょうど・1 バイト不足、範囲ちょうど・1 超過、個数 0、`u32::MAX` | ちょうどは `Ok`、不足・超過は `InvalidArgument`、個数 0 は `Ok(0)`（`first_query` が `set_count` を超えれば拒否） |
| `resolve_grid_matches_reference` | CMD-129・CMD-130 | `set_count` 4 値 × `stride` 5 値 × `first_query`・`query_count` × `dst_offset` × `dst_size` の格子 | 参照の分類どおり。誤りが 1 つなら文言の先頭の語まで、2 つ以上なら種類だけ照らす |
| `resolve_stride_values` | CMD-129 | `stride` 0・7・9・`u64::MAX`、8・40・`u64::MAX - 7` | 8 の倍数でない・0 は `InvalidArgument`（stride の語）、8 の倍数は参照どおり |
| `resolve_is_repeatable` | CMD-129 | `Ok` と各種の拒否の入力 | 同じ引数で 2 回呼んで結果が等しい |

```sh
cargo test -p fandhe-silicon-core --test query_resolve
cargo test -p fandhe-silicon-core --all-features
cargo check -p fandhe-silicon-core --tests --target wasm32-unknown-unknown
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/5 | #7683 | 実装 |
| 2/5 | #7684 | 実装 |
| 3/5 | #7685 | テスト |
| 4/5 | #7686 | テスト |
| 5/5 | #7687 | 確かめ |
