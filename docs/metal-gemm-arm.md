# Metal 直接の GEMM の腕の crate（TASK-1008）の所在と本リポの現状

TASK-1008 の作業のうち、測定専用の別 workspace `measure/` に置く腕の crate の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1008。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `measure/crates/metal-gemm-arm`（package `measure-metal-gemm-arm`、lib `measure_metal_gemm_arm`） | `measure/Cargo.toml` の `members` |
| 依存 | `crates/core`（feature `test-support`）への path の 1 つだけ。外部依存は無い | `measure/crates/metal-gemm-arm/Cargo.toml` |
| lints | workspace の表を継ぐ（`unsafe_code = "forbid"`） | `[lints] workspace = true` |
| 流れる場所 | `make measure-verify` | `scripts/measure-verify.sh` |
| 門の枠 | `crates/core` の `parity` の module（TASK-1005 のポインタ。所在は [gemm-test-inputs](gemm-test-inputs.md)） | `crates/core/src/parity/` |

## カーネルのソースの取得と照合（2/5）

固定の rev・パス・sha256 は `src/source.rs` の定数（`SOURCE_REV`・`GEMM_SOURCE_PATH`・`GEMM_SOURCE_SHA256`）。値は本書に書かない。

| 関数 | 動き |
| ---- | ---- |
| `verify_source_against(bytes, expected)` | 長さが `MAX_SOURCE_BYTES` を超えれば `TooLarge`、sha256 が違えば `DigestMismatch`、UTF-8 でなければ `NotUtf8`、通れば `&str` |
| `verify_source(bytes)` | `GEMM_SOURCE_SHA256` で `verify_source_against` を呼ぶ |
| `load_source(path)` | 通常のファイルを上限つきで読み、`verify_source` を通ったものだけを `String` で返す。読めなければ `Unreadable` |

`SourceError` の文言は固定の英語で、パスやハッシュの値を含まない。

### bin `fetch-kernel-source`

使い方: `fetch-kernel-source <clone> <output>`。clone から `git show <rev>:<path>` で取り出し、`verify_source` を通ったときだけ `<output>` に新規作成で書く。取得したファイルは repo の中に置かない。

| 終了コード | 条件 |
| ---------- | ---- |
| 0 | 書いた |
| 1 | git の取得の失敗、上限超過、照合の失敗、書き込みの失敗（作ったファイルは消す） |
| 2 | 引数の数が違う、書き出し先が既にある、書き出し先を作れない |

- 書き出し先が既にあるかは git を起動する前に見る。上書きしない
- 標準エラーには固定の文言と引数の番号だけを出す。git の標準エラーは捨てる

### 試験

| ファイル | 内容 |
| -------- | ---- |
| `tests/source.rs` | 合成の入力での長さ・sha256・UTF-8 の拒否と受理、定数の形、`load_source` の失敗、`Display` の文言 |
| `tests/fetch_cmd.rs` | bin の引数の誤用、clone が無いときの終了コードと書き出し先が作られないこと、既にある書き出し先を変えないこと、標準エラーに引数の値が出ないこと |

確かめ方: `cargo test --manifest-path measure/Cargo.toml --locked -p measure-metal-gemm-arm`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 2/5 | カーネルのソースの取得と照合 | #6310 |
