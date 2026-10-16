# 機材の要らないジョブの枠（TASK-112）の所在と現状の事実

CI で動かす、機材（実機・GPU）の要らない検査の枠について、本リポのコードと公開テストから説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-112・TASK-18。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
ジョブ・runner の一覧は [ci-job-split](./ci-job-split.md) を指す。本書は「枠に関わる事実」と「どの検査をどのジョブが呼ぶか」だけを置く。

## 枠の対象

- GitHub ホステッドの runner の上で完結する検査。入力は本リポの追跡ファイルと合成 fixture で、ソースを変更しない
- `ci.yml` の `runs-on` は `ubuntu-latest` と、`test-hooks` の matrix（`ubuntu-latest`・`macos-latest`）、`wasm-macos`（`macos-latest`）だけ。self-hosted の指定は無い

## 分類（どのジョブに足すか）

検査は、要る道具をすでに用意しているジョブへ、ステップとして足す。

| ジョブ | 用意しているもの | 足せるステップ |
| ------ | ---------------- | -------------- |
| `check` | 版固定の editorconfig-checker・shellcheck（Rust の toolchain は入れていない） | 書式・シェルの静的検査 |
| `verify` | `rust-toolchain.toml` の toolchain | workspace のビルド・lint・試験、feature の組・target ごとの確かめ |
| `deny` | toolchain、版固定の cargo-deny、runner 同梱の jq・python3 | 依存グラフ・manifest・`ci.yml` を読む検査、検査スクリプト自体の回帰テスト |
| `test-hooks` | bash + git、runner 同梱の python3（ubuntu・macOS） | `scripts/hooks/` と、macOS の上で直接実行する手順のスクリプトの回帰テスト |
| `wasm-macos` | toolchain、版固定・SHA256 検証済みの Node と wasm-bindgen の CLI（arm64）、runner 同梱の jq・python3 | wasm32 のビルド・glue 込みの量・Node での照合（実物の道具が要るもの） |
| `pr-title` | `pull_request` のときだけ動く | PR の題の検査だけ |

- 当てはまるジョブが無い（別の OS・別の toolchain・別の時間の上限が要る）ときだけ、新しいジョブを検討する
- 新しいジョブ・ラベル・matrix で check 名が変わる場合の扱いは [github-actions](../.claude/rules/github-actions.md)（「必須チェックと集約ジョブ」）に従う

## 実行場所

- `runs-on` は既存のラベルのまま（[github-actions](../.claude/rules/github-actions.md) の「runner 方針」）
- 許可ラベルは `ubuntu-latest`・`macos-latest`。`ci-frame-check` の F3 が確かめる

## 失敗したときの扱い

| 層 | 守ること | 根拠 |
| -- | -------- | ---- |
| スクリプト | 違反で非 0 終了。ツール未導入・入力なし・誤用を成功にしない | `scripts/lib.sh`、[makefile](../.claude/rules/makefile.md) の契約の表 |
| make | 1 行の呼び出しで終了コードをそのまま返す。中身の無いターゲットを置かない | [makefile](../.claude/rules/makefile.md) |
| ステップ | `continue-on-error`・`\|\| true`・`if: always()` を付けない。条件で黙って飛ばさない。ジョブごと飛ばす条件を足すなら、`ci-complete` で明示的に許容する（`pr-title` の先例） | `ci.yml` の現状。F4〜F6 が確かめる |
| ジョブ | `ci-complete` の `needs` にあるジョブへ足す | `ci.yml`。F2 が確かめる |
| 集約 | `ci-complete` は success 以外を失敗にする。skipped を許すのは `pr-title` が `pull_request` 以外のイベントで skip された場合だけ | `ci.yml` の `ci-complete` |
| 失敗することの確かめ | 検査ごとに合成 fixture の回帰テストを置き、違反の入力で非 0 終了と文言を確かめる。実物の違反をコミットしない。期待値を緩めない | `scripts/test-*-check.sh` の先例、[unsafe-policy](./unsafe-policy.md) の同じ考え方 |
| 必須チェック | 登録は本リポの PR では変えない | [github-actions](../.claude/rules/github-actions.md)（「必須チェックと集約ジョブ」） |

## `ci.yml` が呼ぶ検査と make ターゲットの対応

`ci.yml` のジョブが `run` で呼ぶ make ターゲットを全件挙げる（`pr-title`・`ci-complete` は make を呼ばない）。ツールの導入・版の表示のステップは含めない。

