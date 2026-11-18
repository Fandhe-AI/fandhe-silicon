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

## device の要る試験（2/3）

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_reclaimed_token_calls_back_complete_once_inside_the_call` | 回収済みの印を `on_complete` に渡す | 呼び出しの中で `Complete` を 1 回、呼ばれたスレッドは呼び出し元、登録の表の件数は 0、続けて `poll` を呼んでも 1 件のまま |
| `a_foreign_token_calls_back_lost_once_without_registering` | 別 device の印を `on_complete` に渡す | 呼び出しの中で `Lost` を 1 回、登録の表の件数は 0、通し番号は呼ぶ前と同じ、続けて投入と `poll` を呼んでも 1 件のまま |
| `an_unissued_sequence_calls_back_lost_once_without_registering` | 払い出していない番号の印を `on_complete` に渡す | 呼び出しの中で `Lost` を 1 回、登録の表の件数は 0、続けて投入と `poll` を呼んでも 1 件のまま |
| `a_submitted_token_calls_back_complete_exactly_once_and_never_pending` | 投入した印を `on_complete` に渡し、`poll` と投入を続ける | `Complete` がちょうど 1 回で `Pending` は現れない、呼ばれたスレッドは呼び出し元、登録の表の件数は 0 |
| `the_validation_callback_is_not_called_by_normal_operations` | 登録した関数のあとに確保・書き込み・投入・`poll`・`on_complete`・解放を呼ぶ | 関数は呼ばれず、`delivered_count` と `undelivered_count` は 0。登録し直しても同じ |

| 項目 | 値 |
| ---- | -- |
| module | `on_complete_device_tests`（`crates/contract/src/backend_cuda.rs`） |
| `cfg` | `#[cfg(test)]` |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib on_complete_device_tests
```

5 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 検証の報告の受け口の登録を CUDA の device で受け付ける | #5206 |
| 2/3 | on_complete と検証の報告の受け口の device の要る試験を ignore 付きで足す | #5207 |
