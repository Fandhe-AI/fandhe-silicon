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

## CPU の経路の f16 の積和（2/4）

### 所在

- `crates/exec/src/cpu.rs` の `f16_fma`、`crates/exec/src/binary16.rs` の `to_f32`・`from_f32`（どちらも `pub(crate)`。公開の面には無い）
- 入口の本体は `grep -n 'crate::cpu::f16_fma\|Outcome::Ran' crates/exec/src/entry.rs`

### 返り値

| 経路と列の形 | 返り値 | 出力 |
| ------------ | ------ | ---- |
| 経路なし | `Ok(Outcome::Unavailable { report })` | 書かない |
| `Route::Cpu` で host の形 | `Ok(Outcome::Ran { report })` | `out` に書く |
| `Route::Cpu` で device の形 | `Err(ContractError::Unsupported { op: "f16_fma", .. })` | buffer に書かない |
| `Route::Native`・`Route::GpuEmulated` | `Err(ContractError::Unsupported { op: "f16_fma", .. })` | 書かない |

`report` は `Exec::query_route` と同じ値。

### 計算の手順

要素ごとに、f16 を f32 へ正確に広げ、`f32::mul_add` で融合積和をとり、f16 へ最近接偶数で 1 回丸めて `out` に書く。
`memory()`・`queue()` を呼ばない（試験用の device の数えが 0）。

### 変換の確かめ

`binary16.rs` の単体試験が、core の `parity::binary16` の 2 関数と、全 65,536 のビット列と丸めの境界で同じ値を返すことを突き合わせる。

### 試験

| ファイル | 試験 | 今の照らし |
| -------- | ---- | ---------- |
| `binary16.rs` | `widening_matches_core_for_all_bit_patterns` | 全ビット列で core と同じ f32 |
| `binary16.rs` | `rounding_matches_core` | 広げた値・前後のビット列・中点・境界の値で core と同じビット列 |
| `cpu.rs` | `hand_computed_cases` | 手計算の 6 組 |
| `cpu.rs` | `single_rounding_after_f32_fma` | 本体 `0x9EB8`、core の参照 `0x9EB9`、順序の差 1 |
| `entry.rs` | `f16_fma_with_cpu_override_runs_on_host` | `Ran`、`Route::Cpu`、`query_route` と同じ報告、`out` が `0x4000` |
| `entry.rs` | `four_entries_with_cpu_override_are_unsupported_err` | 残りの 3 つの入口が `Unsupported` で出力が変わらない |
| `fault_injection_ports.rs` | `forced_op_follows_opt_in` | `f16_fma` と `Route::Cpu` の組は `Ran`・出力が書き換わる。ほかの組は従来どおり |
| `opt_in_default.rs` | `chosen_route_does_not_write_output` | `f16_fma` 以外の 3 つの入口が `Unsupported` で出力が変わらない |
| `opt_in_default.rs` | `cpu_route_writes_f16_output` | `Ran`、`Route::Cpu`、出力が書き換わる、`memory`・`queue` の数えが 0 |
| `route_query.rs` | `query_route_matches_entry_report` | `Ran` の腕で `query_route` と同じ報告、`Route::Cpu`、出力が書き換わる |
| `unsupported_fields.rs` | `unsupported_carries_entry_name_and_device_backend` | `f16_fma` は `allow_gpu_emulated` だけの印で `Unsupported` の欄を照らす |
| `unsupported_fields.rs` | `f16_fma_cpu_route_runs` | `allow_cpu` の印の `f16_fma` が `Ran` |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --lib
cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields
```
