#!/usr/bin/env bash
# 比較基準の別 workspace（baseline-wgpu/）に、製品向けの 2 つの検査を当てる「陽性対照」。
# wgpu を実際に持つ workspace で検査が 1 件以上を検出できる＝検査が働いていることを確かめる
# （spec TASK-12 のポインタ。親 Issue #365 の 2/4）。
# 呼び出し元: `make baseline-positive-control` と CI の `deny` ジョブ（`make deny` の後のステップ）。
# `make deny` の中には入れない（`make deny` / `make public-api-check` は製品の workspace だけを見る契約を保つ）。
#
# 当てる検査（どちらも内側の検査が wgpu を検出すると終了コード 1）:
#   scripts/dep-graph-wgpu-check.sh --manifest-path baseline-wgpu/Cargo.toml --allow-metadata-superset
#   scripts/public-api-check.sh     --manifest-path baseline-wgpu/Cargo.toml
# 内側の cargo 呼び出しは `--locked` 付きで、比較基準の Cargo.lock を書き換えない。
# 内側の標準出力（`scanned:`・`record:`）と標準エラー（`violation:` など）はそのまま通し、件数をログに残す。
#
# 内側の終了コードの写し（検出が合格、0 件が失敗。通常の検査と向きが逆）:
#   1   1 件以上検出  → その検査は働いている（合格）
#   0   検出 0 件     → 失敗（`positive-control-failed:`）
#   2   誤用・前提の不足・読めない入力 → 失敗（検出とは数えない）
#   他  想定外        → 失敗
# 両方の検査を、片方が失敗しても止めずに流す（両方の `record:` を残すため）。
# 終了コード: 0 両方が検出 / 1 どちらかが 0 件 / 2 誤用・前提の不足・内側の想定外の終了
# 限界: 終了コード単位の粗い判定。`record:` を読んで定義ごと（狭い・広い・公開 API）の件数を判定するのは別の葉の範囲。
# 出力は英語。ホストの絶対パス・環境変数の値は出さない。ソースを変更しない。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd python3 || exit 2
require_cmd cargo || exit 2
require_cmd git || exit 2

cd "${ROOT_DIR}"
MANIFEST="${ROOT_DIR}/baseline-wgpu/Cargo.toml"

dep_rc=0
"${DIR}/dep-graph-wgpu-check.sh" --manifest-path "${MANIFEST}" --allow-metadata-superset || dep_rc=$?
api_rc=0
"${DIR}/public-api-check.sh" --manifest-path "${MANIFEST}" || api_rc=$?

final=0
# judge <検査名> <内側の終了コード>: 結果を 1 行出し、失敗なら final を更新する（2 を優先、次に 1）。
judge() {
  local name="$1" rc="$2"
  case "${rc}" in
    1)
      echo "positive-control: check=${name} inner_exit=${rc} result=detected"
      ;;
    0)
      echo "positive-control-failed: check=${name} inner_exit=${rc} result=no-detection" >&2
      if [ "${final}" -lt 1 ]; then final=1; fi
      ;;
    *)
      echo "positive-control-failed: check=${name} inner_exit=${rc} result=error" >&2
      final=2
      ;;
  esac
}
judge dep-graph-wgpu-check "${dep_rc}"
judge public-api-check "${api_rc}"

if [ "${final}" -eq 0 ]; then
  echo "ok: positive control detected wgpu in baseline-wgpu (2 checks)"
fi
exit "${final}"
