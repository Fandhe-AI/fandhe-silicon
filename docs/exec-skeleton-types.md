# 下-2 の公開の型の所在（TASK-919）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-919。
親の Issue は #863。

## 本リポの現状の事実

記録時点は origin/main `7894f38`。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| `crates/exec/Cargo.toml` の行数 | 16 行 | `wc -l crates/exec/Cargo.toml` |
| `crates/exec/src/lib.rs` の行数 | 5 行（`//!` 3 行と `#![forbid(unsafe_code)]`） | `git show 7894f38:crates/exec/src/lib.rs \| wc -l` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |
| upper からの参照 | `crates/upper/Cargo.toml` が exec に依存する。`crates/upper/src/lib.rs` は 5 行で exec の項目を使っていない | `grep -rn fandhe_silicon_exec crates/upper/src` |

補い: 上の表の `crates/exec/Cargo.toml` の行のコマンド `wc -l crates/exec/Cargo.toml` は、いまは 29 行を返す。記録時点の行数は `git show 7894f38:crates/exec/Cargo.toml | wc -l`（16 行）で確かめる。

補い: 上の表の `crates/upper/src/lib.rs` の行（5 行）は記録した時点の値で、書き換えていない。いまは 19 行で、`pub use` で `UpperDevice`・`Queue`・`CommandEncoder`・`CommandBuffer`・`SubmitId`・`ComputePass`・`Buffer` を公開する（[upper-compute-path](upper-compute-path.md)）。いまも `crates/upper/src` は `fandhe_silicon_exec` の項目を使っていない。

## opt-in の印と経路・操作・理由の型（1/7）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/exec/src/opt_in.rs`・`crates/exec/src/route.rs` |
| 公開のパス | `fandhe_silicon_exec::{OptIn, Route, OpId, UnavailableReason}` |
| feature の条件 | なし |

| 型 | 値または欄 | derive | `#[non_exhaustive]` |
| -- | ---------- | ------ | ------------------- |
| `OptIn` | 欄 `allow_gpu_emulated: bool`・`allow_cpu: bool`（`Default` はどちらも `false`） | `Debug, Clone, Copy, PartialEq, Eq, Hash, Default` | なし |
| `Route` | `Native`・`GpuEmulated`・`Cpu` | `Debug, Clone, Copy, PartialEq, Eq, Hash` | なし |
| `OpId` | `F16Fma`・`SubgroupSumU32`・`SubgroupSumF32`・`AtomicAdd` | `Debug, Clone, Copy, PartialEq, Eq, Hash` | あり |
| `UnavailableReason` | `CapabilityUnsupported(Option<UnsupportedReason>)`・`CapabilityUnknown`・`OptInNotGiven`・`SemanticsNotPreservable`・`F32DenormalBehaviorUnknown` | `Debug, Clone, Copy, PartialEq, Eq` | あり |

`UnsupportedReason` は `fandhe_silicon_core::error::UnsupportedReason`。

確かめ方:

```text
cargo test -p fandhe-silicon-exec --lib
```

## 経路の報告と結果の値の型（2/7）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/exec/src/report.rs` |
| 公開のパス | `fandhe_silicon_exec::{RouteReport, Outcome, F32DenormalReport, F32DenormalBehavior, F32DenormalBasis}` |
| feature の条件 | なし |

| 型 | 値または欄 | derive | `#[non_exhaustive]` |
| -- | ---------- | ------ | ------------------- |
| `F32DenormalBehavior` | `Preserve`・`FlushToZero` | `Debug, Clone, Copy, PartialEq, Eq, Hash` | なし |
| `F32DenormalBasis` | `Observed` | `Debug, Clone, Copy, PartialEq, Eq, Hash` | あり |
| `F32DenormalReport` | 欄 `behavior`・`basis`・`documented` | `Debug, Clone, Copy, PartialEq, Eq` | あり |
| `RouteReport` | 下の 6 欄 | `Debug, Clone, Copy, PartialEq, Eq` | あり |
| `Outcome` | `Ran { report }`・`Unavailable { report }`（`#[must_use]`） | `Debug, Clone, Copy, PartialEq, Eq` | なし |

`RouteReport` の欄:

