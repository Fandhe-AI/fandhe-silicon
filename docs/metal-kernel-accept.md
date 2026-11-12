# 実行時に受けるカーネルの受け付けの道具（TASK-485）の所在と本リポの現状

TASK-485 の作業のうち、実行時に渡されるカーネルの一覧のファイルの読み取りと形の検査、入口名の抜き出しの所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-485。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `measure/crates/kernel-accept`（package `measure-kernel-accept`） | `measure/Cargo.toml` の `members` |
| 一覧の読み取りと検査 | `measure/crates/kernel-accept/src/manifest.rs`（`parse`・`load`・`Manifest`・`ManifestError`。純粋で、device に触れず、`unsafe` を書かない） | `pub mod manifest;`（`src/lib.rs`） |
| 入口名の抜き出し | `measure/crates/kernel-accept/src/scan.rs`（`scan_entries`） | `pub mod scan;`（`src/lib.rs`） |
| 試験 | `measure/crates/kernel-accept/tests/manifest.rs`・`tests/scan.rs`（どの OS でも走る。`#[ignore]` なし） | ファイルの有無 |
| 確かめ方 | `make measure-verify` | fmt / clippy / test / 依存の検査 |

## 一覧のファイルの形（1/6）

### 行の形

- UTF-8 のテキストで、1 行が 1 入口。改行は LF。最後の LF は付けても付けなくてもよい（あれば 1 つだけ）。
- 1 行は半角の空白 1 つで区切った 5 欄 `<rev> <path> <sha256> <mode> <entry>`。前後の空白・空白の重なりは欄の数が合わず拒否する。
- 行の番号は 1 始まり。空行・CR・タブなどの制御文字を含む行は拒否する。

### 欄ごとの検査

| 欄 | 受けるもの | 拒否するもの |
| -- | ---------- | ------------ |
| rev | 40 桁の小文字の 16 進 | それ以外 |
| path | 素直な相対パス（`metal-gemm-arm` の `check_source_path` と同じ） | 空・`-` か `/` で始まる・`..` 成分・`:`・NUL・改行などを含むもの |
| sha256 | 64 桁の小文字の 16 進 | それ以外 |
| mode | `msl`・`specialized` | それ以外 |
| entry | `[A-Za-z_][A-Za-z0-9_]{0,63}` | それ以外 |

### 行をまたぐ検査

- 全行の rev が 1 行目と同じ。
- 同じパスの行は連続し、sha256 が同じ。
- 違うパスの最後の成分（ファイル名）が重ならない。
- 入口名が一覧全体で重ならない。
- 最初に違反した行の番号で失敗を返す。

### 上限と失敗の文言

- 大きさ `MAX_MANIFEST_BYTES`（64 KiB）と行数 `MAX_MANIFEST_LINES`（1024）を超える一覧は、行ごとの検査より前に拒否する。`load` は通常のファイルだけを受け、読む前と開いた後に確かめ、上限つきで読む。
- 空の一覧は拒否する。
- 失敗の文言は固定の英語と行の番号だけで、入力の値（rev・パス・sha256・入口名）を含めない。

### 入口名の抜き出し

`scan_entries` は、正規表現 `kernel\s+void\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(` と同じ規則で、先頭から重ならずに入口名を出現の順に拾う。コメントは除かない。

## 一覧の 1 行の受け付け（2/6）

一覧の 1 入口を 1 プロセスでパイプラインにする bin `accept-kernel` と、その部品、device の要る試験の所在と振る舞いを置く。記録の正は private spec 側にある（TASK-485。ポインタのみ）。

| 項目 | 事実 |
| ---- | ---- |
| bin | `measure/crates/kernel-accept/src/bin/accept-kernel.rs`（`accept-kernel <manifest> <dir> <path> <entry>`） |
| 部品 | `src/accept.rs`（行の選択・ソースの置き場・入口の並びの照合・入力の組み立て・エラーの種類の写し。Metal に触れるのは macOS の cfg の内側の `create_on_device` だけ）、`src/constants.rs`（`specialized_constants`） |
| 試験（どの OS でも） | `tests/accept.rs`・`tests/constants.rs`・`tests/accept_cmd.rs` |
| 試験（device の要るもの） | `tests/accept_device.rs`（`#![cfg(target_os = "macos")]`、`#[ignore]` 付き 2 本） |

### bin の手順と終了コード

手順の順は固定で、device を作るのは最後の手順だけ。

