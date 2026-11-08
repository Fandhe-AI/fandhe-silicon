# CUDA の背後実装の拡張アクセサ（TASK-197）の所在と本リポの現状

TASK-197 の作業のうち、CUDA の背後実装の device が返す物理 GPU の識別子の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-197。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `grep -n 'physical_gpu_id\|read_physical_gpu_id\|uuid_bytes' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")`。その中で feature `ext-interop`・`fault-injection` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda,ext-interop --lib backend_cuda` |

## 物理 GPU の識別子（1/4）

feature `ext-interop` のビルドでは、`request`（device の作成）の中で CUDA context の UUID を 1 回だけ読み、device の欄に持つ。
`Device::physical_gpu_id()` は持っている値を返すだけで、ドライバを呼び直さない。

| 入力 | 返す値 |
| ---- | ------ |
| UUID が読めた | `physical_gpu_id()` が `Some(PhysicalGpuId { device_uuid, driver_uuid: None })`。`device_uuid` は 16 個の `c_char` をビットを変えずに写したバイト列 |
| UUID が読めない | `request` が `DeviceUnavailable` を返す（device を返さない） |
| `ext-interop` の無いビルド | UUID を読まない |

対応する単体試験（どちらも feature `ext-interop`、device に触らない）:

- `uuid_bytes_keep_every_bit`: 高い位のビットを含む 16 バイトが変わらずに写る
- `uuid_read_maps_to_an_id_without_driver_uuid_or_to_device_unavailable`: 読めた値が `driver_uuid` なしの識別子になり、読めない閉包が `DeviceUnavailable` になる

## 相互運用と故障の差し込みの口（2/4）

feature `ext-interop` のビルドでは、`Device::external_interop()` が実行時の判定なしに `Some` を返す。
必須の 4 操作の返す値は次のとおり。

| 操作 | 返す値 |
| ---- | ------ |
| `export_memory` | `Err(Unsupported { op: "export_memory", backend: Cuda })` |
| `import_memory` | `Err(Unsupported { op: "import_memory", backend: Cuda })` |
| `export_semaphore` | `Err(Unsupported { op: "export_semaphore", backend: Cuda })` |
| `import_semaphore` | `Err(Unsupported { op: "import_semaphore", backend: Cuda })` |

拡張の 4 操作（`export_image`・`import_image`・`export_semaphore_timeline`・`import_semaphore_timeline`）は上書きしておらず、trait の既定の本体のままである。

feature `fault-injection` のビルドでは、`Device::fault_hooks()` が `Some` を返す。`FaultHooks` のどの操作も上書きしていない。

対応する試験は `crates/contract/tests/backend_cuda_ext_interop.rs`（device を作るので `#[ignore = "requires a CUDA device"]`。必須 4 操作の `Unsupported`、拡張の timeline 2 操作の既定の本体、`fault_hooks()` が `Some` で `arm_gate` が既定の本体を返すこと）。

門の無いビルドにこれらの口が無いことは、`crates/contract/src/cfg_gate_doctest.rs` の既存の門の doctest（`ExternalInteropGate`・`FaultInjectionGate`・`DeviceFaultHooksGate`）が確かめる。

## device の要る試験（3/4）

試験は `crates/contract/tests/backend_cuda_ext_interop.rs` と `crates/contract/tests/backend_cuda_fault.rs` にもある。どれも device を作るので `#[ignore = "requires a CUDA device"]` を付け、`--ignored` を付けたときだけ走る。

| ファイル | 試験 | 確かめること |
| -------- | ---- | ------------ |
| `backend_cuda_ext_interop.rs` | `physical_gpu_id_has_a_device_uuid_and_no_driver_uuid` | `physical_gpu_id()` が `Some` で、`driver_uuid` が `None` |
| `backend_cuda_ext_interop.rs` | `two_devices_report_the_same_physical_gpu` | 2 つの device の識別子が等しく、`check_same_physical_gpu` が `Ok` |
| `backend_cuda_ext_interop.rs` | `external_interop_is_some_through_dyn_device` | `Box<dyn Device>` 越しの `external_interop()` が `Some` |
| `backend_cuda_fault.rs` | `cuda_device_has_fault_hooks` | `ext-interop` なしの構成で `fault_hooks()` が `Some` |

