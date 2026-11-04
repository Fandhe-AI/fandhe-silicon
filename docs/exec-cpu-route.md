# 下-2 の CPU の経路の所在（TASK-930）

下-2（`crates/exec`）の CPU の経路の所在と、本リポの現状の事実をまとめる。host の形の列の長さの検査と、経路 `Route::Cpu` の `f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add`、その試験を含む。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-930（親の Issue は #872）。

## 本リポの現状の事実

記録時点の commit は `c368abb`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `src/` のファイル | `denormal.rs`・`denormal_tests.rs`・`entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs` | `ls crates/exec/src` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

## host の列の長さと要素数の照合（1/4）

### 所在

- 照合の関数: `crates/exec/src/length.rs` の `check_host_len`（`pub(crate)`。公開の面には無い。長さの値と要素数だけを受ける）
- `f16_fma` からの呼び出し: `grep -n 'check_host_len' crates/exec/src/entry.rs`

### 判定の順

`f16_fma` は host の形のとき、次の順で判定する。

| 段 | 内容 | 違うときの返り値 |
| -- | ---- | ---------------- |
| 1 | `check_lengths`（要素数 0・バイト数のあふれ・上限超過） | `Err(ContractError::InvalidArgument(..))` |
| 2 | `a`・`b`・`c`・`out` の列の長さがどれも `n` と同じ | `Err(ContractError::InvalidArgument(..))` |
| 3 | 能力の問合せと経路の選び方 | 経路の選び方の結果 |

段 1・2 の拒否では `query_ex`・`memory`・`queue` を呼ばず、出力の列に書かない。
device の形（`native-blocking-wait`）は列の長さを照らさない。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `length.rs` の `host_len_matches_only_the_element_count` | 長さと要素数の組 | 同じ値だけ `Ok`、ほかは `InvalidArgument` |
| `length_host.rs` の `mismatched_host_column_is_rejected` | 4 つの列のどれか 1 つを長さ 3・5 にした呼び出し（上書き × 既定の印 × device の答え） | `InvalidArgument`、`query_ex`・`memory`・`queue` の数えが 0、出力が番兵のまま |
| `length_host.rs` の `matching_host_columns_reach_route_choice` | 4 つの列が長さ `n` の呼び出し | `InvalidArgument` でなく、`query_ex` の数えが 1 |
| `length_device.rs` の `device_form_passes_one_before_byte_overflow` | device の形で `n` が `u64::MAX / 2` | `InvalidArgument` でなく、`query_ex` の数えが 1、確保・読み出しの数えが増えず、buffer の中身が変わらない |

既存の `length_limit.rs` は、`f16_fma` の列を長さ `n.min(64)`（最小 1）で作る。
`n` が `u64::MAX / 2` の呼び出しは、`f16_fma` だけ列の長さが `n` と違うので `InvalidArgument` を照らす。

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --lib length
cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_device
```

## CPU の経路の f16 の積和（2/4）

### 所在

- `crates/exec/src/cpu.rs` の `f16_fma`、`crates/exec/src/binary16.rs` の `to_f32`・`from_f32`（どちらも `pub(crate)`。公開の面には無い）
- 入口の本体は `grep -n 'crate::cpu::f16_fma\|Outcome::Ran' crates/exec/src/entry.rs`

### 返り値

| 経路と列の形 | 返り値 | 出力 |
| ------------ | ------ | ---- |
| 経路なし | `Ok(Outcome::Unavailable { report })` | 書かない |
| `Route::Cpu` で host の形 | `Ok(Outcome::Ran { report })` | `out` に書く |
| `Route::Cpu` で device の形 | `Err(ContractError::Unsupported { op: "f16_fma", .. })` | buffer に書かない |
| `Route::Native`・`Route::GpuEmulated` | `Err(ContractError::Unsupported { op: "f16_fma", .. })` | 書かない |

`report` は `Exec::query_route` と同じ値。

### 計算の手順

要素ごとに、f16 を f32 へ正確に広げ、`f32::mul_add` で融合積和をとり、f16 へ最近接偶数で 1 回丸めて `out` に書く。
`memory()`・`queue()` を呼ばない（試験用の device の数えが 0）。

### 変換の確かめ

`binary16.rs` の単体試験が、core の `parity::binary16` の 2 関数と、全 65,536 のビット列と丸めの境界で同じ値を返すことを突き合わせる。

### 試験

| ファイル | 試験 | 今の照らし |
| -------- | ---- | ---------- |
| `binary16.rs` | `widening_matches_core_for_all_bit_patterns` | 全ビット列で core と同じ f32 |
| `binary16.rs` | `rounding_matches_core` | 広げた値・前後のビット列・中点・境界の値で core と同じビット列 |
| `cpu.rs` | `hand_computed_cases` | 手計算の 6 組 |
| `cpu.rs` | `single_rounding_after_f32_fma` | 本体 `0x9EB8`、core の参照 `0x9EB9`、順序の差 1 |
| `entry.rs` | `f16_fma_with_cpu_override_runs_on_host` | `Ran`、`Route::Cpu`、`query_route` と同じ報告、`out` が `0x4000` |
| `entry.rs` | `four_entries_with_cpu_override_are_unsupported_err` | `allow_cpu` だけの上書きで、`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` が `Unsupported` で出力が変わらない |
| `fault_injection_ports.rs` | `forced_op_follows_opt_in` | `f16_fma` と `Route::Cpu` の組は `Ran`・報告が `query_route` と同じ・出力が書き換わる。`f16_fma` のほかの経路の組は、経路が決まれば `Unsupported`、経路なしなら `Unavailable` で出力が変わらない。`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` も同じ |
| `opt_in_default.rs` | `chosen_route_does_not_write_output` | `subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` が `Unsupported` で出力が変わらない。`external_interop_shared_memory`・`hardware_ray_tracing`・`unified_memory_sharing` が上書き 5 通りで `Unsupported` |
| `opt_in_default.rs` | `cpu_route_writes_f16_output` | `Ran`、`Route::Cpu`、出力が書き換わる、`memory`・`queue` の数えが 0 |
| `route_query.rs` | `query_route_matches_entry_report` | `Ran` の腕で `query_route` と同じ報告、`Route::Cpu`、出力が書き換わる |
| `unsupported_fields.rs` | `unsupported_carries_entry_name_and_device_backend` | `f16_fma` は `allow_gpu_emulated` だけの印で `Unsupported` の欄を照らす |
| `unsupported_fields.rs` | `f16_fma_cpu_route_runs` | `allow_cpu` の印の `f16_fma` が `Ran` |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --lib
cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields
```

## 入力の組と CPU の参照との比べの試験（3/4）

### 所在

`crates/exec/tests/cpu_f16_fma.rs`（公開のパスから `Exec::f16_fma` を host の形で呼ぶ外の crate の統合テスト）。

### 経路 `Route::Cpu` を選ばせる組

| 組 | device の答え | `Exec` の作り方 | 上書き |
| -- | ------------- | --------------- | ------ |
| (a) | `Unsupported` | `Exec::new(&dev, CPU_ONLY)`（`allow_cpu: true`・`allow_gpu_emulated: false` の印） | `None` |
| (b) | `Unsupported` | `Exec::deny_by_default(&dev)` | `Some(CPU_ONLY)` |
| 故障の差し込み（feature `fault-injection` のビルドでだけ流れる） | `S2` | `Exec::deny_by_default(&dev).force_unsupported(OpId::F16Fma)` | `Some(CPU_ONLY)` |

呼び出しごとに新しい試験用の device を作る。

### 入力と比べ方

| 項目 | 内容 |
| ---- | ---- |
| 入力 | core の `generate_f16_fma_e1`・`generate_f16_fma_e2`・`generate_f16_fma_s`・`generate_f16_fma_r` と `FALLBACK_LENGTHS` の 13 通り（[parity-fallback-inputs.md](parity-fallback-inputs.md)） |
| 参照 | `reference_f16_fma_slice` |
| 比べ方 | `compare_order_distance_binary16` の `is_match()`。`count_bit_identical_binary16` は件数を出すだけ（[parity-fallback-compare.md](parity-fallback-compare.md)） |

### 試験

| 試験 | 入力 | 組 | 期待 |
| ---- | ---- | -- | ---- |
| `e2_and_s_are_within_order_distance` | `generate_f16_fma_e2`・`generate_f16_fma_s` | (a)・(b) | `Ran`、`Route::Cpu`、`query_route` と同じ報告、参照と順序の差の範囲 |
| `r_is_within_order_distance_over_lengths` | `generate_f16_fma_r` を 13 通りの n で | (a) | 同上 |
| `e1_is_judged_by_order_distance_and_counts_bit_identical` | `generate_f16_fma_e1` を 13 通りの n で | (a) | 判定は順序の差だけ。ビット同一の件数が n 以下 |
| `forced_native_off_runs_cpu` | `generate_f16_fma_s` | 故障の差し込みの組 | 上と同じ照らしに加えて `native_reason` が `ForcedUnsupported` |

### 数えるもの

試験用の device の `query_ex_calls()` が 1、`memory_calls()`・`queue_calls()` が 0（`memory()`・`queue()` を呼ばない）。

### E1 の件数の出し方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_f16_fma -- --nocapture
```

`e1 n=… bit_identical=…` の行が 13 行出る。照らすのは件数が `n` 以下であることだけ。`make verify` のログには出ない。

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_f16_fma
cargo test -p fandhe-silicon-exec --locked --test cpu_f16_fma
```

## 完了の条件の確かめ（4/4）

