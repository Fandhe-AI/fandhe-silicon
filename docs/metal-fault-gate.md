# Metal の背後実装の試験用の門と遅延解放の試験（TASK-319）の所在と本リポの現状

TASK-319 の作業のうち、所在と本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-319。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 門の値の状態 | `crates/contract/src/backend_metal.rs` の `GateValues`・`GATE_EXPIRY`（どの OS でもビルドされる） | `grep -n 'struct GateValues\|const GATE_EXPIRY' crates/contract/src/backend_metal.rs` |
| 門の実装 | 同ファイルの `imp` の `GateInner`・`encode_gate_wait`・`spawn_gate_expiry`・`open_gates`・`expire_gate`・`MetalDevice::open_all_gates`・`impl FaultHooks for MetalDevice` の `arm_gate`・`open_gate`（macOS のビルドだけ） | `grep -n 'fn arm_gate\|fn open_gate\|fn encode_gate_wait\|fn spawn_gate_expiry' crates/contract/src/backend_metal.rs` |
| 試験 | 同ファイルの `gate_tests` | 下の「確かめ方」 |
| ソースの文字列の守り | `crates/contract/tests/backend_metal_no_recording.rs` の `the_gate_wait_is_the_only_event_wait_and_sits_in_the_fault_injection_gate` | 下の「確かめ方」 |

## 門の仕込みと開け（1/4）

| 操作 | 置き場 | 動き |
| ---- | ------ | ---- |
| `arm_gate` | `impl FaultHooks for MetalDevice` | 門を仕込む。仕込み済みなら何もしない。最初の呼び出しで共有の event を作り、作れなければ状態を変えずに `BackendFailure`（`op` は `arm_gate`）を返す。それ以外は `Ok(())` |
| `open_gate` | 同上 | 仕込みを外し、積んだ門を全部開ける。何度呼んでも `Ok(())` |
| 投入 | `commit_records`（`submit`・`submit_many`） | 仕込みがあれば、コマンドバッファの先頭（どの encoder よりも前）に event の値の待ちを積み、上限の見張りのスレッドを始めてから確定する。仕込みは確定の後に外す。確定の前に拒んだ・失敗した投入は仕込みを残す。値が尽きた・見張りのスレッドを始められないときは `BackendFailure`（`op` は `submit`）で、確定しない |
| 門の待ちの上限 | `GATE_EXPIRY` | 500 ms。積んでから過ぎると、門が閉じたままでも見張りのスレッドが門を開け、投入が先へ進む |
| 見張りのスレッド | `spawn_gate_expiry` | 門の錠と event だけに触る。バッファ・投入済みの表・callback には触らない |
| `Drop`・`Device::destroy` | `MetalDevice` | 投入済みの列を待つ前に門を全部開ける |

門は、値 `n` の門が event の値 `n` 以上で開く形で、開けた値は下げない。錠の順は、投入済みの表の錠、門の錠、`commit_hold`（試験のビルドだけ）の順。

feature `fault-injection` が無いビルドには、門の欄も門を積む処理もない。`impl FaultHooks for MetalDevice` と `fault_hooks()` もそのビルドには無い。`fault-injection` は `objc2-metal` の header の feature `MTLEvent` を有効にする（`crates/contract/Cargo.toml` の `[features]`）。

device に触らない試験 `gate_tests` の 8 本が、門の値の状態を照らす。

| 試験 | 照らす値 |
| ---- | -------- |
| `arming_gives_the_next_wait_value` | 仕込みが無ければ積む値が無く、仕込むと 1、2 回仕込んでも 1 |
| `reading_the_wait_value_keeps_the_arming` | 積む値を読んでも仕込みは外れない |
| `issuing_consumes_the_arming` | 積むと仕込みが外れ、次の仕込みは 2 |
| `opening_all_raises_to_the_highest_issued_value_once` | 積んだ 2 つを開ける値は 2 で、仕込みも外れ、2 回目は何も開けない |
| `opening_without_an_issued_gate_only_disarms` | 積んだ門が無ければ何も開けず、仕込みだけが外れる |
| `expiry_opens_up_to_its_value_and_never_lowers` | 上限の過ぎた門はその値まで開け、開けた値を下げない |
| `an_exhausted_value_is_an_error_and_keeps_the_state` | 値が尽きていたら `Err` で、状態は変わらない |
| `the_gate_expiry_is_500_ms` | 上限は 500 ms |

`backend_metal_no_recording.rs` の `backend_metal_code_has_no_words_beyond_compute_dispatch` は、門の待ちを積む 1 行（`buffer.encodeWaitForEvent_value(event, value);`）だけを照らしの外にする。`the_gate_wait_is_the_only_event_wait_and_sits_in_the_fault_injection_gate` が、その行がコード本体にちょうど 1 つで、`#[cfg(feature = "fault-injection")]` の付いた `encode_gate_wait` の中にあることを照らす。

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection --lib backend_metal::gate_tests
cargo test -p fandhe-silicon-contract --test backend_metal_unsafe_count --test backend_metal_no_recording
```
