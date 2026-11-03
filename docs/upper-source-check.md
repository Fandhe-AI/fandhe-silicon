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

## 関数ごとの契約の呼び出しと失敗の構築の走査（2/4）

1/4 の 2 つの走査に、関数の表の各関数の本体を見る 2 つの走査を足した。`scripts/upper-source-check.sh` の同じ実行で 4 つが流れる。終了コード 0 は 4 つの走査がすべて合格したときで、1 は 4 つのどれかの違反。

### 使い方の追加と終了コード 2 の追加の理由

| 呼び方 | 動作 |
| ------ | ---- |
| `--queue-trait-file <path>` | 契約の操作の名前を読む trait のファイルを差し替える（既定は `crates/core/src/execution/submit.rs`）。`--crate-dir` と組み合わせられる |

終了コード 2 に次が加わる: 閉じていないコメント・文字列・文字のリテラル、関数の表の関数が 0 件または 2 件以上、関数の本体が無い（`{` の前に `;` が来る）、本体が閉じない、trait のファイルが無い・シンボリックリンク・UTF-8 でない、trait が 1 件でない、trait の操作が 0 件、関数の表の操作が trait に無い。

### 関数の表

関数の名前（`Queue::`・`CommandEncoder::` の関数。impl の型は照らさず、名前が `src/` 全体でちょうど 1 件であることを求める）と、本体で期待する契約の呼び出しの数。行を足すと関数が増える。

| 関数 | 期待する契約の呼び出し |
| ---- | ---------------------- |
| `create_command_encoder` | `create_encoder` を 1 |
| `submit` | `submit` を 1 |
| `begin_compute_pass` | なし |
| `finish` | なし |

書いていない操作の期待は 0。

### 契約の操作の名前の取り方

core の `CommandQueue` trait の本体の直下にある `fn` の名前を、実行のたびに trait のファイルから読む（`cfg` で複数ある名前は 1 つ。既定の本体の中の入れ子の `fn` は数えない）。名前の一覧は頁に並べない。件数は `record:` の `contract_ops=` に出る。trait の位置は次で確かめる。

```bash
grep -n 'pub trait CommandQueue' crates/core/src/execution/submit.rs
```

見つかった行から対応する `}` までの、一段目の `fn` が名前になる。

呼び出しは `.` または `::` の直後（空白・改行は任意）に名前、続けて `(`（空白は任意）の形を数える。`Device` の操作と上の層の補助（`from_contract` など）は trait の名前ではないので数えない。

### 本体の取り方とマスク

`src/` 以下の `.rs`（`build.rs` を除く）から、次を同じ長さの空白に置き換えた写し（改行は残す）を作り、`fn <名前>` の後の最初の `{`（括弧 `()` `[]` の外）から対応する `}` までを本体とする。(3)(4) はこの写しの本体だけで数える。

| 対象 | 扱い |
| ---- | ---- |
| `//`（doc コメントを含む）・入れ子の `/* */` | 空白にする |
| `"…"`（`\` の逃がし）・`b"…"`・`c"…"` | 空白にする |
| `r"…"`・`r#"…"#`・`br#"…"#`・`cr"…"`（`#` の数を数える） | 空白にする |
| `'x'`・`'\n'`・`'\''`・`'\u{…}'`・`b'x'` | 空白にする |
| 寿命（`'d`・`'static`・`'_`）・`r#ident` | そのまま残す |

### 失敗の構築の字句

| 字句 | 数え方 |
| ---- | ------ |
| `Err(` | 識別子としての `Err` に続く `(`（空白は任意） |
| `panic!`・`unreachable!`・`todo!`・`unimplemented!` | 識別子に続く `!`（空白は任意） |
| `assert!`・`assert_eq!`・`assert_ne!`・`debug_assert!`・`debug_assert_eq!`・`debug_assert_ne!` | 同上 |
| `.unwrap(`・`.expect(` | `.` と名前と `(` の間の空白は任意 |

期待は関数の表のどの本体でも 0 件。表に無い関数と `build.rs` には当てない。

