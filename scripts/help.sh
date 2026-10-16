#!/usr/bin/env bash
# Make を介さない操作一覧。`make help` は Makefile の `## ` コメントから
# 機械的に生成されるのに対し、本ファイルは手動で保守する一覧であり、
# 両者を自動で同期する仕組みは無い。Makefile のターゲットを追加・改名・
# 削除したら同じ変更で本ファイルも更新すること。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

cat <<'EOF'
このリポジトリの操作と、Make を使わずに直接実行する方法:

  help            scripts/help.sh                この一覧を表示する
  doctor          scripts/doctor.sh              開発環境を診断する（読み取りのみ。何も導入・修復しない）
  setup           scripts/setup.sh               Git hooks 有効化と .env 雛形の配置（再実行しても安全）
  check           scripts/check.sh               editorconfig-checker + shellcheck（ソースは変更しない）
  verify          scripts/verify.sh              cargo fmt --check + clippy + test（ソースは変更しない。試験の枠の記録から集計の表を標準出力に出す）
  build-profiles  scripts/build-profiles.sh      feature の組ごとの build / clippy / test と wasm32 の check（ソースは変更しない）
  deny            scripts/deny.sh                cargo deny check + path が crates/ の外を指さない検査 + 依存グラフの wgpu 系の検査（python3・git も要る）+ 別 workspace（measure/・baseline-wgpu/・wasm/）の cargo deny と baseline-wgpu/・wasm/ の deps-check（jq が要る）
  measure-verify  scripts/measure-verify.sh      測定専用の別 workspace（measure/）の fmt --check / clippy（-D warnings）/ test / 依存の検査（cargo + jq。ソースは変更しない・ネットワーク不要・CI の deny ジョブも実行する）
  wasm-verify     scripts/wasm-verify.sh         wasm32 向けの別 workspace（wasm/）の fmt --check / clippy（ホストと wasm32、-D warnings）/ test / wasm32 の build / 依存の検査（cargo + jq。ソースは変更しない・初回は依存の取得が要る・CI の deny ジョブも実行する）
  wasm-size       scripts/wasm-size.sh           wasm-bindgen の glue を含めた成果物の量（gzip 後）を 8 組で測って閾値と比べる（bash + python3 + cargo + wasm-bindgen の CLI 0.2.128。欠けた組は not-run で終了コード 1・前提の不足は 2。CI は wasm-macos ジョブが呼び、deny ジョブは呼ばない）。--from-dir <dir> なら生成済みの出力だけを測る
  deps-check      scripts/deps-check.sh          依存の版指定が =x.y.z の完全固定で承認の記録と一致するかを検査する（cargo metadata + jq）
  test-deps-check scripts/test-deps-check.sh     deps-check 自体の回帰テスト（一時 workspace の fixture。cargo + jq + python3）
  inventory-trace-check scripts/inventory-trace-check.sh  一覧の鍵が項目ファイルに現れるかの検査（既定はファイル単位。行単位は直接実行で --item-rows。--list で対応の一覧を標準出力へ。要 docs/spec の取得。CI では実行しない）
  test-inventory-trace-check scripts/test-inventory-trace-check.sh  inventory-trace-check 自体の回帰テスト（合成 fixture。bash + python3。docs/spec の取得は不要）
  test-session-record-check scripts/test-session-record-check.sh  session-record-check 自体の回帰テスト（合成 fixture。bash + python3）。検査本体は make ターゲットなし: scripts/session-record-check.sh <path>
  test-perf-record-check scripts/test-perf-record-check.sh  perf-record-check 自体の回帰テスト（合成 fixture。bash + python3。cargo 不要）。検査本体は make ターゲットなし: scripts/perf-record-check.sh <path> または --session <回の記録> [<path>...]
  test-record-summary scripts/test-record-summary.sh  record-summary と verify.sh の記録の扱いの回帰テスト（合成の部品と偽の cargo。bash + python3）。集計の本体は make ターゲットなし: scripts/record-summary.sh <部品のディレクトリ>
  ci-frame-check  scripts/ci-frame-check.sh      ci.yml が CI の枠の事実（集約・実行場所・失敗の扱い）を保っているかの検査（bash + python3。--workflow <path> で入力を差し替え可）
  test-ci-frame-check scripts/test-ci-frame-check.sh  ci-frame-check 自体の回帰テスト（合成 fixture。bash + python3）
  public-api-check scripts/public-api-check.sh   4 crate の公開 API の wgpu 系の参照を rustdoc の JSON から数える検査。走査した公開アイテムの数を標準出力の scanned: と締めの record: に出す（bash + python3 + cargo。JSON を作る経路は git で HEAD のコミットを取り、取れなければ終了コード 2。--json <path>... で JSON を差し替え可。CI の deny ジョブも実行する）。回帰テストは make test-public-api-check
  test-public-api-sdk-check scripts/test-public-api-sdk-check.sh  公開 API のチップの SDK の検査（scripts/public-api-sdk-check.sh。make ターゲットなし・直接実行）の回帰テスト（合成 JSON・合成 metadata・偽のコマンドと、一時 workspace の実物の rustdoc の JSON と cargo metadata。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI はまだ呼ばない）
  dep-graph-wgpu-check scripts/dep-graph-wgpu-check.sh  製品の workspace の依存グラフの wgpu 系 crate を狭い定義と広い定義で数える検査。件数を標準出力の record: に出す（bash + python3 + cargo + git・ソースは変更しない・make deny も呼ぶ）
  test-dep-graph-wgpu-check scripts/test-dep-graph-wgpu-check.sh  dep-graph-wgpu-check 自体の回帰テスト（合成 JSON・合成 tree と、一時 workspace の実物の cargo metadata / cargo tree。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）
  baseline-positive-control scripts/baseline-positive-control.sh  比較基準の別 workspace に依存グラフの検査と公開 API の検査を当てる陽性対照（狭い定義・広い定義・公開 API のそれぞれが 1 件以上で合格・どれか 0 件なら失敗・読めない入力や誤用は終了コード 2。件数は record: の行。bash + python3 + cargo + git・make deny には含めない・CI の deny ジョブが make deny の後に呼ぶ）
  test-baseline-positive-control scripts/test-baseline-positive-control.sh  baseline-positive-control 自体の回帰テスト（偽の検査で定義ごとの判定を、一時 workspace の実物の出力でも確かめる。python3 + cargo + git・ネットワーク不要・CI の deny ジョブも実行する）
  upper-layer-check scripts/upper-layer-check.sh  上の層（crates/upper）と測定の workspace（measure/）に、unsafe の不在・チップの crate の不在・直接依存の許可リスト・測定のビルドで検証用の feature が有効でないことの 4 つの検査を当てる。合格は終了コード 0 と ok: の行・違反は 1・誤用や前提の不足は 2。件数は record: の行に残る（bash + python3 + cargo + git。ソースは変更しない・ネットワーク不要。make deny には含めない・CI の deny ジョブが呼ぶ）
  test-upper-layer-check scripts/test-upper-layer-check.sh  upper-layer-check の回帰テスト（合成の入力・ツールが無いとき・偽のコマンドと、一時 workspace の実物の cargo metadata・実物のリポジトリの合格。python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）
  public-name-check scripts/public-name-check.sh   製品の crate の公開アイテムの名前に語の一覧（public-name-check.conf）の語が出ていないことの検査。--source でソースの単語・呼び出しの形も数える（bash + python3 + cargo + git。JSON は public-api-check.sh --emit-json で作る。--json <path>... で差し替え可。CI の deny ジョブも実行する）
  test-public-name-check scripts/test-public-name-check.sh  public-name-check 自体の回帰テスト（ダミーの語の一覧の合成 JSON・合成ソースと、一時 workspace の実物の rustdoc の JSON。bash + python3 + cargo + git。CI の deny ジョブも実行する）
  test-wasm-size  scripts/test-wasm-size.sh      wasm-size 自体の回帰テスト（合成の入力と偽の cargo / wasm-bindgen だけ。bash + python3・CLI もネットワークも不要）
  test-public-api-diff scripts/test-public-api-diff.sh  public-api-diff 自体の回帰テスト（合成 JSON だけ。bash + python3）。本体は引数が要るため make ターゲットなし: scripts/public-api-diff.sh <基準の JSON> <新しい版の JSON>（終了コード 0 差分なし・1 差分あり・2 誤用または前提の不足）
  test-rerun-compare scripts/test-rerun-compare.sh  rerun-compare 自体の回帰テスト（合成の記録の部品だけ。bash + python3）。本体は引数が要るため make ターゲットなし: scripts/rerun-compare.sh <前の記録のディレクトリ> <新しい記録のディレクトリ>（終了コード 0 差分なし・1 差分あり・2 誤用・前提の不足・読めない記録）
  test-metal-session scripts/test-metal-session.sh  metal-session 自体の回帰テスト（偽のコマンドで macOS のコマンドを差し替える。bash + python3 + git）。本体は make ターゲットなし: scripts/metal-session.sh <probe|run|redact|scan>
  wasm-node-check scripts/wasm-node-check.sh     wasm32 の .wasm を glue 越しに Node で呼び参照と照合して行ごとに match / mismatch / not-run を出す土台（Node v24.13.0 と wasm-bindgen の CLI が要る実物の実行。CI の wasm-macos ジョブが呼ぶ。--record-dir と --record-combination を組で渡したときだけ行の結果を記録の部品へ書く。終了コード 0 全行一致・1 不一致か未実施・2 前提の不足か不正）
  test-wasm-node-check scripts/test-wasm-node-check.sh  wasm-node-check 自体の回帰テスト（偽の node・wasm-bindgen・cargo と合成の結果。bash + python3 + node。CI の deny ジョブも実行する）
  test-hooks      scripts/hooks/test-hooks.sh    フックスクリプト自体の回帰テスト（bash + git のみ）

Git hooks（lefthook.yml から呼ばれる。直接実行も可）:

  scripts/hooks/secret-scan.sh          staged 差分の簡易シークレット検知
  scripts/hooks/commit-msg-check.sh F   コミットメッセージ F の Conventional Commits 検証

`make <target>` は上記と同じコマンドを呼ぶだけの薄い入口であり、
処理の定義は scripts/ 側にのみ存在する。
EOF
