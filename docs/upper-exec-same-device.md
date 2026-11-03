# 上の層と下-2 を 1 つの device で通す試験の所在（TASK-261）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-261。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験 | `crates/upper/tests/upper_exec_same_device.rs` |
| feature の条件 | `crates/upper/Cargo.toml` の `[[test]]` の `required-features = ["native-blocking-wait"]` |
| 使う公開のパス | `fandhe_silicon_exec::{Exec, OptIn, Outcome, OpId, F16FmaOperands}`（`Exec` の所在は TASK-919）、`fandhe_silicon_upper::{UpperDevice, Buffer, SubmitId}`（TASK-2386）、`fandhe_silicon_core::device::Device`、`fandhe_silicon_core::memory::BufferUsage`、`fandhe_silicon_core::feature_key::FEATURE_SHADER_F16` |
| 試験だけの型 | `Recorder`（`crates/upper/tests/common/mod.rs`。TASK-2386）。`StubDevice`（TASK-156）を包み、記録の列に `QueryEx`（`query_ex` の鍵）が加わった |
| 関連 | 層をまたぐ型の受け渡しの試験は `docs/cross-layer-types.md`（TASK-260） |

## 上の層と下-2 を 1 つの device で通す試験（1/3）

1 つの `Recorder` を `&dyn Device` で `UpperDevice` と `Exec` の両方に渡す。
buffer は 16 バイト・用途 `STORAGE` で、`Buffer::from_handle` で包み、`handle_mut()` の値を下-2 の device の形に渡す。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `upper_and_exec_calls_reach_one_device` | 既定が「許さない」の `Exec`、上書き `None`。記録先を作り、`query_route(OpId::F16Fma, None)`、`f16_fma`（`n` は 8）、pass の開始と終了、`finish`、`submit`、`wait_blocking(id, None)`、`gpu_timing` の順に呼ぶ | `f16_fma` が `Ok(Outcome::Unavailable { report })` で `report` は `query_route` の報告と等しい。`submit` は `Ok`、`wait_blocking` は `Ok(())`。buffer の中身は埋めた値のまま。`Recorder` の記録は列 1 のとおり（長さ 6）で、`Poll`・`SubmitMany`・`OnComplete`・`DispatchEx` は無い |
| `exec_failure_does_not_change_upper_submit` | 上書き `Some(OptIn { allow_gpu_emulated: false, allow_cpu: true })`、`query_route` を呼ばずに同じ並び | `f16_fma` の戻りが `Ok(Outcome::Unavailable { .. })` ではない。`submit` は `Ok`、`wait_blocking` は `Ok(())`。各 buffer は大きさ 16・用途 `STORAGE`、読み出しの長さは 16。`Recorder` の記録は列 2 のとおり（長さ 5）で、`Poll`・`SubmitMany`・`OnComplete`・`DispatchEx` は無い |

記録の列（`t` は `Submit` が返した印）。

```text
1. CreateEncoder, QueryEx(FEATURE_SHADER_F16), QueryEx(FEATURE_SHADER_F16), Submit(wait_len 0, t), WaitBlocking(t, None), GpuTiming(t)
2. CreateEncoder, QueryEx(FEATURE_SHADER_F16), Submit(wait_len 0, t), WaitBlocking(t, None), GpuTiming(t)
```

### 確かめ方

```text
cargo test -p fandhe-silicon-upper --features native-blocking-wait --test upper_exec_same_device
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
```

## 上の層の検査を新しい試験を含む構成に当てた記録（2/3）

origin/main のコミット `b74ecdb`、`docs/spec` の gitlink は `c7887a1` の時点で、作業ツリーが clean の状態から流した結果。
当てた検査は [upper-layer-check](upper-layer-check.md) の `make upper-layer-check` と `make upper-source-check`、および [upper-layer-crate-check](upper-layer-crate-check.md) の表のコマンド。

### make upper-layer-check の record: の欄

終了コードは 0。

| 欄 | 値 | 方法 |
| -- | -- | ---- |
| `rs_files` | 10 | `make upper-layer-check` |
| `direct_deps` | 3 | 同上 |
| `forbid` | yes | 同上 |
| `unsafe_total` | 0 | 同上 |
| `chip_deps` | 0 | 同上 |
| `chip_paths` | 0 | 同上 |
| `other_deps` | 0 | 同上 |
| `measure_pkgs` | 2 | 同上 |
| `measure_contract_pkgs` | 0 | 同上 |
| `measure_forbidden` | 0 | 同上 |

### make upper-source-check の record: の欄

終了コードは 0。

| 欄 | 値 | 方法 |
| -- | -- | ---- |
| `commit` | `b74ecdbc6f9f` | `make upper-source-check` |
| `rs_files` | 5 | 同上 |
| `build_rs` | no | 同上 |
| `global_state` | 0 | 同上 |
| `env_fs` | 0 | 同上 |
| `table_fns` | 4 | 同上 |
| `contract_ops` | 12 | 同上 |
| `contract_calls` | 2 | 同上 |
| `contract_call_mismatch` | 0 | 同上 |
| `dispatch_ex` | 0 | 同上 |
| `failure_constructs` | 0 | 同上 |

### 直接依存と上の層へ向く依存

| 項目 | 観測 | 方法 |
| ---- | ---- | ---- |
| 通常・build の直接依存 | `fandhe-silicon-core`、`fandhe-silicon-exec` | コマンド 1 |
| `[dev-dependencies]` の直接依存 | `fandhe-silicon-core` | コマンド 1 |
| 上の層へ向く依存の木の行数 | 1（上の層自身の行だけ） | コマンド 2 |

### 単独のビルドと試験

