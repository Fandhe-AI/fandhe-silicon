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

## 既定の feature の公開の面にある試験用の口の名前の走査（2/6）

`crates/exec` の rustdoc の JSON を feature の組ごとに作り、試験用の口と値の名前を数える。既定の feature の JSON は 0 件、`--all-features` の JSON は各名前が 1 件以上であることを期待する。同じスクリプト（`scripts/exec-surface-check.sh`）の走査で、make ターゲットと回帰テストは 1/6 と同じ。

### 使い方の追加

| 呼び方 | 動き |
| ------ | ---- |
| 引数なし、または `--crate-dir <dir>` | ソースの走査の後に、cargo で JSON を 2 組作って名前を数える |
| `--json-default <path> --json-all <path>` | 渡した 2 つの JSON だけで名前を数え、cargo を呼ばない（2 つ揃えて 1 回ずつ。`--crate-dir` と併用できる） |

コミットは 1/6 と同じ規則で、ソースの走査先が既定の `crates/exec` のときだけ HEAD、`--crate-dir` のときは `-`。

終了コード 2 の項目の追加: `--json-default` と `--json-all` の片方だけ・重複、cargo・rustdoc が使えない、JSON を作れない、JSON が読めない・形が崩れている（トップが object でない、`index`・`paths`・`external_crates` が object でない、`root` が無い、`includes_private` が false でない、`format_version` が整数でない、`crate_id` が整数でないアイテム、`index[root]` の名前が `fandhe_silicon_exec` でない）。

### JSON を作る組

| 組 | cargo の feature の引数 | 期待 |
| -- | ----------------------- | ---- |
| `default` | なし | 名前の合計 0 件 |
| `all-features` | `--all-features` | 名前の一覧の各名前が 1 件以上 |

- コマンドの形: `cargo rustdoc --locked --offline --target-dir <target_directory>/exec-surface-check -p fandhe-silicon-exec [--all-features] --lib -- -Z unstable-options --output-format json --document-hidden-items`
- `RUSTC_BOOTSTRAP=1` は `rustdoc -Z unstable-options --version` の確認と `cargo rustdoc` の子プロセスにだけ渡す。子プロセスの出力は出さない。
- 組ごとに古い JSON を消してから流し、出力の存在を確かめて `doc/fandhe_silicon_exec.<組>.json` へ移す。

### 名前の一覧

`crate_id` が 0 のすべてのアイテム（種類と visibility を問わない）の `name` を、一覧の名前と完全一致で数える。`use` のアイテムは `inner.use.name` も見る。1 つのアイテムは 1 回だけ数える。

| 名前 | `--all-features` の JSON での所在 |
| ---- | ------------------------------ |
| `force_unsupported` | `Exec::force_unsupported` |
| `inject_denormal` | `Exec::inject_denormal` |
| `ForcedUnsupported` | `UnavailableReason::ForcedUnsupported` |
| `DenormalInjection` | enum |
| `Injected` | `F32DenormalBasis::Injected` |

### 出力の追加

| 出力 | 内容 |
| ---- | ---- |
| `scanned: features=<default\|all-features> items=<n>` | 組ごとの `crate_id` 0 のアイテムの数 |
| `record:` の追加の欄 | `items_default`・`items_all`・`port_names_default`・`port_names_all`・`port_names_missing_all`（all-features で 0 件の名前の数） |
| `violation: check=port-name features=default name=<名前> count=<n>` | 標準エラー。default で 1 件以上の名前ごと |
| `violation: check=port-name-positive features=all-features name=<名前>` | 標準エラー。all-features で 0 件の名前ごと |

### 回帰テストの場面の追加

1/6 の `--crate-dir` の場面は、合格する合成の JSON を添えて流す（期待は同じ）。

| 場面 | 期待の終了コード | 見る出力 |
| ---- | ---------------- | -------- |
| 実物の引数なしの流し | 0 | `port_names_default=0`・`port_names_missing_all=0`・組ごとの `scanned: features=` |
| 合成の合格（default に名前なし、all-features に 5 つ） | 0 | `ok:`・`port_names_default=0` |
| default に名前を 1 つ足す（関数・variant・enum・欄・`use`） | 1 | `check=port-name features=default name=<名前> count=1` |
| all-features から 1 つの名前を消す | 1 | `check=port-name-positive`・`port_names_missing_all=1` |
| `crate_id` が 1 または 2 の同名、`force_unsupported_count`・`MyInjected` | 0 | `port_names_default=0` |
| `--json-default` だけ・`--json-all` だけ・重複 | 2 | `error:`（`record:`・`ok:` が無い） |
| 無いファイル、JSON でない、トップが配列、`includes_private` が true、`format_version` が文字列、`crate_id` の無いアイテム、root が別の crate の名前 | 2 | `error:`（`record:`・`ok:` が無い） |
| cargo の無い PATH で `--crate-dir` だけ | 2 | `error: cannot run cargo` |

