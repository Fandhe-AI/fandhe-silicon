# Metal の実機での試験の実行手順（TASK-19）

Metal の実機（Mac mini）の上で試験を流す回の手順と、手順を助けるスクリプトの振る舞いを案内する。
書くのは、本リポのスクリプトと回帰テストが示す振る舞いと、決定の所在のポインタだけ。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-19・TASK-28）。本リポには決定の内容・理由・経緯を置かない |
| 手順を助けるスクリプト | `scripts/metal-session.sh`（`probe` / `run` / `redact` / `scan`）。回帰テストは `scripts/test-metal-session.sh`（`make test-metal-session`） |
| 回ごとの記録の形の検査 | `scripts/session-record-check.sh`。回帰テストは `scripts/test-session-record-check.sh`（`make test-session-record-check`）。記入例は [session-record.example.json](./session-record.example.json) |
| 実機の試験の CI での扱い | [ci-job-split](./ci-job-split.md) |
| 完了の条件との対応づけの記録 | spec 側が正。本リポからは書き換えない |

## 役割

実機に届く端末で開いたセッションが流す。自動実装のホストからは実機へ接続しない。

| 作業 | 担当 |
| ---- | ---- |
| 専有の許可、時間帯の決定、回のあいだ止めるものを止める、低電力モードの切り替え | オーナー |
| 導入・設定の変更・管理者権限の操作 | オーナー（本手順に含まない） |
| 値の取得・試験の実行・記録の作成と検査・置き換えと検索・持ち帰り | セッション |

`metal-session.sh` は設定を変えず、プロセスを止めず、`xcode-select` を呼ばない。読む・起動する・別のファイルへ書く、だけを行う。

## 手順

### 1. 回の前

1. 専有の許可と時間帯を、回のたびにオーナーへ確かめる
2. 流す対象の commit を決め、worktree が clean であることを確かめる
3. オーナーが止めるものを止めたことを確かめる。常駐プロセスと GUI のログインの状態は、スクリプトが取らない人の記入欄（下の `not_probed`）
4. 環境の値を取る

```sh
scripts/metal-session.sh probe > probe.json
```

`probe` は macOS の上でだけ動く（それ以外は終了コード 2。JSON を出さない）。読むだけのコマンドを 4 つ呼び、必要な数値と決まった語だけを正規表現で取り出して JSON にする。
コマンドの出力は全文を流さないので、ホスト名・ユーザー名・プロセス名・識別子は出力に現れない。

| キー | 型 | 取り出し元 |
| ---- | -- | ---------- |
| `low_power_mode` | 真偽 | `pmset -g` の `lowpowermode`（0 / 1 のときだけ） |
| `sleep_minutes` | 整数 | `pmset -g` の `sleep` |
| `display_sleep_minutes` | 整数 | `pmset -g` の `displaysleep` |
| `disk_sleep_minutes` | 整数 | `pmset -g` の `disksleep` |
| `power_source` | `ac` / `battery` / `ups` | `pmset -g batt` の先頭の `Now drawing from` の行 |
| `thermal_warning_recorded` | 偽 | `pmset -g therm` に「警告の記録なし」の注記があるときだけ偽。注記が無ければ言い切れないので null |
| `load_average` | 数 3 つ | `sysctl -n vm.loadavg` |
| `unavailable` | 文字列の配列 | 取れなかった値のキー名 |
| `not_probed` | 文字列の配列 | 取らない欄（`resident_process_stopped`・`gui_login_state`）。人が記入する |

取れなかった値は null にして `unavailable` に名前を並べ、標準エラーに `warning: N value(s) unavailable` を出す（終了コードは 0）。既定値で埋めない。
`low_power_mode` は回ごとの記録の同名の欄と同じ型（真偽）。

### 2. 流す

```sh
scripts/metal-session.sh run --combination metal-mac-mini \
  --developer-dir <Xcode の Developer ディレクトリ> \
  -- cargo test -p fandhe-silicon-contract --features backend-metal
```