### 出力の読み方の追加

| 出力 | 形 |
| ---- | -- |
| `record:` の追加の欄 | `table_fns`（関数の表の行数）・`contract_ops`（trait から読んだ操作の数）・`contract_calls`（表の関数の本体での操作の呼び出しの合計）・`contract_call_mismatch`（期待と違った（関数, 操作）の組の数）・`dispatch_ex`（表の関数の本体での `dispatch_ex` の呼び出しの合計）・`failure_constructs`（失敗の構築の合計） |
| 標準エラー | `violation: check=contract-call fn=<関数> op=<名前> expected=<n> found=<n>`、`violation: check=failure-construct fn=<関数> line=<ファイルの行> word=<字句>` |

`FAILED:` の件数は 4 つの走査の違反の合計。`commit=` は `--crate-dir`・`--queue-trait-file` のどちらかを渡すと `-`。

### 回帰テストの場面の追加

| 場面 | 期待の終了コード | 見る出力 |
| ---- | ---------------- | -------- |
| 実物の `crates/upper` | 0 | `table_fns=4`・`contract_call_mismatch=0`・`dispatch_ex=0`・`failure_constructs=0` |
| 契約の呼び出しの変異体（`create_encoder` をもう 1 回・`dispatch_ex`・`gpu_timing`・`::` の形・空白を挟んだ形・`.submit(` を別の名前へ） | 1 | `violation: check=contract-call` の `fn=`・`op=`・`expected=`・`found=` |
| 失敗の構築の変異体（10 字句を、表の関数のどれかの本体に 1 本ずつ） | 1 | `violation: check=failure-construct` の `fn=`・`word=` |
| 陰性の対照（表の外の関数、表の関数のコメント・文字列、`'{'`・`"}"`、ダミーの操作を足した trait の写し、入れ子の `fn`） | 0 | `ok:`・`failure_constructs=0`・`contract_call_mismatch=0` |
| trait の写しにダミーの操作を足し、本体でその操作を呼ぶ | 1 | `op=probe_op expected=0 found=1` |
| 誤用（関数が 0 件・2 件・本体が無い、閉じていないコメント・文字列・文字のリテラル、trait のファイルの欠落・trait 無し・操作 0 件・閉じない・表の操作が無い・UTF-8 でない・シンボリックリンク、値の無い `--queue-trait-file`） | 2 | `error:`、`ok:`・`record:` が無い、ホストのパスが出ない |

### 手元の実行の記録

時点のコミット `5b10b82`（作業ツリーの変更ありのため `commit=` は `-dirty` 付き）。

```text
record: commit=5b10b827dffc-dirty rs_files=5 build_rs=no global_state=0 env_fs=0 table_fns=4 contract_ops=12 contract_calls=2 contract_call_mismatch=0 dispatch_ex=0 failure_constructs=0
passed=71 failed=0
```

### 確かめ方

```bash
make upper-source-check
make test-upper-source-check
```

## CI への組み込み（3/4）

`ci.yml` の `deny` ジョブ（check 名 `make deny (cargo-deny)`）に、本体と回帰テストの 2 ステップを足した。新しいジョブは足していない。

| ステップ | 流すもの | 位置 |
| -------- | -------- | ---- |
| `make upper-source-check` | `scripts/upper-source-check.sh` | `make upper-layer-check` のステップの直後（`make deny` のステップより後） |
| `make test-upper-source-check` | `scripts/test-upper-source-check.sh` | `make test-upper-layer-check` のステップの直後 |

- 2 ステップとも `continue-on-error`・`|| true`・ステップの `if:` を付けていない。検査の失敗はジョブの失敗になる
- `ci-complete` の `needs` と ruleset の check 名は変えていない。`wasm-macos`・`verify-macos` には足していない
- ジョブのログには本体の `scanned:`・`record:`・`ok:` の行と、回帰テストの最後の `passed=<n> failed=0` の行が出る

### 確かめ方

```bash
grep -n "run: make upper-source-check\|run: make test-upper-source-check" .github/workflows/ci.yml
make ci-frame-check
```