4/4（#6050）で、下-2 の CPU の経路の `f16_fma` について本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-930（ポインタのみ）。`uname -sm` は `Linux x86_64`。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | `f16_fma` の照合の順 | `grep -n 'check_lengths\|check_host_len\|route_report\|crate::cpu::f16_fma' crates/exec/src/entry.rs` | `f16_fma` の中は 255 行 `check_lengths`、259 行 `check_host_len`、265 行 `route_report`、269 行 `crate::cpu::f16_fma`。全部の行は下の (1) の出力 |
| (2) | 照合と変換と本体の単体試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --lib` | `test result: ok. 86 passed; 0 failed; 0 ignored` |
| (3) | 列の長さの照合の入口経由の試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_device` | `tests/length_device.rs` が 2 passed、`tests/length_host.rs` が 2 passed、`tests/length_limit.rs` が 4 passed。3 本とも 0 failed; 0 ignored |
| (4) | 入力の組と参照との比べ | `cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_f16_fma`、`cargo test -p fandhe-silicon-exec --locked --test cpu_f16_fma` | `--all-features` は 4 passed; 0 failed; 0 ignored、既定の feature は 3 passed; 0 failed; 0 ignored |
| (5) | `generate_f16_fma_e1` のビット同一の件数 | `cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_f16_fma -- --nocapture` の出力の `^e1 n=` の行 | 13 行。下の (5) の出力 |
| (6) | 直した既存の試験 | `gh pr diff 7972 --name-only` と `gh pr diff 7974 --name-only` の `crates/exec/tests/` の行、`cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields` | #7972 は `length_device.rs`・`length_host.rs`・`length_limit.rs`。#7974 は `fault_injection_ports.rs`・`opt_in_default.rs`・`route_query.rs`・`unsupported_fields.rs`。件数は `fault_injection_ports` が 8 passed、`opt_in_default` が 6 passed、`route_query` が 6 passed、`unsupported_fields` が 2 passed で、すべて 0 failed; 0 ignored |
| (7) | 既存の試験とビルドの組 | `make verify`、`make build-profiles`、`make wasm-verify` | 3 つとも終了コード 0。`make verify` の exec は lib が 86 passed、`tests/cpu_f16_fma.rs` が 4 passed、`tests/device_operands.rs` が 1 passed、`tests/fault_injection_ports.rs` が 8 passed、`tests/length_device.rs` が 2 passed、`tests/length_host.rs` が 2 passed、`tests/length_limit.rs` が 4 passed、`tests/length_zero.rs` が 5 passed、`tests/opt_in_default.rs` が 6 passed、`tests/reasons_and_denormal.rs` が 4 passed、`tests/route_evidence_calls.rs` が 4 passed、`tests/route_query.rs` が 6 passed、`tests/unsupported_fields.rs` が 2 passed で、すべて 0 failed。`make build-profiles` の exec は feature なし・`native-blocking-wait`・`fault-injection` の 3 組で `clippy`・`test`・wasm32 の `cargo check` が通る。`make wasm-verify` は `cargo check (fandhe-silicon-exec tests, wasm32-unknown-unknown, …)` を default features・`native-blocking-wait`・`fault-injection` の 3 通りで通る |
| (8) | 公開の面が変わらない | `make exec-surface-check` | 終了コード 0。下の (8) の出力 |
| (9) | 依存と unsafe を足していない | `gh pr diff <n> --name-only`（n は 7972・7974・7975）を `grep -E '^(Cargo\.toml\|Cargo\.lock\|deps-approved\.toml\|deny\.toml\|crates/core/\|crates/upper/\|crates/exec/Cargo\.toml)'` で絞る、`grep -n forbid crates/exec/src/lib.rs` | 3 つとも grep の出力は空。`crates/exec/src/lib.rs` の 20 行に `#![forbid(unsafe_code)]` |
| (10) | CI で流れる場所 | main の push の CI の run 41793124830（`conclusion=success`、`headSha=15e86ae529cf11b9d041cbb1a036822bd56026dd`）の job `make verify (fmt + clippy + test)`（job id 126450871726）の `make verify` のステップのログ | 下の (10) のログの行。`cpu_f16_fma` と `length_host` が出る |

(1) の出力。

```text
15:use crate::length::{check_host_len, check_lengths};
150:    fn route_report(
186:    /// `route_report` に入口の関数名を渡し、試しの実行の手段が無いときの `Unsupported` に使わせる。
193:        let report = self.route_report(op, opt_in, name)?;
214:        self.route_report(op, opt_in, "query_route")
255:        check_lengths(OpId::F16Fma, n, None, self.device.limits().max_buffer_bytes)?;
259:                    check_host_len(len, n)?;
265:        let report = self.route_report(OpId::F16Fma, opt_in, "f16_fma")?;
269:                crate::cpu::f16_fma(a, b, c, out);
285:        check_lengths(
304:        check_lengths(
324:        check_lengths(
616:    fn query_route_reports_fallback_route() {
```

(5) の出力。

```text
e1 n=1 bit_identical=1
e1 n=31 bit_identical=31
e1 n=32 bit_identical=32
e1 n=33 bit_identical=33
e1 n=255 bit_identical=255
e1 n=256 bit_identical=256
e1 n=257 bit_identical=257
e1 n=1000 bit_identical=1000
e1 n=65539 bit_identical=65539
e1 n=4096 bit_identical=4096
e1 n=65536 bit_identical=65536
e1 n=262144 bit_identical=262144
e1 n=4194304 bit_identical=4194304
```

(8) の出力。

```text
record: commit=15e86ae529cf11b9d041cbb1a036822bd56026dd rs_files=26 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

(10) のログ（ジョブ名・ステップ名・時刻を除いて `Running` の行と `test result:` の行をログの出た順に並べた。`cpu_f16_fma` の直後の 1 行目 86 passed は lib の結果の行で、2 行目の 4 passed が `cpu_f16_fma` の結果）。

```text
Running tests/cpu_f16_fma.rs (target/debug/deps/cpu_f16_fma-94c5986b155befb1)
test result: ok. 86 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.56s
Running tests/length_host.rs (target/debug/deps/length_host-d17c7b4c6c32ccc1)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

確かめた時点は、main のコミット `15e86ae` と、`docs/spec` の gitlink `8e2bc9c0d89ce5c106b6d722cfd630cb393ebf45`。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-930）。

### 確かめ方

```text
grep -n 'check_lengths\|check_host_len\|route_report\|crate::cpu::f16_fma' crates/exec/src/entry.rs
cargo test -p fandhe-silicon-exec --all-features --locked --lib
cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_device
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_f16_fma
cargo test -p fandhe-silicon-exec --locked --test cpu_f16_fma
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_f16_fma -- --nocapture
cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields
make verify
make build-profiles
make wasm-verify
make exec-surface-check
grep -n forbid crates/exec/src/lib.rs
```

## 段の表

| 段 | Issue・PR | 節 |
| -- | --------- | -- |
| 1/4 | #6047（PR #7972） | host の列の長さと要素数の照合（1/4） |
| 2/4 | #6048（PR #7974） | CPU の経路の f16 の積和（2/4） |
| 3/4 | #6049（PR #7975） | 入力の組と CPU の参照との比べの試験（3/4） |
| 4/4 | #6050 | 完了の条件の確かめ（4/4） |

親 Issue は #872。

## 今の値の補い（base dbbb5c0）

記録時点の表と既存の節は書き換えない。以下は base `dbbb5c0` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。

### `src/` のファイル

「本リポの現状の事実」の `src/` の一覧（11 ファイル）は、今は 13 ファイル。`binary16.rs`・`cpu.rs`・`denormal.rs`・`denormal_tests.rs`・`entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs`（`ls crates/exec/src`）。`binary16.rs` と `cpu.rs` が 2/4 で加わった。

### device の形で経路が `Route::Cpu` に決まる `f16_fma`

`crates/exec/tests/device_operands.rs` の `cpu_route_with_device_operands_is_unsupported_and_leaves_buffers`（feature `native-blocking-wait` のビルドでだけ流れる）が、device の形の列を渡した `Exec::f16_fma` を次の 2 つの組で呼ぶ。`CPU_ONLY` は `allow_cpu: true`・`allow_gpu_emulated: false` の印。

| 組 | `Exec` の作り方 | 上書き |
| -- | --------------- | ------ |
| 0 | `Exec::new(&dev, CPU_ONLY)` | `None` |
| 1 | `Exec::deny_by_default(&dev)` | `Some(CPU_ONLY)` |

device は `Scripted::bare()`、`n` は 8、4 つの buffer（16 バイト）の中身は buffer ごとに別の値で埋める。

| 照らす点 | 期待 |
| -------- | ---- |
| 戻り値 | `Err(ContractError::Unsupported { op: "f16_fma", .. })`。`Ran` にならない |
| `query_ex` の数え | 1 回 |
| `memory()`・`queue()` の数え | 呼び出しの前後で変わらない |
| buffer の中身 | 呼ぶ前のまま |

`cargo test -p fandhe-silicon-exec --all-features --locked --test device_operands` は 2 passed; 0 failed; 0 ignored（`unavailable_leaves_device_buffers_unchanged` と上の試験）。

同じ変更で、`cpu_f16_fma.rs` の定数 `C_ONLY` は `CPU_ONLY` になり、経路を略号で書いたコメントは `Route::Cpu`・`Route::Native`・`allow_cpu` の名前になっている。`opt_in_default.rs` の先頭の `//!` は、出力が変わらない呼び出し（`Ok(Outcome::Unavailable { .. })` を返した呼び出しと、`f16_fma` 以外の入口が `Err(ContractError::Unsupported { .. })` を返した呼び出し）と、出力が変わる呼び出し（`f16_fma` が経路 `Route::Cpu` で host の形）に分けて書いている。

### 完了の条件の確かめ（4/4）の値

| 表の行 | 記録の値 → 今の値 | 測ったコマンド |
| ------ | ----------------- | -------------- |
| (1) | 255 行 `check_lengths`・259 行 `check_host_len`・265 行 `route_report`・269 行 `crate::cpu::f16_fma` のまま | `grep -n 'check_lengths\|check_host_len\|route_report\|crate::cpu::f16_fma' crates/exec/src/entry.rs` |
| (2) | 86 passed のまま | `cargo test -p fandhe-silicon-exec --all-features --locked --lib` |
| (3) | `length_device` 2・`length_host` 2・`length_limit` 4 のまま | (3) のコマンド |
| (4) | `--all-features` 4 passed、既定の feature 3 passed のまま | (4) のコマンド |
| (6) | `fault_injection_ports` 8・`opt_in_default` 6・`route_query` 6・`unsupported_fields` 2 のまま | (6) のコマンド |
| (7) の `tests/device_operands.rs` | 1 passed → 2 passed | `make verify` |
| (8) の `record:` | `rs_files=26` のまま（`commit=` の値だけ変わる） | `make exec-surface-check` |
| (9) の `forbid` の行 | `crates/exec/src/lib.rs` の 20 行のまま | `grep -n forbid crates/exec/src/lib.rs` |

### 段の表

| 段 | Issue・PR | 節 |
| -- | --------- | -- |
| 4/4 | #6050（PR #7976） | 完了の条件の確かめ（4/4） |

## subgroup の総和の host の列の長さと出力の数の照合（1/6）（TASK-931）

下-2（`crates/exec`）の `subgroup_sum_u32`・`subgroup_sum_f32` の host の形の列の長さの検査と、その試験の所在をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-931（親の Issue は #873）。

### 本リポの現状の事実

記録時点の commit は `761cdc9`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `src/` のファイル | `binary16.rs`・`cpu.rs`・`denormal.rs`・`denormal_tests.rs`・`entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs` | `ls crates/exec/src` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

### 所在

- 出力の数の関数: `crates/exec/src/length.rs` の `subgroup_output_len`（`pub(crate)`。`n` を 256 で割って切り上げた数）
- 2 つの入口からの呼び出し: `grep -n 'check_host_len\|subgroup_output_len' crates/exec/src/entry.rs crates/exec/src/length.rs`

### 判定の順

`subgroup_sum_u32`・`subgroup_sum_f32` は host の形のとき、次の順で判定する。

