# device から compute の口へ届く読み出し（TASK-2956）の所在と本リポの現状

TASK-2956 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは DEV-59・DEV-60・DEV-164 のポインタだけにする。事実はリポジトリのルートで流したコマンドの出力から書く。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2956。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 現状の形（1/3）

### Device::compute()

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/device.rs:133`。`fn compute(&self) -> Option<&dyn crate::execution::ComputeOps>` | `grep -n 'fn compute' crates/core/src/device.rs` |
| 既定の本体 | `None` を返す。feature の条件はない | 同上 |

### 試験用の device の答え

`StubDevice`（`crates/core/src/stub_device.rs`。feature `test-support` の裏）は要求の `profile` を保持し、`compute()` の答えを次のとおり返す。所有形態（`ProductIndependent`・`Shared`・`Imported`）によらず同じ。

| プロファイル | `compute()` | 確かめる試験 |
| ------------ | ----------- | ------------ |
| `CpuOnly` | `None` | `cpu_only_*_compute_is_none`（3 本） |
| `ComputeOnly` | `Some` | `compute_only_*_compute_creates_pipeline`（3 本） |
| `Graphics` | `Some` | `graphics_*_compute_creates_pipeline`（3 本） |

`Some` から `create_pipeline` を呼ぶと `Ok(PipelineHandle)` が返る。

### 印の device の識別子

- 印の `__backend_device_id()` は device の `generation()` と同じ値
- 印の `__backend_id()` は device ごとに 1 から増える
- 通し番号が尽きたら `BackendFailure { op: "create_pipeline", .. }` を返す（panic しない）

### 対応するテスト

試験で固定した値: `backend` は `BackendKind::Vulkan`、`Imported` は `NativeDeviceHandle::new(7u32)`、入力は空の MSL（入口 `main`）と空の `BindingLayout`。

`crates/core/tests/stub_device_compute.rs`（feature `test-support`）:

| テスト | 確かめること |
| ------ | ------------ |
| `cpu_only_product_independent_compute_is_none` | `CpuOnly`・`ProductIndependent` で `None` |
| `cpu_only_shared_compute_is_none` | `CpuOnly`・`Shared` で `None` |
| `cpu_only_imported_compute_is_none` | `CpuOnly`・`Imported` で `None` |
| `compute_only_product_independent_compute_creates_pipeline` | `ComputeOnly`・`ProductIndependent` で `Some`、`create_pipeline` が `Ok`、印の device の識別子が `generation()` と等しい |
| `compute_only_shared_compute_creates_pipeline` | `ComputeOnly`・`Shared` で同上 |
| `compute_only_imported_compute_creates_pipeline` | `ComputeOnly`・`Imported` で同上 |
| `graphics_product_independent_compute_creates_pipeline` | `Graphics`・`ProductIndependent` で同上 |
| `graphics_shared_compute_creates_pipeline` | `Graphics`・`Shared` で同上 |
| `graphics_imported_compute_creates_pipeline` | `Graphics`・`Imported` で同上 |
| `two_pipelines_from_one_device_have_distinct_ids` | 1 つの device から 2 回作ると `__backend_id()` が異なる |

`crates/core/tests/device_trait_shape.rs`（feature なしでも流れる）:

| テスト | 確かめること |
| ------ | ------------ |
| `default_compute_is_none_through_dyn_device` | 上書きしないダミーの device は `Box<dyn Device>` 越しに `compute()` が `None` |

`crates/core/src/stub_device.rs` の `mod tests`:

| テスト | 確かめること |
| ------ | ------------ |
| `create_pipeline_reports_exhausted_id_space` | 通し番号が `u64::MAX - 1` のとき `Ok`、続く呼び出しは `BackendFailure { op: "create_pipeline", .. }` |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --features test-support --test stub_device_compute
cargo test -p fandhe-silicon-core --test device_trait_shape
cargo test -p fandhe-silicon-core --features test-support --lib stub_device
```

feature なしの `cargo test -p fandhe-silicon-core` では、`stub_device_compute` は `required-features` により走らない。
流れる場所は `make verify`（`--all-features`）と、`make build-profiles` の core の feature なしの段・`test-support` だけの段。

## Metal の上書き（2/3）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/contract/src/backend_metal.rs:1912`。`impl Device for MetalDevice` の `compute()` が `Some(self)` を返す。実装は `cfg(target_os = "macos")` の `imp` の中にあり、ほかの OS ではコンパイルされない | `grep -n 'fn compute' crates/contract/src/backend_metal.rs` |
| `request` が返す device | `request` は `CpuOnly` と、`graphics` が無効のときの `Graphics` を device を作る前に拒むので、得られる device は `ComputeOnly` か（`graphics` 有効時の）`Graphics` で、どれも `Some` | `request_rejects_cpu_only_as_invalid_combination`・`request_rejects_graphics_without_the_graphics_feature`（`crates/contract/tests/backend_metal_request.rs`。後者は `#[cfg(not(feature = "graphics"))]`） |
| 対応するテスト | `crates/contract/tests/backend_metal_device.rs` の `compute_reaches_create_pipeline_through_the_public_device`（`#[ignore]`。required-features `backend-metal`・`native-blocking-wait`）。`ComputeOnly`・`ProductIndependent` の device で、MSL のカーネルと `BindingLayout::new(vec![0])` の `create_pipeline` が `Ok` | 下の確かめ方 |
| 行頭の `#[ignore` の件数 | 51 | `grep -cE '^\s*#\[ignore' crates/contract/tests/backend_metal_device.rs` |

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait --test backend_metal_device -- --ignored compute_reaches_create_pipeline_through_the_public_device
```

device の試験は専用の実機の回だけで流す（手順は [metal-run-procedure](metal-run-procedure.md)）。

## 段の表

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 現状の形（実装とテスト） | #7587 |
| 2/3 | Metal の上書き | #7588 |
