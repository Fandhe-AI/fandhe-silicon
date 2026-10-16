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
| `core` の試験用の部品の feature | `crates/core/src/lib.rs` で `parity` と `test_frame` が `#[cfg(feature = "test-support")]` の下にある。`crates/core/Cargo.toml` の `[features]` は `test-support = []` | `grep -n -e cfg -e 'pub mod' crates/core/src/lib.rs`、`grep -n -A2 '\[features\]' crates/core/Cargo.toml` |
| 試験の枠と一致の道具のファイル | `crates/core/src/test_frame` は 11 ファイル、`crates/core/src/parity` は 5 ファイル | `ls crates/core/src/test_frame crates/core/src/parity` |
| `core` の統合テスト | `crates/core/tests` に `parity_*` 5 本・`test_frame_*` 9 本。どれも `required-features = ["test-support"]`（14 件） | `ls crates/core/tests`、`grep -c required-features crates/core/Cargo.toml` |
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

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。形の具体は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す（親 #135）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)（測定 → 製品の一方向・PoC の記録を写さない）・[dependency-policy](../.claude/rules/dependency-policy.md)・[unsafe-policy](unsafe-policy.md)・[ci-device-free-jobs](ci-device-free-jobs.md)

## 関連

[test-frame](test-frame.md)・[measurement-tool](measurement-tool.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