| 段 | 内容 | 違うときの返り値 |
| -- | ---- | ---------------- |
| 1 | `check_lengths`（要素数 0・バイト数のあふれ・上限超過） | `Err(ContractError::InvalidArgument(..))` |
| 2 | `x` の列の長さが `n` と同じ | `Err(ContractError::InvalidArgument(..))` |
| 3 | `out` の列の長さが `subgroup_output_len(n)` と同じ | `Err(ContractError::InvalidArgument(..))` |
| 4 | 能力の問合せと経路の選び方 | 経路の選び方の結果 |

段 1〜3 の拒否では `query_ex`・`memory`・`queue` を呼ばず、出力の列に書かない。
device の形（`native-blocking-wait`）は列の長さを照らさない。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `length.rs` の `subgroup_output_len_counts_blocks` | `n` が 1・256・257・1000・65_539・`u64::MAX` | 1・1・2・4・257・`u64::MAX / 256 + 1` |
| `length_host.rs` の `mismatched_subgroup_columns_are_rejected` | `n` = 257 で (`x` の長さ, `out` の長さ) が (256, 2)・(258, 2)・(257, 1)・(257, 3) の呼び出し（2 つの入口 × 上書き × 既定の印 × device の答え） | `InvalidArgument`、`query_ex`・`memory`・`queue` の数えが 0、出力が番兵のまま |
| `length_host.rs` の `matching_subgroup_columns_reach_route_choice` | `n` = 257・`x` の長さ 257・`out` の長さ 2 の呼び出し | `InvalidArgument` でなく、`query_ex` の数えが 1 |
| `length_device.rs` の `device_form_subgroup_passes_one_before_byte_overflow` | device の形で `n` が `u64::MAX / 4` | `InvalidArgument` でなく、`query_ex` の数えが 1、確保・読み出しの数えが増えず、buffer の中身が変わらない |

### 出力の列の長さを直した既存の試験

| ファイル | 直し |
| -------- | ---- |
| `crates/exec/src/entry.rs`・`crates/exec/src/denormal_tests.rs` | subgroup の `out` を出力の数の長さ（`n` = 2 で 1）で渡す |
| `fault_injection_ports.rs`・`reasons_and_denormal.rs`・`unsupported_fields.rs`・`route_evidence_calls.rs`・`opt_in_default.rs`・`route_query.rs` | 同上。番兵のままかの比べも同じ長さで行う |
| `length_zero.rs` | `valid_length_reaches_route_choice` が subgroup の 2 つの入口の出力を長さ 1 で渡す |
| `length_limit.rs` | subgroup の `x` を長さ `n.min(64)`（最小 1）、`out` を `k` を 256 で割って切り上げた長さで作る。一つ手前の `n` は `atomic_add` のほかが `InvalidArgument` |

`atomic_add` の列と `f16_fma` の列は変えていない。

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --lib length
cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_zero --test length_device
cargo test -p fandhe-silicon-exec --locked --test length_host
```

## CPU の経路の subgroup の総和（2/6）（TASK-931）

### 所在

- 計算の本体: `crates/exec/src/cpu.rs` の `subgroup_sum_u32`・`subgroup_sum_f32`（`pub(crate)`）
- 入口の本体: `grep -n 'crate::cpu::subgroup_sum\|Outcome::Ran' crates/exec/src/entry.rs`

### 返り値

| 経路と列の形 | 返り値 | 出力 |
| ------------ | ------ | ---- |
| 経路なし | `Ok(Outcome::Unavailable { report })` | 書かない |
| `Route::Cpu` で host の形 | `Ok(Outcome::Ran { report })` | `out` に書く |
| `Route::Cpu` で device の形 | `Err(ContractError::Unsupported { op: <入口名>, .. })` | buffer に書かない |
| `Route::Native`・`Route::GpuEmulated` | `Err(ContractError::Unsupported { .. })` | 書かない |

`report` は `Exec::query_route` と同じ値。`memory()`・`queue()` を呼ばない。
`subgroup_sum_f32` は、経路の選び方が f32 の非正規化数の判定を求め、差し込みの無い経路に届く呼び出しが
`Err(ContractError::Unsupported { op: "subgroup_sum_f32", .. })` を返す。`fault-injection` の `inject_denormal` で
`Route::Cpu` に差し込んだ組では本体に届く。

### 計算

| 入口 | ブロック | 初期値 | 足し方 |
| ---- | -------- | ------ | ------ |
| `subgroup_sum_u32` | 256 要素ずつ。添字の順。端数のブロックは短いまま | `0u32` | `wrapping_add` |
| `subgroup_sum_f32` | 同上 | `+0.0` | f32 の `+` |

### 試験

| ファイル | 試験 | 照らし |
| -------- | ---- | ------ |
| `cpu.rs` | `subgroup_sum_u32_blocks_and_wraps` | あふれの巻き戻りと 257 要素のブロック分け |
| `cpu.rs` | `subgroup_sum_f32_blocks_in_index_order` | 300 要素のブロック分け、足す順で結果が変わる入力、`-0.0` の列の和が `+0.0` |
| `entry.rs` | `subgroup_sums_with_cpu_override_run_on_host` | u32 は `Ran`・`out` が 2・報告が `query_route` と等しい。f32 は `Unsupported` で `out` が変わらない |
| `opt_in_default.rs` | `cpu_route_writes_subgroup_u32_output` | `Ran`・`out` が 2・`memory()`・`queue()` の呼びが 0 |
| `unsupported_fields.rs` | `subgroup_sum_u32_cpu_route_runs` | `Ran` |

### 直した既存の試験

| ファイル | 直し |
| -------- | ---- |
| `entry.rs` | `four_entries_with_cpu_override_are_unsupported_err` が `atomic_add` だけを照らす |
| `fault_injection_ports.rs` | `forced_op_follows_opt_in` が `Route::Cpu` に差し込んだ subgroup の 2 つの入口を `Ran` として照らす |
| `opt_in_default.rs` | `chosen_route_does_not_write_output` が `subgroup_sum_u32` を書かない側から外す |
| `route_query.rs` | `query_route_matches_entry_report` の `Ran` の腕が `subgroup_sum_u32` を含む |
| `unsupported_fields.rs` | `unsupported_carries_entry_name_and_device_backend` の `subgroup_sum_u32` が `Route::GpuEmulated` に決まる印で呼ぶ |

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --lib
cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields
```

## CPU の経路の f32 の総和の非正規化数の判定（3/6）（TASK-931）

### 所在

`crates/exec/src/entry.rs` の `probe` の `match`（`grep -n 'Route::Cpu' crates/exec/src/entry.rs`）。

### 試しの実行

| 組 | 試しの実行 |
| -- | ---------- |
| (`OpId::SubgroupSumF32`, `Route::Cpu`) | 768 要素の入力を CPU の経路の f32 の総和の関数で計算し、3 つの出力を返す |
| ほかの組 | `Err(ContractError::Unsupported { .. })` |
| runner を差した crate の中の試験 | runner の返し |

### 入口の返り値

能力が非対応の device で `OpId::SubgroupSumF32` を呼んだときの `query_route` と `subgroup_sum_f32` の返り値。

| 上書きの印 | 返り値 |
| ---------- | ------ |
| `allow_cpu: true`・`allow_gpu_emulated: false` の印 | `Ok`（入口は `Ok(Outcome::Ran { .. })`） |
| `allow_gpu_emulated: true`・`allow_cpu: false` の印、`allow_gpu_emulated: true`・`allow_cpu: true` の印 | `Err(ContractError::Unsupported { .. })` |

`allow_cpu: true`・`allow_gpu_emulated: false` の印で呼んだ呼び出しの報告は、`route` が `Some(Route::Cpu)`、`f32_denormal` が `behavior: Preserve`・`basis: Observed`・`documented: None`。判定は `Exec` の値ごとに保持する。

### 試験

| ファイル | 試験 | 照らし |
| -------- | ---- | ------ |
| `entry.rs` | `query_route_reports_fallback_route` | `SubgroupSumF32` の `allow_cpu: true`・`allow_gpu_emulated: false` の印で `Preserve`・`Observed`・`documented` が `None` |
| `entry.rs` | `subgroup_sums_with_cpu_override_run_on_host` | f32 は `Ran`・`out` が 2・報告が `query_route` と等しい |
| `entry.rs` | `cpu_judgment_is_kept_per_exec` | 2 回の `query_route` と入口の報告が等しい |
| `entry.rs` | `uninjected_cpu_route_is_judged_on_host` | 差し込みの無い `Route::Cpu` の経路が `Preserve`・`Observed` |
| `opt_in_default.rs` | `cpu_route_writes_subgroup_f32_output` | `Ran`・`out` が 2・`memory()`・`queue()` の呼びが 0 |
| `unsupported_fields.rs` | `subgroup_sum_f32_cpu_route_runs` | `Ran` |

### 直した既存の試験

| ファイル | 直し |
| -------- | ---- |
| `entry.rs` | `uninjected_route_keeps_unsupported_error` が `atomic_add` だけを照らす |
| `route_query.rs` | `unreachable_native_follows_opt_in_order` が `SubgroupSumF32` の `Route::Cpu` を `Ok` として照らす。`query_route_matches_entry_report` の `Ran` の腕が `SubgroupSumF32` を含む |
| `opt_in_default.rs` | `chosen_route_does_not_write_output` が `subgroup_sum_f32` を書かない側から外す |
| `unsupported_fields.rs` | `unsupported_carries_entry_name_and_device_backend` の `subgroup_sum_f32` が `Route::GpuEmulated` に決まる印で呼ぶ |

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --lib entry
cargo test -p fandhe-silicon-exec --all-features --locked --test route_query --test opt_in_default --test unsupported_fields --test fault_injection_ports
```

## 入力の組と CPU の参照との比べの試験（4/6）（TASK-931）

### 所在

`crates/exec/tests/cpu_subgroup_sum.rs`（公開のパスから `Exec::subgroup_sum_u32`・`Exec::subgroup_sum_f32` を host の形で呼ぶ外の crate の統合テスト）。

### 経路 `Route::Cpu` を選ばせる組

| 組 | device の答え | `Exec` の作り方 | 上書き |
| -- | ------------- | --------------- | ------ |
| (a) | `Unsupported` | `Exec::new(&dev, CPU_ONLY)` | `None` |
| (b) | `Unsupported` | `Exec::deny_by_default(&dev)` | `Some(CPU_ONLY)` |
| 故障の差し込み（feature `fault-injection` のビルドでだけ） | `S2` | `Exec::deny_by_default(&dev).force_unsupported(op)` | `Some(CPU_ONLY)` |

呼び出しごとに新しい試験用の device を作る。

### 入力と比べ方

| 入口 | 入力 | 参照 | 比べ方 |
| ---- | ---- | ---- | ------ |
| `subgroup_sum_u32` | `generate_subgroup_sum_u32_u` | `reference_subgroup_sum_u32` | `compare_bits_u32` |
| `subgroup_sum_f32` | `generate_subgroup_sum_f32_fx` | `reference_subgroup_sum_f32`（`F32SubnormalHandling::Preserve`） | `compare_bits_f32` |
| `subgroup_sum_f32` | `generate_subgroup_sum_f32_fg`・`generate_subgroup_sum_f32_fs` | 同上 | `compare_relative_sum_f32` |

長さは `FALLBACK_LENGTHS` の 13 通り（[parity-fallback-inputs.md](parity-fallback-inputs.md)）。比べの関数は [parity-fallback-compare.md](parity-fallback-compare.md)。

### 試験

| 試験 | 入力 | 組 | 期待 |
| ---- | ---- | -- | ---- |
| `u32_outputs_match_bits_over_lengths` | `generate_subgroup_sum_u32_u` の 13 通り | (a)・(b) | ビット同一 |
| `integer_f32_outputs_match_bits_over_lengths` | `generate_subgroup_sum_f32_fx` の 13 通り | (a)・(b) | ビット同一 |
| `fg_is_within_relative_bound_over_lengths` | `generate_subgroup_sum_f32_fg` の 13 通り | (a) | 相対の条件に合い、余裕の最大が有限 |
| `fs_is_compared_with_preserving_reference_over_lengths` | `generate_subgroup_sum_f32_fs` の 13 通り | (a) | 相対の条件に合う |
| `forced_native_off_runs_cpu`（feature `fault-injection`） | 長さ 1000 の u32 と f32 | 故障の差し込み | 上の照らしに加え `native_reason` が `Some(UnavailableReason::ForcedUnsupported)` |

### 報告と数えるもの

- `route` が `Some(Route::Cpu)`。`query_route` と同じ報告になる。
- u32 は `f32_denormal` が `None`。f32 は `behavior: Preserve`・`basis: Observed`・`documented: None`。
- `query_ex_calls()` が 1、`memory_calls()`・`queue_calls()` が 0。

### 余裕の行の出し方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum -- --nocapture
```

