# 下-2 の試験用の口の所在（TASK-921）

下-2（`crates/exec`）で、試験から使う口の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-921（親の Issue は #865）。

## 本リポの現状の事実

記録時点の commit は `de50250`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `[features]` | 記録時点は `native-blocking-wait` の 1 つ | `git show de50250:crates/exec/Cargo.toml` |
| `UnavailableReason` の値の数 | 5 | `git show de50250:crates/exec/src/route.rs` |
| 経路の選び方の所在 | `crates/exec/src/select.rs` の `select_route` | `grep -n 'fn select_route' crates/exec/src/select.rs` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

## 試験用の feature と `Native` を外す口（2/6）

### 所在

- `crates/exec/Cargo.toml` の feature `fault-injection`（core の同名 feature へ転送。既定では無効）
- `Exec::force_unsupported`（`crates/exec/src/entry.rs`）
- `UnavailableReason::ForcedUnsupported`（`crates/exec/src/route.rs`）
- 操作ごとの印を持つ private な module `crates/exec/src/hooks.rs`

どれも feature `fault-injection` の下だけにあり、feature を付けないビルドの公開の面と動きは変わらない。
有効にするには `--features fault-injection` を付けるか、依存の宣言で `features = ["fault-injection"]` を指定する。

### `Exec::force_unsupported` の動き

| 場面 | 動き |
| ---- | ---- |
| 指定した操作 | 能力の段階を見ずに `Native` を候補に入れない。`native_reason` は `ForcedUnsupported` |
| 指定していない操作 | 変わらない |
| `GpuEmulated`・`Cpu` | 効く印が許すときだけ、その順に候補に入る |
| 選べないとき | `route` は `None`、`fallback_reason` は今までどおり（印が無ければ `OptInNotGiven`、あれば `F32DenormalBehaviorUnknown`） |
| 判定を求める操作 | 届いた経路にだけ判定を求める（`Native` には求めない） |
| 3 つの入口 | 変わらない（能力を問い合わせず `Err(Unsupported { op: <関数名>, .. })`） |
| 既定の印 | 変わらない |
| 繰り返し | 何度呼んでも指定は足し合わさる。呼んだ値の `Exec` だけに効く |

### 報告の欄の読み方

- `native_reason`: 指定した操作では `Some(ForcedUnsupported)`
- `observed_level`: `query_ex` の実際の値のまま。`query_ex` は今までどおり 1 回呼ぶ

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `select::forced_tests::forced_never_chooses_native` | 能力の値 11 通り・印 4 通り・4 操作・判定の答え（判定を求める 2 操作は 64 組）の 5720 件 | 経路が `Native` でなく、`native_reason` が `ForcedUnsupported`、判定が `Native` で呼ばれない。ほかの欄は段階を `Unsupported` にした指定なしの出力と等しい |
| `route::forced_tests::forced_reason_is_distinct` | `ForcedUnsupported` とほかの理由 | どれとも等しくない |
| `entry::forced_tests::forced_entry_reports_actual_level` | 素の `StubDevice`・`f16_fma` を指定 | `Ok(Outcome::Unavailable)`・`native_reason` が `ForcedUnsupported`・`observed_level` が `Unsupported`。指定していない操作は変わらず、`query_route` と等しい |
| `entry::forced_tests::force_does_not_touch_three_entries` | 4 操作すべてを指定・印 5 通り | 3 つの入口が `Err(Unsupported { op: <関数名>, .. })` |
| `entry::forced_tests::force_keeps_default_opt_in` | 印 4 通り | `force_unsupported` の前後で `default_opt_in()` が同じ |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --features fault-injection --lib
```

## f32 の非正規化数の判定を差し込む口（3/6）

### 所在

- `DenormalInjection`（`Preserve`・`FlushToZero`・`Unknown`。`crates/exec/src/report.rs`。crate の根で公開）
- `Exec::inject_denormal`（`crates/exec/src/entry.rs`）
- `F32DenormalBasis::Injected`（`crates/exec/src/report.rs`）
- 差し込みを (操作, 経路) ごとに持つ `crates/exec/src/hooks.rs`

どれも feature `fault-injection` の下だけにある。

### `Exec::inject_denormal` の動き

| 場面 | 動き |
| ---- | ---- |
| 差し込みのある (操作, 経路) | 経路の選び方が求める判定がその値になる（`Ok`）。その値で経路が決まった報告の `f32_denormal` は `{ behavior, basis: Injected, documented: None }` |
| 差し込みの無い組 | 呼び手の判定の関数をそのまま呼ぶ（入口は入口の関数名、`query_route` は `"query_route"` の `Err(Unsupported)`） |
| 同じ組で呼び直す | 後の値が効く |
| `F16Fma`・`SubgroupSumU32` | 判定を求めないので報告が変わらない |
| `force_unsupported` と合わせる | 指定した操作の `Native` に差し込んでも `Native` は選ばれず、判定は `GpuEmulated`・`Cpu` にだけ求める |
| 入口の返し | 経路なしは `Ok(Outcome::Unavailable { report })`、経路が決まれば `Err(Unsupported { op: <入口の関数名>, .. })`。`query_route` は経路が決まれば `Ok(報告)` |
| 3 つの入口・既定の印・`query_ex` の回数 | 変わらない（`query_ex` は 1 回） |

### 報告の `f32_denormal.basis` の読み方

- `Observed`: 実行して観察した値
- `Injected`: `Exec::inject_denormal` で差し込んだ値で経路が決まった

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `entry::injected_tests::injected_fallback_route_is_reported` | `SubgroupSumF32`・`GpuEmulated` を許す・`GpuEmulated` に `FlushToZero` | 経路 `GpuEmulated`・`{ FlushToZero, Injected, None }`。入口は `Err(Unsupported { op: "subgroup_sum_f32", .. })` で出力の列は呼ぶ前のまま |
| `entry::injected_tests::injected_unknown_moves_to_next_route` | `AtomicAdd`・両方許す・`GpuEmulated` に `Unknown`・`Cpu` に `Preserve` | 経路 `Cpu`・`{ Preserve, Injected, None }` |
| `entry::injected_tests::all_injected_unknown_is_unavailable` | `AtomicAdd`・両方許す・どちらも `Unknown` | 経路 `None`・`F32DenormalBehaviorUnknown`。入口が `Ok(Outcome::Unavailable)` で同じ報告 |
| `entry::injected_tests::uninjected_route_keeps_unsupported_error` | `GpuEmulated` にだけ `Unknown`・`Cpu` は差し込まない | `query_route` が `Err(Unsupported { op: "query_route", .. })` |
| `entry::injected_tests::injection_has_no_effect_without_judgment` | `F16Fma`・`SubgroupSumU32` の 3 経路すべてに差し込み・印 5 通り | 差し込まない `Exec` と報告が等しい |
| `entry::injected_tests::last_injection_wins` | 同じ組へ 2 回（両方の順） | 後の値 |
| `entry::injected_tests::injection_with_force` | `force_unsupported` と `Native`・`GpuEmulated` への差し込み | `GpuEmulated` が選ばれ `native_reason` は `ForcedUnsupported`。印が無ければ経路 `None`・`OptInNotGiven` |
| `entry::injected_tests::injection_keeps_default_opt_in` | 印 4 通り | `default_opt_in()` が同じ |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --features fault-injection --lib
```
