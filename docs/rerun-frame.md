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
| `core` の統合テスト | `crates/core/tests` に `parity_*` 5 本・`test_frame_*` 9 本・`rerun_*` 3 本。どれも `required-features = ["test-support"]`（17 件） | `ls crates/core/tests`、`grep -c required-features crates/core/Cargo.toml` |
| 記録の部品の書き出しと集計 | 書き出し先の環境変数は `RECORD_DIR_ENV`（`FANDHE_SILICON_TEST_RECORD_DIR`）。`scripts/verify.sh` が `cargo test` の 1 回にだけ一時ディレクトリを渡し、`scripts/record-summary.sh` で集計する | `grep -n RECORD_DIR_ENV crates/core/src/test_frame/export.rs`、`grep -n -e FANDHE_SILICON_TEST_RECORD_DIR -e record-summary scripts/verify.sh` |
| 測定の道具 | `measure/crates/perf` に bin `across-runs` と統合テスト 7 本。`tests/repeat.rs` が書き出しの実物を `scripts/perf-record-check.sh` に通す | `ls measure/crates/perf/src/bin measure/crates/perf/tests`、`grep -n perf-record-check measure/crates/perf/tests/repeat.rs` |
| 引数が要る道具の扱い | `scripts/perf-record-check.sh`・`scripts/record-summary.sh` は make ターゲットを持たず、回帰テストの `make test-perf-record-check`・`make test-record-summary` だけがある | `sed -n 1,15p scripts/perf-record-check.sh scripts/record-summary.sh`、`grep -n -e test-perf-record-check -e test-record-summary Makefile` |
| 製品の workspace | `members` は 4 crate、`exclude` は `baseline-wgpu`・`measure`・`wasm` | `grep -n -e members -e exclude Cargo.toml` |
| deps-approved.toml の件数 | ルート 0 件・`measure/` 0 件 | `grep -c '^\[\[approval\]\]' deps-approved.toml measure/deps-approved.toml` |
| CI のジョブ | `check`・`test-hooks`・`verify`・`deny`・`wasm-macos`・`pr-title`・`ci-complete` | `grep -n -E '^  [a-z0-9-]+:$' .github/workflows/ci.yml` |

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

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。形の具体は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す（親 #135）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)（測定 → 製品の一方向・PoC の記録を写さない）・[dependency-policy](../.claude/rules/dependency-policy.md)・[unsafe-policy](unsafe-policy.md)・[ci-device-free-jobs](ci-device-free-jobs.md)

## 関連

[test-frame](test-frame.md)・[measurement-tool](measurement-tool.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