`margin subgroup-sum-f32-fg n=… blocks=… max=…` の行が 13 行出る。照らすのは `is_match()` と最大が有限であることだけ。`make verify` のログには出ない。

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum
cargo test -p fandhe-silicon-exec --locked --test cpu_subgroup_sum
```

## 出力の sha256 の行（5/6）（TASK-931）

### 所在

`crates/exec/tests/cpu_subgroup_sum.rs` の `outputs_print_sha256_per_case`。

### 行の形

`sha256 <入力の名前> n=<n> <16 進>`。入力の名前は `FallbackInputKind::….name()` の値（`subgroup-sum-u32-u`・`subgroup-sum-f32-fx`・`subgroup-sum-f32-fg`・`subgroup-sum-f32-fs`）、16 進は小文字の 64 文字。長さは `FALLBACK_LENGTHS` の 13 通りで、1 回の流しで 52 行出る。

### バイト列の作り方

| 入口 | 入力 | バイト列 |
| ---- | ---- | -------- |
| `subgroup_sum_u32` | `generate_subgroup_sum_u32_u` | 出力の各要素の `to_le_bytes()` を添字の順に並べる |
| `subgroup_sum_f32` | `generate_subgroup_sum_f32_fx`・`generate_subgroup_sum_f32_fg`・`generate_subgroup_sum_f32_fs` | `0.0` に等しい要素は `0u32`、それ以外は `to_bits()` とし、その `to_le_bytes()` を添字の順に並べる（`-0.0` は `+0.0` のビット列になる） |

sha256 は core の `hash::sha256_bytes`。

### 照らすもの

呼ぶたびに新しい試験用の device と `Exec` を作り、同じ入力を 2 回呼ぶ。照らすのは 2 つの sha256 が等しいことと、4/6 と同じ報告・数えの照らし。組は (a)（`Exec::new(&dev, CPU_ONLY)`・device の答えは `Unsupported`）。

### 3 回の流し方

```bash
d=$(mktemp -d)
for i in 1 2 3; do
  cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum outputs_print_sha256_per_case -- --nocapture | grep '^sha256 ' > "$d/run-$i.txt"
done
wc -l "$d"/run-*.txt
sha256sum "$d"/run-*.txt
cat "$d"/run-1.txt "$d"/run-2.txt "$d"/run-3.txt | sort | uniq -c | awk '{print $1}' | sort | uniq -c
rm -r "$d"
```

各回の行を比べる。

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum
cargo test -p fandhe-silicon-exec --locked --test cpu_subgroup_sum
```

## 完了の条件の確かめ（6/6）（TASK-931）

6/6（#6058）で、下-2 の CPU の経路の `subgroup_sum_u32`・`subgroup_sum_f32` について本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-931（ポインタのみ）。`uname -sm` は `Linux x86_64`。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 2 つの入口の照合と本体の順 | `grep -n 'check_lengths\|check_host_len\|subgroup_output_len\|route_report\|crate::cpu::' crates/exec/src/entry.rs` | `subgroup_sum_u32` の中は 300 行 `check_lengths`、308 行と 309 行 `check_host_len`、314 行 `route_report`、318 行 `crate::cpu::subgroup_sum_u32`。`subgroup_sum_f32` の中は 342 行 `check_lengths`、350 行と 351 行 `check_host_len`、356 行 `route_report`、360 行 `crate::cpu::subgroup_sum_f32`。`probe` の中の `crate::cpu::subgroup_sum_f32` は 237 行。全部の行は下の (1) の出力 |
| (2) | 照合と本体と判定の単体試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --lib` | `test result: ok. 92 passed; 0 failed; 0 ignored` |
| (3) | 列の長さの照合の入口経由の試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_zero --test length_device` | `tests/length_device.rs` が 3 passed、`tests/length_host.rs` が 4 passed、`tests/length_limit.rs` が 4 passed、`tests/length_zero.rs` が 5 passed。4 本とも 0 failed; 0 ignored |
| (4) | 入力の組と参照との比べ | `cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum`、`cargo test -p fandhe-silicon-exec --locked --test cpu_subgroup_sum` | `--all-features` は 6 passed; 0 failed; 0 ignored、既定の feature は 5 passed; 0 failed; 0 ignored |
| (5) | 余裕の行 | `cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum fg_is_within -- --nocapture` の出力の `^margin ` の行 | 13 行。下の (5) の出力 |
| (6) | `cargo test` を 3 回流した sha256 の行 | 「出力の sha256 の行（5/6）（TASK-931）」の「3 回の流し方」のコマンド | 各回の `wc -l` は 52、`sha256sum` の値は 3 回とも `f1f8c0cb11672f8013a00ce12d55dcc0fddca5fb03638a6a35550f61c61d6513`、`uniq -c` の集計は `52 3` の 1 行 |
| (7) | 直した既存の試験 | `gh pr diff 7979 --name-only`・`gh pr diff 7980 --name-only`・`gh pr diff 7981 --name-only` の `crates/exec/` の行、`cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields --test reasons_and_denormal --test route_evidence_calls` | #7979 は `src/denormal_tests.rs`・`src/entry.rs`・`src/length.rs` と `tests/` の `fault_injection_ports.rs`・`length_device.rs`・`length_host.rs`・`length_limit.rs`・`length_zero.rs`・`opt_in_default.rs`・`reasons_and_denormal.rs`・`route_evidence_calls.rs`・`route_query.rs`・`unsupported_fields.rs`。#7980 は `src/cpu.rs`・`src/entry.rs`・`src/lib.rs` と `tests/` の `fault_injection_ports.rs`・`opt_in_default.rs`・`route_query.rs`・`unsupported_fields.rs`。#7981 は `src/entry.rs` と `tests/` の `opt_in_default.rs`・`route_query.rs`・`unsupported_fields.rs`。件数は `fault_injection_ports` が 8 passed、`opt_in_default` が 8 passed、`reasons_and_denormal` が 4 passed、`route_evidence_calls` が 4 passed、`route_query` が 6 passed、`unsupported_fields` が 4 passed で、すべて 0 failed; 0 ignored |
| (8) | 既存の試験とビルドの組 | `make verify`、`make build-profiles`、`make wasm-verify` | 3 つとも終了コード 0。`make verify` の exec は lib が 92 passed、`tests/cpu_f16_fma.rs` が 4 passed、`tests/cpu_subgroup_sum.rs` が 6 passed、`tests/device_operands.rs` が 2 passed、`tests/fault_injection_ports.rs` が 8 passed、`tests/length_device.rs` が 3 passed、`tests/length_host.rs` が 4 passed、`tests/length_limit.rs` が 4 passed、`tests/length_zero.rs` が 5 passed、`tests/opt_in_default.rs` が 8 passed、`tests/reasons_and_denormal.rs` が 4 passed、`tests/route_evidence_calls.rs` が 4 passed、`tests/route_query.rs` が 6 passed、`tests/unsupported_fields.rs` が 4 passed で、すべて 0 failed。`make build-profiles` の exec は feature なし・`native-blocking-wait`・`fault-injection` の 3 組で `clippy`・`test`・wasm32 の `cargo check` が通る。`make wasm-verify` は `cargo check (fandhe-silicon-exec tests, wasm32-unknown-unknown, …)` を default features・`native-blocking-wait`・`fault-injection` で通る |
| (9) | 公開の面が変わらない | `make exec-surface-check` | 終了コード 0。`op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0`、`port_names_all=6`。下の (9) の出力 |
| (10) | 依存と unsafe を足していない | `gh pr diff <n> --name-only`（n は 7979・7980・7981・7982・7983）を `grep -E '^(Cargo\.toml\|Cargo\.lock\|deps-approved\.toml\|deny\.toml\|crates/core/\|crates/upper/\|crates/exec/Cargo\.toml)'` で絞る、`grep -n forbid crates/exec/src/lib.rs` | 5 つとも grep の出力は空。`crates/exec/src/lib.rs` の 21 行に `#![forbid(unsafe_code)]` |
| (11) | CI で流れる場所 | main の push の CI の run 41833856707（`conclusion=success`、`headSha=112380bbb00b6932fae9a9c1cc90aec0aa28a5c1`）の job `make verify (fmt + clippy + test)`（job id 126587613184）のログ | 下の (11) のログの行。`cpu_subgroup_sum` と `length_host` が出る |

(1) の出力。

```text
16:use crate::length::{check_host_len, check_lengths};
151:    fn route_report(
187:    /// `route_report` に入口の関数名を渡し、試しの実行の手段が無いときの `Unsupported` に使わせる。
194:        let report = self.route_report(op, opt_in, name)?;
217:        self.route_report(op, opt_in, "query_route")
237:                crate::cpu::subgroup_sum_f32(input, &mut out);
265:        check_lengths(OpId::F16Fma, n, None, self.device.limits().max_buffer_bytes)?;
269:                    check_host_len(len, n)?;
275:        let report = self.route_report(OpId::F16Fma, opt_in, "f16_fma")?;
279:                crate::cpu::f16_fma(a, b, c, out);
300:        check_lengths(
308:                check_host_len(x.len(), n)?;
309:                check_host_len(out.len(), crate::length::subgroup_output_len(n))?;
314:        let report = self.route_report(OpId::SubgroupSumU32, opt_in, "subgroup_sum_u32")?;
318:                crate::cpu::subgroup_sum_u32(x, out);
342:        check_lengths(
350:                check_host_len(x.len(), n)?;
351:                check_host_len(out.len(), crate::length::subgroup_output_len(n))?;
356:        let report = self.route_report(OpId::SubgroupSumF32, opt_in, "subgroup_sum_f32")?;
360:                crate::cpu::subgroup_sum_f32(x, out);
377:        check_lengths(
729:    fn query_route_reports_fallback_route() {
```

