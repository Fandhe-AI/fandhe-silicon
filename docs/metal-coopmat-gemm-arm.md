# Metal 直接の cooperative matrix の GEMM の腕（TASK-1009）の所在と本リポの現状

TASK-1009 の作業のうち、測定専用の別 workspace `measure/` の腕の crate に置く物の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1009。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `measure/crates/metal-gemm-arm`（crate の所在は [metal-gemm-arm](metal-gemm-arm.md)、TASK-1008） | `measure/Cargo.toml` の `members` |
| MSL のファイル | `measure/crates/metal-gemm-arm/src/shaders/coopmat_gemm_f32.metal` | ファイルの有無 |
| 純粋な部分 | `measure/crates/metal-gemm-arm/src/coopmat.rs`（どの OS でもコンパイルされ、`unsafe` を書かない） | `pub mod coopmat;`（`src/lib.rs`） |
| 門の枠 | `crates/core` の `parity` の module（TASK-1005 のポインタ。所在は [gemm-test-inputs](gemm-test-inputs.md)） | `crates/core/src/parity/` |

## MSL と起動の形（1/4）

### MSL

入口は `coopmat_gemm_f32`（定数 `COOPMAT_ENTRY`）。ソースは `COOPMAT_MSL`（`include_str!`）。

| index | 中身 |
| ----- | ---- |
| buffer 0 | A |
| buffer 1 | B |
| buffer 2 | C |
| buffer 3 | 次元（`m`・`k`・`n` の順の 3 つの u32） |

1 つの threadgroup（32 スレッド = 1 simdgroup）が C の 8×8 を 1 枚書く。

### 起動の形

`coopmat_launch_for(cell)` は、腕が `GemmGateArm::G7` の f32 のセルだけ `Some(CoopmatLaunch)` を返し、ほかは `None`。

- スレッド数は (32, 1, 1)
- 実効の M・N・K は元の値を `launch::pad8_dim`（8 の倍数への切り上げ）に通した値
- threadgroup 数は (実効 N / 8, 実効 M / 8, 1)

### offline compiler のコマンド

`compile_commands(dir)` は `xcrun` に渡す 2 つの引数の列を返す（`xcrun` 自身は含めない。起動しない）。

| 列 | 内容 |
| -- | ---- |
| 1 | `-sdk macosx metal -c -fmetal-math-mode=safe -o <dir>/coopmat_gemm_f32.air <dir>/coopmat_gemm_f32.metal` |
| 2 | `-sdk macosx metallib -o <dir>/coopmat_gemm_f32.metallib <dir>/coopmat_gemm_f32.air` |

### エラー

`ArmError` に `ToolFailed`・`WorkDirFailed`・`LibraryLoadFailed` がある。文言は固定の英語（`offline shader compiler failed`・`work directory operation failed`・`metallib loading failed`）で、パスやツールの出力を含まない。

### 試験

| ファイル | 内容 |
| -------- | ---- |
| `tests/coopmat.rs` | 起動の形が対象のセルだけにあること、実効次元が 8 の倍数で `pad8_dim` と一致すること、threadgroup 数とスレッド数の式、`compile_commands` の 2 つの列の並びと出力先、MSL の入口名と束縛、新しい `ArmError` の文言 |

確かめ方: `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test coopmat`

## metallib の作成と腕の手順（2/4）

macOS 向けに `run_g7`（`src/metal.rs`）が、MSL を metallib にして読み、門の `G7` の f32 のセルを 1 回だけ投入する。成果物（パス・バイト列）は引数に取らない。

### 腕の手順

1. 腕が `G7` でなければ `UnsupportedCell`、起動の形が無ければ `UnsupportedCell`、入力が f32 でなければ `DtypeMismatch`
2. 入力を実効の次元へ詰める（長さの食い違いは `LengthMismatch`、寸法のあふれは `Overflow`）。ここまで offline compiler も device も呼ばない
3. 作業のディレクトリを作って metallib を作る（下記）。ディレクトリは出力を読み終えるまで保持する
4. 既定の device（無ければ `NoDevice`）で metallib の URL から library を読み（`LibraryLoadFailed`）、入口の関数を名前で作り（`FunctionFailed`）、pipeline を作る（`PipelineFailed`）。function constant と `compile_options()` は使わない
5. threadgroup のスレッド数が pipeline の上限を超えれば `ThreadsExceeded`
6. A・B・C のバッファを確保して束縛（0〜2）し、実効の M・K・N をこの並びで buffer 3 に渡し、1 回だけ投入して完了を待つ。完了でなければ `SubmitFailed`
7. 出力を読み、元の M×N に切り出す。事実として `MetallibFacts` の 2 つの値（pipeline のスレッド数の上限と metallib の sha256）を返す

### 作業のディレクトリ

- 一時のディレクトリの下に、一意の名前のディレクトリを非再帰で作る（既にあれば `WorkDirFailed`）
- MSL は新規作成で書く。`/usr/bin/xcrun` を shell を介さず絶対パスで呼び、標準入力は null、出力は捨てる。失敗は `ToolFailed`
- metallib は通常のファイル（シンボリックリンクでない）と確かめ、上限つきで読んで sha256 を取る
- どの出口でも、作ったディレクトリだけを消す

### `unsafe`

`unsafe` を書くのは `src/metal.rs` だけで、`src/metallib.rs` は safe のみ。個数は `tests/unsafe_count.rs` の表が固定する。

### 試験

