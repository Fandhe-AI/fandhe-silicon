# CUDA の背後実装の `wait_blocking`（TASK-606）の所在と本リポの現状

TASK-606 の作業のうち、CUDA の背後実装の `CommandQueue::wait_blocking` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-606。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/contract/src/backend_cuda.rs` の `impl CommandQueue for CudaDevice` の `wait_blocking`（feature `native-blocking-wait` の裏） | `git grep -n "fn wait_blocking" -- crates/contract/src/backend_cuda.rs` |
| 待つ前の照らし | 同ファイルの `check_wait_token`・`Submissions::wait_step`・`WaitStep` | 同ファイル |
| 待ち | 同ファイルの `CudaDevice::wait_issued`・`wait_until_limit` | 同ファイル |
| 待ちの失敗の印 | 同ファイルの `Submissions::mark_wait_failure`（欄 `wait_failures`） | 同ファイル |
| 試験 | 同ファイルの `wait_blocking_tests`（`#[cfg(test)]`） | 下の「確かめ方」 |

## 待つ前の照らしと待ちの部品（1/3）

### 待つ前の照らしの部品

`check_wait_token` は次の順に照らし、最初に当たったものを返す。

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | ロスト済みを表す引数が真 | `DeviceLost`（`generation` は渡した device の世代） |
| 2 | 別の device の印 | `DeviceMismatch` |
| 3 | 印の世代が渡した device の世代と違う | `DeviceLost`（`generation` は印の世代） |
| 4 | ほか | `Ok(())` |

`Submissions::wait_step` は投入済みの表を変えずに、番号から次の一歩を返す。

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 払い出していない番号（0 と、最後に払い出した番号より大きいもの） | `Unissued { last }`（`last` は最後に払い出した番号、無ければ 0） |
| 2 | 待ちの失敗の印のある番号 | `Failed`（印の値） |
| 3 | 失敗した投入 | `Failed`（`BackendFailure`、`op` は `wait_blocking`） |
| 4 | 投入済みの表にある | `Wait` |
| 5 | ほか（回収済み） | `Done` |

`unissued_wait_error` は `InvalidArgument` を返し、文字列に `wait_blocking` と要求した番号と最後の番号を載せる。

### 期限つきの待ち

`wait_until_limit` は完了の確かめが `Pending` の間、眠りを挟んで確かめ直し、経過が期限に届くと `Timeout` を返す。
1 回目の確かめは期限によらず行い、期限 0 では眠らない。挟む眠りは残りの時間を超えない。

### 待ちの失敗の印

`mark_wait_failure` は番号に最初の失敗の値を付け、後の値で上書きしない。印の付いた番号は、`state_of` が `Lost` を返し、
投入済みの表から外れても `is_unfinished` が真を返す。

### 照らす試験

| 試験 | 照らす値 |
| ---- | -------- |
| `lost_is_checked_before_the_owner_and_the_generation` | ロスト済みは別の device の印にも今の世代の `DeviceLost`。ロストでなければ別の device の印は `DeviceMismatch`、世代違いは印の世代の `DeviceLost`、合う印は `Ok(())` |
| `unissued_sequences_are_rejected_without_changing_the_table` | 0・最後の番号 + 1・`u64::MAX` は `Unissued { last: 4 }`、照らした後も表の欄は同じ。何も払い出していない表の 1 は `Unissued { last: 0 }` |
| `the_unissued_error_names_the_operation_and_both_sequences` | `InvalidArgument` の文字列に `wait_blocking` と要求した番号と最後の番号がある |
| `issued_sequences_map_to_done_wait_or_failed` | 回収済みは `Done`、表に残る番号は `Wait`、失敗した投入は `BackendFailure`（`op` は `wait_blocking`、`backend` は `Cuda`） |
| `a_marked_sequence_keeps_its_first_error_and_stays_unfinished` | 2 回印を付けても最初の値。失敗した投入に印を付けると印の値。`state_of` は `Lost`。表から外しても `is_unfinished` は真 |
| `the_limited_wait_stops_on_a_terminal_state_or_at_the_limit` | 期限 500 µs で `Pending` が続くと `Timeout`。眠りの合計は 500 µs、確かめは 4 回。3 回目に `Complete` を返すと `Ok(Complete)` |
| `a_zero_limit_probes_once_and_never_sleeps` | 期限 0 は `Lost` を返す確かめに `Ok(Lost)`、`Pending` に `Timeout`。どちらも眠らない |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib wait_blocking_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count
```

## device の要る試験（2/3）

公開の口（`Device::memory`・`Device::queue`・`Device::fault_hooks`・`Device::generation`）だけを呼ぶ統合の試験。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_gated_copy_times_out_and_completes_after_the_gate_opens` | 門を仕込んでコピーを投入し、期限 100 ms で待ち、門を開けて `poll` を繰り返す | 待ちは `Err(Timeout)`、その後の `poll` は `Pending`。門を開けた後の `poll` は `Complete`、コピーの先は元の内容 |
| `a_limited_wait_returns_timeout_at_the_limit` | 門を仕込んで空の投入をし、期限 0・期限 50 ms で待ち、門を開けて上限なし・期限 0・期限 50 ms で待つ | 期限 0 は `Err(Timeout)` で 250 ms より早く戻る。期限 50 ms は `Err(Timeout)` で、戻るまでの時間は 50 ms 以上。門を開けた後は 3 つとも `Ok(())` |
| `an_unlimited_wait_returns_after_the_copy_and_the_clear` | 1 つの記録先にコピーとクリアを記録して投入し、上限なしで待つ | `Ok(())`。`poll` は `Complete`。コピーの先は元の内容、クリアした先は範囲だけ 0 |
| `the_wait_releases_held_buffers_and_calls_registered_callbacks` | 門で止めたコピーの元を `free` し、`on_complete` を登録し、門を開けずに上限なしで待ち、その後に門を開ける | 待つ前の解放の保留の数は `Some(1)`、callback は呼ばれていない。待った後は `Ok(())`、数は `Some(0)`、callback は `Complete` で呼ばれている |
| `a_reclaimed_submission_returns_ok` | 2 つ投入して待ち、1 つ目に上限なし・期限 0 で待ち直す | どれも `Ok(())` |
| `an_unissued_sequence_is_rejected_without_waiting` | 門で止めた投入の後、番号 0・最後の番号 + 1・`u64::MAX` の印に、上限なし・期限 0・期限 10 秒で待つ | 9 通りとも `Err(InvalidArgument(_))` で 250 ms より早く戻る。その後の `poll` は `Pending` |
| `a_token_of_another_device_is_a_mismatch` | 1 つ目の device の印を 2 つ目の device で、上限なし・期限 0 で待つ | どちらも `Err(DeviceMismatch)`。1 つ目の device では `Ok(())` |
| `a_lost_device_returns_device_lost_before_the_owner_check` | 模擬のロストの後、ロストの前の印と別の device の印に期限 50 ms で待つ | どちらも `Err(DeviceLost { generation })` で、`generation` は device の今の世代。別の device ではその印に `Ok(())` |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_wait_blocking.rs` |
| `required-features` | `backend-cuda`・`fault-injection`・`native-blocking-wait`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_wait_blocking
```

8 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | CUDA の device の wait_blocking を event の同期と期限つきの確かめで実装する | #5198 |
| 2/3 | wait_blocking の device の要る試験を ignore 付きで足す | #5199 |
