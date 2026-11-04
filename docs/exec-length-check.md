# 下-2 の入口の長さの検査の所在（TASK-924）

下-2（`crates/exec`）の 4 つの入口が最初に行う長さの検査の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-924（親の Issue は #868）。

## 本リポの現状の事実

記録時点の commit は `da074ee`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| exec の `src/` のファイル | `entry.rs`・`hooks.rs`・`length.rs`・`lib.rs`・`operands.rs`・`opt_in.rs`・`report.rs`・`route.rs`・`select.rs` | `ls crates/exec/src` |
| 通常の依存 | `fandhe-silicon-core` だけ | `cargo tree -p fandhe-silicon-exec -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/exec/src/lib.rs` |

## 入口の最初の長さの検査（1/4）

### 所在

- 検査の関数: `crates/exec/src/length.rs` の `check_lengths`（`pub(crate)`。公開の面には無い）
- 4 つの入口（`f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add`）の最初の処理からの呼び出し: `grep -n 'check_lengths' crates/exec/src/entry.rs`
- 上限は呼び出しごとに device の `limits().max_buffer_bytes` を読む。device の形（`native-blocking-wait`）も同じ検査を通り、buffer の `size()` は読まない

### 判定の順

どの拒否も `Err(ContractError::InvalidArgument(..))`。

| 段 | 条件 | 返り値 |
| -- | ---- | ------ |
| 0 | `m` が `AtomicAdd` で `None`、または他の操作で `Some` | `Err` |
| 1 | `n` が 0 | `Err` |
| 2 | `AtomicAdd` で `m` が 0 | `Err` |
| 3 | 必要バイト数の `checked_mul` が `u64` であふれる | `Err` |
| 4 | 入力・出力のどちらかの buffer のバイト数が `max_buffer_bytes` を超える（ちょうどは通る。合計ではなく buffer ごと） | `Err`（core の `validate_allocation_size`。`docs/buffer-range-check.md` 参照） |
| 5 | 上のどれにも当たらない | `Ok(())` |

### 操作ごとのバイト数

| 操作 | 入力 | 出力 |
| ---- | ---- | ---- |
| `F16Fma` | n × 2 | n × 2 |
| `SubgroupSumU32`・`SubgroupSumF32` | n × 4 | ⌈n ÷ 256⌉ × 4 |
| `AtomicAdd` | n × 4 | m × 4 |

### 公開の面

`make exec-surface-check` の `record:` の値は `other_fns=5 unlisted_fns=0`（関数が公開の面に増えていない）。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `length::tests::u64_max_length_is_rejected` | 4 操作で n（`AtomicAdd` は m も）が `u64::MAX`、上限 `u64::MAX` と `1 << 28` | すべて `Err` |
| `length::tests::smallest_overflowing_length_is_rejected` | あふれる最小の長さと一つ手前 | 前者は `Err`、後者は `Ok` |
| `length::tests::smallest_length_over_limit_is_rejected` | 上限 64〜67 での上限を超える最小の長さと一つ手前 | 前者は `Err`、後者は `Ok` |
| `length::tests::zero_length_and_zero_outputs_are_rejected` | n = 0、m = 0、`m` の `Some`/`None` の取り違え | すべて `Err` |
| `entry::tests::entries_reject_lengths` | 試験用 device（上限 `1 << 28`）で 4 つの入口に n = 0、m = 0、`u64::MAX`、上限を超える最小の長さ | すべて `Err(InvalidArgument)` |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --lib length
cargo test -p fandhe-silicon-exec --all-features --locked --lib entry::tests::entries_reject_lengths
make exec-surface-check
```

## 長さ 0 と出力の数 0 の入口経由の試験（2/4）

### 所在

`crates/exec/tests/length_zero.rs`。公開のパスと `crates/exec/tests/common/mod.rs` の `Scripted` だけを使う。

### 組

| 軸 | 値 |
| -- | -- |
| opt-in の上書き | `None`・`NONE`・`G_ONLY`・`C_ONLY`・`BOTH` |
| 既定の印（`Exec::new` に渡す） | `NONE`・`G_ONLY`・`C_ONLY`・`BOTH` |
| device の答え | `Scripted::answering` の `S2`・`Unsupported`、`Scripted::query_fails()` |

呼び出しごとに新しい `Scripted` を作る。4 つの入口すべてに全組を流す。

### 数えるもの

- `Scripted` の `query_ex_calls()`・`memory_calls()`・`queue_calls()`
- `route_evidence::SENTINEL_BYTE` で埋めた出力の列（`is_untouched`）

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `zero_length_is_rejected_on_every_entry` | 4 入口に `n = 0`（`atomic_add` は `m = 1`）、入力・出力とも空 | `Err(InvalidArgument)`、3 つの数えがすべて 0 |
| `zero_length_with_sentinel_outputs_is_untouched` | `n = 0`、出力は長さ 2 の番兵 | 同じ `Err`、出力が番兵のまま |
| `zero_outputs_is_rejected_for_atomic_add` | `atomic_add` に `n = 8`・`m = 0`、出力は空と長さ 2 の番兵 | `Err(InvalidArgument)`、3 つの数えがすべて 0、番兵のまま |
| `valid_length_reaches_route_choice` | `n = 2`（`atomic_add` は `m = 1`）、長さ 2 の入力と出力 | `InvalidArgument` でない、`query_ex` が 1 回 |
| `query_route_is_unaffected` | 拒否の呼び出しをした `Exec` と、していない新しい `Exec` | `query_route(op, over).ok()` が等しい |

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --test length_zero
```

