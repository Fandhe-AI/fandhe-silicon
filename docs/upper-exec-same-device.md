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

## 完了の条件の確かめ（3/3）

origin/main のコミット `7dcbd25`、`docs/spec` の gitlink は `c7887a1` の時点で、1/3・2/3 の節を入れた後の作業ツリー（clean）から、GPU の実機の無い Linux で観測し直した結果。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の所在と feature | 3/3 のコマンド 1・2 | `crates/upper/Cargo.toml` の 36・37 行目に `name = "upper_exec_same_device"` と `required-features = ["native-blocking-wait"]`。feature なしの実行に `Running tests/upper_exec_same_device.rs` は出ない（ほかの試験は 7 passed・3 passed・7 passed、いずれも 0 failed） |
| (2) | 上の層と下-2 の呼び出しが 1 つの device の列に届く | 3/3 のコマンド 3 | `upper_and_exec_calls_reach_one_device ... ok`、`exec_failure_does_not_change_upper_submit ... ok`、`2 passed; 0 failed; 0 ignored` |
| (3) | 下-2 の入口の後も上の層の投入と待ちが通り、buffer の中身が変わらない | 3/3 のコマンド 3 | `upper_and_exec_calls_reach_one_device ... ok` |
| (4) | `unsafe`・チップの crate・直接依存 | `make upper-layer-check`、2/3 のコマンド 1・2 | 終了コード 0。`record:` は `rs_files=10 direct_deps=3 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0`、`ok:` の行あり。直接依存は通常・build が `fandhe-silicon-core`・`fandhe-silicon-exec`、dev が `fandhe-silicon-core`。上の層へ向く依存の木の行数は 1 |
| (5) | 契約の crate・ほかの crate のファイルの指定・試験の中の名前 | 単独のビルド 2 つ、3/3 のコマンド 3、2/3 のコマンド 3〜6 | feature なしのビルドの終了コード 0、`--features native-blocking-wait` のビルドの終了コード 0、試験は 2 passed・0 failed。`fandhe-silicon-contract` の行 0、`#[path`・`include!` 系の行 0、試験に現れる名前は `fandhe_silicon_core`・`fandhe_silicon_exec`・`fandhe_silicon_upper`、故障の差し込み・非対応の強制・非正規化の注入・`__backend` の行 0 |
| (6) | feature の木 | 2/3 のコマンド 7〜14、`make upper-layer-check` の `record:` | コマンド 7〜14 の行数は 0・0・1・1・0・0・1・0。dev の辺で当たる 1 行は `fandhe-silicon-core feature "test-support"`。`measure_pkgs=2 measure_contract_pkgs=0 measure_forbidden=0` |
| (7) | CI で流れる場所 | `make verify`、`make build-profiles`、`make wasm-verify`、3/3 のコマンド 4・5 | `make verify` の終了コード 0。`Running tests/upper_exec_same_device.rs` の後に `running 2 tests`、`test result: ok. 2 passed; 0 failed; 0 ignored`。`make build-profiles` の終了コード 0。`native-blocking-wait` つきの `cargo test -p fandhe-silicon-upper` の実行にだけ同じ行が 1 回出る。`make wasm-verify` の終了コード 0。`cargo check (fandhe-silicon-upper tests, wasm32-unknown-unknown, default features)` と `... native-blocking-wait)` の 2 つの log の行あり。`.github/workflows/ci.yml` の 269 行目に `run: make upper-layer-check` |
| (8) | 依存 0・unsafe 0・ソースの変更 0（1/3 のコミット `b74ecdb` の差分） | 3/3 のコマンド 6〜8 | `Cargo.lock`・`deps-approved.toml`・ルートの `Cargo.toml`・`crates/*/src`・`crates/exec`・`crates/core` の差分は 0 行。`crates/upper/Cargo.toml` の追加は空行・コメント 1 行・`[[test]]`・`name`・`required-features` の 5 行。`crates/upper/tests/common/mod.rs` の追加は `enum Call` の doc コメント 1 行と `QueryEx { feature: FeatureId },` の 1 行、`query_ex` の中の `self.record(Call::QueryEx { feature });` の 1 行の計 3 行 |

そのほかの make ターゲットの観測。

