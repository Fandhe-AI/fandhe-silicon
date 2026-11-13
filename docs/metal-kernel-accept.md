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

起動の形・入力・CPU の参照・入口の受け取り・段ごとの投入は、下の「契約の crate の単体試験の所在（4/6）」へ移った。measure の crate には `src/flow.rs`・`src/run.rs`・`tests/flow.rs` は無い。

## 契約の crate の単体試験の所在（4/6）

GEMM と GEMM→加算→Relu の device の要る試験と、その純粋な部品は、契約の crate の単体試験のビルドの中にある（TASK-485 のポインタ）。

| 項目 | 内容 |
| ---- | ---- |
| ファイル | `crates/contract/src/metal_flow_device_tests.rs` |
| 宣言 | `crates/contract/src/lib.rs` の `#[cfg(all(test, feature = "backend-metal", feature = "test-support"))] mod metal_flow_device_tests;`（`pub` にしない） |
| 純粋な部分 | 宣言の cfg のまま、どの OS でも流れる。一覧の 1 行の引き当て・入口の受け取り・ソースの照合・起動の形・入力と CPU の参照を持つ。module は `pure_tests` |
| device の部分 | module `device`。さらに `#[cfg(all(target_os = "macos", feature = "native-blocking-wait"))]`。その下に `gemm`（2 本）と `flow`（3 本） |
| feature の組 | `backend-metal,native-blocking-wait,test-support` |

純粋な部分は、一覧のテキストを LF で行に分け、各行を半角の空白 1 つで 5 欄に分ける（欄の数が 5 でない行は `Err`）。使う欄は `path` の最後の要素・`sha256`（64 桁の小文字の 16 進）・`mode`・入口名。ソースは `<ディレクトリ>/<ファイル名>` を大きさの上限（一覧 64 KiB、ソース 1 MiB）を読む前に照らして読み、`sha256_bytes` の値と一覧の値を比べ、UTF-8 として受ける。どれも device を作る前に済ませる。環境変数の読み出しは `&dyn Fn(&str) -> Option<OsString>` の差し替え口で受け、device の試験は `std::env::var_os` を渡す。エラーは固定の英語の文言で、値・パス・入口名・ソースを含まない。

device の部分は公開の `MetalDeviceProvider::request` で device を作り、段ごとに記録先を作って `dispatch_ex` を 1 件積み、`submit`・`wait_blocking`・`poll` で完了を確かめてから `download_blocking` で出力を読む。段の後には成否に関わらずバッファを解放する。`flow` は先頭の段を流す前に 3 段すべてのカーネルの役割を照らす。

## device の要る試験（4/6）

どれも `#[ignore = "touches the Metal device; run only in a dedicated device session"]` 付きで、専用の実機の回でだけ流す。判定は core の試験の枠で行い、組み合わせの環境変数が無い未実施も失敗にする。

| module | 試験 | 内容 |
| ------ | ---- | ---- |
| `device::gemm` | `specialized_gemm_sq512_matches_reference`・`specialized_gemm_sq2048_matches_reference` | 特殊化する GEMM の出力を CPU の参照と比べる |
| `device::flow` | `gemm_add_relu_sq512_matches_reference`・`gemm_add_relu_sq2048_matches_reference`・`gemm_add_relu_odd_matches_reference` | 3 段の出力をそれぞれの CPU の参照と比べ、3 段すべて一致のときだけ一致とする |

受ける環境変数: `FANDHE_SILICON_KERNEL_MANIFEST`（一覧のファイルの絶対パス）・`FANDHE_SILICON_KERNEL_DIR`（ソースを置いたディレクトリ）・`FANDHE_SILICON_GEMM_SPECIALIZED_ENTRY`・`FANDHE_SILICON_GEMM_PLAIN_ENTRY`・`FANDHE_SILICON_ADD_ENTRY`・`FANDHE_SILICON_RELU_ENTRY`（各役割の入口名）と、試験の枠の `FANDHE_SILICON_TEST_MACHINE_COMBINATION`。欠けた試験は固定の文言で失敗する。標準出力には件ごとに 1 行（形状・段・要素数・最大の差・判定）を出し、入口名とパスは出さない。

流し方（2048 の CPU の参照は debug では遅いので `--release`）:

```bash
cargo test --release -p fandhe-silicon-contract \
  --features backend-metal,native-blocking-wait,test-support \
  --lib metal_flow_device_tests -- --ignored
```

絞るときは `metal_flow_device_tests::device::gemm` や `metal_flow_device_tests::device::flow` を指定する。device の無い OS では `device` module はコンパイルされず、純粋な部分の試験だけが流れる。

