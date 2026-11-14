# Metal の背後実装の request の組み合わせの検査と失敗の伝播の試験（TASK-277）の所在と本リポの現状

TASK-277 の作業のうち、Metal の背後実装の `DeviceProvider::request` の組み合わせの検査と失敗の伝播の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-277。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験のファイル | `crates/contract/tests/backend_metal_request_no_fallback.rs` | 同ファイル |
| `[[test]]` の `required-features` | `backend-metal`・`fault-injection` | `crates/contract/Cargo.toml` |
| cfg | `target_os = "macos"`（macOS 以外では 0 件）。1 本は加えて `feature = "graphics"` | 同ファイルの先頭と該当の試験 |
| 試験 | 4 本（`graphics` を付けないビルドでは 3 本） | 下の「確かめ方」 |

## 組み合わせの検査と失敗の伝播の試験（1/2）

試験は device に触らない入口だけを使う。入口は 2 つで、検査で拒まれる要求を渡した `MetalDeviceProvider::request` と、検査を通っても既定の device の取得を呼ばない `MetalDefaultDeviceNoneProvider::request`（feature `fault-injection`）。

| 試験 | 入口 | 入力 | 確かめること |
| ---- | ---- | ---- | ------------ |
| `other_backends_with_graphics_or_cpu_only_are_invalid_combinations` | 両方 | `Vulkan`・`Cuda`・`Dx12Compute` と `Graphics`・`CpuOnly`（`ProductIndependent`）の全組 | どれも `InvalidCombination` |
| `metal_graphics_passes_the_checks_in_a_graphics_build`（feature `graphics`） | `MetalDefaultDeviceNoneProvider` | `Metal`・`Graphics`・`ProductIndependent` | `DeviceUnavailable`（検査を通って device の取得まで進む） |
| `failures_reach_both_policies_as_is` | 両方 | `Metal`・`ComputeOnly`（差し込み）と `Vulkan`・`ComputeOnly`（検査の失敗） | 失敗の値が 2 つの利用コードの模擬へそのまま届く（観測するのは返り値のエラーの伝播だけ） |
| `the_metal_source_names_no_other_backend_provider` | なし | `crates/contract/src/backend_metal.rs` のソース | `VulkanDeviceProvider`・`CudaDeviceProvider`・`backend_vulkan`・`backend_cuda` を含まない |

エラーの照合は値の種類だけ（`matches!`）。

確かめ方（macOS）:

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection --test backend_metal_request_no_fallback
cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection,graphics --test backend_metal_request_no_fallback
```

前者は 3 本、後者は 4 本を流す。Linux での型検査:

```bash
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,fault-injection,graphics -- -D warnings
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 組み合わせの検査と失敗の伝播の試験 | #4080 |
