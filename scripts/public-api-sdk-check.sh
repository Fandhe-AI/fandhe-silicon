#!/usr/bin/env bash
# 製品の 4 crate の公開 API に、チップの SDK の crate（`objc2-metal`・`ash`・`cudarc`）の型・経路が出ていないことを、
# rustdoc の JSON から機械で数えて確かめる。あわせて、公開 API から解決された外部 crate の一覧を出す
# （spec TASK-13・BUILD-51 のポインタ。親 Issue #366 の 1/4）。
# 呼び出し元: 直接実行（make ターゲットと CI からは呼ばれていない）。検査の枠（rustdoc の JSON を作る段）は
# `scripts/public-api-check.sh --emit-json` を呼んで共有し、抽出の実体をここに持たない（`scripts/public-name-check.sh` と同じ形）。
#
# 使い方:
#   scripts/public-api-sdk-check.sh                         workspace の lib crate の JSON を作って検査する
#   scripts/public-api-sdk-check.sh --json <path> [<path>…]  渡した JSON だけを検査する（cargo を呼ばない。組は `-`、commit は `-`）
# 終了コード: 0 参照が 0 件 / 1 参照が 1 件以上（違反） / 2 誤用・前提の不足・JSON を読み切れない（成功扱いにしない）
#   2 になるもの: 引数の誤用、python3・cargo・git の不足、HEAD のコミットが取れない、JSON の形の不一致
#   （public-api-check.sh の V1〜V7 と同じ形の検査）、公開アイテムが 0 件、JSON を作る段の失敗、想定外の例外。
#   除外の印・除外の一覧・対象を外す口は持たない。
#
# 照合の規則:
#   crate 名の `-` を `_` にそろえ、`objc2_metal`・`ash`・`cudarc` と完全一致したときだけ当てる（接頭辞には広げない）。
#   数え方（V4〜V6b）は public-api-check.sh と同じ。親の鍵が resolved_path なら type、それ以外は other。どちらも違反。
#   外部 crate の一覧は、公開 API から実際に解決された crate 名（自 crate を除く）の重複なしの昇順。
#   `external_crates` の表そのものではない（公開 API から参照されない依存も入るため）。
#
# 標準出力: JSON ごとの `scanned:`、全部を検査し終えたとき（終了コード 0 と 1）だけ締めの `record:`、合格時の `ok:`。
#   `scanned: crate=<name> features=<組> format_version=<n> public_items=<n> sdk_type_refs=<n> sdk_other_refs=<n> external_crates=<a,b | ->`
#   `record: commit=<rev|-> documents=<n> public_items_total=<n> sdk_type_refs_total=<n> sdk_other_refs_total=<n>`
#   commit は JSON を作る経路では ROOT_DIR の HEAD（変更が在れば末尾に `-dirty`）、--json の経路では `-`。
# 標準エラー: `violation: crate=<name> features=<組> item=<name> kind=<type|other> references=<crate>`・`FAILED:`・`error:`。
#
# 検査しないもの: 共通の crate の `[dependencies]`、上記 3 つ以外の名前、件数の増減（数は出すだけ）、wasm32 の公開 API。
# ホストの絶対パス・環境変数の値は出力に出さない。ソースを変更しない。Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# 検査の対象にするチップの SDK の crate 名（`-` を `_` にそろえた形）。
CHIP_SDK = frozenset({"objc2_metal", "ash", "cudarc"})


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def clean(text):
    """出力に出す値から制御文字や想定外の文字を落とす。"""
    return re.sub(r"[^A-Za-z0-9_.:-]", "?", str(text))[:80]


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def is_chip_sdk(name):
    return name.replace("-", "_") in CHIP_SDK


def crate_name(doc, cid):
    ext = doc["external_crates"].get(str(cid))
    need(isinstance(ext, dict) and isinstance(ext.get("name"), str), "unresolved external crate id")
    return ext["name"]


def resolve(doc, ref):
    """参照 id を crate 名に解決する。内部参照なら None。"""
    key = str(ref)
    if key in doc["paths"]:
        entry = doc["paths"][key]
        need(isinstance(entry, dict), "malformed paths entry")
        cid = entry.get("crate_id")
    elif key in doc["index"]:
        cid = doc["index"][key].get("crate_id")
    else:
        raise Misuse("unresolved reference id")
    need(is_int(cid), "crate_id is not an integer")
    if cid == 0:
        return None
    return crate_name(doc, cid)


