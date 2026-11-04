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

補い: 上の手順 1〜4 は `crates/exec/src/entry.rs` の内部の関数 `route_report` が通す。経路の選び方（手順 3 の段）は `crates/exec/src/select.rs` の `select_route` にある。入口の呼び出しの前に同じ報告を返す `Exec::query_route` と、判定を受ける経路の選び方は [exec-route-choice.md](exec-route-choice.md) にある。

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

## 今の値の補い（base c79863d）

上の確かめの節の値は記録した時点のもので、書き換えない。同じコマンドを今の値で流した結果は次のとおり。

| # | 項目 | 今の値 |
| - | ---- | ------ |
| (1) | `grep -n forbid crates/exec/src/lib.rs` | `14:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけ（ほか 6 ファイルは 0 件） |
| (2) | `grep -rn 'pub fn\|pub struct\|pub enum' crates/exec/src` | 26 行。`pub fn` が 12 行 |
| (3) | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|&mut self' crates/exec/src` | 2 行。`entry.rs:86` の `name: &'static str,` と `entry.rs:112` の `fn unsupported(&self, op: &'static str) -> ContractError {` |
| (4) | `--lib` / `--lib --features native-blocking-wait` | 30 passed / 31 passed（0 failed; 0 ignored）。`opt_in_default` は 5、`reasons_and_denormal` は 4、`unsupported_fields` は 1、`device_operands` は 1、`route_query`（統合テスト）は 6 passed |
| (5) | `cargo test -p fandhe-silicon-exec`（feature なし / `--features native-blocking-wait`） | 合計 46 passed（30 + 5 + 4 + 6 + 1 + doctest 0）/ 合計 48 passed（31 + 1 + 5 + 4 + 6 + 1 + doctest 0）。いずれも 0 failed |
| (6) | `scanned: crate=fandhe_silicon_exec` の行 | `public-api-check` は production で `public_items=458`、all-features で `public_items=472`（`wgpu_type_refs=0 wgpu_other_refs=0`）。`public-name-check` は production で `public_items=403 named_items=123`、all-features で `public_items=417 named_items=137`（`name_hits=0`）。`public-api-sdk-check` は production で `public_items=403`、all-features で `public_items=417`（`sdk_type_refs=0 sdk_other_refs=0`） |

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

## 今の値の補い（base 2f36d3d）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（7/7）の (1) の `make upper-layer-check` の `record:` | `rs_files=4 direct_deps=2 forbid=yes unsafe_total=0` → `rs_files=9 direct_deps=3 forbid=yes unsafe_total=0`（`chip_deps=0 chip_paths=0 other_deps=0` のまま。上の層の crate の記録の行）。上の層の crate の `.rs` が 5 個増え、直接の依存が 1 つ増えた | `make upper-layer-check \| grep '^record:'` |

## 今の値の補い（base 48025c6）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（7/7）の (1) の `forbid` の行 | `14:#![forbid(unsafe_code)]` → `18:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけで、ほか 7 ファイルは 0 件（`crates/exec/src` は 8 ファイル） | `grep -n forbid crates/exec/src/lib.rs` |
| 同 (2) の公開の型と入口 | 26 行 → 29 行（`pub fn` は 12 行 → 14 行） | `grep -rn 'pub fn\|pub struct\|pub enum' crates/exec/src` |
| 同 (3) の `&'static` の行 | `entry.rs:86`・`entry.rs:112` → `entry.rs:146` の `name: &'static str,` と `entry.rs:172` の `fn unsupported(&self, op: &'static str) -> ContractError {`（2 行のまま） | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|&mut self' crates/exec/src` |
| 同 (6) の `public-api-check` の all-features | `public_items=472` → `public_items=511`（production は `458` のまま。`wgpu_type_refs=0 wgpu_other_refs=0`） | `make public-api-check` |
| 同 (6) の `public-name-check` の all-features | `public_items=417 named_items=137` → `public_items=451 named_items=149`（production は `403`・`123` のまま。`name_hits=0`） | `make public-name-check` |
| 同 (6) の `public-api-sdk-check` の all-features | `public_items=417` → `public_items=451`（production は `403` のまま。`sdk_type_refs=0 sdk_other_refs=0`） | `make public-api-sdk-check` |
| `crates/exec/Cargo.toml` の行数 | 29 行 → 36 行（`[features]` に `fault-injection = ["fandhe-silicon-core/fault-injection"]` が加わり、`fault_injection_ports` の `[[test]]` が `required-features = ["fault-injection"]` で加わった） | `wc -l crates/exec/Cargo.toml` |