1. 引数の数が 4 でない（UTF-8 でない引数を含む）と終了コード 2。
2. `<manifest>` を `load` で読む。読めない・形の違反は終了コード 2。`<path> <entry>` の組が一覧の行に無いときも終了コード 2。
3. macOS 以外は終了コード 3 で終わる。
4. `<dir>/<path のファイル名>` を、その行の sha256 で `load_source_against` が照らして読む。失敗は終了コード 1。
5. `scan_entries` の並びが、一覧のそのパスの入口の並び（数も順も）と一致しなければ終了コード 1。
6. device を作り（Metal・ComputeOnly・ProductIndependent）、`compute()` の口で空の配置のパイプラインを作る。投入はしない。入力は行の種類が `specialized` なら `MslSpecialized`、`msl` なら `Msl`。device が取れない・compute の口が無いときは終了コード 1。
7. 作れたら標準出力に `accepted` の 1 行で終了コード 0。作れなかったら標準出力にエラーの種類の固定の文字列を 1 行出し、終了コード 1。

| 終了コード | 意味 |
| ---------- | ---- |
| 0 | パイプラインを作れた |
| 1 | 手順 4〜7 の失敗 |
| 2 | 引数・一覧・選んだ組が不正（device に届く前に返る） |
| 3 | macOS 以外 |

標準エラーには固定の英語の文言と引数の番号だけを出し、引数の値・入口名・パスは出さない。契約のエラーの文言も出さず、固定の種類の文字列に写す。

### `specialized` の行に渡す定数

`specialized_constants` は、起動の表（`launch_for`）の f32・`Sq512` のセルの function constant 18 個を、`ConstantValue::UInt` → `U32`、`Bool` → `Bool` に写す（index はそのまま）。`tests/constants.rs` が個数・index・型・値を表のセルと突き合わせる。

### device の試験

- 試験の名前: `msl_entries_return_pipelines`・`specialized_entries_return_pipelines`。それぞれ一覧のその種類の全行について子プロセス `accept-kernel` を起動し、全部が終了コード 0 でシグナルでの終了が 0 のときだけ一致とする。期待の本数はその種類の行数で、0 行は失敗。
- 標準出力には行ごとの 1 行（一覧の行の番号と終了コードまたは `signal`）と集計の 1 行（行数・0 の数・それ以外の数・シグナルの数）を出す。
- 環境変数: `FANDHE_SILICON_KERNEL_MANIFEST`（一覧のパス）・`FANDHE_SILICON_KERNEL_DIR`（ソースを置いたディレクトリ）・`FANDHE_SILICON_TEST_MACHINE_COMBINATION`（試験の枠の組み合わせ）、必要なら `FANDHE_SILICON_TEST_RECORD_DIR`。欠けたときは skip せず失敗する。
- 流し方: `cargo test --manifest-path measure/Cargo.toml -p measure-kernel-accept --test accept_device -- --ignored`

## 起動の形と CPU の参照（3/6）

特殊化する GEMM・特殊化しない GEMM・加算・Relu の 4 つの役割を、契約の公開 API で流す口と、device に触れない起動の形・入力・CPU の参照を置く。所在は `src/flow.rs`（純粋な部品）・`src/run.rs`（段ごとに投入して待つ口。Metal に触れるのは macOS の cfg の内側だけ）・`tests/flow.rs`（どの OS でも走る試験）。

### 入口の受け取り

役割ごとに入口名を環境変数で受ける。

| 役割 | 環境変数 | 一覧の `mode` |
| ---- | -------- | ------------- |
| 特殊化する GEMM | `FANDHE_SILICON_GEMM_SPECIALIZED_ENTRY` | `specialized` |
| 特殊化しない GEMM | `FANDHE_SILICON_GEMM_PLAIN_ENTRY` | `msl` |
| 加算 | `FANDHE_SILICON_ADD_ENTRY` | `msl` |
| Relu | `FANDHE_SILICON_RELU_ENTRY` | `msl` |

一覧のファイルとソースのディレクトリは `FANDHE_SILICON_KERNEL_MANIFEST`・`FANDHE_SILICON_KERNEL_DIR` で受ける。受け取りは device を作る前に全部済ませる。順は次のとおり。

