# CUDA の背後実装の完了の通知と検証の報告の受け口（TASK-607）の所在と本リポの現状

TASK-607 の作業のうち、所在と本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-607。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `impl ValidationIntake for CudaDevice` | 同ファイル |
| 試験 | 同ファイルの `validation_intake_tests` | 下の「確かめ方」 |

## 検証の報告の受け口の登録（1/3）

| 操作 | 置き場 | 動き |
| ---- | ------ | ---- |
| `set_validation_callback` | `CudaDevice` の `ValidationIntake` の実装 | 登録を受け付ける |
| 登録の保持 | 欄 `validation`（`ValidationReporter`） | 登録済みなら置き換えて保持する |
| 報告 | なし | この背後実装からは呼ばれない |

device に触らない試験 `the_cuda_device_takes_a_validation_callback` が、`CudaDevice` が `ValidationIntake` を実装し `Send + Sync` であることを型で照らす。

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib validation_intake_tests
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 検証の報告の受け口の登録を CUDA の device で受け付ける | #5206 |
