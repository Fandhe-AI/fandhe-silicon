# CUDA の背後実装の投入状態と資源の寿命（TASK-347）の所在と本リポの現状

TASK-347 の作業のうち、CUDA の背後実装で、門で止めた投入に使ったバッファの解放の保留が待ちの `Timeout` と `poll_ex` の後も続くことと、dispatch を記録した記録先を投入せずに捨てたときの投入の回数と開いている記録先の数を照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-347。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_cuda.rs` | 同左 |
| 解放の保留の数 | [cuda-fault-gate](cuda-fault-gate.md) | 同左 |
| `wait_blocking` | [cuda-wait-blocking](cuda-wait-blocking.md) | 同左 |
| `poll`・`poll_ex` | [cuda-poll](cuda-poll.md) | 同左 |
| 記録先の集合と投入 | [cuda-command-submit](cuda-command-submit.md) | 同左 |
| dispatch の記録 | [cuda-dispatch-record](cuda-dispatch-record.md) | 同左 |

## 投入状態と資源の寿命の device の要る試験（1/2）

どの試験も CUDA の device に触るので `#[ignore]` を付け、通常の確認では走らず `ignored` になる。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。

統合の試験（公開の口の `Device::memory`・`Device::queue`・`Device::fault_hooks`）:

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_timeout_keeps_the_freed_source_of_a_gated_copy_held` | 門を仕込んでコピーを投入し、元を `free` してから、期限 100 ms の `wait_blocking`・`poll_ex`・`open_gate`・空の投入を呼ぶ | `free` の後の `pending_release_count` は `Some(1)`。`wait_blocking` は 100 ms 以上待って `Err(Timeout)` で、その後も `Some(1)`。`poll_ex` は `Pending` で、その後も `Some(1)`。門を開けると `poll_ex` が `Complete` になり、空の投入の後は `Some(0)` で、コピーの先は元の内容 |
| `sources_behind_a_gated_copy_stay_held_until_completion_is_observed` | 門を仕込んでコピーを投入し、門の無いコピーを続けて投入し、2 つの元を `free` してから、後ろの投入への期限 100 ms の `wait_blocking`・2 つの投入への `poll_ex`・`open_gate`・空の投入を呼ぶ | 2 つの `free` の後は `Some(2)`。2 つの投入の `poll_ex` は `wait_blocking` の `Err(Timeout)` の前も後も `Pending` で、`Some(2)` のまま。門を開けると 2 つとも `Complete` になり、空の投入の後は `Some(0)` で、2 つのコピーの先はそれぞれの元の内容 |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_submission_state.rs` |
| `required-features` | `backend-cuda`・`fault-injection`・`native-blocking-wait`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

lib の試験（`#[cfg(test)]` の数の口 `test_submitted_count`・`test_open_encoder_count` を使う）:

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `dropping_an_encoder_with_a_recorded_dispatch_submits_and_launches_nothing` | 4 語に定数を書く dispatch を記録した記録先を投入せずに捨て、空の投入を待ち、同じ dispatch を記録した別の記録先を投入して待つ | 開いている記録先の数は作成の前 0、記録の後 1、捨てた後 0。記録の後と捨てた後の空の投入の後は出力が 0 のまま。捨てた後の投入の回数は前と同じで、`pending_release_count` は `Some(0)`。別の記録先の投入の後は出力が定数になり、投入の回数は前より 2 多い |
| `a_timeout_keeps_a_freed_buffer_bound_to_a_gated_dispatch_held` | 門を仕込んで入力を出力に写す dispatch を投入し、入力を `free` してから、期限 100 ms の `wait_blocking`・`poll_ex`・`open_gate`・期限なしの `wait_blocking` を呼ぶ | `free` の後は `Some(1)`。`wait_blocking` は `Err(Timeout)` で、その後も `Some(1)`。`poll_ex` は `Pending` で、その後も `Some(1)`。門を開けて期限なしの `wait_blocking` が `Ok(())` を返した後は `Some(0)` で、出力は入力の写し |

| 置き場 | cfg・feature |
| ------ | ------------ |
| `crates/contract/src/backend_cuda.rs` の `submission_state_device_tests` | `test`・`native-blocking-wait`・`fault-injection` |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_submission_state
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --lib backend_cuda::submission_state_device_tests
```

どちらも 2 件が `ignored` になる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 投入状態と資源の寿命の device の要る試験を ignore 付きで足す | #4469 |
