# 共通の試験の枠（TASK-23）の所在と現状の事実

契約の試験を背後実装・機材の組み合わせごとに宣言し、結果のラベルを決め、記録と集計にする共通の枠について、本リポのコードと公開テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-23。親 Issue #91。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 置き場と有効になる条件

- 実装: `crates/core/src/test_frame.rs` と配下 10 ファイル（`combination`・`declaration`・`setting`・`decision`・`identifier`・`threshold`・`label`・`outcome`・`record`・`summary`）。`crates/core/src/lib.rs` の `#[cfg(feature = "test-support")] pub mod test_frame;` で公開する
- 有効になる条件: `core` の feature `test-support`。`contract` の `test-support` が `core` へ転送する。どの crate の `default` にも入れていない。`test-support` なしのビルドには入らない（`cargo build -p fandhe-silicon-core` で枠はコンパイルされない）
- 背後実装: `core` に背後実装の feature は無い。枠は名前を表すだけで、device には触れない
- 依存の向き: `test_frame` が `parity` の比較型を読む（`From<&比較型> for Verdict`）。`parity` は `test_frame` に依存しない。製品の経路（`contract`・`exec`・`upper` の非テストのコード）は参照しない
- 統合テスト: `crates/core/tests/test_frame_*.rs` の 8 本（`crates/core/Cargo.toml` の `[[test]]` で `required-features = ["test-support"]`）と `crates/contract/tests/test_support_test_frame.rs`。単体テストが `crates/core/src/test_frame/record.rs` と `outcome.rs` にある
- `unsafe`・依存: 使わない（`#![forbid(unsafe_code)]`、外部依存 0）。個数テストの期待値は変えていない（[unsafe-policy](unsafe-policy.md)）
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
| `Outcome::to_record_json` | 結果 1 件の記録（JSON の文字列） |
| `Summary`・`LabelCounts`・`SummaryError` | 組み合わせごとのラベルの件数と、その表 |
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
- 表には ID の綴りだけを置き、閾値の数字は持たない。表の根拠の所在は `docs/spec/05-tasks.md` の TASK-53（ポインタのみ）。表を変える PR は、承認の記録の所在を PR 本文に引く
- 表の内容はコードの定数が正で、本書には再掲しない

### ラベルと理由

- ラベルは OK・NG・判定保留・未実施・判定不能の 5 種。種類のトークンは `ok`・`ng`・`pending`・`not-run`・`indeterminate`（`LabelKind::ALL` の順）
- 理由を持つのは判定保留・未実施・判定不能。`Label::reason_token` は、判定保留では未承認の閾値の ID、未実施・判定不能では理由のトークンを返し、OK・NG では `None`。`Label::reason_name` は判定保留でも `None` を返す
- 未実施の理由のトークン: `not-configured`・`invalid-combination-setting`・`invalid-permission-setting`・`combination-not-declared`・`permission-missing`。判定不能の理由のトークン: `device-mismatch`・`machine-unreachable`
- 表示（`Display`）とトークンは英語の ASCII で、入力の値・自由な文を含まない

### 試験ごとの記録

`Outcome::to_record_json` は結果 1 件を、JSON のオブジェクト 1 つ（改行なし・ASCII のみ）の文字列にする純粋な関数。環境・ファイル・標準出力・標準エラーには触れない。キーの順は下の表のとおりで固定。

| キー | 型 | 出す条件 |
| ---- | -- | -------- |
| `behavior_ids` | 文字列の配列 | 常に（宣言の順・重複なし。無ければ空の配列） |
| `machine_combination` | 文字列 | 走らせた組み合わせがあるとき（組み合わせの名前） |
| `machine` | 文字列 | 走らせた組み合わせがあるとき（機材の呼び名） |
| `label` | 文字列 | 常に（種類のトークン） |
| `label_reason` | 文字列 | ラベルが理由を持つとき（`Label::reason_token` の値） |

