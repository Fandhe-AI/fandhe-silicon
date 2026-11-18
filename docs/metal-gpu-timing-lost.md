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

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-596。ポインタのみ）。

時点のコミット: `9cb83d5f`。`docs/spec` の gitlink は `becfefdc2fb47fa610c6ecc914f3bf601910f5f3`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `grep -c '#\[test\]'`、`grep -c '^[[:space:]]*#\[ignore'`（どちらも `crates/contract/tests/backend_metal_gpu_timing_lost.rs`） | `#[test]` は 1、`#[ignore` は 1 |
| 2 | 受け手の試験の名前 | `git grep -n "fn gpu_timing_" -- crates/contract/tests crates/contract/src/backend_metal.rs` | 下の (2) の出力 |
| 3 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait,fault-injection -- -D warnings` | 終了コード 0 |
| 4 | CI の macOS のビルド | `gh run view 44234792683 --log --job 134568207897`（`make verify + build-profiles (macos-latest)`）の `backend_metal_gpu_timing_lost` の部分。run は `afecd5cf` の main の push | `test result: ok. 0 passed; 0 failed; 1 ignored`（`make verify` と `make build-profiles` の 2 か所）、run の conclusion `success`、headSha `afecd5cfec6b68c8d71ecabbf5bc357fa156428b` |
| 5 | 依存 0・unsafe 0 | `git diff --stat cc92aa34..HEAD -- Cargo.lock deps-approved.toml crates/core crates/contract/src`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 6 | CI | `gh run view 44234792683 --json conclusion,headSha,jobs` | `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`、headSha `afecd5cfec6b68c8d71ecabbf5bc357fa156428b` |

(5) の `cc92aa34` は、1/2 の 1 つ前のコミット。

(2) の出力。

```text
crates/contract/src/backend_metal.rs:10000:    fn gpu_timing_covers_the_four_returns() {
crates/contract/src/backend_metal.rs:10011:    fn gpu_timing_boundaries_follow_the_watermark_and_the_failed_set() {
crates/contract/src/backend_metal.rs:10025:    fn gpu_timing_keeps_a_recorded_time_after_reclaiming() {
crates/contract/src/backend_metal.rs:10048:    fn gpu_timing_does_not_reclaim_or_change_the_table() {
crates/contract/tests/backend_metal_device.rs:1073:fn gpu_timing_is_available_after_wait_blocking() {
crates/contract/tests/backend_metal_device.rs:1099:fn gpu_timing_is_available_after_a_sleep_then_wait_blocking() {
crates/contract/tests/backend_metal_device.rs:1123:fn gpu_timing_before_waiting_is_not_available_and_does_not_reclaim() {
crates/contract/tests/backend_metal_device.rs:1144:fn gpu_timing_of_a_token_reclaimed_by_poll_is_reclaimed() {
crates/contract/tests/backend_metal_device.rs:1160:fn gpu_timing_after_a_bounded_wait_is_not_available() {
crates/contract/tests/backend_metal_device.rs:1176:fn gpu_timing_after_on_complete_is_not_available() {
crates/contract/tests/backend_metal_device.rs:1216:fn gpu_timing_of_a_foreign_or_unissued_token_is_unavailable() {
crates/contract/tests/backend_metal_device.rs:1233:fn gpu_timing_table_drops_the_oldest_after_1025_waits() {
crates/contract/tests/backend_metal_gpu_timing_lost.rs:31:fn gpu_timing_is_unavailable_after_the_simulated_loss() {
```

(4) の出力。

```text
Running tests/backend_metal_gpu_timing_lost.rs (target/debug/build/fandhe-silicon-contract/2d9bf4aca721c571/out/backend_metal_gpu_timing_lost-2d9bf4aca721c571)

running 1 test
test gpu_timing_is_unavailable_after_the_simulated_loss ... ignored, touches the Metal device; run only in a dedicated device session

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後の `gpu_timing` の device の要る試験 | #5140 |
| 2/2 | 完了の条件の確かめ | #5141 |
