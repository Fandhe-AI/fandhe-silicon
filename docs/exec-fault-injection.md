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

## 3 つの口を公開のパスから使う試験と使い方（4/6）

### 所在

- `crates/exec/tests/fault_injection_ports.rs`（`crates/exec/Cargo.toml` の `[[test]]` で `required-features = ["fault-injection"]`）
- 試験用の device の構築の関数 `Scripted::overriding(feature, query)`（`crates/exec/tests/common/mod.rs`。`FaultScript::CapabilityOverride` で包む）

3 つの口は、能力の問合せの値の差し替え（`FaultScript::CapabilityOverride`）、`Exec::force_unsupported`、`Exec::inject_denormal`。
試験は crate の外の統合テストなので、公開のパスだけを使う。

### 使い方

1. dev-dependency で `fandhe-silicon-exec` に feature `fault-injection` を付ける（core の同名 feature へ転送される。`StubDevice` を使うなら core の `test-support` も付ける）。
2. 試験の device を `FaultInjecting` と `FaultScript::CapabilityOverride { feature, query }` で包み、試験の型が `Capabilities` と `Device` を実装する（`Scripted` が例）。差し替えるのは指定した鍵だけで、ほかの鍵は内側の値のまま。
3. `Exec::new`・`Exec::deny_by_default` の後に `force_unsupported`・`inject_denormal` をつなぐ。
4. 報告を読む。`native_reason` は `ForcedUnsupported`（指定）、`CapabilityUnsupported`・`CapabilityUnknown`（能力の値）。`observed_level` は device が返した段階。`f32_denormal.basis` は差し込みで経路が決まったとき `Injected`。

### 試験