def inspect(doc):
    """JSON 1 本を検査して (crate 名, format_version, 公開アイテム数, type 数, other 数, 外部 crate の一覧, 違反の一覧) を返す。"""
    need(isinstance(doc, dict), "top level is not an object")
    for k in ("index", "paths", "external_crates"):
        need(isinstance(doc.get(k), dict), "missing or malformed '%s'" % k)
    need("root" in doc, "missing 'root'")
    need(doc.get("includes_private") is False, "includes_private is not false")
    fv = doc.get("format_version")
    need(is_int(fv), "format_version is not an integer")
    root = doc["index"].get(str(doc["root"]))
    root_ok = (
        isinstance(root, dict)
        and root.get("crate_id") == 0
        and not isinstance(root.get("crate_id"), bool)
        and isinstance(root.get("inner"), dict)
        and isinstance(root.get("name"), str)
    )
    need(root_ok, "malformed root item")

    items = doc["index"]
    for it in items.values():
        need(isinstance(it, dict) and is_int(it.get("crate_id")), "index item lacks integer crate_id")

    violations = []
    externals = set()
    n_type = 0
    n_other = 0
    public = 0

    def hit(item, kind, crate):
        nonlocal n_type, n_other
        if kind == "type":
            n_type += 1
        else:
            n_other += 1
        violations.append((clean(item), kind, clean(crate)))

    def seen(crate):
        if crate is not None:
            externals.add(crate)
        return crate

    def walk(node, parent, item, direct_use):
        if isinstance(node, dict):
            if "id" in node:
                rid = node["id"]
                if rid is None and direct_use:
                    pass
                else:
                    need(is_int(rid), "reference id is not an integer")
                    crate = seen(resolve(doc, rid))
                    if crate is not None and is_chip_sdk(crate):
                        hit(item, "type" if parent == "resolved_path" else "other", crate)
            for k, v in node.items():
                if k != "id":
                    walk(v, k, item, False)
        elif isinstance(node, list):
            for el in node:
                if el is None:
                    need(parent == "tuple", "null in a list outside 'tuple'")
                elif is_int(el):
                    crate = seen(resolve(doc, el))
                    if crate is not None and is_chip_sdk(crate):
                        hit(item, "other", crate)
                else:
                    walk(el, parent, item, False)

    for key, it in items.items():
        label = it.get("name") if isinstance(it.get("name"), str) else key
        if it["crate_id"] != 0:
            crate = seen(crate_name(doc, it["crate_id"]))
            if is_chip_sdk(crate):
                hit(label, "other", crate)
            continue
        if it.get("visibility") not in ("public", "default"):
            continue
        public += 1
        inner = it.get("inner")
        need(isinstance(inner, dict), "public item lacks 'inner'")
        for kind, body in inner.items():
            walk(body, kind, label, kind == "use")
    need(public >= 1, "no public items found")
    return root["name"], fv, public, n_type, n_other, sorted(externals), violations


def _git(root, args):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
    try:
        done = subprocess.run(
            ["git"] + args, cwd=root, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if done.returncode != 0:
        return None
    return done.stdout.decode("utf-8", "replace")


def source_commit(root):
    """記録に載せる HEAD のコミット（変更が在れば -dirty）。取れなければ Misuse。"""
    rev = _git(root, ["rev-parse", "--verify", "HEAD^{commit}"])
    sha = rev.strip() if rev is not None else ""
    need(re.fullmatch(r"[0-9a-f]{40}([0-9a-f]{24})?", sha) is not None, "cannot resolve the source commit")
    st = _git(root, ["status", "--porcelain", "--untracked-files=normal", "--ignore-submodules=all"])
    need(st is not None, "cannot resolve the source commit")
    return sha + ("-dirty" if st.strip() else "")


def emit_docs(root, tmp):
    """public-api-check.sh --emit-json で JSON を作り、(crate, 組, パス) の一覧を返す。"""
    script = os.path.join(root, "scripts", "public-api-check.sh")
    try:
        done = subprocess.run(
            ["bash", script, "--emit-json", tmp], cwd=root, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=None, check=False)
    except OSError:
        raise Misuse("cannot run public-api-check.sh")
    need(done.returncode == 0, "public-api-check.sh --emit-json failed")
    out = []
    for line in done.stdout.decode("utf-8", "replace").splitlines():
        m = re.fullmatch(r"emitted: crate=(\S+) features=(\S+) file=([A-Za-z0-9_.-]+\.json)", line)
        if m:
            out.append((m.group(1), m.group(2), os.path.join(tmp, m.group(3))))
    need(out, "no JSON was emitted")
    return out


def load_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, UnicodeDecodeError, ValueError):
        raise Misuse("cannot read JSON input")


def run_docs(inputs, total):
    for _crate, label, path in inputs:
        name, fv, public, n_type, n_other, ext, violations = inspect(load_json(path))
        print("scanned: crate=%s features=%s format_version=%d public_items=%d sdk_type_refs=%d sdk_other_refs=%d external_crates=%s"
              % (clean(name), label, fv, public, n_type, n_other, ",".join(clean(e) for e in ext) or "-"))
        for item, kind, crate in violations:
            print("violation: crate=%s features=%s item=%s kind=%s references=%s"
                  % (clean(name), label, item, kind, crate), file=sys.stderr)
        total["public"] += public
        total["type"] += n_type
        total["other"] += n_other


def main():
    root = sys.argv[1]
    parser = argparse.ArgumentParser(prog="public-api-sdk-check.sh")
    parser.add_argument("--json", nargs="+", metavar="PATH")
    args = parser.parse_args(sys.argv[2:])

    total = {"public": 0, "type": 0, "other": 0}
    if args.json is not None:
        commit = "-"
        inputs = [("-", "-", p) for p in args.json]
        run_docs(inputs, total)
        ndocs = len(inputs)
    else:
        commit = source_commit(root)
        with tempfile.TemporaryDirectory() as tmp:
            inputs = emit_docs(root, tmp)
            run_docs(inputs, total)
        ndocs = len(inputs)

    print("record: commit=%s documents=%d public_items_total=%d sdk_type_refs_total=%d sdk_other_refs_total=%d"
          % (commit, ndocs, total["public"], total["type"], total["other"]))
    refs = total["type"] + total["other"]
    if refs:
        print("FAILED: %d reference(s)" % refs, file=sys.stderr)
        return 1
    print("ok: %d public API document(s) checked, no chip SDK references" % ndocs)
    return 0


sys.setrecursionlimit(20000)
try:
    sys.exit(main())
except Misuse as e:
    print("error: %s" % e, file=sys.stderr)
    sys.exit(2)
except SystemExit:
    raise
except Exception as e:  # 想定外は違反(1)と読ませない
    print("error: unexpected %s" % type(e).__name__, file=sys.stderr)
    sys.exit(2)
PY
