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