(5) の出力。

```text
margin subgroup-sum-f32-fg n=1 blocks=1 max=0e0
margin subgroup-sum-f32-fg n=31 blocks=1 max=0e0
margin subgroup-sum-f32-fg n=32 blocks=1 max=3.35686796849697e-8
margin subgroup-sum-f32-fg n=33 blocks=1 max=5.337674224106808e-8
margin subgroup-sum-f32-fg n=255 blocks=1 max=2.2240440556387224e-8
margin subgroup-sum-f32-fg n=256 blocks=1 max=1.6173850889528602e-8
margin subgroup-sum-f32-fg n=257 blocks=2 max=4.596052684280081e-8
margin subgroup-sum-f32-fg n=1000 blocks=4 max=4.919321777944319e-8
margin subgroup-sum-f32-fg n=65539 blocks=257 max=3.0823860390701323e-7
margin subgroup-sum-f32-fg n=4096 blocks=16 max=1.4504704131737224e-7
margin subgroup-sum-f32-fg n=65536 blocks=256 max=1.5740754378890993e-7
margin subgroup-sum-f32-fg n=262144 blocks=1024 max=2.790627640649346e-7
margin subgroup-sum-f32-fg n=4194304 blocks=16384 max=4.5725164680961514e-7
```

(9) の出力。

```text
record: commit=112380bbb00b6932fae9a9c1cc90aec0aa28a5c1 rs_files=27 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

(11) のログ（ジョブ名・ステップ名・時刻を除いて `Running` の行と `test result:` の行をログの出た順に並べた。lib の 92 passed の直後の 4 passed が `cpu_f16_fma` の結果）。

```text
Running unittests src/lib.rs (target/debug/deps/fandhe_silicon_exec-86ed7c4c97b219bf)
test result: ok. 92 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.35s
Running tests/cpu_subgroup_sum.rs (target/debug/deps/cpu_subgroup_sum-1fec7f5e66070089)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.57s
Running tests/length_host.rs (target/debug/deps/length_host-d17c7b4c6c32ccc1)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

確かめた時点は、main のコミット `112380b` と、`docs/spec` の gitlink `8e2bc9c0d89ce5c106b6d722cfd630cb393ebf45`。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-931）。

### 確かめ方

```text
grep -n 'check_lengths\|check_host_len\|subgroup_output_len\|route_report\|crate::cpu::' crates/exec/src/entry.rs
cargo test -p fandhe-silicon-exec --all-features --locked --lib
cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_zero --test length_device
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum
cargo test -p fandhe-silicon-exec --locked --test cpu_subgroup_sum
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum fg_is_within -- --nocapture
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum outputs_print_sha256_per_case -- --nocapture
cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields --test reasons_and_denormal --test route_evidence_calls
make verify
make build-profiles
make wasm-verify
make exec-surface-check
grep -n forbid crates/exec/src/lib.rs
```

## 段の表（TASK-931）

| 段 | Issue・PR | 節 |
| -- | --------- | -- |
| 1/6 | #6053（PR #7979） | subgroup の総和の host の列の長さと出力の数の照合（1/6） |
| 2/6 | #6054（PR #7980） | CPU の経路の subgroup の総和（2/6） |
| 3/6 | #6055（PR #7981） | CPU の経路の f32 の総和の非正規化数の判定（3/6） |
| 4/6 | #6056（PR #7982） | 入力の組と CPU の参照との比べの試験（4/6） |
| 5/6 | #6057（PR #7983） | 出力の sha256 の行（5/6） |
| 6/6 | #6058 | 完了の条件の確かめ（6/6） |

親 Issue は #873。

## 今の値の補い（base 515bb19）

記録時点の表と既存の節は書き換えない。以下は base `515bb19` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。

### `subgroup_sum_f32` の判定と返り値

「CPU の経路の subgroup の総和（2/6）」の節の「返り値」の節の、`subgroup_sum_f32` が差し込みの無い経路に届く呼び出しで `Err(ContractError::Unsupported { op: "subgroup_sum_f32", .. })` を返すと書いた文（`fault-injection` の `inject_denormal` で `Route::Cpu` に差し込んだ組では本体に届くという文を含む）と、同じ節の「試験」の表の `subgroup_sums_with_cpu_override_run_on_host` の行の「f32 は `Unsupported` で `out` が変わらない」は、今は次のとおり。

| 呼び出し | 返り値 |
| -------- | ------ |
| `subgroup_sum_f32`・経路が `Route::Cpu`・host の形（差し込みの無い呼び出し） | 判定を `crate::cpu::subgroup_sum_f32` で計算して本体に届き、`Ok(Outcome::Ran { report })`。`out` に書く |
| `subgroup_sum_f32`・`Route::Native` または `Route::GpuEmulated` に決まり、差し込みの無い呼び出し | `Err(ContractError::Unsupported { op: "subgroup_sum_f32", .. })`。`out` に書かない（`unsupported_fields.rs` の `unsupported_carries_entry_name_and_device_backend`） |

`entry::tests::subgroup_sums_with_cpu_override_run_on_host` は、`subgroup_sum_u32` に続けて `subgroup_sum_f32` を `Ran` として照らす。`out` は `[2.0]`、`report.route` は `Some(Route::Cpu)`、報告は `exec.query_route(OpId::SubgroupSumF32, over)` と等しく、`f32_denormal` は `behavior` が `Preserve`・`basis` が `Observed`。

`crates/exec/src/entry.rs` の `fn probe`（223 行）は、runner を差していないとき、(`OpId::SubgroupSumF32`, `Route::Cpu`) を 237 行の `crate::cpu::subgroup_sum_f32` で計算した 3 つの出力で返し、ほかの組は `Err(ContractError::Unsupported { .. })` を返す。入口の本体は、`subgroup_sum_u32` が 318 行、`subgroup_sum_f32` が 360 行、`f16_fma` が 279 行の `crate::cpu::` の呼び出し。検査と報告は `subgroup_sum_u32` が 300 行の `check_lengths`・308 行と 309 行の `check_host_len`・314 行の `route_report`、`subgroup_sum_f32` が 342 行・350 行と 351 行・356 行（`grep -n 'check_lengths\|check_host_len\|route_report\|crate::cpu::' crates/exec/src/entry.rs`）。

### device の形で経路が `Route::Cpu` に決まる subgroup の 2 入口

`crates/exec/tests/device_operands.rs` の `cpu_route_subgroup_sums_with_device_operands_are_unsupported_and_leave_buffers`（108 行。feature `native-blocking-wait` のビルドでだけ流れる）が、device の形の列を渡した `Exec::subgroup_sum_u32`・`Exec::subgroup_sum_f32` を、入口ごとに次の 2 つの組で呼ぶ。`CPU_ONLY` は `allow_cpu: true`・`allow_gpu_emulated: false` の印。

| 組 | `Exec` の作り方 | 上書き |
| -- | --------------- | ------ |
| 0 | `Exec::new(&dev, CPU_ONLY)` | `None` |
| 1 | `Exec::deny_by_default(&dev)` | `Some(CPU_ONLY)` |

device は `Scripted::bare()`、`n` は 257（出力は 256 要素ごとに 1 つで 2 つ）。`x` の buffer は `n` × 4 バイトを値 1 で、`out` の buffer は 2 × 4 バイトを値 2 で埋める。

| 照らす点 | 期待 |
| -------- | ---- |
| 戻り値 | `Err(ContractError::Unsupported { op: <入口の関数名>, .. })`（`"subgroup_sum_u32"`・`"subgroup_sum_f32"`）。`Ran` にならない |
| `query_ex` の数え | 1 回 |
| `memory()`・`queue()` の数え | 呼び出しの前後で変わらない |
| `x`・`out` の buffer の中身 | 呼ぶ前のまま |

`cargo test -p fandhe-silicon-exec --all-features --locked --test device_operands` は 3 passed; 0 failed; 0 ignored（`unavailable_leaves_device_buffers_unchanged`、`cpu_route_with_device_operands_is_unsupported_and_leaves_buffers` と上の試験）。この試験は PR #7985 で加わった。

### 書き方の対応

3 行の冒頭の文は、頁の全体（host の形の列の長さの検査、経路 `Route::Cpu` の `f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`、その試験）の所在の文にした。`CLAUDE.md` の構成ツリーと `README.md` の文書の表の、この頁の行と同じ範囲を指す。

「CPU の経路の f32 の総和の非正規化数の判定（3/6）」の節で、経路を 1 文字で指していた書き方は、次の名前に直した。

| 場所 | 直し後の書き方 |
| ---- | -------------- |
| 「入口の返り値」の表の 1 行目と、表の下の文 | `allow_cpu: true`・`allow_gpu_emulated: false` の印 |
| 同表の 2 行目 | `allow_gpu_emulated: true`・`allow_cpu: false` の印と、`allow_gpu_emulated: true`・`allow_cpu: true` の印 |
| 「試験」の表の `query_route_reports_fallback_route` の行 | `allow_cpu: true`・`allow_gpu_emulated: false` の印 |
| 「試験」の表の `uninjected_cpu_route_is_judged_on_host` の行 | `Route::Cpu` の経路 |
| 「直した既存の試験」の表の `unreachable_native_follows_opt_in_order` の行 | `Route::Cpu` |

### 完了の条件の確かめ（6/6）の値

