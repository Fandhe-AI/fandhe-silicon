# 実行時に照らすソースの Metal 直接の GEMM の腕（TASK-1009）の所在と本リポの現状

TASK-1009 の作業のうち、カーネルのソースの取得の道具が rev・パス・sha256 を実行時に受ける形の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1009。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `measure/crates/metal-gemm-arm`（crate の所在は [metal-gemm-arm](metal-gemm-arm.md)、TASK-1008） | `measure/Cargo.toml` の `members` |
| bin | `measure/crates/metal-gemm-arm/src/bin/fetch-kernel-source.rs` | ファイルの有無 |
| 引数の検査 | `measure/crates/metal-gemm-arm/src/source.rs`（`check_rev`・`check_source_path`・`parse_sha256_hex`・エラーの型 `ArgError`。どれも純粋な関数で、`unsafe` を書かない） | `pub mod source;`（`src/lib.rs`） |
| 試験 | `measure/crates/metal-gemm-arm/tests/fetch_cmd_pinned.rs` | ファイルの有無 |

## 取得の道具の 5 引数の形（5/7）

### 形

- 2 引数: `fetch-kernel-source <clone> <output>`。crate に固定した値で取り出す。挙動と終了コードは変えない。
- 5 引数: `fetch-kernel-source <clone> <output> <rev> <path> <sha256>`。3 値を実行時に必須で受ける。

5 引数の形は `git --no-pager -C <clone> show <rev>:<path>` を shell を介さず引数 1 つずつで起動し、
出力を上限つきで読み、sha256 を照らし、通ったものだけを新規作成で書く。標準エラーは捨てる。

### 検査

検査はどれも git を起動する前に、引数の 3・4・5 番目、書き出し先の順で行う。

| 引数 | 受けるもの | 拒否するもの |
| ---- | ---------- | ------------ |
| 3 rev | 40 桁の小文字の 16 進 | 桁数の違い・大文字・16 進でない文字 |
| 4 パス | 素直な相対パス | 空・4096 バイト超・`-` か `/` で始まる・`\`・NUL・改行・`:` を含む・空か `.` か `..` の成分 |
| 5 sha256 | 64 桁の小文字の 16 進 | 桁数の違い・大文字・16 進でない文字 |

### 終了コード

| コード | 条件 |
| ------ | ---- |
| 0 | 照合を通り、書き出した |
| 1 | git の失敗（木に無いパスなど）または照合の失敗 |
| 2 | 引数の数・引数の形式・既にある書き出し先・書き出し先を作れない |

標準エラーは固定の文言と引数の番号の 1 行で、引数の値を出さない。

### 試験

合成の一時 git repo を使い、期待の sha256 は試験の中で計算する。

| 試験 | 確かめること |
| ---- | ------------ |
| `matching_sha_writes_the_file_bytes` | 合う sha で 0、書き出し先がファイルとバイトで一致 |
| `one_digit_off_sha_fails_without_output` | 1 桁違う sha で 1、書き出し先ができない |
| `path_missing_from_tree_fails_without_output` | 木に無いパスで 1、書き出し先ができない |
| `invalid_arguments_are_usage_errors_before_git` | 不正な rev・パス・sha で 2、標準エラーが固定の 1 行、書き出し先ができない（存在しない clone で git の前に返ることも確かめる） |
| `existing_output_is_kept_in_five_arg_form` | 既にある書き出し先は 2 で中身が変わらない |
| `other_arg_counts_are_usage_errors` | 引数の数 3・4・6 は 2 |
| `check_source_path_boundaries` | 検査の関数を直に呼ぶ（NUL・改行・`\`・`:`・空の成分・4096／4097 バイト） |
| `check_rev_and_sha_boundaries` | 桁数・大文字・16 進でない文字の境界と、sha256 の往復 |

### 確かめ方

```text
cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test fetch_cmd_pinned
```

## 実行時に照らすソースの腕の手順（6/7）

### 手順

`run_g5`（`src/metal.rs`。macOS 向け）は `unsafe fn` で、照合は内容の同一性しか保証しないため、ソースの入口が宣言どおりにだけ buffer を使うことを呼び出し側が `# Safety` の契約として保証する。成果物の引数は取らず、次の順で進む。1〜5 の検査は safe な `check_g5_request`（`src/external.rs`）に切り出してあり、`run_g5` は本体の最初にこれを呼ぶ。device を作るのは 7 の後。

