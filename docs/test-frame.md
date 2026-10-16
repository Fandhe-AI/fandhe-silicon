# 共通の試験の枠（TASK-23）の所在と現状の事実

契約の試験を背後実装・機材の組み合わせごとに宣言し、結果のラベルを決め、集計にする共通の枠について、本リポのコードと公開テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-23。親 Issue #91。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 置き場と有効になる条件

- 実装: `crates/core/src/test_frame.rs` と配下 11 ファイル（`combination`・`declaration`・`setting`・`decision`・`identifier`・`threshold`・`label`・`outcome`・`summary`・`record`・`export`）。`crates/core/src/lib.rs` の `#[cfg(feature = "test-support")] pub mod test_frame;` で公開する
- 有効になる条件: `core` の feature `test-support`。`contract` の `test-support` が `core` へ転送する。どの crate の `default` にも入れていない。`test-support` なしのビルドには入らない（`cargo build -p fandhe-silicon-core` で枠はコンパイルされない）
- 背後実装: `core` に背後実装の feature は無い。枠は名前を表すだけで、device には触れない
- 依存の向き: `test_frame` が `parity` の比較型を読む（`From<&比較型> for Verdict`）。`parity` は `test_frame` に依存しない。製品の経路（`contract`・`exec`・`upper` の非テストのコード）は参照しない。`rerun` が `test_frame` を読み、`judge` と `export_parts` を使う（`test_frame` は `rerun` に依存しない）
- 統合テスト: `crates/core/tests/test_frame_*.rs` の 9 本（`crates/core/Cargo.toml` の `[[test]]` で `required-features = ["test-support"]`）と `crates/contract/tests/test_support_test_frame.rs`。単体テストが `crates/core/src/test_frame/outcome.rs`・`record.rs`（JSON の文字列のエスケープ）・`export.rs`（後始末の失敗）にある
- `unsafe`・依存: 使わない（`#![forbid(unsafe_code)]`、外部依存 0）。個数テストの期待値は変えていない（[unsafe-policy](unsafe-policy.md)）
- ファイルを触る場所: `std::fs` を使うのは `export.rs` だけ。枠は `println!`・`eprintln!` を使わない（標準出力・標準エラーに何も出さない）
- 走る場所: `make verify`（`--all-features`）と、`scripts/build-profiles.sh` の `test-support` を含む組。GPU の実機は要らない
- 記録の集計: `scripts/record-summary.sh`（部品のディレクトリから表を出す道具）と、`scripts/verify.sh` の `cargo test` の 1 回の扱い。回帰テストは `scripts/test-record-summary.sh`（`make test-record-summary`）。下の「記録の集計の道具」

## 公開面

`fandhe_silicon_core::test_frame` から公開するもの。

| 名前 | 内容 |
| ---- | ---- |
| `Backend`・`Machine`・`MachineCombination` | 背後実装・機材の呼び名・その組み合わせの固定の列挙。`MachineCombination::ALL` は 7 値 |
| `Permission`・`Declaration`・`declaration!` | 試験が宣言する組み合わせと要る許可 |
| `RunSetting`・`COMBINATION_ENV`・`PERMISSIONS_ENV` | いまの実行の組み合わせと得ている許可（環境変数から作る） |
| `decide`・`Decision`・`NotRunReason` | 宣言と設定から、走らせるかどうかを決める |
| `TestId`・`BehaviorId`・`ThresholdId`・`Subject` | 試験が自分について宣言する識別子の集まり |
| `judge`・`Outcome`・`RunResult`・`Verdict` | 結果を決める関数と、決まった結果 |
| `Label`・`LabelKind`・`IndeterminateReason` | 5 種のラベルと理由 |
| `APPROVED_THRESHOLDS` | 承認済みの閾値の ID の表（唯一の置き場） |
| `Summary`・`LabelCounts`・`SummaryError` | 組み合わせごとのラベルの件数と、その表 |
| `Outcome::record_parts`・`RecordPart`・`BODY_KIND` | 試験ごとの記録の部品（試験の結果の欄だけ） |
| `RECORD_DIR_ENV`・`export_parts`・`export_from_lookup`・`export_from_env`・`record_file_name`・`ExportError` | 部品のファイルへの書き出し |
| `judge_and_export`・`judge_and_export_with` | `judge` の後に部品を書き出す入口 |
| 上限の定数 | `MAX_COMBINATIONS` 64・`MAX_PERMISSIONS` 16・`MAX_PERMISSION_LEN` 64・`MAX_TEST_ID_LEN` 128・`MAX_BEHAVIOR_ID_LEN` 32・`MAX_BEHAVIORS` 16・`MAX_THRESHOLDS` 16 |

## 振る舞い

### 宣言と実行の可否