| 項目 | 記録の値 → 今の値 | 測ったコマンド |
| ---- | ----------------- | -------------- |
| `crates/exec/src` のファイル | 13 ファイルのまま | `ls crates/exec/src` |
| `.rs` の数 | `make exec-surface-check` と `make exec-deps-check` の `rs_files=26` → `rs_files=27`。`crates/exec/tests` は `common/mod.rs` を含め 13 ファイル → 14 ファイル（`cpu_subgroup_sum.rs` が加わった） | `make exec-surface-check`、`make exec-deps-check` |
| `forbid` の行 | `20:#![forbid(unsafe_code)]` → `21:#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |
| `cpu_subgroup_sum` | feature なしで 5 passed、`--all-features` で 6 passed（`forced_native_off_runs_cpu` が `fault-injection` の裏） | `cargo test -p fandhe-silicon-exec --locked --test cpu_subgroup_sum`、`cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_subgroup_sum` |
| exec の試験の合計 | feature なし 114、`fault-injection` 付き 148、`native-blocking-wait` 付き 121、`--all-features` 155（`--lib` は順に 68・91・69・92） | `cargo test -p fandhe-silicon-exec <組>` |
| `make verify` | `test result:` の行 231、passed の合計 2368、failed の合計 0、ignored の合計 7。終了コード 0 | `make verify` |

### 段の表

| 段 | Issue・PR | 節 |
| -- | --------- | -- |
| 6/6 | #6058（PR #7984） | 完了の条件の確かめ（6/6） |

## atomic の加算の host の列の長さと出力の数の照合（1/4）（TASK-932）

下-2（`crates/exec`）の `atomic_add` の host の形の列の長さの検査と、その試験の所在をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-932（親の Issue は #874）。

### 本リポの現状の事実

記録時点の commit は `2bc1090`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `src/` のファイル | `binary16.rs`・`cpu.rs`・`denormal.rs`・`denormal_tests.rs`・`entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs` | `ls crates/exec/src` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

### 所在

- `atomic_add` からの照合の呼び出し: `grep -n 'check_host_len' crates/exec/src/entry.rs`（`crates/exec/src/length.rs` の `check_host_len` を `x` と `out` に呼ぶ）

### 判定の順

`atomic_add` は host の形のとき、次の順で判定する。

| 段 | 内容 | 違うときの返り値 |
| -- | ---- | ---------------- |
| 1 | `check_lengths`（要素数・出力の数 0・バイト数のあふれ・上限超過） | `Err(ContractError::InvalidArgument(..))` |
| 2 | `x` の列の長さが `n` と同じ | `Err(ContractError::InvalidArgument(..))` |
| 3 | `out` の列の長さが `m` と同じ | `Err(ContractError::InvalidArgument(..))` |
| 4 | 能力の問合せと経路の選び方 | 経路の選び方の結果 |

段 1〜3 の拒否では `query_ex`・`memory`・`queue` を呼ばず、出力の列に書かない。
device の形（`native-blocking-wait`）は列の長さを照らさない。列の中身は読まず、出力に書かない。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `length_host.rs` の `mismatched_atomic_columns_are_rejected` | `n` = 8・`m` = 3 で (`x` の長さ, `out` の長さ) が (7, 3)・(9, 3)・(8, 2)・(8, 4) の呼び出し（上書き × 既定の印 × device の答え） | `InvalidArgument`、`query_ex`・`memory`・`queue` の数えが 0、出力が番兵のまま |
| `length_host.rs` の `matching_atomic_columns_reach_route_choice` | (`n`, `m`) が (8, 3)・(2, 5) で `x` の長さ `n`・`out` の長さ `m` の呼び出し | `InvalidArgument` でなく、`query_ex` の数えが 1 |
| `length_device.rs` の `device_form_atomic_passes_one_before_byte_overflow` | device の形で (`n`, `m`) が (`u64::MAX / 4`, 1)・(1, `u64::MAX / 4`) | `InvalidArgument` でなく、`query_ex` の数えが 1、確保・読み出しの数えが増えず、buffer の中身が変わらない |

### 出力の列の長さを直した既存の試験

| ファイル | 直し |
| -------- | ---- |
| `crates/exec/src/entry.rs` | `atomic_add` の `out` を出力の数の長さ（`m` = 1 で 1）で渡す |
| `fault_injection_ports.rs`・`reasons_and_denormal.rs`・`unsupported_fields.rs`・`route_evidence_calls.rs`・`opt_in_default.rs`・`route_query.rs` | 同上。番兵のままかの比べも同じ長さで行う |
| `length_zero.rs` | `valid_length_reaches_route_choice` が `f16_fma` の出力を長さ 2、ほかの入口の出力を長さ 1 で渡す |
| `length_limit.rs` | `atomic_add` の `out` を長さ `m.min(64)`（最小 1）で作る。一つ手前の `n`・`m` は全部の入口が `InvalidArgument` |

`f16_fma` の列と subgroup の列は変えていない。

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_zero --test length_device
cargo test -p fandhe-silicon-exec --locked --test length_host
```

## CPU の経路の atomic の加算と非正規化数の判定（2/4）（TASK-932）

### 所在

- 計算の本体: `crates/exec/src/cpu.rs` の `atomic_add`（`pub(crate)`）
- 入口の本体と試しの実行の腕: `grep -n 'crate::cpu::atomic_add\|Outcome::Ran' crates/exec/src/entry.rs`

### 返り値

| 経路と列の形 | 返り値 | 出力 |
| ------------ | ------ | ---- |
| 経路なし | `Ok(Outcome::Unavailable { report })` | 書かない |
| `Route::Cpu` で host の形 | `Ok(Outcome::Ran { report })` | `out` に書く |
| `Route::Cpu` で device の形 | `Err(ContractError::Unsupported { op: "atomic_add", .. })` | buffer に書かない |
| `Route::Native`・`Route::GpuEmulated` | `Err(ContractError::Unsupported { .. })` | 書かない |

`report` は `Exec::query_route` と同じ値。`memory()`・`queue()` を呼ばない。

### 計算

| 手順 | 内容 |
| ---- | ---- |
| 1 | `out` を `+0.0` で埋める（`out` が空なら何もしない） |
| 2 | `x` を `out` の長さずつに区切る |
| 3 | 区切りごとに添字の昇順で、`out` の同じ位置の要素へ f32 の `+` で足す |

### 試しの実行

| 組 | 試しの実行 |
| -- | ---------- |
| (`OpId::AtomicAdd`, `Route::Cpu`) | 768 要素の入力を CPU の経路の atomic の加算の関数で計算し、3 つの出力を返す |
| (`OpId::AtomicAdd`, `Route::Native`・`Route::GpuEmulated`) | `Err(ContractError::Unsupported { .. })` |
| runner を差した crate の中の試験 | runner の返し |

`allow_cpu: true`・`allow_gpu_emulated: false` の印で呼んだ呼び出しの報告は、`route` が `Some(Route::Cpu)`、`f32_denormal` が `behavior: Preserve`・`basis: Observed`・`documented: None`。

### 入口の数

private の `Exec::run` を消した。入口の数と返し方は変わらない。

### 試験

| ファイル | 試験 | 照らし |
| -------- | ---- | ------ |
| `cpu.rs` | `atomic_add_starts_from_positive_zero_in_index_order` | 添字の昇順の足し込み、`-0.0` の列の和が `+0.0`、足す順で結果が変わる入力、空の `out` |
| `entry.rs` | `atomic_add_with_cpu_override_runs_on_host` | `Ran`・`out` が 2・報告が `query_route` と等しい・`Preserve`・`Observed` |
| `entry.rs` | `uninjected_cpu_route_is_judged_on_host` | 差し込みの無い `Route::Cpu` の経路が `Preserve`・`Observed`（`SubgroupSumF32` と `AtomicAdd`） |
| `opt_in_default.rs` | `cpu_route_writes_atomic_output` | `Ran`・`out` が 2・`memory()`・`queue()` の呼びが 0 |
| `unsupported_fields.rs` | `atomic_add_cpu_route_runs` | `Ran` |

### 直した既存の試験

| ファイル | 直し |
| -------- | ---- |
| `entry.rs` | `query_route_reports_fallback_route` が `AtomicAdd` を `Route::Cpu` の `Ok` として照らす。`uninjected_route_keeps_unsupported_error` が `SubgroupSumF32` と `AtomicAdd` の `Route::GpuEmulated` の `Unsupported` を照らす |
| `fault_injection_ports.rs` | `forced_op_follows_opt_in` が `Route::Cpu` に差し込んだ `atomic_add` を `Ran` として照らす |
| `opt_in_default.rs` | `chosen_route_does_not_write_output` が `Route::GpuEmulated` に決まる印の 4 つの入口を照らす |
| `route_query.rs` | `query_route_matches_entry_report` の `Ran` の腕が `AtomicAdd` を含む。`query_route_follows_per_key_answer` と `unreachable_native_follows_opt_in_order` が `AtomicAdd` の `Route::Cpu` を `Ok` として照らす |
| `unsupported_fields.rs` | `unsupported_carries_entry_name_and_device_backend` の `atomic_add` が `Route::GpuEmulated` に決まる印で呼ぶ |

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --lib
cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields
```

## 入力の組と CPU の参照との比べと出力の sha256 の行（3/4）（TASK-932）

### 所在

`crates/exec/tests/cpu_atomic_add.rs`（公開のパスから `Exec::atomic_add` を host の形で呼ぶ外の crate の統合テスト）。

### 経路 `Route::Cpu` を選ばせる組

| 組 | device の答え | `Exec` の作り方 | 上書き |
| -- | ------------- | --------------- | ------ |
| (a) | `Unsupported` | `Exec::new(&dev, CPU_ONLY)` | `None` |
| (b) | `Unsupported` | `Exec::deny_by_default(&dev)` | `Some(CPU_ONLY)` |
| 故障の差し込み（feature `fault-injection` のビルドでだけ） | `S2` | `Exec::deny_by_default(&dev).force_unsupported(OpId::AtomicAdd)` | `Some(CPU_ONLY)` |

呼び出しごとに新しい試験用の device を作る。

### 入力と比べ方

| 入力 | 参照 | 比べ方 |
| ---- | ---- | ------ |
| `generate_atomic_add_ax` | `reference_atomic_add_f32`（`F32SubnormalHandling::Preserve`） | `compare_bits_f32` |
| `generate_atomic_add_ag` | 同上 | `compare_relative_sum_f32` |
| `generate_atomic_add_as` | 同上 | `compare_relative_sum_f32` |

長さは `FALLBACK_LENGTHS` の 13 通り、出力の数は `ATOMIC_ADD_OUTPUT_COUNTS`（1・64・4096）。`generate_atomic_add_ax` の組は `ax_pair_allowed(n, m)` が真のものだけ。詳細は [parity-fallback-inputs.md](parity-fallback-inputs.md)・[parity-fallback-compare.md](parity-fallback-compare.md)。

### (n, m) の組の数

| 入力 | m = 1 | m = 64 | m = 4096 | 計 |
| ---- | ----- | ------ | -------- | -- |
| `generate_atomic_add_ax` | 9 | 12 | 13 | 34 |
| `generate_atomic_add_ag` | 13 | 13 | 13 | 39 |
| `generate_atomic_add_as` | 13 | 13 | 13 | 39 |

### 試験

| 試験 | 入力 | 組 | 期待 |
| ---- | ---- | -- | ---- |
| `ax_outputs_match_bits_over_lengths_and_counts` | `generate_atomic_add_ax` の 34 組 | (a)・(b) | ビット同一。組の数が 34 |
| `ag_is_within_relative_bound_over_lengths_and_counts` | `generate_atomic_add_ag` の 39 組 | (a) | 相対の条件に合い、余裕の最大が有限 |
| `as_patterns_are_compared_with_preserving_reference_over_lengths_and_counts` | `generate_atomic_add_as` の 39 組 | (a) | 相対の条件に合う |
| `outputs_print_sha256_per_case` | 上の 3 入力の 112 組 | (a) | 同じ入力の 2 回の sha256 が等しい |
| `forced_native_off_runs_cpu`（feature `fault-injection`） | `generate_atomic_add_as(1000, 64)` | 故障の差し込み | 上の照らしに加え `native_reason` が `Some(UnavailableReason::ForcedUnsupported)` |

### 報告と数えるもの

- `route` が `Some(Route::Cpu)`。`query_route` と同じ報告になる。
- `f32_denormal` が `behavior: Preserve`・`basis: Observed`・`documented: None`。
- `query_ex_calls()` が 1、`memory_calls()`・`queue_calls()` が 0。
- `out` は長さ m で、`7.0` で埋めて渡す。

### 余裕の行の出し方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add ag_is_within -- --nocapture
```

