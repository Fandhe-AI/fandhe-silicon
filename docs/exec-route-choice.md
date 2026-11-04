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

補い（base c79863d）: 選び方の通り道は `crates/exec/src/entry.rs` の内部の関数 `route_report` で、効く印の決定（`effective_opt_in`）・`query_ex` の 1 回・`select_route`・`build_report` の順に通す。4 つの入口の `run` と `Exec::query_route` はどちらもこの関数を呼び、違いは判定の関数の `Err` に使う `Unsupported` の `op` の名前（入口の関数名か `query_route`）だけ。

補い: 記録時点の commit は、この節の `fa67b42` と、完了の条件の確かめ（5/5）の方法の `6b35dd9` で異なる。`git diff --stat 6b35dd9 fa67b42 -- crates/exec` の出力は空で、この 2 つの commit の間で `crates/exec` の差分は 0 件。

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

## 能力の値・印・判定の全組み合わせの試験（3/5）

### 所在

`crates/exec/src/select.rs` の `#[cfg(test)]` の中の `combination_tests`。ライブラリのコードは変えない。

### 組み合わせの軸と件数

| 軸 | 通り |
| -- | ---- |
| 能力の値 | 11（`S0`〜`S6` の 7、`Unsupported` × 理由 `None`・`NoApi`・`NotSupported`・`UnknownFeature` の 4） |
| 印 | 4（なし・`GpuEmulated` だけ・`Cpu` だけ・両方） |
| 操作 | `F16Fma`・`SubgroupSumU32` は判定の関数 1 通り。`SubgroupSumF32`・`AtomicAdd` は `Native`・`GpuEmulated`・`Cpu` ごとの答え 4 通り（`Preserve`・`FlushToZero`・`Unknown`・`Err`）の 64 通り |
| 全件 | 11 × 4 × (2 + 2 × 64) = 5720 |

期待の値は、試験の中の関数が候補を並べて先頭から判定の答えを当てて作る。

### 試験

| 試験 | 確かめる点 |
| ---- | ---------- |
| `every_combination_matches_candidate_rule` | 5720 件すべてで、返しと呼ばれた経路の並びが期待と等しい |
| `unsupported_rows_keep_query_reason` | `Unsupported` の行で `Native` に判定を求めず、`native_reason` が問合せの理由のまま |
| `unknown_levels_never_choose_native` | `S0`・`S1` の行で `Native` を選ばず、`native_reason` が `CapabilityUnknown` |
| `report_f32_item_follows_judged_route` | 報告の f32 の項目が、判定した操作で経路ありのときだけ判定の値を持つ |

確かめ方は `cargo test -p fandhe-silicon-exec --lib`。

## 模擬の能力の値での `query_route` と入口の一致の試験（4/5）

### 所在

`crates/exec/tests/route_query.rs`（統合テスト）と、`crates/exec/tests/common/mod.rs` の `Scripted::answering`。ライブラリのコードは変えない。

### 試験用の device

`Scripted::answering(値)` は、どの機能の鍵の問合せにも同じ能力の値を返す。`query` はその段階を返し、`query_ex` は呼び出しを数えたうえで値をそのまま返す。

### 試験

| 試験 | device | 入力 | 確かめる点 |
| ---- | ------ | ---- | ---------- |
| `query_route_matches_entry_report` | 能力の値 9 通り（`S0`〜`S6`、`Unsupported` × 理由 `None`・`NotSupported`） | 操作 4 つ × 呼び方 5 通り（既定のまま・上書きの 4 通り） | 入口が `Ok(Outcome::Unavailable { report })` のとき `query_route` の報告が `report` と等しい。入口が `Err(Unsupported)` のとき、`F16Fma`・`SubgroupSumU32` は経路ありの `Ok`、`SubgroupSumF32`・`AtomicAdd` は `Err(Unsupported)`。出力の列は呼ぶ前のまま |
| `native_levels_report_native_route` | `S2`〜`S6` | 上と同じ呼び方 | `F16Fma`・`SubgroupSumU32` は `Native`・理由なし・観察した段階を報告する。ほかの 2 操作は `op` が `"query_route"` の `Err(Unsupported)` |
| `unreachable_native_follows_opt_in_order` | `Unsupported` × 理由 4 通り、`S0`、`S1` | 呼び方 5 通り | 印が `GpuEmulated` だけ・両方なら `GpuEmulated`、`Cpu` だけなら `Cpu`、印なしなら経路なしで `OptInNotGiven`。`native_reason` は `Unsupported` なら問合せの理由、`S0`・`S1` なら `CapabilityUnknown` |
| `query_route_calls_query_ex_once` | 何も差し込まない device と `S2` を返す device | 操作 4 つ × 上書き 3 通り | `query_ex` が 1 回、`memory`・`queue` が 0 回（`Err` を返す呼び出しも同じ） |