- 組み合わせの名前は ASCII の小文字・数字・ハイフンだけ（例: `metal-mac-mini`）。`from_name` は完全一致だけを受け、空白の除去も大文字小文字の読み替えもしない
- `Declaration` は組み合わせが空なら `Err`。重複は取り除かれる。組み合わせは 64 件、許可は 16 件、許可の名前は 64 バイト（`[a-z0-9-]`）までで、超えると `Err`。エラーは種類だけで入力の値を含まない
- 環境変数の名前は `FANDHE_SILICON_TEST_MACHINE_COMBINATION`（組み合わせの名前 1 つ）と `FANDHE_SILICON_TEST_PERMISSIONS`（許可の名前のカンマ区切り）。値は完全一致の列挙と文字種の検査だけに使い、エラーや表示には入れない。許可は未設定と空文字を「許可なし」とし、不正な要素・長すぎる要素・件数の超過・UTF-8 でない値は「不正」とする
- `RunSetting::from_lookup` は名前から値を引く関数を受けて設定を作り、生成は失敗しない。`from_env` は `from_lookup` へ `std::env::var_os` を渡すだけ
- `decide` は下の順で先に当たった条件が勝つ。入力以外の状態を読まず、panic しない。走らせない場合は理由を列挙で返し、成功にも失敗にも読み替えない
    1. 組み合わせの設定が無い
    2. 組み合わせの設定が不正
    3. 許可の設定が不正
    4. 設定の組み合わせが宣言に無い
    5. 要る許可が足りない
    6. 走らせる

### 結果の決め方

- `Subject::from_parts(試験の ID, ビヘイビア項目の ID の列, 閾値の ID の列)` は識別子を検査し、重複を取り除く。ID はいずれも信頼しない入力として検査し、外れは種類だけの `Err` を返す。検査の順は、件数の上限、空、長さ、文字の順
- 文字種: 試験の ID は `[a-z0-9_-]`、ビヘイビア項目の ID は `[A-Z0-9-]`、閾値の ID は `t-` に先頭が 0 でない 1〜3 桁の数字を続けた綴りだけ
- `judge(&Subject, &Declaration, &RunSetting, 本体)` は下の順で先に当たったものでラベルを決める。入力以外の状態を読まず、自分では panic しない。`Outcome` は `judge` 以外からは作れない
    1. 実行の可否が走らせない → 未実施（本体は呼ばない）
    2. 本体が判定不能を返した → 判定不能
    3. 本体が参照の欠けを返した（`RunResult::NoReference`）→ 未実施（理由は `reference-missing`・`reference-unavailable`。閾値の承認は見ない）
    4. 宣言した閾値に未承認がある → 判定保留（宣言の順で最初の未承認の ID）
    5. 一致 → OK、不一致 → NG
- 失敗にするのは `Label::fails_test` が `true` の NG だけ。`Outcome` は `Termination` を実装しないので、試験の関数の戻り値にはならない
- `parity` の 5 つの比較型（`tolerance`・`topk`・`pixel` の 2 つ・`special_values`）から `Verdict` への `From` がある（`is_match()` を読むだけ）

### 承認の表

- 閾値の承認の有無は `APPROVED_THRESHOLDS`（`crates/core/src/test_frame/threshold.rs`）の 1 か所だけが持つ。表に無い ID は未承認で、判定保留に倒れる
- 承認の判定を試験の側から差し込む公開の口は無い（`judge` が表を引く）。差し込みでしか確かめられない順序の規則は単体テストで確かめている
- 表には ID の綴りだけを置き、閾値の数字は持たない。表の根拠の所在は `docs/spec/05-tasks.md` の TASK-53（ポインタのみ）
- 表の内容はコードの定数が正で、本書には再掲しない

### ラベルと理由

- ラベルは OK・NG・判定保留・未実施・判定不能の 5 種。種類のトークンは `ok`・`ng`・`pending`・`not-run`・`indeterminate`（`LabelKind::ALL` の順）
- 理由のトークンを持つのは判定保留・未実施・判定不能（`Label::reason_name`）。OK・NG では `None`。判定保留の理由は常に `threshold-unapproved`（`PENDING_REASON`）で、どの閾値かは `Label::Pending` の値が持つ
- 未実施の理由のトークン: `not-configured`・`invalid-combination-setting`・`invalid-permission-setting`・`combination-not-declared`・`permission-missing`・`reference-missing`・`reference-unavailable`（後ろの 2 つは本体が返す）。判定不能の理由のトークン: `device-mismatch`・`machine-unreachable`
- 表示（`Display`）とトークンは英語の ASCII で、入力の値・自由な文を含まない

### 集計

