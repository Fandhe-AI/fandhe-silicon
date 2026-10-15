# 一覧と項目の対応を確かめる検査（TASK-16）の所在と現状の事実

private spec 内の 2 つの資料（一覧の CSV と、項目のファイル群）の対応を機械で確かめる検査について、本リポのスクリプトと回帰テストから説明できる振る舞いを記録する。
追随分の一覧を別ファイルとして取り込む拡張と、その取り込みの手順（「追随分の取り込みの手順」の節）は TASK-17 のポインタ（親 Issue #65 の 2/5・4/5）。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-16・TASK-17。ポインタのみ）。spec の本文・件数・列の内容・鍵・項目 ID の具体例は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/inventory-trace-check.sh`（bash + `python3 -I`、標準ライブラリのみ）。入口は `make inventory-trace-check`。既定の入力のパスはスクリプトの中に定数として持つ | `scripts/inventory-trace-check.sh` |
| 回帰テスト | `scripts/test-inventory-trace-check.sh`。入口は `make test-inventory-trace-check`。合成の fixture だけで動き、`docs/spec` の取得は要らない | `scripts/test-inventory-trace-check.sh` |
| CI | `ci.yml` の `deny` ジョブが回帰テストを呼ぶ。検査の本体は呼ばない（入力が private の submodule の中にあり、CI は submodule を取得しない） | `.github/workflows/ci.yml` |
| 入力 | 読むだけ。ファイルを作らず、ネットワークを使わない | `inputs are not modified and no files are created`（回帰テスト） |
| 追随分の一覧 | 既定値を持たず、引数 `--follow-csv` でパスを受ける。置き場は private spec 側の記録に従う（TASK-17 のポインタのみ） | `follow-up: without the option the output is unchanged`（回帰テスト） |
| 依存 | 外部依存を足していない | `Cargo.lock` |
| スクリプトが持たない語 | 列名・鍵の形・項目の行の書式は定数として持たず、引数で受けるか入力から実行時に求める | `scripts/inventory-trace-check.sh` |

## 検査のスクリプト

引数（直接実行で渡す。`make` 経由は既定のパス）:

| 引数 | 既定 | 意味 |
| ---- | ---- | ---- |
| `--csv <path>` | スクリプトの定数の一覧のパス | 先頭のレコードを見出しとして扱う |
| `--items-dir <dir>` | スクリプトの定数の項目のディレクトリ | 直下の `*.md` だけを読む（再帰しない） |
| `--key-column <name>` | 先頭の列 | 鍵の列を見出しの名前で指す。ちょうど 1 つに一致しなければ誤用 |
| `--exclude <basename>` | なし（繰り返し可） | 探索から外すファイル名。`README.md` は常に外す。ディレクトリ直下に無い名前・区切りを含む名前は誤用 |
| `--item-rows` | 無効 | 項目の行（表のデータ行）まで絞って判定する。未指定ならファイル単位 |
| `--item-id-column <name>` | 各表の先頭の列 | 項目の行のラベルの列を見出しの名前で指す。`--item-rows` と併用（単独は誤用）。指定したときは、その名前の見出しがちょうど 1 つある表だけを項目の表として扱う |
| `--follow-csv <path>` | なし（1 回だけ。2 回以上は誤用） | 追随分の一覧の CSV。基準版と合わせて同じ規則で検査する。渡さない呼び出しの合否・出力は変わらない |
| `--list` | 無効 | 合格のときだけ、対応の一覧を標準出力へ TSV で出す（下記）。ファイルは作らない |
| `--show-keys` | 無効 | 違反の行に鍵の文字列も出す。出力は spec の内容なので、PR・Issue・CI のログ・コミットに貼らない |

終了コード:

| コード | 意味 |
| ------ | ---- |
| 0 | 合格。既定（ファイル単位）では、すべての行の鍵が少なくとも 1 つの項目ファイルに現れた。`--item-rows` のときは、少なくとも 1 つの項目の行に現れた |
| 1 | 違反（fail-closed）。項目ファイルに現れない鍵（`--item-rows` のときは項目の行に現れない鍵）・項目の行が 0（`--item-rows` のとき）・鍵が空・鍵が重複・列の数の不一致・データの行が 0・探索対象が 0・CSV（追随分を含む）や項目ファイルを読めない・追随分の見出しが基準版と合わない・追随分のデータの行が 0 |
| 2 | 誤用・前提の不足（検査を終えていない）。未知の引数・省略形の引数・余分な位置引数・入力が無い・CSV が 0 バイト・`python3` が無い・`--key-column`・`--exclude`・`--item-id-column` が解決できない・`--follow-csv` が無い／0 バイト／2 回以上・予期しない内部エラー |

