#!/usr/bin/env bash
# 製品の 4 crate の公開 API に、チップの SDK の crate（`objc2-metal`・`ash`・`cudarc`）の型・経路が出ていないことを、
# rustdoc の JSON から機械で数えて確かめる。あわせて、公開 API から解決された外部 crate の一覧を出す。
# また、共通の crate（`crates/core`）が宣言する依存にチップの SDK の crate が無いことを、cargo metadata から確かめる
# （spec TASK-13・BUILD-51・BUILD-54・U-30 の案 A のポインタ。親 Issue #366 の 1/4・2/4・3/4）。
# 呼び出し元: 直接実行（検査本体の make ターゲットと CI からは呼ばれていない）。回帰テストは `scripts/test-public-api-sdk-check.sh`（`make test-public-api-sdk-check`）。検査の枠（rustdoc の JSON を作る段）は
# `scripts/public-api-check.sh --emit-json` を呼んで共有し、抽出の実体をここに持たない（`scripts/public-name-check.sh` と同じ形）。
#
# 使い方:
#   scripts/public-api-sdk-check.sh                         rustdoc の JSON を作り、cargo metadata も流して両方を検査する
#   scripts/public-api-sdk-check.sh --json <path> [<path>…]  渡した JSON だけを検査する（依存側は実行しない。cargo・git を呼ばない）
#   scripts/public-api-sdk-check.sh --metadata-json <path>   渡した metadata だけを検査する（rustdoc 側は実行しない。cargo・git を呼ばない）
#   両方を渡すと、渡した 2 つだけを検査する（cargo・git を呼ばない）。`--metadata-json` は 1 つだけ。
#   実行しなかった側は `record:` の行で `-` と出し、部分の実行が全部の合格に読めないようにする。
#   組と commit は、cargo・git を呼ばない経路では `-`。
# 終了コード: 0 実行した側の参照・依存が 0 件 / 1 どちらかで 1 件以上（違反） / 2 誤用・前提の不足・入力を読み切れない（成功扱いにしない）
#   2 になるもの: 引数の誤用、python3・cargo・git の不足、HEAD のコミットが取れない、JSON の形の不一致
#   （public-api-check.sh の V1〜V7 と同じ形の検査）、公開アイテムが 0 件、JSON を作る段の失敗、
#   metadata の形の崩れ（`packages`・`dependencies` の型、`kind` が null・build・dev 以外、`name` が文字列でない）、
#   共通の crate が workspace_root からの `crates/core/Cargo.toml` でちょうど 1 件に定まらない、cargo metadata の失敗、想定外の例外。
#   除外の印・除外の一覧・対象を外す口は持たない。
#
# 照合の規則:
#   crate 名を小文字にし `-` を `_` にそろえて、`objc2_metal`・`ash`・`cudarc` と完全一致したときだけ当てる（接頭辞には広げない）。
#   この 3 つの名前は Python 部先頭の「数え方の定義」の 1 か所にだけ持ち、rustdoc 側と依存側で共有する。
#   rustdoc 側の数え方（V4〜V6b）は public-api-check.sh と同じ。親の鍵が resolved_path なら type、それ以外は other。どちらも違反。
#   外部 crate の一覧は、公開 API から実際に解決された crate 名（自 crate を除く）の重複なしの昇順。
#   `external_crates` の表そのものではない（公開 API から参照されない依存も入るため）。
#   依存側は metadata の package の `dependencies[].name`（package 名）を、`[dependencies]`・`[build-dependencies]`・
#   `[dev-dependencies]`（target 付きと optional を含む）のすべてについて数える。feature の解決には依らない。
#
# 標準出力: rustdoc の JSON ごとの `scanned:`、依存側の `scanned:`、全部を検査し終えたとき（終了コード 0 と 1）だけ締めの `record:`、合格時の `ok:`。
#   `scanned: crate=<name> features=<組> format_version=<n> public_items=<n> sdk_type_refs=<n> sdk_other_refs=<n> external_crates=<a,b | ->`
#   `scanned: check=common-deps manifest=crates/core/Cargo.toml package=<name> declared_deps=<n> chip_deps=<n>`
#   `record: commit=<rev|-> documents=<n|-> public_items_total=<n|-> sdk_type_refs_total=<n|-> sdk_other_refs_total=<n|-> common_declared_deps=<n|-> common_chip_deps=<n|->`
#   commit は cargo・git を呼ぶ経路では ROOT_DIR の HEAD（変更が在れば末尾に `-dirty`）、それ以外では `-`。
# 標準エラー: `violation: crate=<name> features=<組> item=<name> kind=<type|other> references=<crate>`、
#   `violation: check=common-dep crate=<package> dependency=<name> kind=<normal|build|dev> target=<cfg|->`・`FAILED:`・`error:`。
#
# 検査しないもの: 共通の crate 以外の crate の依存、推移的な依存（`Cargo.lock` にだけ出るもの）、`package =` で改名した依存の別名
# （package 名で拾う）、上記 3 つ以外の名前、件数の増減（数は出すだけ）、wasm32 の公開 API。
# ホストの絶対パス・環境変数の値は出力に出さない（パスは workspace_root からの相対だけ）。ソースを変更しない。Python 3.8 以上の標準ライブラリだけ。
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