- `Summary` は `Outcome` の getter だけを入力に、`MachineCombination::ALL` の 7 行と全体の件数を数える。純粋な計算で、環境・ファイル・標準出力には触れない
- 数え方（`Outcome` 1 件につき）: 走らせた組み合わせの行にラベルの種類を 1 件、宣言したが走らせていない行に `not-run` を 1 件、宣言していない行は数えない。全体にはラベルの種類を 1 件。5 種の件数を常に持つので、OK 以外が表から消えない
- 結果は `Outcome` の順に依らない。`record` を繰り返した結果は `from_outcomes` と一致する
- `has_ng` は NG が 1 件でもあるときだけ `true`
- 件数のあふれは `SummaryError::CountOverflow`（`checked_add`。失敗した `record` は集計を変えない）
- `Display` は Markdown の表。固定の見出しと区切り、`MachineCombination::ALL` の順の 7 行で、セルは固定のトークンと 10 進の数だけ。試験の ID・ビヘイビア項目の ID・理由・環境の値は出ない

### 試験ごとの記録の部品

- `Outcome::record_parts` は部品の列を返す。走らせた結果は走らせた組み合わせの 1 件、走らせなかった結果は宣言した組み合わせごとに 1 件（宣言の順）。どの部品も同じラベル・理由・ビヘイビア項目の ID・試験の ID を持つ
- 部品 1 件の欄は次の順。条件を満たさない欄はキーごと出さない
    1. `behavior_ids`: ビヘイビア項目の ID の配列（宣言の順・重複なし。無ければ空の配列）
    2. `machine_combination`: 組み合わせの名前（例: `metal-mac-mini`）
    3. `machine`: 機材の呼び名（例: `mac-mini`）
    4. `label`: 5 種のラベルのトークン
    5. `label_reason`: 判定保留・未実施・判定不能のときだけ、その理由のトークン。OK・NG では出さない
    6. `body_kind`: 常に `contract-test`
    7. `body`: `{"test_id": 試験の ID}`。判定保留のときだけ `test_id` の後に `"threshold_id"`（未承認だった閾値の ID）が続く。ほかのラベルではキーごと出さない
- 部品は run の欄（形の版・run の ID・時刻・環境・経路の証拠・回への参照など）を持たない。受け取る側がそれらを足して 1 件の記録に組む
- `RecordPart::fields` は (欄の名前, 値の JSON の文字列) の順序つきの列。値の文字列はそれ単体で閉じた JSON の値。`RecordPart::to_json` は同じ欄を 1 つの JSON オブジェクトにした 1 行の文字列（改行なし・ASCII のみ）で、二つは一致する
- 文字列の値は 1 か所のエスケープを通る（引用符・バックスラッシュ・制御文字・ASCII でない文字。BMP の外の文字はサロゲート対）
- 環境変数の値・許可の名前・自由な文は部品に入らない

### 記録の部品の書き出し

- 環境変数 `FANDHE_SILICON_TEST_RECORD_DIR`（`RECORD_DIR_ENV`）に絶対パスのディレクトリを渡すと、部品 1 件につき 1 ファイルをその直下に作る。未設定なら何も書かない（既定はファイルを作らない）
- ファイル名は `<試験の ID>.<組み合わせの名前>.json`（`record_file_name`）。試験の ID は `[a-z0-9_-]`、組み合わせの名前は `[a-z0-9-]` だけなので、`.`・パスの区切りを含まず、対が違えば名前も違う。中身は部品の 1 行の JSON と改行 1 つ。ファイル名は試験の ID で始まり、Windows の予約のデバイス名に当たる ID のときの振る舞いは確かめていない
- 同じ試験の部品を同じディレクトリへ 2 回書くと、2 回目は `AlreadyExists` で失敗する（`an_existing_file_is_never_overwritten`）
- 既存のファイルは上書きしない（同名のシンボリックリンクがあっても辿らず `AlreadyExists`）。ディレクトリは作らない。`..` の要素を含むパスは拒否し、末尾の区切りと `.` の要素は取り除いてからリンクかどうかを見る（`link/`・`link/.` も拒否）。限界として、途中の要素のシンボリックリンクは辿り、検査と書き込みのあいだの差し替えは防がない。失敗は `ExportError` で返り、種類は次のとおり（パスは含まない）
    - `NotAbsolute`（空・相対パス）・`ParentComponent`・`DirectoryMissing`・`DirectoryIsSymlink`・`NotADirectory`・`AlreadyExists`・`Io(ErrorKind)`・`CleanupFailed { cause, cleanup }`
