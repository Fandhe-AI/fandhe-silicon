# CUDA の背後実装の模擬のロストの後のパイプラインの生成とロストの理由（TASK-561）の所在と本リポの現状

TASK-561 の作業のうち、CUDA の背後実装の模擬のロストの後のパイプラインの生成と、模擬のロストの理由の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-561。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_cuda.rs` | 同左 |
| `create_pipeline` | `CudaDevice` の `ComputeOps` の実装。最初に `check_not_lost` を呼ぶ | `git grep -n -F "fn create_pipeline(" -- crates/contract/src/backend_cuda.rs` |
| ロストの後の操作の拒否の部品 | [cuda-device-lost](cuda-device-lost.md) | 同左 |
| 検証の報告の受け口の登録 | [cuda-on-complete](cuda-on-complete.md) | 同左 |

## ロストの後のパイプラインの生成と device の要る試験（1/2）

どの試験も CUDA の device に触るので `#[ignore]` を付け、通常の確認では走らず `ignored` になる。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `create_pipeline_after_the_loss_returns_device_lost_without_compiling` | 具体型の device の `create_pipeline`・`create_pipeline_checked` と、`Device::compute` から得た `create_pipeline` | ロストの前の `create_pipeline` で、NVRTC の呼び出し口に入った回数と下位の呼び出しの前に数えた回数が増える。ロストの後は `CudaC`・`Ptx`・`Msl`・`Wgsl`・ソースに NUL のバイトを持つ `CudaC`・重複した slot の入力と、引数の宣言が合う `create_pipeline_checked` が `Err(DeviceLost { generation: generation() })`。2 回繰り返しても同じで、世代・パイプラインの表の件数と次の識別子・2 つの回数は変わらない |
| `a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback` | 具体型の device の `set_lost_callback`・`set_validation_callback`・`simulate_validation_report`・`fault_hooks` | 報告を模す差し込みは `Err(UnsupportedExtensionDefault { op: "simulate_validation_report" })` で、ロストせず世代も変わらない。理由に `Unknown`・`Unrecoverable` を指定した模擬のロストは、通知の列がその理由の 1 件で、世代が進む。検証の報告の受け口に登録した関数は呼ばれず、届けた数と届けなかった数は 0。ロストの後の `create_pipeline` は `Err(DeviceLost { generation: generation() })` |

| 置き場 | cfg・feature |
| ------ | ------------ |
| `crates/contract/src/backend_cuda.rs` の `lost_pipeline_device_tests` | `test`・`fault-injection` |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib backend_cuda::lost_pipeline_device_tests
```

2 件が `ignored` になる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後のパイプラインの生成を拒み、device の要る試験を足す | #4214 |
