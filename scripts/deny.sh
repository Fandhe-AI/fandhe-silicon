#!/usr/bin/env bash
# 依存の監査: cargo deny check（advisories / bans / licenses / sources。設定は deny.toml）。
# CI の deny ジョブも本スクリプトを呼ぶ（チェック内容の定義はここにのみ置く）。
# 呼び出し元: `make deny`。cargo-deny が無い場合は未検査で成功にせず失敗させる。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd cargo-deny

cd "${ROOT_DIR}"

log "cargo deny --locked check advisories bans licenses sources"
cargo deny --locked check advisories bans licenses sources
