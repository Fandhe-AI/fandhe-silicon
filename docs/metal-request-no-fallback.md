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

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-277。ポインタのみ）。

時点のコミット: `db9ff8a`。`docs/spec` の gitlink は `3f9168084bba265e4d9044c513a30e7f2a84243d`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の本数と `#[ignore]` | `grep -c '#\[test\]' crates/contract/tests/backend_metal_request_no_fallback.rs`、`grep -c '#\[ignore' crates/contract/tests/backend_metal_request_no_fallback.rs` | `#[test]` は 4、`#[ignore` は 0 |
| 2 | macOS の試験の結果 | `gh run view 43623306060 --log --job 132533550933 \| grep -A12 'backend_metal_request_no_fallback'`（`make verify + build-profiles (macos-latest)` のジョブ） | `running 4 tests`、`test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`。run の headSha `db9ff8a7f58850ddc6cac80823aecf1c1f746c0a`、conclusion `success` |
| 3 | macOS 向けの型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,fault-injection,graphics -- -D warnings` | 終了コード 0 |
| 4 | 依存 0・unsafe 0・`src/` の差分 0 | `git diff --stat 49a0b7b..HEAD -- Cargo.lock deps-approved.toml crates/contract/src crates/core`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 5 | CI | `gh run view 43623306060 --json conclusion,headSha,jobs`（`db9ff8a` の main の push の run） | conclusion `success`、headSha `db9ff8a7f58850ddc6cac80823aecf1c1f746c0a`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success` |

(4) の `49a0b7b` は、1/2 の 1 つ前のコミット。`db9ff8a` が 1/2 のコミット。

(1) の出力。

```text
4
0
```

(2) の出力。

```text
running 4 tests
test metal_graphics_passes_the_checks_in_a_graphics_build ... ok
test failures_reach_both_policies_as_is ... ok
test other_backends_with_graphics_or_cpu_only_are_invalid_combinations ... ok
test the_metal_source_names_no_other_backend_provider ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(3) の出力。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

(4) の出力。

```text
（diff は出力なし）
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 組み合わせの検査と失敗の伝播の試験 | #4080 |
| 2/2 | 完了の条件の確かめ | #4081 |
