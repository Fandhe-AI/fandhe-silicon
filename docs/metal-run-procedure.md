# Metal の実機での試験の実行手順（TASK-19）

Metal の実機（Mac mini）の上で試験を流す回の手順を助けるスクリプトの振る舞いを案内する。
書くのは、本リポのスクリプトと回帰テストが示す振る舞いと、決定の所在のポインタだけ。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

回の前後に人が行うこと、記録の作り方と戻し方は、private spec 側の記録に従う（`docs/spec/05-tasks.md` の TASK-19・TASK-28。ポインタのみ）。本書は、本リポのスクリプトと検査の振る舞いだけを述べる。

## 所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-19・TASK-28）。本リポには決定の内容・理由・経緯を置かない |
| 手順を助けるスクリプト | `scripts/metal-session.sh`（`probe` / `run` / `redact` / `scan`）。回帰テストは `scripts/test-metal-session.sh`（`make test-metal-session`） |
| 回ごとの記録の形の検査 | `scripts/session-record-check.sh`。回帰テストは `scripts/test-session-record-check.sh`（`make test-session-record-check`）。記入例は [session-record.example.json](./session-record.example.json) |
| 実機の試験の CI での扱い | [ci-job-split](./ci-job-split.md) |
| 完了の条件との対応づけの記録 | 記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-19。ポインタのみ） |

`metal-session.sh` は設定を変えず、プロセスを止めず、`xcode-select` を呼ばない。読む・起動する・別のファイルへ書く、だけを行う。
値を含む記録（環境の値・ログ）は本リポに置かない。実ホスト名・アドレス・資格情報などの秘密情報は書かない（[security](../.claude/rules/security.md)）。

## コマンドの並び

1 回の回で、スクリプトを使う部分は次の順になる。`probe` は回の前と後の 2 回流す。`<作業ツリーの外のディレクトリ>` は、値を含む記録を置く場所で、本リポの作業ツリーの外を指す。

```sh
scripts/metal-session.sh probe > <作業ツリーの外のディレクトリ>/before.json
scripts/metal-session.sh run --combination metal-mac-mini \
  --developer-dir <Xcode の Developer ディレクトリ> \
  -- cargo test -p fandhe-silicon-contract --features backend-metal
scripts/metal-session.sh probe > <作業ツリーの外のディレクトリ>/after.json
scripts/session-record-check.sh <回の記録のパス>
scripts/metal-session.sh redact <語の一覧> <ログ> <置き換え後のログ>
scripts/metal-session.sh scan <語の一覧> <置き換え後のログ>
```

### 1. `probe`（環境の値）

macOS の上でだけ動く（それ以外は終了コード 2。JSON を出さない）。読むだけのコマンドを 5 つ（`date -u`・`pmset -g`・`pmset -g batt`・`pmset -g therm`・`sysctl -n vm.loadavg`）呼び、必要な数値と決まった語だけを正規表現で取り出して JSON にする。
コマンドの出力は全文を流さないので、ホスト名・ユーザー名・プロセス名・識別子は出力に現れない。

| キー | 型 | 取り出し元 |
| ---- | -- | ---------- |
| `utc` | `YYYY-MM-DDTHH:MM:SSZ` | `date -u`（形と暦を検査する。`session-record-check.sh` の時刻と同じ形） |
| `power_source` | `ac` / `battery` / `ups` | `pmset -g batt` の先頭の `Now drawing from` の行 |
| `low_power_mode` | 真偽 | `pmset -g` の `lowpowermode`（0 / 1 のときだけ） |
| `thermal_warning` | 偽 | `pmset -g therm` に「警告の記録なし」の注記があるときだけ偽。注記が無ければ言い切れないので null |
| `loadavg` | 数 3 つ | `sysctl -n vm.loadavg` |
| `reference` | オブジェクト | 回の記録の欄には入らない参考の値。`sleep_minutes`・`display_sleep_minutes`・`disk_sleep_minutes`（`pmset -g` の同名の行の整数） |
| `unavailable` | 文字列の配列 | 取れなかった値のキー名（`reference` の中は `reference.<名前>`） |
| `not_probed` | 文字列の配列 | 取らない欄（`resident_process_stopped`・`gui_login_state`）。スクリプトは触れない |

