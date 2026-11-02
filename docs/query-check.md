# 問合せの結果の配置と問合せの組・解決先の検査（TASK-1877）

private spec の本文は転記しない。以下は本リポのコードと確かめ方のコマンドから書いた事実である。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1877・`04-behavior/` の CMD-127〜CMD-130。ポインタのみ）。

## 所在

- ファイル: `crates/core/src/execution/query_check.rs`
- 公開のパス: `fandhe_silicon_core::execution::{PipelineStatisticsFlags, QUERY_SIZE}`
- feature の条件なし。外部依存 0。`forbid(unsafe_code)` のまま

| 名前 | 値・内容 |
| ---- | -------- |
| `PipelineStatisticsFlags::VERTEX_SHADER_INVOCATIONS` | ビット `1 << 0` |
| `PipelineStatisticsFlags::CLIPPER_INVOCATIONS` | ビット `1 << 1` |
| `PipelineStatisticsFlags::CLIPPER_PRIMITIVES_OUT` | ビット `1 << 2` |
| `PipelineStatisticsFlags::FRAGMENT_SHADER_INVOCATIONS` | ビット `1 << 3` |
| `PipelineStatisticsFlags::COMPUTE_SHADER_INVOCATIONS` | ビット `1 << 4` |
| `QUERY_SIZE` | `8`（問合せの結果 1 つのバイト数） |

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

## 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| doctest（`PipelineStatisticsFlags`） | 例の 2 個の組の `count`・`stride`・`position` |
| `five_constants_are_distinct_single_bits` | 5 定数が互いに違う 1 ビット |
| `empty_has_zero_count_and_stride` | 空の `count`・`stride` が 0 |
| `all_flags_have_count_five_and_stride_forty` | 全部の和の `count` 5・`stride` 40・位置 0〜4 |
| `two_flags_report_count_stride_and_positions` | 例の組の値 |
| `position_is_none_for_empty_multi_bit_or_absent_flag` | `None` の 3 通り |

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib query_check
cargo test -p fandhe-silicon-core --doc PipelineStatisticsFlags
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/5 | #7683 | 実装 |
| 2/5 | #7684 | 実装 |
| 3/5 | #7685 | テスト |
| 4/5 | #7686 | テスト |
| 5/5 | #7687 | 確かめ |
