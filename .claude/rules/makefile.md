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
| `deny` | cargo-deny の検査 + workspace の path が `crates/` の外を指さないことの検査（TASK-6）+ 製品の workspace の依存グラフの wgpu 系 crate の検査（`dep-graph-wgpu-check.sh`。TASK-11 のポインタ。python3・git が要る）+ 契約の crate のプロファイルごとの依存グラフの検査と陽性対照（`profile-dep-graph-check.sh`・`profile-dep-graph-positive-control.sh`。TASK-228 のポインタ）+ `measure/`・`baseline-wgpu/`・`wasm/` の cargo-deny（`baseline-wgpu/` は `baseline-wgpu/deny.toml` を `--config` で明示。他はルートの `deny.toml`）+ `baseline-wgpu/`・`wasm/` の deps-check（TASK-27・TASK-22。`wasm/` は `--allow-path-root crates`。jq が要る・ビルドはしない） |
| `wasm-verify` | wasm32 向けの別 workspace（`wasm/`）の `cargo fmt --check` + clippy（`-D warnings`。ホストと wasm32）+ test（ホスト）+ wasm32 の build（`--profile wasm-size`）と `.wasm` の存在確認 + `deps-check.sh --allow-path-root crates wasm/Cargo.toml`（TASK-22）。すべて `--locked`。ソースを変更しない・初回は依存の取得が要る。wasm32 の target 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ。回帰テストは持たない（検査本体を持たず既存の道具を順に呼ぶ入口のため） |
| `measure-verify` | 測定専用の別 workspace（`measure/`）の `cargo fmt --check` + clippy（`-D warnings`）+ test（`--locked`）+ `deps-check.sh --allow-path-root crates measure/Cargo.toml`。ソースを変更しない・ネットワーク不要。ツール未導入は失敗。CI の `deny` ジョブが呼ぶ。回帰テストは持たない（検査本体を持たず既存の道具を順に呼ぶ入口のため） |
| `wasm-size` | wasm-bindgen の glue を含めた成果物の量の測定（TASK-22・BUILD-42・T-12 のポインタ）。`wasm/` を 4 設定（profile × 命令）でビルドし CLI で web・bundler の glue を生成して、gzip 後の合計を閾値と比べる。`--from-dir <dir>` なら生成済みの出力だけを測る。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要。終了コード 0 全部の組が閾値以下・1 閾値超えまたは未実施の組がある・2 誤用または前提の不足（CLI 無し・版違い・target 無し・依存未取得を含む）または読めない入力。python3・cargo・CLI 未導入は失敗（skip しない）。CI は `wasm-macos` ジョブが呼ぶ（CLI を入れる。`deny` ジョブは呼ばない） |
| `test-wasm-size` | `scripts/wasm-size.sh` の回帰テスト。合成の入力と、一時ディレクトリの偽の cargo / rustc / wasm-bindgen を `PATH` の先頭に入れた既定の経路だけを使い、CLI・Node・ネットワーク・`docs/spec` の取得は要らない。リポジトリのファイルを変更しない。判定は終了コード + 出力の部分一致。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `deps-check` | 依存の版指定が `=x.y.z` の完全固定かと承認の記録（`deps-approved.toml`）との一致の検査（cargo metadata + jq）。`--allow-path-root <dir>` を付けた直接実行だけ、`<dir>` 配下への path 依存を免除する（`wasm/`・`measure/` の検査用。make ターゲットは付けない）。ソースを変更しない・ネットワーク不要。ツール未導入は失敗、違反は終了コード 1 |
| `test-deps-check` | `scripts/deps-check.sh` の回帰テスト。一時ディレクトリの fixture だけを使い、リポジトリのファイルを変更しない・ネットワーク不要。ツール未導入は失敗（skip しない） |
| `test-inventory-trace-check` | `scripts/inventory-trace-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要・`docs/spec` の取得は不要。判定は終了コード + 標準出力 + 標準エラーの部分一致 + 違反の件数。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `test-session-record-check` | `scripts/session-record-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。python3 未導入は失敗（skip しない）。CI の `deny` ジョブと `test-hooks` ジョブ（macOS を含む）が呼ぶ。検査本体は引数（記録のパス）が要るため make ターゲットを持たない（漏れではない。直接実行する） |
| `test-perf-record-check` | `scripts/perf-record-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・cargo もネットワークも不要。判定は終了コード + 標準出力の完全一致 + 標準エラーの部分一致。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ。検査本体は引数（run の記録のパス）が要るため make ターゲットを持たない（漏れではない。直接実行する） |
| `test-metal-session` | `scripts/metal-session.sh` の回帰テスト。macOS のコマンド（`uname`・`date`・`pmset`・`sysctl`・`caffeinate`・`xcode-select`）は一時ディレクトリの偽のコマンドを `PATH` の先頭に入れて差し替えるので Linux でも全部流れる。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致（先頭が `!` の語は含まれないこと）。python3・git 未導入は失敗（skip しない）。本体はテストを動かしている bash で起動する（macOS の CI は `/bin/bash` で直接も実行する）。CI の `deny` ジョブと `test-hooks` ジョブ（macOS を含む）が呼ぶ。本体はサブコマンドと引数が要るため make ターゲットを持たない（漏れではない。直接実行する） |
| `test-record-summary` | `scripts/record-summary.sh`（試験の枠の記録の部品のディレクトリから、機材の組み合わせ × ラベルの件数の表を標準出力に出す）の回帰テスト。合成の部品と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 標準出力の完全一致 + 標準エラーの部分一致 + 違反の件数。python3 未導入は失敗（skip しない）。本体はテストを動かしている bash で起動する。本体は引数（部品のディレクトリ）が要るため make ターゲットを持たない（漏れではない。直接実行する。終了コード 0 集計を出した・1 違反または部品が 0 件・2 誤用または前提の不足） |
| `ci-frame-check` | `.github/workflows/ci.yml`（`--workflow <path>` で差し替え可）が CI の枠の事実を保っているかの検査（集約ジョブの存在と `needs` の網羅・`runs-on` の許可ラベル・失敗の握りつぶしの綴りが無いこと・`always()` の場所）。入力は読むだけ・ネットワーク不要。終了コード 0 合格・1 違反・2 誤用または前提の不足。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `test-ci-frame-check` | `scripts/ci-frame-check.sh` の回帰テスト。合成 fixture と一時ディレクトリだけを使い、リポジトリのファイルを変更しない・ネットワーク不要。python3 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `public-api-check` | 4 crate の公開 API に wgpu 系（`wgpu*`・`naga`）の crate の型・経路が出ていないことの検査（TASK-10 のポインタ）。crate ごとに 2 組の feature で rustdoc の JSON を作って参照を数える（`--json <path>...` なら渡した JSON だけを検査し cargo を呼ばない。`--emit-json <dir>` なら JSON を作って写すだけで検査しない。`--emit-json` と組の `--without-feature <名前>` なら契約の production の feature 列から名前を除いた 1 組だけを作る）。JSON を作る段は `cargo target_directory` 配下の専用の target ディレクトリを使い、ソースを変更しない。標準出力は組ごとの `scanned:` の行と、全部の文書を検査し終えたとき（終了コード 0・1）だけ出す締めの `record:` の行（件数の合計とコミット）。JSON を作る経路は git で HEAD のコミットを取り、取れなければ終了コード 2。終了コード 0 参照なし・1 参照あり・2 誤用または前提の不足または読み切れない JSON。python3・cargo 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `public-name-check` | 製品の crate の公開アイテムの名前に語の一覧（`public-name-check.conf`）の語が出ていないことの検査（TASK-113 のポインタ）。`public-api-check.sh --emit-json` で作った rustdoc の JSON の公開アイテムの名前を数える（`--json <path>...` なら渡した JSON だけで cargo を呼ばない。`--source <path>...` を渡したときだけソースの単語・呼び出しの形も数える）。専用の target ディレクトリを使いソースを変更しない・ネットワーク不要。標準出力は `scanned:`・`scanned-source:` の行と締めの `record:`。終了コード 0 一致なし・1 一致あり・2 誤用または前提の不足または読み切れない入力。python3・cargo 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `public-api-webgpu-check` | feature `native-blocking-wait` を外した構成で、契約のコア（`crates/core`・`crates/contract`）の公開面に、語の一覧（`public-api-webgpu-check.conf`）の語を `_` 区切りの語として含む関数・メソッド・再公開が無いことの検査（TASK-241 のポインタ）。`public-api-check.sh --emit-json <dir> --without-feature native-blocking-wait` で作った 2 本の rustdoc の JSON を数える（`--json <path>...` なら渡した JSON だけで cargo・git を呼ばない）。標準出力は `scanned:` の行と締めの `record:`。終了コード 0 一致なし・1 一致あり・2 誤用または前提の不足または読み切れない入力（core と contract の文書がそろわない場合を含む）。ソースを変更しない・ネットワーク不要。python3・cargo・git 未導入は失敗（skip しない）。CI にはまだ組み込まない（手元で流す） |
| `test-public-api-check` | `scripts/public-api-check.sh` の回帰テスト。合成 JSON の節と、実行時に一時ディレクトリへ作る wgpu 系の名前の合成 path crate の一時 workspace で実物の `cargo rustdoc` の JSON を通す節（比較基準の `baseline-wgpu/` ではない）。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `public-api-sdk-check` | 製品の 4 crate の公開 API と共通の crate（`crates/core`）の依存宣言にチップの SDK の crate が無いことの検査（TASK-13 のポインタ）。rustdoc の JSON の参照と cargo metadata の依存宣言の 2 側を数える（`--json <path>...` / `--metadata-json <path>` なら渡した側だけで cargo・git を呼ばない）。JSON を作る段は `public-api-check.sh --emit-json` を共有する。専用の target ディレクトリを使いソースを変更しない・ネットワーク不要。標準出力は `scanned:` の行と締めの `record:`。終了コード 0 一致なし・1 一致あり・2 誤用または前提の不足または読み切れない入力。python3・cargo・git 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `test-public-api-sdk-check` | `scripts/public-api-sdk-check.sh`（公開 API と共通の crate の依存にチップの SDK の crate が無いことの検査。TASK-13 のポインタ）の回帰テスト。合成 rustdoc JSON・合成 cargo metadata の節（完全一致の陰性・違反・誤用・複数本）、偽の git / cargo / JSON を作る段と PATH を絞った環境の節、実行時に一時ディレクトリへ作る合成 path crate の一時 workspace で実物の rustdoc の JSON と cargo metadata を通す節。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI の deny ジョブも実行する |
| `core-deps-check` | 共通の crate（`crates/core`）が宣言する外部の crate の一覧が決められた一覧と完全に一致することと、その feature が契約の crate の同じ名前の feature から転送されていることの検査（TASK-258 のポインタ）。cargo metadata の依存の宣言と feature を読む（`--metadata-json <path>` なら渡した metadata だけで cargo・git を呼ばない）。期待の一覧はスクリプトの 1 か所だけに持つ。ソースを変更しない・ネットワーク不要。標準出力は `scanned:` の行と締めの `record:`。終了コード 0 違反なし・1 違反あり・2 誤用または前提の不足または読み切れない入力。python3・cargo・git 未導入は失敗（skip しない）。CI のジョブからはまだ呼ばない |
| `test-core-deps-check` | `scripts/core-deps-check.sh`（TASK-258 のポインタ）の回帰テスト。合成 cargo metadata の節（合格・依存の違反・feature の違反・誤用）、偽の git / cargo と PATH を絞った環境の節、実行時に一時ディレクトリへ作る一時 workspace で実物の cargo metadata を通す節（外部の crate を足すと落ちること）、実物のリポジトリの節。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI のジョブからはまだ呼ばない |
| `dep-graph-wgpu-check` | 製品の workspace の依存グラフ（全 feature・全 target）の wgpu 系 crate を狭い定義と広い定義で数えて検査する（TASK-11 のポインタ）。件数は標準出力の `record:` の行に出す。終了コード 0 合格・1 違反・2 誤用または前提の不足。ソースを変更しない・ネットワーク不要。python3・cargo・git 未導入は失敗（skip しない）。`make deny`（`scripts/deny.sh`）が呼ぶので、CI の `deny` ジョブの `make deny` の中で流れる（別ステップにしない） |
| `test-dep-graph-wgpu-check` | `scripts/dep-graph-wgpu-check.sh`（TASK-11 のポインタ）の回帰テスト。合成 JSON・合成 tree の節（2 定義とも 0 件・狭い定義 1 件・広い定義だけ 1 件の失敗・読めない入力は終了コード 2）と、実行時に一時ディレクトリへ作る合成 path crate の一時 workspace で実物の `cargo metadata` / `cargo tree` を通す節（比較基準の `baseline-wgpu/` ではない）。偽の cargo / git と PATH を絞った環境でツール未導入も確かめる。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI の `deny` ジョブが呼ぶ |
| `profile-dep-graph-check` | 契約の crate（`fandhe-silicon-contract`）をプロファイルの feature の組にした依存グラフ（`cargo tree -e normal,build,dev --target all`。CpuOnly と ComputeOnly の Metal の組。後者は `-e features`〔逆向きを含む〕で `objc2-metal` の per-header feature も数える）で、判定に使う crate 名を数えて検査する（TASK-228・BUILD-1・BUILD-2 のポインタ）。件数は標準出力の `record:`・`count:` の行に出す。`--tree-text <path> --profile <name>` なら渡した tree だけを数えて cargo・git を呼ばない（per-header feature を数えるプロファイルは `--features-text <path>` が必須、他は受け付けない）。`--extra-judged-crate <名前>` は確かめ用の opt-in で、全プロファイルの判定の名前の一覧の末尾に 1 件だけ足して数える（足す向きにしか働かない。既に一覧にある名前・2 回の指定・不正な文字は終了コード 2。渡したときだけ `record:`・`ok:` の末尾に `extra_judged_crate=` が付く）。終了コード 0 合格・1 違反・2 誤用・前提の不足・読めない入力・0 件の tree。ソースと `Cargo.lock` を変更しない・ネットワーク不要。python3・cargo・git 未導入は失敗（skip しない）。`make deny`（`scripts/deny.sh`）が呼ぶので、CI の `deny` ジョブの `make deny` の中で流れる（別ステップにしない） |
| `test-profile-dep-graph-check` | `scripts/profile-dep-graph-check.sh`（TASK-228 のポインタ）の回帰テスト。合成 tree の節（0 件・違反・名前の折りたたみ・接頭辞に広げない・per-header feature の判定と記録のみ・読めない入力は終了コード 2）、偽の cargo / git と PATH を絞った環境の節、一時 workspace の実物の `cargo tree` の節（feature の組・全 target・dev の依存・per-header feature の各経路）、実物のリポジトリの節。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI のジョブからは呼ばない（手元で流す） |
| `profile-dep-graph-positive-control` | `profile-dep-graph-check.sh` の陽性対照（TASK-228・BUILD-1・BUILD-2 のポインタ）。内側を引数なしで流して終了コード 0 を基準に取り、次に `--extra-judged-crate <実物の tree に載る名前>` で判定の名前の一覧へ 1 件足して流して、終了コード 1・全プロファイルで 1 件以上・judged_total の差が件数に一致・足した名前以外の `violation:` が無いことを確かめる（通常の検査と向きが逆で、検出が合格、0 件が失敗）。足す名前は本体の 1 箇所だけに置く。終了コード 0 全プロファイルで検出・1 どれかが 0 件または基準が違反・2 誤用・前提の不足・読めない入力・想定外の終了・件数の食い違い（2 は 1 に勝つ）。内側の出力はそのまま流し直す。ソースと `Cargo.lock` を変更しない・ネットワーク不要。python3・cargo・git 未導入は失敗（skip しない）。`make deny`（`scripts/deny.sh`）が呼ぶので、CI の `deny` ジョブの `make deny` の中で流れる（別ステップにしない） |
| `test-profile-dep-graph-positive-control` | `scripts/profile-dep-graph-positive-control.sh` の回帰テスト。本体と lib.sh を一時ディレクトリへ写し、偽の内側の検査（環境変数で振る舞いを切り替える）で検出・0 件・基準の違反・fail-closed の各経路を確かめ、実物のリポジトリでも本物の本体を通す。リポジトリのファイルを変更しない・ネットワーク不要。判定は終了コード + 出力の部分一致。python3・cargo・git 未導入は失敗（skip しない）。CI のジョブからは呼ばない（手元で流す） |
| `baseline-positive-control` | 比較基準の別 workspace（`baseline-wgpu/`）に `dep-graph-wgpu-check.sh`（`--allow-metadata-superset` 付き）と `public-api-check.sh` を `--manifest-path` で当てる陽性対照（TASK-12 のポインタ）。内側の終了コードと `record:` の行を読み、狭い定義・広い定義・公開 API のそれぞれが 1 件以上で合格（0 件の定義があれば終了コード 1・読めない入力・食い違い・2 や想定外は終了コード 2）。片方が失敗しても両方を流し、`record:` の行をログに残す。`make deny` には含めない（`make deny` は製品の workspace だけ）。内側の cargo は `--locked` で、ソースと `Cargo.lock` を変更しない。python3・cargo・git 未導入は失敗（skip しない）。CI の `deny` ジョブが `make deny` の後のステップで呼ぶ |
| `test-baseline-positive-control` | `scripts/baseline-positive-control.sh` の回帰テスト。本体を一時ディレクトリへ写し、偽の検査で定義ごとの判定と引数の渡し方を確かめ、一時 workspace の実物の出力でも通す。python3・cargo・git が要る（無ければ失敗）・ネットワーク不要・リポジトリのファイルを変更しない。判定は終了コード + 出力の部分一致。CI の `deny` ジョブが呼ぶ |
| `upper-layer-check` | `scripts/upper-layer-check.sh`。上の層（`crates/upper`）と測定の workspace に、unsafe の不在・チップの crate の不在・直接依存の許可リスト・測定の feature の 4 つの検査を当てる（TASK-15 のポインタ）。終了コードは 0 合格・1 違反・2 誤用や前提の不足。標準出力の `record:` の行に件数を残し、合格時は `ok:` の行。python3・cargo・git 未導入は失敗（skip しない）。ソースと `Cargo.lock` を変更しない・ネットワーク不要。`make deny` には含めない。CI の `deny` ジョブが呼ぶ |
| `test-upper-layer-check` | `scripts/upper-layer-check.sh` の回帰テスト。合成の入力・ツールが無いとき・偽のコマンドと、一時 workspace の実物の cargo metadata、実物のリポジトリの合格を確かめる。python3・cargo・git が要る（無ければ失敗）・ネットワーク不要・リポジトリのファイルを変更しない。CI の `deny` ジョブが呼ぶ |
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
