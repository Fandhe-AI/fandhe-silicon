#!/usr/bin/env bash
# 4 crate の公開 API を rustdoc の JSON から取り出し、wgpu 系の crate への参照を数える
# （spec TASK-10・BUILD-47 のポインタ。親 Issue #363 の 2/6）。
# 呼び出し元: `make public-api-check`。CI のジョブからは呼ばれていない（手元の検査）。
#
# 使い方:
#   scripts/public-api-check.sh                      workspace の lib crate の JSON を作って検査する
#   scripts/public-api-check.sh --json <path> [...]  渡した rustdoc の JSON だけを検査する（cargo を呼ばない）
# 終了コード: 0 全部の組で参照が 0 件 / 1 参照が 1 件以上（違反）
#   / 2 誤用・前提の不足（未知の引数・python3 や cargo が無い・JSON を出せない toolchain・cargo の失敗・
#       JSON が読めない／要る鍵が無い／公開アイテムが 0 件・想定外の例外）。成功扱いにしない。
#
# 作る段: workspace の lib target を持つ package ごとに、次の 2 組で
#   cargo rustdoc --locked -p <pkg> <指定> --lib -- -Z unstable-options --output-format json
# を流す。組 production は contract だけ BUILD-47 の本番の feature 列（定数 CONTRACT_PRODUCTION_FEATURES）、
# ほかは既定の feature。組 all-features は --all-features。target は native だけ（wasm32 は対象外）。
# JSON の出力には `-Z unstable-options` を受ける toolchain が要る。toolchain はこのスクリプトが選ばない
# （rust-toolchain.toml の stable では終了コード 2 になる。rustup の仕組みで選ぶ）。
#
# 数える段: 自 crate の公開アイテム（visibility が public / default）の inner をたどり、整数の id を持つ
# object を拾って paths / external_crates で crate 名に解決する（paths に無い id は index の
# 内部アイテムだけ許し、どちらにも無い id・整数でない id・整数でない format_version は終了コード 2）。wgpu 系の crate（定数 WGPU_NAMES・
# 接頭辞 WGPU_PREFIXES）への参照を、型の参照（type）とそれ以外の参照（path: トレイト・境界・再 export）に分けて
# 数え、合計が 1 件以上なら違反。標準出力に組ごとに `scanned:` の行を出す。ホストのパスは出さない。
#
# 検査しないもの: wasm32、依存グラフ（make deny の役割）、陽性対照、wgpu 以外の名前の検査。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import json
import os
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# contract の本番の feature 列（spec BUILD-47 のポインタ。名前は crates/contract/Cargo.toml）。
CONTRACT_PRODUCTION_FEATURES = (
    "contract-core,cpu-isa,cpu-ops,native-blocking-wait,graphics,ext-interop,"
    "backend-metal,backend-vulkan,backend-cuda"
)
CONTRACT_PACKAGE = "fandhe-silicon-contract"
# wgpu 系の crate 名（- は _ にそろえて比べる）。
WGPU_NAMES = ("wgpu", "wgpu_core", "wgpu_hal", "wgpu_types", "naga")
WGPU_PREFIXES = ("wgpu", "naga")


class Misuse(Exception):
    """終了コード 2（誤用・前提の不足）にする失敗。"""


def is_wgpu(name):
    n = name.replace("-", "_")
    return n in WGPU_NAMES or n.startswith(WGPU_PREFIXES)


def collect_ids(node, out, in_resolved=False):
    """inner を再帰でたどり (id, 種別) を集める。resolved_path の値の id は type、ほかは path。"""
    if isinstance(node, dict):
        if "id" in node:
            nid = node["id"]
            if not isinstance(nid, int) or isinstance(nid, bool):
                raise Misuse("rustdoc json: non-integer reference id (unsupported format)")
            out.append((nid, "type" if in_resolved else "path"))
        for k, v in node.items():
            collect_ids(v, out, k == "resolved_path")
    elif isinstance(node, list):
        for v in node:
            collect_ids(v, out, False)


