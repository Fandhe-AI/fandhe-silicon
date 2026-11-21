# CUDA の背後実装の模擬のロストの後の操作の拒否と作り直し（TASK-188）の所在と本リポの現状

TASK-188 の作業のうち、CUDA の背後実装の模擬のロストの後の操作の拒否の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-188。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_cuda.rs` | 同左 |
| `lost_result` | `lost` と `generation` を受け、device に触らずに `Ok(())` か `DeviceLost { generation }` を返す | `git grep -n -F "fn lost_result(" -- crates/contract/src/backend_cuda.rs` |
| `check_not_lost` | `CudaDevice` のメソッド。`is_lost` の値と呼んだ時点の `generation()` を `lost_result` に渡す。`fault-injection` の無いビルドでは `is_lost` は常に偽 | `git grep -n -F -e "fn check_not_lost(" -e "fn is_lost(" -- crates/contract/src/backend_cuda.rs` |
| 模擬のロストの口 | `docs/cuda-simulated-device-lost.md` | 同左 |

## ロストの後の操作の拒否（1/3）

device に触らない試験（`crates/contract/src/backend_cuda.rs` の `lost_check_tests`）は次のとおり。

| 試験 | 内容 |
| ---- | ---- |
| `lost_result_passes_when_not_lost` | `lost` が偽なら複数の世代で `Ok(())` |
| `lost_result_carries_the_given_generation_when_lost` | `lost` が真なら複数の世代で、その世代を載せた `DeviceLost` |

`backend_cuda::device_tests::lost::a_handle_from_before_the_loss_can_still_be_freed_once` は、ロストの後のロスト前のハンドルの `upload` を `Err(DeviceLost { generation: d.generation() })` で照らす。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::lost_check_tests
```

2 件が通る。

## device の要る試験（2/3）

どの試験も CUDA の device に触るので `#[ignore]` を付け、通常の確認では走らず `ignored` になる。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。

| 試験 | 置き場 | 呼ぶ口 | 照らす値 |
| ---- | ------ | ------ | -------- |
| `two_losses_call_the_callback_once_and_move_the_generation_once` | `lost_invalidation_device_tests` | 具体型の device の `set_lost_callback` と `fault_hooks` | 通知の列が `[Unknown]` の 1 件。1 回目の後に世代が進み、2 回目の後は同じ値 |
| `the_listed_operations_issue_nothing_after_the_loss` | `lost_invalidation_device_tests` | 具体型の device | 並べた操作が `Err(DeviceLost { generation: generation() })`、`poll`・`poll_ex` は `Lost`。0 の長さ・表に無いハンドル・開いていない記録先・0 の要素・空でない `wait`・空の `submit_many` も同じ。2 回繰り返しても同じで、バッファの表の件数・次の識別子・投入済みの表の次の通し番号は変わらず、空の記録先の記録の数は 0 のまま。ロスト前のバッファの `free` は表から外れる |
| `a_wait_that_spans_the_loss_returns_device_lost` | `lost_invalidation_device_tests` | 具体型の device の `arm_gate`・`open_gate` | 門で止めた投入を待っている間に模擬のロストを起こし、門を開けると、待ちが `Err(DeviceLost { generation: generation() })` を返す。同じ印の `poll` は `Lost` |
| `a_new_request_after_the_loss_runs_a_dispatch_and_reads_back` | `lost_invalidation_device_tests` | `CudaDeviceProvider::request` の新しい device | 世代がロストの前後のどちらよりも大きい。確保・`upload`・コピーと dispatch を記録した記録先の投入・読み戻しが期待と一致。ロストした device の `allocate` は `DeviceLost` のまま |
| `the_listed_operations_are_lost_through_the_public_device` | `crates/contract/tests/backend_cuda_device_lost.rs` | 公開の `Device` の `memory()`・`queue()`・`compute()` | 並べた操作が `DeviceLost`、`poll`・`poll_ex` は `Lost`。dispatch を記録した記録先の `submit`・`submit_many` も `Unsupported` ではなく `DeviceLost` |
| `a_new_request_after_the_loss_round_trips_through_a_copy` | `crates/contract/tests/backend_cuda_device_lost.rs` | 公開の `Device` | 世代がロストした device の後の値より大きい。確保・`upload`・コピーを記録した記録先の `submit`・読み戻しが一致。ロストした device の `allocate` は `DeviceLost` のまま |

| 置き場 | cfg・feature |
| ------ | ------------ |
| `crates/contract/src/backend_cuda.rs` の `lost_invalidation_device_tests` | `test`・`native-blocking-wait`・`fault-injection` |
| `crates/contract/tests/backend_cuda_device_lost.rs` | `[[test]]` の `required-features` は `backend-cuda`・`native-blocking-wait`・`fault-injection` |

上の 2 本目・3 本目と統合の試験の 1 本目が照らす、ロストの後の値は次のとおり。`generation` は呼んだ時点の `generation()` の値。

| 操作 | ロストの後に返す値 | 2 本目で同じ値を照らす入力 |
| ---- | ------------------ | -------------------------- |
| `allocate` | `Err(DeviceLost { generation })` | 0 の長さ |
| `upload` | `Err(DeviceLost { generation })` | 表に無いハンドル |
| `download_blocking`・`download_async` | `Err(DeviceLost { generation })`（`download_async` は callback に渡す） | — |
| `copy_buffer_to_buffer`・`clear_buffer` | `Err(DeviceLost { generation })` | 開いていない記録先 |
| `dispatch`・`dispatch_ex`・`dispatch_checked` | `Err(DeviceLost { generation })`（`dispatch_checked` は宣言が合う入力） | 開いていない記録先と 0 の要素（`dispatch_ex`） |
| `submit`・`submit_many` | `Err(DeviceLost { generation })` | dispatch を記録した記録先、空でない `wait`（`submit`）、空の `submit_many` |
| `wait_blocking` | `Err(DeviceLost { generation })`（待っている間のロストを含む。3 本目） | — |
| `poll`・`poll_ex` | `Lost` | ロストの前の印（`poll`・`poll_ex`）と、今の世代の印（`poll`） |

通常の確認:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --lib backend_cuda::lost_invalidation_device_tests
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --test backend_cuda_device_lost
```

1 つ目は 4 件、2 つ目は 2 件が `ignored` になる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 模擬のロストの後の操作の拒否を実装する | #3834 |
| 2/3 | device の要る試験を足す | #3835 |