## 3/6 の口の移しの補い

GEMM と GEMM→加算→Relu の口と device の要る試験は、上の「device の要る試験（4/6）」の契約の crate の単体試験にある。measure の crate からは `src/flow.rs`・`src/run.rs`・`tests/flow.rs` を外した。

| 項目 | 現状 |
| ---- | ---- |
| `launch_function_constants` | `src/constants.rs` にあり、`tests/constants.rs` が特殊化する GEMM の 2 つの形状で起動の表と照らす |
| 契約の crate の依存の feature | `backend-metal` だけ（macOS 向けの依存として宣言） |
| Metal に触れる箇所 | `accept` の macOS の cfg の内側だけ |

確かめ方: `git grep -n 'flow::\|run::' -- measure/crates/kernel-accept` が 0 件、`make measure-verify` が通る。

## バッチの積和と部分 Top-k（5/6）

クエリ×文書の内積（得点）と、クエリごとの部分 Top-k を Metal の device に 2 段で投入し、CPU の参照と比べる試験。カーネルは試験のファイルの中の文字列で、`Msl` として作る。

| 項目 | 現状 |
| ---- | ---- |
| ファイル | `crates/contract/src/metal_batch_topk_device_tests.rs` |
| `lib.rs` の宣言 | `#[cfg(all(test, feature = "backend-metal", feature = "test-support"))] mod metal_batch_topk_device_tests;` |
| 純粋な部分 | 入力の生成・得点の参照・候補の併合・起動の形・バイト列の変換。module `pure_tests` の試験はどの OS でも流れる |
| device の部分 | module `device`。`#[cfg(all(target_os = "macos", feature = "native-blocking-wait"))]` |
| feature の組 | `backend-metal,native-blocking-wait,test-support` |

起動の形:

| 段 | スレッド数 | threadgroup 数 | 定数（slot 3） |
| -- | ---------- | -------------- | -------------- |
| 得点 | `[16, 16, 1]` | `[ceil(N/16), ceil(Q/16), 1]` | `{ n_docs, dim, n_queries }`（u32 が 3 個） |
| 部分 Top-k | `[256, 1, 1]` | `[Q, 1, 1]` | `{ n_docs, k }`（u32 が 2 個） |

どちらも layout は `[0, 1, 2, 3]`。得点の出力のバッファが、そのまま部分 Top-k の束縛 0 になる。

device の要る試験（どちらも `#[ignore = "touches the Metal device; run only in a dedicated device session"]` 付き。判定は core の試験の枠で行い、組み合わせの環境変数が無い未実施も失敗にする）:

| 試験 | 規模（N, D, Q） | 内容 |
| ---- | --------------- | ---- |
| `device::scores_and_topk_small_match_reference` | (20000, 128, 64) | 得点を CPU の参照と比べ、device が返した得点の各行に当てた上位 10 件と、候補を併合した結果を全クエリで比べる |
| `device::scores_and_topk_large_match_reference` | (100000, 128, 64) | 同上 |

得点と全クエリの上位 10 件がすべて一致のときだけ一致とする。標準出力には規模ごとに 1 行（規模・k・得点の最大の差・一致したクエリの数・判定）を出す。受ける環境変数は試験の枠の `FANDHE_SILICON_TEST_MACHINE_COMBINATION` だけ。

流し方（CPU の参照は debug では遅いので `--release`）:

```bash
cargo test --release -p fandhe-silicon-contract \
  --features backend-metal,native-blocking-wait,test-support \
  --lib metal_batch_topk_device_tests -- --ignored
```

device の無い OS では `device` module はコンパイルされず、純粋な部分の試験だけが流れる。

## 完了の条件の確かめ（6/6）

TASK-485 の 6/6 として、ここまでの成果物の所在と、device の回で流す形を、本リポで観測できる事実だけでまとめる。完了の条件の正は private spec 側の記録に従う（`docs/spec/05-tasks.md` の TASK-485。ポインタのみ）。

所在の案内: 上の「所在」の表は measure の crate の分。契約の crate の単体試験の所在は「契約の crate の単体試験の所在（4/6）」と「バッチの積和と部分 Top-k（5/6）」の節、measure から外した口は「3/6 の口の移しの補い」の節にある。

### 確かめの対象

