# 流し直しの枠と前の記録と比べる道具（TASK-29）の所在と本リポの現状

TASK-29（親 Issue #135）の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-29）。本リポには内容・理由・経緯を置かない
- 前提のタスク（ポインタのみ）と、対応する本リポの物

| 前提 | 本リポの物 |
| ---- | ---------- |
| TASK-23 | `crates/core/src/test_frame`（[test-frame](test-frame.md)）、`scripts/record-summary.sh` |

- `docs/spec` は submodule（`update = none`）。指している版は `git ls-tree origin/main docs/spec` で確かめる

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `core` の試験用の部品の feature | `crates/core/src/lib.rs` で `parity`・`test_frame`・`rerun` が `#[cfg(feature = "test-support")]` の下にある。`crates/core/Cargo.toml` の `[features]` は `test-support = []` | `grep -n -e cfg -e 'pub mod' crates/core/src/lib.rs`、`grep -n -A2 '\[features\]' crates/core/Cargo.toml` |
| 試験の枠と一致の道具のファイル | `crates/core/src/test_frame` は 11 ファイル、`crates/core/src/rerun` は 4 ファイル（入口は `rerun.rs`）、`crates/core/src/parity` は 5 ファイル | `ls crates/core/src/test_frame crates/core/src/rerun crates/core/src/rerun.rs crates/core/src/parity` |
| `core` の統合テスト | `crates/core/tests` に `parity_*` 7 本・`test_frame_*` 9 本・`rerun_*` 5 本。どれも `required-features = ["test-support"]`（21 件） | `ls crates/core/tests`、`grep -c required-features crates/core/Cargo.toml` |
| 記録の部品の書き出しと集計 | 書き出し先の環境変数は `RECORD_DIR_ENV`（`FANDHE_SILICON_TEST_RECORD_DIR`）。`scripts/verify.sh` が `cargo test` の 1 回にだけ一時ディレクトリを渡し、`scripts/record-summary.sh` で集計する | `grep -n RECORD_DIR_ENV crates/core/src/test_frame/export.rs`、`grep -n -e FANDHE_SILICON_TEST_RECORD_DIR -e record-summary scripts/verify.sh` |
| 測定の道具 | `measure/crates/perf` に bin `across-runs` と統合テスト 7 本。`tests/repeat.rs` が書き出しの実物を `scripts/perf-record-check.sh` に通す | `ls measure/crates/perf/src/bin measure/crates/perf/tests`、`grep -n perf-record-check measure/crates/perf/tests/repeat.rs` |
| 引数が要る道具の扱い | `scripts/perf-record-check.sh`・`scripts/record-summary.sh`・`scripts/rerun-compare.sh` は make ターゲットを持たず、回帰テストの `make test-perf-record-check`・`make test-record-summary`・`make test-rerun-compare` だけがある | `sed -n 1,15p scripts/perf-record-check.sh scripts/record-summary.sh`、`grep -n -e test-perf-record-check -e test-record-summary Makefile` |
| 製品の workspace | `members` は 4 crate、`exclude` は `baseline-wgpu`・`measure`・`wasm` | `grep -n -e members -e exclude Cargo.toml` |
| deps-approved.toml の件数 | ルートは `crates/contract` の macOS 向けの 4 件（`core` を `used_by` に持つ記録は 0 件）・`measure/` 0 件 | `grep -c '^\[\[approval\]\]' deps-approved.toml measure/deps-approved.toml`、`grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml` |
| CI のジョブ | `check`・`test-hooks`・`verify`・`deny`・`wasm-macos`・`verify-macos`・`pr-title`・`ci-complete` | `grep -n -E '^  [a-z0-9-]+:$' .github/workflows/ci.yml` |

既にある関連の部品（名前とリンクのみ）:

- 結果の一致の道具: [parity-tool](parity-tool.md)・[pixel-parity-tool](pixel-parity-tool.md)・[parity-non-finite](parity-non-finite.md)
- 測定の道具と run の記録: [measurement-tool](measurement-tool.md)・[measurement-record](measurement-record.md)
- wasm32 と Node での照合の土台: [wasm32-node-foundation](wasm32-node-foundation.md)
- 機材の要らない CI の枠: [ci-device-free-jobs](ci-device-free-jobs.md)

