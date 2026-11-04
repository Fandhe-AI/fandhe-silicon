# 下-2 の依存の向きの検査（TASK-259）の所在と現状の事実

下-2 の crate（`crates/exec`）が、製品の他の層の crate やチップの SDK の crate に依存していないことを
機械で確かめる検査の所在と、本リポの現状の事実をまとめる。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-259。ポインタのみ）。
spec の本文・決定の内容は本頁へ転記しない。
`upper-layer-check`・`core-deps-check` とは対象の crate が違う。

## 検査の本体（1/4）

### 所在

| 項目 | 内容 |
| ---- | ---- |
| スクリプト | `scripts/exec-deps-check.sh`（bash + python3。Python 3.8 以上の標準ライブラリだけ） |
| 呼び出し元 | `make exec-deps-check`（`make deny` には含めない） |
| 入力のコマンド | `cargo metadata --locked --offline --no-deps --format-version 1 --all-features` と `cargo tree --locked --offline -p <対象> --all-features -e normal,build,dev --target all --prefix depth` |
| 依存の追加 | なし（外部の crate・Rust のコードの変更なし） |

### 使い方

| 呼び方 | 動き |
| ------ | ---- |
| 引数なし | git で HEAD のコミットを取り、cargo の 2 つのコマンドを流して検査する |
| `--metadata-json <path> --tree-text <path>` | 渡した 2 つだけを読む（cargo・git を呼ばない）。ソースは JSON の `manifest_path` が指す crate ディレクトリから読む |

片方だけの指定、同じオプションの重複、未知のオプション（`-h` を含む）は終了コード 2。

### 終了コード

| 終了コード | 意味 |
| ---------- | ---- |
| 0 | 合格 |
| 1 | 違反あり |
| 2 | 誤用・前提の不足（python3・cargo・git が無い、cargo の失敗、metadata が JSON でない・形が崩れている、tree の行が読めない・深さ 0 が対象でない、対象が一意でない、`.rs` が 0 件、UTF-8 でない、閉じていないコメント・文字列、シンボリックリンク、想定外の例外） |

cargo が失敗したときに検査を飛ばして 0 にしない。

### 照合の規則

| 検査 | 動き | `check=` |
| ---- | ---- | -------- |
| (a) 依存の宣言 | 対象の宣言された依存（通常・build・dev。`target` 付き・optional を含む。`package =` の改名は実体の名前）のうち、許す名前以外を違反にする。許す名前でも path が製品の `crates/core` を指さないものは別の違反にする | `dep-allow`・`dep-source` |
| (b) 依存グラフ | tree を (深さ, 名前) の集合として読む。深さ 0 が対象でなければ誤用。深さ 1 の名前が許す名前以外なら違反。深さ 1 以上に禁じる名前が出れば違反。禁じる名前に当たらない外部の crate は判定しない | `tree-direct`・`tree-forbidden` |
| (c) ソースの検索 | 対象の crate ディレクトリ配下のすべての `.rs` を、コメント・文字列・文字リテラルを除いて識別子に分け、禁じる名前と一致するものを位置に関係なく数える（ローカルの同名も数える） | `source-name` |

禁じる名前は、製品の workspace の member のうち対象と共通の crate 以外のすべて（metadata から実行時に作る）と、チップの crate の名前。

### 出力の読み方

| 行 | 出力先 | 内容 |
| -- | ------ | ---- |
| `scanned:` | 標準出力 | 読んだ入力ごと（metadata・tree・source） |
| `record:` | 標準出力 | 数え終えたとき（終了コード 0 と 1）だけ。コミットと各件数 |
| `ok:` | 標準出力 | 合格時の 1 行 |
| `violation:` | 標準エラー | 違反 1 件ごと。`check=` と名前（ソースは crate ディレクトリからの相対パスと行） |
| `FAILED:` | 標準エラー | 違反の件数（終了コード 1） |
| `error:` | 標準エラー | 誤用・前提の不足の理由（英語。終了コード 2） |

ホストの絶対パスと環境変数の値は出力に出さない。

### 数え方の定義の所在

