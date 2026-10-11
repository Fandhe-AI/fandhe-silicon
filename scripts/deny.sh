#!/usr/bin/env bash
# 依存の監査: cargo deny check（advisories / bans / licenses / sources。設定は deny.toml）。
# CI の deny ジョブも本スクリプトを呼ぶ（チェック内容の定義はここにのみ置く）。
# 追加の守り: workspace の依存グラフ（全 feature・全 target）の path が crates/ の外を指さないことを検査する
# （基盤に足場を残さない向き。ポインタ: TASK-6・AP-4・AC-2）。段階 (1) の最終の判定（TASK-83）ではない。
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

# path 依存・workspace メンバーが ALLOWED_ROOT の外を指していないことを検査する。
# 測定専用 crate の置き場（ポインタ: TASK-15・TASK-1354・TASK-1356）が決まったら、この 1 行を見直す。
ALLOWED_ROOT="${ROOT_DIR}/crates/"

log "workspace path scope check (allowed: crates/)"
# --locked で lock 不一致を失敗させる。cargo tree の失敗は変数代入の終了コードで伝播させる。
tree_out="$(cargo tree --locked --workspace --all-features --target all \
  -e normal,build,dev --prefix none --no-dedupe)"

violations=0
while IFS= read -r line; do
  # "name vX.Y.Z (/path) ..." の形のみ対象。括弧なし（registry）は対象外。
  case "${line}" in
    *" ("*")"*) ;;
    *) continue ;;
  esac
  src="${line#* (}"
  src="${src%%)*}"
  # "(proc-macro)" "(*)" 等の注記は path ではない。
  case "${src}" in
    proc-macro | "*") continue ;;
  esac
  if [[ "${src}" == "${ALLOWED_ROOT}"* ]]; then
    continue
  fi
  name="${line%% *}"
  rel="${src}"
  if [[ "${src}" == "${ROOT_DIR}/"* ]]; then
    rel="${src#"${ROOT_DIR}"/}"
  fi
  echo "error: package outside crates/: ${name} (${rel})" >&2
  violations=$((violations + 1))
done <<<"${tree_out}"

if [ "${violations}" -ne 0 ]; then
  exit 1
fi