| 欄 | 型 | 表すもの |
| -- | -- | -------- |
| `op` | `OpId` | 対象の操作 |
| `route` | `Option<Route>` | 選ばれた経路。`None` は経路が無いこと |
| `native_reason` | `Option<UnavailableReason>` | device の機能をそのまま使えない理由 |
| `fallback_reason` | `Option<UnavailableReason>` | 代わりの実行が使えない理由 |
| `observed_level` | `Option<CapabilityLevel>` | 観察した能力の段階（`fandhe_silicon_core::capability::CapabilityLevel`） |
| `f32_denormal` | `Option<F32DenormalReport>` | f32 の非正規化数の項 |

`Outcome` は 2 つの場合のどちらでも `report()` で報告を返す。

確かめ方:

```text
cargo test -p fandhe-silicon-exec --lib
```

## 列の渡し方の型（3/7）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/exec/src/operands.rs` |
| 公開のパス | `fandhe_silicon_exec::{F16FmaOperands, SubgroupSumU32Operands, SubgroupSumF32Operands, AtomicAddOperands}` |
| feature の条件 | `Device` の形は `native-blocking-wait` のときだけ在る |

4 型はどれも `enum` で、derive は `Debug`、`#[non_exhaustive]` はなし。`Host` の欄はホストのスライス、`Device` の欄は `fandhe_silicon_core::memory::BufferHandle` の排他の借用（`&mut`）。

| 型 | `Host` の欄 | `Device` の欄 |
| -- | ----------- | ------------- |
| `F16FmaOperands<'a>` | `a`・`b`・`c`: `&[u16]`、`out`: `&mut [u16]`（f16 はビット列） | `a`・`b`・`c`・`out` |
| `SubgroupSumU32Operands<'a>` | `x`: `&[u32]`、`out`: `&mut [u32]` | `x`・`out` |
| `SubgroupSumF32Operands<'a>` | `x`: `&[f32]`、`out`: `&mut [f32]` | `x`・`out` |
| `AtomicAddOperands<'a>` | `x`: `&[f32]`、`out`: `&mut [f32]` | `x`・`out` |

| feature | 転送先 | 既定 |
| ------- | ------ | ---- |
| `native-blocking-wait` | `fandhe-silicon-core/native-blocking-wait` | 無効 |

確かめ方:

```text
cargo test -p fandhe-silicon-exec --lib
cargo test -p fandhe-silicon-exec --lib --features native-blocking-wait
```

## 下-2 の値と 7 つの入口（4/7）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/exec/src/entry.rs`・`crates/exec/src/select.rs`（選び方と報告の組み立ては crate の中だけの関数で公開しない） |
| 公開のパス | `fandhe_silicon_exec::Exec` |
| feature の条件 | なし（入口はどのビルドにも在る） |

### `Exec` の作り方と既定の読み方

| 関数 | 動き |
| ---- | ---- |
| `Exec::new(device, default_opt_in)` | device を借り、既定の印を持つ値を作る |
| `Exec::deny_by_default(device)` | 既定の印を `OptIn::default()`（どちらも許さない）にして作る |
| `default_opt_in(&self)` | 作るときに決めた既定の印を返す |

作った後に既定を変える関数は無い。入口の `opt_in` に `Some` を渡すと、その呼び出しの印が既定を丸ごと置き換える（欄ごとには混ぜない）。`None` は既定の印を使う。

### 7 つの入口

どれも `&self` の関数で、戻り値は `Result<Outcome, ContractError>`。

| 入口 | 引数（`opt_in: Option<OptIn>` の後） | 能力の鍵 |
| ---- | ------------------------------------ | -------- |
| `f16_fma` | `n: u64`・`F16FmaOperands` | `FEATURE_SHADER_F16` |
| `subgroup_sum_u32` | `n: u64`・`SubgroupSumU32Operands` | `FEATURE_SUBGROUP` |
| `subgroup_sum_f32` | `n: u64`・`SubgroupSumF32Operands` | `FEATURE_SUBGROUP` |
| `atomic_add` | `n: u64`・`m: u64`・`AtomicAddOperands` | `FEATURE_SHADER_FLOAT32_ATOMIC` |
| `external_interop_shared_memory` | なし | なし |
| `hardware_ray_tracing` | なし | なし |
| `unified_memory_sharing` | なし | なし |

