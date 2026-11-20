# 時刻の分解能の測定（TASK-2950）の所在と本リポの現状

TASK-2950 の作業のうち、時刻の分解能を測る道具の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2950。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| crate | `measure/crates/clock-resolution`（package `measure-clock-resolution`。測定の別 workspace `measure/` の member） | `grep -n 'crates/clock-resolution' measure/Cargo.toml` |
| 純粋な関数 | `src/lib.rs` の `adjacent_differences`・`min_nonzero`・`histogram`・`runs`・`distinct_differences`・`min_distinct_difference`・`seconds_span_to_ns`・`elapsed_ns`・`cpu_resolution`・`args_from_os`・`check_time_order`・`write_summary` | `grep -n '^pub fn' measure/crates/clock-resolution/src/lib.rs` |
| bin | `clock-resolution`（`src/main.rs`） | `grep -n 'name = "clock-resolution"' measure/crates/clock-resolution/Cargo.toml` |
| 試験 | `measure/crates/clock-resolution/tests/resolution.rs`・`src/lib.rs` の単体試験 | 下の「CPU の時計の分解能（1/4）」 |

## CPU の時計の分解能（1/4）

bin `clock-resolution` は `Instant::now()` を 1,000,000 回続けて読み、隣り合う値の差のうち 0 でない最小の値と、差の値ごとの回数を、1 行の JSON で標準出力に書く。ファイルは書かない。

| 項目 | 値 |
| ---- | -- |
| 引数 | `--run-id <ID>`（必須）・`--session-ref <回の ID>`・`--repo-commit <commit>`。UTF-8 でない引数・未知の引数・重複・値の欠け・形に合わない値は、標準出力に書かず、標準エラーに 1 行を出して終了コード 2 |
| 出力の欄（この順） | `task_id`（`TASK-2950`）・`run_id`・`session_ref`（`--session-ref` が無ければ `_none`）・`started_utc`・`ended_utc`・`repo_commit`（無ければ `null`）・`clock`（`cpu-instant`）・`readings`（1000000）・`resolution_ns`・`differences` |
| `differences` | `[差の ns, 回数]` の組を、差の昇順に並べた列。差が 0 の組も含む |
| 失敗 | 0 でない差が無い・CPU の時計の値が直前の値より前・壁時計の終わりが始まりより前（秒未満まで比べる）・値が `u64` の ns に収まらない・列の領域を確保できない（`cannot allocate memory`）ときは、値を出さず標準エラーに 1 行を出して終了コード 1。標準出力に書けない・flush できないときも終了コード 1 |
| 時刻の欄 | `started_utc`・`ended_utc` は壁時計の秒未満を切り捨てた UTC |

