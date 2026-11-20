# 時刻の分解能の測定（TASK-2950）の所在と本リポの現状

TASK-2950 の作業のうち、CPU の時計の分解能を測る道具（段ごとに追加）の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2950。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| crate | `measure/crates/clock-resolution`（package `measure-clock-resolution`。測定の別 workspace `measure/` の member） | `grep -n 'crates/clock-resolution' measure/Cargo.toml` |
| 純粋な関数 | `src/lib.rs` の `adjacent_differences`・`min_nonzero`・`histogram`・`distinct_differences`・`min_distinct_difference`・`seconds_span_to_ns`・`cpu_resolution` | `grep -n '^pub fn' measure/crates/clock-resolution/src/lib.rs` |
| bin | `clock-resolution`（`src/main.rs`） | `grep -n 'name = "clock-resolution"' measure/crates/clock-resolution/Cargo.toml` |
| 試験 | `measure/crates/clock-resolution/tests/resolution.rs` | 下の「CPU の時計の分解能（1/4）」 |

## CPU の時計の分解能（1/4）

bin `clock-resolution` は `Instant::now()` を 1,000,000 回続けて読み、隣り合う値の差のうち 0 でない最小の値と、差の値ごとの回数を、1 行の JSON で標準出力に書く。ファイルは書かない。

| 項目 | 値 |
| ---- | -- |
| 引数 | `--run-id <ID>`（必須）・`--session-ref <回の ID>`・`--repo-commit <commit>`。未知の引数・重複・値の欠け・形に合わない値は、標準エラーに 1 行を出して終了コード 2 |
| 出力の欄（この順） | `task_id`（`TASK-2950`）・`run_id`・`session_ref`（`--session-ref` が無ければ `_none`）・`started_utc`・`ended_utc`・`repo_commit`（無ければ `null`）・`clock`（`cpu-instant`）・`readings`（1000000）・`resolution_ns`・`differences` |
| `differences` | `[差の ns, 回数]` の組を、差の昇順に並べた列。差が 0 の組も含む |
| 失敗 | 0 でない差が無い・時刻が戻った・値が `u64` の ns に収まらないときは、値を出さず標準エラーに 1 行を出して終了コード 1 |

| 試験 | 照らす値 |
| ---- | -------- |
| `adjacent_differences_are_the_gaps_between_readings` | `[0, 0, 41, 83, 83]` の差は `[0, 41, 42, 0]`。長さ 0・1 は `Empty`、値が前より小さいと `InvalidTime` |
| `min_nonzero_skips_zero_and_rejects_all_zero` | `[0, 42, 41, 0, 83]` は 41。全部 0 は `NoNonzeroDifference`、空は `Empty` |
| `histogram_counts_each_value_in_ascending_order` | `[41, 0, 41, 42, 0, 0]` は `(0, 3)`・`(41, 2)`・`(42, 1)` |
| `distinct_differences_use_sorted_distinct_values` | `[125, 42, 125, 0, 83]` の異なる値の差は `[42, 41, 42]` で、最小は 41。異なる値が 1 つなら `NoDistinctPair`、空は `Empty` |
| `seconds_span_rounds_half_away_from_zero` | `(1.0, 1.0)` は 0、`(2.0, 2.0 + 0.5e-9)` は 1、差がちょうど 2.5 ns の `(2.5e-9, 5e-9)` は 3、`(1.0, 1.000001)` は 1000、`(1.0, 3.0)` は 2000000000。始まりが 0・終わりが始まりより前・NaN・無限大は `InvalidTime`、`(1.0, 1.0e11)` は `Overflow` |
| `cpu_resolution_reports_min_nonzero_and_distribution` | `[0, 0, 41, 83, 83, 125]` は読んだ数 6・最小 41・分布 `(0, 2)`・`(41, 1)`・`(42, 2)`。差が全部 0 なら `NoNonzeroDifference` |
| `reading_the_cpu_clock_is_monotonic_and_counted` | 1000 回読むと長さ 1000・先頭 0・減らない。1 回は `Empty`。`CPU_READINGS` は 1000000 |
| `parse_args_requires_a_run_id_and_rejects_unknown_or_duplicate` | `--run-id` の欠け・未知の引数・重複・値の欠け・形に合わない ID を、それぞれの `Usage` の文言で拒む |
| `summary_json_has_fixed_fields_and_order` | 欄の順と字句が上の表のとおり |
| `run_writes_one_line_with_the_full_reading_count` | 1 行だけを書き、末尾は改行、`"readings":1000000`、`"session_ref":"_none"` |

## 確かめ方

```bash
cargo test --manifest-path measure/Cargo.toml -p measure-clock-resolution
make measure-verify
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | CPU の時計の分解能を測る bin と純粋な関数を足す | #7348 |
