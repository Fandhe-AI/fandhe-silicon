# Metal の背後実装の模擬のロストの後の `gpu_timing` の試験（TASK-596）の所在と本リポの現状

TASK-596 の作業のうち、模擬のロストの後の `gpu_timing` の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-596。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験のファイル | `crates/contract/tests/backend_metal_gpu_timing_lost.rs` | 同左 |
| `[[test]]` の feature | `backend-metal`・`native-blocking-wait`・`fault-injection` | `crates/contract/Cargo.toml` |
| cfg | `target_os = "macos"`。macOS 以外では 0 件 | 下の「確かめ方」 |
| `gpu_timing` のほかの device の試験 | `crates/contract/tests/backend_metal_device.rs` の `gpu_timing_` で始まる試験 | `grep -n "fn gpu_timing_" crates/contract/tests/backend_metal_device.rs` |
| 模擬のロストの口 | `docs/contract-fault-injection.md` の節「Metal の `fault_hooks`（TASK-2932）」 | 同左 |

## 模擬のロストの後の `gpu_timing` の device の要る試験（1/2）

試験は `gpu_timing_is_unavailable_after_the_simulated_loss` の 1 本で、`#[ignore]` が付く。
入口は `MetalDeviceProvider::request`（`Metal`・`ComputeOnly`・`ProductIndependent`）で作った device の `Device::queue()` の `submit`・`wait_blocking`・`gpu_timing` と、`Device::fault_hooks()` に `fandhe_silicon_core::fault::inject` で `HookFault::DeviceLost(DeviceLostReason::Unknown)` を渡す口。

| 順 | 呼ぶ口 | 返る値 |
| -- | ------ | ------ |
| 1 | 空の投入（`submit`）を `wait_blocking(token, None)` で待つ | `Ok` |
| 2 | `gpu_timing(token)` | `GpuTiming::Available { .. }` |
| 3 | `inject(device.fault_hooks(), HookFault::DeviceLost(DeviceLostReason::Unknown))` | `Ok` |
| 4 | 同じ `token` の `gpu_timing(token)` | `GpuTiming::Unavailable` |
| 5 | 同じ `token` の `gpu_timing(token)`（もう 1 回） | `GpuTiming::Unavailable` |

macOS 以外ではファイルの全体が 0 件になる。

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,fault-injection --test backend_metal_gpu_timing_lost
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait,fault-injection -- -D warnings
```

1 本目は macOS 以外では `running 0 tests`、macOS では `ignored` の件数が 1 になる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後の `gpu_timing` の device の要る試験 | #5140 |
