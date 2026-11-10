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

`run_g5`（`src/metal.rs`。macOS 向け）は `unsafe fn` で、照合は内容の同一性しか保証しないため、ソースの入口が宣言どおりにだけ buffer を使うことを呼び出し側が `# Safety` の契約として保証する。成果物の引数は取らず、次の順で進む。device を作るのは 7 の後。

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

`unsafe` を書くのは `src/metal.rs` だけ。`src/external.rs` と `src/source.rs` には書かない。

### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `tests/external.rs` | 未設定・sha256 と入口名の形・読めないパス・不一致・上限超え・UTF-8 でない入力の `Err` と、文言に値が出ないこと。合成のファイルでの `Ok`。起動の形 |
| `tests/external_macos.rs` | 腕・dtype・長さの食い違いが device に届く前に `Err`（macOS のみ） |
| `tests/external_validation_macos.rs` | 検証レイヤーの環境変数を外した子プロセスで `ValidationLayerRequired`（macOS のみ） |

### 確かめ方

```text
cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm --test external
```

macOS では `--test external_macos`・`--test external_validation_macos`・`--test unsafe_count` も流す。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 5/7 | 取得の道具の 5 引数の形 | #8191 |
| 6/7 | 実行時に照らすソースの腕の手順 | #8192 |
