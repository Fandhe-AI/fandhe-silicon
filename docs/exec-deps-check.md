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

### 確かめ方

```bash
make exec-deps-check
scripts/exec-deps-check.sh | grep '^record:'
```

実物の `crates/exec` に当てて、終了コード 0 と `ok:` の行が出る。