対象の位置・許す依存・チップの crate の名前・依存の種類・名前の折りたたみ・cargo の引数・走査の拡張子は、
スクリプトの Python 部の先頭の「数え方の定義」ブロックにだけ置く。
値は `grep -n -A 40 '数え方の定義（' scripts/exec-deps-check.sh` で確かめる（本頁には写さない）。

### 検査しないもの

- マクロが生成するパス
- `include!`・`#[path]` で crate ディレクトリの外から読むファイル
- `target/` と隠しディレクトリ
- resolve による feature の統一
- 対象の feature の形
- `unsafe` の有無

節 4 の tree は実物の `cargo tree` の出力を通す。tree の読み取りは `[dev-dependencies]`・`[build-dependencies]` のような節の見出し行を区切りとして読み飛ばし、見出しの後の行を判定に流す（節 1 の `tree_header_*` が合成の入力で確かめる）。

### 確かめ方

```bash
make exec-deps-check
scripts/exec-deps-check.sh | grep '^record:'
```

実物の `crates/exec` に当てて、終了コード 0 と `ok:` の行が出る。

## 回帰テストと向きの違う依存の陽性対照（2/4）

### 所在

| 項目 | 内容 |
| ---- | ---- |
| スクリプト | `scripts/test-exec-deps-check.sh` |
| 呼び出し | `make test-exec-deps-check` |
| 前提 | python3・cargo・git（無ければ失敗。skip しない）。製品の依存が取得済みであること以外ネットワーク不要 |
| 書くもの | 一時ディレクトリだけ。リポジトリのファイルは変更しない |

### 節の構成

| 節 | 入力 | 確かめること |
| -- | ---- | ------------ |
| 1 合成の節 | 一時ディレクトリの合成の metadata・tree・ソース（`--metadata-json` と `--tree-text`。cargo・git を呼ばない） | 合格、(a)(b)(c) それぞれの違反、誤用の終了コード 2 |
| 2 ツールが無いとき | PATH を絞った環境 | python3・cargo・git の未導入が終了コード 2 |
| 3 偽のコマンド | PATH の先頭の偽の cargo・git | 引数なしの経路の cargo への引数、cargo・git の失敗や読めない出力が終了コード 2 |
| 4 一時 workspace の実物 | 空の path crate の一時 workspace と実物の cargo metadata / cargo tree | 合格のあと、向きの違う依存を 1 つずつ足すと落ちること |
| 5 実物のリポジトリ | `crates/exec` | 合格（件数の値は固定しない） |

違反と誤用のケースでは `ok:` と（誤用では）`record:` が出ないこと、ホストの絶対パスが出ないことも見る。

### 陽性対照

節 4 で exec の Cargo.toml（または core の Cargo.toml、ソース）に足す内容と、落ちる検査・出る `check=` は次のとおり。

| 足す内容 | 落ちる検査 | 出る `check=` |
| -------- | ---------- | ------------- |
| `[dependencies]` に契約の crate | (a)(b) | `dep-allow`・`tree-direct`・`tree-forbidden`（深さ 1） |
| `[dev-dependencies]` に契約の crate | (a)(b) | 同上（`kind=dev`） |
| `[build-dependencies]` にチップの crate `ash` | (a)(b) | `dep-allow`（`kind=build`）・`tree-direct`・`tree-forbidden`（深さ 1） |
| `[dependencies]` に上の層の crate | (a)(b) | `dep-allow`・`tree-direct`・`tree-forbidden`（深さ 1） |
| `[target.'cfg(target_os = "macos")'.dependencies]` にチップの crate `objc2-metal` | (a)(b) | `dep-allow`・`tree-direct`・`tree-forbidden`（深さ 1。Linux のホストでも落ちる） |
| `[dev-dependencies]` に `plain-dummy` | (a)(b) | `dep-allow`・`tree-direct`（`tree-forbidden` は出ない） |
| core の `[dependencies]` に `ash`（exec の宣言は変えない） | (b) | `tree-forbidden`（深さ 2） |
| 宣言を変えず exec のソースに `use fandhe_silicon_contract;` | (c) | `source-name` |

### 確かめ方

```bash
make test-exec-deps-check
```

最後の行が `passed=<n> failed=0` で、終了コードが 0 になる。

## CI への組み込み（3/4）

TASK-259 の 3/4 で、`ci.yml` の `deny` ジョブ（check 名 `make deny (cargo-deny)`）に次の 2 つのステップを足した。

