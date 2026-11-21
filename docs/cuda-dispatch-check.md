# CUDA の背後実装の dispatch の記録時の照らし（TASK-806）の所在と本リポの現状

TASK-806 の作業のうち、CUDA の背後実装の `CommandQueue::dispatch_ex`（簡易版の `dispatch` を含む）が記録の時点に `dispatch_dims` を照らす形と、記録の時点の拒否を確かめる試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-806。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/contract/src/backend_cuda.rs` の `impl CommandQueue for CudaDevice` の `dispatch_ex`。`dispatch` は上書きしない | `git grep -n -F -e "fn dispatch_ex(" -e "fn dispatch(" -- crates/contract/src/backend_cuda.rs` |
| `dispatch_dims` の照らし | 同じファイルの `check_dispatch_dims`。`dispatch_ex` は上限に `limits().max_dispatch_dims` と同じ値を渡す | `git grep -n -F -e "fn check_dispatch_dims(" -e "check_dispatch_dims(dispatch_dims, self.limits.max_dispatch_dims)?;" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `dispatch_check_tests` module。device に触らない | 下の「確かめ方」 |

## dispatch_dims の 0 と上限の照らし（1/3）

`check_dispatch_dims` は `dispatch_dims` と軸ごとの上限を受け、最初に当たったものを `InvalidArgument` で返す。

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| 各軸が上限ちょうどまで | 通る | `dispatch_dims_pass_up_to_the_limit_on_each_axis` |
| どれかの軸が 0 | `dispatch_dims contains zero`。上限を超える軸があっても、この文を返す | `dispatch_dims_reject_a_zero_on_any_axis_before_the_limit` |
| どれかの軸が上限を超える | `dispatch_dims exceeds the limit` | `dispatch_dims_reject_one_more_than_the_limit_on_each_axis` |

試験が渡す上限は `[2147483647, 65535, 65535]`・`[u32::MAX; 3]`・`[4, 4, 4]`。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_check_tests
```

3 件が通る。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | dispatch の dispatch_dims の 0 と上限の照らしを実装する | #5735 |