- 条件を満たさない欄はキーごと出さない。`null` や空文字では埋めない
- 試験の ID・環境変数の値・許可の名前は記録に出ない
- 文字列の値は 1 つのエスケープの関数だけを通す。引用符・バックスラッシュ・制御文字（U+0000〜U+001F と U+007F）・ASCII でない文字をエスケープし、BMP の外の文字はサロゲート対にする。`\n`・`\r`・`\t`・`\b`・`\f` は短い形、ほかは `\u` に小文字の 16 進 4 桁
- 出力の例（試験の ID `record_case`、ビヘイビア項目の ID `MEM-1`、閾値の ID `t-999` は未承認、判定不能の理由 `device-mismatch`）:

```text
{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"ok"}
{"behavior_ids":["MEM-1"],"machine_combination":"cuda-node-b","machine":"node-b","label":"ng"}
{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"pending","label_reason":"t-999"}
{"behavior_ids":["MEM-1"],"label":"not-run","label_reason":"not-configured"}
{"behavior_ids":["MEM-1"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"indeterminate","label_reason":"device-mismatch"}
```

### 集計

- `Summary` は `Outcome` の getter だけを入力に、`MachineCombination::ALL` の 7 行と全体の件数を数える。純粋な計算で、環境・ファイル・標準出力には触れない
- 数え方（`Outcome` 1 件につき）: 走らせた組み合わせの行にラベルの種類を 1 件、宣言したが走らせていない行に `not-run` を 1 件、宣言していない行は数えない。全体にはラベルの種類を 1 件。5 種の件数を常に持つので、OK 以外が表から消えない
- 結果は `Outcome` の順に依らない。`record` を繰り返した結果は `from_outcomes` と一致する
- `has_ng` は NG が 1 件でもあるときだけ `true`
- 件数のあふれは `SummaryError::CountOverflow`（`checked_add`。失敗した `record` は集計を変えない）
- `Display` は Markdown の表。固定の見出しと区切り、`MachineCombination::ALL` の順の 7 行で、セルは固定のトークンと 10 進の数だけ。試験の ID・ビヘイビア項目の ID・理由・環境の値は出ない

## 振る舞いとテストの対応

テストは `crates/core/tests/` の関数。別 crate からの到達は `crates/contract/tests/test_support_test_frame.rs`。

