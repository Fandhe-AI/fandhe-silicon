# 下-2 の f32 の非正規化数の判定の所在（TASK-925）

下-2（`crates/exec`）の `Exec::route_report` が経路の選び方に渡す、f32 の非正規化数の判定の中身の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-925（親の Issue は #869）。

## 本リポの現状の事実

記録時点の commit は `904126a`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `src/` のファイル | `denormal.rs`・`denormal_tests.rs`・`entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs` | `ls crates/exec/src` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

## 判定と操作・経路ごとの保持（1/6）

### 所在

- 入力・判定・保持: `crates/exec/src/denormal.rs`（module は private。項目は `pub(crate)` か private で、公開の面には無い）
- 呼び出し: `Exec::route_report` が経路の選び方に渡す判定の関数が、保持の `judge` を呼ぶ（`grep -n 'held\|judge' crates/exec/src/entry.rs`）
- 試験用の部品と `Exec` 越しの試験: `crates/exec/src/denormal_tests.rs`（`#[cfg(test)]` だけ）

### 入力

判定を求める操作は `SubgroupSumF32` と `AtomicAdd`。入力はループで計算して作る。

| 操作 | 要素数 | 列の作り方 |
| ---- | ------ | ---------- |
| `SubgroupSumF32` | 768 | 要素 t（0〜255）は f32 のビット列 `t + 1`、要素 256 + t はその符号を反転したもの、要素 512〜767 はビット列 `0x0000_FFFF` |
| `AtomicAdd` | 768 | 添字 i の要素は上の列の要素 `(i mod 3) × 256 + ⌊i ÷ 3⌋`（出力 `i mod 3` に足す値） |
| `F16Fma`・`SubgroupSumU32` | なし | 入力は無い（`None`） |

### 判定

`-0.0` を `+0.0` にしたビット列で比べる。保つときの 3 つの出力のビット列は `0x0000_8080`・`0x8000_8080`・`0x00FF_FF00`。

| 3 つの出力 | 判定 |
| ---------- | ---- |
| 保つときのビット列と一致 | `Preserve` |
| 3 つとも 0 | `FlushToZero` |
| それ以外（NaN・±Inf・出力ごとに分かれる値を含む） | `Unknown` |

### 保持の動き

保持は `Exec` の値ごとに、(2 操作 × 3 経路) の組あたり 1 つ持つ。

| 呼び出し | 返し | 保持 |
| -------- | ---- | ---- |
| 保持あり | 保持した判定を `Ok` で返す。試しの実行を流さない | そのまま |
| 保持なし・試しの実行が `Ok` | 判定して `Ok` で返す | 保持する（`Unknown` も保持する） |
| 保持なし・試しの実行が `Err` | `Err` をそのまま返す | 保持しない |
| `F16Fma`・`SubgroupSumU32` | 試しの実行を流さず `Err(ContractError::InvalidArgument(..))` | 保持しない |

試しの実行の手段を持たない `Exec`（`new`・`deny_by_default` で作った値）で判定を求められた組は、`Err(ContractError::Unsupported { op: <入口の関数名 または "query_route">, .. })` を返し、保持しない。

### 試験の口