1. 腕が `GemmGateArm::G5` でなければ `UnsupportedCell`
2. 起動の形（`external_launch_for`）が無ければ `UnsupportedCell`
3. 入力が f32 でなければ `DtypeMismatch`
4. A・B の長さが M×K・K×N でなければ `LengthMismatch`（計算のあふれは `Overflow`）
5. 検証レイヤーが有効でなければ `ValidationLayerRequired`
6. 環境変数からソースを読んで照らす。sha256 の不一致は `SourceMismatch`、ほかの失敗は `KernelSettingsInvalid`
7. 既定の device（無ければ `NoDevice`）
8. 照合を通ったメモリ上の文字列を、オプションを設定せずにコンパイル（`CompileFailed`）
9. 入口名の関数（function constant なし）と pipeline を作る（`FunctionFailed`・`PipelineFailed`）
10. 256 スレッドが pipeline の上限を超えれば `ThreadsExceeded`
11. A・B・C の共有バッファを作り、A・B を書き、C を 0 で埋める
12. 束縛して 1 回投入し、完了待ちの後に状態が完了でなければ `SubmitFailed`、完了なら C を読む（切り出しなし）

### 環境変数

| 名前 | 内容 |
| ---- | ---- |
| `FANDHE_SILICON_KERNEL_SOURCE` | ソースのファイルのパス |
| `FANDHE_SILICON_KERNEL_SHA256` | 期待の sha256（64 桁の小文字の 16 進） |
| `FANDHE_SILICON_KERNEL_ENTRY` | 入口名（`[A-Za-z_][A-Za-z0-9_]{0,63}`） |

3 つとも必須。`ExternalKernel::from_env`（または `from_lookup`）の検査の順は、未設定、sha256 の形、入口名の形、ソースの読み取りと照合。
失敗の文言は固定の英語で、値・パスを含まない。

### 束縛と起動の形

| 位置 | 内容 |
| ---- | ---- |
| buffer 0 | A（M×K、f32） |
| buffer 1 | B（K×N、f32） |
| buffer 2 | C（M×N、f32。0 で埋める） |
| buffer 3 | `Dims { m, n, k }`（12 バイト。元の M・N・K） |

- threadgroup は 16×16×1、threadgroup 数は `(ceil(n / 16), ceil(m / 16), 1)`。詰め物はしない。
- threadgroup のメモリは設定しない。

### 事実の型

`SourceFacts` は `max_total_threads_per_threadgroup`（pipeline の上限）と `source_sha256`（照合を通ったソースについて計算した値）を持つ。

### unsafe の置き場

`unsafe` を書くのは `src/metal.rs` だけ（`allow(unsafe_code)` は `src/lib.rs` の `mod metal` と、`#[ignore]` の device の試験 `tests/gate_g5.rs` の 2 か所だけ）。`src/external.rs` と `src/source.rs` には書かない。

### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `tests/external.rs` | 未設定・sha256 と入口名の形・読めないパス・不一致・上限超え・UTF-8 でない入力の `Err` と、文言に値が出ないこと。合成のファイルでの `Ok`。起動の形 |
| `tests/external_macos.rs` | safe な `check_g5_request`（`src/external.rs`）で、腕・dtype・長さの食い違いが device に届く前に `Err`（macOS のみ。`allow(unsafe_code)` は持たない） |
| `tests/external_validation_macos.rs` | 検証レイヤーの環境変数を外した子プロセスで `check_g5_request` が `ValidationLayerRequired`（macOS のみ。`allow(unsafe_code)` は持たない） |

### 確かめ方

```text
cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test external
```

macOS では `--test external_macos`・`--test external_validation_macos`・`--test unsafe_count` も流す。

## 門の run の device の試験（7/7）

macOS 向けに、門の 1 セルを枠の `run_selected_from_env` に `run_g5` を渡して流す試験を `#[ignore]` 付きで置く。device の要る専用の回でだけ `--ignored` で流し、既定の `cargo test` では流れない。

| ファイル | 試験 | 期待 |
| -------- | ---- | ---- |
| `tests/gate_g5.rs` | 選ばれた 1 セルを `run_g5` で流す（腕が `G5` でないセルは `UnsupportedCell`） | セルの腕が `G5`、`bit_mismatches()` が 0、`max_abs_diff()` が 0.0、`comparison().is_match()`、`validation_layer()` が真、`max_total_threads_per_threadgroup` が 256 以上。標準出力に結果の行・`source_sha256=<64 桁の小文字の 16 進>`・`max_total_threads_per_threadgroup=<値>` を 1 行ずつ出す |

- 試験は 1 本だけ（枠の入口が 1 つのテストのバイナリにつき 1 回のため）
- ソースのファイル・sha256・入口名は `run_g5` が環境変数で受ける（試験は新しい環境変数を読まない）
- 標準エラーには何も書かない
- 計算した sha256 は出力するだけで、期待の値とは比べない
- `tests/gate_g5.rs` の先頭に `#![allow(unsafe_code)]` を置き、`run_g5`（`unsafe fn`）を呼ぶ `unsafe` ブロックを 1 つ書く。`src/` の `unsafe` の個数は変わらない

流すときの形:

