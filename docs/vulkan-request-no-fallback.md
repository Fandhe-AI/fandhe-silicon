# Vulkan の背後実装の request の組み合わせの検査と、ほかの背後実装の provider を呼ばないことの試験（TASK-284）の所在と本リポの現状

TASK-284 の作業のうち、Vulkan の背後実装の `DeviceProvider::request` の組み合わせの検査の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-284。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験のファイル | `crates/contract/tests/backend_vulkan_request_combinations.rs` | 同ファイル |
| `[[test]]` の `required-features` | `backend-vulkan`・`fault-injection` | `crates/contract/Cargo.toml` |
| cfg | wasm32 以外。1 本は加えて `feature = "graphics"` | 同ファイルの先頭と該当の試験 |
| 試験 | 3 本（`graphics` を付けないビルドでは 2 本） | 下の「確かめ方」 |
| `DeviceUnavailable` の試験 | 差し込みの provider（feature `fault-injection`）で確かめる。`crates/contract/tests/backend_vulkan_fault.rs`（TASK-159） | 同ファイル |
| request の試験 | `crates/contract/tests/backend_vulkan_request.rs`（TASK-159） | 同ファイル |

## 組み合わせの検査の試験（1/3）

試験は device に触らない入口だけを使う。入口は 2 つで、検査で拒まれる要求を渡した `VulkanDeviceProvider::request` と、差し込みの provider 7 個の `request`（feature `fault-injection`）。

| 試験 | 入口 | 入力 | 確かめること |
| ---- | ---- | ---- | ------------ |
| `other_backends_with_graphics_or_cpu_only_are_invalid_combinations` | 両方 | `Metal`・`Cuda`・`Dx12Compute` と `Graphics`・`CpuOnly`（`ProductIndependent`）の全組 | どれも `InvalidCombination` |
| `vulkan_graphics_passes_the_checks_in_a_graphics_build`（feature `graphics`） | 差し込みの provider 7 個 | `Vulkan`・`Graphics`・`ProductIndependent` | どれも `DeviceUnavailable`（検査を通って差し込みの失敗まで進む） |
| `the_vulkan_source_names_no_other_backend_provider` | なし | `crates/contract/src/backend_vulkan.rs` のソース | `MetalDeviceProvider`・`CudaDeviceProvider`・`backend_metal`・`backend_cuda` を含まない |

エラーの照合は値の種類だけ（`matches!`）。

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-vulkan,fault-injection --test backend_vulkan_request_combinations
cargo test -p fandhe-silicon-contract --features backend-vulkan,fault-injection,graphics --test backend_vulkan_request_combinations
```

前者は 2 本、後者は 3 本を流す。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 組み合わせの検査の試験 | #4116 |