前提の検査を先に行い、欠けていれば内容を読まずに 2 で終える。入力が無いときに 0 を返さない。違反の診断は件数つきで標準エラーに出し、鍵の文字列は `--show-keys` のときだけ出す。

判定の規則（前向きだけ）:

1. CSV を `csv` モジュールで解析し、鍵の列の値を各行の鍵とする
2. `--item-rows` のときの項目の行は、行頭（空白を除く）が `|` の markdown の表のデータ行 1 行。`\|` は区切りにしない。列の数が見出しと合わない行・ラベルが空の行は項目の行に数えない（違反にはせず、件数だけを標準エラーに出す）。コードフェンス（バッククォート・チルダ）の中の表は数えない。ラベルの列は `--item-id-column` で指し、既定は各表の先頭の列
3. 鍵が「現れる」とは、（`--item-rows` のときは項目の行のラベル以外のセルに、既定では項目ファイルのどこかに）その文字列があり、直前と直後の文字がどちらも「全鍵に現れる文字の集合」に入らないこと（別の鍵の一部は数えない境界。正規表現は使わない）
4. すべての鍵が現れれば合格。表の外の本文にだけ現れる鍵は、`--item-rows` のときだけ違反になる。既定では地の文やラベルのセルだけに現れる鍵でも合格する。1 つの鍵が複数の項目に現れる場合は、一覧にすべて出す

追随分（`--follow-csv`）の規則:

1. 追随分の見出しのレコードは、基準版の見出しと名前・数・順序まで完全に一致すること。一致しなければ違反 1 件で、追随分のデータ行は検査しない（列の位置がずれている恐れがあるため）。`--key-column` は基準版で解決した列の位置を追随分にも使う
2. 鍵の重複は基準版と追随分を通して判定する（両方にある鍵は重複の違反。上書き・マージはしない）
3. 鍵の境界の文字集合は両ファイルの鍵の和集合から作る。そのほかの判定の規則は基準版と同じ
4. 追随分の行の診断は `at follow-up line N` と書き、基準版の `at line N` と区別する。読み取りの順は、前提の検査 → 基準版 → 追随分 → 項目ファイル

帰属の単位は `locate()` と `locate_files()`、現れるかの判定は `occurs()` に閉じ込めてある。

### 対応の一覧

`--list` を付け、かつ合格のときだけ、標準出力へ TSV を出す（診断はすべて標準エラー）。違反・誤用のときは標準出力を空にする。
`--follow-csv` を付けたときだけ、先頭に列 `csv`（`base` / `follow-up`）を足し、基準版の行 → 追随分の行の順に並べる。付けないときの列と行は変わらない。
列は `line`（CSV の行番号。引用符つきの複数行のセルを含む行は、そのレコードの最後の行）・`key`・`file`（項目ファイル名）・`item`（ラベル。`--item-rows` のときだけ値が入り、既定は空）。鍵と項目の組ごとに 1 行で、並びは CSV の行順 → ファイル名順 → ファイル内の行順。
一覧は spec の内容なので、PR・Issue・CI のログ・コミットに貼らない。成果物としての対応の一覧は本リポに置かない。表計算ソフトで開く場合は、セルが数式として解釈されうる点に注意する。

### 既知の限界

- 前向きだけ。逆向きの検査は持たない
- 追随分は 1 ファイルまで。複数を渡すことはできない
- `--item-rows` の項目の単位は行頭が `|` の表のデータ行だけ。列の数が合わない行にしか現れない鍵は帰属できず違反になる
- 表の外の本文（地の文・箇条書き）にだけ現れる鍵は、`--item-rows` のときだけ違反になる。`make inventory-trace-check` は引数を渡さないので既定のファイル単位で、行単位の確認は済まない
- 鍵の直後・直前に、鍵の文字集合に入る文字が句読点として続く書き方は「現れた」に数えない
- CI では走らない。既定の clone では入力が無く、2 で終わる
- 回帰テストは合成 fixture の範囲に限る
- 回帰テストのうち、読めない項目ディレクトリのケースは権限が効かない環境（root 実行など）では 2 で止まり、成功扱いにしない

## 追随分の取り込みの手順（TASK-17）

引数・終了コード・追随分の規則・対応の一覧は上の表を参照する。ここでは追随分を検査へ渡す順序だけを書く。書くのはスクリプトと回帰テストが示す振る舞いに限る。

### 前提

