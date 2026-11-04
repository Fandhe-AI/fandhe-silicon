# 下-2 の CPU の経路の所在（TASK-930）

下-2（`crates/exec`）の `f16_fma` の host の形の列の長さの検査、経路 `Route::Cpu` の f16 の積和、その試験の所在と、本リポの現状の事実をまとめる。
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
| C だけ許す | `Ok`（入口は `Ok(Outcome::Ran { .. })`） |
| G だけ許す・両方許す | `Err(ContractError::Unsupported { .. })` |

C だけ許す呼び出しの報告は、`route` が `Some(Route::Cpu)`、`f32_denormal` が `behavior: Preserve`・`basis: Observed`・`documented: None`。判定は `Exec` の値ごとに保持する。

### 試験

| ファイル | 試験 | 照らし |
| -------- | ---- | ------ |
| `entry.rs` | `query_route_reports_fallback_route` | `SubgroupSumF32` の C だけの印で `Preserve`・`Observed`・`documented` が `None` |
| `entry.rs` | `subgroup_sums_with_cpu_override_run_on_host` | f32 は `Ran`・`out` が 2・報告が `query_route` と等しい |
| `entry.rs` | `cpu_judgment_is_kept_per_exec` | 2 回の `query_route` と入口の報告が等しい |
| `entry.rs` | `uninjected_cpu_route_is_judged_on_host` | 差し込みの無い C の経路が `Preserve`・`Observed` |
| `opt_in_default.rs` | `cpu_route_writes_subgroup_f32_output` | `Ran`・`out` が 2・`memory()`・`queue()` の呼びが 0 |
| `unsupported_fields.rs` | `subgroup_sum_f32_cpu_route_runs` | `Ran` |

### 直した既存の試験

| ファイル | 直し |
| -------- | ---- |
| `entry.rs` | `uninjected_route_keeps_unsupported_error` が `atomic_add` だけを照らす |
| `route_query.rs` | `unreachable_native_follows_opt_in_order` が `SubgroupSumF32` の C を `Ok` として照らす。`query_route_matches_entry_report` の `Ran` の腕が `SubgroupSumF32` を含む |
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
