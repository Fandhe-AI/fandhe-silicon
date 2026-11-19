# CUDA の背後実装のバッファのコピーとクリアの記録（TASK-624）の所在と本リポの現状

TASK-624 の作業のうち、CUDA の背後実装の `MemoryOps::copy_buffer_to_buffer`・`clear_buffer` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-624。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 記録の時点の照らし | `crates/contract/src/backend_cuda.rs` の `check_copy_record`・`check_clear_record` | 同ファイル |
| 投入の時点の組み立て | 同ファイルの `plan_transfers`・`CudaDevice::launch_transfers`・`CudaDevice::issue_event` | 同ファイル |
| 記録先の列 | 同ファイルの `OpenEncoders` | 同ファイル |
| 解放の保留 | 同ファイルの `PendingReleases`・`hold_or_release`・`CudaDevice::release_finished`・`impl MemoryOps for CudaDevice` の `free` | 同ファイル |
| 試験 | 同ファイルの `copy_clear_tests` | 下の「確かめ方」 |

## 記録と投入と解放の保留（1/3）

### 記録の時点

`copy_buffer_to_buffer`・`clear_buffer` の記録の時点の照らす順と返す値は次のとおり。どれも記録先を変えない。

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | この device で開いていない記録先 | `InvalidArgument` |
| 2 | 別の device のバッファ（元 → 先） | `DeviceMismatch` |
| 3 | ハンドルの世代が device の今の世代と違う | `InvalidArgument` |
| 4 | 表に無い・払い出しの世代がハンドルの世代と違う | `InvalidArgument` |
| 5 | ハンドルの大きさが表のバッファの長さと違う | `InvalidArgument` |
| 6 | 用途が足りない（コピーは元が `DOWNLOAD`・先が `UPLOAD`、クリアは `UPLOAD`） | `InvalidArgument`（操作の名前を前置） |
| 7 | 範囲がバッファの外・あふれる | `InvalidArgument` |
| 8 | offset と長さが 4 の倍数でない | `InvalidArgument` |
| 9 | コピーの元と先が同じバッファ | `InvalidArgument` |
| - | 上のどれにも当たらず長さが 0 | 何も記録せず `Ok(())` |
| - | 上のどれにも当たらない | 記録を足して `Ok(())` |

- 2・6・7 は、各行を元と先の両方に当ててから次の行へ進む
- 3・4・5 は、元の 3 行（世代・表・大きさ）を順に照らしてから、先の同じ 3 行を照らす

### 投入の時点と解放の保留の部品

device に触らない部品の振る舞いは次のとおり。

| 部品 | 振る舞い |
| ---- | -------- |
| `plan_transfers` | 記録の順を保ったまま、バッファの範囲を表の長さで照らして (始点, 終点) に直す |
| `plan_transfers` | 解放済み・世代違いのバッファは、範囲より先に `InvalidArgument` で拒む |
| `hold_or_release` | 最後に使った投入が終わっていない間だけ保留にし、無い・終わっているときはそのまま返す |
| `PendingReleases` | 終わったものだけを取り出し、残りは入れた順のまま残す |
| `OpenEncoders` | 開いている記録先にだけ記録を足し、取り出すと記録の順に返す |

device に触らない試験 `copy_clear_tests` の 17 本が、照らしと組み立てと保留の部品を確かめる。

| 試験 |
| ---- |
| `copy_passes_and_records_the_operands` |
| `clear_passes_and_records_the_operand` |
| `copy_rejects_each_row` |
| `clear_rejects_each_row` |
| `copy_reports_the_earlier_row_when_two_adjacent_rows_apply` |
| `clear_reports_the_earlier_row_when_two_adjacent_rows_apply` |
| `u64_overflow_is_rejected` |
| `zero_length_records_nothing_unless_an_earlier_row_applies` |
| `usage_rejections_name_the_operation_and_the_missing_usage` |
| `plan_keeps_the_record_order_and_converts_the_ranges` |
| `plan_rejects_a_freed_or_stale_buffer_before_any_range` |
| `jobs_name_the_buffers_they_touch` |
| `open_encoders_record_only_to_open_ids_and_claim_returns_the_records_in_order` |
| `pair_mut_borrows_two_distinct_entries` |
| `hold_or_release_holds_only_while_the_last_submission_is_unfinished` |
| `pending_releases_take_finished_keeps_the_unfinished_in_order` |
| `a_sequence_stays_unfinished_until_reclaim_removes_it` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib copy_clear_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | バッファのコピーとクリアを記録して投入で積み、使用中のバッファの解放を保留する | #5250 |