| # | 確かめる振る舞い | コードの所在 | 試験 | 流し方 |
| - | ---------------- | ------------ | ---- | ------ |
| 1 | 一覧の各行の入口をパイプラインにする受け付け | `measure/crates/kernel-accept`（bin `accept-kernel`、`tests/accept_device.rs`） | `msl_entries_return_pipelines`・`specialized_entries_return_pipelines` | `cargo test --release --manifest-path measure/Cargo.toml --locked -p measure-kernel-accept --test accept_device -- --ignored` |
| 2 | 特殊化する GEMM の出力と CPU の参照の比べ | `crates/contract/src/metal_flow_device_tests.rs` の `device::gemm` | `specialized_gemm_sq512_matches_reference`・`specialized_gemm_sq2048_matches_reference` | `cargo test --release -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,test-support --lib metal_flow_device_tests::device::gemm -- --ignored` |
| 3 | GEMM→加算→Relu の 3 段の出力の比べ | 同ファイルの `device::flow` | `gemm_add_relu_sq512_matches_reference`・`gemm_add_relu_sq2048_matches_reference`・`gemm_add_relu_odd_matches_reference` | 同じ形で `metal_flow_device_tests::device::flow` |
| 4 | バッチの積和と部分 Top-k の比べ | `crates/contract/src/metal_batch_topk_device_tests.rs` の `device` | `scores_and_topk_small_match_reference`・`scores_and_topk_large_match_reference` | 同じ形で `metal_batch_topk_device_tests` |

### 流す形

- 同じ回で 1 の受け付けを先に流し、一覧の形は measure 側で確かめる。その後に製品の workspace で 2〜4 を流す。
- 一覧のファイルは「一覧のファイルの形（1/6）」の形のもので、repo の外に置く。一覧に現れる重複しないパスごとに、ソースを `fetch-kernel-source <clone> <dir>/<ファイル名> <rev> <path> <sha256>` の 5 引数の形で repo の外のディレクトリに取得する。
- 環境変数は同じシェルで渡す。値は書かず、置き場の字句で示す。

| 試験の組 | 受ける環境変数 |
| -------- | -------------- |
| 1（measure） | `FANDHE_SILICON_KERNEL_MANIFEST`（`<一覧のファイルの絶対パス>`）・`FANDHE_SILICON_KERNEL_DIR`（`<取得したソースのディレクトリ>`）と試験の枠の分 |
| 2・3（`metal_flow_device_tests`） | 上の 2 つに `FANDHE_SILICON_GEMM_SPECIALIZED_ENTRY`・`FANDHE_SILICON_GEMM_PLAIN_ENTRY`・`FANDHE_SILICON_ADD_ENTRY`・`FANDHE_SILICON_RELU_ENTRY` と試験の枠の分 |
| 4（`metal_batch_topk_device_tests`） | 試験の枠の分だけ |

試験の枠の分は `FANDHE_SILICON_TEST_MACHINE_COMBINATION`（必要なら `FANDHE_SILICON_TEST_RECORD_DIR`）。どれも `--release` と `--ignored` で流す。環境変数が欠けた試験は skip せず失敗する。

### `#[ignore` の件数

数え方は `grep -cE '^\s*#\[ignore' <file>`。

| ファイル | 件数 |
| -------- | ---- |
| `measure/crates/kernel-accept/tests/accept_device.rs` | 2 |
| `crates/contract/src/metal_flow_device_tests.rs` | 5 |
| `crates/contract/src/metal_batch_topk_device_tests.rs` | 2 |

### 確かめの行

| # | 確かめ | コマンド |
| - | ------ | -------- |
| 1 | 投入の門の守りの試験が通る | `cargo test -p fandhe-silicon-contract --all-features --test backend_metal_no_recording --test backend_metal_unsafe_count` |
| 2 | 3 つの移しの commit の差分に `crates/contract/src/backend_metal.rs`・`crates/contract/Cargo.toml`・`crates/contract/tests/` が無い | `git show --stat 7e4899a 8844241 fbda0eb` |
| 3 | 新しい 2 つのファイルに `unsafe` が無い | `git grep -c unsafe -- crates/contract/src/metal_flow_device_tests.rs crates/contract/src/metal_batch_topk_device_tests.rs` が何も出さない |
| 4 | Linux では device の部分がコンパイルされない | `cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,test-support --lib -- --list` に `device::` の試験が出ない。macOS の型検査は CI の `verify-macos` と `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --features backend-metal,native-blocking-wait,test-support --lib --tests -- -D warnings` |
| 5 | `test-support` なしの組の `--lib` に入らない | `cargo test -p fandhe-silicon-contract --features backend-metal --lib -- --list` に `metal_flow_device_tests`・`metal_batch_topk_device_tests` が出ない |
| 6 | measure の crate に `run`・`flow` の module が無い | `git grep -nE 'mod (run\|flow)\b\|(run\|flow)::' -- measure/crates/kernel-accept` が 0 件 |

確かめた base は `fbda0eb`。
