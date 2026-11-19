# CUDA の背後実装の試験用の門（TASK-345）の所在と本リポの現状

TASK-345 の作業のうち、所在と本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-345。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `GateState`・`GateLatch`・`gate_host_fn`・`CudaDevice::launch_gate`・`impl FaultHooks for CudaDevice` | 同ファイル |
| 試験 | 同ファイルの `gate_tests` | 下の「確かめ方」 |

## 門の仕込みと開け（1/3）

| 操作 | 置き場 | 動き |
| ---- | ------ | ---- |
| `arm_gate` | `impl FaultHooks for CudaDevice` | 門を仕込む。仕込み済みなら何もしない。常に `Ok(())` |
| `open_gate` | 同上 | 仕込んだ門と積んだ門を全部開ける。何度呼んでも `Ok(())` |
| 投入 | `CudaDevice::issue_event` | 仕込んだ門があれば event の前に門を待つ host の関数を積む。積めなければ `BackendFailure`（`op` は `launch_host_function`）で、通し番号を消費せず仕込みを残す。門を積んだ後に event の記録が失敗したら、門は積んだまま残る |
| 門の待ちの上限 | `GATE_EXPIRY` | 500 ms。過ぎると門が閉じたままでも host の関数が戻り、stream が先へ進む |
| `Drop`・`Device::destroy` | `CudaDevice` | 同期の前に門を全部開ける |
| `free` の中の stream の記録 | `MemoryOps::free` | 門を積まない |

feature `fault-injection` が無いビルドには、門の欄も門を積む処理もない。`impl FaultHooks for CudaDevice` と `fault_hooks()` もそのビルドには無い。

device に触らない試験 `gate_tests` の 8 本が、門の部品を照らす。

| 試験 | 照らす値 |
| ---- | -------- |
| `attaching_without_an_armed_gate_launches_nothing` | 仕込みが無ければ積む手段は 0 回で、積んだ門は 0 |
| `arming_twice_launches_one_gate_for_the_next_submission_only` | 2 回仕込んでも積む手段は 1 回で、仕込みは消え、積んだ門は 1 つで閉じている |
| `opening_before_the_submission_clears_the_armed_gate` | 投入の前に開けると積む手段は 0 回で、仕込みは消える |
| `a_failed_launch_keeps_the_armed_gate` | 積めなかったときは仕込みが残り、次の投入が同じ門を積み、開けるとその門も開く |
| `opening_opens_every_issued_gate_and_is_idempotent` | 積んだ 2 つが開いて一覧が空になり、もう一度開けても開いたまま |
| `an_opened_gate_is_dropped_from_the_issued_list_on_the_next_attach` | 開いた門は次に積むときに一覧から外れる |
| `waiting_on_a_closed_gate_returns_false_after_the_limit` | 閉じた門の待ちは上限の後に偽を返す |
| `waiting_returns_true_once_another_thread_opens_the_gate` | 別のスレッドが開けると待ちが真を返す |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib gate_tests
cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
```

## device の要る試験（2/3）

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_gated_submission_stays_pending_until_the_gate_opens` | 門を仕込んで空の投入をし、`poll` と `open_gate` を呼ぶ | 開けるまで `Pending`、開けると上限の半分より早く `Complete`、もう一度開けても `Ok(())` |
| `a_later_submission_waits_behind_the_gate` | 門を仕込んで空の投入を 2 つする | 2 つ目も開けるまで `Pending`、開けると 2 つとも `Complete` |
| `a_closed_gate_expires_and_the_submission_completes` | 門を仕込んで空の投入をし、開けない | 先に `Pending`、上限が過ぎると `Complete` |
| `opening_before_the_submission_leaves_it_ungated` | 門を仕込んで開けてから空の投入をする | 上限の半分より早く `Complete` |
| `dropping_with_a_closed_gate_returns_before_the_expiry` | 閉じた門を積んだまま `drop` する | 上限の半分より早く戻る |
| `destroying_with_a_closed_gate_returns_before_the_expiry` | 閉じた門を積んだまま `destroy` する | 上限の半分より早く戻る |

| 項目 | 値 |
| ---- | -- |
| module | `gate_device_tests`（`crates/contract/src/backend_cuda.rs`） |
| `cfg` | `#[cfg(all(test, feature = "fault-injection"))]` |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib gate_device_tests
```

6 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 試験用の門の仕込みと開け | #4455 |
| 2/3 | 門で止めた投入の device の要る試験 | #4456 |
