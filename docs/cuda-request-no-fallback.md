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

## ほかの背後実装の provider を呼ばないことの試験（2/3）

CUDA の `request` の失敗の後に、Vulkan の provider（macOS かつ `backend-metal` のビルドでは Metal の provider も）の `request` が呼ばれないことを、`request_call_count`（TASK-277 で足した口）で照らす。device に触らない入口だけを使う。

| 試験 | 入口 | 入力 | 確かめること |
| ---- | ---- | ---- | ------------ |
| `cuda_request_failures_call_no_other_backend_provider`（`crates/contract/tests/backend_cuda_request_no_other_provider.rs`） | `CudaDriverMissingProvider`・`CudaDeviceProvider`・`VulkanDeviceProvider`（macOS かつ `backend-metal` では `MetalDeviceProvider`） | CUDA の provider に `Cuda`・`Vulkan` の要求（fail-fast の模擬 3 回と縮退の模擬 2 回）、続けて Vulkan・Metal の provider に `Cuda` の要求（検査で拒まれる） | CUDA・Vulkan の数が `[0, 0]` → `[5, 0]` → `[5, 1]`。macOS かつ `backend-metal` のビルドでは Metal の数が 0 → 0 → 1 |

- `[[test]]` の `required-features`: `backend-cuda`・`backend-vulkan`・`fault-injection`（`crates/contract/Cargo.toml`）
- cfg は `not(target_arch = "wasm32")`。Metal の数は macOS かつ `backend-metal` のビルドでだけ照らす
- 数は値そのもので照らし、エラーの照合は値の種類だけ（`matches!`）

確かめ方。

- Linux: `cargo test -p fandhe-silicon-contract --features backend-cuda,backend-vulkan,fault-injection --test backend_cuda_request_no_other_provider`
- macOS: `cargo test -p fandhe-silicon-contract --all-features --test backend_cuda_request_no_other_provider`

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-290。ポインタのみ）。

時点のコミット: `26674d0`。`docs/spec` の gitlink は `e380dd445150e39139f92f6fc0ce05484d49ac0b`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の本数と `#[ignore]` | `grep -c '#\[test\]'` と `grep -c '#\[ignore'` を `crates/contract/tests/backend_cuda_request_combinations.rs`・`crates/contract/tests/backend_cuda_request_no_other_provider.rs` に | `#[test]` は 2 と 1、`#[ignore` は 0 と 0 |
| 2 | 数を足す呼び出しの数 | `crates/contract/src/backend_{metal,vulkan,cuda}.rs` に `grep -c 'note_request_call();'` と `grep -c 'pub fn request_call_count'` | `note_request_call();` は Metal 3・Vulkan 2・CUDA 2、`pub fn request_call_count` は各 1 |
| 3 | 組み合わせの入力を受ける試験 | `grep -n '^fn '` を `crates/contract/tests/backend_cuda_request_combinations.rs`・`backend_cuda_request.rs`・`backend_cuda_fault.rs`・`backend_cuda_profiles.rs` に | `backend_cuda_request_combinations.rs` は `other_backends_with_graphics_or_cpu_only_are_invalid_combinations`・`the_cuda_source_names_no_other_backend_provider`。`backend_cuda_request.rs` は `other_backends_are_invalid_combinations`・`graphics_and_cpu_only_are_invalid_combinations`・`shared_and_imported_are_unsupported`・`the_backend_is_checked_before_the_profile_and_the_profile_before_the_ownership`。`backend_cuda_fault.rs` は `the_request_checks_come_before_an_injected_failure`。`backend_cuda_profiles.rs` の device の要らない 2 本は `cpu_only_is_an_invalid_combination_for_every_ownership`・`graphics_is_an_invalid_combination_for_every_ownership` |
| 4 | `DeviceUnavailable` を受ける試験 | 同じ `grep -n '^fn '` の `backend_cuda_fault.rs` の行 | `each_injected_failure_is_an_initialization_failure`・`an_injected_failure_is_distinct_from_the_other_kinds`。feature `fault-injection` の差し込みの provider で確かめる |
| 5 | Linux の試験の結果 | run `43742314781`（`26674d0` の main の push）の job `132897957916` のログを試験の関数名で引く | 下の (5) の出力 |
| 6 | macOS の試験の結果 | 同じ run の job `132897957747` のログ | 下の (6) の出力 |
| 7 | 依存・`crates/contract/src`・`crates/core` | `git diff --stat f7ab34b 26674d0 -- Cargo.lock deps-approved.toml crates/contract/src crates/core`、`make deps-check` | diff は出力なし。`make deps-check` は終了コード 0 |
| 8 | CI | `gh run view 43742314781 --json conclusion,headSha,jobs` | conclusion `success`、headSha `26674d0b1467ba93b9be60f825bc1aa2cf553c4e`。job `132897957916`・`132897957747`・`132897957830` は `success` |

(7) の `f7ab34b` は、1/3 の 1 つ前のコミット。`87331db` が 1/3、`26674d0` が 2/3 のコミット。

(5) の出力。

```text
Running tests/backend_cuda_request_combinations.rs
running 2 tests
test other_backends_with_graphics_or_cpu_only_are_invalid_combinations ... ok
test the_cuda_source_names_no_other_backend_provider ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Running tests/backend_cuda_request_no_other_provider.rs
running 1 test
test cuda_request_failures_call_no_other_backend_provider ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(6) の出力。

```text
Running tests/backend_cuda_request_no_other_provider.rs
running 1 test
test cuda_request_failures_call_no_other_backend_provider ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 組み合わせの検査の試験 | #4148 |
| 2/3 | ほかの背後実装の provider を呼ばないことの試験 | #4149 |
| 3/3 | 完了の条件の確かめ | #4150 |
