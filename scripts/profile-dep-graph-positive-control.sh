#!/usr/bin/env bash
# プロファイルごとの依存グラフの検査（profile-dep-graph-check.sh）が空振りしていないことを確かめる「陽性対照」。
# 判定の名前の一覧に実物の依存グラフへ載っている名前を 1 件足して流し、検査が実際に違反を出す（終了コード 1・
# 全プロファイルで 1 件以上）ことを示す（spec TASK-228・BUILD-1・BUILD-2 のポインタ。親 Issue #459 の 3/6）。
# 呼び出し元: `make profile-dep-graph-positive-control`。直接実行も可。`make deny`・CI からは呼ばない。
# 回帰テストは `make test-profile-dep-graph-positive-control`（`scripts/test-profile-dep-graph-positive-control.sh`）。
#
# 流す順:
#   1. 基準  : 内側の検査を引数なしで流す。終了コード 0 であること（0 件の状態が基準）
#   2. 足した組: 内側の検査を `--extra-judged-crate <名前>` 付きで流す。足す名前は下の EXTRA_JUDGED_CRATE の 1 箇所だけ
# 判定（通常の検査と向きが逆で、検出が合格、0 件が失敗）:
#   足した組が終了コード 1、かつ基準に出たすべてのプロファイルで `count: profile=<p> crate=<名前> n=<k>` が k >= 1、
#   かつ `violation:` の行がすべて足した名前のもの、かつ各プロファイルの judged_total が「基準の値 + k」に一致すること。
#   プロファイルごとに 1 行、検出は標準出力に `positive-control: profile=<p> crate=<名前> n=<k> result=detected`、
#   0 件は標準エラーに `positive-control-failed: profile=<p> crate=<名前> n=0 result=no-detection`。
# 読めない入力は成功にも 0 件にも数えず終了コード 2（fail-closed）:
#   内側の終了コードが想定外 / `record:`・`count:` の行の欠落・重複・非負の整数でない値 / プロファイルの集合の食い違い /
#   足した名前以外の `violation:` / judged_total の差の不一致 / 終了コードと件数の食い違い
# 内側の標準出力・標準エラーは一時ファイルに取り、そのまま流し直す（`record:` を CI のログに残す）。
# 終了コード: 0 全プロファイルで検出 / 1 どれかが 0 件、または基準が違反を出した / 2 誤用・前提の不足・読めない入力（2 は 1 に勝つ）
# 出力は英語。ホストの絶対パス・環境変数の値は出さない。ソースと Cargo.lock を変更しない。Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd python3 || exit 2
require_cmd cargo || exit 2
require_cmd git || exit 2

# 足す名前。契約の crate（fandhe-silicon-contract）が無条件の path 依存で持つので、全プロファイルの tree に載る。
EXTRA_JUDGED_CRATE="fandhe-silicon-core"

cd "${ROOT_DIR}"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp:?}"' EXIT

base_rc=0
"${DIR}/profile-dep-graph-check.sh" >"${tmp}/base.out" 2>"${tmp}/base.err" || base_rc=$?
cat "${tmp}/base.out"
cat "${tmp}/base.err" >&2
extra_rc=0
"${DIR}/profile-dep-graph-check.sh" --extra-judged-crate "${EXTRA_JUDGED_CRATE}" >"${tmp}/extra.out" 2>"${tmp}/extra.err" || extra_rc=$?
cat "${tmp}/extra.out"
cat "${tmp}/extra.err" >&2

final=0
python3 -I - "${EXTRA_JUDGED_CRATE}" "${base_rc}" "${tmp}/base.out" "${extra_rc}" "${tmp}/extra.out" "${tmp}/extra.err" <<'PY' || final=$?
import re
import sys

name, base_rc, base_out, extra_rc, extra_out, extra_err = sys.argv[1:7]
base_rc, extra_rc = int(base_rc), int(extra_rc)
UINT = re.compile(r"[0-9]+")


def fail2(result):
    print("positive-control-failed: crate=%s result=%s" % (name, result), file=sys.stderr)
    sys.exit(2)


def read_lines(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return [ln.rstrip("\n") for ln in f]
    except OSError:
        fail2("unreadable-output")


def fields(line, prefix):
    out = {}
    for tok in line[len(prefix):].split():
        k, sep, v = tok.partition("=")
        if sep:
            out[k] = None if k in out else v
    return out


def judged_totals(path):
    """record: の行から {プロファイル: judged_total} を返す。欠落・重複・非負の整数でない値は fail-closed。"""
    totals = {}
    for ln in read_lines(path):
        if not ln.startswith("record: "):
            continue
        f = fields(ln, "record: ")
        p, t = f.get("profile"), f.get("judged_total")
        if p is None or t is None or not UINT.fullmatch(t) or p in totals:
            fail2("unreadable-record")
        totals[p] = int(t)
    if not totals:
        fail2("unreadable-record")
    return totals


def name_counts(path):
    """count: の行のうち足した名前の {プロファイル: n} を返す。"""
    counts = {}
    for ln in read_lines(path):
        if not ln.startswith("count: "):
            continue
        f = fields(ln, "count: ")
        if f.get("crate") != name:
            continue
        p, n = f.get("profile"), f.get("n")
        if p is None or n is None or not UINT.fullmatch(n) or p in counts:
            fail2("unreadable-count")
        counts[p] = int(n)
    return counts


if base_rc == 1:
    print("positive-control-failed: crate=%s result=baseline-violation" % name, file=sys.stderr)
    sys.exit(1)
if base_rc != 0 or extra_rc not in (0, 1):
    fail2("inner-error")

base = judged_totals(base_out)
extra = judged_totals(extra_out)
counts = name_counts(extra_out)
if set(base) != set(extra) or set(counts) != set(base):
    fail2("profile-mismatch")
for p in base:
    if extra[p] != base[p] + counts[p]:
        fail2("inconsistent")
# violation: の行は足した名前のものだけ（ほかの名前が出たら基準が壊れている）
for ln in read_lines(extra_err):
    if ln.startswith("violation: "):
        if fields(ln, "violation: ").get("crate") != name:
            fail2("unexpected-violation")
total = sum(counts.values())
if extra_rc == 0 and total > 0:
    fail2("inconsistent")
if extra_rc == 1 and total == 0:
    fail2("inconsistent")

bad = False
for p in base:
    n = counts[p]
    if n >= 1:
        print("positive-control: profile=%s crate=%s n=%d result=detected" % (p, name, n))
    else:
        bad = True
        print("positive-control-failed: profile=%s crate=%s n=%d result=no-detection" % (p, name, n), file=sys.stderr)
sys.exit(1 if bad else 0)
PY

if [ "${final}" -eq 0 ]; then
  echo "ok: positive control detected the added crate in every profile (${EXTRA_JUDGED_CRATE})"
fi
exit "${final}"