- 書き込みに失敗したファイルは消す。1 回の呼び出しの途中で失敗したら、その呼び出しで作ったファイルも消す（もとからあったファイルには触れない）。1 つ消せなくても残りは消し、消せないものがあれば `CleanupFailed` を返す（元の失敗の種類 `cause` と削除の失敗の種類 `cleanup` だけを持ち、パス・ファイル名・OS のメッセージは持たない。作ったファイルが残りうる）。すでに無いファイル（`NotFound`）は失敗に数えない
- `export_parts` はディレクトリを引数で受ける。`export_from_lookup` は名前から値を引く関数で `RECORD_DIR_ENV` を引き、`export_from_env` はそれへ `std::env::var_os` を渡すだけ
- `judge_and_export`（環境変数を使う）と `judge_and_export_with`（値を引く関数を受ける）は、`judge` と同じ順で結果を決めたあとに部品を書き出し、`Result<Outcome, ExportError>` を返す。書き出しの失敗は `Err` で返り、試験の側が失敗として扱う。書き出しの成否でラベルは変わらず、走らせない場合は本体を呼ばない

### 記録の集計の道具

`scripts/record-summary.sh <部品のディレクトリ>` は、部品のファイル（1 ファイルに 1 行の JSON オブジェクト）を読み、機材の組み合わせ 7 通り × ラベル 5 種の件数を Markdown の表にして標準出力に出す。`make` ターゲットは持たない（引数が要る）。実装は bash から `python3 -I -`（3.8 以上の標準ライブラリだけ）を呼ぶ形で、入力の値はシェルに触れない。

- 表の形は `Summary` の `Display` と同じ（見出し、区切り、`MachineCombination::ALL` の順の 7 行、列は `LabelKind::ALL` の順）。道具は末尾に改行を足す。セルは 10 進の件数だけで、試験の ID・ファイル名・理由は出さない
- 1 件の部品は、その組み合わせの行とラベルの列に 1 件を数える。`Summary` は 1 つの結果を、走らせた行に 1 件、宣言したが走らせていない行に未実施を 1 件数えるので、同じ結果の部品を数えた表と一致する
- 終了コード: 0 は集計を出した（未実施・判定保留・NG・判定不能が何件あっても 0。NG で試験を失敗にするのは試験の側）、1 は違反または部品が 0 件（表は出さない）、2 は誤用・前提の不足（引数の数が 1 でない・`-` で始まる引数・ディレクトリが無い・ディレクトリでない〔シンボリックリンクを含む〕・python3 が無い）
- 部品が 0 件のディレクトリは違反（`no-records`）にする。試験が何も書かなかったことを、全部 0 の表で成功に見せないため
- 違反にするもの（どれも件数に数えず、黙って飛ばさない。1 つにつき 1 件の違反）: 通常ファイルでない項目（ディレクトリ・シンボリックリンクなど）、`.json` で終わらない名前、64 KiB を超えるファイル、UTF-8 でない内容、複数行、JSON でない内容、同じキーの重複、NaN などの定数、オブジェクトでない値、必須の欄（`behavior_ids`・`machine_combination`・`machine`・`label`・`body_kind`・`body`）の欠け、一覧に無い欄、未知の組み合わせ・ラベル・`body_kind`・理由、`machine` と `machine_combination` の食い違い、理由を持つラベルで `label_reason` が無い・持たないラベルで有る・別のラベルの理由、`body` の欄の不備（`test_id` の綴り、判定保留のときだけ `threshold_id`）、`behavior_ids` の綴り・重複、同じ（試験の ID, 組み合わせ）の重複
- 違反の表示は `violation: <種類>` の行と `FAILED: <件数> violation(s)` で、種類だけを標準エラーに出す。試験の ID・ファイル名・未知の欄の名前は出さない
- ファイルを作らず、入力を変更せず、ネットワークを使わない
- ディレクトリの引数は、末尾の区切りと `.` の要素を取り除いてから検査する。シンボリックリンクのディレクトリは `link/`・`link/.` の綴りでも拒否する（終了コード 2）
- 綴り（組み合わせ・機材・ラベル・理由・`body_kind`）は道具の定数表が持つ。回帰テストが `combination.rs`・`label.rs`・`decision.rs`・`record.rs` の綴りと、表の見出しの行（`test_support_test_frame.rs` の期待の表）と突き合わせる

### `make verify` と集計