### 手元の実行

base `c956c96` の上で、作業ツリーの変更ありのときの実行。

```text
record: commit=c956c962f0ae50300b4c7b8fe32ca95de2001d3b-dirty rs_files=15 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0
```

回帰テストの最終行。

```text
passed=55 failed=0
```

### 確かめ方

```bash
make exec-surface-check
make test-exec-surface-check
```

## 操作を実行する関数の戻り値の型と一覧に無い公開の関数の走査（3/6）

default の組の rustdoc の JSON だけに当てる。同じスクリプト・同じ make ターゲットで流れる（all-features の組には当てない）。

### 一覧

| 一覧 | 内容 |
| ---- | ---- |
| 操作の関数（持ち主 `Exec`） | `f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add`・`external_interop_shared_memory`・`hardware_ray_tracing`・`unified_memory_sharing` |
| 報告の型の名前 | `RouteReport` |
| 操作でない関数（持ち主, 名前） | `Exec::new`・`Exec::deny_by_default`・`Exec::default_opt_in`・`Exec::query_route`・`Outcome::report` |

### 公開の関数の集め方

`crate_id` 0 の `function` で `visibility` が `public` のアイテム。持ち主は inherent の impl（`trait` が無いもの）の対象の型の名前で、impl に属さなければ `-`。trait の impl の関数は集めない。`#[doc(hidden)]` の関数も含む（JSON は `--document-hidden-items` つき）。

### 報告を持つ型の判定

戻り値が `Result<T, _>`（`core::result::Result` または `std::result::Result`）で、`T` が次のどれかを満たす。`Err` の側は見ない。

| 型 | 条件 |
| -- | ---- |
| 報告の型そのもの | `crate_id` 0 で名前が `RouteReport` |
| enum | variant が 1 つ以上あり、すべての variant が報告の型の欄を 1 つ以上持つ |
| struct | 報告の型の欄を 1 つ以上持つ（`plain` または `tuple`） |

持たない形: `unit` の struct、欄の無い variant、`Option<RouteReport>` など包んだ形の欄、外部の crate の型。

### 判定と出力

| 判定 | 標準エラーの形 |
| ---- | -------------- |
| 操作の関数が `Exec` にちょうど 1 件ではない | `violation: check=op-fn-count fn=<名前> found=<n>` |
| 操作の関数の戻り値が報告を持たない | `violation: check=op-fn-report fn=<名前>` |
| 上の 2 つの一覧のどちらにも無い公開の関数 | `violation: check=unlisted-fn owner=<型 または -> fn=<名前>` |

`record:` に `op_fns`・`op_fns_with_report`・`other_fns`・`unlisted_fns` の 4 つの欄を足す。操作でない一覧の関数の戻り値は見ない。

終了コード 2 に加わるもの: `Exec` が JSON に無い、戻り値の型の path を解決できない、impl の対象を解決できない、関数の sig の形が崩れている。

### 回帰テストの場面の追加

1/6・2/6 の合成の default の JSON は、この節の合格の形の面（`Exec` と操作の関数 7 つ・`Outcome`・`RouteReport`）を持つ。

| 場面 | 期待の終了コード | 見る出力 |
| ---- | ---------------- | -------- |
| 合成の合格 | 0 | `op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0` |
| 戻り値が `Result<(), _>`、または `Outcome` そのまま | 1 | `check=op-fn-report` |
| `Outcome` に欄の無い variant、`u32` だけの tuple の variant | 1 | 7 つすべてに `check=op-fn-report` |
| `Exec` に一覧に無い公開の関数（`doc(hidden)` つきを含む）、自由な公開の関数 | 1 | `check=unlisted-fn` |
| 操作の関数を 1 つ消す、同じ名前を 2 つにする | 1 | `check=op-fn-count` の `found=0`・`found=2` |
| trait の impl の関数、tuple の形の variant で報告を持つ enum、`Result<RouteReport, _>`、報告の欄を持つ struct、`std::result::Result` | 0 | `ok:` |
| `Exec` が無い、`Result` の id を解決できない | 2 | `error:`（`record:`・`ok:` が無い） |

