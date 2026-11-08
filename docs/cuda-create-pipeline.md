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
| NVRTC の失敗 | `ArtifactRejected { kind: "cuda-c" }`。`NVRTC_ERROR_OUT_OF_MEMORY` は `BackendFailure { op: <呼び出しの名前> }` | `nvrtc_failures_map_to_artifact_rejected_or_backend_failure` |
| モジュールの読み込み・関数の取得の失敗 | `ArtifactRejected { kind: "module" }` / `{ kind: "function" }`。メモリ不足は `BackendFailure` | 同上 |
| ドライバや NVRTC を呼ぶ中の panic | `BackendFailure { op: "create_pipeline" }` | `a_panic_in_the_pipeline_guard_is_backend_failure` |

表に入るものは、取得した関数と `layout` の写し。`layout` は `slots` の重複の検査と写しのほかには読まない。識別子は device ごとに 1 から払い出し、尽きたときは `BackendFailure { op: "create_pipeline" }` を返す。device は読み取った compute capability を持つ（試験は `attributes_keep_the_compute_capability`）。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 入力の分類と配置の検査 | #5713 |
| 2/4 | NVRTC のコンパイルとパイプラインの表 | #5714 |