- 取れなかった値は null にして `unavailable` に名前を並べる。値が 1 つでも取れなければ、JSON を出したうえで標準エラーに `error: N value(s) unavailable` を出して終了コード 1 で終わる。既定値で埋めない
- `low_power_mode` は回ごとの記録の同名の欄と同じ型（真偽）で、同じ名前の欄に写せる

### 2. `run`（試験を流す）

| 引数 | 意味 |
| ---- | ---- |
| `--combination <name>` | 必須。環境変数 `FANDHE_SILICON_TEST_MACHINE_COMBINATION` に渡す。既定値を持たず、呼び出し側の環境の同名の変数も読まない。文字種は `[a-z0-9-]`・64 バイト以下 |
| `--permissions <csv>` | 付けたときだけ環境変数 `FANDHE_SILICON_TEST_PERMISSIONS` を設定する。付けないときは、呼び出し側の環境に同名の変数があっても取り除く。名前は `[a-z0-9-]`（64 バイト以下・16 個まで）をカンマで区切る。空は拒否 |
| `--developer-dir <dir>` | `DEVELOPER_DIR` に渡す。省略時と空文字を渡したときは、呼び出し側の環境の `DEVELOPER_DIR`。どちらも無い・絶対パスでない・実在するディレクトリでないときは起動しない。既定の開発者ディレクトリは切り替えない |
| `--no-caffeinate` | 付けたときだけ `caffeinate -i` を使わない。既定は `caffeinate -i` の下で起動する。`caffeinate` が見つからず `--no-caffeinate` も無いときは起動しない（終了コード 2）。`--caffeinate` は受け付けない |
| `--` 以降 | 起動するコマンドと引数。配列のまま `exec` する（連結も `eval` もしない）。終了コードは起動したコマンドのもの |

- 環境変数の名前と受ける値は `crates/core/src/test_frame/setting.rs` が定める。試験の枠は `FANDHE_SILICON_TEST_MACHINE_COMBINATION` が未設定のとき「未実施」を返し、`FANDHE_SILICON_TEST_PERMISSIONS` が未設定のとき許可は 0 件として扱う（公開テスト: `crates/core/tests/test_frame_decide.rs` の `unset_combination_is_not_configured`・`unset_and_empty_permissions_mean_none_granted`）。試験の枠の現状は [test-frame](./test-frame.md)
- 組み合わせの名前の正は `crates/core/src/test_frame/combination.rs`
- feature の組は `scripts/build-profiles.sh` を指す

### 3. 回の記録の形の検査

回ごとの記録は JSON 1 ファイル。欄の一覧と値の形の正は `scripts/session-record-check.sh` の定数表で、本書には複製しない（記入例は値がすべてダミー）。

```sh
scripts/session-record-check.sh <記録のパス>
```

- 終了コード 0 が合格、1 が違反、2 が誤用・前提の不足
- 形（欄の集合・型・値の形）だけを見る。専有が成り立ったか、採るかどうかは見ない
- 入力の値と、表に無い欄の名前は出力に出さない

### 4. `redact` / `scan`（置き換えと検索）

```sh
scripts/metal-session.sh redact <語の一覧> <入力> <出力>
scripts/metal-session.sh scan <語の一覧> <ファイル>...
```

語の一覧は、1 行に「語 TAB 印」（UTF-8・BOM なし・1 MiB 以下）。印は `<` + `[A-Za-z0-9-]+` + `>` の形だけを受ける（機材の別名に英大文字を含んでよい。空白・記号・山括弧の入れ子・空は拒否）。例（作り物）:

```text
example-host<TAB><example-machine>
example-dir<TAB><example-place>
```