| ステップ | 実行するもの |
| -------- | ------------ |
| `make exec-deps-check` | `scripts/exec-deps-check.sh` |
| `make test-exec-deps-check` | `scripts/test-exec-deps-check.sh` |

- 位置は `make test-core-deps-check` のステップの直後で、`make deny` のステップより後
- `continue-on-error`・`|| true`・ステップの `if:` は付けていない。検査が落ちればジョブも落ちる
- 新しいジョブは足していない
- ジョブのログには、`make exec-deps-check` の `scanned:`・`record:`・`ok:` の行と、回帰テストの最後の `passed=<n> failed=0` の行が出る

### 確かめ方

```bash
grep -n "run: make exec-deps-check\|run: make test-exec-deps-check" .github/workflows/ci.yml
make ci-frame-check
```

## 完了の条件の確かめ（4/4）

TASK-259 の 1/4〜3/4 を入れた後の実物で観測した結果を表にする。記録の正は private spec 側の TASK-259（ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 実物の `crates/exec` に検査が通る | `make exec-deps-check` | 終了コード 0。`record: commit=567e7cda2b7b56c04b2add0995d2cbed74aeab1e rs_files=12 declared_deps=2 other_deps=0 dep_source=0 tree_pkgs=1 tree_direct=1 tree_forbidden=0 source_hits=0`、`ok: 12 .rs file(s), 2 declared dependenc(ies) and 1 tree package(s) checked, other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0` |
| 2 | 宣言された依存の名前と種類 | `cargo metadata --locked --offline --no-deps --format-version 1 --all-features` の `fandhe-silicon-exec` の `dependencies` | `fandhe-silicon-core` が通常（kind は null）と dev の 2 件だけ |
| 3 | 依存グラフ（`cargo tree --locked --offline -p fandhe-silicon-exec --all-features -e normal,build,dev --target all --prefix depth`）の package | 深さと名前 | 深さ 0 が `fandhe-silicon-exec`、深さ 1 が `fandhe-silicon-core`。ほかは 0 件 |
| 4 | ソースの走査 | 行 1 の `record:` の値 | `rs_files=12`、`source_hits=0` |
| 5 | 回帰テスト | `make test-exec-deps-check` | 終了コード 0、最後の行 `passed=110 failed=0`。向きの違う依存の陽性対照は `real_contract_normal`・`real_contract_dev`・`real_chip_build`・`real_upper_normal`・`real_chip_target_macos`・`real_other_dev`・`real_chip_in_core`・`real_source_only` で、クリーンな状態と未コミットの状態は `real_clean`・`real_dirty` |
| 6 | 実物の exec の写しに向きの違う依存を足す | `git archive` の写しを一時ディレクトリで `git init` しコミットしたうえで、`crates/exec/Cargo.toml` に 1 件足して `scripts/exec-deps-check.sh` | `fandhe-silicon-contract` を通常の依存に足した状態は終了コード 1、`violation:` は `check=dep-allow kind=normal`・`check=tree-direct`・`check=tree-forbidden depth=1`。`fandhe-silicon-upper` を dev の依存に足した状態も終了コード 1、`check=dep-allow kind=dev`・`check=tree-direct`・`check=tree-forbidden depth=1` |
| 7 | CI で流れる場所 | `.github/workflows/ci.yml` の行と、`gh run list` | `run: make exec-deps-check` が 359 行目、`run: make test-exec-deps-check` が 362 行目。3/4 を含む `1e1ab40` の run `41620565383` は観測時に in_progress で、ログの `ok:` の行は観測していない |
| 8 | 1/4〜3/4 が変えたファイル | `git show --stat` | #7879（`9edc30f`）は 7 ファイル、#7882（`422787d`）は 8 ファイル、#7884（`1e1ab40`）は 9 ファイル。いずれも `scripts/`・`Makefile`・`.github/workflows/ci.yml`・`.claude/rules/`・文書だけで、`Cargo.toml`・`Cargo.lock`・`deny.toml`・`deps-approved.toml`・`crates/` 配下は 0 件 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-259）。

確かめた base は `567e7cda2b7b56c04b2add0995d2cbed74aeab1e`、`docs/spec` の gitlink は `c7887a1a7f75903d4016ba42e0595ae6cbb2cd0b`。

