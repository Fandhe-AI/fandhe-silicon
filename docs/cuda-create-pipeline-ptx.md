# CUDA の背後実装の PTX の入力のパイプラインの生成（TASK-804）の所在と本リポの現状

TASK-804 の作業のうち、CUDA の背後実装の `ComputeOps::create_pipeline` が `PipelineInput::Ptx` を受け付けて読み込む形と、NVRTC の呼び出し口の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-804。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入力の分類 | `crates/contract/src/backend_cuda.rs` の `classify_pipeline_input`。受け付けた入力は `AcceptedInput` で返す | `git grep -n "fn classify_pipeline_input\|enum AcceptedInput" -- crates/contract/src/backend_cuda.rs` |
| 読み込む PTX を作る | 同じファイルの `module_input` と `check_ptx_no_nul` | `git grep -n "fn module_input\|fn check_ptx_no_nul" -- crates/contract/src/backend_cuda.rs` |
| NVRTC の呼び出し口 | 同じファイルの `impl CudaDevice` の `compile_with_nvrtc` | `git grep -n "fn compile_with_nvrtc" -- crates/contract/src/backend_cuda.rs` |
| 口 | `impl ComputeOps for CudaDevice` の `create_pipeline` | `git grep -n "fn create_pipeline(" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `tests` module と `ptx_input_tests` module。device に触らない | 下の「確かめ方」 |

## PTX の入力の受け付けと NVRTC の呼び出し口（1/3）

`create_pipeline` は `module_input` に、device の NVRTC の呼び出し口 `compile_with_nvrtc` を渡して呼ぶ（`git grep -n "module_input(accepted" -- crates/contract/src/backend_cuda.rs`）。

入力ごとの分類と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| `CudaC` | 通る（`AcceptedInput::CudaC`） | `cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified` |
| `Ptx` | 通る（`AcceptedInput::Ptx`） | 同上 |
| `Wgsl` | `Unsupported { op: "create_pipeline", backend: Cuda }` | 同上 |
| `Msl`・`SpirV`・`Dxil`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions` | `ArtifactRejected { kind: "unsupported-input" }` | 同上 |

`Msl` と `MslSpecializedWithOptions` の理由の文は `pipeline input <variant> is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx`（`<variant>` は受けた variant の名前）。試験は `cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified`。

`module_input` が返す値と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| `Ptx` | テキストをそのまま PTX にし、入口名と返す。渡した呼び出し口を呼ばない | `ptx_input_becomes_the_module_text_without_calling_the_compiler` |
| `CudaC` | 渡した呼び出し口をソースで 1 回呼び、その結果と入口名を返す。呼び出し口の失敗はそのまま返す | `cuda_c_input_calls_the_compiler_once_with_the_source` |
| `Ptx` のテキストの NUL のバイト | `ArtifactRejected { kind: "module" }`。呼び出し口を呼ばない | `nul_bytes_are_rejected_before_the_module_text_is_made` |
| `CudaC` のソースの NUL のバイト | `ArtifactRejected { kind: "cuda-c" }`。呼び出し口を呼ばない | 同上 |
| 入口名の NUL のバイト（`Ptx`・`CudaC`） | `ArtifactRejected { kind: "function" }`。呼び出し口を呼ばない | 同上 |

テキストと入口名の両方に NUL のバイトがある `Ptx` は `module`（テキストを先に見る）。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::ptx_input_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::tests::cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified
```

1 つ目は 3 件、2 つ目は 1 件が通る。

## device の要る試験（2/3）

device に触る試験は `crates/contract/src/backend_cuda.rs` の `ptx_pipeline_device_tests` module にあり、どれも `#[ignore = "requires a CUDA device"]` を付けている。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。試験のビルドの device は `compile_with_nvrtc` に入った回数を持ち、試験はその回数を読む。PTX のテキストは、試験が CUDA C のカーネルを、`--gpu-architecture=compute_<major><minor>` と `--frandom-seed=0x5eed` を付けて NVRTC で作る（device の `compile_with_nvrtc` を通らない）。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `nvrtc_ptx_text_creates_a_pipeline_of_this_device` | `Ptx` の入力で `create_pipeline` を 2 回 | 2 回とも通り、その device の識別子を持つ。2 つの識別子は異なる。表の件数は 1 回ごとに 1 増える |
| `ptx_creation_does_not_enter_the_nvrtc_call_site` | `Ptx` の入力で `create_pipeline` を 2 回、続けて同じカーネルの `CudaC` の入力で 1 回 | `Ptx` の 2 回の後も `compile_with_nvrtc` に入った回数は 0。`CudaC` の 1 回の後は 1 |
| `ptx_load_failures_are_artifact_rejected_without_entering_the_nvrtc_call_site` | 読めないテキスト・NUL のバイトを含むテキスト・無い入口名・NUL のバイトを含む入口名の `Ptx` の入力で `create_pipeline`、続けて `CudaC` の入力で 1 回 | 読めないテキストと NUL のバイトを含むテキストは `ArtifactRejected { kind: "module" }`、無い入口名と NUL のバイトを含む入口名は `ArtifactRejected { kind: "function" }`。表の件数は増えない。4 つの後も `compile_with_nvrtc` に入った回数は 0。`CudaC` の 1 回の後は 1 |

| 項目 | 値 |
| ---- | -- |
| module | `crates/contract/src/backend_cuda.rs` の `ptx_pipeline_device_tests` |
| feature | `backend-cuda` |
| `#[ignore]` | `requires a CUDA device` |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::ptx_pipeline_device_tests
```

3 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | PTX の入力を NVRTC を呼ばずに読み込む形と NVRTC の呼び出し口を実装する | #5721 |
| 2/3 | PTX の入力のパイプラインの生成の device の要る試験を ignore 付きで足す | #5722 |