| 振る舞い | テスト |
| -------- | ------ |
| 組み合わせは 7 値で重複なく、名前の綴り・背後実装・機材の対応が固定。`from_name` は完全一致のみ | `all_combinations_are_distinct`・`name_round_trips_for_every_combination`・`names_use_only_lowercase_digits_and_hyphen`・`backend_and_machine_are_fixed_per_combination`・`from_name_is_exact_match_only`・`combination_and_machine_names_are_fixed_spellings`・`every_combination_has_a_fixed_backend_and_machine`・`combination_name_ends_with_its_machine_name`（`test_frame_types.rs`） |
| 許可の名前の検査、宣言の検査（空・重複・上限ちょうどと 1 つ超え・検査の順）、マクロが関数と同じ宣言を作る | `permission_accepts_valid_names`・`permission_rejects_invalid_names`・`declaration_requires_a_combination`・`declaration_limits_counts`・`declaration_drops_duplicates`・`from_parts_reports_permission_errors`・`declaration_accepts_exact_limits_and_rejects_one_more`・`count_limit_is_checked_before_permission_names`・`macro_matches_function_api`・`macro_accepts_several_items_and_trailing_commas`・`error_messages_are_ascii_and_do_not_echo_input`（`test_frame_types.rs`） |
| 実行の可否の順、設定なしと不正の区別、許可の境界、設定が引く名前は 2 つだけ | `unset_combination_is_not_configured`・`invalid_combination_values_are_distinguished_from_unset`・`invalid_permission_values_are_rejected`・`unset_and_empty_permissions_mean_none_granted`・`undeclared_combination_is_not_run`・`run_when_declared_and_permissions_granted`・`one_missing_permission_among_several_is_not_run`・`earlier_conditions_win_over_later_ones`・`every_combination_runs_only_when_declared`・`duplicate_declaration_decides_like_deduplicated_one`・`max_count_and_max_length_permissions_are_runnable`・`permission_count_and_length_boundaries_in_the_setting`・`lookup_is_asked_only_for_the_two_fixed_names`・`unset_and_empty_permissions_give_equal_settings`・`from_env_does_not_panic`（`test_frame_decide.rs`） |
| 環境変数の名前、理由と可否の表示が英語の ASCII で入力の値を含まない | `environment_names_are_fixed_constants`・`reasons_and_decisions_have_ascii_text_without_input_values`（`test_frame_decide.rs`） |
| 識別子の文字種・長さ・検査の順、エラーが入力の値を含まない | `test_id_accepts_valid_names`・`test_id_rejects_invalid_names`・`behavior_id_accepts_valid_names`・`behavior_id_rejects_invalid_names`・`threshold_id_accepts_only_the_canonical_spelling`・`length_limits_are_fixed`・`length_is_checked_before_characters_and_errors_display_fixed_text`・`identifier_errors_are_ascii_and_do_not_echo_input`（`test_frame_identifiers.rs`） |
| 表の各 ID が正規の綴りで重複が無く承認済みと判定され、表に無い ID は未承認 | `every_id_in_the_table_is_canonical_unique_and_approved`・`ids_outside_the_table_are_not_approved`（`test_frame_identifiers.rs`） |
| 結果を決める順（未実施 → 判定不能 → 判定保留 → OK・NG）、本体を呼ぶ・呼ばない、本体の panic が伝わる | `not_run_reasons_pass_through_and_the_body_is_not_called`・`not_run_wins_over_pending_and_indeterminate`・`indeterminate_wins_over_pending_and_verdicts`・`approved_thresholds_map_verdicts_to_ok_and_ng`・`unapproved_threshold_gives_pending_with_the_first_one_in_declared_order`・`a_declared_threshold_outside_the_table_gives_pending`・`body_panic_propagates`・`only_declared_combinations_run_over_all_seven`（`test_frame_outcome.rs`） |
| 承認の判定を差し替えたときの順序の規則（crate 内の単体テスト） | `injected_approval_picks_the_first_unapproved_threshold_in_declared_order`・`injected_full_approval_maps_verdicts_to_ok_and_ng`（`crates/core/src/test_frame/outcome.rs`） |
| `Subject` の検査・重複の除去・上限、`Outcome` の getter、失敗にするのは NG だけ、`parity` の結果の写像 | `subject_validates_and_deduplicates`・`subject_limits_counts`・`count_limits_are_checked_before_identifier_names`・`subject_errors_are_ascii_and_do_not_echo_input`・`outcome_carries_the_subject_and_declaration_as_given`・`outcome_getters_report_what_was_declared_and_run`・`fails_test_is_true_only_for_ng`・`from_match_maps_booleans`・`parity_comparisons_map_to_verdicts`（`test_frame_outcome.rs`） |
| ラベルの種類 5 つのトークン、理由のトークンの綴りと網羅、表示 | `kind_tokens_are_distinct_and_well_formed`・`label_kinds_are_five_distinct_tokens`・`kind_matches_name_for_every_label`・`reason_tokens_exist_only_for_kinds_with_a_reason`・`not_run_reason_tokens_cover_every_variant_without_duplicates`・`indeterminate_reason_tokens_cover_every_variant_without_duplicates`・`reason_tokens_are_fixed_spellings`・`only_ng_fails_the_test`・`display_is_ascii_english`（`test_frame_label.rs`） |
| 記録の形（5 種のラベルの期待の文字列との完全一致、キーの順、条件を満たさない欄を出さないこと） | `ok_record_has_the_combination_and_no_reason`・`ng_record_has_the_combination_and_no_reason`・`pending_record_carries_the_unapproved_threshold_id_as_reason`・`not_run_record_has_the_reason_and_no_combination`・`indeterminate_record_has_the_combination_and_the_reason`・`behavior_ids_keep_declared_order_without_duplicates_and_may_be_empty`・`every_not_run_reason_appears_as_its_token_in_the_record`（`test_frame_record.rs`） |
| 理由の取り出し（判定保留で閾値の ID を返す）、記録が 1 行の ASCII で決まった形、環境変数の値と許可の名前が漏れない | `reason_token_covers_every_reasoned_label_and_only_those`・`record_is_a_single_line_ascii_object_with_balanced_quoting`・`the_same_outcome_gives_the_same_record`・`record_does_not_leak_environment_values_or_permission_names`（`test_frame_record.rs`） |
| 記録のエスケープ（引用符・バックスラッシュ・制御文字・非 ASCII・サロゲート対）。エスケープの関数は非公開のため単体テスト | `plain_ascii_is_only_quoted`・`quote_and_backslash_are_escaped`・`control_characters_use_short_or_u_escapes`・`non_ascii_is_escaped_and_astral_uses_surrogate_pairs`（`crates/core/src/test_frame/record.rs`） |
| 集計の行・数え方・順序・`has_ng`・表の固定・あふれの表示 | `empty_summary_has_seven_rows_of_zero`・`ran_outcomes_count_under_the_combination_they_ran_on`・`not_run_outcomes_count_under_every_declared_combination`・`declared_but_not_run_combinations_count_as_not_run`・`undeclared_combinations_are_not_counted`・`row_total_equals_outcomes_declaring_the_combination`・`totals_count_each_outcome_once_by_its_label`・`outcomes_with_the_same_test_id_are_each_counted`・`has_ng_is_true_only_when_an_outcome_is_ng`・`result_does_not_depend_on_order_and_record_agrees_with_from_outcomes`・`table_has_fixed_header_and_seven_rows_in_all_order`・`table_shows_counts_and_keeps_non_ok_columns`・`table_is_ascii_and_has_no_identifiers`・`table_with_every_kind_and_a_two_digit_count_is_fixed`・`summary_error_is_ascii`（`test_frame_summary.rs`） |
| 宣言 → 実行の可否 → ラベル → 集計 → 表の通し | `each_backend_declares_its_combinations_and_runs_only_there`・`one_flow_yields_all_five_label_kinds`・`unconfigured_run_counts_every_declared_row_as_not_run`・`run_on_one_combination_gives_the_fixed_table`・`seven_runs_give_one_ok_and_six_not_run_per_row`・`has_ng_agrees_with_fails_test_over_the_outcomes`・`flow_with_the_process_environment_never_yields_ng`（`test_frame_end_to_end.rs`） |
| `contract` の `test-support` だけで届き、背後実装の feature に依らず同じ結果（可否・ラベル・記録・集計・表・マクロ） | `contract_test_support_enables_core_test_frame`・`decisions_are_fixed_under_any_backend_feature_set`・`judge_is_reachable_and_fixed_under_any_backend_feature_set`・`record_json_is_reachable_and_fixed_under_any_backend_feature_set`・`summary_is_reachable_and_fixed_under_any_backend_feature_set`・`declaration_macro_is_reachable_from_another_crate`・`end_to_end_table_is_fixed_under_any_backend_feature_set`（`test_support_test_frame.rs`） |
| 背後実装の feature ごとの試験が、その背後実装の組み合わせだけを宣言して走る（対応する feature があるときだけ） | `metal_test_declares_on_the_frame`・`vulkan_test_declares_on_the_frame`・`cuda_test_declares_on_the_frame`（`test_support_test_frame.rs`） |

テストはプロセスの環境と作業ツリーを書き換えず、設定は `from_lookup` へ渡す。公開テストから到達させていない経路は次のとおり。

- `CountOverflow` に至る経路と `LabelCounts::total` の飽和の経路（`usize::MAX` 件の入力が要る）
- `RunSetting::from_env` が読む実際の環境の値（呼び出しが落ちないことと、可否との整合だけを確かめている）
- feature が無いときに `test_frame` が見えないこと、`Outcome` を外から組み立てられないこと（コンパイル時の性質）

## 入れていないもの

- 記録のファイル・標準出力・標準エラーへの書き出し、出力の先を受ける環境変数・引数
- 記録に出るキーは上の 5 つだけ。`Outcome` が持つ試験の ID は記録に出ない
- 記録（JSON の文字列）から `Summary` を作る読み込み
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