feature なしの値は変わらない。`cargo test -p fandhe-silicon-exec` は合計 46 passed、`--features native-blocking-wait` 付きは合計 48 passed（0 failed; 0 ignored）。

feature `fault-injection` の下では、上の「opt-in の印と経路・操作・理由の型（1/7）」の表の `UnavailableReason` の値に `ForcedUnsupported` が加わる（`crates/exec/src/route.rs` の `#[cfg(feature = "fault-injection")]` つきの値）。「経路の報告と結果の値の型（2/7）」の表の `F32DenormalBasis` の値は `Observed` だけだったところに `Injected` が加わる（`crates/exec/src/report.rs`。同じ `#[cfg(feature = "fault-injection")]`）。feature を付けないビルドの値は `Observed` だけ、`UnavailableReason` は 5 値のままである。`cargo test -p fandhe-silicon-exec --features fault-injection` は合計 67 passed（43 + 8 + 5 + 4 + 6 + 1 + doctest 0。0 failed; 0 ignored）。所在は [exec-fault-injection](exec-fault-injection.md)。

確かめ方:

```text
grep -n 'fault-injection' crates/exec/src/route.rs crates/exec/src/report.rs
cargo test -p fandhe-silicon-exec --features fault-injection
```

## 今の値の補い（base c956c96）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「今の値の補い（base 2f36d3d）」の `make upper-layer-check` の `record:` | `rs_files=9` → `rs_files=10`（`crates/upper/tests/upper_exec_same_device.rs` が加わった） | `make upper-layer-check \| grep '^record:'` |

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

記録時点の表と既存の節は書き換えない。以下は base `904126a` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。4 つの入口に長さの検査（`crates/exec/src/length.rs` の `check_lengths`。所在は [exec-length-check.md](exec-length-check.md)）が加わった（TASK-924）。

### 4 つの入口の動きへの補い

4 つの入口は、上の「4 つの入口の動き」の手順 1 より前に `check_lengths` を呼ぶ。`n` が 0、`atomic_add` の `m` が 0、必要バイト数が `u64` であふれる、入力・出力のどちらかの buffer のバイト数が device の `max_buffer_bytes` を超える、のどれかなら `Err(ContractError::InvalidArgument(..))` を返し、能力の問合せに進まない。通れば手順 1〜6 のとおり。上の「どの入口も `n`・`m`・`operands` を読まず」は、4 つの入口については `n`（と `atomic_add` の `m`）を読んで検査する形になった。`operands` は読まず、出力の列・buffer に書かない点は変わらない。3 つの入口（`external_interop_shared_memory`・`hardware_ray_tracing`・`unified_memory_sharing`）は長さを持たず、動きは変わらない。

| 入口 | 検査の呼び出し（`crates/exec/src/entry.rs`） | `self.run` の呼び出し |
| ---- | -------------------------------------------- | --------------------- |
| `f16_fma` | 189 行 | 191 行 |
| `subgroup_sum_u32` | 203 行 | 210 行 |
| `subgroup_sum_f32` | 222 行 | 229 行 |
| `atomic_add` | 242 行（`n` と `Some(m)`） | 249 行 |

取り込みは `entry.rs:10` の `use crate::length::check_lengths;`。測ったコマンドは `grep -n 'check_lengths\|self.run(OpId' crates/exec/src/entry.rs`。

