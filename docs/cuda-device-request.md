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
- device の識別子は共通の crate の `__backend_next_device_id` から取る（源は [contract-device-types](./contract-device-types.md) の「背後実装をまたぐ device の識別子の源」）。世代はこの背後実装の中の源から取る
- 確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda`

## 初期化の失敗の差し込み（3/5）

次の provider は実際の故障ではない差し込みで、feature `backend-cuda` と `fault-injection` の両方を付けたビルドにだけある。

| provider | 差し込む失敗 | 返す値 |
| -------- | ------------ | ------ |
| `CudaDriverMissingProvider` | ドライバのライブラリが読めない | `DeviceUnavailable`（文字列に `injected` を含む） |
| `CudaContextNewFailsProvider` | context の作成の失敗 | `DeviceUnavailable`（同上） |
| `CudaNameFailsProvider` | GPU の名前の読み取りの失敗 | `DeviceUnavailable`（同上。属性の読み取りの写し方を通る） |
| `CudaComputeCapabilityFailsProvider` | compute capability の読み取りの失敗 | `DeviceUnavailable`（同上。同じ写し方を通る） |
| `CudaTotalMemFailsProvider` | メモリの大きさの読み取りの失敗 | `DeviceUnavailable`（同上。同じ写し方を通る） |

- 各 provider は request の検査（1/5 の表）を先に行い、弾かれる入力では `CudaDeviceProvider` と同じ値を返す
- 差し込みの値の `failure_class` は `Initialization` で、`Timeout`・`CapabilityInsufficient`・実行時エラー・ロストと値と種類で区別できる
- `CudaDeviceProvider` の経路は変わらない
- 試験は `crates/contract/tests/backend_cuda_fault.rs`。device に触らない
- 確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_fault`

「所在」の表の公開の型の行と試験の行は 1/5 の時点の値である。今は feature `fault-injection` の裏に上の 5 つの provider があり、試験に `crates/contract/tests/backend_cuda_fault.rs` が加わっている。

## device の要る試験（4/5）

置き場は `crates/contract/tests/backend_cuda_device.rs`。feature `backend-cuda` と `native-blocking-wait` の両方を付けたビルドにだけある。どの試験も `#[ignore = "requires a CUDA device"]` を付けていて、`--ignored` を付けたときだけ走る。

| 試験 | 確かめること |
| ---- | ------------ |
| `request_returns_a_cuda_device_that_answers_queries_and_round_trips_bytes` | `request` の成功、`backend()` が `Cuda`、`query` が `Unsupported`、`query_ex` が `UnknownFeature`、`max_buffer_bytes` が 0 より大きいこと、64 バイトの `allocate`・`upload`・`download_blocking` の往復 |
| `diagnostics_report_the_cuda_backend_name` | `backend_name` が `"cuda"`、`adapter_name` が空でない |
| `adapter_name_carries_the_compute_capability` | `adapter_name` が ` (cuda compute_capability=` を含む。得た値を `adapter_name=<値>` の 1 行で標準出力に出す |
| `two_devices_do_not_accept_each_others_buffers` | 2 つ目の `generation` が大きい。別の device のハンドルでの `upload`・`download_blocking` は `DeviceMismatch` で、自分のバッファは変わらない |
| `download_async_calls_back_once_with_the_same_bytes` | コールバックが 1 回呼ばれ、`upload` した内容を受ける |
| `upload_to_a_freed_handle_is_rejected_as_invalid_argument` | `free` の後の `upload`・`download_blocking` は `InvalidArgument` |
| `freeing_another_devices_handle_keeps_the_buffer_with_the_same_id` | 別の device のハンドルの `free` は無視され、同じ識別子のバッファが残る |
| `a_handle_that_differs_only_in_generation_is_rejected_and_ignored_by_free` | 世代だけが違う偽のハンドルの `upload` は `InvalidArgument`、`free` は無視され、正規のハンドルが使える |
| `allocate_of_zero_bytes_is_invalid_argument` | `allocate(0)` は `InvalidArgument` |
| `allocate_past_the_buffer_limit_is_invalid_argument` | `max_buffer_bytes` を超える `allocate` は `InvalidArgument` |
| `out_of_range_upload_fails_and_leaves_the_contents_unchanged` | 範囲外の `upload` は `InvalidArgument` で、内容が変わらない |
| `out_of_range_download_blocking_fails_and_leaves_the_contents_unchanged` | 範囲外の `download_blocking` は `InvalidArgument` で、内容が変わらない |

