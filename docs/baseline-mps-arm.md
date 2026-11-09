# 比較基準の MPS の腕（TASK-1011）の所在と本リポの現状

比較基準の別 workspace `baseline-wgpu/` にある、MPS の腕の lib crate の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1011。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `baseline-wgpu/crates/mps`。package は `baseline-wgpu-mps`、lib は `baseline_wgpu_mps` | `baseline-wgpu/crates/mps/Cargo.toml` |
| 依存 | 製品の `crates/core`（feature `test-support`）への path だけ。外部依存と `[dev-dependencies]` は無い | `baseline-wgpu/crates/mps/Cargo.toml`、`make deny` |
| lints | workspace の表を継ぐ（`unsafe_code = "forbid"`） | `baseline-wgpu/crates/mps/Cargo.toml` |
| 流れる場所 | `make baseline-verify`（fmt --check / clippy `-D warnings` / test）に含まれる。device は使わない | `scripts/baseline-verify.sh` |
| 門の run の枠 | `fandhe_silicon_core::parity::gemm_gate`（TASK-1005 の所在は [gemm-test-inputs](gemm-test-inputs.md)） | 同左 |
| 別 workspace の置き場 | TASK-7 の所在は [baseline-wgpu](baseline-wgpu.md) | 同左 |

## セルの計画と入力の詰め替え（1/3）

どの OS でもコンパイルされる純粋な関数だけを `src/lib.rs` に置く。

### `plan(cell, output)`

| セルの腕 | セルの dtype | `OutputKind` | 返し |
| -------- | ------------ | ------------ | ---- |
| `Mps` 以外 | 任意 | 任意 | `PlanError::NotMpsCell` |
| `Mps` | `F32` | `SameAsInput` | 入力 `F32`・出力 `F32` |
| `Mps` | `F32` | `F32` | `PlanError::OutputNotForCell` |
| `Mps` | `F16Acc` | `SameAsInput` | 入力 `F16`・出力 `F16` |
| `Mps` | `F16Acc` | `F32` | 入力 `F16`・出力 `F32` |

返す `MpsPlan` の `m`・`n`・`k` はセルの行の値。

### `check_row_bytes(columns, element, row_bytes, rows)`

- `row_bytes` が `columns × size` より小さいとき `PlanError::RowBytesTooSmall`、`size` の倍数でないとき `PlanError::RowBytesNotAligned`、計算があふれるとき `PlanError::SizeOverflow`
- 満たすとき `rows × row_bytes`（バッファの長さ）を返す。0 列は `row_bytes` が `size` の倍数なら受け付ける

### 詰め替え

| 関数 | 返し |
| ---- | ---- |
| `pack_f32` / `pack_f16` | 行優先の密な列を、行の間隔つきの little-endian のバイト列にする。行の末尾の余りは 0。列の長さが `rows × columns` と違うとき `PlanError::LengthMismatch` |
| `unpack_f32` / `unpack_f16` | 逆向き。各行の先頭の `columns × size` バイトだけを読み、余りは読まない。バイト列の長さが `check_row_bytes` の値と違うとき `PlanError::LengthMismatch` |

行の間隔の不備とあふれは `check_row_bytes` と同じ `Err`。確保の失敗は `PlanError::AllocationFailed`。`PlanError` は `#[non_exhaustive]` で、`Display` の文言は固定の英語。

### 試験

| ファイル | 確かめる事項 |
| -------- | ------------ |
| `tests/plan.rs` | `Mps` の全セルの計画（大きさと要素の型）、`F16Acc` のセルの f32 の出力、`F32` のセルの `OutputNotForCell`、`Mps` 以外の腕の全セルの `NotMpsCell`、要素のバイト数、`check_row_bytes` の境界、詰めて戻すと元に戻ること（行の間隔が最小のときと余りがあるとき）、余りが 0 で埋まること、余りを読まないこと、長さの不一致と行の間隔の不備が `Err` |

### 確かめ方

worktree のルートで実行する。

- `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test plan`
- `make baseline-verify`

## MPS の呼び出しと device の要る試験（2/3）