def scan(doc, features):
    """JSON 1 本を数える。(scanned 行, 違反の行の配列) を返す。"""
    if not isinstance(doc, dict):
        raise Misuse("rustdoc json: top level is not an object")
    for key in ("index", "paths", "external_crates"):
        if not isinstance(doc.get(key), dict):
            raise Misuse("rustdoc json: missing or malformed '%s'" % key)
    if "root" not in doc:
        raise Misuse("rustdoc json: missing 'root'")
    if doc.get("includes_private") is not False:
        raise Misuse("rustdoc json: includes_private must be false")
    index, paths, ext = doc["index"], doc["paths"], doc["external_crates"]
    root_item = index.get(str(doc["root"]))
    if not isinstance(root_item, dict) or not isinstance(root_item.get("inner"), dict):
        raise Misuse("rustdoc json: 'root' does not point to a valid item in 'index'")
    crate = "-"
    if isinstance(root_item.get("name"), str):
        crate = root_item["name"]
    fv = doc.get("format_version")
    if not isinstance(fv, int) or isinstance(fv, bool):
        raise Misuse("rustdoc json: missing or non-integer 'format_version'")

    public_items = 0
    counts = {"type": 0, "path": 0}
    violations = []
    for sid, item in index.items():
        if not isinstance(item, dict):
            raise Misuse("rustdoc json: malformed index entry")
        if "crate_id" in item:
            own = item["crate_id"] == 0
        else:
            p = paths.get(sid)
            own = not isinstance(p, dict) or p.get("crate_id") == 0
        if not own or item.get("visibility") not in ("public", "default"):
            continue
        if not isinstance(item.get("inner"), dict):
            raise Misuse("rustdoc json: public item without a valid 'inner'")
        public_items += 1
        found = []
        collect_ids(item["inner"], found)
        for rid, kind in found:
            p = paths.get(str(rid))
            if not isinstance(p, dict):
                # paths に無い id は、index にある内部アイテムへの参照だけ許す。
                # どちらにも無い参照は解決できないので、成功扱いにしない。
                if str(rid) in index:
                    continue
                raise Misuse("rustdoc json: unresolved reference id")
            cid = p.get("crate_id")
            if cid == 0:
                continue
            e = ext.get(str(cid))
            if not isinstance(e, dict) or not isinstance(e.get("name"), str):
                raise Misuse("rustdoc json: unresolved external crate id")
            if is_wgpu(e["name"]):
                counts[kind] += 1
                nm = item.get("name") if isinstance(item.get("name"), str) else "-"
                violations.append(
                    "violation: crate=%s features=%s item=%s kind=%s references=%s"
                    % (crate, features, nm, kind, e["name"])
                )
    if public_items == 0:
        raise Misuse("rustdoc json: no public items of the crate itself")
    line = (
        "scanned: crate=%s features=%s format_version=%s public_items=%d "
        "wgpu_type_refs=%d wgpu_path_refs=%d"
        % (crate, features, fv, public_items, counts["type"], counts["path"])
    )
    return line, violations


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, UnicodeDecodeError, ValueError):
        raise Misuse("cannot read rustdoc json as UTF-8 JSON")


def run(cmd, root, capture=False):
    try:
        return subprocess.run(
            cmd, cwd=root, check=False,
            stdout=subprocess.PIPE if capture else None,
        )
    except OSError:
        raise Misuse("cannot execute %s" % cmd[0])


def build_sets(root):
    """(JSON のパス, 組の名前) を、組ごとに cargo を流しながら順に返す。"""
    r = run(["rustdoc", "-Z", "unstable-options", "--version"], root, capture=True)
    if r.returncode != 0:
        raise Misuse(
            "this toolchain cannot emit rustdoc json: a toolchain accepting "
            "'-Z unstable-options' is required (select it via rustup, e.g. RUSTUP_TOOLCHAIN)"
        )
    r = run(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], root, capture=True)
    if r.returncode != 0:
        raise Misuse("cargo metadata failed")
    meta = json.loads(r.stdout.decode("utf-8"))
    members = set(meta["workspace_members"])
    targets = []
    for pkg in meta["packages"]:
        if pkg["id"] not in members:
            continue
        for t in pkg["targets"]:
            if "lib" in t["kind"]:
                targets.append((pkg["name"], t["name"].replace("-", "_")))
    if not targets:
        raise Misuse("no workspace package with a lib target")
    tdir = meta["target_directory"]
    for name, lib in sorted(targets):
        if name == CONTRACT_PACKAGE:
            prod = ["--no-default-features", "--features", CONTRACT_PRODUCTION_FEATURES]
        else:
            prod = []
        for label, flags in (("production", prod), ("all-features", ["--all-features"])):
            out = os.path.join(tdir, "doc", lib + ".json")
            try:
                os.remove(out)
            except FileNotFoundError:
                pass
            cmd = ["cargo", "rustdoc", "--locked", "-p", name] + flags + [
                "--lib", "--", "-Z", "unstable-options", "--output-format", "json"]
            if run(cmd, root).returncode != 0:
                raise Misuse("cargo rustdoc failed for %s (%s)" % (name, label))
            if not os.path.isfile(out):
                raise Misuse("rustdoc json was not produced for %s (%s)" % (name, label))
            yield out, label


def main():
    root = sys.argv[1]
    args = sys.argv[2:]
    all_violations = []
    sets = 0

    def handle(path, label):
        nonlocal sets
        line, vio = scan(load(path), label)
        print(line)
        all_violations.extend(vio)
        sets += 1

    if args and args[0] == "--json":
        files = args[1:]
        if not files:
            raise Misuse("usage: public-api-check.sh [--json <path> [...]]")
        for p in files:
            handle(p, "-")
    elif args:
        raise Misuse("usage: public-api-check.sh [--json <path> [...]]")
    else:
        for p, label in build_sets(root):
            handle(p, label)

    if all_violations:
        for v in all_violations:
            print(v, file=sys.stderr)
        print("FAILED: %d reference(s)" % len(all_violations), file=sys.stderr)
        return 1
    print("ok: no wgpu references in public api (%d sets)" % sets)
    return 0


try:
    code = main()
except Misuse as e:
    print("error: %s" % e, file=sys.stderr)
    code = 2
except Exception as e:  # 想定外の例外を違反（終了コード 1）と読ませない
    print("error: unexpected failure: %s" % type(e).__name__, file=sys.stderr)
    code = 2
sys.stdout.flush()
sys.exit(code)
PY
