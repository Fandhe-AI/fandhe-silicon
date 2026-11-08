# CUDA の背後実装の投入（TASK-604）の所在と本リポの現状

TASK-604 の作業のうち、CUDA の背後実装の `CommandQueue` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-604。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 口 | `CudaDevice` の `CommandQueue` の `create_encoder`・`submit` | `grep -n 'fn submit\|fn create_encoder' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |

## 記録先の集合と submit（1/5）

### create_encoder

- 記録先の識別子を払い出し、device の開いている集合に入れる。何も投入しない
- 投入せずに破棄した記録先は、集合から外れる
- 識別子が尽きたときは id 0 の記録先を返し、集合には入れない

### submit

検査の順と返す値:

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 投入済みの表の回収 | 返す値なし |
| 2 | 空でない `wait` | `Unsupported { op: "submit(wait)", backend: Cuda }` |
| 3 | この device で開いていない記録先（別 device の記録先・投入済みの記録先・id 0） | `InvalidArgument` |
| 4 | 通し番号が尽きた | `BackendFailure { op: "submit" }` |
| 5 | event の記録の失敗 | `BackendFailure { op: "record_event" }` |
| - | 上のどれにも当たらない | 印 |

- 拒んだ・失敗した呼び出しは通し番号を消費しない
- 通し番号は device ごとに 1 から払い出す。印の世代は device の世代
- `submit` は完了を待たずに返す

### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `the_first_sequence_is_one_and_a_failed_issue_does_not_consume_a_number` | 最初の番号は 1。失敗した払い出しは番号を消費しない |
| `an_exhausted_sequence_does_not_call_make` | 番号が尽きると記録を呼ばずに `BackendFailure` |
| `reclaim_stops_at_a_pending_head_and_moves_failures_aside` | 未完了の先頭で回収が止まり、失敗は別の集合へ移る |
| `claim_rejects_unknown_zero_and_duplicate_ids_without_changing_the_set` | 開いていない・0・重複は `InvalidArgument` で集合を変えない |
| `close_removes_an_open_id_and_ignores_unknown_ones` | 破棄で集合から外れ、無い id は何もしない |
| `a_nonempty_wait_is_unsupported` | 空の `wait` は通り、空でなければ `Unsupported` |
| `state_ex_distinguishes_pending_complete_failed_and_unissued` | 未完了・完了・失敗・払い出していない番号を区別する |
| `take_ready_keeps_pending_callbacks_and_releases_terminal_ones` | 終端の callback だけを外へ出し、未完了は残す |

### 完了の問合せ

- `poll`・`poll_ex` は投入済みの表を回収して答える。別 device・世代違いの印は `Lost`
- 失敗した列は `poll` で `Lost`、`poll_ex` で `RuntimeError`。払い出していない番号（0 を含む）は `poll` で `Lost`、`poll_ex` で `Unknown`
- `on_complete` は終端ならその場で、未完了なら登録して `submit`・`poll`・`poll_ex`・`on_complete` の中で終端と分かった時に、錠の外で 1 回呼ぶ

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | 記録先の集合と submit | #5184 |
