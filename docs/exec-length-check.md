# 下-2 の入口の長さの検査の所在（TASK-924）

下-2（`crates/exec`）の 4 つの入口が最初に行う長さの検査の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-924（親の Issue は #868）。

## 本リポの現状の事実

記録時点の commit は `da074ee`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `src/` のファイル | `entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs` | `ls crates/exec/src` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

## 入口の最初の長さの検査（1/4）

### 所在

- 検査の関数: `crates/exec/src/length.rs` の `check_lengths`（`pub(crate)`。公開の面には無い）
- 4 つの入口（`f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add`）の最初の処理からの呼び出し: `grep -n 'check_lengths' crates/exec/src/entry.rs`
- 上限は呼び出しごとに device の `limits().max_buffer_bytes` を読む。device の形（`native-blocking-wait`）も同じ検査を通り、buffer の `size()` は読まない

### 判定の順

どの拒否も `Err(ContractError::InvalidArgument(..))`。

| 段 | 条件 | 返り値 |
| -- | ---- | ------ |
| 0 | `m` が `AtomicAdd` で `None`、または他の操作で `Some` | `Err` |
| 1 | `n` が 0 | `Err` |
| 2 | `AtomicAdd` で `m` が 0 | `Err` |
| 3 | 必要バイト数の `checked_mul` が `u64` であふれる | `Err` |
| 4 | 入力・出力のどちらかの buffer のバイト数が `max_buffer_bytes` を超える（ちょうどは通る。合計ではなく buffer ごと） | `Err`（core の `validate_allocation_size`。`docs/buffer-range-check.md` 参照） |
| 5 | 上のどれにも当たらない | `Ok(())` |

### 操作ごとのバイト数

| 操作 | 入力 | 出力 |
| ---- | ---- | ---- |
| `F16Fma` | n × 2 | n × 2 |
| `SubgroupSumU32`・`SubgroupSumF32` | n × 4 | ⌈n ÷ 256⌉ × 4 |
| `AtomicAdd` | n × 4 | m × 4 |

### 公開の面

`make exec-surface-check` の `record:` の値は `other_fns=5 unlisted_fns=0`（関数が公開の面に増えていない）。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `length::tests::u64_max_length_is_rejected` | 4 操作で n（`AtomicAdd` は m も）が `u64::MAX`、上限 `u64::MAX` と `1 << 28` | すべて `Err` |
| `length::tests::smallest_overflowing_length_is_rejected` | あふれる最小の長さと一つ手前 | 前者は `Err`、後者は `Ok` |
| `length::tests::smallest_length_over_limit_is_rejected` | 上限 64〜67 での上限を超える最小の長さと一つ手前 | 前者は `Err`、後者は `Ok` |
| `length::tests::zero_length_and_zero_outputs_are_rejected` | n = 0、m = 0、`m` の `Some`/`None` の取り違え | すべて `Err` |
| `entry::tests::entries_reject_lengths` | 試験用 device（上限 `1 << 28`）で 4 つの入口に n = 0、m = 0、`u64::MAX`、上限を超える最小の長さ | すべて `Err(InvalidArgument)` |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --lib length
cargo test -p fandhe-silicon-exec --all-features --locked --lib entry::tests::entries_reject_lengths
make exec-surface-check
```
