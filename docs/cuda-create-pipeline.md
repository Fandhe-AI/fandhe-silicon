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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 入力の分類と配置の検査 | #5713 |
