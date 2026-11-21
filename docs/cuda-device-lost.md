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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 模擬のロストの後の操作の拒否を実装する | #3834 |