| 引数 | 意味 |
| ---- | ---- |
| `--combination <name>` | 必須。環境変数 `FANDHE_SILICON_TEST_MACHINE_COMBINATION` に渡す。既定値を持たず、呼び出し側の環境の同名の変数も読まない。文字種は `[a-z0-9-]`・64 バイト以下 |
| `--permissions <csv>` | 付けたときだけ環境変数 `FANDHE_SILICON_TEST_PERMISSIONS` を設定する。付けないときは、呼び出し側の環境に同名の変数があっても取り除く。名前は `[a-z0-9-]`（64 バイト以下・16 個まで）をカンマで区切る。空は拒否 |
| `--developer-dir <dir>` | `DEVELOPER_DIR` に渡す。省略時は呼び出し側の環境の `DEVELOPER_DIR`。どちらも無い・絶対パスでない・実在するディレクトリでないときは起動しない。既定の開発者ディレクトリは切り替えない |
| `--caffeinate` | 付けたときだけ `caffeinate -i` の下で起動する |
| `--` 以降 | 起動するコマンドと引数。配列のまま `exec` する（連結も `eval` もしない）。終了コードは起動したコマンドのもの |

- 環境変数の名前と受ける値は `crates/core/src/test_frame/setting.rs` が定める。試験の枠は `FANDHE_SILICON_TEST_MACHINE_COMBINATION` が未設定のとき「未実施」を返し、`FANDHE_SILICON_TEST_PERMISSIONS` が未設定のとき許可は 0 件として扱う（公開テスト: `crates/core/tests/test_frame_decide.rs` の `unset_combination_is_not_configured`・`unset_and_empty_permissions_mean_none_granted`）。試験の枠の現状は [test-frame](./test-frame.md)
- 組み合わせの名前が枠の知らない値のときは、`run` は通しても枠が走らせない側に倒す。名前の正は `crates/core/src/test_frame/combination.rs`
- feature の組は `scripts/build-profiles.sh` を指す。コンパイルの成功を、実機での動作の証拠にしない
- 受け取った文字列をコマンドに連結しない。値は引用する

### 3. 回の記録を作って形を確かめる

回ごとの記録は JSON 1 ファイル。欄の一覧と値の形の正は `scripts/session-record-check.sh` の定数表で、本書には複製しない（記入例は値がすべてダミー）。

```sh
scripts/session-record-check.sh <記録のパス>
```

- 終了コード 0 が合格、1 が違反、2 が誤用・前提の不足
- 形（欄の集合・型・値の形）だけを見る。専有が成り立ったか、採るかどうかは見ない
- 入力の値と、表に無い欄の名前は出力に出さない

### 4. 持ち帰る前の置き換えと検索

持ち帰るテキスト（ログなど）は、機材の側で置き換えてから持ち出す。

```sh
scripts/metal-session.sh redact <語の一覧> <入力> <出力>
scripts/metal-session.sh scan <語の一覧> <ファイル>...
```

- 語の一覧は機材の側に置き、git に入れない。1 行に 1 語（UTF-8・BOM なし・1 MiB 以下）。空行は読み飛ばす。リポジトリの中のファイル（リンク先を含む）は拒否する
- 一覧が空・空行だけ、空白だけの行がある、語が置き換えの印に含まれる、のいずれかも拒否する（終了コード 2）
- 語は固定の文字列で、正規表現として解釈しない。大文字小文字を区別するので、表記の揺れは別々の語として並べる。重なる語は長い方を先に置き換える
- `redact` は印 `<redacted>` に置き換えて、入力とは別の新しいファイル（権限 600）へ書く。出力が既に在る（リンクを含む）・出力の置き場が無いときは何も書かずに終了コード 2 で終わる。置き換えの結果に語が残る場合（印との境目で語ができる場合）は、書かずに終了コード 1 で終わる。バイナリ・UTF-8 でない入力・通常ファイルでない入力・64 MiB を超える入力は終了コード 1
- `scan` は 1 つ以上のファイルに語が残っていないことを確かめる。残っていれば終了コード 1。バイナリを含むファイルも対象にする
- 標準出力・標準エラーに語は出ない。出すのは件数と、ファイルの順番と行番号（20 行まで）だけ

