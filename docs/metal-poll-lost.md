# Metal の背後実装の模擬のロストの後の `poll`・`poll_ex` の試験（TASK-593）の所在と本リポの現状

TASK-593 の作業のうち、模擬のロストの後の `poll`・`poll_ex` の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-593。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験のファイル | `crates/contract/tests/backend_metal_poll_lost.rs` | 同左 |
| `[[test]]` の feature | `backend-metal`・`native-blocking-wait`・`fault-injection` | `crates/contract/Cargo.toml` |
| cfg | `target_os = "macos"`。macOS 以外では 0 件 | 下の「確かめ方」 |
| 模擬のロストの口 | `docs/contract-fault-injection.md` の「Metal の `fault_hooks`（TASK-2932）」の節 | 同左 |

## 模擬のロストの後の `poll`・`poll_ex` の device の要る試験（1/2）

試験は `poll_and_poll_ex_are_lost_after_the_simulated_loss` の 1 本で、`#[ignore]` が付く。
入口は `MetalDeviceProvider::request`（`Metal`・`ComputeOnly`・`ProductIndependent`）で作った device の `Device::queue()` の `submit`・`wait_blocking`・`poll`・`poll_ex` と、`Device::fault_hooks()` に `fandhe_silicon_core::fault::inject` で `HookFault::DeviceLost` を差し込む口。

空の投入を 1 回して `wait_blocking` で待った印を「待った印」、その番号に 98 を足した払い出していない番号の印を「払い出していない印」とする。

| 順 | 印 | 呼ぶ口 | 返る値 |
| -- | -- | ------ | ------ |
| ロストの前 | 待った印 | `poll_ex` | `Complete` |
| ロストの前 | 待った印 | `poll` | `Complete` |
| ロストの前 | 払い出していない印 | `poll_ex` | `Unknown` |
| ロストの後 | 待った印 | `poll_ex` | `Lost` |
| ロストの後 | 待った印 | `poll` | `Lost` |
| ロストの後 | 払い出していない印 | `poll_ex` | `Lost` |

差し込みは `HookFault::DeviceLost(DeviceLostReason::Unknown)`。ロストの後に新しい投入はしない。

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,fault-injection --test backend_metal_poll_lost
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait,fault-injection -- -D warnings
```

1 本目は macOS 以外では `running 0 tests`、macOS では `ignored` の件数が 1 になる。device の所在は `docs/metal-device-request.md`（TASK-158）。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後の `poll`・`poll_ex` の device の要る試験 | #5118 |