## 照合の型の所在（TASK-29 の 2/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/rerun.rs`（入口）と `rerun/check.rs`・`rerun/set.rs`。feature `test-support` の裏だけ。feature なしのビルドには入らない | `grep -n -B1 'pub mod rerun' crates/core/src/lib.rs`、`cargo build -p fandhe-silicon-core` |
| 公開面 | trait `Check`（`subject`・`declaration`・`run`）、`CheckSet`・`CheckSetError`・`MAX_CHECKS` | `grep -n 'pub ' crates/core/src/rerun.rs crates/core/src/rerun/*.rs` |
| 依存の向き | 結果・ラベル・識別子は `test_frame` の型をそのまま使う。`rerun` は `test_frame` を読み、`parity` は直接参照しない。製品の経路から参照しない | `grep -rn rerun crates/contract/src crates/exec/src crates/upper/src`（0 件）、`grep -rn parity crates/core/src/rerun*`（0 件） |
| 副作用 | `rerun` のコードはファイル・環境変数・標準出力に直接触れない（書き出しは `test_frame::export_parts` を通す）。`unsafe` なし・外部依存なし | `grep -rn -e 'std::fs' -e 'println!' -e 'env::var' -e unsafe crates/core/src/rerun*`（0 件） |
| テスト | `crates/core/tests/rerun_check.rs`（値はダミー） | `cargo test -p fandhe-silicon-core --features test-support --test rerun_check` |

## 実行の口の所在（TASK-29 の 3/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/rerun/run.rs`。`rerun.rs` から再公開。feature `test-support` の裏だけ | `ls crates/core/src/rerun`、`grep -n 'pub use' crates/core/src/rerun.rs` |
| 公開面 | `run_checks(&CheckSet, &RunSetting) -> Vec<Outcome>`、`export_outcomes(&Path, &[Outcome]) -> Result<Vec<String>, ExportError>`、`run_and_export(Option<&Path>, &CheckSet, &RunSetting) -> Result<Vec<Outcome>, ExportError>`。新しいエラー型・集計の型は無い | `grep -n 'pub fn' crates/core/src/rerun/run.rs` |
| 振る舞い | 結果は宣言の順。組み合わせが未設定・宣言に無いなどで未実施なら本体を呼ばない（`test_frame::judge` に従う）。書き出しは全部の部品を 1 回の `export_parts` にまとめ、失敗したらその呼び出しで作ったファイルが消える。ディレクトリが `None` なら何も書かない。ラベルは書き出しの成否で変わらない。本体が panic したら 1 件も書かない | `cargo test -p fandhe-silicon-core --features test-support --test rerun_run` |
| 記録の形 | 新しい形は無い。`<試験の ID>.<組み合わせの名前>.json`（1 行の JSON）の部品のファイルを並べたディレクトリ。`scripts/record-summary.sh` がそのまま読める。run の欄は足さない | 同上（ファイル名と中身は `record_file_name`・`to_json` と一致） |
| 環境 | 設定（`RunSetting`）と書き出し先は呼び出し側が渡す。環境から作る経路は公開テストで通していない | `grep -rn 'env::var' crates/core/src/rerun*`（0 件） |
| 依存の向き | 製品の経路から参照しない。`test_frame` の `judge`・`export_parts` を使う。`unsafe` なし・外部依存なし | `grep -rn rerun crates/contract/src crates/exec/src crates/upper/src`（0 件） |
| テスト | `crates/core/tests/rerun_run.rs`（値はダミー。書き出し先は試験ごとの一時ディレクトリ。機材は要らず、`make verify` で流れる） | `cargo test -p fandhe-silicon-core --features test-support --test rerun_run` |

## Node での照合の写し（TASK-29 の 4/8）

