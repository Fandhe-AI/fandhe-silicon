# 上の層のソースの走査（TASK-2387）の所在と本リポの現状

上の層の crate（`crates/upper`）のソースを字句で数えるスクリプトについて、本リポのスクリプトとテストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2387。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
関連する既存の頁は [上の層の検査（TASK-15）](./upper-layer-check.md)・[上の層の compute の経路（TASK-2386）](./upper-compute-path.md)。

## 環境変数・ファイルの読み取りとグローバルな状態の走査（1/4）

### 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 本体 | `scripts/upper-source-check.sh`（bash + `python3 -I`、標準ライブラリのみ。cargo を呼ばない） | `make upper-source-check` |
| 回帰テスト | `scripts/test-upper-source-check.sh`。実物の `crates/upper` を一時ディレクトリへ写して字句を 1 か所足した変異体、陰性の対照、誤用の入力、実物の合格を確かめる。リポジトリのファイルを変更しない・ネットワーク不要 | `make test-upper-source-check` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

### 使い方と終了コード

| 呼び方 | 動作 |
| ------ | ---- |
| 引数なし | `crates/upper` を走査する |
| `--crate-dir <dir>` | `<dir>` の crate を走査する |

| コード | 意味 |
| ------ | ---- |
| 0 | 2 つの走査がどちらも 0 件 |
| 1 | 違反あり |
| 2 | 誤用・前提の不足・読み切れない入力（未知の引数、crate や `src/` が無い、`src/` の `.rs` が 0 件、UTF-8 でない、シンボリックリンク、`use std::{` の対応する `}` が無い、python3 が無い） |

### 出力の読み方

| 出力 | 形 |
| ---- | -- |
| 標準出力 | 読んだファイルごとに `scanned: file=<crate からの相対パス>`。数え終えたら `record: commit=<rev または -> rs_files=<n> build_rs=<yes|no> global_state=<n> env_fs=<n>`。合格なら `ok: upper source scan passed` |
| 標準エラー | 違反ごとに `violation: check=<global-state|env-fs> file=<相対パス> line=<n> word=<字句>`。違反があれば `FAILED: <n> violation(s)` |

### 走査の対象のファイル

| 対象 | 内容 |
| ---- | ---- |
| 見る | `src/` 以下を辿って見つかったすべての `.rs`（`mod` の宣言の有無と `#[cfg(test)]` の中を問わない）と、crate 直下の `build.rs` |
| 見ない | `tests/`・`benches/`・`examples/`・`target/`・`.` 始まりのディレクトリ |

### 字句

数えるのは各ファイルの生のテキストで、コメント・doc コメント・文字列も含む。

| 走査 | 字句 | 数え方 |
| ---- | ---- | ------ |
| グローバルな状態 | `static` | 識別子として（前後が識別子の文字でない）。`'static` も数える |
| グローバルな状態 | `thread_local!` | `thread_local` に続く `!`（空白は任意） |
| グローバルな状態 | `Mutex` | 識別子として（`MutexGuard` は数えない） |
| 環境変数・ファイル | `std::env`・`env::var`・`option_env!`・`env!`・`std::fs`・`File::open` | 部分一致をそれぞれ独立に数える（重なりは引かない） |
| 環境変数・ファイル | `use std::{…}` の中の `env`・`fs` | 入れ子の `{…}` を含めて、識別子として現れる段を数える |

### 回帰テストの場面

| 場面 | 期待の終了コード | 見る出力 |
| ---- | ---------------- | -------- |
| 実物の `crates/upper` | 0 | `ok:`・`global_state=0 env_fs=0` |
| 変異体（字句ごと・コメント・doc コメント・文字列・`#[cfg(test)]`・新しいファイル・`build.rs`） | 1 | `violation:` の `check=`・`word=`・`file=`、`FAILED:`、`ok:` が無い |
| 陰性の対照（`tests/`・`benches/`・`examples/`・`.` 始まりのディレクトリ・識別子の一部の一致） | 0 | `ok:` |
| 誤用・前提の不足 | 2 | `error:`、`ok:`・`record:` が無い |
| リビジョンを読む外部コマンドが無い | 0 | `commit=-` |

### 手元の実行の記録

時点のコミット `be4f598`（作業ツリーの変更ありのため `commit=` は `-dirty` 付き）。

```text
record: commit=be4f5984dc30-dirty rs_files=5 build_rs=no global_state=0 env_fs=0
passed=33 failed=0
```

### 確かめ方

```bash
make upper-source-check
make test-upper-source-check
```