| ジョブ ID | 呼ぶ make ターゲット | 実体 | 検査の内容 |
| --------- | -------------------- | ---- | ---------- |
| `check` | `make check` | `scripts/check.sh` | 整形・シェルの静的検査 |
| `test-hooks` | `make test-hooks` | `scripts/hooks/test-hooks.sh` | フックの回帰テスト |
| `test-hooks` | `make test-session-record-check` | `scripts/test-session-record-check.sh` | 回ごとの記録の形の検査の回帰テスト |
| `test-hooks` | `make test-metal-session` | `scripts/test-metal-session.sh` | Metal の実機の回の手順のスクリプトの回帰テスト |
| `verify` | `make verify` | `scripts/verify.sh` | fmt・clippy・workspace の試験。試験の枠の記録から集計の表をジョブのログに出す（[test-frame](./test-frame.md)） |
| `verify` | `make build-profiles` | `scripts/build-profiles.sh` | feature の組ごとの build・clippy・test と wasm32 の check |
| `deny` | `make deny` | `scripts/deny.sh` | 依存（ライセンス・ソース・禁止対象）と workspace の path の検査、製品の依存グラフの wgpu 系 crate の検査（`record:` の件数がログに残る） |
| `deny` | `make measure-verify` | `scripts/measure-verify.sh` | 測定専用の別 workspace（`measure/`）の fmt / clippy / test / 依存の検査（回帰テストは持たない） |
| `deny` | `make wasm-verify` | `scripts/wasm-verify.sh` | wasm32 向けの別 workspace（`wasm/`）の fmt / clippy（ホストと wasm32）/ test / wasm32 の build / 依存の検査（回帰テストは持たない） |
| `wasm-macos` | `make wasm-verify` | `scripts/wasm-verify.sh` | wasm32 向けの別 workspace の fmt / clippy / test / build / 依存の検査（依存の取得もここ。後の 2 つは `--offline`） |
| `wasm-macos` | `make wasm-size` | `scripts/wasm-size.sh` | glue 込みの量（gzip 後）を 8 組で測って閾値と比べる実物の測定 |
| `wasm-macos` | `make wasm-node-check` | `scripts/wasm-node-check.sh` | wasm32 の `.wasm` を glue 越しに Node で呼び参照と照合する実物の実行 |
| `deny` | `make test-wasm-size` | `scripts/test-wasm-size.sh` | glue 込みの量の測定の道具 `scripts/wasm-size.sh` の回帰テスト（合成の入力と偽のコマンド。wasm-bindgen の CLI は要らない） |
| `deny` | `make test-wasm-node-check` | `scripts/test-wasm-node-check.sh` | Node での照合の土台（`scripts/wasm-node-check.sh`）の回帰テスト（偽の node・wasm-bindgen・cargo と合成の結果） |
| `deny` | `make deps-check` | `scripts/deps-check.sh` | 依存の `=x.y.z` 固定と承認の記録の一致 |
| `deny` | `make test-deps-check` | `scripts/test-deps-check.sh` | 上の回帰テスト |
| `deny` | `make test-inventory-trace-check` | `scripts/test-inventory-trace-check.sh` | 一覧と項目の対応の検査の回帰テスト（合成 fixture） |
| `deny` | `make test-session-record-check` | `scripts/test-session-record-check.sh` | 上記 `test-hooks` と同じ回帰テスト |
| `deny` | `make test-perf-record-check` | `scripts/test-perf-record-check.sh` | run ごとの記録の形の検査の回帰テスト（`test-hooks` では流さない） |
| `deny` | `make ci-frame-check` | `scripts/ci-frame-check.sh` | `ci.yml` が枠の事実を保っているかの検査 |
| `deny` | `make test-ci-frame-check` | `scripts/test-ci-frame-check.sh` | 上の回帰テスト（合成 fixture） |
| `deny` | `make test-metal-session` | `scripts/test-metal-session.sh` | 上記 `test-hooks` と同じ回帰テスト |
| `deny` | `make public-api-check` | `scripts/public-api-check.sh` | 4 crate の公開 API に wgpu 系の crate の型・経路が出ていないことの検査（走査した公開アイテムの数が `scanned:` / `record:` の行としてログに残る） |
| `deny` | `make test-public-api-check` | `scripts/test-public-api-check.sh` | 上の回帰テスト（合成 JSON と一時 workspace の実物の rustdoc の JSON） |
| `deny` | `make public-name-check` | `scripts/public-name-check.sh` | 4 crate の公開アイテムの名前に語の一覧の語が出ていないことの検査（走査の件数が `scanned:` / `record:` の行としてログに残る） |
| `deny` | `make test-public-name-check` | `scripts/test-public-name-check.sh` | 上の回帰テスト（ダミーの語の合成 JSON・合成ソースと、一時 workspace の実物の rustdoc の JSON） |
| `deny` | `make test-dep-graph-wgpu-check` | `scripts/test-dep-graph-wgpu-check.sh` | 依存グラフの wgpu 系 crate の検査（`make deny` の中で流れる）の回帰テスト（合成 JSON・合成 tree と一時 workspace の実物の cargo metadata / cargo tree） |
| `deny` | `make test-public-api-diff` | `scripts/test-public-api-diff.sh` | 公開 API の差分の道具 `scripts/public-api-diff.sh` の回帰テスト（合成 JSON だけ） |
| `deny` | `make test-rerun-compare` | `scripts/test-rerun-compare.sh` | 前の記録と比べる道具 `scripts/rerun-compare.sh` の回帰テスト（合成の記録の部品だけ） |
| `deny` | `make test-record-summary` | `scripts/test-record-summary.sh` | 試験の枠の記録から集計を出す道具と、`make verify` の記録の扱いの回帰テスト（合成の部品と偽の cargo） |