## 今の値の補い（base 6b35dd9）

上の記録の節は記録時点の値で、書き換えない。今の値は次のとおり。

- `cargo tree --locked --offline -p fandhe-silicon-exec --all-features -e normal,build,dev --target all --prefix depth` の出力に `[dev-dependencies]` のような節の見出し行は出ない（実物を流して確かめた。出力は深さ 0 の 1 行と深さ 1 の 2 行）。検査の本体は、見出し行が出たときは読み飛ばす。上の「cargo tree が種別ごとに見出し行を挟む」という記述は、この出力には当たらない。
- 上の表の項目 7 の「`ok:` の行は観測していない」について、CI の `deny` ジョブのログに `ok:` の行がある（`gh run view --job <id> --log`）。

| run | job | `record:` の `commit=` | `ok:` の行 | `make test-exec-deps-check` の最後の行 |
| --- | --- | ---------------------- | ---------- | ------------------------------------- |
| `41620565383` | `125864898315` | `1e1ab401966c81aaaa12b30a267dbb1c0433ac82` | `ok: 12 .rs file(s), 2 declared dependenc(ies) and 1 tree package(s) checked, other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0` | `passed=110 failed=0` |
| `41623102841` | `125873537161` | `31a50d5c4fbe5913a439ec228779b9ad14b4ff80` | 同じ | `passed=110 failed=0` |

補い（base c79863d）: 今の `make exec-deps-check` は終了コード 0 で、`record: commit=c79863dea276714c8e80030eaf5e3583ffbc849f rs_files=13 declared_deps=2 other_deps=0 dep_source=0 tree_pkgs=1 tree_direct=1 tree_forbidden=0 source_hits=0`、`ok: 13 .rs file(s), 2 declared dependenc(ies) and 1 tree package(s) checked, other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0`。`rs_files` は `crates/exec/tests/route_query.rs` が加わって 12 から 13 になった（`crates/exec/src` は 7 ファイル、`crates/exec/tests` は `common/mod.rs` を含め 6 ファイル）。

## 今の値の補い（base 48025c6）

上の補い（base c79863d）の `rs_files=13` は、いまは `rs_files=15` である。`make exec-deps-check` は終了コード 0 で、`record: commit=48025c6a580dc41e0d04dee4148622fa2b13385e rs_files=15 declared_deps=2 other_deps=0 dep_source=0 tree_pkgs=1 tree_direct=1 tree_forbidden=0 source_hits=0`、`ok: 15 .rs file(s), 2 declared dependenc(ies) and 1 tree package(s) checked, other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0`。`crates/exec/src` は 8 ファイル（`hooks.rs` が加わった）、`crates/exec/tests` は `common/mod.rs` を含め 7 ファイル（`fault_injection_ports.rs` が加わった）。`declared_deps=2` のまま。

```text
make exec-deps-check
find crates/exec/src crates/exec/tests -name '*.rs'
```

## 今の値の補い（base f4b9d8f）

4/4 の表の 7 の `ci.yml` の行番号は、今の `.github/workflows/ci.yml` では `run: make exec-deps-check` が 368 行目、`run: make test-exec-deps-check` が 371 行目（`grep -n 'run: make exec-deps-check\|run: make test-exec-deps-check' .github/workflows/ci.yml` の出力）。

## 今の値の補い（base 2373576）

記録時点の表と既存の節は書き換えない。以下は base `2373576` で観測した今の値。

- 「今の値の補い（base 48025c6）」の `rs_files=15` は、いまは `rs_files=16`。`crates/exec/src` は 8 ファイル、`crates/exec/tests` は `common/mod.rs` を含め 8 ファイル（`route_evidence_calls.rs` が加わった）。`make exec-deps-check` は終了コード 0 で、`declared_deps=2` のまま。

```text
record: commit=237357677e4f69913c2ecf50adf2e9448f42d32b rs_files=16 declared_deps=2 other_deps=0 dep_source=0 tree_pkgs=1 tree_direct=1 tree_forbidden=0 source_hits=0
ok: 16 .rs file(s), 2 declared dependenc(ies) and 1 tree package(s) checked, other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0
```