- 実物の入力での実行には `docs/spec` の取得が要る。取得していなければ 2 で終わる（上の終了コードの表）
- 追随分のファイルの置き場、結果を受けた一覧・項目の側の更新、進み具合の記録は private spec 側の記録に従う（ポインタのみ）。本リポには書かない
- 基準版の CSV と項目のディレクトリのパスは、スクリプトの定数のまま使う（省けば既定）

### 渡し方

1. `make inventory-trace-check` は引数を渡さないので、追随分は渡せない。worktree のルートで `scripts/inventory-trace-check.sh --follow-csv <追随分の CSV のパス>` を直接実行する（`<…>` は実際のパスに置き換える。文書やコミットには実在のパスを書かない）
2. 先に `--follow-csv` なしで流し、基準版だけの合否を確かめる。次に `--follow-csv` を付けて流すと、2 回目の違反を追随分の側に絞りやすい。渡さないときの合否・出力が変わらないことは `follow-up: without the option the output is unchanged` が示す
3. 基準版だけで流すときと同じ `--item-rows`・`--key-column` を併用できる（`follow-up: --item-rows key in a table row`・`follow-up: --key-column by name passes with BOM and CRLF`）
4. `--follow-csv` は 1 回だけ。2 回指定は誤用で 2 になる（`follow-up: --follow-csv given twice`）

### 終了コードの読み方

- 2: 検査を終えていない。追随分のパスが無い・0 バイト・2 回指定、submodule が無いなど。直して流し直す。合否は出ていない（`follow-up: missing --follow-csv path`・`follow-up: empty follow-up CSV file`）
- 1: 違反。次の「違反の見方」へ
- 0: 合格。`--list` を付けたときだけ、このときに対応の一覧が出る

### 違反の見方

- 標準エラーの最後の `FAILED: N violation(s)` で件数を見る
- 行の位置は `at follow-up line N` が追随分、`at line N` が基準版（`follow-up: column mismatch and empty key are labelled`）
- 基準版と追随分にまたがる鍵の重複も違反で、上書き・マージはしない（`follow-up: key in both CSVs is a duplicate`）
- `follow-up CSV header differs from the base CSV header` が出たときは、追随分のデータ行は検査されていない。ほかの違反が出ていなくても、追随分は確かめ終わっていない。見出しを直して流し直す（`follow-up: header name differs from the base`・`follow-up: header column count differs from the base`）
- `the follow-up CSV has no data rows`（`follow-up: header-only CSV`）と `cannot parse the follow-up CSV`（`follow-up: invalid UTF-8 in the follow-up CSV`・`follow-up: unterminated quote in the follow-up CSV`）は、追随分そのものを直す
- 鍵の文字列は既定で出ない（`follow-up: keys are hidden by default`）。手元で鍵を特定したいときだけ `--show-keys` を付ける（`follow-up: --show-keys prints the key`）。この出力と `--list` の出力は spec の内容なので、PR・Issue・CI のログ・コミットに貼らず、本リポにファイルとして残さない

## 振る舞いとテストの対応

テストは `make test-inventory-trace-check` のケース名（`scripts/test-inventory-trace-check.sh`）。判定は終了コード + 標準出力 + 標準エラーの部分一致 + 違反の件数。