上の「本リポの現状」の表の依存・lints の行は 1/3 の時点の値。

### 依存と lints

- `baseline-wgpu/crates/mps/Cargo.toml` の `[target.'cfg(target_os = "macos")'.dependencies]` に、`objc2`（`=0.6.5`）・`objc2-metal`（`=0.3.2`。`default-features = false`、feature は `std`・`MTLAllocation`・`MTLBuffer`・`MTLCommandBuffer`・`MTLCommandQueue`・`MTLDevice`・`MTLResource`）・`objc2-core-graphics`（`=0.3.2`。`default-features = false`）を置く。記録は `baseline-wgpu/deps-approved.toml`
- `[lints]` は workspace の継承をやめて自前の表（`unsafe_code = "deny"`・`unsafe_op_in_unsafe_fn = "deny"`・`missing_docs = "warn"`・`undocumented_unsafe_blocks = "deny"`・`missing_safety_doc = "deny"`）

### `run_matmul(plan, inputs)`

macOS 向けだけの safe な公開関数。`MpsError` は `src/lib.rs` にあり、どの OS でもコンパイルされる（`#[non_exhaustive]`、`Display` は固定の英語）。呼び出しの順と、手順が失敗したときの値は次のとおり。

| 手順 | 失敗したときの `MpsError` |
| ---- | ------------------------ |
| 入力の種類が計画の入力の型と合うか（device には触れない） | `InputMismatch` |
| 既定の Metal の device を得る | `NoDevice` |
| `MPSSupportsMTLDevice` | `Unsupported` |
| `MPSMatrixDescriptor`・`MPSMatrix`・`MPSMatrixMultiplication` のクラスを引く | `Unsupported` |
| 行の間隔を尋ね、`check_row_bytes` で照らす | `Plan` |
| A・B を詰める | `Plan` |
| A・B・C のバッファを作る（長さ 0 を含む） | `AllocationFailed` |
| 記述子・行列・kernel を作る | `Unsupported` |
| コマンドキュー・コマンドバッファを作る | `AllocationFailed` |
| 投入して 1 回だけ完了を待ち、状態が `Completed` か | `Execution` |
| 出力のバッファの長さを確かめ、完了の後だけ読んで戻す | `Execution`・`Plan` |

A は M×K、B は K×N、C は M×N で、転置なし・`alpha = 1`・`beta = 0`。出力の型は計画に従う。

### `unsafe` の所在

- 橋の module は `src/mps.rs`。`#[allow(unsafe_code)]` は `src/lib.rs` の `mod mps;` の宣言だけに付く
- `unsafe` は、`MPSSupportsMTLDevice` の宣言と呼び出し、`msg_send!` の呼び出し、`newBufferWithBytes_length_options`、出力の読み出しの各箇所にあり、どれにも `// SAFETY:` がある。`unsafe fn` は持たない
- 個数の試験は `tests/unsafe_count.rs`。種別ごとの個数を表と照合し、表に無いファイルは全種別 0 を期待する

### device の要る試験

| ファイル | 試験 | 出力の型 | 判定 |
| -------- | ---- | -------- | ---- |
| `tests/mps_gate.rs` | `mps_gate_cell_matches_t1` | 入力と同じ型 | 枠の結果の `comparison().is_match()` の assert |
| `tests/mps_gate_f32_output.rs` | `mps_gate_cell_f32_output_runs` | f32 | 結果の行と `output=f32` の行の出力だけ |

どちらも `#[ignore]` 付きで macOS 向けだけ。セルは門の run の環境変数で選ぶ。device の回では次の形で流す（セルの識別名は `GemmGateCell::id` の形）。

```text
scripts/metal-session.sh run --combination <name> --validation-layer --gate-cell <セルの識別名> -- cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test mps_gate -- --ignored --exact mps_gate_cell_matches_t1
```

f32 の出力の試験は `--test mps_gate_f32_output` と `mps_gate_cell_f32_output_runs` に替える。

### 確かめ方

worktree のルートで実行する。`--ignored` は付けない。