1. 値が欠ける・空であると `Err`。
2. 入口名が `[A-Za-z_][A-Za-z0-9_]{0,63}` の形でなければ `Err`。
3. 一覧にちょうど 1 回現れなければ `Err`。
4. 行の `mode` が役割と合わなければ `Err`。
5. 行のパスと sha256 で `load_source_against` が読む。失敗は `Err`。

device の口は、照合を終えた `FlowKernels` だけを受ける。エラーは固定の英語の文言と役割の名前だけで、値・パス・入口名・ソースを含めない。

### 形状と入力

形状は `Sq512`・`Sq2048`・`Odd37x41x53`。M・N・K は core の値。`Sq512`・`Sq2048` は特殊化する GEMM、`Odd37x41x53` は特殊化しない GEMM を使う。

- A・B は `generate_f32(row)`。
- 加算の第 2 項（M×N）は、固定の seed の `SplitMix64` の `next_signed_unit_f32` を M×N 回呼んで作る。

### 起動の形

| 役割 | パイプラインの入力 | スレッド数 | threadgroup 数 | 束縛 | 定数（u32、ネイティブエンディアン） | threadgroup のメモリ | layout |
| ---- | ------------------ | ---------- | -------------- | ---- | ---------------------------------- | -------------------- | ------ |
| 特殊化する GEMM | `MslSpecialized` | `launch_for` の値 `[threads,1,1]` | `[groups_width, groups_height, 1]` | slot 0 A・1 B・2 C | slot 3 `{m,n,k}`・4 `{k,n,0,0}`・5 `{0,0,groups_height,groups_width}`・6 `{1,k,0,0}` | index 0・`launch_for` の値 | `[0..=6]` |
| 特殊化しない GEMM | `Msl` | `[16,16,1]` | `[ceil(n/16), ceil(m/16), 1]` | slot 0 A・1 B・2 C | slot 3 `{m,n,k}` | なし | `[0,1,2,3]` |
| 加算 | `Msl` | `[256,1,1]` | `[ceil(len/256),1,1]` | slot 0 GEMM の出力・1 第 2 項・2 出力 | slot 3 `{len}` | なし | `[0,1,2,3]` |
| Relu | `Msl` | `[256,1,1]` | `[ceil(len/256),1,1]` | slot 0 加算の出力・1 出力 | slot 2 `{len}` | なし | `[0,1,2]` |

特殊化する GEMM の function constant と、スレッド数・threadgroup 数・threadgroup のメモリは `launch_for` の `Launch` の数値をそのまま使う（詰め物なし）。`tests/flow.rs` は `eff_m`・`eff_n`・`eff_k` が M・N・K と同じこと、threadgroup のメモリが 16 の倍数であることを確かめる。要素数が 0 の加算・Relu の形は `Err`。

### CPU の参照

- GEMM は `reference_gemm_f32(a, b, m, k, n)`。
- 加算は GEMM の参照の各要素に第 2 項を足した値。
- Relu は加算の参照の各要素 `x` に対し `if x > 0.0 { x } else { 0.0 }`（`-0.0` と NaN は `+0.0`）。

### 段ごとの投入

1 段は、パイプラインを作る → 入力と出力のバッファを確保して入力を書く → 記録先を作って `dispatch_ex` を 1 件積む → `submit` → `wait_blocking` → `poll` で完了を確かめる → `download_blocking` で出力を読む、の順。確保したバッファは成否に関わらず解放する。`run_flow` は GEMM → 加算 → Relu を 3 回の投入で流し、前の段の出力を次の段の入力にする。`run_gemm` は GEMM の段だけを流す。失敗は段・手順・固定の種類の文字列だけを持つ `RunError` で返し、値・入口名・契約の文言を含めない。要素数とバイト数の計算は `checked_*` で行う。製品の Metal の背後実装は記録のある投入を単体試験のビルド以外では `Unsupported` で拒むため、依存 crate としてこの口を呼ぶと段の投入は固定の種類の失敗で返り、出力は得られない。この crate は既存の拒否を迂回せず、device の試験はパイプラインの作成までを確かめる。

### 試験

`tests/flow.rs` は device に触れず、3 形状の起動の形、第 2 項の再現と長さ、加算と Relu の参照の式（`-1.0`・`0.0`・`-0.0`・正の値）、バイト列の往復、入口の受け取り（`lookup` の差し替えで欠け・形の誤り・一覧に無い・`mode` の食い違い・ソースの照合の失敗）と、エラーの文言に値が出ないことを確かめる。device の要る試験はこの節の範囲の外。
