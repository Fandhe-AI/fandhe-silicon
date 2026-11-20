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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 受け付けない入力の理由に Metal 専用を載せる形と失敗の写しの値の試験を足す | #4152 |