### 5. 戻し方

- 値を含む記録は spec リポ側への PR で戻す。本リポの Issue・PR には「流した事実」と「記録の正は spec 側」のポインタだけを書く
- バイナリは git に入れない
- 実ホスト名・アドレス・資格情報などの秘密情報は書かない（[security](../.claude/rules/security.md)）

## 振る舞いと回帰テスト

`make test-metal-session` のケース名の接頭辞と、`make test-session-record-check` のケース名の接頭辞で示す。

| 振る舞い | 確かめるケース |
| -------- | -------------- |
| `probe` が決まった入力から決まった JSON を出す | `probe: values from fixed input` |
| コマンドが混ぜた余計な文字列を出力に出さない | `probe: noise from commands not echoed` |
| 取れない値・範囲外の値を null と一覧で示す | `probe: ... unavailable`・`probe: all values null and listed` |
| 読むだけのコマンドしか呼ばない | `probe: only read-only queries issued`・`probe: never calls xcode-select` |
| macOS 以外では 0 を返さない | `probe: non-macOS fails` |
| `run` が環境変数と `DEVELOPER_DIR` を子に渡す | `run: env reaches child`・`run: env developer dir reaches child` |
| 組み合わせの既定を持たない | `run: no default combination even if the caller's env has one` |
| 許可は明示したときだけ渡す | `run: caller's permissions not passed on`・`run: permissions passed when given` |
| `DEVELOPER_DIR` が実在しないときは起動しない | `run: missing developer dir`・`run: developer dir required`・`run: child not started ...` |
| `xcode-select` を呼ばない | `run: never calls xcode-select` |
| 終了コードの伝播 | `run: child exit code propagates`・`run: exit code propagates through caffeinate` |
| 空白・`;`・`$()` を含む引数が 1 つの引数のまま渡る | `run: odd arguments arrive as single arguments`・`run: nothing was evaluated` |
| 不正な値・引数を拒否する | `run: bad combination ...`・`run: bad permissions ...`・`run: unknown option` |
| `redact` が置き換え、入力を変えない | `redact: replaces words`・`redact: input is not modified` |
| 既存の出力を上書きしない | `redact: existing output is not overwritten`・`redact: output symlink is refused` |
| 語が固定の文字列として扱われる | `redact: words are fixed strings, not regexes`・`scan: regex characters in words are literal` |
| 置き換えの後に語が残るなら書かない | `redact: a word formed at the mark boundary is refused`・`scan: finds remaining words` |
| 出力に語が出ない | `redact: replaces words`・`scan: finds remaining words` |
| 空の一覧・リポジトリの中の一覧の拒否 | `words: empty list refused`・`words: file inside the repository refused`・`words: symlink into the repository refused` |
| 入力の種類と大きさの拒否 | `input: ...` |
| 環境変数の名前と組み合わせの名前が Rust 側と揃う | `sync: ...` |
| 回ごとの記録の形の検査 | `scripts/test-session-record-check.sh` の `set:`・`type:`・`form:`・`word:`・`input:`・`misuse:`・`secret:`・`sync:` |

## 確かめ方

```sh
make test-metal-session
make test-session-record-check
scripts/session-record-check.sh docs/session-record.example.json
```

`scripts/metal-session.sh` を変更したら `make test-metal-session` を通す。CI は `deny` ジョブと `test-hooks` ジョブ（macOS の bash 3.2 を含む）が呼ぶ（check 名は増えない。[ci-job-split](./ci-job-split.md)）。

## 関連

[ci-job-split](./ci-job-split.md)・[measurement-record](./measurement-record.md)・[test-frame](./test-frame.md)・[security](../.claude/rules/security.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