PoC-4 の Node での再現（BUILD-41 のポインタ）を流し直しの枠に載せるための部品。照合の中身（どの行を比べるか）は持たない。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/rerun/reference.rs`。`rerun.rs` から再公開。feature `test-support` の裏だけ | `grep -n 'pub use' crates/core/src/rerun.rs` |
| 公開面 | `ReferenceSource`（`Fixed`・`NativeNeon`。`name`・`ALL`・`from_name`）、`reference_gap`、`RowState`（`Match`・`Mismatch`・`NotRun(ReferenceGap)`。`from_tokens`・`run_result`）、`RowCheck`（`Check` を実装）。`test_frame` 側は `ReferenceGap`・`NotRunReason` の `ReferenceMissing`・`ReferenceUnavailable`・`RunResult::NoReference` | `grep -n 'pub ' crates/core/src/rerun/reference.rs` |
| 出どころのトークン | `fixed`・`native-neon`。完全一致だけを受け、空白の除去も大文字小文字の読み替えもしない。`wasm/node/cases.json` の各行の `reference_source` と同じ綴り | `cargo test -p fandhe-silicon-core --features test-support --test rerun_reference`（`source_tokens_round_trip`） |
| 欠けの規則 | 出どころを先に見る。作れない出どころは参照の値があっても `Unavailable`、作れる出どころで参照が無ければ `Missing`。スクリプト側（`scripts/wasm-node-check.sh`）の規則と同じ順 | 同上（`gap_prefers_unavailable_source`）、`bash scripts/test-wasm-node-check.sh`（項目 26） |
| ラベルへの写し | `Match`→OK、`Mismatch`→NG、`NotRun(Missing)`→未実施（`reference-missing`）、`NotRun(Unavailable)`→未実施（`reference-unavailable`）。トークンの組は `from_tokens` が完全一致で受け、理由の無い `not-run`・未知の語・`match` に付いた理由は `None` | 同上（`row_tokens_parse_exactly`・`row_states_map_to_task23_labels`） |
| 走らせた組み合わせと記録 | 参照の欠けは本体が返すので `ran_on` は走らせた組み合わせ。閾値の承認は見ない。記録の部品は 1 件で、理由のトークンが `label_reason` に入る。設定が未設定なら本体を呼ばず `not-configured`（機材の無い環境の経路） | 同上（`device_free_setting_is_not_run_without_calling_rows`・`reference_gap_rows_export_reason`）、`cargo test -p fandhe-silicon-core --features test-support --test test_frame_outcome` |
| 宣言 | 組み合わせは決めない。`RowCheck::new` へ呼び出し側が渡す。同じ ID の行は `CheckSet::new` が拒む | 同上（`row_checks_reject_duplicate_ids`） |
| 副作用 | 文字列のトークンを受ける純粋な変換だけ。ファイル・環境変数・標準出力・子プロセス（Node）に触れない。入力の値は `Display`・エラーに入れない。`unsafe` なし・外部依存なし | `grep -rn -e 'std::fs' -e 'println!' -e 'env::var' -e unsafe -e 'process::Command' crates/core/src/rerun*`（0 件） |
| 依存の向き | 製品の経路から参照しない | `grep -rn rerun crates/contract/src crates/exec/src crates/upper/src`（0 件） |
| 綴りの一致 | スクリプトの出どころ・理由のトークンが core のソースに在ることを回帰テストが確かめる（一方向）。集計の `scripts/record-summary.sh` も新しい理由を受ける | `bash scripts/test-wasm-node-check.sh`（項目 27）、`make test-record-summary` |
| テスト | `crates/core/tests/rerun_reference.rs`（値はダミー。宣言の組み合わせもダミーの扱い。書き出し先は一時ディレクトリ。機材・Node は要らず、`make verify` で流れる） | `cargo test -p fandhe-silicon-core --features test-support --test rerun_reference` |

## Node での照合の結果の書き出し（TASK-29 の追加の葉。#7188）

`scripts/wasm-node-check.sh` が出す行の結果を、前の記録と比べられる部品（`record-summary.sh`・`rerun-compare.sh` が読む形）として書き出す選択式の口。記録そのものは本リポに置かない。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `scripts/wasm-node-check.sh`（書き出しは判定と同じ python3 の中）。Rust 側のコードは変えない | `grep -n -e record-dir -e BEGIN scripts/wasm-node-check.sh` |
| 口 | `--record-dir <dir>` と `--record-combination <name>` を必ず組で渡す。既定は無い。片方だけ・値の欠け・空の `<dir>` は終了コード 2。組み合わせは 7 つの名前の完全一致だけを受け、外れは終了コード 2 | `bash scripts/test-wasm-node-check.sh`（項目 32） |
| 書き出し先 | 絶対パスにして、`..` の要素・存在しない・ディレクトリでない・シンボリックリンク（末尾の区切りと `.` を落としてから見る）を終了コード 2 にする。`.wasm` のビルドと Node の起動より前に検査する。ディレクトリは作らない。エラーに引数のパスを出さない | 同上（項目 33） |
| 部品の形 | 新しい形は無い。ファイル名は `<行の id>.<組み合わせ>.json`、中身は `RecordPart::to_json` と同じ欄の順の 1 行と末尾の改行。`behavior_ids` は固定の 1 件、`body_kind` は `contract-test`、`body` は行の id だけ。`label_reason` は未実施のときだけ付く。`mismatch` の詳細（自由な文）は入れない | 同上（項目 28・36） |
| 写し | `match`→`ok`、`mismatch`→`ng`、`not-run`＋`reference-missing`／`reference-unavailable`→`not-run`＋同じ理由。表はスクリプトの目印（`row-labels`・`combinations`）の間にだけ置く | `grep -n -e 'BEGIN row-labels' -e 'BEGIN combinations' scripts/wasm-node-check.sh` |
| 写しの食い違い | 表を `RowState::from_tokens` → `run_result` → 枠のラベルと突き合わせ、全ての `RowState` が 1 回ずつ在ること、組み合わせの表が `MachineCombination::ALL` と順を含めて一致することを `make verify` の試験が確かめる。表と綴りが食い違うと落ちる | `cargo test -p fandhe-silicon-core --features test-support --test rerun_node_record` |
| 書き方 | `O_CREAT\|O_EXCL\|O_NOFOLLOW` で新しく作り、上書きしない。同じ名前のシンボリックリンクも辿らない。失敗したらこの実行で作ったファイルを消して終了コード 2（`row:`・`summary:` は出さない）。入力や結果が不正・Node の失敗・行が 0 件のときは何も書かない | 同上（項目 34・35） |
| 選ばないとき | 標準出力と終了コードは変わらない（選んだ場合も標準出力は同じで、書き出しの行は足さない） | 同上（項目 30） |
| 前の記録との比較 | 書いた部品は `record-summary.sh` を通り、結果が変わると `rerun-compare.sh` が差を分類して出し、同じなら出さない | 同上（項目 28・29・31） |
| 副作用 | 外部依存なし・`unsafe` なし。製品の経路から参照しない。CI の手順は変えない。実機の run は含めない | `grep -rn rerun crates/contract/src crates/exec/src crates/upper/src`（0 件） |
| テスト | `crates/core/tests/rerun_node_record.rs`（スクリプトを読むだけ。起動も書き出しもしない）と `scripts/test-wasm-node-check.sh` の項目 28〜36 | `make verify`、`make test-wasm-node-check` |

## 前の記録と比べる道具の所在（TASK-29 の 5/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `scripts/rerun-compare.sh`（本体）と `scripts/test-rerun-compare.sh`（回帰テスト）。本体は make ターゲットを持たず、入口は回帰テストの `make test-rerun-compare` だけ | `ls scripts/rerun-compare.sh scripts/test-rerun-compare.sh`、`grep -n rerun-compare Makefile` |
| 入力 | 前の記録と新しい記録の部品のディレクトリを 1 つずつ（引数はちょうど 2 つ。既定の入力なし）。部品は `scripts/record-summary.sh` が読むのと同じ形。`measure-perf` の run ごとの記録は扱わない | `sed -n 1,50p scripts/rerun-compare.sh` |
| 形の検査 | 各ディレクトリをまず `scripts/record-summary.sh` に通す（定数表を本道具に写さない）。0 以外なら本道具は終了コード 2。通ったあと比べるのに要る欄だけを読み直し、想定と違えば 2 | `make test-rerun-compare`（`malformed_in_new_only`・`empty_dir` ほか） |
| 比べ方 | 鍵は (試験の ID, 組み合わせ)、値は (ラベル, 理由)。理由だけの違いも差。分類は `regressed`・`improved`・`changed`・`removed`・`added` で、見出しであって合否の判定ではない | `make test-rerun-compare`（`reason_changed`・`mixed`・`order_deterministic`） |
| 出力と終了コード | 標準出力は分類の順・鍵の昇順の行と `summary:` の行。出すのは試験の ID・組み合わせ・ラベル・理由と件数だけ。終了コード 0 差分なし・1 差分あり・2 誤用・前提の不足・読めない記録 | `make test-rerun-compare`（`identical`・`regressed`・`args_*`） |
| 副作用 | 入力を読むだけ。ファイルを作らず、ネットワークを使わない | `make test-rerun-compare`（`creates_no_files`・`inputs_unchanged`） |
| 依存 | bash + python3（3.8 以上の標準ライブラリだけ）。依存の追加なし | `git diff --stat origin/main -- Cargo.toml Cargo.lock deps-approved.toml` |
| テスト | `scripts/test-rerun-compare.sh`（値はダミーの合成の部品。CI の `deny` ジョブが呼ぶ） | `make test-rerun-compare` |

## 枠と道具の通しの公開テストの所在（TASK-29 の 6/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/tests/rerun_round_trip.rs`。feature `test-support` の裏だけ | `grep -n -A2 rerun_round_trip crates/core/Cargo.toml` |
| 通すもの | `run_and_export` が書いた部品を `scripts/rerun-compare.sh` に通す（中で `scripts/record-summary.sh` を通る）。期待の出力は完全一致の文字列で固定 | `cargo test -p fandhe-silicon-core --features test-support --test rerun_round_trip` |
| 前提 | bash と python3。無ければ skip せず失敗にする | 同上 |
| 書き込み先 | 試験ごとの一時ディレクトリだけ。終わると消す。`FANDHE_SILICON_TEST_RECORD_DIR` には書かない | `ls "${TMPDIR:-/tmp}" \| grep fandhe-rerun-round-trip`（0 件） |
| CI | 既存の `verify` ジョブの `make verify` で流れる。機材・ネットワークは要らず、ジョブも make ターゲットも足していない | `make verify` |

