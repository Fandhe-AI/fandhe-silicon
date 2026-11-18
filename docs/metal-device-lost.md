# Metal の背後実装の模擬のロストの後の操作の拒否と作り直し（TASK-186）の所在と本リポの現状

TASK-186 の作業のうち、Metal の背後実装の模擬のロストの後の操作の拒否の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-186。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_metal.rs` | 同左 |
| `lost_result` | `imp` の外の関数。`lost` と `generation` を受け、device に触らずに `Ok(())` か `DeviceLost { generation }` を返す | `grep -n 'fn lost_result' crates/contract/src/backend_metal.rs` |
| `check_not_lost` | `MetalDevice` のメソッド。`fault-injection` のビルドでは `LostState::is_lost`、無いビルドでは `false` を `lost_result` に渡す。世代は呼んだ時点の `generation()` | `grep -n 'fn check_not_lost' crates/contract/src/backend_metal.rs` |
| 模擬のロストの口 | `docs/contract-fault-injection.md` の「Metal の `fault_hooks`（TASK-2932）」の節 | 同左 |

## ロストの後の操作の拒否（1/3）

`check_not_lost` は、次の操作の先頭（ほかの照らしより前）で呼ぶ。ロスト済みなら、呼んだ時点の `generation()` を載せた値を返す。

| 操作 | 照らす位置 | ロストの後に返す値 |
| ---- | ---------- | ------------------ |
| `allocate` | 先頭と、駆動の直後 | `Err(DeviceLost { generation })` |
| `upload` | 先頭 | `Err(DeviceLost { generation })` |
| `download_blocking`・`download_async`（`read_range` を通る） | 先頭 | `Err(DeviceLost { generation })`（`download_async` は callback に渡す） |
| `dispatch_ex` | 先頭 | `Err(DeviceLost { generation })` |
| `dispatch_checked` | 先頭 | `Err(DeviceLost { generation })` |
| `copy_buffer_to_buffer`・`clear_buffer` | 先頭 | `Err(DeviceLost { generation })` |
| `submit`・`submit_many` | 先頭と、駆動の直後 | `Err(DeviceLost { generation })` |
| `wait_blocking` | 先頭（印の照合と `timeout` の分岐の前） | `Err(DeviceLost { generation })` |
| `poll` | 先頭 | `CompletionState::Lost` |
| `poll_ex` | 先頭 | `CompletionStateEx::Lost` |

変えない操作の今の値は次のとおり。

| 操作 | 今の値 |
| ---- | ------ |
| `free` | ロストの後もロスト前のハンドルで 1 回外せる |
| `on_complete` | 変更なし |
| `gpu_timing` | 変更なし |

`fault-injection` の無いビルドでは、照らしは常に通る。

device に触らない試験（`crates/contract/src/backend_metal.rs` の `lost_check_tests`）は次のとおり。

| 試験 | 内容 |
| ---- | ---- |
| `lost_result_passes_when_not_lost` | `lost` が偽なら複数の世代で `Ok(())` |
| `lost_result_carries_the_given_generation_when_lost` | `lost` が真なら複数の世代で、その世代を載せた `DeviceLost` |
| `is_lost_turns_true_after_the_first_lose_and_stays_true` | `fault-injection` のビルド。`lose` の前は偽、1 回目の後も 2 回目の後も真 |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection --lib lost_check_tests
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,fault-injection -- -D warnings
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 模擬のロストの後の操作を `DeviceLost` で拒む | #3818 |