| 振る舞い | ケース |
| -------- | ------ |
| 鍵がどの項目ファイルにも現れない行があると 1 で終わり、該当の CSV 行番号を標準エラーに出す | `default: absent key fails`・`one row of several is absent` |
| すべての鍵が現れれば 0 で終わる。地の文・ラベルのセルに現れても既定では合格する | `default: key in an item file`・`default: key in prose passes`・`default: key in label cell passes` |
| 入力が無い・読めない前提の不足は 0 を返さず 2 で終わる | `missing --csv path`・`missing --items-dir path`・`empty CSV file`・`python3 missing from PATH` |
| 誤用は 2 で終わる | `--item-id-column without --item-rows is misuse`・`unknown option`・`abbreviated option is rejected`・`extra positional argument`・`--key-column matches no header`・`--key-column matches two headers`・`--exclude names a missing file`・`--exclude with a path separator`・`--item-id-column matches two headers in a table`・`--item-id-column matches no table` |
| 違反は件数つきで数え、鍵の空・重複・列の数の不一致・探索対象 0 も違反になる | `column mismatch, empty key and duplicate key`・`header-only CSV`・`only README.md to search`・`key only in README, non-md and subdirectory files` |
| `--exclude` で外したファイルだけにある鍵は違反。外さなければ合格 | `key only in an excluded file`・`same key passes when the file is not excluded` |
| 鍵の境界（別の鍵の接頭辞・接尾辞は数えない。鍵の文字集合の外の句読点は境界。ファイルの先頭・末尾） | `key only as a prefix of another key`・`key only as a suffix of another key`・`punctuation outside the key alphabet is a boundary`・`key at the very start and end of a file` |
| 読めない入力（不正な UTF-8・引用符の崩れ・権限なし）は 1 で終わる | `invalid UTF-8 in the CSV`・`unterminated quote in the CSV`・`invalid UTF-8 in an item file`・`unreadable items directory exits 1` |
| BOM と CRLF を読める。鍵の列を名前で指せる | `BOM and CRLF with --key-column by name`・`BOM and CRLF with a non-first key column` |
| `--item-rows`: 表の行に現れれば合格。地の文・ラベルのセルだけなら違反。`\|` はセルの中に残る | `item rows: key in a table row`・`item rows: key only in prose`・`item rows: key only in the label cell`・`item rows: escaped pipe stays in the cell` |
| `--item-rows`: 列の数が合わない行・ラベルが空の行は項目に数えず、注記は違反にならない | `item rows: column mismatch row is not an item`・`item rows: skipped-row note is not an error`・`item rows: header and separator widths differ`・`item rows: only empty-label and mismatched rows`・`item rows: aligned separator row` |
| `--item-rows`: コードフェンスの中の表は数えない | `item rows: key only in a backtick fence`・`item rows: key only in a tilde fence`・`item rows: shorter inner fence does not close`・`item rows: table after a closed fence still counts` |
| `--item-rows`: `--item-id-column` で別の列をラベルにできる | `item rows: --item-id-column selects another column` |
| `--list` は合格のときだけ標準出力へ出し、違反・誤用では空にする。並びと引用 | `--list order: CSV row, file name, line in file`・`default: --list has an empty item column`・`quoted multi-line cell: --list line is the record's last line`・`--list quotes a key containing a tab`・`no stdout without --list`・`--list prints nothing on violation`・`--list prints nothing on violation (file unit)`・`--list prints nothing on misuse` |
| 診断に鍵の文字列は既定で出ない（`--show-keys` のときだけ出る） | `keys are hidden by default`・`--show-keys prints the key`・`item rows: keys are hidden by default` |
| 入力を書き換えず、ファイルを作らない | `inputs are not modified and no files are created`・`follow-up: inputs are not modified and no files are created` |
| `--follow-csv`: 追随分だけにある鍵も項目に現れれば合格し、`--item-rows`・`--key-column` と併用できる | `follow-up: key only in the follow-up CSV passes`・`follow-up: --item-rows key in a table row`・`follow-up: --key-column by name passes with BOM and CRLF` |
| `--follow-csv`: 追随分の違反は `follow-up line N` と書き、基準版の表記と区別する。鍵の重複は 2 ファイルを通して数える | `follow-up: absent follow-up key fails`・`follow-up: --key-column by name with BOM and CRLF`・`follow-up: key in both CSVs is a duplicate`・`follow-up: duplicate inside the follow-up CSV`・`follow-up: column mismatch and empty key are labelled` |
| `--follow-csv`: 見出しが基準版と合わない・データ行が 0 は 1 | `follow-up: header name differs from the base`・`follow-up: header column count differs from the base`・`follow-up: header-only CSV` |
| `--follow-csv`: 鍵の境界の文字集合は両ファイルの鍵の和集合 | `follow-up: boundary uses the keys of both CSVs` |
| `--follow-csv`: 読めない追随分は 1、無い・空・2 回指定は 2 | `follow-up: invalid UTF-8 in the follow-up CSV`・`follow-up: unterminated quote in the follow-up CSV`・`follow-up: missing --follow-csv path`・`follow-up: empty follow-up CSV file`・`follow-up: --follow-csv given twice` |
| `--follow-csv`: `--list` は `csv` 列を足す。鍵は既定で出ない | `follow-up: --list adds the csv column`・`follow-up: --list prints nothing on violation`・`follow-up: keys are hidden by default`・`follow-up: --show-keys prints the key` |
| `--follow-csv` を渡さない呼び出しは変わらない | `follow-up: without the option the output is unchanged`（と、既存のすべてのケース） |

回帰テストは、実物の入力での実行結果を持たない。

## 確かめ方

worktree のルートで実行する。

- `make test-inventory-trace-check`: 回帰テスト（合成 fixture。`docs/spec` の取得は要らない）
- `make inventory-trace-check`: 実物の入力での検査（`docs/spec` の取得が要る。出力は spec の内容なので貼らない）
- `make check`: editorconfig-checker と shellcheck
- `make ci-frame-check`: CI の枠が保たれていること

## 関連

[workspace-boundary](workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)・[github-actions](../.claude/rules/github-actions.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
