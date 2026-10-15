#!/usr/bin/env bash
# 同じ crate の 2 つの版（比較基準の版と新しい版）について、rustdoc の JSON から公開 API の差分
# （足された項目・消えた項目）を機械で出す（spec TASK-17 のポインタ。親 Issue #65 の 1/5）。
# 呼び出し元: 手元の直接実行と、回帰テスト `scripts/test-public-api-diff.sh`（`make test-public-api-diff`。CI の `deny` ジョブも呼ぶ）。
# make ターゲットは持たない: 比べる JSON のパスを引数で要るため。既定の入力を持たせると、
# 実物を検査したと誤解させる（scripts/perf-record-check.sh と同じ扱い）。JSON の作り方は別の葉（#68）が文書にする。
#
# 使い方:
#   scripts/public-api-diff.sh <比較基準の JSON> <新しい版の JSON>
# 終了コード（diff(1) の意味にそろえる）:
#   0 差分なし / 1 差分あり（足された項目か消えた項目が 1 件以上） / 2 誤用・前提の不足・JSON を読み切れない・想定外の例外
#   （2 を成功扱いにしない。読めない入力を「差分なし」と読ませない）
# 標準出力（終了コード 0 と 1 のとき）: 鍵で並べた決定的な行。
#   `removed: kind=<種別> path=<パス>` → `added: kind=<種別> path=<パス>` → 締めの
#   `summary: added=<n> removed=<n> unchanged=<n> format_version=<n>`。0 のときは続けて `ok: no public API differences`。
# 標準エラー: 終了コード 1 で `DIFFERENT: <n> difference(s)`、終了コード 2 で `error: <固定の文言>`。
#
# 突き合わせの鍵は (kind, path を "::" でつないだ文字列)。id は版をまたいで安定しないので比べない。
# 同じ path で kind が変わった項目は「消えた + 足された」として出す。
#
# 数える項目の規則（外れたら終了コード 2）:
#   D1 トップが object で index・paths・external_crates が object、root が在り、includes_private が false、format_version が整数
#   D2 index[root] が crate_id 0 で name が文字列のアイテム
#   D3 paths の全項目が object で、整数の crate_id・文字列の kind・文字列の並びの path を持つ
#   D4 2 本の format_version が等しい（食い違う JSON の形は比べない）
#   D5 2 本の root の crate 名が等しい（1 つの crate の版違いを比べる道具）
#   D6 公開項目が 1 件以上（0 件は取り出しの失敗と区別できない）
#   公開項目 = paths の crate_id が 0 の項目のうち、id が index に在り、visibility が public / default のもの。
#   外部 crate（crate_id が 0 以外）・index に無い id・非公開の項目は数えない。variant は paths に載るので数える。
#
# 比べないもの: シグネチャ・型の変化（added / removed だけ）、impl の下にだけ在るメソッド・フィールド・
#   関連定数（paths に載らない）、再公開（use）の別名、doc の文言、wasm32 など target 違いの API。
# path・kind は入力の値なので、[A-Za-z0-9_.:-] 以外を ? にして出す。引数のファイルパス・ホストの絶対パス・環境変数の値は出さない。
# 入力は読むだけで、ファイルを作らず、ネットワークを使わない。Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "$@" <<'PY'
import argparse
import json
import re
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

USAGE = "usage: public-api-diff.sh <baseline.json> <new.json>"


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


class Parser(argparse.ArgumentParser):
    def error(self, message):
        # argparse の既定は入力の語を含む文言を出すので、固定の文言に置き換える。
        print(USAGE, file=sys.stderr)
        sys.exit(2)


def clean(text):
    """出力に出す名前から制御文字や想定外の文字を落とす。別の path が潰れ合わないよう長めに切る。"""
    return re.sub(r"[^A-Za-z0-9_.:-]", "?", str(text))[:256]


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, UnicodeDecodeError, ValueError):
        raise Misuse("cannot read input as JSON")


def surface(doc):
    """検証して (root の crate 名, format_version, {(kind, path)}) を返す。D1〜D3。"""
    need(isinstance(doc, dict), "top level is not an object")
    for k in ("index", "paths", "external_crates"):
        need(isinstance(doc.get(k), dict), "missing or malformed '%s'" % k)
    need("root" in doc, "missing 'root'")
    need(doc.get("includes_private") is False, "includes_private is not false")
    fv = doc.get("format_version")
    need(is_int(fv), "format_version is not an integer")
    root = doc["index"].get(str(doc["root"]))
    need(
        isinstance(root, dict) and root.get("crate_id") == 0 and not isinstance(root.get("crate_id"), bool)
        and isinstance(root.get("name"), str),
        "malformed root item",
    )
    items = set()
    for key, ent in doc["paths"].items():
        need(isinstance(ent, dict), "malformed paths entry")
        need(is_int(ent.get("crate_id")), "malformed paths entry")
        need(isinstance(ent.get("kind"), str), "malformed paths entry")
        p = ent.get("path")
        need(isinstance(p, list) and all(isinstance(s, str) for s in p), "malformed paths entry")
        if ent["crate_id"] != 0:
            continue
        it = doc["index"].get(key)
        if not isinstance(it, dict):
            continue
        if it.get("visibility") not in ("public", "default"):
            continue
        items.add((ent["kind"], "::".join(p)))
    return root["name"], fv, items


def main():
    ap = Parser(prog="public-api-diff.sh", add_help=True)
    ap.add_argument("paths", nargs="*")
    try:
        args = ap.parse_args(sys.argv[1:])
    except SystemExit as e:
        sys.exit(0 if e.code in (0, None) else 2)
    if len(args.paths) != 2:
        ap.error("")
    name_a, fv_a, a = surface(load(args.paths[0]))
    name_b, fv_b, b = surface(load(args.paths[1]))
    need(fv_a == fv_b, "format_version differs between inputs")
    need(name_a == name_b, "root crate name differs between inputs")
    need(a and b, "no public items found")
    removed = sorted(a - b)
    added = sorted(b - a)
    for kind, path in removed:
        print("removed: kind=%s path=%s" % (clean(kind), clean(path)))
    for kind, path in added:
        print("added: kind=%s path=%s" % (clean(kind), clean(path)))
    print("summary: added=%d removed=%d unchanged=%d format_version=%d" % (len(added), len(removed), len(a & b), fv_a))
    if removed or added:
        print("DIFFERENT: %d difference(s)" % (len(removed) + len(added)), file=sys.stderr)
        return 1
    print("ok: no public API differences")
    return 0


try:
    code = main()
except Misuse as e:
    print("error: %s" % e, file=sys.stderr)
    code = 2
except SystemExit:
    raise
except BaseException as e:
    # 想定外の例外（深すぎる入れ子の RecursionError など）を違反の 1 と読ませない。
    print("error: unexpected %s" % type(e).__name__, file=sys.stderr)
    code = 2
sys.exit(code)
PY