- 一覧は git の作業ツリーの外に置く（中にあると拒否する。条件は下）。語と印の対応（どの語をどの印にするか）は一覧が持ち、スクリプトは既定の印を持たない
- 拒否する一覧（終了コード 2）: 空・空行だけ、TAB の無い行（印なし）、形の違う印、IP アドレスや UUID の形をした印、空または空白だけの語、同じ語の重複（印が同じでも別でも）、印の中に語が含まれる（置き換えの後の検索が必ず残りを見つけるため）。空行は読み飛ばす
- 一覧の実パスの親ディレクトリが git の作業ツリーの中（`.git` の中、別の worktree、別のリポジトリを含む）にあるとき、または祖先に `.git` があるときは、一覧も `redact` の出力先も拒否する（終了コード 2）。シンボリックリンクは実パスで判定する。ハードリンクは検出しない
- 語は固定の文字列で、正規表現として解釈しない。大文字小文字を区別するので、表記の揺れは別々の語として並べる。重なる語は長い方を先に置き換える
- `redact` は語を一覧の印に置き換えて、入力とは別の新しいファイル（権限 600）へ書く。出力が既に在る（リンクを含む）・出力の置き場が無いときは、何も書かずに終了コード 2 で終わる。置き換えの結果に語が残る場合（印との境目で語ができる場合）は、書かずに終了コード 1 で終わる。バイナリ・UTF-8 でない入力・通常ファイルでない入力・64 MiB を超える入力は終了コード 1
- `scan` は 1 つ以上のファイルに、語の一覧の語と、下の「形で拾う値」が残っていないことを確かめる。残っていれば終了コード 1。バイナリを含むファイルも対象にする
- 形で拾う値（前もって値を知らないもの）は、IPv4 アドレス・IPv6 アドレス・UUID の 3 種。`scan` は語の一覧の有無にかかわらず常にこの 3 種も探し、検出を切るオプションは無い。`redact` はこの 3 種を置き換えない（置き換え後の印を持たない）。残っていれば件数を標準エラーへ注記して終了コード 0 で終わるので、`scan` で必ず確かめる
- 形は安全側に広く拾う。止まった行は人が確かめる（除外の指定は無い）。例: 版の番号のような 4 つ組の数字は IPv4 として止まる。5 つ組、各部が 255 を超えるもの、3 つ組は止まらない。IPv6 は `::` を含む表記（1〜7 組）と 8 組ちょうどの表記を拾い、時刻・6 組の MAC アドレス・語の途中の `::`（`std::string` など）は拾わない。2 桁 8 組の EUI-64 と、16 進数だけの識別子の `a::b` は IPv6 の形に合うので止まる
- 標準出力・標準エラーに語も印も形で拾った値も出ない。出すのは件数と（形は種類ごとの件数）、ファイルの順番と行番号（20 行まで）だけ

## 振る舞いと回帰テスト

`make test-metal-session` のケース名の接頭辞と、`make test-session-record-check` のケース名の接頭辞で示す。

