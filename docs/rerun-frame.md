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
| 試験の枠と一致の道具のファイル | `crates/core/src/test_frame` は 11 ファイル、`crates/core/src/rerun` は 3 ファイル（入口は `rerun.rs`）、`crates/core/src/parity` は 5 ファイル | `ls crates/core/src/test_frame crates/core/src/rerun crates/core/src/rerun.rs crates/core/src/parity` |
| `core` の統合テスト | `crates/core/tests` に `parity_*` 5 本・`test_frame_*` 9 本・`rerun_*` 2 本。どれも `required-features = ["test-support"]`（16 件） | `ls crates/core/tests`、`grep -c required-features crates/core/Cargo.toml` |
| 記録の部品の書き出しと集計 | 書き出し先の環境変数は `RECORD_DIR_ENV`（`FANDHE_SILICON_TEST_RECORD_DIR`）。`scripts/verify.sh` が `cargo test` の 1 回にだけ一時ディレクトリを渡し、`scripts/record-summary.sh` で集計する | `grep -n RECORD_DIR_ENV crates/core/src/test_frame/export.rs`、`grep -n -e FANDHE_SILICON_TEST_RECORD_DIR -e record-summary scripts/verify.sh` |
| 測定の道具 | `measure/crates/perf` に bin `across-runs` と統合テスト 7 本。`tests/repeat.rs` が書き出しの実物を `scripts/perf-record-check.sh` に通す | `ls measure/crates/perf/src/bin measure/crates/perf/tests`、`grep -n perf-record-check measure/crates/perf/tests/repeat.rs` |
| 引数が要る道具の扱い | `scripts/perf-record-check.sh`・`scripts/record-summary.sh`・`scripts/rerun-compare.sh` は make ターゲットを持たず、回帰テストの `make test-perf-record-check`・`make test-record-summary`・`make test-rerun-compare` だけがある | `sed -n 1,15p scripts/perf-record-check.sh scripts/record-summary.sh`、`grep -n -e test-perf-record-check -e test-record-summary Makefile` |
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

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。形の具体は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す（親 #135）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)（測定 → 製品の一方向・PoC の記録を写さない）・[dependency-policy](../.claude/rules/dependency-policy.md)・[unsafe-policy](unsafe-policy.md)・[ci-device-free-jobs](ci-device-free-jobs.md)

## 関連

[test-frame](test-frame.md)・[measurement-tool](measurement-tool.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