| 項目 | 結果 | 方法 |
| ---- | ---- | ---- |
| 単独のビルド | 終了コード 0 | `cargo build --locked --offline -p fandhe-silicon-upper` |
| `native-blocking-wait` つきのビルド | 終了コード 0 | `cargo build --locked --offline -p fandhe-silicon-upper --features native-blocking-wait` |
| 新しい試験 | 2 passed、0 failed | `cargo test --locked --offline -p fandhe-silicon-upper --features native-blocking-wait --test upper_exec_same_device` |

### 契約の crate・ファイルの指定・試験の中の名前

| 項目 | 観測 | 方法 |
| ---- | ---- | ---- |
| 全 feature の木（dev を含む）の `fandhe-silicon-contract` の行 | 0 | コマンド 3 |
| `crates/upper` の `#[path`・`include!`・`include_str!`・`include_bytes!` の行 | 0 | コマンド 4 |
| 新しい試験に現れる `fandhe_silicon_` の名前 | `fandhe_silicon_core`、`fandhe_silicon_exec`、`fandhe_silicon_upper` | コマンド 5 |
| 新しい試験の中の故障の差し込み・非対応の強制・非正規化の注入・契約の非公開の項目の名前の行 | 0 | コマンド 6 |

### feature の木

`test-support`・`fault-injection` の行数。

| 辺 | feature | 行数 | 方法 |
| -- | ------- | ---- | ---- |
| normal, build | 指定なし | 0 | コマンド 7 |
| normal, build | `--all-features` | 0 | コマンド 8 |
| normal, build, dev | 指定なし | 1 | コマンド 9 |
| normal, build, dev | `--all-features` | 1 | コマンド 10 |

- dev の辺で当たる 1 行は、`[dev-dependencies]` の下の `fandhe-silicon-core feature "test-support"`。
- `fandhe-silicon-exec` の feature の行は、指定なしも `--all-features` も `fandhe-silicon-exec feature "default"` の 1 種類だけで、`fault-injection` の行は 0（コマンド 11）。

### 測定の workspace の feature の木

| 項目 | 観測 | 方法 |
| ---- | ---- | ---- |
| normal, build の `test-support`・`fault-injection` の行 | 0 | コマンド 12 |
| normal, build, dev の同じ行 | 1（`fandhe-silicon-core feature "test-support"`） | コマンド 13 |
| `measure/Cargo.lock` の `fandhe-silicon-contract` の package | 0 | コマンド 14 |
| `measure_pkgs`・`measure_contract_pkgs`・`measure_forbidden` | 2・0・0 | `make upper-layer-check` の record: |

### 新しい試験が流れる場所

| make のターゲット | 観測した行 | 終了コード |
| ----------------- | ---------- | ---------- |
| `make verify` | `Running tests/upper_exec_same_device.rs` の直後に 2 tests、`test result: ok. 2 passed; 0 failed` | 0 |
| `make build-profiles` | `cargo test -p fandhe-silicon-upper --features native-blocking-wait` の実行に `Running tests/upper_exec_same_device.rs`（2 passed、0 failed）。feature なしの `cargo test -p fandhe-silicon-upper` の実行にはこの行は出ない | 0 |
| `make wasm-verify` | `cargo check (fandhe-silicon-upper tests, wasm32-unknown-unknown, default features)` と `cargo check (fandhe-silicon-upper tests, wasm32-unknown-unknown, native-blocking-wait)` の 2 つの log の行 | 0 |

### コマンド

```text
# 1: cargo tree --locked --offline --target all -p fandhe-silicon-upper --depth 1 -e normal,build,dev
# 2: cargo tree --locked --offline --target all --workspace -i fandhe-silicon-upper -e normal,build,dev | wc -l
# 3: cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e normal,build,dev --prefix none | grep -c fandhe-silicon-contract
# 4: grep -rnE '#\[path|include!|include_str!|include_bytes!' crates/upper | wc -l
# 5: grep -ohE 'fandhe_silicon_[a-z_]+' crates/upper/tests/upper_exec_same_device.rs | sort -u
# 6: grep -cE 'fault_injection|force_unsupported|inject_denormal|__backend' crates/upper/tests/upper_exec_same_device.rs
# 7: cargo tree --locked --offline --target all -p fandhe-silicon-upper -e features,normal,build | grep -cE 'test-support|fault-injection'
# 8: cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e features,normal,build | grep -cE 'test-support|fault-injection'
# 9: cargo tree --locked --offline --target all -p fandhe-silicon-upper -e features,normal,build,dev | grep -cE 'test-support|fault-injection'
# 10: cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e features,normal,build,dev | grep -cE 'test-support|fault-injection'
# 11: cargo tree --locked --offline --target all -p fandhe-silicon-upper --all-features -e features,normal,build,dev | grep 'fandhe-silicon-exec feature' | grep -c fault-injection
# 12: cargo tree --locked --offline --target all --manifest-path measure/Cargo.toml --workspace --all-features -e features,normal,build | grep -cE 'test-support|fault-injection'
# 13: cargo tree --locked --offline --target all --manifest-path measure/Cargo.toml --workspace --all-features -e features,normal,build,dev | grep -cE 'test-support|fault-injection'
# 14: grep -c 'name = "fandhe-silicon-contract"' measure/Cargo.lock
```

### 再実行の契機

次のときは上のコマンドを流し直し、記録時点の表は書き換えずに表の後に補いを置く。

- `crates/upper` の `Cargo.toml`・`src/`・`tests/` が変わったとき
- `crates/exec/Cargo.toml` の `[features]` が変わったとき
- `scripts/upper-layer-check.sh` の数え方の定義が変わったとき