- `scripts/verify.sh` の `cargo test --workspace --all-features`（1 回）だけ、`FANDHE_SILICON_TEST_RECORD_DIR` に新しい一時ディレクトリを向ける。試験のあとに `record-summary.sh` を流して表を標準出力に出し、一時ディレクトリを消す（`trap`。試験が失敗しても消す）。`make verify` を続けて流しても、ディレクトリは毎回新しい
- 試験が失敗したときは、その終了コードをそのまま返す（出せる集計は出す）。試験が通ったのに部品が 0 件・形の違反のときは、`make verify` が失敗する
- 呼び出し側の環境に `FANDHE_SILICON_TEST_RECORD_DIR` が既にあるとき（空文字を含む）は、上書きせずそのまま渡す。集計も片づけもしない
- 一時ディレクトリは絶対パス（`TMPDIR` が相対パスなら現在のディレクトリを前に付ける）。HUP・INT・TERM のときは 129・130・143 で終わり、ディレクトリは消す
- `scripts/build-profiles.sh` の `cargo` には渡さない（呼び出し側の環境に在っても外す。同じ試験を組ごとに何度も流すため。同じディレクトリへ同じ部品を 2 回書くと失敗する）
- CI の `verify` ジョブは `make verify` を呼ぶだけなので、表はジョブのログに出る。記録のファイルは残さず、artifact にも PR の Summary にも出さない。機材の無い環境では宣言した組み合わせごとの未実施の部品だけが書かれ、未実施は失敗にならない
- 背後実装ごとの宣言の試験（`crates/contract/tests/test_support_test_frame.rs`。Metal・Vulkan・CUDA の feature がある組でだけ走る）は、その背後実装の組み合わせだけを宣言し、走らせない経路でだけ記録の部品を書き出す。この試験は device に触れないので、枠が走らせると決めた組み合わせ（機材の組み合わせの指定が宣言に当たる）では何も書かない。走らせない経路では、宣言した組み合わせごとの未実施の部品を 1 件ずつ書き出す。書き出しの失敗は試験の失敗にする。試験の ID は背後実装ごとの定数（`contract_metal_declaration`・`contract_vulkan_declaration`・`contract_cuda_declaration`）。書き出す部分は書き出し先と組み合わせの指定を引数で受ける関数に分けてあり、環境を書き換えずに確かめている
- 試験の ID は、記録を書き出す試験の間で一意にする。部品のファイル名は試験の ID と組み合わせの名前で決まり、既存のファイルは上書きされないので、同じ ID を別の試験が使うと、同じディレクトリへの 2 回目の書き出しが `AlreadyExists` で失敗してその試験が落ちる（`make verify` は毎回新しいディレクトリを渡すので、そこで見つかる）。ソースを走査して重複を探す静的な検査は置いていない。ファイルの中の重複は `export_test_ids_are_distinct` が確かめる

## 振る舞いとテストの対応

テストは `crates/core/tests/` の関数。別 crate からの到達は `crates/contract/tests/test_support_test_frame.rs`。

