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

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-593。ポインタのみ）。

時点のコミット: `9cb83d5f`。`docs/spec` の gitlink は `becfefdc2fb47fa610c6ecc914f3bf601910f5f3`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `grep -c '#\[test\]'`、`grep -c '^[[:space:]]*#\[ignore'`（どちらも `crates/contract/tests/backend_metal_poll_lost.rs`） | `#[test]` は 1、`#[ignore` は 1 |
| 2 | 受け手の試験の名前 | 下の (2) の `git grep` | 下の (2) の出力（7 行） |
| 3 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait,fault-injection -- -D warnings` | 終了コード 0 |
| 4 | CI の macOS のビルド | `gh run view 44235061797 --log --job 134569088230`（`make verify + build-profiles (macos-latest)`）の `backend_metal_poll_lost` の部分。run は `9cb83d5f` の main の push | `test result: ok. 0 passed; 0 failed; 1 ignored`（`make verify` と `make build-profiles` の 2 か所）、run の conclusion `success`、headSha `9cb83d5fc886ab262f2e3d9612aea6420e33b17a` |
| 5 | 依存 0・unsafe 0 | `git diff --stat afecd5cf..HEAD -- Cargo.lock deps-approved.toml crates/core crates/contract/src`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 6 | CI | `gh run view 44235061797 --json conclusion,headSha,jobs` | `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`、headSha `9cb83d5fc886ab262f2e3d9612aea6420e33b17a` |

(5) の `afecd5cf` は、1/2 の 1 つ前のコミット。

(2) の出力（`git grep -n -e "fn poll_and_poll_ex_are_lost_after_the_simulated_loss" -e "fn poll_ex_is_complete_after_waiting" -e "fn reclaimed_tokens_poll_as_complete" -e "fn poll_ex_of_unissued_is_unknown_and_of_a_foreign_token_is_lost" -e "fn drive_path_poll_calls_a_pending_registration" -e "fn drive_path_poll_ex_calls_a_pending_registration" -e "fn poll_ex_covers_pending_complete_runtime_error_and_unknown_as_distinct_values" -- crates/contract`）。

```text
crates/contract/src/backend_metal.rs:5934:        fn drive_path_poll_calls_a_pending_registration() {
crates/contract/src/backend_metal.rs:5948:        fn drive_path_poll_ex_calls_a_pending_registration() {
crates/contract/src/backend_metal.rs:9473:    fn poll_ex_covers_pending_complete_runtime_error_and_unknown_as_distinct_values() {
crates/contract/tests/backend_metal_device.rs:1022:fn poll_ex_is_complete_after_waiting() {
crates/contract/tests/backend_metal_device.rs:1033:fn reclaimed_tokens_poll_as_complete() {
crates/contract/tests/backend_metal_device.rs:1049:fn poll_ex_of_unissued_is_unknown_and_of_a_foreign_token_is_lost() {
crates/contract/tests/backend_metal_poll_lost.rs:31:fn poll_and_poll_ex_are_lost_after_the_simulated_loss() {
```

(4) の出力。

```text
Running tests/backend_metal_poll_lost.rs (target/debug/build/fandhe-silicon-contract/168f303e18a50437/out/backend_metal_poll_lost-168f303e18a50437)

running 1 test
test poll_and_poll_ex_are_lost_after_the_simulated_loss ... ignored, touches the Metal device; run only in a dedicated device session

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後の `poll`・`poll_ex` の device の要る試験 | #5118 |
| 2/2 | 完了の条件の確かめ | #5119 |
