# CUDA の背後実装のバッファの再利用の一覧とゼロ化（TASK-340）の所在と本リポの現状

TASK-340 の作業のうち、CUDA の背後実装の `MemoryOps::allocate`・`free` が使う再利用の一覧とゼロ化の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-340。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 部品 | `ReusePool`・`retire_flow`・`BufferEntry`・`CudaDevice::settle_pool`・定数 `POOL_MAX_ENTRIES`・`POOL_MAX_BYTES` | `grep -n 'ReusePool\|retire_flow\|settle_pool' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |
| device の取得・確保 | [cuda-device-request.md](./cuda-device-request.md)（TASK-160） | 同頁 |

## 再利用の一覧とゼロ化（1/4）

`allocate` と `free` の動き:

| 操作 | 順 | 呼ぶもの | 失敗のときの扱い |
| ---- | -- | -------- | ---------------- |
| `free` | 1 | 表から項目を外す（無ければ何もしない） | なし |
| `free` | 2 | 一覧に入れられるか確かめる（`would_accept`） | 入れられなければゼロ化を積まずに解放する |
| `free` | 3 | stream にゼロ化（`memset_zeros`）と完了の印（`record_event`）を積む。host は待たない | 積めなければ一覧に入れずに解放する |
| `free` | 4 | 待つ側に入れる（`retire`） | 入れられなければ解放する。panic は外へ出さない |
| `allocate` | 1 | 今までの検査（長さ 0・上限・`usize`） | 今までどおり |
| `allocate` | 2 | 待つ側の完了を問い合わせる（`settle_pool`） | 問い合わせが失敗したものは一覧から外して解放する |
| `allocate` | 3 | id を払い出し、長さと確保のときの用途のビットが一致するものを使える側から取り出す（`take`） | id が尽きたら `AllocationFailed`（一覧は減らさない） |
| `allocate` | 4 | 取れなければ `alloc_zeros` と `synchronize` で新規に確保する | 今までどおり |

上限は件数 16・大きさの合計 64 MiB で、待つ側と使える側の合計。件数と合計は `checked_add` で数え、あふれは入れない。
再利用の回数と、取り出した slot が直前に入れた slot と一致した回数を数える。

device に触らない試験:

| 試験 | 確かめること |
| ---- | ------------ |
| `pool_takes_only_settled_buffers_with_the_same_key` | 完了を確かめたものだけが、長さと用途のビットが一致する鍵で取り出される。回数の数え方 |
| `pool_drops_buffers_whose_zeroing_cannot_be_confirmed` | 完了を確かめられないものが一覧から外れて返り、件数と合計から引かれる。未完了のものは残る |
| `pool_counts_zeroing_entries_toward_the_limits` | 待つ側の分も件数と合計に数え、上限を超える入れ方は引数を返す |
| `retire_flow_skips_zeroing_when_rejected_and_keeps_failed_buffers_out` | 入れられないときはゼロ化を積まず、ゼロ化の失敗では一覧に入れず、バッファを返す |

## 境界の入力の試験（2/4）

device に触らない試験で、同じファイルの `tests` module にある。

| 試験 | 確かめること |
| ---- | ------------ |
| `pool_entry_limit_is_exact_on_the_zeroing_side_the_ready_side_and_both` | 件数 16 が待つ側だけ・使える側だけ・混在のどれでもちょうどで効き、17 件目は引数を返して一覧が変わらない。1 件外れる・取り出すと 1 件入る |
| `pool_byte_limit_is_exact_and_an_overflowing_sum_leaves_the_pool_unchanged` | 合計がちょうど上限まで入り、1 バイト多いと拒む。長さ `u64::MAX` と件数の `usize` のあふれでも一覧が変わらない |
| `pool_take_matches_only_when_both_length_and_usage_bits_are_equal` | 長さだけ・用途のビットだけ・両方違う鍵は `None` で何も変わらず、両方同じ鍵だけ取れる |
| `pool_settle_keeps_pending_moves_them_later_and_subtracts_only_failed_entries` | 未完了は残り、完了で使える側へ移り、失敗だけが件数と合計から引かれる。使える側のものは問い合わせず、空の一覧では問い合わせない |
| `pool_counts_a_slot_match_only_for_the_most_recently_put_slot` | 後に入れたものから取り、一致は直前に入れた slot のときだけ数える。拒まれた入れ方は直前の slot を動かさない |
| `pool_take_on_an_empty_pool_or_another_key_returns_none_without_counting` | 空の一覧・別の鍵・待つ側にしか無い鍵の取り出しは `None` で、何も数えない。最後の 1 件を取ると鍵が消える |
| `retire_flow_calls_zero_and_put_only_when_allowed_and_returns_what_put_returns` | 呼ぶ順と引数。受け入れないと `zero`・`put` を呼ばず、`zero` の失敗では `put` を呼ばない。本物の一覧と組んだ満杯の場合 |

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 再利用の一覧とゼロ化 | #4429 |
| 2/4 | 境界の入力の試験 | #4430 |