```text
scripts/metal-session.sh run --combination <名前> --validation-layer --gate-cell <セルの識別名> -- env FANDHE_SILICON_KERNEL_SOURCE=<ファイル> FANDHE_SILICON_KERNEL_SHA256=<値> FANDHE_SILICON_KERNEL_ENTRY=<入口名> cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test gate_g5 -- --ignored --nocapture
```

`<ファイル>` は `fetch-kernel-source <clone> <output> <rev> <path> <sha256>` の 5 引数の形で書く。

確かめ方: `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test gate_g5`（macOS では `1 ignored`、Linux では 0 本）・`cargo clippy --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --all-targets --target aarch64-apple-darwin --locked -- -D warnings`

## 完了の条件の確かめ（7/7）

TASK-1009 の 7/7（#8193）として、ここまでの成果物を本リポで観測できる事実だけで確かめた記録。(1)〜(5)・(8) は Linux での手元の実行、(6)・(7) は 6/7 のマージ（`268802f`）の後の main の push の run（run id `42858489754`）の CI のログと conclusion。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 腕の crate が `measure/` の member で、製品の workspace の外にある | `grep -n '^members' measure/Cargo.toml`、`grep -n '^exclude' Cargo.toml`、`make upper-layer-check` | `members = ["crates/cpu-ops-arm", "crates/detect", "crates/metal-gemm-arm", "crates/perf"]`、`exclude = ["baseline-wgpu", "measure", "wasm"]`、`record:` の行は `measure_pkgs=4 measure_contract_pkgs=1 measure_forbidden=0`、終了コード 0 |
| 2 | 取得の道具と実行時に照らす入力の試験が通る | `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test fetch_cmd_pinned`・`--test fetch_cmd`・`--test external` | `fetch_cmd_pinned` は `ok. 8 passed; 0 failed; 0 ignored`、`fetch_cmd` は `ok. 3 passed; 0 failed; 0 ignored`、`external` は `ok. 7 passed; 0 failed; 0 ignored` |
| 3 | unsafe | 同 `--test unsafe_count`、`git grep -n 'allow(unsafe_code)' -- measure/crates/metal-gemm-arm`、`grep -hv '^\s*//' measure/crates/metal-gemm-arm/src/external.rs \| grep -c '\bunsafe\b'` | `ok. 5 passed; 0 failed; 0 ignored`、`allow(unsafe_code)` は `src/lib.rs:25`・`tests/external_macos.rs:6`・`tests/external_validation_macos.rs:7`・`tests/unsafe_count.rs:298`（試験の入力の文字列リテラルの中）の 4 行（`tests/gate_g5.rs` を足す前の base の値）、コメントの行を除いた `unsafe` の件数は 0 |
| 4 | 依存 | `git diff ae152fb -- measure/crates/metal-gemm-arm/Cargo.toml measure/Cargo.lock measure/deps-approved.toml`、`make measure-verify` の `deps-check (measure/)` | 差分は空、`make measure-verify` の終了コード 0 |
| 5 | 取得したソースを repo に置いていない | `git ls-files measure \| grep -c '\.metal$'` | `1` |
| 6 | macOS の腕の crate の試験 | `gh run view 42858489754 --log --job <verify-macos の job id>` の `measure-metal-gemm-arm` の `Running` 行ごとの `test result:` | lib の unittests は `ok. 7 passed; 0 failed; 0 ignored`、`coopmat` は 6 passed、`coopmat_macos` は 3 passed、`device` は `0 passed; 0 failed; 2 ignored`、`external` は 7 passed、`external_macos` と `external_validation_macos` の `Running` 行の後に `3 passed` と `1 passed` の 2 行、`fetch_cmd` は 3 passed、`fetch_cmd_pinned` は 8 passed、`gate` と `gate_g7` の `Running` 行の後に `1 ignored` の 2 行、`launch` は 9 passed、`source` は 10 passed、`unsafe_count` は 5 passed |
| 7 | CI | `gh run view 42858489754 --json headSha,jobs` | headSha は `268802fd55864352194ca926be8ee61c017459a8`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` の conclusion はいずれも `success` |
| 8 | `tests/gate_g5.rs`（手元の実行） | `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test gate_g5`、`cargo clippy --manifest-path measure/Cargo.toml -p measure-metal-gemm-arm --all-targets --target aarch64-apple-darwin --locked -- -D warnings` | Linux では `ok. 0 passed; 0 failed; 0 ignored`（`cfg(target_os = "macos")` で外れる）、`aarch64-apple-darwin` の clippy は終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1009）。

確かめた base は `268802f`、`docs/spec` の gitlink は `c272f877055ba0939110b9552bc8a18c4c6b9e22`。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 5/7 | 取得の道具の 5 引数の形 | #8191 |
| 6/7 | 実行時に照らすソースの腕の手順 | #8192 |
| 7/7 | 門の run の device の試験・完了の条件の確かめ | #8193 |