| 試験 | device | 入力 | 期待 |
| ---- | ------ | ---- | ---- |
| `force_applies_to_the_target_op_only` | `answering(S2)` | 4 操作それぞれを対象に指定・上書き 5 通り。判定を求める操作には `Native`・`GpuEmulated` に `Preserve` | 対象は GpuEmulated だけを許す上書きで `GpuEmulated`・`ForcedUnsupported`・`observed_level == Some(S2)`。ほかの 3 操作は `Native`・`native_reason == None` |
| `force_without_injection_keeps_unsupported_for_f32_ops` | `answering(S2)` | `F16Fma` だけ指定・差し込み無し | `SubgroupSumF32`・`AtomicAdd` の `query_route` が `Err(Unsupported { op: "query_route", .. })` |
| `forced_op_follows_opt_in` | `answering` で `S0`〜`S6`・`Unsupported`（理由 `None`・`NotSupported`） | 4 操作を指定・上書き 4 通り | GpuEmulated を許す印は `GpuEmulated`（Cpu も許す印を含む）、Cpu だけを許す印は `Cpu`、どちらも許さない印は経路 `None`・`OptInNotGiven`。`native_reason` はいつも `ForcedUnsupported`、`observed_level` は渡した段階。入口は経路なしで `Ok(Unavailable)`（`query_route` と等しい）、経路ありで `Err(Unsupported { op: <入口の関数名>, .. })` |
| `override_reaches_route_choice` | `overriding(FEATURE_SUBGROUP, 値)` | 値は `S0`〜`S6`・`Unsupported`（理由 4 通り）・根拠付き | `SubgroupSumU32` は `S2`〜`S6` で `Native`、`S0`・`S1` で `CapabilityUnknown`、`Unsupported` で `CapabilityUnsupported(渡した理由)`。`F16Fma` は内側の値（`Unsupported`・`UnknownFeature`）。`query_ex` は呼び出しごとに 1 回 |
| `injection_reaches_reports` | `answering(S2)` | `SubgroupSumF32`・`AtomicAdd`。`Native` が `Unknown` で `GpuEmulated` が `FlushToZero`／`Native` が `Preserve`／全経路が `Unknown` | `GpuEmulated`・`F32DenormalBehaviorUnknown`・`{ FlushToZero, Injected, None }`／`Native`・`{ Preserve, Injected, None }`／経路 `None`・`F32DenormalBehaviorUnknown`（入口は `Ok(Unavailable)` で同じ報告） |
| `ports_combine` | `overriding(FEATURE_SUBGROUP, S4)` | `Exec::new(.., 両方を許す印)` に 3 つの口を別々の操作へ | `SubgroupSumU32` は `S4` と `ForcedUnsupported` が同時に出る。`SubgroupSumF32` は `Native`・`Injected`。`AtomicAdd`（差し替えの外）は内側の値で `Cpu`・`Injected`。`F16Fma` は内側の値。`query_ex` は 4 回 |
| `ports_do_not_touch_three_entries` | `answering(S2)` | 4 操作を指定し 4 x 3 の全組へ差し込み・上書き 5 通り | 3 入口が `Err(Unsupported { op: <関数名>, .. })`・`query_ex`／`memory`／`queue` が 0 回 |
| `ports_keep_query_ex_once` | 呼び出しごとに作る `overriding(FEATURE_SUBGROUP, S2)` | 指定のみ・差し込みのみ・両方 × 4 操作 × 上書き 5 通り × `query_route`・入口 | `query_ex` 1 回・`memory`／`queue` 0 回 |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --features fault-injection --test fault_injection_ports
```

## build-profiles と wasm-verify の feature の組（5/6）

### 所在

- `scripts/build-profiles.sh`: exec だけのブロック（native の clippy・test）と、wasm32 の check の行
- `scripts/wasm-verify.sh`: exec の試験の wasm32 の型検査の行

### 足した組

| スクリプト | target | コマンド |
| ---------- | ------ | -------- |
| `build-profiles.sh` | native | `cargo clippy -p fandhe-silicon-exec --all-targets --features fault-injection -- -D warnings` |
| `build-profiles.sh` | native | `cargo test -p fandhe-silicon-exec --features fault-injection` |
| `build-profiles.sh` | wasm32-unknown-unknown | `cargo check -p fandhe-silicon-exec --target wasm32-unknown-unknown --features fault-injection` |
| `wasm-verify.sh` | wasm32-unknown-unknown | `cargo check -p fandhe-silicon-exec --tests --features fault-injection --locked --target wasm32-unknown-unknown` |

- upper の組には足していない。`crates/upper/Cargo.toml` の `[features]` は `native-blocking-wait` だけで、exec の `fault-injection` を転送しない
- 既存の exec と upper の組（feature なし・`native-blocking-wait`）は変えていない

### 確かめ方

```text
make build-profiles
make wasm-verify
cargo clippy -p fandhe-silicon-exec --all-targets --features fault-injection -- -D warnings
cargo test -p fandhe-silicon-exec --features fault-injection
cargo check -p fandhe-silicon-exec --target wasm32-unknown-unknown --features fault-injection
cargo check -p fandhe-silicon-exec --tests --features fault-injection --locked --target wasm32-unknown-unknown
```

## 完了の条件の確かめ（6/6）

6/6（#6006）で、下-2 の crate（`crates/exec`）と core の試験用の口について、本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-921（ポインタのみ）。GPU の実機の無い Linux で実行した。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 3 つの口を試験から使う | `cargo test -p fandhe-silicon-core --features fault-injection --test fault_decorator_capability_override`、`cargo test -p fandhe-silicon-exec --features fault-injection --test fault_injection_ports`、`cargo test -p fandhe-silicon-exec --features fault-injection --lib` | 順に 3 passed、8 passed、43 passed。3 本とも 0 failed; 0 ignored |
| (2) | 口が feature の下にある | `grep -rn 'feature = "fault-injection"' crates/exec/src crates/exec/Cargo.toml`、`grep -n 'fault-injection' crates/exec/Cargo.toml`、`grep -n '^default' crates/exec/Cargo.toml` | 1 つ目は 20 行で、`select.rs`（28・47・56・77・734 行）・`route.rs`（48・82 行）・`lib.rs`（21・34 行）・`report.rs`（25・30 行）・`entry.rs`（43・53・69・86・117・123・134・546・652 行）。`Cargo.toml` の行は無い。2 つ目は `Cargo.toml` の 19 行（`fault-injection = ["fandhe-silicon-core/fault-injection"]`）・23 行（dev-dependencies の core の features）・30 行（コメント）・33 行（`required-features = ["fault-injection"]`）。3 つ目は出力が空（終了コード 1） |
| (3) | 報告の欄 | `cargo test -p fandhe-silicon-exec --features fault-injection --test fault_injection_ports` と `--lib` の `ok` の行 | `ForcedUnsupported` の試験: `test entry::forced_tests::forced_entry_reports_actual_level ... ok`、`test force_applies_to_the_target_op_only ... ok`、`test forced_op_follows_opt_in ... ok`。`Injected` の試験: `test entry::injected_tests::injected_fallback_route_is_reported ... ok`、`test injection_reaches_reports ... ok` |
| (4) | 依存・unsafe・差分 | `cargo metadata --no-deps` の `fandhe-silicon-exec` の依存、`grep -rc unsafe crates/exec/src`、`grep -n forbid crates/exec/src/lib.rs`、`git diff --stat 2085eb6 -- Cargo.lock deps-approved.toml`、`make deps-check`、`make exec-deps-check` | 依存は `fandhe-silicon-core`（normal）と `fandhe-silicon-core`（dev）の 2 件。`unsafe` の数は `lib.rs` が 1、ほかの 7 ファイルが 0（`lib.rs` の 1 行は `lib.rs:18` の `#![forbid(unsafe_code)]`）。`git diff --stat` の出力は空。`make deps-check` は終了コード 0 で `ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml`。`make exec-deps-check` は終了コード 0 で `ok: 15 .rs file(s), 2 declared dependenc(ies) and 1 tree package(s) checked, other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0` |
| (5) | feature の 4 組と wasm32 | exec の 4 組（feature なし・`--features fault-injection`・`--features native-blocking-wait`・`--all-features`）それぞれで `cargo clippy -p fandhe-silicon-exec --all-targets <組> -- -D warnings` と `cargo test -p fandhe-silicon-exec <組>`、`cargo check -p fandhe-silicon-exec --tests --target wasm32-unknown-unknown`（feature なしと `--features fault-injection`）、`cargo test -p fandhe-silicon-core --features fault-injection` | clippy は 4 組とも終了コード 0。test の合計は feature なし 46（lib 30・`opt_in_default` 5・`reasons_and_denormal` 4・`route_query` 6・`unsupported_fields` 1）、`--features fault-injection` 67（lib 43・`fault_injection_ports` 8・`opt_in_default` 5・`reasons_and_denormal` 4・`route_query` 6・`unsupported_fields` 1）、`--features native-blocking-wait` 48（lib 31・`device_operands` 1・`opt_in_default` 5・`reasons_and_denormal` 4・`route_query` 6・`unsupported_fields` 1）、`--all-features` 69（lib 44・`device_operands` 1・`fault_injection_ports` 8・`opt_in_default` 5・`reasons_and_denormal` 4・`route_query` 6・`unsupported_fields` 1）。doctest は 4 組とも 0 passed。すべて 0 failed; 0 ignored。wasm32 の check は 2 組とも終了コード 0。core の `--features fault-injection` の test は合計 856 passed; 0 failed |
| (6) | 公開名・公開 API の検査 | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check` の `scanned: crate=fandhe_silicon_exec` と `scanned: crate=fandhe_silicon_core` の行 | 3 つとも終了コード 0。`public-api-check`: exec は production の `public_items=458`・all-features の `public_items=511`、core は production の `public_items=4174`・all-features の `public_items=8850`、4 行とも `wgpu_type_refs=0 wgpu_other_refs=0`。`public-name-check`: exec は production の `public_items=403 named_items=123`・all-features の `public_items=451 named_items=149`、core は production の `public_items=3714 named_items=1530`・all-features の `public_items=7882 named_items=3014`、4 行とも `name_hits=0`。`public-api-sdk-check`: exec と core の production・all-features の 4 行とも `sdk_type_refs=0 sdk_other_refs=0` |

補い: 上の表 (5) の `device_operands` は `native-blocking-wait` の下の試験で、feature なしと `--features fault-injection` では流れない。`fault_injection_ports` は `required-features = ["fault-injection"]` の試験で、`--features native-blocking-wait` では流れない。

確かめた時点は main の `1d7d7b1`、`docs/spec` の gitlink は `c7887a1a7f75903d4016ba42e0595ae6cbb2cd0b`。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-921）。

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/6 | #6001 | feat(core): TASK-921 の 1/6 能力の問合せの値を差し替える筋書き |
| 2/6 | #6002 | feat(exec): TASK-921 の 2/6 試験用の feature と操作ごとに Native を外す口 |
| 3/6 | #6003 | feat(exec): TASK-921 の 3/6 f32 の非正規化数の判定を差し込む口 |
| 4/6 | #6004 | test(exec): TASK-921 の 4/6 3 つの口を公開のパスから使う試験と使い方 |
| 5/6 | #6005 | ci(scripts): TASK-921 の 5/6 build-profiles と wasm-verify に exec の fault-injection の組を足す |
| 6/6 | #6006 | docs: TASK-921 の 6/6 完了の条件の確かめ |

## 今の値の補い（base dbbb5c0）

記録時点の表と既存の節は書き換えない。以下は base `dbbb5c0` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。

### 入口の返し

「`Exec::inject_denormal` の動き」の表の「入口の返し」の行と「3 つの入口・既定の印・`query_ex` の回数」の行は、次のとおり。

| 場面 | 動き |
| ---- | ---- |
| `f16_fma`・経路が `Route::Cpu`・host の形 | `Ok(Outcome::Ran { report })`。`report` は `query_route` の報告と等しく、出力の列が書き換わる（`fault_injection_ports.rs` の `forced_op_follows_opt_in`） |
| 経路なし | `Ok(Outcome::Unavailable { report })` |
| 経路が決まったそのほかの呼び出し | `Err(Unsupported { op: <入口の関数名>, .. })`。`query_route` は経路が決まれば `Ok(報告)` |
| `external_interop_shared_memory`・`hardware_ray_tracing`・`unified_memory_sharing`・既定の印・`query_ex` の回数 | 変わらない（`query_ex` は 1 回。3 つの入口は `force_unsupported` を重ねても入口の関数名の `Unsupported` を返す。`entry::forced_tests::force_does_not_touch_three_entries`） |

### 完了の条件の確かめ（6/6）の値

| 表の行 | 記録の値 → 今の値 | 測ったコマンド |
| ------ | ----------------- | -------------- |
| (1) | 順に 3 passed、8 passed、43 passed → 順に 3 passed、8 passed、85 passed（`--lib` は `fault-injection` 付き）。3 本とも 0 failed; 0 ignored | `cargo test -p fandhe-silicon-core --features fault-injection --test fault_decorator_capability_override`、`cargo test -p fandhe-silicon-exec --features fault-injection --test fault_injection_ports`、`cargo test -p fandhe-silicon-exec --features fault-injection --lib` |
| (2) の 1 つ目 | 20 行 → 24 行。`select.rs`（28・47・75・732 行）・`route.rs`（48・82 行）・`lib.rs`（28・42 行）・`report.rs`（25・30 行）・`entry.rs`（71・84・110・127・164・170・180・781・887 行）・`denormal_tests.rs`（299・404・579・823・996 行）。`Cargo.toml` の行は無い | `grep -rn 'feature = "fault-injection"' crates/exec/src crates/exec/Cargo.toml` |
| (2) の 2 つ目・3 つ目 | `Cargo.toml` の 19・23・30・33 行のまま。`^default` の出力は空（終了コード 1）のまま | `grep -n 'fault-injection' crates/exec/Cargo.toml`、`grep -n '^default' crates/exec/Cargo.toml` |
| (4) | `unsafe` の数は `lib.rs` が 1、ほかの 12 ファイルが 0。`lib.rs:20` の `#![forbid(unsafe_code)]`。`git diff --stat 2085eb6 -- Cargo.lock deps-approved.toml` の出力は空のまま。`make deps-check` は終了コード 0 で `ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml`。`make exec-deps-check` は終了コード 0 で `ok: 26 .rs file(s), 2 declared dependenc(ies) and 1 tree package(s) checked, other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0` | `grep -rc unsafe crates/exec/src`、`grep -n forbid crates/exec/src/lib.rs`、`make deps-check`、`make exec-deps-check` |
| (5) の test の合計 | feature なし 46 → 98、`--features fault-injection` 67 → 130、`--features native-blocking-wait` 48 → 103、`--all-features` 69 → 135。doctest は 4 組とも 0 passed | `cargo test -p fandhe-silicon-exec <組>` |