# ---- 数え方の定義（rustdoc 側と依存側で共有する唯一の場所。値をここの外へ書き写さない）----
# 検査の対象にするチップの SDK の crate 名（小文字・`-` を `_` にそろえた形）。
CHIP_SDK = frozenset({"objc2_metal", "ash", "cudarc"})
# 共通の crate の manifest（workspace_root からの相対。公開名は未決なので package 名では探さない）。
COMMON_MANIFEST_REL = "crates/core/Cargo.toml"
# 数える依存の種類（metadata の kind。null は通常の依存）。宣言を全部拾う側に倒す。
DEP_KINDS = {None: "normal", "build": "build", "dev": "dev"}
# 依存側の metadata を取るコマンド（ネットワークを使わず、lock を書き換えない）。
METADATA_CMD = ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1", "--all-features"]


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
    return name.lower().replace("-", "_") in CHIP_SDK


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


def inspect_metadata(meta):
    """metadata を検査して (package 名, 宣言数, 違反の一覧) を返す。共通の crate がちょうど 1 件に定まらなければ Misuse。"""
    need(isinstance(meta, dict), "metadata top level is not an object")
    wroot = meta.get("workspace_root")
    need(isinstance(wroot, str) and wroot != "", "missing or malformed 'workspace_root'")
    pkgs = meta.get("packages")
    need(isinstance(pkgs, list), "missing or malformed 'packages'")
    want = os.path.normpath(os.path.join(wroot, COMMON_MANIFEST_REL))
    found = []
    for pkg in pkgs:
        need(isinstance(pkg, dict), "malformed package entry")
        mp = pkg.get("manifest_path")
        if isinstance(mp, str) and os.path.normpath(mp) == want:
            found.append(pkg)
    need(len(found) == 1, "common crate not found exactly once")
    pkg = found[0]
    need(isinstance(pkg.get("name"), str), "malformed package name")
    deps = pkg.get("dependencies")
    need(isinstance(deps, list), "missing or malformed 'dependencies'")
    violations = []
    for dep in deps:
        need(isinstance(dep, dict), "malformed dependency entry")
        name = dep.get("name")
        need(isinstance(name, str), "dependency name is not a string")
        kind = dep.get("kind")
        need(kind is None or (isinstance(kind, str) and kind in DEP_KINDS), "dependency kind outside the counted kinds")
        target = dep.get("target")
        need(target is None or isinstance(target, str), "malformed dependency target")
        if is_chip_sdk(name):
            violations.append((clean(name), DEP_KINDS[kind], clean(target) if target else "-"))
    return clean(pkg["name"]), len(deps), violations


def run_cargo_metadata(root):
    """cargo metadata を argv のリストで起動して JSON を返す。失敗は Misuse（ホストの出力は載せない）。"""
    env = dict(os.environ, CARGO_TERM_COLOR="never")
    try:
        done = subprocess.run(
            METADATA_CMD, cwd=root, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False, timeout=300)
    except (OSError, subprocess.TimeoutExpired):
        raise Misuse("cannot run cargo metadata")
    need(done.returncode == 0, "cargo metadata failed")
    try:
        return json.loads(done.stdout.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        raise Misuse("cargo metadata output is not JSON")


def run_deps(meta, total):
    pkg, declared, violations = inspect_metadata(meta)
    print("scanned: check=common-deps manifest=%s package=%s declared_deps=%d chip_deps=%d"
          % (COMMON_MANIFEST_REL, pkg, declared, len(violations)))
    for name, kind, target in violations:
        print("violation: check=common-dep crate=%s dependency=%s kind=%s target=%s"
              % (pkg, name, kind, target), file=sys.stderr)
    total["declared"] = declared
    total["chip_deps"] = len(violations)


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
    parser.add_argument("--metadata-json", metavar="PATH", action="append")
    args = parser.parse_args(sys.argv[2:])
    need(args.metadata_json is None or len(args.metadata_json) == 1, "--metadata-json takes exactly one path")
    metadata_path = args.metadata_json[0] if args.metadata_json else None

    # 既定は両側を実行。どちらかを渡したときは、渡したものだけを検査する（cargo・git を呼ばない）。
    given = args.json is not None or args.metadata_json is not None
    run_rustdoc = not given or args.json is not None
    run_dependencies = not given or args.metadata_json is not None

    total = {"public": 0, "type": 0, "other": 0, "declared": 0, "chip_deps": 0}
    commit = "-" if given else source_commit(root)
    ndocs = 0
    if run_rustdoc:
        if args.json is not None:
            inputs = [("-", "-", p) for p in args.json]
            run_docs(inputs, total)
            ndocs = len(inputs)
        else:
            with tempfile.TemporaryDirectory() as tmp:
                inputs = emit_docs(root, tmp)
                run_docs(inputs, total)
            ndocs = len(inputs)
    if run_dependencies:
        meta = load_json(metadata_path) if metadata_path is not None else run_cargo_metadata(root)
        run_deps(meta, total)

    def num(v, ran):
        return str(v) if ran else "-"

    fields = [
        ("commit", commit),
        ("documents", num(ndocs, run_rustdoc)),
        ("public_items_total", num(total["public"], run_rustdoc)),
        ("sdk_type_refs_total", num(total["type"], run_rustdoc)),
        ("sdk_other_refs_total", num(total["other"], run_rustdoc)),
        ("common_declared_deps", num(total["declared"], run_dependencies)),
        ("common_chip_deps", num(total["chip_deps"], run_dependencies)),
    ]
    print("record: " + " ".join("%s=%s" % kv for kv in fields))
    refs = total["type"] + total["other"] + total["chip_deps"]
    if refs:
        print("FAILED: %d reference(s)" % refs, file=sys.stderr)
        return 1
    parts = []
    if run_rustdoc:
        parts.append("%d public API document(s) checked" % ndocs)
    if run_dependencies:
        parts.append("common crate dependencies checked")
    print("ok: %s, no chip SDK references" % ", ".join(parts))
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
