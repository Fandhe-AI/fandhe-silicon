# CUDA の背後実装の device の取得・診断・確保（TASK-160）の所在と本リポの現状

TASK-160 の作業のうち、CUDA の背後実装の `DeviceProvider::request` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-160。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 公開の型 | `CudaDeviceProvider` | `grep -n 'pub ' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | `crates/contract/tests/backend_cuda_request.rs`。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_request` |

## request の検査（1/5）

`request` が要求を照らす順（device を作る前。最初に当たったものを返す）:

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 背後実装が CUDA でない | `InvalidCombination` |
| 2 | `Graphics`・`CpuOnly` | `InvalidCombination` |
| 3 | `Shared`・`Imported` | `Unsupported { backend: Cuda, .. }` |

- 背後実装と profile の拒否は、所有形態の拒否より先に見る
- `Graphics` は contract の feature `graphics` の有無によらず拒む

## device の取得・診断・確保（2/5）

依存の記録の所在は `deps-approved.toml` の `cudarc`。

`request` は検査（上の表）を通った後に device を作る。device に触らない試験が示す動き:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| ドライバの共有ライブラリが読み込めない | panic せず `DeviceUnavailable` | `absent_library_is_device_unavailable_without_panicking`（ライブラリがある環境では device に触らず戻る） |
| ドライバを呼ぶ閉包の中の panic | `DeviceUnavailable`（panic の内容は載せない） | `a_panic_in_the_guard_is_device_unavailable` |
| メモリ操作でドライバを呼ぶ閉包の中の panic | `BackendFailure`（panic の内容は載せない） | `a_panic_in_the_memory_guard_is_backend_failure` |
| GPU の名前・compute capability・メモリの大きさの読み取りの失敗 | `DeviceUnavailable` | `attribute_failures_are_device_unavailable` |
| grid の各軸の上限の読み取りの失敗・負の値 | `DeviceUnavailable` | `grid_dims_come_from_attributes_and_reject_failures` |

- 読み取りに成功したときの `adapter_name` は `<名前> (cuda compute_capability=<major>.<minor>)` の形
- `limits().max_dispatch_dims` は、読み取った各軸の上限の値
- 確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | request の検査と provider の入口 | #3718 |
| 2/5 | device の取得・診断・確保 | #3719 |