確かめ方（ignore の宣言の数え上げ。`--ignored` は付けない）:

- `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,ext-interop --test backend_cuda_ext_interop`: `0 passed; 5 ignored`
- `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,ext-interop,fault-injection --test backend_cuda_ext_interop`: `0 passed; 6 ignored`
- `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --test backend_cuda_fault`: `3 passed; 1 ignored`

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-197。ポインタのみ）。

時点のコミット: `819811c`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 単体の試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda,ext-interop --lib backend_cuda` | `46 passed; 0 failed; 11 ignored`、終了コード 0 |
| 2 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-cuda,ext-interop --test backend_cuda_ext_interop` | `0 passed; 0 failed; 5 ignored`、終了コード 0 |
| 3 | 故障の差し込みの試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_fault` | `3 passed; 0 failed; 1 ignored`、終了コード 0 |
| 4 | 門ごとの型と未使用 | `cargo clippy -p fandhe-silicon-contract --all-targets --no-default-features --features <組> -- -D warnings`（`<組>` は `backend-cuda`・`backend-cuda,ext-interop`・`backend-cuda,fault-injection`・`backend-cuda,ext-interop,fault-injection,native-blocking-wait`） | 4 組とも終了コード 0 |
| 5 | 組のビルド | `make build-profiles` | 終了コード 0。`[build-profiles.sh] cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,ext-interop` の行が出る |
| 6 | 公開 API | `make public-api-sdk-check`、`make public-api-webgpu-check` | どちらも終了コード 0。`sdk_type_refs_total=0` を含む `record:` の行と `ok:` の行が出る |
| 7 | 個数テスト | `cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count`、`git diff --stat 4970298 HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs` | `1 passed; 0 failed`、終了コード 0。diff は空 |
| 8 | 依存 | `make deps-check`、`git diff --stat 4970298 HEAD -- Cargo.lock deps-approved.toml` | 終了コード 0。`[deps-check.sh] ok: ...` の行が出る。diff は空 |
| 9 | CI | `gh run view 42504030757 --json status,conclusion,headSha,jobs`（`819811c` の main の push の run） | conclusion は `success`。headSha は `819811cb9f82ec42daf3767502427bf2e6c3fa04`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` の conclusion は `success` |

(1) の出力（該当の行）。

```text
running 57 tests
test backend_cuda::tests::uuid_bytes_keep_every_bit ... ok
test backend_cuda::tests::uuid_read_maps_to_an_id_without_driver_uuid_or_to_device_unavailable ... ok
test result: ok. 46 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

(2) の出力。

```text
running 5 tests
test result: ok. 0 passed; 0 failed; 5 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(3) の出力。

```text
running 4 tests
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(5) の出力（該当の行）。

```text
[build-profiles.sh] cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,ext-interop
```

(6) の出力（`ok:` の行）。

```text
ok: 10 public API document(s) checked, common crate dependencies checked, no chip SDK references
ok: 2 public API document(s) checked, no blocking-wait names, pipeline input ok, ext-interop ok, submit token ops ok
```

(7) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

(8) の出力。

```text
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 物理 GPU の識別子を device の作成で読んで返す | #3884 |
| 2/4 | 相互運用と故障の差し込みの口を device から返す | #3885 |
| 3/4 | device の要る拡張アクセサの試験を ignore 付きで足す | #3886 |
| 4/4 | 完了の条件の確かめ | #3887 |

## 今の値の補い（base 788d89a）

記録時点の表と既存の節は書き換えない。以下は base `788d89a` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 相互運用と故障の差し込みの口（2/4）の `FaultHooks` の文 | どの操作も上書きしていない → `simulate_device_lost` だけを上書きし、ほかの 4 操作は既定の本体のまま | `grep -n 'fn simulate_device_lost' crates/contract/src/backend_cuda.rs` |

## 今の値の補い（base 1417dc5）

記録時点の表と既存の節は書き換えない。以下は base `1417dc5` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「完了の条件の確かめ（4/4）」の (1) の行と出力（`--features backend-cuda,ext-interop --lib backend_cuda`） | `46 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out` → `52 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out`（`running 63 tests`） | `cargo test -p fandhe-silicon-contract --features backend-cuda,ext-interop --lib backend_cuda` |
