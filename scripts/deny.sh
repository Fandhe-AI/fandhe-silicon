#!/usr/bin/env bash
# 依存の監査: cargo deny check（advisories / bans / licenses / sources。設定は deny.toml）。
# CI の deny ジョブも本スクリプトを呼ぶ（チェック内容の定義はここにのみ置く）。
# 追加の守り: workspace の依存グラフ（全 feature・全 target）の path が crates/ の外を指さないことを検査する
# （基盤に足場を残さない向き。ポインタ: TASK-6・AP-4・AC-2）。段階 (1) の最終の判定（TASK-83）ではない。
# 別 workspace も監査する（ポインタ: TASK-27）。measure/ にはルートの deny.toml をそのまま当て、
# baseline-wgpu/ には wgpu 系の禁止だけを外した baseline-wgpu/deny.toml を --config で明示して当てる
# （明示しないと cargo-deny は上の階層のルートの deny.toml を拾って bans で落ちる）。あわせて
# baseline-wgpu/ の依存の承認の記録を deps-check.sh で検査する。別 workspace のビルドはしない。
# measure/ の deps-check と build / test は `make measure-verify` 側にある。
# 呼び出し元: `make deny`。cargo-deny・jq が無い場合は未検査で成功にせず失敗させる。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd cargo-deny
require_cmd jq

cd "${ROOT_DIR}"

log "cargo deny --locked check advisories bans licenses sources"
cargo deny --locked check advisories bans licenses sources

# path 依存・workspace メンバーが ALLOWED_ROOT の外を指していないことを検査する。
# 測定専用の別 workspace measure/（ポインタ: TASK-27）はルート Cargo.toml の exclude にあり、
# ここで見る cargo tree --workspace に現れないので、ALLOWED_ROOT は crates/ のままでよい
# （別 workspace の cargo-deny は末尾で別に行う）。
ALLOWED_ROOT="${ROOT_DIR}/crates/"

log "workspace path scope check (allowed: crates/)"
# --locked で lock 不一致を失敗させる。cargo tree の失敗は変数代入の終了コードで伝播させる。
tree_out="$(cargo tree --locked --workspace --all-features --target all \
  -e normal,build,dev --prefix none --no-dedupe)"

violations=0
while IFS= read -r line; do
  # 末尾の重複注記 " (*)" を除く。
  line="${line% (\*)}"
  # "name vX.Y.Z (path)" と "name vX.Y.Z (proc-macro) (path)" の末尾の括弧を path とみなす。
  # 括弧なし（registry）と "name vX.Y.Z (proc-macro)" 単独（registry の proc-macro）は対象外。
  if [[ ! "${line}" =~ ^[^\ ]+\ v[^\ ]+(\ \(proc-macro\))?\ \((.+)\)$ ]]; then
    continue
  fi
  src="${BASH_REMATCH[2]}"
  if [[ "${src}" == "proc-macro" ]]; then
    continue
  fi
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

# 別 workspace の依存の監査（ポインタ: TASK-27）。いずれか 1 つでも失敗したら非 0 で終える。
BASELINE_DENY="${ROOT_DIR}/baseline-wgpu/deny.toml"
if [ ! -f "${BASELINE_DENY}" ]; then
  echo "error: missing config: baseline-wgpu/deny.toml" >&2
  exit 1
fi

log "cargo deny (measure/) --locked check advisories bans licenses sources"
cargo deny --manifest-path "${ROOT_DIR}/measure/Cargo.toml" --locked \
  --config "${ROOT_DIR}/deny.toml" check advisories bans licenses sources

log "cargo deny (baseline-wgpu/) --locked check advisories bans licenses sources"
cargo deny --manifest-path "${ROOT_DIR}/baseline-wgpu/Cargo.toml" --locked \
  --config "${BASELINE_DENY}" check advisories bans licenses sources

log "deps-check (baseline-wgpu/)"
"${DIR}/deps-check.sh" "${ROOT_DIR}/baseline-wgpu/Cargo.toml"
