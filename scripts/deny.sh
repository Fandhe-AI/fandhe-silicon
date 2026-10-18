#!/usr/bin/env bash
# 依存の監査: cargo deny check（advisories / bans / licenses / sources。設定は deny.toml）。
# CI の deny ジョブも本スクリプトを呼ぶ（チェック内容の定義はここにのみ置く）。
# 追加の守り: workspace の依存グラフ（全 feature・全 target）の path が crates/ の外を指さないことを検査する
# （基盤に足場を残さない向き。ポインタ: TASK-6・AP-4・AC-2）。段階 (1) の最終の判定（TASK-83）ではない。
# 製品の workspace の依存グラフの wgpu 系 crate の検査（ポインタ: TASK-11）も `dep-graph-wgpu-check.sh` で呼ぶ
# （終了コード 1・2 はそのまま `make deny` の失敗にする。python3・git が要る）。
# 契約の crate のプロファイルごとの依存グラフの検査（ポインタ: TASK-228・BUILD-1・BUILD-2）と、その陽性対照も呼ぶ
# （`profile-dep-graph-check.sh`・`profile-dep-graph-positive-control.sh`。終了コード 1・2 はそのまま `make deny` の失敗にする）。
# 別 workspace も監査する（ポインタ: TASK-27）。measure/ にはルートの deny.toml をそのまま当て、
# baseline-wgpu/ には wgpu 系の禁止だけを外した baseline-wgpu/deny.toml を --config で明示して当てる
# （明示しないと cargo-deny は上の階層のルートの deny.toml を拾って bans で落ちる）。あわせて
# baseline-wgpu/ の依存の承認の記録を deps-check.sh で検査する。別 workspace のビルドはしない。
# measure/ の deps-check と build / test は `make measure-verify` 側にある。
# wasm32 向けの別 workspace wasm/（ポインタ: TASK-22）にもルートの deny.toml をそのまま当て（wgpu 系・naga の禁止は残る）、
# deps-check.sh を --allow-path-root crates で呼ぶ（製品の crate への path だけを免除する）。
# 製品の crate が wasm/ を path で参照すると、上の path の検査（crates/ の外）で落ちる。ビルドと test は `make wasm-verify` 側にある。
# 呼び出し元: `make deny`。cargo-deny・jq・python3・git が無い場合は未検査で成功にせず失敗させる。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd cargo-deny
require_cmd jq
require_cmd python3
require_cmd git

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

# 製品の workspace の依存グラフに wgpu 系 crate が無いことの検査（ポインタ: TASK-11）。
# cargo-deny が依存を取得した後なので、中の cargo metadata / cargo tree（--locked）はネットワークを要しない。
# 違反（1）も誤用・前提の不足（2）も握りつぶさず、set -e でそのまま失敗にする。
log "dep-graph-wgpu-check (product workspace)"
"${DIR}/dep-graph-wgpu-check.sh"

# 契約の crate のプロファイルごとの依存グラフの検査（ポインタ: TASK-228・BUILD-1・BUILD-2）。
# 順は「検査 → 陽性対照」に固定する。検査の終了コード 1 は違反そのものと読めるが、陽性対照の 1 は
# 「基準の実行が違反した」場合も含むので、先に流すと失敗の意味がぼやける。
# 陽性対照は中で基準の検査をもう一度流すため、検査は合計 3 回流れる（cargo tree だけで数秒）。
# 陽性対照だけを呼ぶ形にまとめず、失敗の意味の分かりやすさを優先する。
# cargo-deny が依存を取得した後なので、中の cargo tree（--locked）はネットワークを要しない。
# 違反（1）も誤用・前提の不足（2）も握りつぶさず、set -e でそのまま失敗にする。
log "profile-dep-graph-check (contract profiles)"
"${DIR}/profile-dep-graph-check.sh"

log "profile-dep-graph-positive-control"
"${DIR}/profile-dep-graph-positive-control.sh"

# 別 workspace の依存の監査（ポインタ: TASK-27）。いずれか 1 つでも失敗したら非 0 で終える。
BASELINE_DENY="${ROOT_DIR}/baseline-wgpu/deny.toml"
if [ ! -f "${BASELINE_DENY}" ]; then
  echo "error: missing config: baseline-wgpu/deny.toml" >&2
  exit 1
fi

# 注意: --config は cargo-deny 0.20.2 では check の前に置く上位オプション。
# check の後ろへ移すと引数解析で拒否される（実測済み）ので並べ替えない。
log "cargo deny (measure/) --locked check advisories bans licenses sources"
cargo deny --manifest-path "${ROOT_DIR}/measure/Cargo.toml" --locked \
  --config "${ROOT_DIR}/deny.toml" check advisories bans licenses sources

# 注意: --config は cargo-deny 0.20.2 では check の前に置く上位オプション。
# check の後ろへ移すと引数解析で拒否される（実測済み）ので並べ替えない。
log "cargo deny (baseline-wgpu/) --locked check advisories bans licenses sources"
cargo deny --manifest-path "${ROOT_DIR}/baseline-wgpu/Cargo.toml" --locked \
  --config "${BASELINE_DENY}" check advisories bans licenses sources

log "deps-check (baseline-wgpu/)"
"${DIR}/deps-check.sh" "${ROOT_DIR}/baseline-wgpu/Cargo.toml"

# wasm/ はルートの deny.toml をそのまま当てる（wgpu 系・naga の禁止を外さない）。
# 注意: --config は check の前に置く（上と同じ理由）。
log "cargo deny (wasm/) --locked check advisories bans licenses sources"
cargo deny --manifest-path "${ROOT_DIR}/wasm/Cargo.toml" --locked \
  --config "${ROOT_DIR}/deny.toml" check advisories bans licenses sources

# cargo-deny が依存を取得した後に実行する（deps-check は --offline で cargo metadata を読む）。
log "deps-check (wasm/)"
"${DIR}/deps-check.sh" --allow-path-root "${ROOT_DIR}/crates" "${ROOT_DIR}/wasm/Cargo.toml"
