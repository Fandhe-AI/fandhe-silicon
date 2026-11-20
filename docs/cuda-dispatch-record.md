# CUDA の背後実装の dispatch の記録と投入（TASK-805）の所在と本リポの現状

TASK-805 の作業のうち、CUDA の背後実装の `CommandQueue::dispatch_ex`（簡易版の `dispatch` を含む）が記録の時点に照らして記録先に積む形と、投入（`submit`・`submit_many`）が dispatch の記録を扱う形の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-805。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/contract/src/backend_cuda.rs` の `impl CommandQueue for CudaDevice` の `dispatch_ex`。`dispatch` は上書きしない | `git grep -n "fn dispatch_ex(\|fn dispatch(" -- crates/contract/src/backend_cuda.rs` |
| 記録の照らし | 同じファイルの `check_dispatch_record`。記録の型は `DispatchRecord` と `RecordedArg` | `git grep -n "fn check_dispatch_record\|struct DispatchRecord\|enum RecordedArg" -- crates/contract/src/backend_cuda.rs` |
| 投入の照らし | 同じファイルの `plan_transfers` と `check_dispatch_gate`。門は `DISPATCH_EXECUTION_ENABLED` | `git grep -n "fn plan_transfers\|fn check_dispatch_gate\|const DISPATCH_EXECUTION_ENABLED" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `dispatch_record_tests` module。device に触らない | 下の「確かめ方」 |

## 記録の時点の照らしと投入の門（1/4）

`dispatch_ex` は、slot・定数・共有メモリを `check_dispatch_record` で照らし、通れば記録を記録先に積む。

`check_dispatch_record` が返す値と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| 束縛と定数が配置の slot をちょうど 1 回ずつ持つ | 束縛と定数を slot の昇順に並べた記録。定数の bytes は写し、共有メモリは各 `bytes` の合計で、`index` は使わない | `record_orders_buffers_and_constants_by_slot_and_sums_shared_memory` |
| `offset` が `u64::MAX` | 長さと比べずにそのまま記録する | `record_keeps_the_offset_without_an_upper_check` |
| 空の定数 | `dispatch_ex: constant slot <slot> is empty` | `record_rejects_an_empty_constant_with_its_slot` |
| 同じ slot が 2 回 | `dispatch_ex: duplicate slot in dispatch: <slot>` | `record_rejects_duplicate_unknown_and_missing_slots_in_this_order` |
| 配置に無い slot | `dispatch_ex: slot <slot> is not in the binding layout` | 同上 |
| 配置にあって渡されない slot | `dispatch_ex: slot <slot> of the binding layout is not provided` | 同上 |
| 共有メモリの合計が `u32` をあふれる | `dispatch_ex: threadgroup memory total overflows` | `record_rejects_a_shared_memory_total_that_overflows` |

slot は束縛・定数の順に 1 つずつ、重なり・配置に無いの順で見て、その後で配置にあって渡されない slot を見る。その後で空の定数を見て、最後に共有メモリの合計を見る。

投入は、記録したバッファの解放済みの確かめ（`plan_transfers`）を全記録について先に行う。dispatch の束縛の解放済みは `a bound buffer was freed before submit (binding slot <slot>)`、コピー・クリアは `a copied or cleared buffer was freed before submit` で、どちらも `InvalidArgument`。解放済みの確かめの後、コピー・クリアの記録のバッファの範囲を表の長さで照らし（`plan_transfers` の中。dispatch の束縛は生存と世代だけを確かめ、範囲は照らさない）、範囲の外なら `InvalidArgument` で返す。この範囲の照らしは `check_dispatch_gate` に先行するので、dispatch を含む列でもコピー・クリアに範囲の外の記録があれば門より先に `InvalidArgument` が返る。その後で `check_dispatch_gate` が、門が閉じていれば dispatch の件を含む列を `Unsupported { op: "submit", backend: Cuda }` で拒む。コピー・クリアだけの列は門に関わらず通す。

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| 束縛のバッファが表に無い・世代が違う | slot を載せた `InvalidArgument`。前の記録の範囲が外れていても先に返す | `plan_rejects_a_freed_bound_buffer_with_its_slot` |
| クリアと dispatch の列 | 記録の順の件。閉じた門は `Unsupported`、開いた門は通す。クリアだけの列は閉じた門でも通す | `gate_rejects_lists_with_a_dispatch_only_when_closed` |
| 投入の経路が参照する門の定数 | どのビルドでも閉じたまま | `dispatch_execution_gate_const_stays_closed` |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::record_
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::plan_
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::gate_
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::dispatch_execution_gate_const_stays_closed
```

1 つ目は 5 件、2 つ目は 1 件、3 つ目と 4 つ目は 1 件ずつが通る。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | dispatch の記録の時点の照らしと投入の門を実装する | #5727 |