### 4 つの入口の動き

1. 効く印を決める（上書きの `Some` ならその印、`None` なら既定）。
2. 能力の鍵で device の `query_ex` を呼ぶ（入口 1 回につき 1 回）。
3. 経路の届き方を決める。

| 条件 | 経路 | `native_reason` | `fallback_reason` |
| ---- | ---- | --------------- | ----------------- |
| 段階が `S2`〜`S6` | `Native` | なし | なし |
| 段階が `Unsupported` | 下の順 | `CapabilityUnsupported(問合せの理由)` | 下の順 |
| 段階が `S0`・`S1` とそれ以外 | 下の順 | `CapabilityUnknown` | 下の順 |

`Native` が届かないときは、`allow_gpu_emulated` が `true` なら `GpuEmulated`、そうでなく `allow_cpu` が `true` なら `Cpu`、どちらも `false` なら経路なしで `fallback_reason` は `OptInNotGiven`。

4. 報告を組み立てる。`observed_level` は問合せの段階。`f32_denormal` はいつも `None`。
5. 経路なしなら `Ok(Outcome::Unavailable { report })` を返す。
6. 経路が決まれば `Err(ContractError::Unsupported { .. })` を返す（`op` は入口の関数名、`backend` は device の背後実装。`crates/exec/tests/unsupported_fields.rs` が 7 つの入口で固定する）。

3 つの入口は能力を問い合わせず、`opt_in` の値によらず `Err(ContractError::Unsupported { .. })` を返す。

どの入口も `n`・`m`・`operands` を読まず、出力の列・buffer に書かない。`Outcome::Ran` は返さない。

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --lib
cargo test -p fandhe-silicon-exec --lib --features native-blocking-wait
```

## opt-in の既定と呼び出しごとの上書きの試験（5/7）

公開のパスだけを使う統合テストで、`Exec` の 7 つの入口の動きを確かめる。本体のコードは変えない。

### 置き場

| パス | 役割 |
| ---- | ---- |
| `crates/exec/tests/common/mod.rs` | 試験用の device `Scripted`。能力の問合せの値を差し込み、`query_ex`・`memory`・`queue` の呼び出し回数を数える |
| `crates/exec/tests/opt_in_default.rs` | host の形の列での入口の試験（5 本） |
| `crates/exec/tests/device_operands.rs` | device の形の列での入口の試験（1 本）。`Cargo.toml` の `[[test]]` で `native-blocking-wait` を要求する |

### 試験用の device

| device | 能力の問合せの値 |
| ------ | ---------------- |
| `Scripted::bare()` | 何も差し込まない。4 つの鍵とも `Unsupported`（理由 `UnknownFeature`） |
| `Scripted::insufficient(鍵)` | 指定した鍵だけ `Unsupported`（理由 `NotSupported`）。入口ごとにその入口の鍵で作る |
| `Scripted::query_fails()` | 4 つの鍵とも `S0` |

補い: 能力の鍵は 3 つ（`FEATURE_SHADER_F16`・`FEATURE_SUBGROUP`・`FEATURE_SHADER_FLOAT32_ATOMIC`）で、`subgroup_sum_u32` と `subgroup_sum_f32` が `FEATURE_SUBGROUP` を共有する。上の表の「4 つの鍵とも」は、4 つの入口が使う鍵（3 つ）とも、と読む。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `deny_default_without_override_is_unavailable` | 上の 3 つの device・`deny_by_default`・上書きなしで 4 入口 | `Ok(Outcome::Unavailable)`。`route` は `None`、`fallback_reason` は `OptInNotGiven`、`native_reason`・`observed_level` は device ごとの値。出力の列は不変 |
| `override_applies_to_that_call_only` | 既定が拒否で C を許す上書き、既定が両方許すで拒否の上書き | 上書きはその呼び出しだけに効き、`default_opt_in()` は変わらない |
| `each_flag_reaches_route_choice` | 非対応の 3 つの device・上書き G だけ・C だけ・両方・なし | G・C・両方は Unavailable でなく、なしは `OptInNotGiven` の Unavailable |
| `chosen_route_does_not_write_output` | `bare`・C を許す上書きで 4 入口。3 入口は上書き 5 通り | `Err(ContractError::Unsupported)`。出力の列は不変 |
| `entries_query_once_and_skip_memory_and_queue` | 呼び出し回数の計数 | 4 入口は 1 回につき `query_ex` 1 回、3 入口は 0 回。どれも `memory`・`queue` を呼ばない |
| `unavailable_leaves_device_buffers_unchanged` | device の形の列（`f16_fma`・`atomic_add`）・上書きなし | Unavailable で、全 buffer の中身が不変 |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --test opt_in_default
cargo test -p fandhe-silicon-exec --features native-blocking-wait --test device_operands
```