### 手元の実行

base `18d8f91` の上で、作業ツリーの変更ありのときの実行。

```text
record: commit=18d8f9122de3f77967a6e5021b58d091a23fb761-dirty rs_files=15 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
```

回帰テストの最終行。

```text
passed=72 failed=0
```

### 確かめ方

```bash
make exec-surface-check
make test-exec-surface-check
```

## 一時 workspace を走査する口と実物の下-2 を写した変異体（4/6）

TASK-923 のポインタ。本体に `--workspace-dir <dir>` を足し、回帰テストに実物の `crates/core`・`crates/exec` を写した一時 workspace の変異体を足す。

### 使い方の追加

```text
scripts/exec-surface-check.sh --workspace-dir <dir>
```

- `<dir>` を cargo の作業ディレクトリにして rustdoc の JSON を 2 組作り、ソースの走査は `<dir>/crates/exec` に当てる。`record:` の `commit` は `-`
- `--crate-dir`・`--json-default`・`--json-all` との併用、`--workspace-dir` の重複、`<dir>` が無い、`<dir>` の workspace に `fandhe-silicon-exec` が無い、は終了コード 2

### 一時 workspace の作り方

| 写すもの | 内容 |
| -------- | ---- |
| `crates/core`・`crates/exec` | `target` と `.` 始まりを除いて丸ごと写す |
| ルートの `Cargo.toml` | `members` を 2 crate にし、`exclude` の行を消す |
| `rust-toolchain.toml` | 作業ディレクトリが変わっても同じ toolchain で JSON を作る |
| `Cargo.lock` | 写さず `cargo metadata --offline` で作る |

target は一時ディレクトリの下に分ける。変異は 1 本ずつ入れ、流した後に元のファイルへ戻す。

### 変異体

| 変える場所 | 変え方 | 期待の終了コード | 見る `check=` |
| ---------- | ------ | ---------------- | -------------- |
| `Cargo.toml` | `default = ["fault-injection"]` を足す | 1 | `port-name`・`unlisted-fn` |
| `src/route.rs` | 試験用の variant の cfg を外す | 1 | `port-name` |
| `src/lib.rs` | `doc(hidden)` の `Injected` 型を足す | 1 | `port-name` |
| `src/entry.rs` | `doc(hidden)` の公開の関数を足す | 1 | `unlisted-fn` |
| `src/entry.rs` | 操作の関数の戻り値を `Result<(), _>` にする | 1 | `op-fn-report` |
| `src/report.rs` | `Outcome` に欄の無い variant を足す | 1 | `op-fn-report` |
| `src/select.rs` | `std::env::var` を呼ぶ関数を足す | 1 | `env-fs` |

変異のない写しは終了コード 0 で、`record:` の `commit` 以外の欄が実物の流しと一致する。

### 誤用

| 場面 | 期待の終了コード |
| ---- | ---------------- |
| 写しの `lib.rs` に構文の誤りを足す（`cargo rustdoc failed (default)`） | 2 |
| `members` に exec が無い workspace | 2 |
| `--json-default`/`--json-all`、`--crate-dir` との併用、重複、無いディレクトリ | 2 |

### 手元の実行

base `b851a7e` の上で、変異のない写しの `record:` の行。

```text
record: commit=- rs_files=15 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
```

回帰テストの最終行と所要時間（target が空の状態から）。

```text
passed=86 failed=0
real 20s
```

### 確かめ方

```bash
make exec-surface-check
make test-exec-surface-check
```

## CI への組み込み（5/6）

`ci.yml` の `deny` ジョブ（check 名 `make deny (cargo-deny)`）に、本体と回帰テストの 2 ステップを足した。新しいジョブは足していない（TASK-923）。

| ステップ | 流すもの | 位置 |
| -------- | -------- | ---- |
| `make exec-surface-check` | `scripts/exec-surface-check.sh` | `make exec-deps-check` のステップの直後 |
| `make test-exec-surface-check` | `scripts/test-exec-surface-check.sh` | `make test-exec-deps-check` のステップの直後 |