| 振る舞い | 確かめるケース |
| -------- | -------------- |
| `probe` が決まった入力から決まった JSON を出す | `probe: values from fixed input`・`probe: time comes from date` |
| コマンドが混ぜた余計な文字列を出力に出さない | `probe: noise from commands not echoed` |
| 取れない値・範囲外の値・不正な時刻を null と一覧で示し、1 で終わる | `probe: ... unavailable`・`probe: ... exits 1`・`probe: all values null and listed` |
| 読むだけのコマンドしか呼ばない | `probe: only read-only queries issued`・`probe: never calls xcode-select` |
| macOS 以外では 0 を返さない | `probe: non-macOS fails` |
| `run` が環境変数と `DEVELOPER_DIR` を子に渡す | `run: env reaches child`・`run: env developer dir reaches child` |
| 組み合わせの既定を持たない | `run: no default combination even if the caller's env has one` |
| 許可は明示したときだけ渡す | `run: caller's permissions not passed on`・`run: permissions passed when given` |
| `DEVELOPER_DIR` が実在しないときは起動しない | `run: missing developer dir`・`run: developer dir required`・`run: child not started ...` |
| 空の `--developer-dir` は環境の値に回る | `run: empty developer dir option falls through to required` |
| `xcode-select` を呼ばない | `run: never calls xcode-select` |
| 既定で `caffeinate -i`、`--no-caffeinate` で外す、無ければ起動しない | `run: caffeinate -i wraps the command by default`・`run: caffeinate not called with --no-caffeinate`・`run: missing caffeinate refuses to start`・`run: the old --caffeinate option is misuse` |
| 終了コードの伝播 | `run: child exit code propagates`・`run: exit code propagates through caffeinate` |
| 空白・`;`・`$()` を含む引数が 1 つの引数のまま渡る | `run: odd arguments arrive as single arguments`・`run: nothing was evaluated` |
| 不正な値・引数を拒否する | `run: bad combination ...`・`run: bad permissions ...`・`run: unknown option` |
| `redact` が語ごとの印に置き換え、入力を変えない | `redact: replaces words`・`redact: each word gets its own mark`・`redact: input is not modified` |
| 既存の出力を上書きしない | `redact: existing output is not overwritten`・`redact: output symlink is refused` |
| 語が固定の文字列として扱われる | `redact: words are fixed strings, not regexes`・`scan: regex characters in words are literal` |
| 置き換えの後に語が残るなら書かない | `redact: a word formed at the mark boundary is refused`・`scan: finds remaining words` |
| 出力に語も印も出ない | `redact: replaces words`・`scan: finds remaining words` |
| 件数と先頭の行番号が正しい | `scan: many matching lines`・`scan: several matches per line counted once per line` |
| 印が英大文字を受け、形の違う印は拒否のまま | `marks: ...`・`words: malformed mark ...` |
| IPv4・IPv6・UUID の形を種類ごとの件数と行番号で止める（値は出さない）。誤検出の範囲を固定する | `shapes: each kind reports its own lines`・`shapes: look-alikes pass`・`shapes: words and shapes are both reported` |
| `redact` は形で拾う値を置き換えず、注記する | `shapes: redact leaves shapes and notes them without values`・`shapes: redact output keeps the shaped value`・`shapes: scan stops on redact output with a shape` |
| 多数の行・大きな入力でも結果が正しい | `shapes: many shaped lines`・`shapes: 60 MiB input finds the one shaped value on its line` |
| 一覧の形の拒否（印なし・形の違う印・重複・印に語が含まれる・空） | `words: ...` |
| 作業ツリーの中の一覧と出力先の拒否 | `words: ... inside a git work tree refused`・`words: file in another worktree refused`・`words: file inside the .git directory refused`・`words: ancestor with a .git entry refused ...`・`redact: output inside ... refused` |
| 入力の種類と大きさの拒否 | `input: ...` |
| 環境変数の名前と組み合わせの名前が Rust 側と揃う | `sync: ...` |
| 回ごとの記録の形の検査 | `scripts/test-session-record-check.sh` の `set:`・`type:`・`form:`・`word:`・`input:`・`misuse:`・`secret:`・`sync:` |

## 確かめ方

```sh
make test-metal-session
make test-session-record-check
scripts/session-record-check.sh docs/session-record.example.json
```

`scripts/metal-session.sh` を変更したら `make test-metal-session` を通す。回帰テストは、自分を動かしている bash でスクリプト本体を起動する。
CI は `deny` ジョブと `test-hooks` ジョブが呼ぶ。`test-hooks` ジョブの macOS では、`/bin/bash`（標準の 3.2）で `scripts/test-session-record-check.sh` と `scripts/test-metal-session.sh` を直接実行する step もある（check 名は増えない。[ci-job-split](./ci-job-split.md)）。

## 関連

[ci-job-split](./ci-job-split.md)・[measurement-record](./measurement-record.md)・[test-frame](./test-frame.md)・[security](../.claude/rules/security.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