## 上限を差し替えた device とバイト数のあふれの入口経由の試験（3/4）

### 所在

- `crates/exec/tests/length_limit.rs`（host の形）
- `crates/exec/tests/length_device.rs`（device の形。feature `native-blocking-wait` のビルドだけ。`Cargo.toml` の `[[test]]`）
- 上限を差し替える口: `crates/exec/tests/common/mod.rs` の `Scripted::with_max_buffer_bytes`。core の `FaultInjecting::with_limits_override` で上限の表の `max_buffer_bytes` だけを置き換え、`limits()` に効く。ほかの口・数えは変えない

### 組

| 軸 | 値 |
| -- | -- |
| opt-in の上書き | `None`・`NONE`・`G_ONLY`・`C_ONLY`・`BOTH` |
| 既定の印（`Exec::new` に渡す） | `NONE`・`G_ONLY`・`C_ONLY`・`BOTH` |
| device の答え | `Scripted::answering` の `S2`・`Unsupported` |

呼び出しごとに新しい `Scripted` を作る。入力の列は長さ 1 で、長さ n の列は作らない。

### 試験（`length_limit.rs`）

| 試験 | 上限 | n・m | 期待 |
| ---- | ---- | ---- | ---- |
| `smallest_length_over_limit_is_rejected_through_entry` | 64 | `f16_fma` は n = 33、`subgroup_sum_u32`・`subgroup_sum_f32` は n = 17、`atomic_add` は n = 17（m = 1）と m = 17（n = 1） | `Err(InvalidArgument)`、3 つの数えが 0、出力が番兵のまま |
| `largest_length_within_limit_reaches_route_choice` | 64 | `f16_fma` は n = 32、ほかは n = 16、`atomic_add` は m = 16（n = 1） | `InvalidArgument` でない、`query_ex` が 1 回 |
| `byte_count_overflow_is_rejected_through_entry` | `u64::MAX` | `f16_fma` は n = `u64::MAX` と `u64::MAX / 2 + 1`、ほかは n = `u64::MAX / 4 + 1`、`atomic_add` は m = `u64::MAX / 4 + 1`（n = 1）。一つ手前（`u64::MAX / 2`・`u64::MAX / 4`）は拒否でない | 拒否の側は `Err(InvalidArgument)`、3 つの数えが 0、番兵のまま |
| `rejected_call_does_not_block_next_call` | 64 | 同じ `Exec` で拒否の長さの後に通る長さ | 1 回目は拒否、2 回目は拒否でなく、`query_ex` が 1 回 |

### 試験（`length_device.rs`）

device の形の buffer（16 バイト）を渡し、上限 `1 << 28` で n = 0 と `atomic_add` の m = 0、上限 64 で `f16_fma` は n = 33・ほかは n = 17、上限 `u64::MAX` で n = `u64::MAX` を呼ぶ。期待は `Err(InvalidArgument)`、能力の問合せが 0 回、buffer の中身が埋めた値のまま。`memory_calls()` は確保・読み出しでも増えるので、入口の呼び出しの前後で比べる。

### 確かめ方

```text
cargo test -p fandhe-silicon-exec --all-features --locked --test length_limit --test length_device
```

## 完了の条件の確かめ（4/4）