## 振る舞いとテストの対応

テストは `crates/core/tests/` の関数と、`scripts/` の回帰テストのケース名。ファイルは各行の括弧に書く。段の順に並べる。

| 振る舞い | テスト |
| -------- | ------ |
| `Check` は trait object として使え、`subject`・`declaration`・`run` が実装どおりの値を返す（2/8） | `check_is_usable_as_trait_object`（`rerun_check.rs`） |
| `CheckSet` は渡された順を保つ。空・上限超え・ID の重複（ほかの部分が違っても）を拒み、上限ちょうどは受ける。検査の順は空 → 件数 → 重複（2/8） | `set_keeps_declaration_order`・`empty_is_rejected`・`count_limit_is_enforced`・`duplicate_id_is_rejected_even_if_other_parts_differ`・`validation_order_is_empty_then_count_then_duplicate`（`rerun_check.rs`） |
| `CheckSetError` の表示は ASCII で、入力の値を含まない（2/8） | `error_text_is_ascii_and_has_no_input`（`rerun_check.rs`） |
| 組み合わせが未設定なら本体を呼ばず未実施になる。結果は `test_frame::judge` のラベルに写る（2/8） | `unset_combination_does_not_call_run`・`judge_maps_results_to_labels`（`rerun_check.rs`） |
| `run_checks` は、未設定では本体を呼ばず全件を未実施にする。設定済みではラベルが枠に従い、宣言に無い組み合わせでは本体を呼ばない。結果は宣言の順（3/8） | `unset_combination_runs_nothing_and_every_check_is_not_run`・`labels_follow_the_frame_on_a_configured_combination`・`undeclared_combination_does_not_call_the_body`・`outcomes_keep_declaration_order`（`rerun_run.rs`） |
| 書き出し: ディレクトリが `None` なら何も書かない。部品ごとに枠と同じ名前と中身のファイルを書き、返す名前は部品の順。失敗したらこの呼び出しが作ったファイルだけが消える。相対パス・無いディレクトリは誤り（3/8） | `no_directory_writes_nothing`・`export_writes_one_file_per_part_with_the_frame_names_and_content`・`export_outcomes_returns_names_in_part_order`・`a_failure_removes_every_file_this_call_made_and_keeps_others`・`invalid_directories_are_errors`（`rerun_run.rs`） |
| ラベルは書き出しの有無に依らない。結果からの集計は枠の集計と一致する（3/8） | `labels_do_not_depend_on_export`・`summary_from_the_outcomes_matches_the_frame`（`rerun_run.rs`） |
| 出どころのトークンは往復し、完全一致だけを受ける。行のトークンの組も完全一致だけを受ける。欠けの規則は作れない出どころが先（4/8） | `source_tokens_round_trip`・`row_tokens_parse_exactly`・`gap_prefers_unavailable_source`（`rerun_reference.rs`） |
| 行の状態は枠のラベルに写る。機材の無い設定では行を呼ばず未実施。参照の欠けの理由は記録の部品に入る。同じ ID の行は拒む（4/8） | `row_states_map_to_task23_labels`・`device_free_setting_is_not_run_without_calling_rows`・`reference_gap_rows_export_reason`・`row_checks_reject_duplicate_ids`（`rerun_reference.rs`） |
| 参照の欠けは理由つきの未実施になり、閾値の検査を飛ばす。未設定は参照の欠けより先で、本体を呼ばない（4/8） | `missing_reference_gives_not_run_with_a_reason_and_skips_the_threshold_check`・`unset_setting_wins_over_a_reference_gap_and_the_body_is_not_called`（`test_frame_outcome.rs`） |
| スクリプト側の欠けの規則と、トークンの綴りの core との一致（一方向）（4/8） | 項目 26・27（`scripts/test-wasm-node-check.sh`。`make test-wasm-node-check`） |
| 差分なし（同じ内容、同じディレクトリを 2 回）は終了コード 0（5/8） | `identical`・`same_dir_twice`（`test-rerun-compare.sh`） |
| 分類（`regressed`・`improved`・`changed`〔理由だけの違い・OK 以外どうしの違い〕・`removed`・`added`・同じ ID の別の組み合わせ）と件数。混在と出力の順の決定性（5/8） | `regressed`・`improved`・`reason_changed`・`label_changed_non_ok`・`added`・`removed`・`same_id_other_combination_is_added`・`mixed`・`order_deterministic`（`test-rerun-compare.sh`） |
| 読めない記録（空・形の不備・JSON の不備・記録でない名前・片方だけの不備）は終了コード 2（5/8） | `empty_dir`・`empty_dir_previous`・`malformed_part`・`malformed_in_new_only`・`malformed_in_previous_only`・`invalid_json_part`・`non_record_name`（`test-rerun-compare.sh`） |
| ディレクトリでない・無い・シンボリックリンク（末尾の区切りつきを含む）は終了コード 2 の誤り。無いパスの名前を出さない（5/8） | `not_a_directory`・`missing_dir`・`symlink_dir`・`symlink_dir_trailing_separator`（`test-rerun-compare.sh`） |
| 誤用（引数の数・`-` 始まりの引数・未知のオプション）は終了コード 2、`--help` は 0（5/8） | `args_0`・`args_1`・`args_3`・`dash_arg`・`unknown_option`・`help`（`test-rerun-compare.sh`） |
| ファイルを作らず、入力を変えない（5/8） | `creates_no_files`・`inputs_unchanged`（`test-rerun-compare.sh`） |
| 同じ内容を 2 回書いて比べると差分なし（終了コード 0、件数は書いた部品の数）（6/8） | `same_unconfigured_run_twice_has_no_differences`（`rerun_round_trip.rs`） |
| 枠が書くラベルの全種類（ok・ng・判定不能 2 種・判定保留・未実施）の部品が、形の検査と比較を通る（6/8） | `every_label_kind_the_frame_writes_passes_the_tool`（`rerun_round_trip.rs`） |
| 結果の違いは分類の順・鍵の昇順で報告され、終了コード 1（6/8） | `changed_results_are_reported_in_the_fixed_order`（`rerun_round_trip.rs`） |
| 組み合わせの未設定と設定済みは、組み合わせごとに比べる（6/8） | `unconfigured_then_configured_is_compared_per_combination`（`rerun_round_trip.rs`） |
| 空の新しい記録は差分なしではなく誤りとして終了コード 2（6/8） | `an_empty_new_record_is_an_error_not_no_difference`（`rerun_round_trip.rs`） |
| スクリプトの行の状態 → ラベルの表は `RowState` と一致し、全ての状態が 1 回ずつ在る。組み合わせ → 機材の表は `MachineCombination::ALL` と一致する。固定のビヘイビア ID は受け入れられる（追加の葉） | `script_row_labels_match_row_state`・`script_row_labels_cover_every_row_state`・`script_combinations_match_machine_combination`・`script_behavior_ids_are_valid`・`script_source_tokens_are_known`（`rerun_node_record.rs`） |
| 書き出した部品の名前・中身・`record-summary.sh` の通過（7 つの組み合わせ）。選ばないときの出力は不変（追加の葉） | 項目 28〜30（`scripts/test-wasm-node-check.sh`） |
| 書き出した部品を `rerun-compare.sh` にかけると、同じなら差なし、変われば `regressed`・`improved`・`changed` が出る（追加の葉） | 項目 31（同上） |
| 誤用・不正な書き出し先・上書き禁止と後始末・失敗時に書かない・詳細を部品に入れない（追加の葉） | 項目 32〜36（同上） |

