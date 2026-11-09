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

（上の表の行番号は記録時点の値。今は `:149`）

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

## 完了の条件の確かめ（3/3）

TASK-2956 の 3/3（#7589）として、本リポで観測できる事実だけを記録する。GPU の実機のない Linux の隔離した作業ツリー（base `85a1ceea1422c0bee77a641d7b785300d021c91b`）で流した出力による。コード・試験・CI は変えていない。ID は対応づけのポインタ（DEV-59・DEV-60・DEV-164）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 既定の本体が `None` | `cargo test -p fandhe-silicon-core --test device_trait_shape` | 終了コード 0。`default_compute_is_none_through_dyn_device ... ok`、`6 passed; 0 failed` |
| 2 | 試験用の device の 3 × 3（DEV-59・DEV-60） | `cargo test -p fandhe-silicon-core --features test-support --test stub_device_compute`、`--lib stub_device`、feature なしの `cargo test -p fandhe-silicon-core` | 終了コード 0。`10 passed; 0 failed`（`cpu_only_*_compute_is_none` 3・`compute_only_*_compute_creates_pipeline` 3・`graphics_*_compute_creates_pipeline` 3・`two_pipelines_from_one_device_have_distinct_ids`）。lib は `create_pipeline_reports_exhausted_id_space` を含め `5 passed`。feature なしの実行に `Running tests/stub_device_compute` は 0 件 |
| 3 | Metal の上書きと device の試験（DEV-164） | `grep -n 'fn compute' crates/contract/src/backend_metal.rs`、`grep -n -B1 'fn compute_reaches_create_pipeline_through_the_public_device' crates/contract/tests/backend_metal_device.rs`、`grep -cE '^\s*#\[ignore' crates/contract/tests/backend_metal_device.rs`、macOS の CI のログ | `1912:        fn compute(&self) -> Option<&dyn ComputeOps> {`。試験の直前の行は `#[ignore = "touches the Metal device; run only in a dedicated device session"]`。行頭の `#[ignore` は 51。PR #7591 の CI の run 41281958182・job 124775505110（`make verify + build-profiles (macos-latest)`）の `make verify` の段のログに `compute_reaches_create_pipeline_through_the_public_device ... ignored, touches the Metal device; run only in a dedicated device session` と `test result: ok. 0 passed; 0 failed; 51 ignored`（`Running tests/backend_metal_device.rs` の後） |
| 4 | CI で流れる場所 | `grep -nE 'run: make (verify\|build-profiles)' .github/workflows/ci.yml`、`grep -n 'CORE_PKG' scripts/build-profiles.sh`、`gh run list --branch main --commit <sha> --workflow ci.yml` | `make verify` は `:177`・`:465`、`make build-profiles` は `:181`・`:468`。`build-profiles.sh` は core の feature なし（`:85`・`:86`）と `test-support` だけ（`:89`・`:90`）の clippy と test を流す。main の push の run は、1/3 の `0a848d7` が 41281414499（completed / success）、2/3 の `85a1cee` が 41282534972（completed / success） |
| 5 | wasm32 の型検査 | `make wasm-verify` | 終了コード 0 |
| 6 | 公開 API の検査 5 つ | `make public-api-check`・`make public-name-check`・`make public-api-webgpu-check`・`make public-api-sdk-check`・`make core-deps-check` | すべて終了コード 0。`documents=10 public_items_total=10163 wgpu_type_refs_total=0 wgpu_other_refs_total=0`／`name_hits_total=0 source_hits_total=0 field_hits_total=0`／`documents=2 blocking_wait_names_total=0 webgpu_names_total=0`／`sdk_type_refs_total=0 common_declared_deps=0`／`unexpected=0 missing=0 forward_violations=0` |
| 7 | 依存 0・unsafe の追加 0 | `git diff --stat 7b787e3 HEAD -- Cargo.lock deps-approved.toml`、`git diff 7b787e3 HEAD -- crates/core/Cargo.toml crates/contract/Cargo.toml`、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n unsafe_code crates/core/src/lib.rs`、`backend_metal.rs` の `unsafe {` の行の数 | `Cargo.lock`・`deps-approved.toml` の差分は空。`Cargo.toml` の差分は core の `[[test]]`（`stub_device_compute`・`required-features = ["test-support"]`）だけで、contract は差分なし。`[[approval]]` は 4。`28:#![forbid(unsafe_code)]`。`backend_metal.rs` の `unsafe {` は `7b787e3` で 3、base で 3 |
| 8 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles` | すべて終了コード 0（Linux・GPU なし） |

- 補い（TASK-150 の後）: 上の表の 1 の `device_trait_shape` は、試験が増えて今は `7 passed; 0 failed` である。
- 完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-2956）。確かめた base は `85a1ceea1422c0bee77a641d7b785300d021c91b`。

## 段の表

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 現状の形（実装とテスト） | #7587 |
| 2/3 | Metal の上書き | #7588 |
| 3/3 | 完了の条件の確かめ | #7589 |

## 今の値の補い（base 6b35dd9）

上の表の項目 4 の `ci.yml` の行番号は記録時点の値で、書き換えない。今の `.github/workflows/ci.yml` では `make verify` が `:177`・`:505`、`make build-profiles` が `:181`・`:508` である（数えたコマンド: `grep -nE 'run: make (verify|wasm-verify|build-profiles|profile-test-count-check)$' .github/workflows/ci.yml`）。

## 今の値の補い（base d533488）

上の表の項目 3 と「Metal の上書き（2/3）」の表の行頭の `#[ignore` の件数、完了の条件の確かめの項目 7 の `[[approval]]` の件数は記録時点の値で、書き換えない。今の値は次のとおり。

| 場所 | 記録時点 | 今 | 数えたコマンド |
| ---- | -------- | -- | -------------- |
| 行頭の `#[ignore` の件数（`crates/contract/tests/backend_metal_device.rs`） | 51 | 52 | `grep -cE '^\s*#\[ignore' crates/contract/tests/backend_metal_device.rs` |
| `deps-approved.toml` の `[[approval]]` | 4 | 5（`cudarc` を含む） | `grep -c '^\[\[approval\]\]' deps-approved.toml` |

## 今の値の補い（base dd1a116）

記録時点の表と既存の節は書き換えない。以下は base `dd1a116` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「今の値の補い（base d533488）」の行頭の `#[ignore` の件数（`crates/contract/tests/backend_metal_device.rs`） | 52 → 53 | `grep -cE '^\s*#\[ignore' crates/contract/tests/backend_metal_device.rs` |

## 今の値の補い（base 70ab958）

記録時点の表と既存の節は書き換えない。以下は base `70ab958` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「Metal の上書き（2/3）」の表の所在（`backend_metal.rs:1912`） | `:1912` → `:2119`（`compute()` が `Some(self)` を返すことは同じ） | `grep -n 'fn compute' crates/contract/src/backend_metal.rs` |
| CUDA の背後実装の上書き（記載なし） | `CudaDevice` も `Device::compute()` を上書きして `Some(self)` を返す（所在は `backend_cuda.rs` の `:1661`） | `grep -n 'fn compute(' crates/contract/src/backend_cuda.rs` |
| 「今の値の補い（base 6b35dd9）」の `ci.yml` の行（`make verify` `:177`・`:505`、`make build-profiles` `:181`・`:508`） | `make verify` は `:177`・`:520`、`make build-profiles` は `:181`・`:523` | `grep -nE 'run: make (verify\|build-profiles)$' .github/workflows/ci.yml` |
