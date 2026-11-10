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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 5/7 | 取得の道具の 5 引数の形 | #8191 |
