# 共通の試験の枠（TASK-23）の所在と現状の事実

契約の試験を背後実装・機材の組み合わせごとに宣言し、結果のラベルを決め、集計にする共通の枠について、本リポのコードと公開テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-23。親 Issue #91。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 置き場と有効になる条件

- 実装: `crates/core/src/test_frame.rs` と配下 11 ファイル（`combination`・`declaration`・`setting`・`decision`・`identifier`・`threshold`・`label`・`outcome`・`summary`・`record`・`export`）。`crates/core/src/lib.rs` の `#[cfg(feature = "test-support")] pub mod test_frame;` で公開する
- 有効になる条件: `core` の feature `test-support`。`contract` の `test-support` が `core` へ転送する。どの crate の `default` にも入れていない。`test-support` なしのビルドには入らない（`cargo build -p fandhe-silicon-core` で枠はコンパイルされない）
- 背後実装: `core` に背後実装の feature は無い。枠は名前を表すだけで、device には触れない
- 依存の向き: `test_frame` が `parity` の比較型を読む（`From<&比較型> for Verdict`）。`parity` は `test_frame` に依存しない。製品の経路（`contract`・`exec`・`upper` の非テストのコード）は参照しない
- 統合テスト: `crates/core/tests/test_frame_*.rs` の 9 本（`crates/core/Cargo.toml` の `[[test]]` で `required-features = ["test-support"]`）と `crates/contract/tests/test_support_test_frame.rs`。単体テストが `crates/core/src/test_frame/outcome.rs` と `record.rs`（JSON の文字列のエスケープ）にある
- `unsafe`・依存: 使わない（`#![forbid(unsafe_code)]`、外部依存 0）。個数テストの期待値は変えていない（[unsafe-policy](unsafe-policy.md)）
- ファイルを触る場所: `std::fs` を使うのは `export.rs` だけ。枠は `println!`・`eprintln!` を使わない（標準出力・標準エラーに何も出さない）
- 走る場所: `make verify`（`--all-features`）と、`scripts/build-profiles.sh` の `test-support` を含む組。GPU の実機は要らない

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
    3. 宣言した閾値に未承認がある → 判定保留（宣言の順で最初の未承認の ID）
    4. 一致 → OK、不一致 → NG
- 失敗にするのは `Label::fails_test` が `true` の NG だけ。`Outcome` は `Termination` を実装しないので、試験の関数の戻り値にはならない
- `parity` の 5 つの比較型（`tolerance`・`topk`・`pixel` の 2 つ・`special_values`）から `Verdict` への `From` がある（`is_match()` を読むだけ）

### 承認の表

- 閾値の承認の有無は `APPROVED_THRESHOLDS`（`crates/core/src/test_frame/threshold.rs`）の 1 か所だけが持つ。表に無い ID は未承認で、判定保留に倒れる
- 承認の判定を試験の側から差し込む公開の口は無い（`judge` が表を引く）。差し込みでしか確かめられない順序の規則は単体テストで確かめている
- 表には ID の綴りだけを置き、閾値の数字は持たない。表の根拠の所在は `docs/spec/05-tasks.md` の TASK-53（ポインタのみ）
- 表の内容はコードの定数が正で、本書には再掲しない

### ラベルと理由

- ラベルは OK・NG・判定保留・未実施・判定不能の 5 種。種類のトークンは `ok`・`ng`・`pending`・`not-run`・`indeterminate`（`LabelKind::ALL` の順）
- 理由のトークンを持つのは未実施・判定不能（`Label::reason_name`）。OK・NG・判定保留では `None`
- 未実施の理由のトークン: `not-configured`・`invalid-combination-setting`・`invalid-permission-setting`・`combination-not-declared`・`permission-missing`。判定不能の理由のトークン: `device-mismatch`・`machine-unreachable`
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
    5. `label_reason`: 未実施・判定不能のときだけ、その理由のトークン。OK・NG・判定保留では出さない
    6. `body_kind`: 常に `contract-test`
    7. `body`: `{"test_id": 試験の ID}`
- 部品は run の欄（形の版・run の ID・時刻・環境・経路の証拠・回への参照など）を持たない。受け取る側がそれらを足して 1 件の記録に組む
- `RecordPart::fields` は (欄の名前, 値の JSON の文字列) の順序つきの列。値の文字列はそれ単体で閉じた JSON の値。`RecordPart::to_json` は同じ欄を 1 つの JSON オブジェクトにした 1 行の文字列（改行なし・ASCII のみ）で、二つは一致する
- 文字列の値は 1 か所のエスケープを通る（引用符・バックスラッシュ・制御文字・ASCII でない文字。BMP の外の文字はサロゲート対）
- 環境変数の値・許可の名前・自由な文は部品に入らない

