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
  deny            scripts/deny.sh                cargo deny check + path が crates/ の外を指さない検査
  deps-check      scripts/deps-check.sh          依存の版指定が =x.y.z の完全固定で承認の記録と一致するかを検査する（cargo metadata + jq）
  test-deps-check scripts/test-deps-check.sh     deps-check 自体の回帰テスト（一時 workspace の fixture。cargo + jq + python3）
  inventory-trace-check scripts/inventory-trace-check.sh  一覧の鍵が項目ファイルに現れるかの検査（既定はファイル単位。行単位は直接実行で --item-rows。--list で対応の一覧を標準出力へ。要 docs/spec の取得。CI では実行しない）
  test-inventory-trace-check scripts/test-inventory-trace-check.sh  inventory-trace-check 自体の回帰テスト（合成 fixture。bash + python3。docs/spec の取得は不要）
  test-session-record-check scripts/test-session-record-check.sh  session-record-check 自体の回帰テスト（合成 fixture。bash + python3）。検査本体は make ターゲットなし: scripts/session-record-check.sh <path>
  test-record-summary scripts/test-record-summary.sh  record-summary と verify.sh の記録の扱いの回帰テスト（合成の部品と偽の cargo。bash + python3）。集計の本体は make ターゲットなし: scripts/record-summary.sh <部品のディレクトリ>
  ci-frame-check  scripts/ci-frame-check.sh      ci.yml が CI の枠の事実（集約・実行場所・失敗の扱い）を保っているかの検査（bash + python3。--workflow <path> で入力を差し替え可）
  test-ci-frame-check scripts/test-ci-frame-check.sh  ci-frame-check 自体の回帰テスト（合成 fixture。bash + python3）
  test-metal-session scripts/test-metal-session.sh  metal-session 自体の回帰テスト（偽のコマンドで macOS のコマンドを差し替える。bash + python3 + git）。本体は make ターゲットなし: scripts/metal-session.sh <probe|run|redact|scan>
  test-hooks      scripts/hooks/test-hooks.sh    フックスクリプト自体の回帰テスト（bash + git のみ）

Git hooks（lefthook.yml から呼ばれる。直接実行も可）:

  scripts/hooks/secret-scan.sh          staged 差分の簡易シークレット検知
  scripts/hooks/commit-msg-check.sh F   コミットメッセージ F の Conventional Commits 検証

`make <target>` は上記と同じコマンドを呼ぶだけの薄い入口であり、
処理の定義は scripts/ 側にのみ存在する。
EOF