- 2 ステップとも `continue-on-error`・`|| true`・ステップの `if:` を付けていない。検査の失敗はジョブの失敗になる
- `ci-complete` の `needs` と ruleset の check 名は変えていない。`wasm-macos`・`verify-macos` には足していない
- ジョブのログには本体の `scanned:`・`record:`・`ok:` の行と、回帰テストの最後の `passed=<n> failed=0` の行が出る

### 確かめ方

```bash
grep -n "run: make exec-surface-check\|run: make test-exec-surface-check" .github/workflows/ci.yml
make ci-frame-check
```

### 手元の実行

base 2467833 で流した。本体の `record:`・`ok:` の行と、回帰テストの最後の行。

```text
record: commit=2467833ffd52-dirty rs_files=15 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
passed=86 failed=0
```

## 完了の条件の確かめ（6/6）

1/6〜5/6 の走査・回帰テスト・CI への組み込みを、手元の実行と main の push の CI の出力で確かめた。結果の欄は出力をそのまま写した。

- 手元: `origin/main` の `5b39cae`、`docs/spec` の gitlink は `c7887a1a7f75903d4016ba42e0595ae6cbb2cd0b`、`id -u` は `1000`
- CI: run `41739470984`（headSha `5b39caeefdf7009eb11c4e556a2c0277167077f8`、conclusion `success`）のジョブ `make deny (cargo-deny)`（job `126277060587`）

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 既定の feature の公開の面の名前 | `make exec-surface-check` の `record:` | `items_default=458` `port_names_default=0` `items_all=511` `port_names_all=6` `port_names_missing_all=0` |
| (2) | ソースの字句 | 同じ `record:` | `rs_files=15` `build_rs=no` `env_fs=0` |
| (3) | 操作を実行する関数の戻り値と一覧に無い公開の関数 | 同じ `record:` | `op_fns=7` `op_fns_with_report=7` `other_fns=5` `unlisted_fns=0` |
| (4) | 陽性対照 | `make test-exec-surface-check` の最終行と、場面の群ごとの数 | `passed=86 failed=0`。群ごとの数は下の表 |
| (5) | CI で流れる場所 | `grep -n` の出力と、上の run のジョブのログの行 | 下のブロック |
| (6) | 5 つの PR の `crates/`・`Cargo.toml`・`Cargo.lock`・`deps-approved.toml`・`deny.toml` の変更 | 各マージコミット `<c>^..<c>` に `git diff --stat <c>^..<c> -- crates Cargo.toml Cargo.lock deps-approved.toml deny.toml`（`c956c96`・`18d8f91`・`b851a7e`・`2467833`・`5b39cae`） | 5 つとも出力なし |

(1)〜(3) の手元の実行（終了コード 0）:

```text
record: commit=5b39caeefdf7009eb11c4e556a2c0277167077f8 rs_files=15 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

(4) の手元の実行（終了コード 0。所要 12.212 秒）の最終行:

```text
passed=86 failed=0
```

(4) の場面の群ごとの数。`scripts/test-exec-surface-check.sh` の節の見出しと場面の名前に、出力の `ok:` の行を照らして数えた。

| 群 | 期待の終了コード | 数 |
| -- | ---------------- | -- |
| 実物の合格 | 0 | 1 |
| ソースの字句の変異体 | 1 | 20 |
| ソースの走査の陰性の対照 | 0 | 1 |
| ソースの走査の誤用・前提の不足 | 2 | 11 |
| リビジョンを読むコマンドが無いときの crate dir の走査 | 0 | 1 |
| 公開の面の名前の合成の JSON の合格と陰性の対照 | 0 | 2 |
| 公開の面の名前の合成の JSON の違反 | 1 | 8 |
| 公開の面の名前の合成の JSON の誤用・前提の不足 | 2 | 11 |
| 操作の関数の合成の JSON の合格と陰性の対照 | 0 | 6 |
| 操作の関数の合成の JSON の違反 | 1 | 9 |
| 操作の関数の合成の JSON の誤用 | 2 | 2 |
| 一時 workspace の合格 | 0 | 1 |
| 一時 workspace の変異体 | 1 | 7 |
| 一時 workspace の誤用 | 2 | 6 |
| 合計 | | 86 |

(5) の `grep -n`:

```text
374:        run: make exec-surface-check
377:        run: make test-exec-surface-check
```

(5) の CI のジョブのログ:

```text
make deny (cargo-deny)	make exec-surface-check	2026-11-03T23:34:29.3043787Z record: commit=5b39caeefdf7009eb11c4e556a2c0277167077f8 rs_files=15 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
make deny (cargo-deny)	make exec-surface-check	2026-11-03T23:34:29.3044867Z ok: exec surface check passed
make deny (cargo-deny)	make test-exec-surface-check	2026-11-03T23:34:46.6054075Z passed=86 failed=0
```

(6) の 5 つのマージコミットは、`c956c96` が #7945、`18d8f91` が #7947、`b851a7e` が #7948、`2467833` が #7949、`5b39cae` が #7950。

### 確かめ方

```bash
make exec-surface-check
make test-exec-surface-check
grep -n "run: make exec-surface-check\|run: make test-exec-surface-check" .github/workflows/ci.yml
gh run view 41739470984 --json conclusion,headSha
```

## 段の表

| 段 | Issue・PR | 節 |
| -- | --------- | -- |
| 1/6 | #6013（PR #7945） | 下-2 のソースの環境変数・ファイルの読み取りの走査（1/6） |
| 2/6 | #6014（PR #7947） | 既定の feature の公開の面にある試験用の口の名前の走査（2/6） |
| 3/6 | #6015（PR #7948） | 操作を実行する関数の戻り値の型と一覧に無い公開の関数の走査（3/6） |
| 4/6 | #6016（PR #7949） | 一時 workspace を走査する口と実物の下-2 を写した変異体（4/6） |
| 5/6 | #6017（PR #7950） | CI への組み込み（5/6） |
| 6/6 | #6018 | 完了の条件の確かめ（6/6） |

親 Issue は #867。

## 今の値の補い（base f4b9d8f）

記録時点の表と既存の節は書き換えない。以下は base `f4b9d8f` で観測した今の値。

- 冒頭の文と 1/6 の所在の表: 今の `scripts/exec-surface-check.sh` は、ソースの字句の走査に加えて、既定の feature と all-features の rustdoc の JSON の走査（試験用の口の名前）と、操作を実行する関数の戻り値の走査（`op_fns`・`op_fns_with_report`・`other_fns`・`unlisted_fns`）も行う。`record:` の行は 1 行にこれらの欄を持つ（`make exec-surface-check` の出力）。
- 1/6 の所在の表の「`make deny` には含めない」: 今は `ci.yml` の `deny` ジョブが `make exec-surface-check`（373 行目の `- name:`・374 行目の `run:`）と `make test-exec-surface-check`（376 行目の `- name:`・377 行目の `run:`）を呼ぶ（`grep -n 'exec-surface-check' .github/workflows/ci.yml`）。`make deny` の中身には含まれないまま、ジョブの別のステップとして流れる。
- 2/6 の `port_names_all=6`: 名前は `PORT_NAMES` の 5 つ（`scripts/exec-surface-check.sh` の 82 行目）。数える関数 `count_port_names`（285〜296 行目）は、アイテムの `name` と、`use` アイテムの `inner.use.name` のどちらかが一致すれば 1 件に数える。`DenormalInjection` は `crates/exec/src/report.rs:32` の enum の定義と `crates/exec/src/lib.rs:35` の根の `pub use` の 2 アイテムが一致するので 2 件になり、5 つの名前で合計 6 になる。`make exec-surface-check` の出力は `port_names_default=0 port_names_all=6 port_names_missing_all=0`。
- 5/6 の「手元の実行」の `commit=2467833ffd52-dirty`: これは 12 桁の写し。スクリプトの出力は 40 桁で、今の作業ツリーでは次の行（変更なしのため `-dirty` なし）。

```text
record: commit=f4b9d8f287ad42ef32c20f0fca43294f8f47b6aa rs_files=15 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
```

- 5/6 の CI の出力: main の push の CI の最新の success の run は `41742334323`（headSha `5e75fcca376f10065f5ed219aee607975c3959cc`、conclusion `success`）。確かめのコマンドは次。回帰テストの最終行は `passed=86 failed=0`（`scripts/test-exec-surface-check.sh`）。

```bash
gh run list -R Fandhe-AI/fandhe-silicon --branch main --workflow ci.yml --limit 5
gh run view 41742334323 -R Fandhe-AI/fandhe-silicon --json conclusion,headSha
```

## 今の値の補い（base 2373576）

記録時点の表と既存の節は書き換えない。以下は base `2373576` で観測した今の値。

- 上の記録の `rs_files=15` は、いまは `rs_files=16`。`crates/exec/src` は 8 ファイル、`crates/exec/tests` は `common/mod.rs` を含め 8 ファイル（`route_evidence_calls.rs` が加わった）。`make exec-surface-check` の `record:` の行は次のとおりで、ほかの欄は変わらない。

```text
record: commit=237357677e4f69913c2ecf50adf2e9448f42d32b rs_files=16 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
```

## 今の値の補い（base 904126a）

記録時点の表と既存の節は書き換えない。以下は base `904126a` で観測した今の値。

- 上の記録の `rs_files=16` は、いまは `rs_files=20`。`crates/exec/src` は 9 ファイル（`length.rs` が加わった）、`crates/exec/tests` は `common/mod.rs` を含め 11 ファイル（`length_zero.rs`・`length_limit.rs`・`length_device.rs` が加わった）。`make exec-surface-check` は終了コード 0 で、`record:` の行は次のとおり。ほかの欄は変わらない。

```text
record: commit=904126a006b240f372926d49e3077747ecce4b3d rs_files=20 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