- `test-hooks` の macOS だけで、`/bin/bash` による直接実行のステップ（`scripts/hooks/test-hooks.sh` と、`scripts/test-session-record-check.sh`・`scripts/test-metal-session.sh`・`scripts/test-record-summary.sh`）がある
- `wasm-macos` ジョブには、make ターゲットのほかに Node と wasm-bindgen の CLI の導入（版固定・SHA256 検証）と版の確認のステップがある
- `deny` ジョブには、上の make ターゲットのほかに cargo-deny の導入・版の確認・jq / python3 の版の表示のステップがある（枠の検査ではないため表から外した）

### CI が呼ばない make ターゲット

| ターゲット | 理由（コード内の記述による） |
| ---------- | ---------------------------- |
| `help`・`doctor`・`setup` | 開発者向けの案内・環境確認・導入。検査ではない |
| `dep-graph-wgpu-check` | `make deny` の中で同じスクリプトを呼ぶので、別ステップにしない（`scripts/deny.sh`） |
| `inventory-trace-check` | 入力が private 側にあり、本リポの CI には組み込まない（`ci.yml` の `deny` ジョブのコメント） |

`scripts/session-record-check.sh`・`scripts/metal-session.sh`・`scripts/record-summary.sh`・`scripts/public-api-diff.sh`・`scripts/rerun-compare.sh` の本体は引数が要るため make ターゲットを持たない（各スクリプトの先頭コメントと [makefile](../.claude/rules/makefile.md)）。

## `ci-frame-check`（`scripts/ci-frame-check.sh`）

`ci.yml` 自身を入力にして枠の事実を確かめる。YAML の汎用パーサではなく、行とインデントだけを読む（Python 3.8 以上の標準ライブラリ）。

### 入口と終了コード

| 入口 | 内容 |
| ---- | ---- |
| `make ci-frame-check` | 既定の入力 `.github/workflows/ci.yml` を検査する |
| `scripts/ci-frame-check.sh` | 直接実行 |
| `scripts/ci-frame-check.sh --workflow <path>` | 入力を差し替える（回帰テストが合成 fixture を渡すために使う） |

| 終了コード | 意味 |
| ---------- | ---- |
| 0 | 合格（`ok: ci workflow frame (<ジョブ数> jobs, rules F1-F6)` を標準出力へ） |
| 1 | 違反（`violation: <規則> line <行> job <ジョブ>: <文言>` と `FAILED: <件数> violation(s)` を標準エラーへ） |
| 2 | 誤用・前提の不足（未知の引数・パスが無い・python3 が無い。成功扱いにしない） |

入力を読むだけで、ファイルを作らず、ネットワークを使わない。

### 規則

許可ラベルの定数はスクリプトの 1 か所だけ。

| 規則 | 内容 |
| ---- | ---- |
| F1 | 集約ジョブ `ci-complete` があり、ジョブ単位の `if` が `always()` である |
| F2 | `ci-complete` の `needs` が、それ以外の全ジョブ ID と過不足なく一致する |
| F3 | 各ジョブの `runs-on` が許可ラベル（`ubuntu-latest`・`macos-latest`）か、リテラルの並びの matrix 参照である（matrix に `include` / `exclude` があれば読み切れないとして違反） |
| F4 | `continue-on-error` のキーがどこにも無い（値・引用符・フロー形式の中でも問わない） |
| F5 | コメント行以外に `\|\| true`・`\|\| :` の綴りが無い |
| F6 | `always()` が F1 の 1 か所以外に現れない |

読めない形（タブのインデント・アンカー・マージキー・対応外の `needs` / `runs-on` / matrix の書き方・ジョブ ID の重複・`jobs` の欠落）は違反にする。行の途中の `#` 以降は読み飛ばさないので、行中コメントの綴りも違反になる。

### 検査しないもの