### 記録の部品の書き出し

- 環境変数 `FANDHE_SILICON_TEST_RECORD_DIR`（`RECORD_DIR_ENV`）に絶対パスのディレクトリを渡すと、部品 1 件につき 1 ファイルをその直下に作る。未設定なら何も書かない（既定はファイルを作らない）
- ファイル名は `<試験の ID>.<組み合わせの名前>.json`（`record_file_name`）。試験の ID は `[a-z0-9_-]`、組み合わせの名前は `[a-z0-9-]` だけなので、`.`・パスの区切りを含まず、対が違えば名前も違う。中身は部品の 1 行の JSON と改行 1 つ
- 既存のファイルは上書きしない（同名のシンボリックリンクがあっても辿らず `AlreadyExists`）。ディレクトリは作らない。失敗は `ExportError` で返り、種類は次のとおり（パスは含まない）
    - `NotAbsolute`（空・相対パス）・`DirectoryMissing`・`DirectoryIsSymlink`・`NotADirectory`・`AlreadyExists`・`Io(ErrorKind)`
- 書き込みに失敗したファイルは消す。1 回の呼び出しの途中で失敗したら、その呼び出しで作ったファイルも消す（もとからあったファイルには触れない）
- `export_parts` はディレクトリを引数で受ける。`export_from_lookup` は名前から値を引く関数で `RECORD_DIR_ENV` を引き、`export_from_env` はそれへ `std::env::var_os` を渡すだけ
- `judge_and_export`（環境変数を使う）と `judge_and_export_with`（値を引く関数を受ける）は、`judge` と同じ順で結果を決めたあとに部品を書き出し、`Result<Outcome, ExportError>` を返す。書き出しの失敗は `Err` で返り、試験の側が失敗として扱う。書き出しの成否でラベルは変わらず、走らせない場合は本体を呼ばない

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
| 部品の形（5 種のラベル・走らせた結果と未実施の件数と組み合わせ・欄の順と 1 行の JSON の一致・環境変数の値と許可の名前が出ない）、エスケープ | `ok_part_is_one_with_the_combination_and_no_reason`・`ng_part_is_one_with_the_combination_and_no_reason`・`pending_part_has_no_reason`・`indeterminate_part_has_the_reason`・`not_run_gives_one_part_per_declared_combination_in_declared_order`・`a_run_on_one_combination_gives_exactly_one_part_even_with_several_declared`・`not_run_for_an_undeclared_combination_uses_its_own_reason`・`behavior_ids_keep_declared_order_without_duplicates_and_may_be_empty`・`fields_are_an_ordered_list_that_agrees_with_the_one_line_json`・`every_label_kind_and_combination_gives_a_valid_shape`・`parts_do_not_show_environment_values_or_permission_names`（`test_frame_record.rs`）、`plain_ascii_is_only_quoted`・`quote_and_backslash_are_escaped`・`control_characters_use_short_or_u_escapes`・`non_ascii_is_escaped_and_astral_uses_surrogate_pairs`（`crates/core/src/test_frame/record.rs`） |
| 書き出し（環境変数の名前・ファイル名と中身・未設定で書かない・上書きしない・途中の失敗の後始末・ディレクトリの検査・シンボリックリンク・エラーにパスが出ない・名前の衝突しなさ・並行 100 件・入口の挙動） | `environment_name_is_fixed`・`a_run_writes_one_file_with_the_expected_name_and_content`・`a_not_run_writes_one_file_per_declared_combination`・`an_unset_directory_writes_nothing`・`the_lookup_is_asked_only_for_the_directory_name`・`an_existing_file_is_never_overwritten`・`a_failure_part_way_removes_the_files_this_call_made_and_keeps_others`・`a_missing_directory_is_an_error_and_is_not_created`・`a_file_is_not_accepted_as_the_directory`・`relative_and_empty_paths_are_rejected`・`a_symbolic_link_directory_is_not_followed`・`a_symbolic_link_at_the_file_name_is_not_followed`・`errors_do_not_show_the_path`・`file_names_stay_inside_the_directory_and_do_not_collide`・`judge_and_export_writes_and_returns_the_same_outcome`・`judge_and_export_does_not_call_the_body_when_not_run_and_writes_every_declared_one`・`judge_and_export_writes_nothing_when_unset`・`a_write_failure_is_returned_as_an_error_not_swallowed`・`a_hundred_concurrent_writes_neither_collide_nor_go_missing`（`test_frame_export.rs`。シンボリックリンクの 2 つは unix のみ） |
| 宣言 → 実行の可否 → ラベル → 集計 → 表の通し | `each_backend_declares_its_combinations_and_runs_only_there`・`one_flow_yields_all_five_label_kinds`・`unconfigured_run_counts_every_declared_row_as_not_run`・`run_on_one_combination_gives_the_fixed_table`・`seven_runs_give_one_ok_and_six_not_run_per_row`・`has_ng_agrees_with_fails_test_over_the_outcomes`・`flow_with_the_process_environment_never_yields_ng`（`test_frame_end_to_end.rs`） |
| `contract` の `test-support` だけで届き、背後実装の feature に依らず同じ結果（可否・ラベル・集計・表・マクロ・記録の部品） | `contract_test_support_enables_core_test_frame`・`decisions_are_fixed_under_any_backend_feature_set`・`judge_is_reachable_and_fixed_under_any_backend_feature_set`・`summary_is_reachable_and_fixed_under_any_backend_feature_set`・`declaration_macro_is_reachable_from_another_crate`・`end_to_end_table_is_fixed_under_any_backend_feature_set`・`record_parts_are_reachable_and_fixed_under_any_backend_feature_set`（`test_support_test_frame.rs`） |
| 背後実装の feature ごとの試験が、その背後実装の組み合わせだけを宣言して走る（対応する feature があるときだけ） | `metal_test_declares_on_the_frame`・`vulkan_test_declares_on_the_frame`・`cuda_test_declares_on_the_frame`（`test_support_test_frame.rs`） |