| ファイル | 内容 |
| -------- | ---- |
| `tests/coopmat_macos.rs` | `G6` のセルが `UnsupportedCell`、binary16 の入力が `DtypeMismatch`、別の行の入力が `LengthMismatch` になること（device にも offline compiler にも届く前に返る） |
| `src/metal.rs`（inline） | 絶対パスから作った `NSURL` がファイルの URL で、パスのバイト長が一致すること |
| `src/metallib.rs`（inline） | 作業のディレクトリが作られ drop で消えること、同じ名前の再作成が `WorkDirFailed` になること、上限を超えるファイルの読み出しが `WorkDirFailed` になること |

確かめ方（macOS で）: `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test coopmat_macos`・`--test unsafe_count`

## 門の run の device の試験（3/4）

macOS 向けに、門の 1 セルを枠の `run_selected_from_env` に `run_g7` を渡して流す試験を `#[ignore]` 付きで置く。device の要る専用の回でだけ `--ignored` で流し、既定の `cargo test` では流れない。

| ファイル | 試験 | 期待 |
| -------- | ---- | ---- |
| `tests/gate_g7.rs` | 選ばれた 1 セルを `run_g7` で流す（腕が `G7` でないセルは `UnsupportedCell`） | セルの腕が `G7`、`bit_mismatches()` が 0、`max_abs_diff()` が 0.0、`comparison().is_match()`、`validation_layer()` が真、`max_total_threads_per_threadgroup` が 32 以上。標準出力に結果の行・`metallib_sha256=<64 桁の小文字の 16 進>`・`max_total_threads_per_threadgroup=<値>` を 1 行ずつ出す |

- 試験は 1 本だけ（枠の入口が 1 つのテストのバイナリにつき 1 回のため）
- 環境変数で成果物を渡さない（metallib は腕が作る）
- 標準エラーには何も書かない
- metallib の sha256 は出力するだけで、期待の値とは比べない
- offline compiler は環境を引き継いで `/usr/bin/xcrun` で呼ぶので、`DEVELOPER_DIR` の Xcode のものが使われる

流すときの形:

```text
scripts/metal-session.sh run --combination <名前> --validation-layer --gate-cell <セルの識別名> -- cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test gate_g7 -- --ignored --nocapture
```

確かめ方: `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test gate_g7`（macOS では `1 ignored`、Linux では 0 本）・`cargo clippy --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --all-targets --target aarch64-apple-darwin --locked -- -D warnings`

## 完了の条件の確かめ（4/4）

TASK-1009 の 4/4（#6318）として、ここまでの成果物を本リポで観測できる事実だけで確かめた記録。(1)〜(5) は GPU なしの Linux での手元の実行、(6)・(7) は 3/4 のマージの後の main の push の run（run id `42670445568`）の CI のログと conclusion。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 腕の crate が `measure/` の member で、製品の workspace の外にある | `grep -n '^members' measure/Cargo.toml`、`grep -n '^exclude' Cargo.toml`、`make upper-layer-check` | `members = ["crates/cpu-ops-arm", "crates/detect", "crates/metal-gemm-arm", "crates/perf"]`、`exclude = ["baseline-wgpu", "measure", "wasm"]`、`record:` の行は `measure_pkgs=4 measure_contract_pkgs=1 measure_forbidden=0`、終了コード 0 |
| 2 | MSL と起動の形の試験が通る | `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test coopmat` | `ok. 6 passed; 0 failed; 0 ignored` |
| 3 | unsafe | 同 `--test unsafe_count`、`grep -rn 'allow(unsafe_code)' measure/crates/metal-gemm-arm/src`、`grep -hv '^\s*//' measure/crates/metal-gemm-arm/src/coopmat.rs measure/crates/metal-gemm-arm/src/metallib.rs \| grep -c '\bunsafe\b'` | `ok. 5 passed; 0 failed; 0 ignored`、`allow(unsafe_code)` は 1 件（`src/lib.rs:21`。直後の行が `mod metal;`）、コメントの行を除いた `unsafe` の件数は 0 |
| 4 | 依存 | `git diff d082bc1 -- measure/crates/metal-gemm-arm/Cargo.toml measure/Cargo.lock measure/deps-approved.toml`、`make measure-verify` の `deps-check (measure/)` | 差分は空、`ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml`、`make measure-verify` の終了コード 0 |
| 5 | metallib を repo に置いていない | `git ls-files measure \| grep -c '\.metallib$\|\.air$'` | `0` |
| 6 | macOS の腕の crate の試験 | `gh run view 42670445568 --log --job <verify-macos の job id>` の `measure-metal-gemm-arm` の `Running` 行ごとの `test result:` | lib の unittests は `ok. 7 passed; 0 failed; 0 ignored`、`coopmat` は 6 passed、`coopmat_macos` は 3 passed、`unsafe_count` は 5 passed、`gate_g7` は `0 passed; 0 failed; 1 ignored`、`source` は 10 passed、`fetch_cmd` は 3 passed、`launch` は 9 passed、`gate` は `0 passed; 0 failed; 1 ignored`、`device` は `0 passed; 0 failed; 2 ignored` |
| 7 | CI | `gh run view 42670445568 --json headSha,jobs` | headSha は `807baccddf483037b232d359c83386510d95ec98`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` の conclusion はいずれも `success` |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1009）。

確かめた base は `807bacc`、`docs/spec` の gitlink は `050073a33856cff4559718a8e760289ec3d5a65e`。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | MSL と起動の形 | #6315 |
| 2/4 | metallib の作成と腕の手順 | #6316 |
| 3/4 | 門の run の device の試験 | #6317 |
| 4/4 | 完了の条件の確かめ | #6318 |