- 確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --test backend_cuda_device` が `test result: ok. 0 passed; 0 failed; 12 ignored` の形で通る

「所在」の表の試験の行は 1/5 の時点の値である。今は `crates/contract/tests/backend_cuda_device.rs` も加わっている。

## 完了の条件の確かめ（5/5）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-160。ポインタのみ）。

時点のコミット: `899857a`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | request の検査 | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_request` | 4 passed、終了コード 0 |
| 2 | 差し込み | `cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_fault` | 3 passed、終了コード 0 |
| 3 | 単体 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` | 5 passed、終了コード 0 |
| 4 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --test backend_cuda_device` | 0 passed; 0 failed; 12 ignored、終了コード 0 |
| 5 | 個数テスト | `cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 1 passed、終了コード 0 |
| 6 | 公開 API | `make public-api-sdk-check` | `sdk_type_refs_total=0`、終了コード 0 |
| 7 | wasm32 | `make wasm-backend-check` | `name=cudarc wasm32_n=0 native_n=1 result=match`、終了コード 0 |
| 8 | 依存グラフ | `make profile-dep-graph-check` | 下の (8) の行、終了コード 0 |
| 9 | 依存の検査 | `make deps-check` | 終了コード 0。`deps-approved.toml` の `[[approval]]` は 5 件 |
| 10 | CI | `gh run view 42373294505`（main の push の run） | conclusion `success`、headSha `899857ae813dd73906f7a11badcce1df334232f3`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success` |

(1) の出力。

```text
running 4 tests
test other_backends_are_invalid_combinations ... ok
test graphics_and_cpu_only_are_invalid_combinations ... ok
test the_backend_is_checked_before_the_profile_and_the_profile_before_the_ownership ... ok
test shared_and_imported_are_unsupported ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。

```text
running 3 tests
test an_injected_failure_is_distinct_from_the_other_kinds ... ok
test each_injected_failure_is_an_initialization_failure ... ok
test the_request_checks_come_before_an_injected_failure ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(3) の出力。

```text
running 5 tests
test backend_cuda::tests::a_panic_in_the_guard_is_device_unavailable ... ok
test backend_cuda::tests::a_panic_in_the_memory_guard_is_backend_failure ... ok
test backend_cuda::tests::grid_dims_come_from_attributes_and_reject_failures ... ok
test backend_cuda::tests::attribute_failures_are_device_unavailable ... ok
test backend_cuda::tests::absent_library_is_device_unavailable_without_panicking ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

(4) の出力。

```text
running 12 tests
test a_handle_that_differs_only_in_generation_is_rejected_and_ignored_by_free ... ignored, requires a CUDA device
test adapter_name_carries_the_compute_capability ... ignored, requires a CUDA device
test allocate_of_zero_bytes_is_invalid_argument ... ignored, requires a CUDA device
test allocate_past_the_buffer_limit_is_invalid_argument ... ignored, requires a CUDA device
test diagnostics_report_the_cuda_backend_name ... ignored, requires a CUDA device
test download_async_calls_back_once_with_the_same_bytes ... ignored, requires a CUDA device
test freeing_another_devices_handle_keeps_the_buffer_with_the_same_id ... ignored, requires a CUDA device
test out_of_range_download_blocking_fails_and_leaves_the_contents_unchanged ... ignored, requires a CUDA device
test out_of_range_upload_fails_and_leaves_the_contents_unchanged ... ignored, requires a CUDA device
test request_returns_a_cuda_device_that_answers_queries_and_round_trips_bytes ... ignored, requires a CUDA device
test two_devices_do_not_accept_each_others_buffers ... ignored, requires a CUDA device
test upload_to_a_freed_handle_is_rejected_as_invalid_argument ... ignored, requires a CUDA device

test result: ok. 0 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(5) の出力。

```text
running 1 test
test backend_cuda_unsafe_count_matches_expected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(6) の出力。

```text
record: documents=10 public_items_total=13617 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
```

(7) の出力。

```text
deps: feature=backend-cuda name=objc2-metal wasm32_n=0 native_n=0 result=match
deps: feature=backend-cuda name=ash wasm32_n=0 native_n=0 result=match
deps: feature=backend-cuda name=cudarc wasm32_n=0 native_n=1 result=match
```

(8) の出力。

```text
baseline: commit=899857ae813dd73906f7a11badcce1df334232f3 profile=all-features crates_total=13 sdk_total=2
baseline-count: profile=all-features crate=objc2-metal n=1
baseline-count: profile=all-features crate=ash n=0
baseline-count: profile=all-features crate=cudarc n=1
diff: profile=cpu-only baseline=all-features sdk_baseline=2 sdk_profile=0 diff=2
diff: profile=compute-only-metal baseline=all-features sdk_baseline=2 sdk_profile=1 diff=1
diff: profile=graphics-vulkan baseline=all-features sdk_baseline=2 sdk_profile=0 diff=2
diff: profile=graphics-interop baseline=all-features sdk_baseline=2 sdk_profile=1 diff=1
```

(9) の出力。

```text
$ grep -c '^\[\[approval\]\]' deps-approved.toml
5
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | request の検査と provider の入口 | #3718 |
| 2/5 | device の取得・診断・確保 | #3719 |
| 3/5 | 初期化の失敗の差し込み | #3720 |
| 4/5 | device の要る試験 | #3721 |
| 5/5 | 完了の条件の確かめ | #3722 |