(5) の内訳。

| 組 | 内訳 |
| -- | ---- |
| feature なし | lib 63・`cpu_f16_fma` 3・`length_host` 2・`length_limit` 4・`length_zero` 5・`opt_in_default` 6・`reasons_and_denormal` 4・`route_evidence_calls` 3・`route_query` 6・`unsupported_fields` 2 |
| `--features fault-injection` | lib 85・`cpu_f16_fma` 4・`fault_injection_ports` 8・`length_host` 2・`length_limit` 4・`length_zero` 5・`opt_in_default` 6・`reasons_and_denormal` 4・`route_evidence_calls` 4・`route_query` 6・`unsupported_fields` 2 |
| `--features native-blocking-wait` | lib 64・`cpu_f16_fma` 3・`device_operands` 2・`length_device` 2・`length_host` 2・`length_limit` 4・`length_zero` 5・`opt_in_default` 6・`reasons_and_denormal` 4・`route_evidence_calls` 3・`route_query` 6・`unsupported_fields` 2 |
| `--all-features` | lib 86・`cpu_f16_fma` 4・`device_operands` 2・`fault_injection_ports` 8・`length_device` 2・`length_host` 2・`length_limit` 4・`length_zero` 5・`opt_in_default` 6・`reasons_and_denormal` 4・`route_evidence_calls` 4・`route_query` 6・`unsupported_fields` 2 |