試しの実行を差す口（`Exec` の欄と `with_probe`）は `#[cfg(test)]` だけにあり、公開の面には無い。
`make exec-surface-check` の `record:` は `port_names_all=6`・`other_fns=5`・`unlisted_fns=0` のまま。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `denormal::tests::preserve_values_do_not_depend_on_order` | 2 操作の入力の順・逆順・f64 の和の丸め | どれも保つときのビット列で、判定は `Preserve` |
| `denormal::tests::inputs_are_denormal_only` | 2 操作の入力、ほかの 2 操作 | 768 要素がどれも非正規化数、ほかは `None` |
| `denormal::tests::classify_maps_outputs` | 保つ値、0、-0、1 つだけ 0・1 ulp ずれ・符号反転・NaN・±Inf | `Preserve`・`FlushToZero`・`Unknown` |
| `denormal::tests::judgment_is_kept_per_op_and_route` | 同じ組の 2 回目、別の経路、別の操作、`Unknown` | 同じ組は流さない、別の組は流す、`Unknown` も保持、受けた入力は入力の関数の値と等しい |
| `denormal::tests::errors_are_not_kept` | `Err(Timeout)` の後に `Ok` | `Err` を返し、次の呼び出しで流して保持する |
| `denormal_tests::query_route_and_entries_share_judgment` | `query_route` と入口を両方の順で呼ぶ | 試しの実行は 1 回、報告は同じ |
| `denormal_tests::each_exec_value_keeps_its_own_judgment` | 同じ device の 2 つの `Exec` | それぞれ 1 回流す |
| `denormal_tests::flush_to_zero_and_fallback_routes_are_reported` | 3 つとも 0、`Unsupported` の device と上書き | `FlushToZero`、経路ごとに流す先が決まる |
| `denormal_tests::ports::judgment_survives_ports`（feature `fault-injection`） | 保持の後に `force_unsupported`・`inject_denormal` | 同じ組の試しの実行は増えない |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --locked --lib denormal
cargo test -p fandhe-silicon-exec --all-features --locked --lib denormal
make exec-surface-check
```

## 報告の資料の記述の項（2/6）

### 所在

- `crates/exec/src/entry.rs` の `route_report` の最後の段と、その段が呼ぶ private な関数 `with_documented`
- `crates/exec/src/report.rs` の `F32DenormalReport::documented`
- 試験は `crates/exec/src/denormal_tests.rs` の `mod documented`

### 組

`route_report` が組み立てた報告の `f32_denormal` が `Some` のときだけ、`documented` を次の表で決める。
判定・経路の選び方・`behavior`・`basis` には使わない（`basis` が `Observed` でも `Injected` でも同じ）。

| device の `backend()` | 操作 | 経路 | `documented` |
| --------------------- | ---- | ---- | ------------ |
| `BackendKind::Cuda` | `OpId::AtomicAdd` | `Route::Native` | `Some(F32DenormalBehavior::FlushToZero)` |
| ほか | ほか | ほか | `None` |

経路が `None` の報告は `f32_denormal` も `None` のままで、`documented` から作らない。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `denormal_tests::documented::documented_is_set_for_one_combination` | `Cuda`・`AtomicAdd`、保つ値・0・判定できない値の runner | 保つ値と 0 では `behavior` は観察のまま `documented` が `Some(FlushToZero)`、判定できない値では `f32_denormal` が `None` |
| `denormal_tests::documented::documented_is_none_elsewhere` | `Cuda` の `SubgroupSumF32`、`Cuda` の `AtomicAdd` の 2 つの上書き、ほかの 3 つの `backend()` | どれも `documented` は `None` |
| `denormal_tests::documented::documented_with_injection`（feature `fault-injection`） | `Cuda`・`AtomicAdd`・`Native` に `inject_denormal` | `basis` は `Injected`、`documented` は `Some(FlushToZero)`、試しの実行は流れない |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --locked --lib denormal_tests::documented
cargo test -p fandhe-silicon-exec --all-features --locked --lib denormal_tests::documented
```

## 拒否した呼び出しと経路の無い呼び出しで試しの実行を流さない試験（3/6）

### 所在

試験は `crates/exec/src/denormal_tests.rs` の `mod rejected`。ライブラリのコードは変えない。

### 組

| 軸 | 値 |
| -- | -- |
| 上書きの `opt_in` | `None`・全拒否・`G_ONLY`・`C_ONLY`・`BOTH` の 5 通り |
| `Exec::new` の既定の印 | 全拒否・`G_ONLY`・`C_ONLY`・`BOTH` の 4 通り |
| device | 能力が `S2` の device、能力が `Unsupported`（`UnknownFeature`）の device の 2 通り |

数えるのは、模擬の runner が呼ばれた回数を各呼び出しの前後で読んだ差。戻り値の文言は照らさない。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `rejected_calls_do_not_run_judgment` | 上の 40 組で、`n` が 0、`atomic_add` の `m` が 0、`n` が `u64::MAX`、`n` または `m` が上限を 1 超える長さ | どれも `InvalidArgument`、runner の回数の差は 0 |
| `rejection_does_not_consume_judgment` | 40 組で、拒否の呼び出しの後に長さの合う入口または `query_route` | `InvalidArgument` ではなく、経路が届く組は回数が 1、届かない組は 0 |
| `calls_without_route_do_not_run_judgment` | 能力が `S0`・`S1` の device、全拒否の効く印 | 入口は `Ok(Outcome::Unavailable { .. })`、`query_route` の経路は `None`、回数は 0 |
| `forced_calls_without_opt_in_do_not_run_judgment`（feature `fault-injection`） | `S2` の device で `force_unsupported` した操作 | 全拒否では回数 0、`G_ONLY` では `GpuEmulated` で 1 回、`Native` は 0 回 |
| `f16_and_u32_never_run_judgment` | 40 組で `f16_fma`・`subgroup_sum_u32` の入口と `query_route` | 回数は 0 のまま |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --locked --lib denormal_tests::rejected
cargo test -p fandhe-silicon-exec --all-features --locked --lib denormal_tests::rejected
```