公開テストから到達させていない経路:

- `RunSetting::from_env` を通す経路（設定と書き出し先は呼び出し側が渡す形でだけ通している）
- `RowState::run_result` を直接呼ぶ試験（`RowCheck` の `run` 経由でだけ通る）
- `ExportError` の `ParentComponent`・`DirectoryIsSymlink`・`NotADirectory`・`Io` を `rerun` の口から出す試験（`rerun_run.rs` が通すのは `AlreadyExists`・`NotAbsolute`・`DirectoryMissing`）
- Node を起動する経路（`rerun` のコードは子プロセスに触れない。書き出しの端から端までは偽の node を使うスクリプトの回帰テストで通す）

## 入れていないもの

- 新しいエラー型・集計の型・記録の形（`test_frame` のものをそのまま使う）。run の欄
- 照合の中身（どの行を比べるか）と、組み合わせの決め方
- `measure-perf` の run ごとの記録を比べる処理（`rerun-compare` は扱わない）
- `rerun-compare` の make ターゲットと、それを呼ぶ CI のジョブ（入口は回帰テストの `make test-rerun-compare` だけ。既存の `deny` ジョブが呼ぶ）
- 閾値の承認の判定（`RowCheck` は見ない）

## 確かめ方

- `cargo test -p fandhe-silicon-core --features test-support --test rerun_check`（`rerun_run`・`rerun_reference`・`rerun_round_trip`・`test_frame_outcome` も同じ形）
- `make test-rerun-compare`・`make test-wasm-node-check`・`make test-record-summary`
- `make verify`（上の Rust の試験を `--all-features` で含む）
- 製品の経路から参照しないこと: `grep -rn rerun crates/contract/src crates/exec/src crates/upper/src`（0 件）

