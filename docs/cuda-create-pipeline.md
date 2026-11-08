# CUDA の背後実装のパイプラインの生成（TASK-803）の所在と本リポの現状

TASK-803 の作業のうち、CUDA の背後実装の `ComputeOps::create_pipeline` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-803。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 口 | `ComputeOps` の `create_pipeline`（`CudaDevice` の実装）。`Device::compute()` は既定の `None` のまま | `grep -n 'create_pipeline' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |

## 入力の分類と配置の検査（1/4）

`create_pipeline` が照らす順（最初に当たったものを返す）:

| 順 | 検査 | 関数 |
| -- | ---- | ---- |
| 1 | 入力の分類 | `classify_pipeline_input` |
| 2 | `layout.slots` の重複 | `check_unique_slots` |
| 3 | ソースと入口名の NUL のバイト | `check_no_nul` |

入力ごとの返す値と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| `CudaC` | 通る | `only_cuda_c_is_accepted_and_other_inputs_are_classified` |
| `Ptx`・`Wgsl`・知らない variant | `Unsupported { op: "create_pipeline", backend: Cuda }` | 同上 |
| `Msl`・`SpirV`・`Dxil`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions` | `ArtifactRejected { kind: "unsupported-input" }` | 同上 |
| 重複した slot | `InvalidArgument`（重複した最初の番号を載せる） | `duplicate_slots_are_invalid_argument` |
| ソースの NUL | `ArtifactRejected { kind: "cuda-c" }` | `nul_bytes_are_rejected_before_compilation` |
| 入口名の NUL | `ArtifactRejected { kind: "function" }` | 同上 |

## NVRTC のコンパイルとパイプラインの表（2/4）

1/4 の 3 つの検査の後に続く手順:

| 順 | 手順 | 関数・呼び出し |
| -- | ---- | -------------- |
| 4 | `CudaC` のソースを PTX にコンパイルする | `compile_cuda_c` |
| 5 | context を呼び出し元の thread に束ねる | `bind_to_thread` |
| 6 | モジュールを読み込む | `load_module` |
| 7 | 入口名で関数を取得する | `load_function` |
| 8 | 関数と配置の写しを device のパイプラインの表に登録し、`PipelineHandle` を返す | `create_pipeline` |

NVRTC に渡すオプションは `--gpu-architecture=compute_<major><minor>` の 1 つだけ。試験は `compile_options_are_only_the_gpu_architecture`。

失敗ごとの返す値と試験:

| 失敗 | 返す値 | 試験 |
| ---- | ------ | ---- |
| NVRTC のライブラリが無い | `Unsupported { op: "create_pipeline", backend: Cuda }` | `absent_nvrtc_library_is_unsupported_without_panicking` |
| compute capability が負 | `BackendFailure { op: "create_pipeline" }` | `compile_options_are_only_the_gpu_architecture` |
| NVRTC の失敗（`COMPILATION`・`INVALID_PROGRAM`・`INVALID_OPTION`・`INVALID_INPUT`） | `ArtifactRejected { kind: "cuda-c" }` | `nvrtc_failures_map_to_artifact_rejected_or_backend_failure` |
| NVRTC のそのほかの失敗（メモリ不足・内部エラーなど） | `BackendFailure { op: <呼び出しの名前> }` | 同上 |
| モジュールの読み込みの失敗（`INVALID_PTX`・`INVALID_IMAGE`・`NO_BINARY_FOR_GPU`・`UNSUPPORTED_PTX_VERSION`・`NOT_FOUND`） | `ArtifactRejected { kind: "module" }` | `driver_codes_are_classified_by_artifact_cause` |
| 関数の取得の失敗（上と同じコード） | `ArtifactRejected { kind: "function" }` | 同上 |
| ドライバのそのほかの失敗（メモリ不足・context の無効化・不正なアドレスなど） | `BackendFailure { op: "cuModuleLoadData" / "cuModuleGetFunction" }` | 同上 |
| ドライバや NVRTC を呼ぶ中の panic | `BackendFailure { op: "create_pipeline" }` | `a_panic_in_the_pipeline_guard_is_backend_failure` |

表に入るものは、取得した関数と `layout` の写し。`layout` は `slots` の重複の検査と写しのほかには読まない。識別子は device ごとに 1 から払い出し、尽きたときは `BackendFailure { op: "create_pipeline" }` を返す。device は読み取った compute capability を持つ（試験は `attributes_keep_the_compute_capability`）。

## device の要る試験（3/4）

device に触る試験は `crates/contract/src/backend_cuda.rs` の `device_tests` module にあり、どれも `#[ignore = "requires a CUDA device"]` を付けている。`open_device` が `request_device` の本体で、試験はこれで具体の型 `CudaDevice` を受け取る。

| 試験 | 確かめること |
| ---- | ------------ |
| `cuda_c_kernel_creates_a_pipeline_of_this_device` | CUDA C のカーネルからパイプラインが作られ、その device の識別子を持つ |
| `two_pipelines_on_one_device_get_distinct_ids` | 同じ device で 2 回作ると識別子が異なる |
| `slots_that_differ_from_the_kernel_arguments_still_create_a_pipeline` | 空の `slots` も、引数より多い `slots` も作られる |
| `the_pipeline_table_keeps_a_copy_of_the_layout` | 表が渡した `layout` の写しを持つ |
| `duplicate_slots_are_invalid_argument_and_add_no_pipeline` | 重複した `slots` は `InvalidArgument` で、表の件数が増えない |
| `uncompilable_source_is_artifact_rejected_cuda_c` | コンパイルできないソースは `ArtifactRejected { kind: "cuda-c" }` |
| `missing_entry_point_is_artifact_rejected_function` | 無い入口名は `ArtifactRejected { kind: "function" }` |
| `ptx_is_unsupported_and_msl_is_unsupported_input` | PTX は `Unsupported`、MSL は `ArtifactRejected { kind: "unsupported-input" }` |
| `compute_capability_is_read_from_the_device` | device が読み取った compute capability が `(0, 0)` でない |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::device_tests
```

`0 passed; 0 failed; 9 ignored` の形で通る。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 入力の分類と配置の検査 | #5713 |
| 2/4 | NVRTC のコンパイルとパイプラインの表 | #5714 |
| 3/4 | device の要る試験 | #5715 |