| 振る舞い | テスト |
| -------- | ------ |
| 組み合わせは 7 値で重複なく、名前の綴り・背後実装・機材の対応が固定。`from_name` は完全一致のみ | `all_combinations_are_distinct`・`name_round_trips_for_every_combination`・`names_use_only_lowercase_digits_and_hyphen`・`backend_and_machine_are_fixed_per_combination`・`from_name_is_exact_match_only`・`combination_and_machine_names_are_fixed_spellings`・`every_combination_has_a_fixed_backend_and_machine`・`combination_name_ends_with_its_machine_name`（`test_frame_types.rs`） |
| 許可の名前の検査、宣言の検査（空・重複・上限ちょうどと 1 つ超え・検査の順）、マクロが関数と同じ宣言を作る | `permission_accepts_valid_names`・`permission_rejects_invalid_names`・`declaration_requires_a_combination`・`declaration_limits_counts`・`declaration_drops_duplicates`・`from_parts_reports_permission_errors`・`declaration_accepts_exact_limits_and_rejects_one_more`・`count_limit_is_checked_before_permission_names`・`macro_matches_function_api`・`macro_accepts_several_items_and_trailing_commas`・`error_messages_are_ascii_and_do_not_echo_input`（`test_frame_types.rs`） |
| 実行の可否の順、設定なしと不正の区別、許可の境界、設定が引く名前は 2 つだけ | `unset_combination_is_not_configured`・`invalid_combination_values_are_distinguished_from_unset`・`invalid_permission_values_are_rejected`・`unset_and_empty_permissions_mean_none_granted`・`undeclared_combination_is_not_run`・`run_when_declared_and_permissions_granted`・`one_missing_permission_among_several_is_not_run`・`earlier_conditions_win_over_later_ones`・`every_combination_runs_only_when_declared`・`duplicate_declaration_decides_like_deduplicated_one`・`max_count_and_max_length_permissions_are_runnable`・`permission_count_and_length_boundaries_in_the_setting`・`lookup_is_asked_only_for_the_two_fixed_names`・`unset_and_empty_permissions_give_equal_settings`・`from_env_does_not_panic`（`test_frame_decide.rs`） |
| 環境変数の名前、理由と可否の表示が英語の ASCII で入力の値を含まない | `environment_names_are_fixed_constants`・`reasons_and_decisions_have_ascii_text_without_input_values`（`test_frame_decide.rs`） |
| 識別子の文字種・長さ・検査の順、エラーが入力の値を含まない | `test_id_accepts_valid_names`・`test_id_rejects_invalid_names`・`behavior_id_accepts_valid_names`・`behavior_id_rejects_invalid_names`・`threshold_id_accepts_only_the_canonical_spelling`・`length_limits_are_fixed`・`length_is_checked_before_characters_and_errors_display_fixed_text`・`identifier_errors_are_ascii_and_do_not_echo_input`（`test_frame_identifiers.rs`） |
| 表の各 ID が正規の綴りで重複が無く承認済みと判定され、表に無い ID は未承認。表の中身が期待の集合と完全に一致する | `every_id_in_the_table_is_canonical_unique_and_approved`・`the_table_equals_the_expected_set_exactly`・`ids_outside_the_table_are_not_approved`（`test_frame_identifiers.rs`） |
| 結果を決める順（未実施 → 判定不能 → 判定保留 → OK・NG）、本体を呼ぶ・呼ばない、本体の panic が伝わる | `not_run_reasons_pass_through_and_the_body_is_not_called`・`not_run_wins_over_pending_and_indeterminate`・`indeterminate_wins_over_pending_and_verdicts`・`approved_thresholds_map_verdicts_to_ok_and_ng`・`unapproved_threshold_gives_pending_with_the_first_one_in_declared_order`・`a_declared_threshold_outside_the_table_gives_pending`・`body_panic_propagates`・`only_declared_combinations_run_over_all_seven`（`test_frame_outcome.rs`） |
| 承認の判定を差し替えたときの順序の規則（crate 内の単体テスト） | `injected_approval_picks_the_first_unapproved_threshold_in_declared_order`・`injected_full_approval_maps_verdicts_to_ok_and_ng`（`crates/core/src/test_frame/outcome.rs`） |
| `Subject` の検査・重複の除去・上限、`Outcome` の getter、失敗にするのは NG だけ、`parity` の結果の写像 | `subject_validates_and_deduplicates`・`subject_limits_counts`・`count_limits_are_checked_before_identifier_names`・`subject_errors_are_ascii_and_do_not_echo_input`・`outcome_carries_the_subject_and_declaration_as_given`・`outcome_getters_report_what_was_declared_and_run`・`fails_test_is_true_only_for_ng`・`from_match_maps_booleans`・`parity_comparisons_map_to_verdicts`（`test_frame_outcome.rs`） |
| ラベルの種類 5 つのトークン、理由のトークンの綴りと網羅、表示 | `kind_tokens_are_distinct_and_well_formed`・`label_kinds_are_five_distinct_tokens`・`kind_matches_name_for_every_label`・`reason_tokens_exist_only_for_kinds_with_a_reason`・`not_run_reason_tokens_cover_every_variant_without_duplicates`・`indeterminate_reason_tokens_cover_every_variant_without_duplicates`・`reason_tokens_are_fixed_spellings`・`only_ng_fails_the_test`・`display_is_ascii_english`（`test_frame_label.rs`） |
| 集計の行・数え方・順序・`has_ng`・表の固定・あふれの表示 | `empty_summary_has_seven_rows_of_zero`・`ran_outcomes_count_under_the_combination_they_ran_on`・`not_run_outcomes_count_under_every_declared_combination`・`declared_but_not_run_combinations_count_as_not_run`・`undeclared_combinations_are_not_counted`・`row_total_equals_outcomes_declaring_the_combination`・`totals_count_each_outcome_once_by_its_label`・`outcomes_with_the_same_test_id_are_each_counted`・`has_ng_is_true_only_when_an_outcome_is_ng`・`result_does_not_depend_on_order_and_record_agrees_with_from_outcomes`・`table_has_fixed_header_and_seven_rows_in_all_order`・`table_shows_counts_and_keeps_non_ok_columns`・`table_is_ascii_and_has_no_identifiers`・`table_with_every_kind_and_a_two_digit_count_is_fixed`・`summary_error_is_ascii`（`test_frame_summary.rs`） |
| 部品の形（5 種のラベル・走らせた結果と未実施の件数と組み合わせ・欄の順と 1 行の JSON の一致・環境変数の値と許可の名前が出ない）、エスケープ | `ok_part_is_one_with_the_combination_and_no_reason`・`ng_part_is_one_with_the_combination_and_no_reason`・`pending_part_has_the_fixed_reason_and_the_threshold_id_in_the_body`・`only_pending_parts_carry_a_threshold_id`・`indeterminate_part_has_the_reason`・`not_run_gives_one_part_per_declared_combination_in_declared_order`・`a_run_on_one_combination_gives_exactly_one_part_even_with_several_declared`・`not_run_for_an_undeclared_combination_uses_its_own_reason`・`behavior_ids_keep_declared_order_without_duplicates_and_may_be_empty`・`fields_are_an_ordered_list_that_agrees_with_the_one_line_json`・`every_label_kind_and_combination_gives_a_valid_shape`・`parts_do_not_show_environment_values_or_permission_names`（`test_frame_record.rs`）、`plain_ascii_is_only_quoted`・`quote_and_backslash_are_escaped`・`control_characters_use_short_or_u_escapes`・`non_ascii_is_escaped_and_astral_uses_surrogate_pairs`（`crates/core/src/test_frame/record.rs`） |
| 書き出し（環境変数の名前・ファイル名と中身・未設定で書かない・上書きしない・途中の失敗の後始末・ディレクトリの検査・シンボリックリンク・エラーにパスが出ない・名前の衝突しなさ・別々の試験の ID での並行 100 件・同じ名前への並行・宣言の順が違う 2 つの並行・入口の挙動） | `environment_name_is_fixed`・`a_run_writes_one_file_with_the_expected_name_and_content`・`a_not_run_writes_one_file_per_declared_combination`・`an_unset_directory_writes_nothing`・`the_lookup_is_asked_only_for_the_directory_name`・`an_existing_file_is_never_overwritten`・`a_failure_part_way_removes_the_files_this_call_made_and_keeps_others`・`a_missing_directory_is_an_error_and_is_not_created`・`a_file_is_not_accepted_as_the_directory`・`relative_and_empty_paths_are_rejected`・`a_symbolic_link_directory_is_not_followed`・`a_symbolic_link_at_the_file_name_is_not_followed`・`errors_do_not_show_the_path`・`file_names_stay_inside_the_directory_and_do_not_collide`・`judge_and_export_writes_and_returns_the_same_outcome`・`judge_and_export_does_not_call_the_body_when_not_run_and_writes_every_declared_one`・`judge_and_export_writes_nothing_when_unset`・`a_write_failure_is_returned_as_an_error_not_swallowed`・`a_hundred_concurrent_writes_of_different_tests_neither_collide_nor_go_missing`・`concurrent_writes_of_the_same_outcome_succeed_exactly_once`・`concurrent_writes_in_different_declared_orders_never_remove_the_others_files`・`a_symbolic_link_directory_with_a_trailing_separator_or_dot_is_rejected`・`parent_components_are_rejected`（`test_frame_export.rs`。シンボリックリンクの 2 つは unix のみ） |
| 後始末の削除が失敗したときの専用のエラー・残りの削除を続けること・`NotFound` を数えないこと・元の失敗の種類の保持・エラーにパスが出ないこと | `a_failed_cleanup_gives_the_dedicated_error_and_keeps_deleting_the_rest`・`a_successful_cleanup_returns_the_original_error`・`a_file_that_is_already_gone_is_not_a_cleanup_failure`・`an_already_exists_failure_keeps_its_kind_inside_the_cleanup_error`（`crates/core/src/test_frame/export.rs` の単体テスト。書き込みと削除の関数を差し替えて作る） |
| 宣言 → 実行の可否 → ラベル → 集計 → 表の通し | `each_backend_declares_its_combinations_and_runs_only_there`・`one_flow_yields_all_five_label_kinds`・`unconfigured_run_counts_every_declared_row_as_not_run`・`run_on_one_combination_gives_the_fixed_table`・`seven_runs_give_one_ok_and_six_not_run_per_row`・`has_ng_agrees_with_fails_test_over_the_outcomes`・`flow_with_the_process_environment_never_yields_ng`（`test_frame_end_to_end.rs`） |
| `contract` の `test-support` だけで届き、背後実装の feature に依らず同じ結果（可否・ラベル・集計・表・マクロ・記録の部品） | `contract_test_support_enables_core_test_frame`・`decisions_are_fixed_under_any_backend_feature_set`・`judge_is_reachable_and_fixed_under_any_backend_feature_set`・`summary_is_reachable_and_fixed_under_any_backend_feature_set`・`declaration_macro_is_reachable_from_another_crate`・`end_to_end_table_is_fixed_under_any_backend_feature_set`・`record_parts_are_reachable_and_fixed_under_any_backend_feature_set`（`test_support_test_frame.rs`） |
| 背後実装の feature ごとの試験が、その背後実装の組み合わせだけを宣言して走る（対応する feature があるときだけ）。走らせる経路では記録を書かず、走らせない経路では宣言した組み合わせの数だけ書く。書き出しの失敗が返る。記録を書き出す試験の ID が互いに別 | `metal_test_declares_on_the_frame`・`vulkan_test_declares_on_the_frame`・`cuda_test_declares_on_the_frame`・`a_run_path_writes_no_record_and_a_not_run_path_writes_one_per_declared_combination`・`an_unset_record_directory_is_not_an_error_on_the_not_run_path`・`an_export_failure_on_the_not_run_path_is_returned`・`export_test_ids_are_distinct`（`test_support_test_frame.rs`） |
| 記録の集計の道具（`scripts/test-record-summary.sh`。ケース名の接頭辞）: 混在した部品の表の完全一致・NG だけでも 0・機材なしの形・改行の有無 | `pass:` |
| 部品が 0 件は違反で、全部 0 の表を出さない | `empty:` |
| JSON の不備・必須の欄の欠け・一覧に無い欄・未知の値・機材の食い違い・理由の過不足・本体の欄の不備 | `json:`・`missing:`・`unknown:`・`value:`・`reason:`・`body:` |
| 重複・通常ファイルでない項目・名前・大きさ・UTF-8・複数行・複数の違反の件数 | `duplicate:`・`entry:`・`multiple:` |
| 違反の表示に入力の値が出ない・誤用の終了コード 2・入力を変えない | `secret:`・`misuse:`・`invariant:` |
| Rust 側の綴り・表の行と列の順・表の見出しとの一致 | `sync:` |
| `verify.sh` の記録の扱い（新しい一時ディレクトリ・表の出力と片づけ・試験の失敗の終了コード・部品が無い／壊れているときの失敗・環境変数が既にあるときは上書きしない） | `verify:`（偽の `cargo` を `PATH` に置いて関数を呼ぶ。相対の `TMPDIR`・HUP/INT/TERM の終了コードを含む）。`build-profiles:`（呼び出し側の書き出し先を引き継がない） |

