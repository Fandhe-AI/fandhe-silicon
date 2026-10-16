---
paths:
  - "Makefile"
  - "scripts/*.sh"
description: >
  Makefile と scripts/*.sh（scripts/hooks/ を除く）を編集する際の方針。Makefile は scripts/ を呼ぶだけの
  薄い入口とし、処理の定義は scripts/ に置く。ターゲットを増減するときの同期点と、各ターゲットの契約を規定する。
---

# Makefile / scripts 編集ルール

`scripts/hooks/**` と `lefthook.yml` は [git-hooks.md](./git-hooks.md) の担当。本ファイルは扱わない。
詳細な設計指針は make スキル（`.claude/skills/make`）を参照する。

## 基本方針

- Makefile は**薄い入口**。レシピは `@scripts/<name>.sh` か、言語ネイティブの 1 行コマンドのみ
- 複数行・条件分岐・`for`・環境依存の処理は `scripts/` の bash スクリプトへ置く
- 中身の無いターゲット（`@true` 等）は置かない（呼び出し元が終了コードを信用してしまう）
- GNU Make 3.81（macOS 標準）互換。`.ONESHELL`・`.RECIPEPREFIX` 等の 3.82+ 専用機能は使わない
- `.DEFAULT_GOAL := help` を維持する。一覧は各ターゲット行の `## ` コメントから生成される

## ターゲットを追加・改名・削除するとき

1. `.PHONY` を更新する
2. `## ` コメントを書く（無いターゲットは `make help` に出ない）
3. `scripts/help.sh`（Make 非依存の一覧）を同じ変更で同期する
4. 呼び出し元を洗い出して更新する: `grep -rn "make " .github .claude docs README.md CLAUDE.md AGENTS.md lefthook.yml scripts`
5. `make help` と `make -n <target>` で確認する

## 新規スクリプトの規約

- 先頭に `#!/usr/bin/env bash` と `set -euo pipefail`、`scripts/lib.sh` を `source` する
- 引数を取らないなら `reject_args`、必須コマンドは `require_cmd` を使う
- 実行権限を付与し（`chmod +x`）、`shellcheck` を通す（`make check` が `scripts/**/*.sh` を検査する）
- エラー出力は英語（既存の `lib.sh` に合わせる）。人向けのコメントは日本語

## 既存ターゲットの契約

| ターゲット | 契約 |
| ---------- | ---- |
| `check` | editorconfig-checker + shellcheck。ソースを変更しない。ツール未導入は成功扱いにせず失敗 |
| `verify` | `cargo fmt --check` + clippy（`-D warnings`）+ test。ソースを変更しない（整形は別途 `cargo fmt --all`） |
| `build-profiles` | feature の組ごとの build / clippy / test と wasm32 の check。ソースを変更しない。wasm32 の target 未導入は失敗（skip しない） |
| `deny` | cargo-deny の検査 + workspace の path が `crates/` の外を指さないことの検査（TASK-6）+ 製品の workspace の依存グラフの wgpu 系 crate の検査（`dep-graph-wgpu-check.sh`。TASK-11 のポインタ。python3・git が要る）+ `measure/`・`baseline-wgpu/`・`wasm/` の cargo-deny（`baseline-wgpu/` は `baseline-wgpu/deny.toml` を `--config` で明示。他はルートの `deny.toml`）+ `baseline-wgpu/`・`wasm/` の deps-check（TASK-27・TASK-22。`wasm/` は `--allow-path-root crates`。jq が要る・ビルドはしない） |
| `wasm-verify` | wasm32 向けの別 workspace（`wasm/`）の `cargo fmt --check` + clippy（`-D warnings`。ホストと wasm32）+ test（ホスト）+ wasm32 の build（`--profile wasm-size`）と `.wasm` の存在確認 + `deps-check.sh --allow-path-root crates wasm/Cargo.toml`（TASK-22）。すべて `--locked`。ソースを変更しない・初回は依存の取得が要る。wasm32 の target 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ。回帰テストは持たない（検査本体を持たず既存の道具を順に呼ぶ入口のため） |
| `measure-verify` | 測定専用の別 workspace（`measure/`）の `cargo fmt --check` + clippy（`-D warnings`）+ test（`--locked`）+ `deps-check.sh measure/Cargo.toml`。ソースを変更しない・ネットワーク不要。ツール未導入は失敗。CI の `deny` ジョブが呼ぶ。回帰テストは持たない（検査本体を持たず既存の道具を順に呼ぶ入口のため） |
| `wasm-size` | wasm-bindgen の glue を含めた成果物の量の測定（TASK-22・BUILD-42・T-12 のポインタ）。`wasm/` を 4 設定（profile × 命令）でビルドし CLI で web・bundler の glue を生成して、gzip 後の合計を閾値と比べる。`--from-dir <dir>` なら生成済みの出力だけを測る。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要。終了コード 0 全部の組が閾値以下・1 閾値超えまたは未実施の組がある・2 誤用または前提の不足（CLI 無し・版違い・target 無し・依存未取得を含む）または読めない入力。python3・cargo・CLI 未導入は失敗（skip しない）。CI は `wasm-macos` ジョブが呼ぶ（CLI を入れる。`deny` ジョブは呼ばない） |
| `test-wasm-size` | `scripts/wasm-size.sh` の回帰テスト。合成の入力と、一時ディレクトリの偽の cargo / rustc / wasm-bindgen を `PATH` の先頭に入れた既定の経路だけを使い、CLI・Node・ネットワーク・`docs/spec` の取得は要らない。リポジトリのファイルを変更しない。判定は終了コード + 出力の部分一致。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `deps-check` | 依存の版指定が `=x.y.z` の完全固定かと承認の記録（`deps-approved.toml`）との一致の検査（cargo metadata + jq）。`--allow-path-root <dir>` を付けた直接実行だけ、`<dir>` 配下への path 依存を免除する（`wasm/` の検査用。make ターゲットは付けない）。ソースを変更しない・ネットワーク不要。ツール未導入は失敗、違反は終了コード 1 |
| `test-deps-check` | `scripts/deps-check.sh` の回帰テスト。一時ディレクトリの fixture だけを使い、リポジトリのファイルを変更しない・ネットワーク不要。ツール未導入は失敗（skip しない） |
| `test-inventory-trace-check` | `scripts/inventory-trace-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要・`docs/spec` の取得は不要。判定は終了コード + 標準出力 + 標準エラーの部分一致 + 違反の件数。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `test-session-record-check` | `scripts/session-record-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。python3 未導入は失敗（skip しない）。CI の `deny` ジョブと `test-hooks` ジョブ（macOS を含む）が呼ぶ。検査本体は引数（記録のパス）が要るため make ターゲットを持たない（漏れではない。直接実行する） |
| `test-perf-record-check` | `scripts/perf-record-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・cargo もネットワークも不要。判定は終了コード + 標準出力の完全一致 + 標準エラーの部分一致。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ。検査本体は引数（run の記録のパス）が要るため make ターゲットを持たない（漏れではない。直接実行する） |
| `test-metal-session` | `scripts/metal-session.sh` の回帰テスト。macOS のコマンド（`uname`・`date`・`pmset`・`sysctl`・`caffeinate`・`xcode-select`）は一時ディレクトリの偽のコマンドを `PATH` の先頭に入れて差し替えるので Linux でも全部流れる。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致（先頭が `!` の語は含まれないこと）。python3・git 未導入は失敗（skip しない）。本体はテストを動かしている bash で起動する（macOS の CI は `/bin/bash` で直接も実行する）。CI の `deny` ジョブと `test-hooks` ジョブ（macOS を含む）が呼ぶ。本体はサブコマンドと引数が要るため make ターゲットを持たない（漏れではない。直接実行する） |
| `test-record-summary` | `scripts/record-summary.sh`（試験の枠の記録の部品のディレクトリから、機材の組み合わせ × ラベルの件数の表を標準出力に出す）の回帰テスト。合成の部品と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 標準出力の完全一致 + 標準エラーの部分一致 + 違反の件数。python3 未導入は失敗（skip しない）。本体はテストを動かしている bash で起動する。本体は引数（部品のディレクトリ）が要るため make ターゲットを持たない（漏れではない。直接実行する。終了コード 0 集計を出した・1 違反または部品が 0 件・2 誤用または前提の不足） |
| `ci-frame-check` | `.github/workflows/ci.yml`（`--workflow <path>` で差し替え可）が CI の枠の事実を保っているかの検査（集約ジョブの存在と `needs` の網羅・`runs-on` の許可ラベル・失敗の握りつぶしの綴りが無いこと・`always()` の場所）。入力は読むだけ・ネットワーク不要。終了コード 0 合格・1 違反・2 誤用または前提の不足。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `test-ci-frame-check` | `scripts/ci-frame-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `public-api-check` | 4 crate の公開 API に wgpu 系（`wgpu*`・`naga`）の crate の型・経路が出ていないことの検査（TASK-10 のポインタ）。crate ごとに 2 組の feature で rustdoc の JSON を作って参照を数える（`--json <path>...` なら渡した JSON だけを検査し cargo を呼ばない。`--emit-json <dir>` なら JSON を作って写すだけで検査しない）。JSON を作る段は `cargo target_directory` 配下の専用の target ディレクトリを使い、ソースを変更しない。標準出力は組ごとの `scanned:` の行と、全部の文書を検査し終えたとき（終了コード 0・1）だけ出す締めの `record:` の行（件数の合計とコミット）。JSON を作る経路は git で HEAD のコミットを取り、取れなければ終了コード 2。終了コード 0 参照なし・1 参照あり・2 誤用または前提の不足または読み切れない JSON。python3・cargo 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `public-name-check` | 製品の crate の公開アイテムの名前に語の一覧（`public-name-check.conf`）の語が出ていないことの検査（TASK-113 のポインタ）。`public-api-check.sh --emit-json` で作った rustdoc の JSON の公開アイテムの名前を数える（`--json <path>...` なら渡した JSON だけで cargo を呼ばない。`--source <path>...` を渡したときだけソースの単語・呼び出しの形も数える）。専用の target ディレクトリを使いソースを変更しない・ネットワーク不要。標準出力は `scanned:`・`scanned-source:` の行と締めの `record:`。終了コード 0 一致なし・1 一致あり・2 誤用または前提の不足または読み切れない入力。python3・cargo 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `test-public-api-check` | `scripts/public-api-check.sh` の回帰テスト。合成 JSON の節と、実行時に一時ディレクトリへ作る wgpu 系の名前の合成 path crate の一時 workspace で実物の `cargo rustdoc` の JSON を通す節（比較基準の `baseline-wgpu/` ではない）。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `test-public-api-sdk-check` | `scripts/public-api-sdk-check.sh`（公開 API と共通の crate の依存にチップの SDK の crate が無いことの検査。TASK-13 のポインタ）の回帰テスト。合成 rustdoc JSON・合成 cargo metadata の節（完全一致の陰性・違反・誤用・複数本）、偽の git / cargo / JSON を作る段と PATH を絞った環境の節、実行時に一時ディレクトリへ作る合成 path crate の一時 workspace で実物の rustdoc の JSON と cargo metadata を通す節。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI はまだ呼ばない。本体は make ターゲットを持たない（直接実行する） |
| `dep-graph-wgpu-check` | 製品の workspace の依存グラフ（全 feature・全 target）の wgpu 系 crate を狭い定義と広い定義で数えて検査する（TASK-11 のポインタ）。件数は標準出力の `record:` の行に出す。終了コード 0 合格・1 違反・2 誤用または前提の不足。ソースを変更しない・ネットワーク不要。python3・cargo・git 未導入は失敗（skip しない）。`make deny`（`scripts/deny.sh`）が呼ぶので、CI の `deny` ジョブの `make deny` の中で流れる（別ステップにしない） |
| `test-dep-graph-wgpu-check` | `scripts/dep-graph-wgpu-check.sh`（TASK-11 のポインタ）の回帰テスト。合成 JSON・合成 tree の節（2 定義とも 0 件・狭い定義 1 件・広い定義だけ 1 件の失敗・読めない入力は終了コード 2）と、実行時に一時ディレクトリへ作る合成 path crate の一時 workspace で実物の `cargo metadata` / `cargo tree` を通す節（比較基準の `baseline-wgpu/` ではない）。偽の cargo / git と PATH を絞った環境でツール未導入も確かめる。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `baseline-positive-control` | 比較基準の別 workspace（`baseline-wgpu/`）に `dep-graph-wgpu-check.sh`（`--allow-metadata-superset` 付き）と `public-api-check.sh` を `--manifest-path` で当てる陽性対照（TASK-12 のポインタ）。内側の終了コードと `record:` の行を読み、狭い定義・広い定義・公開 API のそれぞれが 1 件以上で合格（0 件の定義があれば終了コード 1・読めない入力・食い違い・2 や想定外は終了コード 2）。片方が失敗しても両方を流し、`record:` の行をログに残す。`make deny` には含めない（`make deny` は製品の workspace だけ）。内側の cargo は `--locked` で、ソースと `Cargo.lock` を変更しない。python3・cargo・git 未導入は失敗（skip しない）。CI の `deny` ジョブが `make deny` の後のステップで呼ぶ |
| `test-baseline-positive-control` | `scripts/baseline-positive-control.sh` の回帰テスト。本体を一時ディレクトリへ写し、偽の検査で定義ごとの判定と引数の渡し方を確かめ、一時 workspace の実物の出力でも通す。python3・cargo・git が要る（無ければ失敗）・ネットワーク不要・リポジトリのファイルを変更しない。判定は終了コード + 出力の部分一致。CI の `deny` ジョブが呼ぶ |
| `test-public-name-check` | `scripts/public-name-check.sh` の回帰テスト。ダミーの語の一覧と合成 JSON・合成ソースを一時ディレクトリに作って渡し、一時 workspace の実物の rustdoc の JSON でも既定の経路を流す。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `test-public-api-diff` | `scripts/public-api-diff.sh`（同じ crate の 2 つの版の rustdoc の JSON から公開 API の差分を出す。TASK-17 のポインタ）の回帰テスト。合成 JSON だけを一時ディレクトリに作って渡し、リポジトリのファイルを変更しない・cargo もネットワークも不要。判定は終了コード + 出力の部分一致。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ。本体は比べる JSON のパスが要るため make ターゲットを持たない（漏れではない。直接実行する。終了コード 0 差分なし・1 差分あり・2 誤用または前提の不足または読み切れない JSON） |
| `test-rerun-compare` | `scripts/rerun-compare.sh`（試験の枠の記録の部品の 2 つのディレクトリを (試験の ID, 組み合わせ) で突き合わせ、ラベルと理由の差を分類して出す。TASK-29 のポインタ）の回帰テスト。合成の部品だけを一時ディレクトリに作って渡し、リポジトリのファイルを変更しない・cargo もネットワークも不要。判定は終了コード + 出力の部分一致（並びは完全一致）。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ。本体は記録のパスが要るため make ターゲットを持たない（漏れではない。直接実行する。終了コード 0 差分なし・1 差分あり・2 誤用・前提の不足・読めない記録） |
| `inventory-trace-check` | 一覧の鍵が項目ファイルに現れるかの検査（既定はファイル単位。項目の行〔表のデータ行〕への帰属は直接実行で `--item-rows` を付けたときだけ。TASK-16 のポインタ。bash + python3）。`--list` で合格のときだけ対応の一覧を標準出力へ出す（保存しない）。入力は読むだけ・ネットワーク不要。終了コード 0 合格・1 違反・2 誤用または入力なし（`--item-id-column` が解決できない場合を含む）（入力が無ければ成功扱いにしない）。CI は呼ばない |
| `wasm-node-check` | wasm32 の `.wasm` を wasm-bindgen の glue 越しに Node で呼び、結果をネイティブの参照と照合して行ごとに match / mismatch / not-run を出す土台（TASK-22）。Node は `v24.13.0`、wasm-bindgen の CLI は `wasm/crates/glue/Cargo.toml` の版と完全一致を要求し、無い・版違いは失敗。参照の無い行は一致にしない。終了コード 0 全行 match・1 mismatch か not-run か行 0 件・2 誤用・前提の不足・入力や結果が不正・Node の失敗。ネットワークは使わない・生成物は一時ディレクトリだけ。`--record-dir <dir> --record-combination <name>` を組で渡したときだけ、行の結果を試験の枠の記録の部品として `<dir>` へ新しく書く（上書きしない・失敗は終了コード 2・選ばなければ出力と終了コードは不変）。実物の実行で、CI は `wasm-macos` ジョブが呼ぶ |
| `test-wasm-node-check` | `scripts/wasm-node-check.sh` の回帰テスト。node・wasm-bindgen・cargo は一時ディレクトリの偽のコマンドを `PATH` の先頭に入れて差し替え、`run-glue.cjs` は本物の node で偽の glue を読ませる。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致（先頭が `!` の語は含まれないこと）。python3・node 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `test-hooks` | `scripts/hooks/` の回帰テスト（bash + git のみ） |
| `doctor` | 診断のみ。何も導入しない |
| `setup` | Git hooks 有効化と `.env` 作成。再実行可（冪等） |

CI（`.github/workflows/ci.yml`）はこれらのターゲットを呼ぶ。名前・引数・終了コードを変えるときは CI も同じ PR で更新する。