`margin atomic-add-ag n=… m=… max=…` の行が 39 行出る。照らすのは `is_match()` と最大が有限であることだけ。`make verify` のログには出ない。

### sha256 の行の形

`sha256 <入力の名前> n=<n> m=<m> <16 進>`。入力の名前は `FallbackInputKind::….name()` の値（`atomic-add-ax`・`atomic-add-ag`・`atomic-add-as`）、16 進は小文字の 64 文字。1 回の流しで 112 行出る。

### バイト列の作り方

出力の各要素について、`0.0` に等しい要素は `0u32`、それ以外は `to_bits()` とし、その `to_le_bytes()` を添字の順に並べる（`-0.0` は `+0.0` のビット列になる）。sha256 は core の `hash::sha256_bytes`。

### 照らすもの

呼ぶたびに新しい試験用の device と `Exec` を作り、同じ入力を 2 回呼ぶ。照らすのは 2 つの sha256 が等しいことと、上の報告・数えの照らし。組は (a)。

### 3 回の流し方

```bash
d=$(mktemp -d)
for i in 1 2 3; do
  cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add outputs_print_sha256_per_case -- --nocapture | grep '^sha256 ' > "$d/run-$i.txt"
done
wc -l "$d"/run-*.txt
sha256sum "$d"/run-*.txt
cat "$d"/run-1.txt "$d"/run-2.txt "$d"/run-3.txt | sort | uniq -c | awk '{print $1}' | sort | uniq -c
rm -r "$d"
```

各回の行を比べる。`--test-threads=1` を付けると最初の行の頭に `test … ` が付き、`grep '^sha256 '` から 1 行漏れる。

### 確かめ方

```bash
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add
cargo test -p fandhe-silicon-exec --locked --test cpu_atomic_add
```

## 完了の条件の確かめ（4/4）（TASK-932）

4/4（#6062）で、下-2 の CPU の経路の `atomic_add` について本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-932（ポインタのみ）。`uname -sm` は `Linux x86_64`。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 入口の照合と本体と判定の順 | `grep -n 'check_lengths\|check_host_len\|route_report\|crate::cpu::atomic_add' crates/exec/src/entry.rs`、`grep -n 'fn run\b' crates/exec/src/entry.rs` | `atomic_add` の中は 375 行 `check_lengths`、383 行と 384 行 `check_host_len`、389 行 `route_report`、393 行 `crate::cpu::atomic_add`。`probe` の中の `crate::cpu::atomic_add` は 226 行。2 本目の grep は 0 行。全部の行は下の (1) の出力 |
| (2) | 照合と本体と判定の単体試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --lib` | `test result: ok. 93 passed; 0 failed; 0 ignored` |
| (3) | 列の長さの照合の入口経由の試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_zero --test length_device` | `tests/length_device.rs` が 4 passed、`tests/length_host.rs` が 6 passed、`tests/length_limit.rs` が 4 passed、`tests/length_zero.rs` が 5 passed。4 本とも 0 failed; 0 ignored |
| (4) | 入力の組と参照との比べ | `cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add`、`cargo test -p fandhe-silicon-exec --locked --test cpu_atomic_add` | `--all-features` は 5 passed; 0 failed; 0 ignored、既定の feature は 4 passed; 0 failed; 0 ignored |
| (5) | 余裕の行 | `cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add ag_is_within -- --nocapture` の出力の `^margin ` の行 | 39 行。下の (5) の出力 |
| (6) | `cargo test` を 3 回流した sha256 の行 | 「入力の組と CPU の参照との比べと出力の sha256 の行（3/4）（TASK-932）」の「3 回の流し方」のコマンド | 各回の `wc -l` は 112、3 つのファイルの `sha256sum` の値は 3 回とも `6b113ab7a6ff664d3d4194e6ee1a0f7189a632f6bdc649d4c51e9a12cf7e0e6f`、`uniq -c` の集計は `112 3` の 1 行。入力の名前ごとの行の数は `atomic-add-ag` が 39、`atomic-add-as` が 39、`atomic-add-ax` が 34 |
| (7) | 直した既存の試験 | `gh pr diff 7987 --name-only`・`gh pr diff 7988 --name-only` の `crates/exec/` の行、`cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields --test reasons_and_denormal --test route_evidence_calls` | #7987 は `src/entry.rs`・`src/length.rs` と `tests/` の `fault_injection_ports.rs`・`length_device.rs`・`length_host.rs`・`length_limit.rs`・`length_zero.rs`・`opt_in_default.rs`・`reasons_and_denormal.rs`・`route_evidence_calls.rs`・`route_query.rs`・`unsupported_fields.rs`。#7988 は `src/cpu.rs`・`src/entry.rs`・`src/lib.rs` と `tests/` の `fault_injection_ports.rs`・`opt_in_default.rs`・`route_query.rs`・`unsupported_fields.rs`。件数は `fault_injection_ports` が 8 passed、`opt_in_default` が 9 passed、`reasons_and_denormal` が 4 passed、`route_evidence_calls` が 4 passed、`route_query` が 6 passed、`unsupported_fields` が 5 passed で、すべて 0 failed; 0 ignored |
| (8) | 既存の試験とビルドの組 | `make verify`、`make build-profiles`、`make wasm-verify` | 3 つとも終了コード 0。`make verify` の exec は lib が 93 passed、`tests/cpu_atomic_add.rs` が 5 passed、`tests/cpu_f16_fma.rs` が 4 passed、`tests/cpu_subgroup_sum.rs` が 6 passed、`tests/device_operands.rs` が 3 passed、`tests/fault_injection_ports.rs` が 8 passed、`tests/length_device.rs` が 4 passed、`tests/length_host.rs` が 6 passed、`tests/length_limit.rs` が 4 passed、`tests/length_zero.rs` が 5 passed、`tests/opt_in_default.rs` が 9 passed、`tests/reasons_and_denormal.rs` が 4 passed、`tests/route_evidence_calls.rs` が 4 passed、`tests/route_query.rs` が 6 passed、`tests/unsupported_fields.rs` が 5 passed で、すべて 0 failed。`make build-profiles` の exec は feature なし・`native-blocking-wait`・`fault-injection` の 3 組で `clippy`・`test`・wasm32 の `cargo check` が通る。`make wasm-verify` は `cargo check (fandhe-silicon-exec tests, wasm32-unknown-unknown, …)` を default features・`native-blocking-wait`・`fault-injection` で通る |
| (9) | 公開の面が変わらない | `make exec-surface-check` | 終了コード 0。`op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0`、`port_names_all=6`、`items_default=458 items_all=511`。下の (9) の出力 |
| (10) | 依存と unsafe を足していない | `gh pr diff <n> --name-only`（n は 7987・7988・7989）を `grep -E '^(Cargo\.toml\|Cargo\.lock\|deps-approved\.toml\|deny\.toml\|crates/core/\|crates/upper/\|crates/exec/Cargo\.toml)'` で絞る、`grep -n forbid crates/exec/src/lib.rs` | 3 つとも grep の出力は空。`crates/exec/src/lib.rs` の 22 行に `#![forbid(unsafe_code)]` |
| (11) | CI で流れる場所 | main の push の CI の run 41868695385（`conclusion=success`、`headSha=d2893a9bf6512370d790793feda235beb7106da3`）の job `make verify (fmt + clippy + test)`（job id 126709542476）のログ | 下の (11) のログの行。`cpu_atomic_add` と `length_host` が出る |

(1) の出力。

```text
15:use crate::length::{check_host_len, check_lengths};
150:    fn route_report(
200:        self.route_report(op, opt_in, "query_route")
226:                crate::cpu::atomic_add(input, &mut out);
254:        check_lengths(OpId::F16Fma, n, None, self.device.limits().max_buffer_bytes)?;
258:                    check_host_len(len, n)?;
264:        let report = self.route_report(OpId::F16Fma, opt_in, "f16_fma")?;
289:        check_lengths(
297:                check_host_len(x.len(), n)?;
298:                check_host_len(out.len(), crate::length::subgroup_output_len(n))?;
303:        let report = self.route_report(OpId::SubgroupSumU32, opt_in, "subgroup_sum_u32")?;
331:        check_lengths(
339:                check_host_len(x.len(), n)?;
340:                check_host_len(out.len(), crate::length::subgroup_output_len(n))?;
345:        let report = self.route_report(OpId::SubgroupSumF32, opt_in, "subgroup_sum_f32")?;
375:        check_lengths(
383:                check_host_len(x.len(), n)?;
384:                check_host_len(out.len(), m)?;
389:        let report = self.route_report(OpId::AtomicAdd, opt_in, "atomic_add")?;
393:                crate::cpu::atomic_add(x, out);
752:    fn query_route_reports_fallback_route() {
```

(5) の出力。

```text
margin atomic-add-ag n=1 m=1 max=0e0
margin atomic-add-ag n=1 m=64 max=0e0
margin atomic-add-ag n=1 m=4096 max=0e0
margin atomic-add-ag n=31 m=1 max=1.4212126353121722e-7
margin atomic-add-ag n=31 m=64 max=0e0
margin atomic-add-ag n=31 m=4096 max=0e0
margin atomic-add-ag n=32 m=1 max=0e0
margin atomic-add-ag n=32 m=64 max=0e0
margin atomic-add-ag n=32 m=4096 max=0e0
margin atomic-add-ag n=33 m=1 max=0e0
margin atomic-add-ag n=33 m=64 max=0e0
margin atomic-add-ag n=33 m=4096 max=0e0
margin atomic-add-ag n=255 m=1 max=2.6274075555018383e-8
margin atomic-add-ag n=255 m=64 max=1.1455645756409365e-7
margin atomic-add-ag n=255 m=4096 max=0e0
margin atomic-add-ag n=256 m=1 max=1.4913489657336404e-8
margin atomic-add-ag n=256 m=64 max=1.1896968729991551e-7
margin atomic-add-ag n=256 m=4096 max=0e0
margin atomic-add-ag n=257 m=1 max=5.5649060695687195e-8
margin atomic-add-ag n=257 m=64 max=1.1260264787608179e-7
margin atomic-add-ag n=257 m=4096 max=0e0
margin atomic-add-ag n=1000 m=1 max=4.189949681834022e-8
margin atomic-add-ag n=1000 m=64 max=1.2713833054027617e-7
margin atomic-add-ag n=1000 m=4096 max=0e0
margin atomic-add-ag n=65539 m=1 max=4.591491703389732e-8
margin atomic-add-ag n=65539 m=64 max=1.5845194743196718e-7
margin atomic-add-ag n=65539 m=4096 max=2.67072941338045e-7
margin atomic-add-ag n=4096 m=1 max=1.314528607385641e-8
margin atomic-add-ag n=4096 m=64 max=1.758139166306915e-7
margin atomic-add-ag n=4096 m=4096 max=0e0
margin atomic-add-ag n=65536 m=1 max=5.4405779834576424e-8
margin atomic-add-ag n=65536 m=64 max=2.40819414122819e-7
margin atomic-add-ag n=65536 m=4096 max=2.872197959403692e-7
margin atomic-add-ag n=262144 m=1 max=1.954442517842839e-8
margin atomic-add-ag n=262144 m=64 max=1.2958882866885887e-7
margin atomic-add-ag n=262144 m=4096 max=3.2007235218561117e-7
margin atomic-add-ag n=4194304 m=1 max=2.4376108914373616e-8
margin atomic-add-ag n=4194304 m=64 max=1.8418538516965294e-7
margin atomic-add-ag n=4194304 m=4096 max=3.135756748211731e-7
```