## 今の値の補い（base dbbb5c0）

記録時点の表と既存の節は書き換えない。以下は base `dbbb5c0` で観測した今の値。

- 「今の値の補い（base 904126a）」の `rs_files=20` は、いまは `rs_files=26`。`crates/exec/src` は 13 ファイル（`binary16.rs`・`cpu.rs`・`denormal.rs`・`denormal_tests.rs` が加わった）、`crates/exec/tests` は `common/mod.rs` を含め 13 ファイル（`cpu_f16_fma.rs`・`length_host.rs` が加わった）。`make exec-surface-check` は終了コード 0 で、`record:` の行は次のとおり。ほかの欄は変わらない。

```text
record: commit=dbbb5c01ceb260fa806fb87abdd02d129fdb4690 rs_files=26 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

- `op_fns=7 op_fns_with_report=7` の 7 つの関数のうち `f16_fma` は、経路が `Route::Cpu` で host の形なら `Ok(Outcome::Ran { report })` を返す（`crates/exec/src/entry.rs`）。戻り値の型は `Result<Outcome, ContractError>` のまま。

## 今の値の補い（base 515bb19）

記録時点の表と既存の節は書き換えない。以下は base `515bb19` で観測した今の値。

- 「今の値の補い（base dbbb5c0）」の `rs_files=26` は、いまは `rs_files=27`。`crates/exec/src` は 13 ファイルのまま、`crates/exec/tests` は `common/mod.rs` を含め 13 ファイル → 14 ファイル（`cpu_subgroup_sum.rs` が加わった）。`make exec-surface-check` は終了コード 0 で、`record:` の行は次のとおり。ほかの欄は変わらない。

```text
record: commit=515bb196d9ee162f877a437fc1ffba9223cd84fe rs_files=27 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

- `op_fns=7 op_fns_with_report=7` の 7 つの関数のうち `f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32` は、経路が `Route::Cpu` で host の形なら `Ok(Outcome::Ran { report })` を返す（`crates/exec/src/entry.rs`）。戻り値の型は `Result<Outcome, ContractError>` のまま。

## 今の値の補い（base 8e4b99b）

記録時点の表と既存の節は書き換えない。以下は base `8e4b99b` で観測した今の値。

- 「今の値の補い（base 515bb19）」の `rs_files=27` は、いまは `rs_files=28`。`crates/exec/src` は 13 ファイルのまま、`crates/exec/tests` は `common/mod.rs` を含め 14 ファイル → 15 ファイル（`cpu_atomic_add.rs` が加わった）。`make exec-surface-check` は終了コード 0 で、`record:` の行は次のとおり。ほかの欄は変わらない。

```text
record: commit=8e4b99b9c26f5a4eba683321b8ce41ec0fe4ceaa rs_files=28 build_rs=no env_fs=0 items_default=458 items_all=511 port_names_default=0 port_names_all=6 port_names_missing_all=0 op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0
ok: exec surface check passed
```

- `op_fns=7 op_fns_with_report=7` の 7 つの関数のうち `f16_fma`・`subgroup_sum_u32`・`subgroup_sum_f32`・`atomic_add` の 4 つは、経路が `Route::Cpu` で host の形なら `Ok(Outcome::Ran { report })` を返す（`crates/exec/src/entry.rs`）。戻り値の型は `Result<Outcome, ContractError>` のまま。