テストはプロセスの環境と作業ツリーを書き換えず、設定は `from_lookup` へ渡す。書き出しのテストは `std::env::temp_dir()` の下に試験ごとの別の名前のディレクトリを作って消し、ディレクトリは引数か値を引く関数で渡す。公開テストから到達させていない経路は次のとおり。

- `CountOverflow` に至る経路と `LabelCounts::total` の飽和の経路（`usize::MAX` 件の入力が要る）
- `RunSetting::from_env`・`export_from_env`・`judge_and_export` が読む実際の環境の値（`RunSetting::from_env` は呼び出しが落ちないことと可否との整合だけを確かめている。残りは値を引く関数を受ける版で確かめている）
- 書き込み途中の入出力の失敗（ディスクの空きが無いなど）の後始末
- feature が無いときに `test_frame` が見えないこと、`Outcome` を外から組み立てられないこと（コンパイル時の性質）

## 入れていないもの

- run の欄（形の版・run の ID・時刻・環境・経路の証拠・回への参照など）と、部品に足して 1 件の記録に組む処理
- 書き出したファイルを集めて集計を標準出力に出す道具
- 複数の試験（別プロセスを含む）の結果を 1 つの `Summary` に集める経路、未実施・判定不能の理由ごとの内訳、同じ試験の ID の重複の検出
- 実機の device を取って宣言と照合する経路（`core` から device は取れない。判定不能の値を作る経路は試験の側にある）
- 承認の判定を試験の側から差し込む公開の口
- 比べの関数が返す `Err`（長さの不一致など）からラベルへの写像
- `Outcome` への `Termination` の実装
- 外部依存、`unsafe`、枠のための feature・make ターゲット・CI ジョブ・`build-profiles` の組

## 確かめ方

worktree のルートで実行する。

- `make verify`: `--all-features` で fmt・clippy・test を流す（枠のテストを含む）
- `cargo test -p fandhe-silicon-core --features test-support`・`cargo test -p fandhe-silicon-contract --features test-support`: 枠のテスト
- `cargo test -p fandhe-silicon-contract --no-default-features --features "backend-<metal / vulkan / cuda>,test-support" --test test_support_test_frame`: 背後実装の feature ごとの宣言のテスト
- `cargo build -p fandhe-silicon-core`: feature なしで枠がコンパイルされないこと
- `make build-profiles`: feature の組ごとのビルド・clippy・test
- `make deny`・`make deps-check`: 依存が増えていないこと
- `make check`: editorconfig-checker と shellcheck

## 関連

[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)（枠から読む比較型）・[ci-job-split](./ci-job-split.md)・[measurement-record](./measurement-record.md)・[unsafe-policy](./unsafe-policy.md)・[workspace-boundary](./workspace-boundary.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[security](../.claude/rules/security.md)