## 完了の条件の確かめ（8/8）

8/8（#143）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-29）。
結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 照合の型（2/8） | `cargo test -p fandhe-silicon-core --features test-support --test rerun_check` | 終了コード 0・`9 passed; 0 failed` |
| A2 | 実行の口（3/8） | 同 `--test rerun_run` | 終了コード 0・`11 passed; 0 failed` |
| A3 | Node での照合の写し（4/8） | 同 `--test rerun_reference`・`--test test_frame_outcome` | 終了コード 0・`7 passed; 0 failed`・`19 passed; 0 failed` |
| A4 | 枠と道具の通しの公開テスト（6/8） | 同 `--test rerun_round_trip` | 終了コード 0・`5 passed; 0 failed` |
| B1 | 前の記録と比べる道具の回帰テスト（5/8） | `make test-rerun-compare` | 終了コード 0・`passed=30 failed=0` |
| B2 | 集計とスクリプト側の欠けの規則・綴りの一致（4/8） | `make test-record-summary`・`make test-wasm-node-check` | 終了コード 0・`passed: 92, failed: 0`・`passed: 80, failed: 0` |
| C1 | 製品の経路から参照しない・副作用と `unsafe` が無い | `crates/contract/src`・`crates/exec/src`・`crates/upper/src` の `rerun` の grep と、`crates/core/src/rerun*` の `std::fs`・`println!`・`env::var`・`unsafe`・`process::Command` の grep | 0 件・0 件 |
| C2 | 依存と workspace が変わっていない | `deps-approved.toml`・`measure/deps-approved.toml` の `[[approval]]` の件数と、ルート `Cargo.toml` の members / exclude | 0 件・0 件・members は 4 crate・exclude は 3（`baseline-wgpu`・`measure`・`wasm`） |
| E1 | main の CI の `verify` ジョブ | main の CI（`151141f` の push run）の `make verify (fmt + clippy + test)` ジョブの step | `make verify`・`make build-profiles` とも conclusion は success |
| E2 | main の CI の `deny` ジョブ | 同じ run の `make deny (cargo-deny)` ジョブの step | `make deny`・`make deps-check`・`make test-record-summary`・`make test-wasm-node-check`・`make ci-frame-check`・`make test-ci-frame-check`・`make public-api-check`・`make public-name-check`・`make test-rerun-compare` とも conclusion は success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make ci-frame-check`・`make test-ci-frame-check`・`make public-api-check`・`make public-name-check` | すべて終了コード 0 |

実機の run は含めない。`make inventory-trace-check` は `docs/spec` の取得が要るので含めない。
照合の中身は TASK-29 の範囲外で、この表に含めない。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-29）。

### 再実行の契機

- `crates/core/src/rerun*`・`crates/core/src/test_frame` の公開面、`crates/core/tests/rerun_*.rs`（`rerun_node_record.rs` を含む）・`test_frame_outcome.rs` を変えたとき
- `scripts/rerun-compare.sh`・`scripts/test-rerun-compare.sh`・`scripts/record-summary.sh`・`scripts/verify.sh`・`scripts/wasm-node-check.sh`・`wasm/node/` を変えたとき
- CI の `verify`・`deny` ジョブの手順を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `151141f`、`docs/spec` の gitlink は `dc955fd`。

### 追加の葉（#7188）の確かめ

上の表は 8/8 の記録で、書き換えない。Node での照合の結果の書き出しを足した時点の実測は次のとおり（base `05fd33a`。GPU の実機なし）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| F1 | 写しの食い違いを落とす試験 | `cargo test -p fandhe-silicon-core --features test-support --test rerun_node_record` | 終了コード 0・`5 passed; 0 failed` |
| F2 | 書き出しの回帰テスト | `make test-wasm-node-check` | 終了コード 0・`passed: 141, failed: 0` |
| F3 | 変えていない道具 | `make test-rerun-compare`・`make test-record-summary` | 終了コード 0・`passed=30 failed=0`・`passed: 92, failed: 0` |
| F4 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make ci-frame-check` | すべて終了コード 0 |
| F5 | 依存と CI の手順が変わっていない | `Cargo.lock`・`deps-approved.toml`・`.github/workflows/ci.yml` の base との差分 | 差分なし |

## 本リポでの反映の方針

- 本書には本リポのコードと公開テストが示す事実だけを置く。テストの無い約束は書かない
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)（測定 → 製品の一方向・PoC の記録を写さない）・[dependency-policy](../.claude/rules/dependency-policy.md)・[unsafe-policy](unsafe-policy.md)・[ci-device-free-jobs](ci-device-free-jobs.md)

## 関連

[test-frame](test-frame.md)・[measurement-tool](measurement-tool.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 依存の承認の件数の現状

上の確かめの表と本書の記録の、依存の承認の件数（0 件）は記録時点の値で、書き換えない。今の値は次のとおり。

- `core` の外部依存は 0 のまま（`cargo tree -p fandhe-silicon-core -e normal --depth 1` が自身だけを出す）
- `grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml` は 0 が返る（`core` を `used_by` に持つ記録は無い）
- `grep -c '^\[\[approval\]\]' deps-approved.toml` は 4 が返る。4 件は `crates/contract` を `used_by` に持つ macOS 向けの記録（`objc2` 系）
