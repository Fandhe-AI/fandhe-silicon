# CUDA の背後実装の模擬のロスト（TASK-2934）の所在と本リポの現状

TASK-2934 の作業のうち、CUDA の背後実装の device の世代の扱いの所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2934。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | `backend_cuda.rs` の `mod tests`。device に触らない | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda` |

device の取得・診断・確保は [cuda-device-request.md](./cuda-device-request.md) を参照。

## 世代の読み方と `free` の照合（1/4）

device の世代は `AtomicU64` で持ち、`current_generation()` で読む。1 つの操作の中の照合と払い出しには、その操作で 1 回読んだ値を使う。

世代を読む操作は次のとおり。

- `Device::generation()`
- ハンドルの照合（`range`）
- `allocate` の払い出し
- 投入の印の照合と払い出し（`submit`・`submit_many`・`poll`）
- `on_complete` の登録

`free` の照らし方は次のとおり。

| 入力 | 動き |
| ---- | ---- |
| 別 device のハンドル | 何もしない |
| 表の項目の払い出しの世代と同じ世代のハンドル | 表から外し、今までの解放の流れに進む |
| 世代が違う・表に無い識別子 | 何もしない |

完了の callback の登録は、登録した印の世代を持つ。

| 登録の世代 | 駆動の動き |
| ---------- | ---------- |
| 駆動が読んだ世代より小さい | 列の状態に関わらず `Lost` |
| 等しい | 列の状態 |
| 大きい | 表に残す |

呼ぶ直前に世代が進んでいれば、決めた値に関わらず `Lost` で呼ぶ。`on_complete` は表の錠の中で世代を読み直し、違えば積まずに `Lost` で呼ぶ。

| 試験 | 確かめること |
| ---- | ------------ |
| `remove_issued_matches_the_issue_generation_not_the_current_one` | 世代違い・表に無い識別子は外さず、一致した項目だけを 1 回外す |
| `remove_issued_ignores_forged_ids_and_an_empty_table` | 空の表と偽の識別子は何も外さず、別の項目を巻き込まない |
| `take_ready_answers_lost_for_an_older_generation_and_keeps_a_newer_one` | 登録の世代が小さければ `Lost`、大きければ表に残す |
| `a_complete_judged_before_the_generation_moved_is_delivered_as_lost` | 世代が進んだ後は `Lost` で呼ぶ |

確かめ方: `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda`

## 模擬のロスト（2/4）

部品は feature `backend-cuda` と `fault-injection` の両方を付けたビルドにある（`Submissions::take_stale` は試験のビルドにもある）。
`FaultHooks::simulate_device_lost(reason)` の動きは次のとおり。

| 呼び出し | 動き |
| -------- | ---- |
| 1 回目 | 理由を記録し、`generation()` を前より大きい値に取り直す。登録済みのコールバックを錠の外で 1 回呼び、その後に登録の世代が古い完了の callback を表から外して `Lost` で 1 回ずつ呼ぶ |
| 2 回目以降 | 何もせず `Ok`。世代もコールバックの呼び出しの数も変わらない |
| 世代の源が尽きている | 状態を変えず `BackendFailure` |

- 理由は `Destroyed`・`Unknown`・`ValidationError`・`Unrecoverable` の 4 値ともそのまま渡す
- ロストの通知先は `DeviceLostNotify::set_lost_callback` で登録する。登録済みなら上書きし、ロストの後の登録は保持するだけで呼ばない。`Box<dyn Device>` からは届かないので、crate の内側の試験から登録する
- `Drop` ではロストのコールバックを呼ばない
- `arm_gate`・`open_gate`・`simulate_runtime_error`・`simulate_validation_report` は既定の本体のまま（`UnsupportedExtensionDefault`）。ロストの後の操作の拒否はこの段では持たない

| 試験 | 確かめること |
| ---- | ------------ |
| `take_stale_returns_older_registrations_as_lost_once` | 登録の世代が古いものだけを `Lost` で外し、2 回目は空 |
| `the_callback_runs_once_and_a_second_lose_is_ignored` | コールバックは 1 回で、2 回目は無視する |
| `every_reason_is_passed_through_unchanged` | 4 値の理由をそのまま渡す |
| `the_generation_is_taken_once` | 世代は 1 回だけ取る |
| `an_exhausted_source_leaves_the_state_unchanged` | 源が尽きたら状態を変えない |
| `a_callback_registered_after_the_loss_is_kept_but_not_called` | ロストの後の登録は呼ばない |
| `a_loss_without_a_callback_then_a_registration_never_calls_it` | コールバック無しのロストの後の登録も呼ばない |
| `dropping_an_overwritten_callback_may_reenter_without_deadlock` | 上書きで捨てる値の `Drop` が再入しても詰まらない |
| `concurrent_loses_call_the_callback_once_and_advance_the_source_once` | 並行のロストでも 1 回だけ |
| `a_later_registration_replaces_the_earlier_one` | 後の登録が前の登録を置き換える |
| `the_callback_runs_outside_the_lock` | コールバックは錠の外で呼ぶ |
| `a_poisoned_lock_does_not_panic` | 毒化した錠でも panic しない |

確かめ方: `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 世代の読み方と `free` の照合 | #4228 |
| 2/4 | 模擬のロスト | #4229 |