4/4（#6022）で、下-2 の入口の長さの検査について本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-924（ポインタのみ）。GPU の実機の無い Linux で実行した。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 4 つの入口の最初に検査がある | `grep -n 'check_lengths\|self.run(OpId' crates/exec/src/entry.rs` | 10 行に `use crate::length::check_lengths;`。`f16_fma` は 189 行に `check_lengths(OpId::F16Fma, …)`、191 行に `self.run(OpId::F16Fma, …)`。`subgroup_sum_u32` は 203 行に検査、210 行に `self.run`。`subgroup_sum_f32` は 222 行に検査、229 行に `self.run`。`atomic_add` は 242 行に検査、249 行に `self.run`。4 つとも検査の行が `self.run` の行より前 |
| (2) | 純粋関数の直接の試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --lib length`、`cargo test -p fandhe-silicon-exec --all-features --locked --lib length::tests` | 前者は 5 passed; 0 failed; 0 ignored（49 件中、44 filtered out）。`length::tests::*` の 4 本（`smallest_overflowing_length_is_rejected`・`smallest_length_over_limit_is_rejected`・`zero_length_and_zero_outputs_are_rejected`・`u64_max_length_is_rejected`）と `entry::tests::entries_reject_lengths`。後者は 4 passed; 0 failed; 0 ignored（45 filtered out） |
| (3) | 長さ 0 と出力の数 0 の入口経由の試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --test length_zero` | `Running tests/length_zero.rs` の `test result: ok. 5 passed; 0 failed; 0 ignored` |
| (4) | 上限超過とあふれの入口経由の試験 | `cargo test -p fandhe-silicon-exec --all-features --locked --test length_limit --test length_device` | `Running tests/length_device.rs` の `test result: ok. 1 passed; 0 failed; 0 ignored`、`Running tests/length_limit.rs` の `test result: ok. 4 passed; 0 failed; 0 ignored` |
| (5) | 既存の試験とビルドの組 | `make verify`、`make build-profiles`、`make wasm-verify` | 3 つとも終了コード 0。`make verify` の exec は lib が 49 passed、`tests/length_device.rs` が 1 passed、`tests/length_limit.rs` が 4 passed、`tests/length_zero.rs` が 5 passed で、すべて 0 failed。`make build-profiles` の exec は feature なし・`native-blocking-wait`・`fault-injection` の 3 組で `clippy`・`test`・wasm32 の `cargo check` が通る。`make wasm-verify` は `cargo check (fandhe-silicon-exec tests, wasm32-unknown-unknown, …)` を default features・`native-blocking-wait`・`fault-injection` の 3 通りで通る |
| (6) | 公開の面が変わらない | `make exec-surface-check` | 終了コード 0。下の `record:` と `ok:` の行 |
| (7) | 依存と unsafe を足していない | 1/4〜3/4 のマージコミット（#7960 は `152d7e8`、#7961 は `f81f20c`、#7962 は `bdbcf05`）それぞれで `git diff --stat <sha>^..<sha> -- Cargo.lock deps-approved.toml deny.toml crates/core crates/upper` と `git diff <sha>^..<sha> -- crates/exec/Cargo.toml`、`grep -n forbid crates/exec/src/lib.rs` | 3 つとも 1 つ目の出力は空。`crates/exec/Cargo.toml` の差分は #7960・#7961 が空、#7962 が `[[test]] name = "length_device"`（`required-features = ["native-blocking-wait"]`）の 4 行の追加だけ。`crates/exec/src/lib.rs` の 19 行に `#![forbid(unsafe_code)]` |
| (8) | CI で流れる場所 | main の push の CI の run 41763850546（`conclusion=success`、`headSha=bdbcf0579ef4d2c1af7aafe135945f47c2e067fd`）の job `make verify (fmt + clippy + test)`（job id 126357499200）の `make verify` のステップのログ | 下のログの行。`length_zero`・`length_limit`・`length_device` の 3 つが出る |

(6) の出力。

```text
record: commit=bdbcf0579ef4d2c1af7aafe135945f47c2e067fd rs_files=20 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

(8) のログ（ジョブ名・ステップ名・時刻を除いて `Running` の行と `test result:` の行を組にした）。

```text
Running tests/length_device.rs (target/debug/deps/length_device-716b5f7cf8ef7941)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/length_limit.rs (target/debug/deps/length_limit-238bd28b73303b8b)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/length_zero.rs (target/debug/deps/length_zero-d547c2c9f17e8b09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

確かめた時点は、main のコミット `bdbcf05` と、`docs/spec` の gitlink `8e2bc9c0d89ce5c106b6d722cfd630cb393ebf45`。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-924）。

### 確かめ方

```text
grep -n 'check_lengths\|self.run(OpId' crates/exec/src/entry.rs
cargo test -p fandhe-silicon-exec --all-features --locked --lib length
cargo test -p fandhe-silicon-exec --all-features --locked --test length_zero
cargo test -p fandhe-silicon-exec --all-features --locked --test length_limit --test length_device
make verify
make build-profiles
make wasm-verify
make exec-surface-check
grep -n forbid crates/exec/src/lib.rs
```

## 段の表

| 段 | Issue・PR | 節 |
| -- | --------- | -- |
| 1/4 | #6019（PR #7960） | 入口の最初の長さの検査（1/4） |
| 2/4 | #6020（PR #7961） | 長さ 0 と出力の数 0 の入口経由の試験（2/4） |
| 3/4 | #6021（PR #7962） | 上限を差し替えた device とバイト数のあふれの入口経由の試験（3/4） |
| 4/4 | #6022 | 完了の条件の確かめ（4/4） |

親 Issue は #868。