確かめ方は `cargo test -p fandhe-silicon-exec --test route_query`。

## 完了の条件の確かめ（5/5）

5/5（#5999）で、下-2 の crate（`crates/exec`）について本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-920（ポインタのみ）。GPU の実機の無い Linux で実行した。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 依存・unsafe・`Cargo.toml` の差分 | `cargo metadata --no-deps` の `fandhe-silicon-exec` の依存、`grep -rc unsafe crates/exec/src`、`grep -n forbid crates/exec/src/lib.rs`、`git diff --stat 6b35dd9 -- crates/exec/Cargo.toml`、`make deps-check` | 依存は `fandhe-silicon-core` だけ（normal は feature なし、dev は `test-support`・`fault-injection` 付き）。`unsafe` の出現は `lib.rs` の 1 件だけ（ほか 6 ファイルは 0 件）で、その行は `14:#![forbid(unsafe_code)]`。`Cargo.toml` の差分は出力なし（空）。`make deps-check` は終了コード 0 で、行は `ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml` |
| (2) | 選び方の関数と呼び出し元 | `grep -rn 'select_route' crates/exec/src` | 定義は `select.rs:61` の `pub(crate) fn select_route(`。`entry.rs:17` の取り込みと `entry.rs:75` の `let output = select_route(&input, judge)?;` が本体側の出現で、残りは `select.rs` の試験の中（`209`・`232`・`247`・`263`・`319`） |
| (3) | 環境・ファイル・`static`・内部の可変性・`&mut self` | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|Cell\|&mut self' crates/exec/src` | 4 行。`select.rs:309` の `use std::cell::RefCell;`、`select.rs:318` の `let called = RefCell::new(Vec::new());`（どちらも試験の中）、`entry.rs:86` の `name: &'static str,`、`entry.rs:112` の `fn unsupported(&self, op: &'static str) -> ContractError {` |
| (4) | 試験の件数 | `cargo test -p fandhe-silicon-exec --lib` / `--lib --features native-blocking-wait` / `--test route_query` / `--test opt_in_default` / `--test reasons_and_denormal` / `--test unsupported_fields` / `--features native-blocking-wait --test device_operands`。組み合わせの数は `grep -n 'assert_eq!(checked, 5720)' crates/exec/src/select.rs` | `--lib` は 30 passed、`native-blocking-wait` 付きは 31 passed。`route_query` は 4 passed、`opt_in_default` は 5 passed、`reasons_and_denormal` は 4 passed、`unsupported_fields` は 1 passed、`device_operands` は 1 passed（すべて 0 failed; 0 ignored）。`grep` の出力は `649:            assert_eq!(checked, 5720);`、`every_combination_matches_candidate_rule` は 1 passed |
| (5) | feature なし・`native-blocking-wait`・wasm32 | `cargo clippy -p fandhe-silicon-exec --all-targets -- -D warnings`（feature なしと `--features native-blocking-wait`）、`cargo test -p fandhe-silicon-exec`（同）、`cargo check -p fandhe-silicon-exec --target wasm32-unknown-unknown`（`--tests` 付きも） | clippy は両方とも終了コード 0。`cargo test` は feature なしが合計 44 passed（30 + 5 + 4 + 4 + 1 + doctest 0）、`native-blocking-wait` 付きが合計 46 passed（31 + 1 + 5 + 4 + 4 + 1 + doctest 0）。いずれも 0 failed; 0 ignored。wasm32 の `cargo check` は `--tests` の有無とも終了コード 0 |
| (6) | 公開名・公開 API の検査 | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check`・`make public-api-webgpu-check` の `scanned: crate=fandhe_silicon_exec` の行 | 4 つとも終了コード 0。`public-api-check` は production で `public_items=458 wgpu_type_refs=0 wgpu_other_refs=0`、all-features で `public_items=472 wgpu_type_refs=0 wgpu_other_refs=0`。`public-name-check` は production で `public_items=403 named_items=123 name_hits=0`、all-features で `public_items=417 named_items=137 name_hits=0`。`public-api-sdk-check` は両方とも `sdk_type_refs=0 sdk_other_refs=0 external_crates=alloc,core,fandhe_silicon_core`。`public-api-webgpu-check` は exec の `scanned:` の行を出さず（対象の文書は core と contract の 2 件、`documents=2`）、`record:` の行の `pipeline_input_violations_total=0` |

補い: exec の依存の向きは `make exec-deps-check`（[exec-deps-check.md](exec-deps-check.md)）が見る。終了コードは 0。

補い: 上の表 (4) の 5720 は標準出力に印字されない。`every_combination_matches_candidate_rule` が passed であることで確かめている。

確かめた時点は main の `5b10b82`、`docs/spec` の gitlink は `c7887a1a7f75903d4016ba42e0595ae6cbb2cd0b`。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-920）。

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/5 | #5995 | feat(exec): TASK-920 の 1/5 判定を受ける経路の選び方と入口の配線 |
| 2/5 | #5996 | feat(exec): TASK-920 の 2/5 経路の報告を返す query_route |
| 3/5 | #5997 | test(exec): TASK-920 の 3/5 能力の値・印・判定の全組み合わせの単体試験 |
| 4/5 | #5998 | test(exec): TASK-920 の 4/5 模擬の能力の値での query_route と入口の一致の試験 |
| 5/5 | #5999 | docs: TASK-920 の 5/5 完了の条件の確かめ |

補い（base c79863d）: 上の表の 3/5 の題は Issue #5997 の題で、マージした PR #7897 の件名は `test(exec): TASK-920 の 3/5 経路の選び方の全組み合わせの単体試験`。

補い（base c79863d）: `cargo test -p fandhe-silicon-exec --test route_query` は 6 passed; 0 failed; 0 ignored（上の表 (4) の 4 passed から、#7903 が試験を 2 本足して増えた）。`cargo test -p fandhe-silicon-exec` の合計は feature なしが 46 passed（30 + 5 + 4 + 6 + 1 + doctest 0）、`native-blocking-wait` 付きが 48 passed（31 + 1 + 5 + 4 + 6 + 1 + doctest 0）。

## 今の値の補い（base 48025c6）

上の完了の条件の確かめ（5/5）の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 表の行 | 記録の値 → 今の値 | 測ったコマンド |
| ------ | ----------------- | -------------- |
| (1) の `forbid` の行 | `14:#![forbid(unsafe_code)]` → `18:#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |
| (1) の `Cargo.toml` の差分 | 出力なし（空）→ `1 file changed, 7 insertions(+)`（`--numstat` は `7 0`） | `git diff --stat 6b35dd9 -- crates/exec/Cargo.toml` |
| (2) の選び方の関数と呼び出し元 | 定義 `select.rs:61` → `select.rs:71`。本体側の呼び出し `entry.rs:75` → `entry.rs:132` の `let output = select_route(&input, judge)?;`。取り込みは `entry.rs:17` のまま。試験の中の出現は `222`・`245`・`260`・`276`・`332`・`828` | `grep -rn 'select_route' crates/exec/src` |
| (3) の `RefCell` の行 | `select.rs:309`・`select.rs:318` → `select.rs:322` の `use std::cell::RefCell;` と `select.rs:331` の `let called = RefCell::new(Vec::new());`。`&'static` の行は `entry.rs:86`・`entry.rs:112` → `entry.rs:146`・`entry.rs:172`（4 行のまま） | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|Cell\|&mut self' crates/exec/src` |
| (4) の組み合わせの数の行 | `649:` → `662:            assert_eq!(checked, 5720);`。`every_combination_matches_candidate_rule` は feature なしでも `--features fault-injection` でも 1 passed | `grep -n 'assert_eq!(checked, 5720)' crates/exec/src/select.rs` |
| (6) の all-features | `public-api-check` の `public_items=472` → `511`、`public-name-check` の `public_items=417 named_items=137` → `451`・`149`、`public-api-sdk-check` の `public_items=417` → `451`（production の `458`・`403`・`123` は変わらない） | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check` |

feature `fault-injection` の下では、上の表の「入口の返しとの対応」の `SubgroupSumF32`・`AtomicAdd` の行の動きが変わる。`Exec::query_route` の rustdoc（`crates/exec/src/entry.rs:160` 付近）は、`Err(ContractError::Unsupported { op: "query_route", backend })` を返すのを「差し込みの無い経路に届く呼び出し」に限っている。`Exec::inject_denormal` で差し込んだ (操作, 経路) の組に届く呼び出しは、`route_report`（`entry.rs:108`）が差し込みの値を判定として使うので `Ok` の報告を返す。差し込みの無い組は今までどおり `Err` を返す。feature を付けないビルドの動きは変わらない。

確かめ方:

```text
cargo test -p fandhe-silicon-exec --features fault-injection --test fault_injection_ports
```

`fault_injection_ports` は 8 passed; 0 failed; 0 ignored。差し込みの無い `F32` 系の操作が `op: "query_route"` の `Err` を返す試験は `force_without_injection_keeps_unsupported_for_f32_ops`、差し込みが報告に届く試験は `injection_reaches_reports`。

`cargo test -p fandhe-silicon-exec --features fault-injection` の合計は 67 passed（43 + 8 + 5 + 4 + 6 + 1 + doctest 0）。feature なしの合計は 46 passed、`native-blocking-wait` 付きは 48 passed のまま。所在は [exec-fault-injection](exec-fault-injection.md)。

## 今の値の補い（base 2373576）

記録時点の表と既存の節は書き換えない。以下は base `2373576` で観測した今の値（0 failed; 0 ignored）。`crates/exec/tests/route_evidence_calls.rs` が加わった。

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 46 passed → 合計 49 passed | 30 + 5 + 4 + 3 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 48 passed → 合計 51 passed | 31 + 1 + 5 + 4 + 3 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 67 passed → 合計 71 passed | 43 + 8 + 5 + 4 + 4 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | -- → 合計 73 passed | 44 + 1 + 8 + 5 + 4 + 4 + 6 + 1 + doctest 0 |

`--lib` は feature なしで 30 passed、`native-blocking-wait` 付きで 31 passed のまま。

## 今の値の補い（base 904126a）

記録時点の表と既存の節は書き換えない。以下は base `904126a` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。4 つの入口に長さの検査（[exec-length-check.md](exec-length-check.md)）が加わった（TASK-924）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| (1) の `forbid` の行 | `18:#![forbid(unsafe_code)]` → `19:#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |
| (1) の `Cargo.toml` の差分 | `1 file changed, 7 insertions(+)` → `1 file changed, 12 insertions(+)`（`--numstat` は `12 0`） | `git diff --stat 6b35dd9 -- crates/exec/Cargo.toml` |
| (2) の選び方の関数と呼び出し元 | 本体側の呼び出し `entry.rs:132` → `entry.rs:133` の `let output = select_route(&input, judge)?;`。取り込みは `entry.rs:18`。定義は `select.rs:71` のまま。試験の中の出現は `222`・`245`・`260`・`276`・`332`・`828` のまま | `grep -rn 'select_route' crates/exec/src` |
| (3) の `RefCell` の行 | `select.rs:322`・`select.rs:331` のまま。`&'static` の行は `entry.rs:146`・`entry.rs:172` → `entry.rs:147`・`entry.rs:173` | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|Cell\|&mut self' crates/exec/src` |
| (4) の組み合わせの数の行 | `662:            assert_eq!(checked, 5720);` のまま | `grep -n 'assert_eq!(checked, 5720)' crates/exec/src/select.rs` |

試験の件数は次のとおり。

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 49 passed → 合計 64 passed | 36 + 4 + 5 + 5 + 4 + 3 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 51 passed → 合計 67 passed | 37 + 1 + 1 + 4 + 5 + 5 + 4 + 3 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 71 passed → 合計 86 passed | 49 + 8 + 4 + 5 + 5 + 4 + 4 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 73 passed → 合計 89 passed | 50 + 1 + 8 + 1 + 4 + 5 + 5 + 4 + 4 + 6 + 1 + doctest 0 |

`route_query` は 6 passed のまま、`fault_injection_ports` は 8 passed のまま、`every_combination_matches_candidate_rule` は `--lib` の中で 1 passed のまま。

## 今の値の補い（base dbbb5c0）

記録時点の表と既存の節は書き換えない。以下は base `dbbb5c0` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。`f16_fma` は経路が `Route::Cpu` で host の形なら出力の列に書き、`Ok(Outcome::Ran { report })` を返す（所在は [exec-cpu-route.md](exec-cpu-route.md)）。

### 入口の返し

「本リポの現状の事実」の「4 つの入口の返し方」の行、「入口の返し方」の節、「入口の返しとの対応」の表は、`f16_fma` については次のとおり。

| 呼び出し | 入口の返し | `query_route` の返し |
| -------- | ---------- | -------------------- |
| `f16_fma`・経路が `Route::Cpu`・host の形 | `Ok(Outcome::Ran { report })`。出力の列に書く | `Ok`・`report` と等しい報告 |
| `f16_fma`・経路なし | `Ok(Outcome::Unavailable { report })` | `Ok`・`report` と等しい報告 |
| `f16_fma`・device の形、または `Route::Cpu` 以外の経路 | `Err(ContractError::Unsupported { op: "f16_fma", .. })` | `Ok`・経路あり |
| `subgroup_sum_u32` で経路が決まる | `Err(ContractError::Unsupported { op: "subgroup_sum_u32", .. })` | `Ok`・経路あり |
| `subgroup_sum_f32`・`atomic_add` で経路が届く | `Err(ContractError::Unsupported { .. })`（`op` は入口の関数名） | `Err(ContractError::Unsupported { .. })`（`op` は `"query_route"`） |

`Exec::query_route` の rustdoc は、入口が `Ok(Outcome::Unavailable { report })` か `Ok(Outcome::Ran { report })` を返す呼び出しでは `report` と等しいと書く（`crates/exec/src/entry.rs` の 203 行）。`Ran` の報告との一致を照らす試験は、`entry::tests::f16_fma_with_cpu_override_runs_on_host`（`entry.rs`）、`cpu_f16_fma.rs` の `r_is_within_order_distance_over_lengths` を含む組、`route_query.rs` の `query_route_matches_entry_report` の `Ran` の腕。

### 完了の条件の確かめ（5/5）の値

| 表の行 | 記録の値 → 今の値 | 測ったコマンド |
| ------ | ----------------- | -------------- |
| (1) の `forbid` の行 | `14:#![forbid(unsafe_code)]` → `20:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけで、ほか 12 ファイルは 0 件 | `grep -n forbid crates/exec/src/lib.rs`、`grep -rc unsafe crates/exec/src` |
| (2) の選び方の関数と呼び出し元 | 定義 `select.rs:71` → `select.rs:69`。本体側の呼び出し `entry.rs:133` → `entry.rs:177` の `let output = select_route(&input, judge)?;`。取り込みは `entry.rs:23`。`select.rs` の試験の中の出現は `220`・`243`・`258`・`274`・`330`・`826` | `grep -rn 'select_route' crates/exec/src` |
| (3) の `RefCell` と `&'static` の行 | 4 行 → 14 行。`select.rs:320` の `use std::cell::RefCell;`、`select.rs:329` の `RefCell::new`、`denormal_tests.rs:24` の `use std::cell::RefCell;`、`denormal_tests.rs:122` の `type CallLog = RefCell<..>`、`denormal.rs:10` の `use std::cell::Cell;`、`denormal.rs:107` の `cells: [[Cell<..>; 3]; 2],`（保持。`HeldJudgments` の欄）、`denormal.rs` の `Cell::new`（193・243・245・291 行。試験の中）、`entry.rs:154`・`191`・`224`・`234` の `&'static str`。`std::env`・`env!`・`option_env!`・`std::fs`・`&mut self` の行は無い | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|Cell\|&mut self' crates/exec/src` |
| (4) の組み合わせの数の行 | `662:` → `660:            assert_eq!(checked, 5720);`。`every_combination_matches_candidate_rule` は feature なしでも `--features fault-injection` でも 1 passed | `grep -n 'assert_eq!(checked, 5720)' crates/exec/src/select.rs` |

`Exec` の保持は `crates/exec/src/denormal.rs` の `HeldJudgments`（`Cell` の欄）にあり、`route_report` が判定の関数の中で使う（所在は [exec-denormal-judgment.md](exec-denormal-judgment.md)）。

### 試験の件数

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 64 passed → 合計 98 passed | 63 + 3 + 2 + 4 + 5 + 6 + 4 + 3 + 6 + 2 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 67 passed → 合計 103 passed | 64 + 3 + 2 + 2 + 2 + 4 + 5 + 6 + 4 + 3 + 6 + 2 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 86 passed → 合計 130 passed | 85 + 4 + 8 + 2 + 4 + 5 + 6 + 4 + 4 + 6 + 2 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 89 passed → 合計 135 passed | 86 + 4 + 2 + 8 + 2 + 2 + 4 + 5 + 6 + 4 + 4 + 6 + 2 + doctest 0 |

`--lib` は feature なしで 36 passed → 63 passed、`native-blocking-wait` 付きで 37 passed → 64 passed、`fault-injection` 付きで 49 passed → 85 passed、`--all-features` で 50 passed → 86 passed。`route_query` は 6 passed のまま。

## 今の値の補い（base 515bb19）

記録時点の表と既存の節は書き換えない。以下は base `515bb19` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。`subgroup_sum_u32`・`subgroup_sum_f32` は経路が `Route::Cpu` で host の形なら出力の列に書き、`Ok(Outcome::Ran { report })` を返す（所在は [exec-cpu-route.md](exec-cpu-route.md)）。

### 入口の返し

「今の値の補い（base dbbb5c0）」の「入口の返し」の表の `subgroup_sum_u32` の行と、`subgroup_sum_f32`・`atomic_add` の行は、次のとおり。

| 呼び出し | 入口の返し | `query_route` の返し |
| -------- | ---------- | -------------------- |
| `subgroup_sum_u32`・`subgroup_sum_f32`・経路が `Route::Cpu`・host の形 | `Ok(Outcome::Ran { report })`。出力の列に書く | `Ok`・`report` と等しい報告 |
| `subgroup_sum_u32`・`subgroup_sum_f32`・経路なし | `Ok(Outcome::Unavailable { report })` | `Ok`・`report` と等しい報告 |
| `subgroup_sum_u32`・device の形、または `Route::Cpu` 以外の経路 | `Err(ContractError::Unsupported { op: "subgroup_sum_u32", .. })` | `Ok`・経路あり |
| `subgroup_sum_f32`・経路が `Route::Cpu`・device の形 | `Err(ContractError::Unsupported { op: "subgroup_sum_f32", .. })` | `Ok`・経路あり |
| `subgroup_sum_f32`・経路が `Route::Native` か `Route::GpuEmulated`（判定の差し込みなし） | `Err(ContractError::Unsupported { op: "subgroup_sum_f32", .. })`（判定の試しの実行が拒否する） | `Err(ContractError::Unsupported { .. })`（`op` は `"query_route"`） |
| `atomic_add` で経路が届く | `Err(ContractError::Unsupported { op: "atomic_add", .. })` | `Err(ContractError::Unsupported { .. })`（`op` は `"query_route"`） |

`Exec::query_route` の rustdoc は、入口が `Ok(Outcome::Unavailable { report })` か `Ok(Outcome::Ran { report })` を返す呼び出しでは `report` と等しいと書く（`crates/exec/src/entry.rs` の 204 行）。subgroup の 2 入口で `Ran` の報告との一致を照らす試験は、`entry::tests::subgroup_sums_with_cpu_override_run_on_host`（`entry.rs`）、`route_query.rs` の `query_route_matches_entry_report` の `Ran` の腕。 `subgroup_sum_f32` で `query_route` が `Unsupported` になることは `route_query.rs` の `native_levels_report_native_route`（`Route::Native`。`op` が `"query_route"`）と `unreachable_native_follows_opt_in_order`（`Route::GpuEmulated`）、入口が入口の関数名の `Unsupported` を返すことは `unsupported_fields.rs` の `unsupported_carries_entry_name_and_device_backend` が照らす。

### 完了の条件の確かめ（5/5）の値

| 表の行 | 記録の値 → 今の値 | 測ったコマンド |
| ------ | ----------------- | -------------- |
| (1) の `forbid` の行 | `20:#![forbid(unsafe_code)]` → `21:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけで、ほか 12 ファイルは 0 件 | `grep -n forbid crates/exec/src/lib.rs`、`grep -rc unsafe crates/exec/src` |
| (2) の選び方の関数と呼び出し元 | 定義 `select.rs:69` のまま。本体側の呼び出し `entry.rs:177` → `entry.rs:178` の `let output = select_route(&input, judge)?;`。取り込みは `entry.rs:23` → `entry.rs:24`。`select.rs` の試験の中の出現は `220`・`243`・`258`・`274`・`330`・`826` のまま | `grep -rn 'select_route' crates/exec/src` |
| (3) の `RefCell` と `&'static` の行 | 14 行のまま。`entry.rs` の `&'static str` は `154`・`191`・`224`・`234` 行 → `155`・`192`・`228`・`244` 行。ほかの行（`select.rs:320`・`329`、`denormal_tests.rs:24`・`122`、`denormal.rs:10`・`107`・`193`・`243`・`245`・`291`）は変わらない。`std::env`・`env!`・`option_env!`・`std::fs`・`&mut self` の行は無い | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|Cell\|&mut self' crates/exec/src` |
| (4) の組み合わせの数の行 | `660:            assert_eq!(checked, 5720);` のまま。`every_combination_matches_candidate_rule` は feature なしでも `--features fault-injection` でも 1 passed | `grep -n 'assert_eq!(checked, 5720)' crates/exec/src/select.rs` |

### 試験の件数

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 98 passed → 合計 114 passed | 68 + 3 + 5 + 4 + 4 + 5 + 8 + 4 + 3 + 6 + 4 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 103 passed → 合計 121 passed | 69 + 3 + 5 + 3 + 3 + 4 + 4 + 5 + 8 + 4 + 3 + 6 + 4 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 130 passed → 合計 148 passed | 91 + 4 + 6 + 8 + 4 + 4 + 5 + 8 + 4 + 4 + 6 + 4 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 135 passed → 合計 155 passed | 92 + 4 + 6 + 3 + 8 + 3 + 4 + 4 + 5 + 8 + 4 + 4 + 6 + 4 + doctest 0 |

内訳の並びは [exec-skeleton-types.md](exec-skeleton-types.md) の「今の値の補い（base 515bb19）」の試験の件数の表と同じ。`--lib` は feature なしで 63 passed → 68 passed、`native-blocking-wait` 付きで 64 passed → 69 passed、`fault-injection` 付きで 85 passed → 91 passed、`--all-features` で 86 passed → 92 passed。`route_query` は 6 passed のまま。

## 今の値の補い（base 8e4b99b）

記録時点の表と既存の節は書き換えない。以下は base `8e4b99b` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。`atomic_add` は経路が `Route::Cpu` で host の形なら出力の列に書き、`Ok(Outcome::Ran { report })` を返す（所在は [exec-cpu-route.md](exec-cpu-route.md)）。

### 入口の返し

「今の値の補い（base 515bb19）」の「入口の返し」の表の `atomic_add` の行は、次のとおり。

| 呼び出し | 入口の返し | `query_route` の返し |
| -------- | ---------- | -------------------- |
| `atomic_add`・経路が `Route::Cpu`・host の形（`allow_cpu: true` だけの印を含む） | `Ok(Outcome::Ran { report })`。出力の列に書く | `Ok`・`report` と等しい報告 |
| `atomic_add`・経路なし | `Ok(Outcome::Unavailable { report })` | `Ok`・`report` と等しい報告 |
| `atomic_add`・経路が `Route::Cpu`・device の形 | `Err(ContractError::Unsupported { op: "atomic_add", .. })` | `Ok`・経路あり |
| `atomic_add`・経路が `Route::Native` か `Route::GpuEmulated`（判定の差し込みなし） | `Err(ContractError::Unsupported { op: "atomic_add", .. })` | `Err(ContractError::Unsupported { .. })`（`op` は `"query_route"`） |

「今の値の補い（base 515bb19）」の `subgroup_sum_f32` の行の `Route::Native`・`Route::GpuEmulated` の返しは、今も `subgroup_sum_f32` に当たる。`Exec::query_route` の rustdoc が `report` と等しいと書く行は `crates/exec/src/entry.rs` の 204 行 → 187 行。`atomic_add` で `Ran` の報告との一致を照らす試験は、`entry::tests::atomic_add_with_cpu_override_runs_on_host`（`entry.rs`）、`route_query.rs` の `query_route_matches_entry_report` の `Ran` の腕（`AtomicAdd` を含む）。`query_route` が `Route::GpuEmulated` の `SubgroupSumF32`・`AtomicAdd` で `Unsupported` になることは `route_query.rs` の `unreachable_native_follows_opt_in_order` が照らす。

### 完了の条件の確かめ（5/5）の値

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| (1) の `forbid` の行 | `21:#![forbid(unsafe_code)]` → `22:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけで、ほか 12 ファイルは 0 件 | `grep -n forbid crates/exec/src/lib.rs`、`grep -rc unsafe crates/exec/src` |
| (2) の選び方の関数と呼び出し元 | 定義 `select.rs:69` のまま。本体側の呼び出し `entry.rs:178` → `entry.rs:177` の `let output = select_route(&input, judge)?;`。取り込みは `entry.rs:24` → `entry.rs:23`。`select.rs` の試験の中の出現は `220`・`243`・`258`・`274`・`330`・`826` のまま | `grep -rn 'select_route' crates/exec/src` |
| (3) の `RefCell` と `&'static` の行 | 14 行 → 13 行（`entry.rs` の `&'static str` が 1 行減った）。`entry.rs` の `&'static str` は `155`・`192`・`228`・`244` 行 → `154`・`212`・`233` 行。ほかの行（`select.rs:320`・`329`、`denormal_tests.rs:24`・`122`、`denormal.rs:10`・`107`・`193`・`243`・`245`・`291`）は変わらない。`std::env`・`env!`・`option_env!`・`std::fs`・`&mut self` の行は無い | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|Cell\|&mut self' crates/exec/src` |
| (4) の組み合わせの数の行 | `660:            assert_eq!(checked, 5720);` のまま。`every_combination_matches_candidate_rule` は feature なしでも `--features fault-injection` でも 1 passed | `grep -n 'assert_eq!(checked, 5720)' crates/exec/src/select.rs`、`cargo test -p fandhe-silicon-exec --locked every_combination`（feature なしと `--features fault-injection`） |

### 試験の件数

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 114 passed → 合計 123 passed | 69 + 4 + 3 + 5 + 6 + 4 + 5 + 9 + 4 + 3 + 6 + 5 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 121 passed → 合計 132 passed | 70 + 4 + 3 + 5 + 4 + 4 + 6 + 4 + 5 + 9 + 4 + 3 + 6 + 5 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 148 passed → 合計 158 passed | 92 + 5 + 4 + 6 + 8 + 6 + 4 + 5 + 9 + 4 + 4 + 6 + 5 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 155 passed → 合計 167 passed | 93 + 5 + 4 + 6 + 4 + 8 + 4 + 6 + 4 + 5 + 9 + 4 + 4 + 6 + 5 + doctest 0 |

内訳の並びは [exec-skeleton-types.md](exec-skeleton-types.md) の「今の値の補い（base 8e4b99b）」の試験の件数の表と同じ。`--lib` は feature なしで 68 passed → 69 passed、`native-blocking-wait` 付きで 69 passed → 70 passed、`fault-injection` 付きで 91 passed → 92 passed、`--all-features` で 92 passed → 93 passed。`route_query` は 6 passed のまま。