(9) の出力。

```text
record: commit=d2893a9bf6512370d790793feda235beb7106da3 rs_files=28 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

(11) のログ（ジョブ名・ステップ名・時刻を除いて `Running` の行と `test result:` の行をログの出た順に並べた）。

```text
Running unittests src/lib.rs (target/debug/deps/fandhe_silicon_exec-86ed7c4c97b219bf)
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/cpu_atomic_add.rs (target/debug/deps/cpu_atomic_add-caae2dc3a33fb01d)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.88s
Running tests/cpu_f16_fma.rs (target/debug/deps/cpu_f16_fma-94c5986b155befb1)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.61s
Running tests/cpu_subgroup_sum.rs (target/debug/deps/cpu_subgroup_sum-1fec7f5e66070089)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.66s
Running tests/device_operands.rs (target/debug/deps/device_operands-f38e027ecd725064)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/fault_injection_ports.rs (target/debug/deps/fault_injection_ports-3b7137a9d5d214b6)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/length_device.rs (target/debug/deps/length_device-716b5f7cf8ef7941)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/length_host.rs (target/debug/deps/length_host-d17c7b4c6c32ccc1)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

確かめた時点は、main のコミット `d2893a9` と、`docs/spec` の gitlink `8e2bc9c0d89ce5c106b6d722cfd630cb393ebf45`。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-932）。

### 確かめ方

```text
grep -n 'check_lengths\|check_host_len\|route_report\|crate::cpu::atomic_add' crates/exec/src/entry.rs
grep -n 'fn run\b' crates/exec/src/entry.rs
cargo test -p fandhe-silicon-exec --all-features --locked --lib
cargo test -p fandhe-silicon-exec --all-features --locked --test length_host --test length_limit --test length_zero --test length_device
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add
cargo test -p fandhe-silicon-exec --locked --test cpu_atomic_add
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add ag_is_within -- --nocapture
cargo test -p fandhe-silicon-exec --all-features --locked --test cpu_atomic_add outputs_print_sha256_per_case -- --nocapture
cargo test -p fandhe-silicon-exec --all-features --locked --test fault_injection_ports --test opt_in_default --test route_query --test unsupported_fields --test reasons_and_denormal --test route_evidence_calls
make verify
make build-profiles
make wasm-verify
make exec-surface-check
grep -n forbid crates/exec/src/lib.rs
```

## 段の表（TASK-932）

| 段 | Issue・PR | 節 |
| -- | --------- | -- |
| 1/4 | #6059（PR #7987） | atomic の加算の host の列の長さと出力の数の照合（1/4） |
| 2/4 | #6060（PR #7988） | CPU の経路の atomic の加算と非正規化数の判定（2/4） |
| 3/4 | #6061（PR #7989） | 入力の組と CPU の参照との比べと出力の sha256 の行（3/4） |
| 4/4 | #6062 | 完了の条件の確かめ（4/4） |

親 Issue は #874。

## 今の値の補い（base 8e4b99b）

記録時点の表と既存の節は書き換えない。以下は base `8e4b99b` で観測した今の値（0 failed; 0 ignored。実行環境は GPU なしの Linux）。`atomic_add` は経路が `Route::Cpu` で host の形なら `crate::cpu::atomic_add`（`crates/exec/src/cpu.rs` の 36 行）で `out` に書き、`Ok(Outcome::Ran { report })` を返す。4 つの入口（`f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add`）が `Route::Cpu` で host の形なら `Ran` を返す。`Exec::run` は無く、`Route::Native`・`Route::GpuEmulated` が決まった呼び出しは入口の関数名の `Err(ContractError::Unsupported { .. })` を返す。

### 既存の節の試験の名前と期待

| 既存の節の記述 | 今の試験・期待 |
| -------------- | -------------- |
| 「CPU の経路の f16 の積和（2/4）」の試験の表の `entry.rs` の `four_entries_with_cpu_override_are_unsupported_err`（`allow_cpu` だけの上書きで `subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` が `Unsupported`） | この名前の試験は無い。`entry::tests::atomic_add_with_cpu_override_runs_on_host` が `allow_cpu` だけの上書きで `atomic_add` を呼び、`Ran`・`Route::Cpu`・`query_route` と等しい報告・`out` が `[2.0]`・`Preserve`・`Observed` を照らす |
| 同じ表の `opt_in_default.rs` の `chosen_route_does_not_write_output`（`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` が `Unsupported` で出力が変わらない） | `allow_gpu_emulated` だけの印（`Route::GpuEmulated` に決まる）で 4 つの入口を呼び、どれも `Unsupported` で出力が変わらないことを照らす。`external_interop_shared_memory`・`hardware_ray_tracing`・`unified_memory_sharing` が上書き 5 通りで `Unsupported` を返す照らしは変わらない |
| 同じ表の `fault_injection_ports.rs` の `forced_op_follows_opt_in`（`atomic_add` も `f16_fma` と同じ） | `Route::Cpu` に決まる上書きで、`f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` の 4 つを `Ran`・報告が `query_route` と同じ・出力が書き換わるとして照らす |
| TASK-931 の 1/6 の節の `length_limit.rs` の行（一つ手前の `n` は `atomic_add` のほかが `InvalidArgument`）と、その下の「`atomic_add` の列と `f16_fma` の列は変えていない」 | 一つ手前の `n` は `atomic_add` を含む 4 つの入口がどれも `InvalidArgument`（`query_ex`・`memory`・`queue` の数えは 0、出力は番兵のまま）。`atomic_add` の `out` は長さ `m.min(64)`（最小 1）。`length_zero.rs` の `valid_length_reaches_route_choice` の出力の列は `f16_fma` が長さ 2、ほかの 3 つが長さ 1 |
| 「CPU の経路の subgroup の総和（2/6）」の「直した既存の試験」の `entry.rs` の `four_entries_with_cpu_override_are_unsupported_err` が `atomic_add` だけを照らす | 上の 1 行目のとおり、この名前の試験は無く `atomic_add_with_cpu_override_runs_on_host` に置き換わっている |
| 「CPU の経路の f32 の総和の非正規化数の判定（3/6）」の「直した既存の試験」の `entry.rs` の `uninjected_route_keeps_unsupported_error`（`atomic_add` だけを照らす） | `SubgroupSumF32` と `AtomicAdd` の両方を回し、`Exec::new` に `allow_gpu_emulated`・`allow_cpu` の両方の印を渡して `Route::Cpu` にだけ判定を差した `query_route` が、`Err(ContractError::Unsupported { op: "query_route", .. })` を返すことを照らす |
| 同じ節の `route_query.rs` の `unreachable_native_follows_opt_in_order`・`query_route_follows_per_key_answer` | `SubgroupSumF32`・`AtomicAdd` の `Route::GpuEmulated` が `Unsupported`、`Route::Cpu` が `Ok`。`query_route_follows_per_key_answer` は、`allow_cpu` だけの印の `query_route` が 4 つの `OpId` のどれでも `Ok` でなければ `panic!("expected Ok: ..")` で失敗する |
| 同じ節の `opt_in_default.rs` の `chosen_route_does_not_write_output`・`unsupported_fields.rs` の `unsupported_carries_entry_name_and_device_backend` | 4 つの入口とも `Route::GpuEmulated` に決まる印（`GPU_OK`）で呼び、入口の関数名の `Unsupported` の欄を照らす |

### `atomic_add` の host の形の動き

TASK-932 の 1/4 の節の「判定の順」の下の「列の中身は読まず、出力に書かない」は、device の形については今も当たる。host の形で段 1〜3 を通り、経路が `Route::Cpu` に決まった呼び出しは `out` に書き、`Ok(Outcome::Ran { report })` を返す。device の形で経路が `Route::Cpu` に決まった呼び出しは `Err(ContractError::Unsupported { op: "atomic_add", .. })` を返し、出力に書かない。

### device の形の `atomic_add`

`device_operands.rs`（feature `native-blocking-wait`）に `cpu_route_atomic_add_with_device_operands_is_unsupported_and_leaves_buffers` が加わった。`n` が 257、`m` が 64 の `AtomicAddOperands::Device` を、`Exec::new` に `allow_cpu` だけの印を渡した組と、`Exec::deny_by_default` に呼び出しごとの上書きで同じ印を渡した組の 2 通りで呼ぶ。期待は次のとおり。

| 照らすもの | 期待 |
| ---------- | ---- |
| 返り値 | `Err(ContractError::Unsupported { op: "atomic_add", .. })` |
| 数え | `query_ex` が 1 回。`memory`・`queue` の数えは呼ぶ前と同じ |
| buffer の中身 | `x`（`n * 4` バイト）と `out`（`m * 4` バイト）を `download_blocking` で読み戻すと、確保時に入れた値のまま |

### 確かめた値

| 流し方 | 今の値 |
| ------ | ------ |
| `cargo test -p fandhe-silicon-exec` | 合計 123 passed |
| `cargo test -p fandhe-silicon-exec --features native-blocking-wait` | 合計 132 passed |
| `cargo test -p fandhe-silicon-exec --features fault-injection` | 合計 158 passed |
| `cargo test -p fandhe-silicon-exec --all-features` | 合計 167 passed（`cpu_atomic_add` 5、`device_operands` 4） |
| `make verify` | `test result:` の行は 232、passed の合計 2380（failed の合計 0、ignored の合計 7。終了コード 0） |
| `make exec-surface-check` | 終了コード 0。`rs_files=28` |

### 段の表（TASK-932）の 4/4 の行

「段の表（TASK-932）」の 4/4 の行は、Issue が #6062、PR が #7990。
