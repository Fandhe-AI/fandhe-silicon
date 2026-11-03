# 下-2 の走査（TASK-923）の所在と本リポの現状

下-2 の crate（`crates/exec`）のソースを字句で数える検査の所在と、本リポのスクリプトとテストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-923。ポインタのみ）。
spec の本文・決定の内容は本頁へ転記しない。親 Issue は #867。

## 下-2 のソースの環境変数・ファイルの読み取りの走査（1/6）

### 所在

| 項目 | 内容 |
| ---- | ---- |
| スクリプト | `scripts/exec-surface-check.sh`（bash + python3。Python 3.8 以上の標準ライブラリだけ） |
| 回帰テスト | `scripts/test-exec-surface-check.sh` |
| make ターゲット | `make exec-surface-check`・`make test-exec-surface-check`（`make deny` には含めない） |
| 依存の追加 | なし（外部の crate・Rust のコードの変更なし） |

### 使い方と終了コード

| 呼び方 | 動き |
| ------ | ---- |
| 引数なし | `crates/exec` を走査し、HEAD のコミットを `record:` の行に載せる |
| `--crate-dir <dir>` | `<dir>` の crate を走査する（コミットは `-`。回帰テストが一時ディレクトリへ写した crate を渡す） |

| 終了コード | 意味 |
| ---------- | ---- |
| 0 | 合格（違反 0 件） |
| 1 | 違反あり |
| 2 | 誤用・前提の不足（未知の引数、値の無い `--crate-dir`、crate ディレクトリが無い、`.rs` が 0 件、UTF-8 でない、シンボリックリンク、読めないディレクトリ、閉じない `use std::{`、既定の走査でコミットを読めない、想定外の例外） |

### 出力の読み方

| 出力 | 内容 |
| ---- | ---- |
| `scanned: file=<相対パス>` | 読んだファイル（crate ディレクトリからの相対パス） |
| `record: commit=<rev\|-> rs_files=<n> build_rs=<yes\|no> env_fs=<n>` | 数え終えたとき（終了コード 0 と 1）の件数。`rs_files` は見つけた `.rs` の数（`build.rs` を含む） |
| `ok: exec surface check passed` | 合格時の 1 行 |
| `violation: check=env-fs file=<相対パス> line=<n> word=<字句>` | 標準エラー。違反ごとに 1 行 |
| `FAILED: <n> violation(s)` | 標準エラー。違反があるとき |
| `error: ...` | 標準エラー。終了コード 2 のとき（`record:`・`ok:` は出ない） |

ホストの絶対パス・環境変数の値は出力に出ない。

### 走査の対象のファイル

| 区分 | 内容 |
| ---- | ---- |
| 見る | crate ディレクトリの下で見つかったすべての `.rs`（`src/`・`tests/`・`benches/`・`examples/`・直下の `build.rs`。`mod` の宣言の有無と `#[cfg(test)]` を問わない） |
| 見ない | `target` と `.` 始まりのディレクトリ（どの深さでも）、`.rs` 以外のファイル |

### 字句

| 字句 | 数え方 |
| ---- | ------ |
| `std::env`・`env::var`・`option_env!`・`env!`・`std::fs`・`File::open`・`include_str!`・`include_bytes!`・`include!` | 各ファイルの生のテキスト（コメント・doc コメント・文字列を除かない）で、部分文字列としてそれぞれ独立に数える。重なり（`option_env!` の中の `env!`）は引かない |
| `std::{env}`・`std::{fs}` | `use` 文の `std::{…}`（`use ::std::{`・`pub use std::{` を含む。空白は任意）の対応する `}` までの中（入れ子を含む）で、識別子として `env`・`fs` に一致する段 |

期待は合計 0 件。識別子の一部（`from_env` など）は数えない。

### 回帰テストの場面

| 場面 | 期待の終了コード | 見る出力 |
| ---- | ---------------- | -------- |
| 実物の `crates/exec` | 0 | `ok:`・`env_fs=0`・`scanned: file=tests/common/mod.rs` |
| 9 つの字句をそれぞれ `src/lib.rs` のコードの行に足した変異体 | 1 | `check=env-fs`・`word=<字句>`・`file=src/lib.rs` |
| `use std::{env};`・`use std::{io, fs::File};`・入れ子・`pub use ::std::{ fs };` | 1 | `word=std::{env}` または `word=std::{fs}` |
| コメント・doc コメント・文字列・`#[cfg(test)]` の module の中の字句 | 1 | `check=env-fs` |
| `tests/` の既存のファイル、`mod` の宣言が無い `src/extra.rs`、直下の新しい `build.rs` | 1 | `file=<足したファイル>`（`build.rs` は `build_rs=yes`） |
| `target/`・`.hidden/`・`src/.hidden/` の下の字句、識別子の一部、`env` も `fs` も無い `use std::{io};` | 0 | `env_fs=0`・`!.hidden` |
| 未知の引数、値の無い `--crate-dir`、無いディレクトリ、`.rs` が 0 件、UTF-8 でない、ファイルとディレクトリのシンボリックリンク、閉じない `use std::{`、読めないサブディレクトリ、python3 が無い | 2 | `error:`（`ok:`・`record:` が無い） |
| リビジョンを読むコマンドが無い PATH で既定の走査 | 2 | `error: cannot resolve the source commit` |
| 同じ PATH で `--crate-dir` の走査 | 0 | `record: commit=-` |

変異体は回帰テストの中で作り、リポジトリにファイルとして置かない。足す場所が写しに見つからない場面は NG になる。

### 手元の実行

base `eaf2391` の上で、作業ツリーの変更ありのときの実行（変更ありは `-dirty` が付く）。

```text
record: commit=eaf239111091fcaa8a16e092dd0444f71d215faf-dirty rs_files=15 build_rs=no env_fs=0
```

回帰テストの最終行。

```text
passed=34 failed=0
```

### 確かめ方

```bash
make exec-surface-check
make test-exec-surface-check
```