| 試験 | 照らす値 |
| ---- | -------- |
| `adjacent_differences_are_the_gaps_between_readings` | `[0, 0, 41, 83, 83]` の差は `[0, 41, 42, 0]`。長さ 0・1 は `Empty`、値が前より小さいと `InvalidTime` |
| `min_nonzero_skips_zero_and_rejects_all_zero` | `[0, 42, 41, 0, 83]` は 41。全部 0 は `NoNonzeroDifference`、空は `Empty` |
| `histogram_counts_each_value_in_ascending_order` | `[41, 0, 41, 42, 0, 0]` をその場で `[0, 0, 0, 41, 41, 42]` に並べ、`(0, 3)`・`(41, 2)`・`(42, 1)` を返す。空の列の `runs` は何も返さない |
| `distinct_differences_use_sorted_distinct_values` | `[125, 42, 125, 0, 83]` の異なる値の差は `Ok([42, 41, 42])` で、最小は 41。異なる値が 1 つなら差は空の列で、最小は `NoDistinctPair`、空は `Empty` |
| `seconds_span_rounds_half_away_from_zero` | `(1.0, 1.0)` は 0、`(2.0, 2.0 + 0.5e-9)` は 1、差がちょうど 2.5 ns の `(2.5e-9, 5e-9)` は 3、`(1.0, 1.000001)` は 1000、`(1.0, 3.0)` は 2000000000。始まりが 0・終わりが始まりより前・NaN・無限大は `InvalidTime`、`(1.0, 1.0e11)` は `Overflow` |
| `cpu_resolution_reports_min_nonzero_and_distribution` | `[0, 0, 41, 83, 83, 125]` は読んだ数 6・最小 41・並べた差 `[0, 0, 41, 42, 42]`・分布 `(0, 2)`・`(41, 1)`・`(42, 2)`。差が全部 0 なら `NoNonzeroDifference` |
| `reading_the_cpu_clock_is_monotonic_and_counted` | 1000 回読むと長さ 1000・先頭 0・減らない。1 回は `Empty`。`CPU_READINGS` は 1000000 |
| `parse_args_requires_a_run_id_and_rejects_unknown_or_duplicate` | `--run-id` の欠け・未知の引数・重複・値の欠け・形に合わない ID を、それぞれの `Usage` の文言で拒む |
| `summary_json_has_fixed_fields_and_order` | 欄の順と字句が上の表のとおり |
| `write_summary_writes_one_fixed_line_and_flushes` | 壁時計 `1795132800.6` 秒・`1795132801.2` 秒と列 `[0, 41, 41, 83]` で、`started_utc` は `2026-11-20T00:00:00Z`・`ended_utc` は `2026-11-20T00:00:01Z`・`readings` 4・分布 `[[0,1],[41,1],[42,1]]` の 1 行と改行だけを書き、flush を 1 回呼ぶ |
| `write_summary_rejects_a_wall_clock_going_back_within_a_second` | 同じ秒の中で終わりが始まりより前なら `InvalidTime` で、何も書かない。flush に失敗する出力では `Output` |
| `wall_clock_order_is_compared_below_a_second` | `check_time_order` は等しい・後なら `Ok`、秒をまたいで前・同じ秒の中で前なら `InvalidTime` |
| `elapsed_ns_compares_each_reading_with_the_previous_one` | 先頭からの差が 0・0・41 ns の時刻は `[0, 0, 41]`、0・80・60 ns は `InvalidTime`、1 つだけは `Empty` |
| `reading_an_unallocatable_count_is_an_allocation_error` | `read_cpu_clock(usize::MAX)` は panic せずに `Allocation`（文言 `cannot allocate memory`） |
| `distinct_differences_return_a_failure_to_allocate_the_copy`（`src/lib.rs`） | 値の写しの領域を確保できないと、差を作らずに `Allocation`。確保を求めた長さは `[5]` |
| `distinct_differences_return_a_failure_to_allocate_the_differences`（`src/lib.rs`） | 差の列の領域を確保できないと `Allocation`。確保を求めた長さは `[5, 3]` |
| `adjacent_differences_return_a_failure_to_allocate_the_differences`（`src/lib.rs`） | 差の列の領域を確保できないと `Allocation`。確保を求めた長さは `[2]` |
| `json_strings_are_written_with_the_same_bytes_as_push_string`（`src/lib.rs`） | 出力へ直に書く文字列は、引用符・逆斜線・制御文字・非 ASCII を含めて `measure_perf::json::push_string` と同じバイト |
| `the_summary_line_of_fixed_readings_has_fixed_bytes` | 1 万個の固定の読みの 1 行は 431 バイトで、sha256 は `4bbbcf2b2dacedbc34881cc84d283c20bd3800673dd4d25ef79711b803b93680` |
| `an_unallocatable_argument_count_is_an_allocation_error` | 長さ `usize::MAX` を名乗る引数の列は、panic せずに `Allocation` |
| `a_non_utf8_argument_is_a_usage_error_with_exit_code_2` | UTF-8 でない引数は `args_from_os` で `Usage`（`invalid value`）。bin に渡すと終了コード 2、標準出力は空、標準エラーは `error: usage: invalid value` の 1 行（Unix だけ） |

## 確かめ方

```bash
cargo test --manifest-path measure/Cargo.toml -p measure-clock-resolution
make measure-verify
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | CPU の時計の分解能を測る bin と純粋な関数を足す | #7348 |
