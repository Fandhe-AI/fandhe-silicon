#!/usr/bin/env bash
# 比較基準の別 workspace（baseline-wgpu/）に、製品向けの 2 つの検査を当てる「陽性対照」。
# wgpu を実際に持つ workspace で検査が 1 件以上を検出できる＝検査が働いていることを確かめる
# （spec TASK-12 のポインタ。親 Issue #365 の 2/4・3/4）。
# 呼び出し元: `make baseline-positive-control` と CI の `deny` ジョブ（`make deny` の後のステップ）。
# `make deny` の中には入れない（`make deny` / `make public-api-check` は製品の workspace だけを見る契約を保つ）。
# 回帰テストは `make test-baseline-positive-control`（`scripts/test-baseline-positive-control.sh`）。
#
# 当てる検査（どちらも内側の検査が wgpu を検出すると終了コード 1）:
#   scripts/dep-graph-wgpu-check.sh --manifest-path baseline-wgpu/Cargo.toml --allow-metadata-superset
#   scripts/public-api-check.sh     --manifest-path baseline-wgpu/Cargo.toml
# 内側の cargo 呼び出しは `--locked` 付きで、比較基準の Cargo.lock を書き換えない。
#
# 判定（検査ごとに内側の終了コードと `record:` の行を読む。通常の検査と向きが逆で、検出が合格、0 件が失敗）:
#   dep-graph-wgpu-check  narrow（狭い定義 = narrow_total）と wide（広い定義 = wide_total）をそれぞれ 1 件以上
#   public-api-check      public-api（wgpu_type_refs_total + wgpu_other_refs_total。内側が違反とみなす向きに合わせる）が 1 件以上
#   定義ごとに 1 行を出す。1 件以上は標準出力に `positive-control: check=… definition=… count=<n> result=detected`、
#   0 件は標準エラーに `positive-control-failed: … result=no-detection`。件数は出すだけで特定の値とは比べない（判定は 1 件以上だけ）。
#   読めない入力は成功にも 0 件にも数えず終了コード 2（fail-closed）:
#     内側の終了コードが 0・1 以外 / `record:` の行が 0 行か 2 行以上 / 要る鍵が欠ける・重複する・非負の整数でない /
#     内側が 1 なのに判定に使う件数がすべて 0、内側が 0 なのにどれかが 1 以上（内側の終了コードと件数の食い違い）
# 両方の検査を、片方が失敗しても止めずに流す。内側の標準出力は一時ファイルに取り、そのまま標準出力へ流し直す
# （`scanned:`・`record:` を CI のログに残す）。標準エラー（`violation:` など）もそのまま通す。
# 終了コード: 0 3 つの定義すべてが検出 / 1 どれかの定義が 0 件 / 2 誤用・前提の不足・読めない入力・想定外の終了（2 は 1 に勝つ）
# 限界: 件数の厳密な一致は見ない（TASK-255 のポインタ）。包み自身の `record:` の行は出さない。
# 出力は英語。ホストの絶対パス・環境変数の値は出さない。ソースを変更しない。Python 3.8 以上の標準ライブラリだけ。
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

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp:?}"' EXIT

dep_rc=0
"${DIR}/dep-graph-wgpu-check.sh" --manifest-path "${MANIFEST}" --allow-metadata-superset >"${tmp}/dep.out" || dep_rc=$?
cat "${tmp}/dep.out"
api_rc=0
"${DIR}/public-api-check.sh" --manifest-path "${MANIFEST}" >"${tmp}/api.out" || api_rc=$?
cat "${tmp}/api.out"

final=0
# judge <検査名> <内側の終了コード> <取り込んだ標準出力>: 定義ごとに判定し、final を更新する（2 を優先、次に 1）。
judge() {
  local name="$1" rc="$2" file="$3" jrc=0
  case "${rc}" in
    0 | 1)
      python3 -I - "${name}" "${rc}" "${file}" <<'PY' || jrc=$?
import re
import sys

name, inner_rc, path = sys.argv[1], int(sys.argv[2]), sys.argv[3]
KEYS = {
    "dep-graph-wgpu-check": [("narrow", ("narrow_total",)), ("wide", ("wide_total",))],
    "public-api-check": [("public-api", ("wgpu_type_refs_total", "wgpu_other_refs_total"))],
}[name]


def fail2(result):
    print("positive-control-failed: check=%s inner_exit=%d result=%s" % (name, inner_rc, result), file=sys.stderr)
    sys.exit(2)


try:
    with open(path, encoding="utf-8", errors="replace") as f:
        lines = [ln.rstrip("\n") for ln in f if ln.startswith("record: ")]
except OSError:
    fail2("unreadable-record")
if len(lines) != 1:
    fail2("unreadable-record")
fields = {}
for tok in lines[0][len("record: "):].split():
    k, sep, v = tok.partition("=")
    if not sep:
        continue
    # 同じ鍵が 2 回出たら読めない入力として扱う
    fields[k] = None if k in fields else v
counts = []
for definition, keys in KEYS:
    total = 0
    for k in keys:
        v = fields.get(k)
        if v is None or not re.fullmatch(r"[0-9]+", v):
            fail2("unreadable-record")
        total += int(v)
    counts.append((definition, total))
if inner_rc == 1 and all(n == 0 for _, n in counts):
    fail2("inconsistent")
if inner_rc == 0 and any(n > 0 for _, n in counts):
    fail2("inconsistent")
bad = False
for definition, n in counts:
    if n >= 1:
        print("positive-control: check=%s definition=%s count=%d result=detected" % (name, definition, n))
    else:
        bad = True
        print("positive-control-failed: check=%s definition=%s count=%d result=no-detection" % (name, definition, n),
              file=sys.stderr)
sys.exit(1 if bad else 0)
PY
      ;;
    *)
      echo "positive-control-failed: check=${name} inner_exit=${rc} result=error" >&2
      jrc=2
      ;;
  esac
  if [ "${jrc}" -ge 2 ]; then
    final=2
  elif [ "${jrc}" -eq 1 ] && [ "${final}" -lt 1 ]; then
    final=1
  fi
}
judge dep-graph-wgpu-check "${dep_rc}" "${tmp}/dep.out"
judge public-api-check "${api_rc}" "${tmp}/api.out"

if [ "${final}" -eq 0 ]; then
  echo "ok: positive control detected wgpu in baseline-wgpu (narrow, wide, public-api)"
fi
exit "${final}"