- `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test unsafe_count`
- `cargo clippy --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-mps --all-targets --locked --target aarch64-apple-darwin -- -D warnings`
- `make baseline-verify`

## 完了の条件の確かめ（3/3）

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-1011。親 #926、この節 #6327）。時点のコミットは `d7584fa`。(6) の CI は push の run `42651966699`（event `push`、conclusion `success`、headSha `d7584faa3b2a635d721812c1cd584c2511da5bb5`）。貼った出力の `Running` の行は行頭の空白を除いている。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 純粋な部分の試験 | `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test plan` | `ok. 13 passed; 0 failed; 0 ignored` |
| 2 | unsafe の個数の試験 | `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test unsafe_count` | `ok. 7 passed; 0 failed; 0 ignored` |
| 3 | macOS 向けの型検査 | `cargo clippy --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-mps --all-targets --locked --target aarch64-apple-darwin -- -D warnings` | 終了コード 0 |
| 4 | device の要る試験の宣言 | `grep -n -A1 '#\[ignore = "touches the Metal device' baseline-wgpu/crates/mps/tests/*.rs` | 出力は (4) |
| 5 | 依存 | `git diff --stat 55fd9c1 HEAD -- Cargo.toml Cargo.lock deps-approved.toml`、`make deny`、`make baseline-positive-control`、`grep -c '^source = "registry' baseline-wgpu/Cargo.lock` | diff は空、`make deny` は終了コード 0、`make baseline-positive-control` は終了コード 0、registry の package は `55fd9c1` で 124・`HEAD` で 124 |
| 6 | CI | `gh run view 42651966699 --json conclusion,headSha,jobs`、`deny` ジョブのログの `make baseline-verify` の段 | run `success`、`make deny (cargo-deny)` は `success`、`make verify (fmt + clippy + test)` は `success`、`make baseline-verify` の段は `success`、出力は (6) |

(1) の出力。

```console
$ cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test plan
Running tests/plan.rs (baseline-wgpu/target/debug/deps/plan-3312a56b55171038)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。

```console
$ cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test unsafe_count
Running tests/unsafe_count.rs (baseline-wgpu/target/debug/deps/unsafe_count-7880abf627b47c95)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(4) の出力。

```console
$ grep -n -A1 '#\[ignore = "touches the Metal device' baseline-wgpu/crates/mps/tests/*.rs
baseline-wgpu/crates/mps/tests/mps_gate.rs:29:#[ignore = "touches the Metal device; run only in a dedicated device session"]
baseline-wgpu/crates/mps/tests/mps_gate.rs-30-fn mps_gate_cell_matches_t1() {
baseline-wgpu/crates/mps/tests/mps_gate_f32_output.rs:28:#[ignore = "touches the Metal device; run only in a dedicated device session"]
baseline-wgpu/crates/mps/tests/mps_gate_f32_output.rs-29-fn mps_gate_cell_f32_output_runs() {
```

(6) の出力。`deny` ジョブ（job `129280409401`）のログから `make baseline-verify` の段の行を抜き、`baseline_wgpu_mps` の最初の `Running` の行から `unsafe_count` の `test result:` の行までを貼る。

```console
$ gh run view 42651966699 --job 129280409401 --log | awk -F'\t' '$2=="make baseline-verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //'
Running unittests src/lib.rs (baseline-wgpu/target/debug/deps/baseline_wgpu_mps-69541977f034b15a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/mps_gate.rs (baseline-wgpu/target/debug/deps/mps_gate-bf52fa86b23e385d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/mps_gate_f32_output.rs (baseline-wgpu/target/debug/deps/mps_gate_f32_output-da72d31f457927d9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/plan.rs (baseline-wgpu/target/debug/deps/plan-0e9926fec5228046)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/unsafe_count.rs (baseline-wgpu/target/debug/deps/unsafe_count-c6b4a5b254b31f11)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1011）。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | crate とセルの計画・入力の詰め替え | #6325 |
| 2/3 | MPS の呼び出しと device の要る試験 | #6326 |
| 3/3 | 完了の条件の確かめ | #6327 |