テストはプロセスの環境と作業ツリーを書き換えず、設定は `from_lookup` へ渡す。書き出しのテストは `std::env::temp_dir()` の下に試験ごとの別の名前のディレクトリを作って消し、ディレクトリは引数か値を引く関数で渡す。公開テストから到達させていない経路は次のとおり。

- `CountOverflow` に至る経路と `LabelCounts::total` の飽和の経路（`usize::MAX` 件の入力が要る）
- `RunSetting::from_env`・`export_from_env`・`judge_and_export` が読む実際の環境の値（`RunSetting::from_env` は呼び出しが落ちないことと可否との整合だけを確かめている。残りは値を引く関数を受ける版で確かめている）
- 書き込み途中の入出力の失敗（ディスクの空きが無いなど）の後始末
- feature が無いときに `test_frame` が見えないこと、`Outcome` を外から組み立てられないこと（コンパイル時の性質）

## 入れていないもの

- run の欄（形の版・run の ID・時刻・環境・経路の証拠・回への参照など）と、部品に足して 1 件の記録に組む処理
- 集計の表を CI の artifact や PR の Summary に出す経路（`make verify` は標準出力にだけ出す）
- 集計の表の理由ごとの内訳、試験の ID の重複をソースから探す静的な検査（重複は書き出しの `AlreadyExists` と、集計の道具の（試験の ID, 組み合わせ）の重複の検出で見つかる）
- 実機の device を取って宣言と照合する経路（`core` から device は取れない。判定不能の値を作る経路は試験の側にある）
- 承認の判定を試験の側から差し込む公開の口
- 比べの関数が返す `Err`（長さの不一致など）からラベルへの写像
- `Outcome` への `Termination` の実装
- 外部依存、`unsafe`、枠のための feature・CI ジョブ・`build-profiles` の組。make ターゲットは集計の道具の回帰テスト（`make test-record-summary`）だけ

