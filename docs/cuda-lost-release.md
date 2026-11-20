# CUDA の背後実装の模擬のロストの後の解放の保留と回収（TASK-348）の所在と本リポの現状

TASK-348 の作業のうち、CUDA の背後実装で、模擬のロストの後に、門で止めたコピーに使ったバッファを `free` したときの解放の保留と、門を開けた後の回収を照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-348。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `free` | `crates/contract/src/backend_cuda.rs` の `impl MemoryOps for CudaDevice` の `free` と、関数 `hold_or_release` | `git grep -n "fn free(" -- crates/contract/src/backend_cuda.rs` と `git grep -n "fn hold_or_release" -- crates/contract/src/backend_cuda.rs` |
| 保留の回収 | `crates/contract/src/backend_cuda.rs` の `impl CudaDevice` の `release_finished` と、それを呼ぶ `poll` | `git grep -n "fn release_finished" -- crates/contract/src/backend_cuda.rs` と `git grep -n "fn poll(" -- crates/contract/src/backend_cuda.rs` |
| 模擬のロストと門 | `crates/contract/src/backend_cuda.rs` の `impl FaultHooks for CudaDevice` の `simulate_device_lost`・`arm_gate`・`open_gate` | `git grep -n "fn simulate_device_lost" -- crates/contract/src/backend_cuda.rs` |
| 試験 | `crates/contract/tests/backend_cuda_lost_release.rs` | 下の「device の要る試験（1/2）」 |

## device の要る試験（1/2）

公開の口（`Device::memory`・`Device::queue`・`Device::fault_hooks`）だけを呼ぶ統合の試験。印は `Copy` なので、ロストの前の印をそのまま `poll` に渡す。本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes` | 3 つ確保して 1 つ目に書き、門を仕込んで 1 つ目から 2 つ目へのコピーを投入し、`simulate_device_lost` の後に 3 つ目・1 つ目・2 つ目を `free` し、`pending_release_count` を読み、ロストの前の印で `poll` を呼び、`open_gate` の後に `poll` を繰り返す | `simulate_device_lost` は `Ok(())`。投入に使っていないバッファの `free` の後の数は `Some(0)`、元の `free` の後は `Some(1)`、先の `free` の後は `Some(2)` で、`poll` は `Lost`、その後も `Some(2)`。計測の起点は門を仕込む前で、`free` の後と 2 回目の数の読みの後の、起点からの経過は 250 ms より短い。`open_gate` の後の `poll` はどの回も `Lost` で、数は起点から 450 ms（門の自動解除の 500 ms より前）までに `Some(0)` になる。投入に使っていないバッファが実際に解放されたことは、公開の口からは観測せず、保留に入らないこと（`Some(0)`）だけを照らす |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_lost_release.rs` |
| `required-features` | `backend-cuda`・`fault-injection`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_lost_release
```

1 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後の解放の保留と回収の device の要る試験を ignore 付きで足す | #4475 |
