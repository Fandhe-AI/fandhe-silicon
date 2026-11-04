# 下-2 の CPU の経路の所在（TASK-930）

下-2（`crates/exec`）の `f16_fma` の host の形の列の長さの照合の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-930（親の Issue は #872）。

## 本リポの現状の事実

記録時点の commit は `c368abb`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `src/` のファイル | `denormal.rs`・`denormal_tests.rs`・`entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs` | `ls crates/exec/src` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

## host の列の長さと要素数の照合（1/4）

### 所在

- 照合の関数: `crates/exec/src/length.rs` の `check_host_len`（`pub(crate)`。公開の面には無い。長さの値と要素数だけを受ける）
- `f16_fma` からの呼び出し: `grep -n 'check_host_len' crates/exec/src/entry.rs`

### 判定の順

`f16_fma` は host の形のとき、次の順で判定する。

| 段 | 内容 | 違うときの返り値 |
| -- | ---- | ---------------- |
| 1 | `check_lengths`（要素数 0・バイト数のあふれ・上限超過） | `Err(ContractError::InvalidArgument(..))` |
| 2 | `a`・`b`・`c`・`out` の列の長さがどれも `n` と同じ | `Err(ContractError::InvalidArgument(..))` |
| 3 | 能力の問合せと経路の選び方 | 経路の選び方の結果 |

段 1・2 の拒否では `query_ex`・`memory`・`queue` を呼ばず、出力の列に書かない。
device の形（`native-blocking-wait`）は列の長さを照らさない。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `length.rs` の `host_len_matches_only_the_element_count` | 長さと要素数の組 | 同じ値だけ `Ok`、ほかは `InvalidArgument` |
| `length_host.rs` の `mismatched_host_column_is_rejected` | 4 つの列のどれか 1 つを長さ 3・5 にした呼び出し（上書き × 既定の印 × device の答え） | `InvalidArgument`、`query_ex`・`memory`・`queue` の数えが 0、出力が番兵のまま |
| `length_host.rs` の `matching_host_columns_reach_route_choice` | 4 つの列が長さ `n` の呼び出し | `InvalidArgument` でなく、`query_ex` の数えが 1 |
| `length_device.rs` の `device_form_passes_one_before_byte_overflow` | device の形で `n` が `u64::MAX / 2` | `InvalidArgument` でなく、`query_ex` の数えが 1、確保・読み出しの数えが増えず、buffer の中身が変わらない |

既存の `length_limit.rs` は、`f16_fma` の列を長さ `n.min(64)`（最小 1）で作る。
`n` が `u64::MAX / 2` の呼び出しは、`f16_fma` だけ列の長さが `n` と違うので `InvalidArgument` を照らす。

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --lib length
cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_device
```