## 確かめ方

worktree のルートで実行する。

- `make verify`: `--all-features` で fmt・clippy・test を流す（枠のテストを含む）。最後に集計の表が標準出力に出る
- `make test-record-summary`: 集計の道具と `verify.sh` の記録の扱いの回帰テスト（末尾に `passed: <件数>, failed: <件数>`）
- `scripts/record-summary.sh <ディレクトリ>`: 部品のディレクトリから表を出す。`FANDHE_SILICON_TEST_RECORD_DIR=<絶対パスの空のディレクトリ> cargo test -p fandhe-silicon-contract --all-features --test test_support_test_frame` で部品を書いてから流せる
- `cargo test -p fandhe-silicon-core --features test-support`・`cargo test -p fandhe-silicon-contract --features test-support`: 枠のテスト
- `cargo test -p fandhe-silicon-contract --no-default-features --features "backend-<metal / vulkan / cuda>,test-support" --test test_support_test_frame`: 背後実装の feature ごとの宣言のテスト
- `cargo build -p fandhe-silicon-core`: feature なしで枠がコンパイルされないこと
- `make build-profiles`: feature の組ごとのビルド・clippy・test
- `make deny`・`make deps-check`: 依存が増えていないこと
- `make check`: editorconfig-checker と shellcheck

## 関連

[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)（枠から読む比較型）・[ci-job-split](./ci-job-split.md)・[measurement-record](./measurement-record.md)・[unsafe-policy](./unsafe-policy.md)・[workspace-boundary](./workspace-boundary.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[security](../.claude/rules/security.md)