## 理由の値の区別と f32 の非正規化数の項の試験（6/7）

公開のパスだけを使う統合テストで、使えない理由の値の区別と、経路の無い報告での f32 の非正規化数の項を確かめる。本体のコードは変えない。

### 置き場

| パス | 役割 |
| ---- | ---- |
| `crates/exec/tests/reasons_and_denormal.rs` | 理由の値と報告の欄の試験（4 本）。試験用の device は `crates/exec/tests/common/mod.rs` の `Scripted` を使う |

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `reasons_are_distinct` | `UnavailableReason` の 8 値（`CapabilityUnsupported` は理由なし・`NoApi`・`NotSupported`・`UnknownFeature` の 4 通り） | どの 2 つも等しくない |
| `unavailable_report_reasons_are_told_apart` | `bare` と `query_fails` の device・`deny_by_default`・上書きなしで 4 入口 | どちらも `Unavailable`。`native_reason` は前者が `CapabilityUnsupported(_)`、後者が `CapabilityUnknown` で等しくない。`fallback_reason` はどちらも `OptInNotGiven` |
| `f32_denormal_absent_on_unavailable` | `bare`・`insufficient(FEATURE_SUBGROUP)`・`query_fails` の device・上書きなしと既定の `OptIn` で 4 入口 | どれも `Unavailable` で、`f32_denormal` は `None`。`SubgroupSumF32`・`AtomicAdd` を含む |
| `report_items_are_readable` | `bare` の device・上書きなしで 4 入口 | `Outcome::report` から 6 欄を読め、`route` は `None`、`native_reason` は `CapabilityUnsupported(Some(UnknownFeature))`、`fallback_reason` は `OptInNotGiven`、`observed_level` は `Unsupported`、`f32_denormal` は `None` |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --test reasons_and_denormal
```

## `Unsupported` の `op` と `backend` の欄の試験

公開のパスだけを使う統合テストで、経路が決まった入口と能力を問い合わせない入口が返す `ContractError::Unsupported` の欄を確かめる。本体のコードは変えない。

| パス | 試験 | 入力 | 期待 |
| ---- | ---- | ---- | ---- |
| `crates/exec/tests/unsupported_fields.rs` | `unsupported_carries_entry_name_and_device_backend` | `Scripted::bare()`・`deny_by_default`。4 入口は C を許す上書き、3 入口は上書きなし | 7 入口とも `Err(ContractError::Unsupported { op, backend })`。`op` は入口の関数名（`f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add`・`external_interop_shared_memory`・`hardware_ray_tracing`・`unified_memory_sharing`）、`backend` は `Device::backend()` の値（この device は `BackendKind::Metal`） |

確かめ方:

```text
cargo test -p fandhe-silicon-exec --test unsupported_fields
```

## 完了の条件の確かめ（7/7）

7/7（#5993）で、下-2 の crate（`crates/exec`）について本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-919（ポインタのみ）。GPU の実機の無い Linux で実行した。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 依存と unsafe | `cargo metadata --no-deps` の `fandhe-silicon-exec` の依存、`grep -rc unsafe crates/exec/src`、`grep -n forbid crates/exec/src/lib.rs`、`make upper-layer-check`、`make deps-check` | 依存は `fandhe-silicon-core` だけ（normal は feature なし、dev は `test-support`・`fault-injection` 付き）。`unsafe` の出現は `lib.rs` の 1 件だけ（ほか 6 ファイルは 0 件）で、その行は `13:#![forbid(unsafe_code)]`。`make upper-layer-check` の `record:` の行は `rs_files=4 direct_deps=2 forbid=yes unsafe_total=0`（upper の crate の記録の行）。`make deps-check` は終了コード 0 |
| (2) | 公開の型と入口 | `grep -rn 'pub fn\|pub struct\|pub enum' crates/exec/src` | 25 行。`pub struct`・`pub enum` が 14 行、`pub fn` が 11 行 |
| (3) | 既定を変える操作・環境・ファイル・`static` | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|&mut self' crates/exec/src` | 2 行だけ。`entry.rs:57` の `name: &'static str,` と `entry.rs:71` の `fn unsupported(&self, op: &'static str) -> ContractError {` |
| (4) | 統合テストと lib の件数 | `cargo test -p fandhe-silicon-exec --test opt_in_default` / `--test reasons_and_denormal` / `--lib` / `--lib --features native-blocking-wait` / `--features native-blocking-wait --test device_operands` | `opt_in_default` は 5 passed; 0 failed; 0 ignored。`reasons_and_denormal` は 4 passed; 0 failed; 0 ignored。`--lib` は 17 passed、`native-blocking-wait` 付きは 18 passed（どちらも 0 failed; 0 ignored）。`device_operands` は 1 passed; 0 failed; 0 ignored |
| (5) | feature なし・`native-blocking-wait`・wasm32 | `cargo clippy -p fandhe-silicon-exec --all-targets -- -D warnings`（feature なしと `--features native-blocking-wait`）、`cargo test -p fandhe-silicon-exec`（同）、`cargo check -p fandhe-silicon-exec --target wasm32-unknown-unknown`（`--tests` 付きも） | clippy は両方とも終了コード 0。`cargo test` は feature なしが終了コード 0 で合計 26 passed（17 + 5 + 4 + doctest 0）、`native-blocking-wait` 付きが終了コード 0 で合計 28 passed（18 + 1 + 5 + 4 + doctest 0）。いずれも 0 failed; 0 ignored。wasm32 の `cargo check` は `--tests` の有無とも終了コード 0 |
| (6) | 公開名・公開 API の検査 | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check` の `scanned: crate=fandhe_silicon_exec` の行 | 3 つとも終了コード 0。`public-api-check` は production で `public_items=457 wgpu_type_refs=0 wgpu_other_refs=0`、all-features で `public_items=471 wgpu_type_refs=0 wgpu_other_refs=0`。`public-name-check` は production で `public_items=402 named_items=122 name_hits=0`、all-features で `public_items=416 named_items=136 name_hits=0`。`public-api-sdk-check` は両方とも `sdk_type_refs=0 sdk_other_refs=0 external_crates=alloc,core,fandhe_silicon_core` |

補い: 上の表 (4)(5) の件数は記録した時点の値で、書き換えていない。`unsupported_fields` の 1 本が加わり、いま `cargo test -p fandhe-silicon-exec` は feature なしで合計 27 passed、`native-blocking-wait` 付きで合計 29 passed（いずれも 0 failed; 0 ignored）。

補い: exec の依存の向きは `make exec-deps-check`（[exec-deps-check.md](exec-deps-check.md)）が見る。

確かめた時点は main の `ac32709`、`docs/spec` の gitlink は `d15921d873c6ce20039ecc7eb25a799d25e7748a`。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-919）。

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/7 | #5987 | feat(exec): TASK-919 の 1/7 opt-in の印と経路・操作・理由の型 |
| 2/7 | #5988 | feat(exec): TASK-919 の 2/7 経路の報告と結果の値の型 |
| 3/7 | #5989 | feat(exec): TASK-919 の 3/7 列の渡し方の型 |
| 4/7 | #5990 | feat(exec): TASK-919 の 4/7 下-2 の値と 7 つの入口 |
| 5/7 | #5991 | test(exec): TASK-919 の 5/7 opt-in の既定と呼び出しごとの上書きの試験 |
| 6/7 | #5992 | test(exec): TASK-919 の 6/7 理由の値の区別と f32 の非正規化数の項の試験 |
| 7/7 | #5993 | docs: TASK-919 の 7/7 完了の条件の確かめ |
