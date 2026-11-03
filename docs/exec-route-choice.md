# 下-2 の経路の選び方の所在（TASK-920）

下-2（`crates/exec`）で、能力の段階と効く opt-in の印から経路を決める内部の関数の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-920（親の Issue は #864）。

## 本リポの現状の事実

記録時点の commit は `fa67b42`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| 選び方の所在 | `crates/exec/src/select.rs`（置き換え前は 240 行、`fn choose` は 42 行目） | `git show fa67b42:crates/exec/src/select.rs \| wc -l` |
| 4 つの入口の返し方 | 経路なしは `Ok(Outcome::Unavailable { report })`、経路が決まれば `Err(ContractError::Unsupported { .. })` | `crates/exec/src/entry.rs` の `run` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

## 判定を受ける経路の選び方（1/5）

### 所在

- `crates/exec/src/select.rs` の `select_route` と `DenormalJudgment`（`Preserve`・`FlushToZero`・`Unknown`）
- `crates/exec/src/entry.rs` の `Exec::run`（4 つの入口が通る内部の関数）

どれも `pub(crate)` または private で、crate の外には公開しない。

### 動き

`select_route(input, judge)` は `SelectionInput`（操作・効く印・能力の問合せの値）と判定の関数を受け、`Result<SelectionOutput, ContractError>` を返す。

1. `Native` が届くのは能力の段階が `S2`〜`S6` のとき。`Unsupported` なら届かず理由は `CapabilityUnsupported(..)`、`S0`・`S1` などは届かず理由は `CapabilityUnknown`。
2. 候補は `Native`（届くとき）、`GpuEmulated`（印が許すとき）、`Cpu`（印が許すとき）の順。
3. `F16Fma`・`SubgroupSumU32` は判定を求めず、候補の先頭を選ぶ。`f32_denormal` は `None`。
4. `SubgroupSumF32`・`AtomicAdd` は候補の順に判定の関数を 1 回ずつ呼ぶ。`Preserve`・`FlushToZero` ならその経路を選んで以降は呼ばない。`Unknown` ならその候補を外して次へ進み、外したのが `Native` なら `Native` の理由は `F32DenormalBehaviorUnknown`。
5. 判定の関数が `Err` を返したら、以降は呼ばずその `Err` をそのまま返す。
6. 選べないとき経路は `None`。代わりの理由は、`GpuEmulated` も `Cpu` も許さなければ `OptInNotGiven`、どちらかを許せば `F32DenormalBehaviorUnknown`。

### 入口の返し方

4 つの入口は `query_ex` を 1 回呼び、`select_route` に入口の関数名で `Err(ContractError::Unsupported { .. })` を返す判定の関数を渡す。
`SubgroupSumF32`・`AtomicAdd` で経路が届く呼び出しは、その `Err` を返す。経路なしは `Ok(Outcome::Unavailable { report })`。

### 試験

| 試験 | 確かめる点 |
| ---- | ---------- |
| `judged_native_is_chosen_with_behavior` | `Native` の判定の値が `f32_denormal` に出る |
| `unknown_native_moves_to_allowed_fallback` | `Native` が `Unknown` なら許された `GpuEmulated` へ進む |
| `unknown_native_without_opt_in_is_unavailable` | 印なしで `Native` が `Unknown` なら経路なし |
| `all_unknown_with_opt_in_is_unavailable` | 候補がすべて `Unknown` なら経路なし |
| `judge_error_is_returned_unchanged` | 判定の関数の `Err` がそのまま返る |
| `judge_is_not_called_without_need` | 判定を求めない場面で判定の関数を呼ばない |

確かめ方は `cargo test -p fandhe-silicon-exec --lib`。

## 経路の報告を返す `query_route`（2/5）

### 所在

`crates/exec/src/entry.rs` の `Exec::query_route`（公開）と、4 つの入口・`query_route` が共通で通す private な `Exec::route_report`。
`route_report` は効く印を決め、`query_ex` を 1 回呼び、`select_route` で経路を選んで報告を組み立てる。

### 引数・戻り値・動き

| 項目 | 内容 |
| ---- | ---- |
| 引数 | `op: OpId`、`opt_in: Option<OptIn>` |
| 戻り値 | `Result<RouteReport, ContractError>` |
| 問合せ | `query_ex` を 1 回。`memory()`・`queue()`・下-1 の投入・確保・読み出しは呼ばない |
| 経路が `None` の報告 | `Ok` |
| 経路がある報告 | `Ok`（入口と違い `Err` に変えない） |
| 判定の関数 | 呼ばれると `Err(ContractError::Unsupported { op: "query_route", backend })`。`backend` は `Device::backend()` の値 |
| 既定の印 | 変えない |

### 入口の返しとの対応

| 入口の返し | `query_route` の返し |
| ---------- | -------------------- |
| `Ok(Outcome::Unavailable { report })` | `Ok`・`report` と等しい報告 |
| 経路が決まって `Err(Unsupported)`（`F16Fma`・`SubgroupSumU32`） | `Ok`・経路あり |
| `SubgroupSumF32`・`AtomicAdd` で経路が届く | `Err(ContractError::Unsupported { .. })`（`op` は `"query_route"`） |

### 試験

| 試験 | 確かめる点 |
| ---- | ---------- |
| `query_route_equals_unavailable_report` | 4 操作で入口の `Unavailable` の報告と等しい |
| `query_route_reports_fallback_route` | `F16Fma`・`SubgroupSumU32` は `Cpu` の経路つきの `Ok`、`SubgroupSumF32`・`AtomicAdd` は `op` が `"query_route"` の `Err` |
| `query_route_keeps_default_opt_in` | 呼んだ後も既定の印が変わらない |

確かめ方は `cargo test -p fandhe-silicon-exec --lib`。