| make のターゲット | 観測 | 終了コード |
| ----------------- | ---- | ---------- |
| `make upper-source-check` | `record:` は `rs_files=5 build_rs=no global_state=0 env_fs=0 table_fns=4 contract_ops=12 contract_calls=2 contract_call_mismatch=0 dispatch_ex=0 failure_constructs=0`、`ok: upper source scan passed` | 0 |
| `make exec-deps-check` | `record:` は `rs_files=15 declared_deps=2 other_deps=0 dep_source=0 tree_pkgs=1 tree_direct=1 tree_forbidden=0 source_hits=0`、`ok:` の行あり | 0 |
| `make deps-check` | `ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml` | 0 |
| `make verify` | 全体の `test result:` の行は 223、passed の合計 2258、failed の合計 0、ignored の合計 7 | 0 |

### 3/3 のコマンド

```text
# 1: grep -n -A2 'upper_exec_same_device' crates/upper/Cargo.toml
# 2: cargo test --locked --offline -p fandhe-silicon-upper
# 3: cargo test --locked --offline -p fandhe-silicon-upper --features native-blocking-wait --test upper_exec_same_device
# 4: grep -n 'run: make upper-layer-check' .github/workflows/ci.yml
# 5: make verify の出力の行を数える。grep -c 'test result:' と、各行の passed・failed・ignored の合計
# 6: git diff --stat b74ecdb~1 b74ecdb -- Cargo.lock deps-approved.toml Cargo.toml 'crates/*/src' crates/exec crates/core
# 7: git diff b74ecdb~1 b74ecdb -- crates/upper/Cargo.toml
# 8: git diff b74ecdb~1 b74ecdb -- crates/upper/tests/common/mod.rs
```

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-261）。

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/3 | #4000 | test(upper): TASK-261 の 1/3 上の層と下-2 を 1 つの device で通す試験 |
| 2/3 | #4001 | docs: TASK-261 の 2/3 上の層の検査を新しい試験を含む構成に当てた記録 |
| 3/3 | #4002 | docs: TASK-261 の 3/3 完了の条件の確かめ |

## 今の値の補い（base c956c96）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 所在の「使う公開のパス」 | 表に載る公開のパスに `ContractError` と `BackendKind` は無い → 試験の `use` に `fandhe_silicon_core::error::ContractError`（11 行目）と `fandhe_silicon_core::capability::BackendKind`（9 行目）が加わっている。`BackendKind` は `Recorder` の `backend()` の値（`Metal`）を照らすのに使う | `sed -n 8,15p crates/upper/tests/upper_exec_same_device.rs` |
| 上の層と下-2 を 1 つの device で通す試験（1/3）の `exec_failure_does_not_change_upper_submit` の期待 | `f16_fma` の戻りが `Ok(Outcome::Unavailable { .. })` ではない → `f16_fma` の戻りが `Err(ContractError::Unsupported { op: "f16_fma", backend: BackendKind::Metal, .. })` に当たる（145〜149 行目）。`backend` の `Metal` は `Recorder` の `backend()` が返す値。`submit`・`wait_blocking`・buffer の大きさ・用途・読み出しの長さの期待は同じ | `grep -n -A5 'Err(ContractError' crates/upper/tests/upper_exec_same_device.rs`、`cargo test -p fandhe-silicon-upper --features native-blocking-wait --test upper_exec_same_device`（2 passed; 0 failed; 0 ignored） |

## 今の値の補い（base f4b9d8f）

上の表の (7) の `.github/workflows/ci.yml` の行番号は、今は `run: make upper-layer-check` が 270 行目（`grep -n 'run: make upper-layer-check' .github/workflows/ci.yml` の出力）。

## 今の値の補い（base 904126a）

記録時点の表と既存の節は書き換えない。以下は base `904126a` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| そのほかの make ターゲットの `make exec-deps-check` の `record:` | `rs_files=15` → `rs_files=20`（`declared_deps=2 other_deps=0 dep_source=0 tree_pkgs=1 tree_direct=1 tree_forbidden=0 source_hits=0` のまま。終了コード 0） | `make exec-deps-check` |
| 同 `make verify` | `test result:` の行は 223、passed の合計 2258 → 行は 228、passed の合計 2302（failed の合計 0、ignored の合計 7 のまま。終了コード 0） | `make verify` の出力の `test result:` の行を数え、passed・failed・ignored を足す |
| (4) の `make upper-layer-check` の `record:` | `rs_files=10 direct_deps=3 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0` のまま | `make upper-layer-check \| grep '^record:'` |
| 同 `make upper-source-check` の `record:` | `rs_files=5` のまま | `make upper-source-check \| grep '^record:'` |
