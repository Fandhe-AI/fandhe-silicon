# CUDA の背後実装のエラーの写し方（TASK-291）の所在と本リポの現状

TASK-291 の作業のうち、CUDA の背後実装の `ComputeOps::create_pipeline` が受け付けない入力に返す値と、ドライバ・NVRTC の呼び出しの失敗を写す関数の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-291。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入力の分類 | `crates/contract/src/backend_cuda.rs` の `classify_pipeline_input` | `git grep -n "fn classify_pipeline_input" -- crates/contract/src/backend_cuda.rs` |
| 失敗の写し | 同じファイルの `backend_failure`・`allocation_failure`・`artifact_failure`・`nvrtc_failure` | `git grep -n "fn backend_failure\|fn allocation_failure\|fn artifact_failure\|fn nvrtc_failure" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `pipeline_error_tests` module。device に触らない | 下の「確かめ方」 |

## 受け付けない入力の理由と失敗の写し（1/3）

`classify_pipeline_input` が受け付けない 6 つの入力に返す値は `ArtifactRejected { kind: "unsupported-input", reason }` で、`reason` は次のとおり。試験は `unsupported_inputs_name_the_variant_and_the_accepted_inputs` で、入力のソースの字句が `reason` に載らないことも照らす。

| 入力 | `reason` |
| ---- | -------- |
| `Msl` | `pipeline input Msl is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `MslSpecialized` | `pipeline input MslSpecialized is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `MslWithOptions` | `pipeline input MslWithOptions is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `MslSpecializedWithOptions` | `pipeline input MslSpecializedWithOptions is Metal-only and is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `SpirV` | `pipeline input SpirV is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `Dxil` | `pipeline input Dxil is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |

失敗を写す関数が返す値と試験:

| 関数と入力 | 返す値 | 試験 |
| ---------- | ------ | ---- |
| `backend_failure("memcpy_htod", "detail")` | `BackendFailure { op: "memcpy_htod", backend: Cuda, detail: "\"detail\"" }` | `driver_failures_carry_the_call_name_and_the_debug_text` |
| `allocation_failure("alloc_zeros", 64, …)` の `CUDA_ERROR_OUT_OF_MEMORY` | `AllocationFailed { requested_bytes: 64 }` | 同上 |
| `artifact_failure(kind, call, true, "detail")`（`module` と `cuModuleLoadData`、`function` と `cuModuleGetFunction`） | `ArtifactRejected { kind, reason: "<call>: \"detail\"" }` | `artifact_failures_carry_the_call_name_and_the_debug_text` |
| `artifact_failure(kind, call, false, "detail")`（同じ 2 組） | `BackendFailure { op: <call>, backend: Cuda, detail: "\"detail\"" }` | 同上 |
| `nvrtc_failure` の `nvrtcCompileProgram` の `NVRTC_ERROR_COMPILATION` | `ArtifactRejected { kind: "cuda-c", reason: "nvrtcCompileProgram: <失敗の Debug の文字列>" }` | 同上 |
| `nvrtc_failure` の `nvrtcCreateProgram` の `NVRTC_ERROR_OUT_OF_MEMORY` | `BackendFailure { op: "nvrtcCreateProgram", backend: Cuda, detail: <失敗の Debug の文字列> }` | 同上 |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::pipeline_error_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::tests::cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified
```

1 つ目は 3 件、2 つ目は 1 件が通る。

## device の要る試験（2/3）

device に触る試験は `crates/contract/src/backend_cuda.rs` の `pipeline_error_device_tests` module にあり、どれも `#[ignore = "requires a CUDA device"]` を付けている。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。試験のビルドの device は、`compile_with_nvrtc` に入った回数と、`create_pipeline` が `bind_to_thread`・モジュールの読み込み・関数の取り出しを呼ぶ前に 1 ずつ足す回数（欄 `pipeline_driver_calls`）を持ち、試験はその 2 つの回数を読む。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `unsupported_inputs_are_rejected_without_entering_nvrtc_or_the_driver` | `Msl`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions`・`SpirV`・`Dxil` の入力で `create_pipeline` を 1 回ずつ、続けて `CudaC` の入力で 1 回 | 6 つとも `ArtifactRejected { kind: "unsupported-input" }` で、理由は受けた variant の名前と `CudaC`・`Ptx` を含み、`Metal` の語は `MslSpecializedWithOptions` の理由だけが含む。6 つの後も表の件数は増えず、2 つの回数は 0。`CudaC` の 1 回の後は `compile_with_nvrtc` に入った回数が 1、もう 1 つの回数が 3 で、表の件数が 1 増える |
| `artifact_failures_name_the_failed_call_and_add_no_pipeline` | コンパイルできない `CudaC`・無い入口名の `CudaC`・読めないテキストの `Ptx` の入力で `create_pipeline` を 1 回ずつ | 順に `ArtifactRejected` の `cuda-c`・`function`・`module` で、理由は `nvrtcCompileProgram: `・`cuModuleGetFunction: `・`cuModuleLoadData: ` で始まる。表の件数は増えない。`compile_with_nvrtc` に入った回数は順に 1・2・2、もう 1 つの回数は順に 0・3・5 |

`Msl` 系の入力のソースは Metal の背後実装の試験の `KERNEL`・`SPECIALIZED_KERNEL` と同じ文字列で、`SpirV` の入力は何もしない compute の入口 `k` の語の列、`Dxil` の入力は DXBC のコンテナの見出しだけの 32 バイト。

| 項目 | 値 |
| ---- | -- |
| module | `crates/contract/src/backend_cuda.rs` の `pipeline_error_device_tests` |
| feature | `backend-cuda` |
| `#[ignore]` | `requires a CUDA device` |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::pipeline_error_device_tests
```

2 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 受け付けない入力の理由に Metal 専用を載せる形と失敗の写しの値の試験を足す | #4152 |
| 2/3 | 受け付けない入力と成果物の生成の失敗の device の要る試験を ignore 付きで足す | #4153 |