### 完了の条件の確かめ（7/7）の値

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| (1) の `forbid` の行 | `18:#![forbid(unsafe_code)]` → `19:#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |
| `crates/exec/src` のファイルの数 | 8 ファイル → 9 ファイル（`length.rs` が加わった） | `ls crates/exec/src \| wc -l` |
| (3) の `&'static` の行 | `entry.rs:146`・`entry.rs:172` → `entry.rs:147` の `name: &'static str,` と `entry.rs:173` の `fn unsupported(&self, op: &'static str) -> ContractError {`（2 行のまま） | `grep -rn 'std::env\|env!\|option_env!\|std::fs\|static \|&mut self' crates/exec/src` |
| `crates/exec/Cargo.toml` の行数 | 36 行 → 41 行（`length_device` の `[[test]]` が `required-features = ["native-blocking-wait"]` で加わった） | `wc -l crates/exec/Cargo.toml` |
| `public-name-check` の exec の all-features | `public_items=451 named_items=149` のまま（production は `403`・`123`。`name_hits=0`） | `make public-name-check` |

### 試験の件数

`crates/exec/tests/` に `length_zero.rs`・`length_limit.rs`・`length_device.rs` が加わり、`--lib` の件数が増えた。

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 49 passed → 合計 64 passed | 36 + 4 + 5 + 5 + 4 + 3 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 51 passed → 合計 67 passed | 37 + 1 + 1 + 4 + 5 + 5 + 4 + 3 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 71 passed → 合計 86 passed | 49 + 8 + 4 + 5 + 5 + 4 + 4 + 6 + 1 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 73 passed → 合計 89 passed | 50 + 1 + 8 + 1 + 4 + 5 + 5 + 4 + 4 + 6 + 1 + doctest 0 |

`--lib` は feature なしで 30 passed → 36 passed、`native-blocking-wait` 付きで 31 passed → 37 passed、`fault-injection` 付きで 43 passed → 49 passed、`--all-features` で 44 passed → 50 passed。

## 今の値の補い（base dbbb5c0）

記録時点の表と既存の節は書き換えない。以下は base `dbbb5c0` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。`f16_fma` は、経路が `Route::Cpu` で host の形なら出力の列に書くようになった（`crates/exec/src/cpu.rs`。所在は [exec-cpu-route.md](exec-cpu-route.md)）。f32 の非正規化数の判定と (操作, 経路) ごとの保持は `crates/exec/src/denormal.rs` にある（所在は [exec-denormal-judgment.md](exec-denormal-judgment.md)）。

### `f16_fma` の入口の動き

「下-2 の値と 7 つの入口（4/7）」の手順 6 の文と、その後の「どの入口も `n`・`m`・`operands` を読まず、出力の列・buffer に書かない。`Outcome::Ran` は返さない。」の文は、`f16_fma` については次のとおり。

| 呼び出し | 返り値 |
| -------- | ------ |
| 経路が `Route::Cpu` で host の形 | `self.run` を呼ばず `crate::cpu::f16_fma` で `out` に書き、`Ok(Outcome::Ran { report })` を返す |
| 経路なし | `Ok(Outcome::Unavailable { report })`。列の中身は読まず、出力に書かない |
| device の形、または `Route::Cpu` 以外の経路が決まった呼び出し | `Err(ContractError::Unsupported { op: "f16_fma", .. })` |
| `n` の検査（`check_lengths`）、host の形の 4 つの列の長さが `n` と同じかの検査（`check_host_len`）のどちらかに外れた呼び出し | `Err(ContractError::InvalidArgument(..))`。能力を問い合わせない |

`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` は、これまでどおり `n`・`m`・`operands` を読まず、出力に書かず、`Outcome::Ran` を返さない。`external_interop_shared_memory`・`hardware_ray_tracing`・`unified_memory_sharing` も `Outcome::Ran` を返さない。

「今の値の補い（base 904126a）」の入口ごとの行の表は、今は次のとおり。

| 入口 | 検査の呼び出し（`crates/exec/src/entry.rs`） | `self.run` の呼び出し |
| ---- | -------------------------------------------- | --------------------- |
| `f16_fma` | 255 行の `check_lengths`、259 行の `check_host_len` | なし（265 行の `self.route_report` と、269 行の `crate::cpu::f16_fma`、270 行の `Ok(Outcome::Ran { report })`、272 行の `Err(self.unsupported("f16_fma"))`） |
| `subgroup_sum_u32` | 285 行 | 292 行 |
| `subgroup_sum_f32` | 304 行 | 311 行 |
| `atomic_add` | 324 行（`n` と `Some(m)`） | 331 行 |

測ったコマンドは `grep -n 'check_lengths\|check_host_len\|self.run(OpId\|Outcome::Ran\|crate::cpu' crates/exec/src/entry.rs`。取り込みは `entry.rs:15` の `use crate::length::{check_host_len, check_lengths};`。

### 完了の条件の確かめ（7/7）の値

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| (1) の `forbid` の行 | `19:#![forbid(unsafe_code)]` → `20:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけで、ほか 12 ファイルは 0 件 | `grep -n forbid crates/exec/src/lib.rs`、`grep -rc unsafe crates/exec/src` |
| `crates/exec/src` のファイルの数 | 9 ファイル → 13 ファイル（`binary16.rs`・`cpu.rs`・`denormal.rs`・`denormal_tests.rs` が加わった） | `ls crates/exec/src \| wc -l` |
| `crates/exec` の `.rs` の数 | `make exec-deps-check` の `rs_files=20` → `rs_files=26` | `make exec-deps-check` |
| `crates/exec/Cargo.toml` の行数 | 41 行のまま | `wc -l crates/exec/Cargo.toml` |

### 試験の件数

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 64 passed → 合計 98 passed | 63 + 3 + 2 + 4 + 5 + 6 + 4 + 3 + 6 + 2 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 67 passed → 合計 103 passed | 64 + 3 + 2 + 2 + 2 + 4 + 5 + 6 + 4 + 3 + 6 + 2 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 86 passed → 合計 130 passed | 85 + 4 + 8 + 2 + 4 + 5 + 6 + 4 + 4 + 6 + 2 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 89 passed → 合計 135 passed | 86 + 4 + 2 + 8 + 2 + 2 + 4 + 5 + 6 + 4 + 4 + 6 + 2 + doctest 0 |

`--lib` は feature なしで 36 passed → 63 passed、`native-blocking-wait` 付きで 37 passed → 64 passed、`fault-injection` 付きで 49 passed → 85 passed、`--all-features` で 50 passed → 86 passed。統合テストは `crates/exec/tests/` の `cpu_f16_fma.rs`（feature なしで 3 passed、`fault-injection` 付きで 4 passed）が加わり、`device_operands.rs` は 2 passed、`opt_in_default.rs` は 6 passed、`unsupported_fields.rs` は 2 passed、`route_evidence_calls.rs` は feature なしで 3 passed、`fault-injection` 付きで 4 passed、`length_host.rs` は 2 passed になっている。

```text
cargo test -p fandhe-silicon-exec
cargo test -p fandhe-silicon-exec --features native-blocking-wait
cargo test -p fandhe-silicon-exec --features fault-injection
cargo test -p fandhe-silicon-exec --all-features
```

## 今の値の補い（base 515bb19）

記録時点の表と既存の節は書き換えない。以下は base `515bb19` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。`subgroup_sum_u32`・`subgroup_sum_f32` は、経路が `Route::Cpu` で host の形なら出力の列に書くようになった（`crates/exec/src/cpu.rs`。所在は [exec-cpu-route.md](exec-cpu-route.md)）。

### subgroup の 2 入口の動き

「今の値の補い（base dbbb5c0）」の「`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` は、これまでどおり `n`・`m`・`operands` を読まず、出力に書かず、`Outcome::Ran` を返さない。」の文は、`atomic_add` について今も当たる。`subgroup_sum_u32`・`subgroup_sum_f32` は次のとおり。

| 呼び出し | 返り値 |
| -------- | ------ |
| 経路が `Route::Cpu` で host の形 | `self.run` を呼ばず `crate::cpu::subgroup_sum_u32`・`crate::cpu::subgroup_sum_f32` で `out` に書き、`Ok(Outcome::Ran { report })` を返す |
| 経路なし | `Ok(Outcome::Unavailable { report })`。列の中身は読まず、出力に書かない |
| device の形、または `Route::Cpu` 以外の経路が決まった呼び出し | `Err(ContractError::Unsupported { op: <入口の関数名>, .. })` |
| `n` の検査（`check_lengths`）、host の形の `x` と `out` の列の長さの検査（`check_host_len`）のどちらかに外れた呼び出し | `Err(ContractError::InvalidArgument(..))`。能力を問い合わせない |

`out` の列の長さの期待は `crate::length::subgroup_output_len(n)`（`n` を 256 で割って切り上げた数）。

「今の値の補い（base dbbb5c0）」の入口ごとの行の表は、今は次のとおり。

| 入口 | 検査の呼び出し（`crates/exec/src/entry.rs`） | `self.run` の呼び出し |
| ---- | -------------------------------------------- | --------------------- |
| `f16_fma` | 265 行の `check_lengths`、269 行の `check_host_len` | なし（275 行の `self.route_report`、279 行の `crate::cpu::f16_fma`、280 行の `Ok(Outcome::Ran { report })`、282 行の `Err(self.unsupported("f16_fma"))`） |
| `subgroup_sum_u32` | 300 行の `check_lengths`、308・309 行の `check_host_len` | なし（314 行の `self.route_report`、318 行の `crate::cpu::subgroup_sum_u32`、319 行の `Ok(Outcome::Ran { report })`、321 行の `Err(self.unsupported("subgroup_sum_u32"))`） |
| `subgroup_sum_f32` | 342 行の `check_lengths`、350・351 行の `check_host_len` | なし（356 行の `self.route_report`、360 行の `crate::cpu::subgroup_sum_f32`、361 行の `Ok(Outcome::Ran { report })`、363 行の `Err(self.unsupported("subgroup_sum_f32"))`） |
| `atomic_add` | 377 行の `check_lengths`（`n` と `Some(m)`） | 384 行 |

測ったコマンドは `grep -n 'check_lengths\|check_host_len\|self.run(OpId\|self.route_report(OpId\|Outcome::Ran\|crate::cpu\|self.unsupported("' crates/exec/src/entry.rs`。取り込みは `entry.rs:16` の `use crate::length::{check_host_len, check_lengths};`。

### 完了の条件の確かめ（7/7）の値

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| (1) の `forbid` の行 | `20:#![forbid(unsafe_code)]` → `21:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけで、ほか 12 ファイルは 0 件 | `grep -n forbid crates/exec/src/lib.rs`、`grep -rc unsafe crates/exec/src` |
| `crates/exec/src` のファイルの数 | 13 ファイルのまま | `ls crates/exec/src \| wc -l` |
| `crates/exec` の `.rs` の数 | `make exec-deps-check` の `rs_files=26` → `rs_files=27`。`crates/exec/tests` は `common/mod.rs` を含め 13 ファイル → 14 ファイル（`cpu_subgroup_sum.rs` が加わった） | `make exec-deps-check`、`find crates/exec/tests -name '*.rs' \| wc -l` |
| `crates/exec/Cargo.toml` の行数 | 41 行のまま | `wc -l crates/exec/Cargo.toml` |

### 試験の件数

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 98 passed → 合計 114 passed | 68 + 3 + 5 + 4 + 4 + 5 + 8 + 4 + 3 + 6 + 4 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 103 passed → 合計 121 passed | 69 + 3 + 5 + 3 + 3 + 4 + 4 + 5 + 8 + 4 + 3 + 6 + 4 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 130 passed → 合計 148 passed | 91 + 4 + 6 + 8 + 4 + 4 + 5 + 8 + 4 + 4 + 6 + 4 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 135 passed → 合計 155 passed | 92 + 4 + 6 + 3 + 8 + 3 + 4 + 4 + 5 + 8 + 4 + 4 + 6 + 4 + doctest 0 |

内訳の並びは、`--lib`、`cpu_f16_fma`、`cpu_subgroup_sum`（feature なしと `native-blocking-wait` では 5、`fault-injection` 付きでは 6）、`device_operands`、`fault_injection_ports`、`length_device`、`length_host`、`length_limit`、`length_zero`、`opt_in_default`、`reasons_and_denormal`、`route_evidence_calls`、`route_query`、`unsupported_fields`、doctest の順で、その流し方に無い試験の本は並べていない。

`--lib` は feature なしで 63 passed → 68 passed、`native-blocking-wait` 付きで 64 passed → 69 passed、`fault-injection` 付きで 85 passed → 91 passed、`--all-features` で 86 passed → 92 passed。統合テストは `crates/exec/tests/` の `cpu_subgroup_sum.rs` が加わり、`device_operands.rs` は 2 passed → 3 passed、`length_device.rs` は 2 passed → 3 passed、`length_host.rs` は 2 passed → 4 passed、`opt_in_default.rs` は 6 passed → 8 passed、`unsupported_fields.rs` は 2 passed → 4 passed になっている。`tests/*.rs` の本は 12 本 → 13 本。

```text
cargo test -p fandhe-silicon-exec
cargo test -p fandhe-silicon-exec --features native-blocking-wait
cargo test -p fandhe-silicon-exec --features fault-injection
cargo test -p fandhe-silicon-exec --all-features
```

## 今の値の補い（base 8e4b99b）

記録時点の表と既存の節は書き換えない。以下は base `8e4b99b` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。`atomic_add` も、経路が `Route::Cpu` で host の形なら出力の列に書くようになった（`crates/exec/src/cpu.rs` の `atomic_add`。所在は [exec-cpu-route.md](exec-cpu-route.md)）。private の `Exec::run` は無くなり、4 つの入口（`f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add`）はどれも `self.run` を呼ばない。

### `atomic_add` の動き

「今の値の補い（base 515bb19）」の「subgroup の 2 入口の動き」の冒頭で `atomic_add` について書いた「`n`・`m`・`operands` を読まず、出力に書かず、`Outcome::Ran` を返さない」は、今は当たらない。`atomic_add` は次のとおり。

| 呼び出し | 返り値 |
| -------- | ------ |
| 経路が `Route::Cpu` で host の形 | `crate::cpu::atomic_add` で `out` に書き、`Ok(Outcome::Ran { report })` を返す |
| 経路なし | `Ok(Outcome::Unavailable { report })`。列の中身は読まず、出力に書かない |
| device の形、または `Route::Cpu` 以外の経路が決まった呼び出し | `Err(ContractError::Unsupported { op: "atomic_add", .. })` |
| `n` と `m` の検査（`check_lengths`）、host の形の `x` と `n`・`out` と `m` の列の長さの検査（`check_host_len`）のどちらかに外れた呼び出し | `Err(ContractError::InvalidArgument(..))`。能力を問い合わせない |

### 入口ごとの行

「今の値の補い（base 515bb19）」の入口ごとの行の表は、今は次のとおり。

| 入口 | 検査の呼び出し（`crates/exec/src/entry.rs`） | 経路の報告・本体・返し |
| ---- | -------------------------------------------- | ---------------------- |
| `f16_fma` | 254 行の `check_lengths`、258 行の `check_host_len` | 264 行の `self.route_report`、268 行の `crate::cpu::f16_fma`、269 行の `Ok(Outcome::Ran { report })`、271 行の `Err(self.unsupported("f16_fma"))` |
| `subgroup_sum_u32` | 289 行の `check_lengths`、297・298 行の `check_host_len` | 303 行の `self.route_report`、307 行の `crate::cpu::subgroup_sum_u32`、308 行の `Ok(Outcome::Ran { report })`、310 行の `Err(self.unsupported("subgroup_sum_u32"))` |
| `subgroup_sum_f32` | 331 行の `check_lengths`、339・340 行の `check_host_len` | 345 行の `self.route_report`、349 行の `crate::cpu::subgroup_sum_f32`、350 行の `Ok(Outcome::Ran { report })`、352 行の `Err(self.unsupported("subgroup_sum_f32"))` |
| `atomic_add` | 375 行の `check_lengths`（`n` と `Some(m)`）、383・384 行の `check_host_len`（`x` と `n`、`out` と `m`） | 389 行の `self.route_report`、393 行の `crate::cpu::atomic_add`、394 行の `Ok(Outcome::Ran { report })`、396 行の `Err(self.unsupported("atomic_add"))` |

測ったコマンドは `grep -n 'check_lengths\|check_host_len\|self.run(\|self.route_report(OpId\|Outcome::Ran\|crate::cpu\|self.unsupported("' crates/exec/src/entry.rs`（`self.run(` の出現は無い）。取り込みは `entry.rs:15` の `use crate::length::{check_host_len, check_lengths};`。

### 完了の条件の確かめ（7/7）の値

| 節 | 「今の値の補い（base 515bb19）」の値 → 今の値 | 測ったコマンド |
| -- | --------------------------------------------- | -------------- |
| (1) の `forbid` の行 | `21:#![forbid(unsafe_code)]` → `22:#![forbid(unsafe_code)]`。`unsafe` の出現は `lib.rs` の 1 件だけで、ほか 12 ファイルは 0 件 | `grep -n forbid crates/exec/src/lib.rs`、`grep -rc unsafe crates/exec/src` |
| `crates/exec/src` のファイルの数 | 13 ファイルのまま | `ls crates/exec/src \| wc -l` |
| `crates/exec` の `.rs` の数 | `make exec-deps-check` の `rs_files=27` → `rs_files=28`。`crates/exec/tests` は `common/mod.rs` を含め 14 ファイル → 15 ファイル（`cpu_atomic_add.rs` が加わった） | `make exec-deps-check`、`find crates/exec/tests -name '*.rs' \| wc -l` |
| `crates/exec/Cargo.toml` の行数 | 41 行のまま | `wc -l crates/exec/Cargo.toml` |

### 試験の件数

| 流し方 | 前の値 → 今の値 | 内訳 |
| ------ | --------------- | ---- |
| `cargo test -p fandhe-silicon-exec` | 合計 114 passed → 合計 123 passed | 69 + 4 + 3 + 5 + 6 + 4 + 5 + 9 + 4 + 3 + 6 + 5 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 121 passed → 合計 132 passed | 70 + 4 + 3 + 5 + 4 + 4 + 6 + 4 + 5 + 9 + 4 + 3 + 6 + 5 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 148 passed → 合計 158 passed | 92 + 5 + 4 + 6 + 8 + 6 + 4 + 5 + 9 + 4 + 4 + 6 + 5 + doctest 0 |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 155 passed → 合計 167 passed | 93 + 5 + 4 + 6 + 4 + 8 + 4 + 6 + 4 + 5 + 9 + 4 + 4 + 6 + 5 + doctest 0 |

内訳の並びは、`--lib`、`cpu_atomic_add`（feature なしと `native-blocking-wait` では 4、`fault-injection` 付きでは 5）、`cpu_f16_fma`、`cpu_subgroup_sum`、`device_operands`、`fault_injection_ports`、`length_device`、`length_host`、`length_limit`、`length_zero`、`opt_in_default`、`reasons_and_denormal`、`route_evidence_calls`、`route_query`、`unsupported_fields`、doctest の順で、その流し方に無い試験の本は並べていない。

`--lib` は feature なしで 68 passed → 69 passed、`native-blocking-wait` 付きで 69 passed → 70 passed、`fault-injection` 付きで 91 passed → 92 passed、`--all-features` で 92 passed → 93 passed。統合テストは `crates/exec/tests/` の `cpu_atomic_add.rs` が加わり、`device_operands.rs` は 3 passed → 4 passed（`cpu_route_atomic_add_with_device_operands_is_unsupported_and_leaves_buffers`）、`length_device.rs` は 3 passed → 4 passed、`length_host.rs` は 4 passed → 6 passed、`opt_in_default.rs` は 8 passed → 9 passed、`unsupported_fields.rs` は 4 passed → 5 passed になっている。`tests/*.rs` の本は 13 本 → 14 本。

```text
cargo test -p fandhe-silicon-exec
cargo test -p fandhe-silicon-exec --features native-blocking-wait
cargo test -p fandhe-silicon-exec --features fault-injection
cargo test -p fandhe-silicon-exec --all-features
```
