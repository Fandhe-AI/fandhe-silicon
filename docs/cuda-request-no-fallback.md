# CUDA の背後実装の request の組み合わせの検査と、ほかの背後実装の provider を呼ばないことの試験（TASK-290）の所在と本リポの現状

TASK-290 の作業のうち、CUDA の背後実装の `DeviceProvider::request` の組み合わせの検査の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-290。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験のファイル | `crates/contract/tests/backend_cuda_request_combinations.rs` | 同ファイル |
| `[[test]]` の `required-features` | `backend-cuda`・`fault-injection` | `crates/contract/Cargo.toml` |
| cfg | `not(target_arch = "wasm32")` | 同ファイルの先頭 |
| 試験 | 2 本 | 下の「確かめ方」 |
| `DeviceUnavailable` の試験 | feature `fault-injection` の差し込みの provider で確かめる | `crates/contract/tests/backend_cuda_fault.rs` |
| request の検査の試験（TASK-160） | `crates/contract/tests/backend_cuda_request.rs` | 同ファイル |

## 組み合わせの検査の試験（1/3）

試験は device に触らない入口だけを使う。入口は、検査で拒まれる要求を渡した `CudaDeviceProvider::request` と、差し込みの 5 つの provider の `request`（feature `fault-injection`）。

| 試験 | 入口 | 入力 | 確かめること |
| ---- | ---- | ---- | ------------ |
| `other_backends_with_graphics_or_cpu_only_are_invalid_combinations` | `CudaDeviceProvider` と差し込みの 5 つ | `Metal`・`Vulkan`・`Dx12Compute` と `Graphics`・`CpuOnly`（`ProductIndependent`）の全組 | どれも `InvalidCombination` |
| `the_cuda_source_names_no_other_backend_provider` | なし | `crates/contract/src/backend_cuda.rs` のソース | `MetalDeviceProvider`・`VulkanDeviceProvider`・`backend_metal`・`backend_vulkan` を含まない |

エラーは値の種類だけ（`matches!`）で見る。

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_request_combinations
```

2 本を流す。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 組み合わせの検査の試験 | #4148 |
