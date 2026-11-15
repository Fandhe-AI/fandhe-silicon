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

## ほかの背後実装の provider を呼ばないことの試験（2/3）

Vulkan の `request` の失敗の後に、CUDA・Metal の provider の `request` が呼ばれないことを、各背後実装の `request_call_count`（TASK-277）で照らす。device に触らない入口だけを使う。

| 試験 | 入口 | 入力 | 確かめること |
| ---- | ---- | ---- | ------------ |
| `vulkan_request_failures_call_no_other_backend_provider`（`crates/contract/tests/backend_vulkan_request_no_other_provider.rs`） | `VulkanLoaderMissingProvider`・`VulkanDeviceProvider`・`CudaDeviceProvider`・macOS の `MetalDeviceProvider` | `ComputeOnly`・`ProductIndependent` の要求。fail-fast と縮退の利用コードの模擬で、Vulkan の差し込みの失敗と Cuda の要求の拒否を受ける | Vulkan・CUDA の数が `[0, 0]` → `[5, 0]` → `[5, 1]`。macOS かつ `backend-metal` のビルドでは Metal の数が 0 → 0 → 1 |

- `[[test]]` の `required-features` は `backend-vulkan`・`backend-cuda`・`fault-injection`
- cfg は wasm32 以外。Metal の数は macOS かつ `backend-metal` のビルドでだけ照らす
- 数は値そのもので照らし、エラーの照合は種類だけ（`matches!`）

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-vulkan,backend-cuda,fault-injection --test backend_vulkan_request_no_other_provider
cargo test -p fandhe-silicon-contract --all-features --test backend_vulkan_request_no_other_provider
```

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-284。ポインタのみ）。

時点のコミット: `b11d035`。`docs/spec` の gitlink は `e380dd445150e39139f92f6fc0ce05484d49ac0b`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の本数と `#[ignore` | `grep -c '#\[test\]'` と `grep -c '#\[ignore'` を `crates/contract/tests/backend_vulkan_request_combinations.rs`・`crates/contract/tests/backend_vulkan_request_no_other_provider.rs` に | `#[test]` は 3 と 1、`#[ignore` は 0 と 0 |
| 2 | 数を足す呼び出しの数 | `crates/contract/src/backend_{metal,vulkan,cuda}.rs` に `grep -c 'note_request_call();'` と `grep -c 'pub fn request_call_count'` | `note_request_call();` は Metal 3・Vulkan 2・CUDA 2、`pub fn request_call_count` は各 1 |
| 3 | 組み合わせの入力の受け手 | `crates/contract/tests/` の 3 ファイルに `grep -n '^fn '` | `backend_vulkan_request_combinations.rs` の 3 本（`other_backends_with_graphics_or_cpu_only_are_invalid_combinations`・`vulkan_graphics_passes_the_checks_in_a_graphics_build`・`the_vulkan_source_names_no_other_backend_provider`）。`backend_vulkan_request.rs` の 5 本（`non_vulkan_backend_is_invalid_combination`・`cpu_only_is_invalid_combination`・`graphics_without_feature_is_invalid_combination`・`shared_and_imported_are_unsupported`・`backend_is_checked_before_profile_and_ownership`）。`backend_vulkan_fault.rs` の `injected_providers_check_the_request_first` |
| 4 | `DeviceUnavailable` の受け手 | `crates/contract/tests/backend_vulkan_fault.rs` の `grep -n '^fn '` | `injected_initialization_failures_are_device_unavailable`・`an_injected_failure_is_distinct_from_the_other_kinds`。どちらも差し込みの provider（feature `fault-injection`）で確かめる |
| 5 | Linux の試験の結果 | run `43743082637`（`b11d035` の main の push）の job `132900309590` のログを試験の関数名で引く | 下の (5) の出力 |
| 6 | macOS の試験の結果 | 同じ run の job `132900309818` のログ | 下の (6) の出力 |
| 7 | 依存と `src/`・`crates/core` の差分 | `git diff --stat 87331dbb HEAD -- Cargo.lock deps-approved.toml crates/contract/src crates/core`、`make deps-check` | diff は出力なし。`make deps-check` は終了コード 0 |
| 8 | CI | `gh run view 43743082637 --json conclusion,headSha,jobs` | conclusion `success`、headSha `b11d03521349bdcfa349513b94cf6150f71c4b64`。job `132900309590`・`132900309818`・`132900309706` は `success` |

(7) の `87331dbb` は、1/3 の 1 つ前のコミット。`e3f62ba` が 1/3、`b11d035` が 2/3 のコミット。

(5) の出力。

```text
Running tests/backend_vulkan_request_combinations.rs
running 3 tests
test other_backends_with_graphics_or_cpu_only_are_invalid_combinations ... ok
test vulkan_graphics_passes_the_checks_in_a_graphics_build ... ok
test the_vulkan_source_names_no_other_backend_provider ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Running tests/backend_vulkan_request_no_other_provider.rs
running 1 test
test vulkan_request_failures_call_no_other_backend_provider ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(6) の出力。

```text
Running tests/backend_vulkan_request_no_other_provider.rs
running 1 test
test vulkan_request_failures_call_no_other_backend_provider ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 組み合わせの検査の試験 | #4116 |
| 2/3 | ほかの背後実装の provider を呼ばないことの試験 | #4117 |
| 3/3 | 完了の条件の確かめ | #4118 |