`ci-complete` の判定式の中身、`ci.yml` 以外の workflow、ruleset の登録、ステップ単位の `if`、F5 の綴り以外の握りつぶし（`set +e` など）。

## 規則とテストの対応

テストは `scripts/test-ci-frame-check.sh`（`make test-ci-frame-check`）。合成 fixture と実物の写しだけを使い、リポジトリのファイルを変更しない。判定は終了コード・標準出力・標準エラーの部分一致・違反の件数で行う。ケース名の接頭辞が規則に対応する。

| 規則・項目 | ケース名（接頭辞または名前） |
| ---------- | ---------------------------- |
| 合格 | `pass: real ci.yml (no arguments)`、`pass: synthetic base`、`pass: needs in flow form`、`pass: needs in flow form with trailing comment`、`pass: '\|\| { ...; exit 1; }' form`、`pass: spellings only in comment lines`、`pass: indented comment line` |
| F1 | `F1: ci-complete without if`、`F1: ci-complete with other condition`、`F1: always() with extra condition`、`F1: ci-complete missing` |
| F2 | `F2: needs lacks a job`、`F2: needs names unknown job and lacks one`、`F2: needs scalar covers only one job`、`F2: needs in unsupported form`、`F2: needs missing`、`F2: real ci.yml copy without one needs line` |
| F3 | `F3: self-hosted`、`F3: runs-on as array`、`F3: label outside the allowed set`、`F3: matrix element outside the allowed set`、`F3: other expression`、`F3: matrix key not found`、`F3: job without runs-on`、`F3: matrix from an expression`、`F3: matrix include is rejected`、`F3: matrix exclude is rejected` |
| F4 | `F4: step-level continue-on-error`、`F4: job-level continue-on-error with false`、`F4: single-quoted continue-on-error key`、`F4: double-quoted continue-on-error key` |
| F5 | `F5: '\|\| true'`、`F5: '\|\| :'`、`F5: spelling in a trailing comment is still a violation` |
| F6 | `F6: step-level always() in another job`、`F6: job-level always() in another job` |
| 複数の違反 | `multiple: F3 and F5` |
| 読めない形 | `unreadable: no jobs key`、`unreadable: tab in indentation`、`unreadable: duplicate job id`、`unreadable: anchor`、`unreadable: merge key` |
| 誤用（終了コード 2） | `misuse: unknown argument`、`misuse: stray positional`、`misuse: --workflow without value`、`misuse: path does not exist`、`misuse: directory instead of file` |
| 入力を変更しない | `invariant: input and directory unchanged` |

python3 が無い場合の終了コード 2 を確かめるケースは無い。

## 検査を足すときの手順

1. 実体を `scripts/<name>.sh` に置く（[makefile](../.claude/rules/makefile.md) の「新規スクリプトの規約」）
2. `make` のターゲットを足し、同期点（`.PHONY`・`## `・`scripts/help.sh`・契約の表・`README.md`・`CLAUDE.md`）を同じ PR で直す
3. 回帰テスト（`scripts/test-<name>.sh` と `make test-<name>`）を足す
4. 「分類」の表で選んだ既存ジョブへ、検査と回帰テストのステップを足す。`ci.yml` 先頭のコメントの列挙と、規約の必須チェックの括弧書きを合わせる。本書の対応表も直す
5. ツールを足すなら版固定 + SHA256 検証（[github-actions](../.claude/rules/github-actions.md) のジョブ追加の 7）。外部依存の追加は承認フロー（[dependency-policy](../.claude/rules/dependency-policy.md)）
6. `lefthook` と重複させない（[git-hooks](../.claude/rules/git-hooks.md)）
7. PR の題・ブランチ名など PR 由来の文字列を `run` へ直接展開しない（`pr-title` の先例）

## 確かめ方

| コマンド | 確かめること |
| -------- | ------------ |
| `make ci-frame-check` | 実物の `ci.yml` が F1〜F6 を満たす（終了コード 0） |
| `make test-ci-frame-check` | 検査自体の回帰テスト（末尾に `passed: <件数>, failed: <件数>`） |
| `make check` | 整形・シェルの静的検査 |
| `grep -n "run: make " .github/workflows/ci.yml` | 上の対応表と `ci.yml` の突き合わせ |

`ci.yml`・`scripts/ci-frame-check.sh` を変えたら、`make ci-frame-check` と `make test-ci-frame-check` を通す。

## 関連

- 関連: TASK-18（[ci-job-split](./ci-job-split.md)）、TASK-23（[test-frame](./test-frame.md)）
- [unsafe-policy](./unsafe-policy.md)
- [github-actions](../.claude/rules/github-actions.md)・[makefile](../.claude/rules/makefile.md)・[git-hooks](../.claude/rules/git-hooks.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
