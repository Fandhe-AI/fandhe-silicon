#!/usr/bin/env bash
# 製品の workspace の依存グラフ（全 feature・全 target・通常 / ビルド時 / dev）に載る crate のうち、
# wgpu プロジェクトの crate を「狭い定義」と「広い定義」の 2 通りで数えて確かめる
# （spec TASK-11・BUILD-48・T-14 のポインタ。親 Issue #364 の 2/6）。
# 呼び出し元: 直接実行。
# 対象は製品の workspace（ROOT_DIR/Cargo.toml）だけ。baseline-wgpu/・measure/・wasm/ には当てない。
#
# 使い方:
#   scripts/dep-graph-wgpu-check.sh                                   cargo metadata と cargo tree を流して数える
#   scripts/dep-graph-wgpu-check.sh --metadata-json <path> [--tree-text <path>]
#                                                                     渡した入力だけを数える（cargo を呼ばない。
#                                                                     --tree-text は --metadata-json と組でだけ受け付ける）
# 終了コード: 0 2 定義とも 0 件 / 1 どちらかの定義で 1 件以上（違反）
#   2 誤用・前提の不足（python3・cargo・git が無い、cargo の失敗、コミットが取れない）・読めない入力・
#     metadata と tree の不一致・数えた crate が 0 件（成功扱いにしない）
# 標準出力: `scanned:` の行（読んだ入力ごと）、数え終えたとき（終了コード 0 と 1）だけ `record:` の行、合格時の `ok:` の 1 行。
#   `record: commit=<rev|-> crates_total=<n> workspace_members=<n> narrow_total=<n> wide_total=<n>`
#   commit は cargo を流す経路では HEAD のコミット（変更が在れば末尾に `-dirty`）、--metadata-json の経路では `-`。
# 標準エラー: `violation:`・`FAILED:`・`error:`。
#
# 数え方: 数えるのは一意の package（metadata の workspace_members から依存をたどって届くもの。
#   dep_kinds の種類は問わない）。tree 側の (名前, 版) の集合と一致しなければ終了コード 2。
#   2 つの定義はこのファイル先頭の定数 NARROW_NAMES・WIDE_REPOSITORY_PREFIX にまとめる。
#   狭い定義は package 名の完全一致（小文字化と `_`→`-` の正規化のうえ）、
#   広い定義は metadata の repository の接頭辞一致。repository が無い crate は広い定義に当たらない扱い。
#   同じ (名前, 版) が複数あるときは、どれか 1 つでも当たれば当たりとする。
# 検査しないもの: 広い定義の表記揺れ（大文字・git+・http・末尾 / ・.git）の吸収と repository 欠落の見える化、
#   別 workspace、件数の増減。
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

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# 狭い定義: package 名（小文字化・`_`→`-` の正規化後）の完全一致。
NARROW_NAMES = ("wgpu", "wgpu-core", "wgpu-hal", "wgpu-types", "naga")
# 広い定義: metadata の repository がこの接頭辞で始まる crate（正規化は持たない素の startswith）。
WIDE_REPOSITORY_PREFIX = "https://github.com/gfx-rs/"

METADATA_CMD = ["cargo", "metadata", "--locked", "--format-version", "1", "--all-features"]
TREE_CMD = [
    "cargo", "tree", "--locked", "--workspace", "--all-features", "--target", "all",
    "-e", "normal,build,dev", "--prefix", "none", "--no-dedupe",
]
TREE_LINE = re.compile(r"^(\S+) v(\S+)( \(proc-macro\))?( \(.+\))?$")


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def clean(text):
    """出力に出す名前から制御文字や想定外の文字を落とす。"""
    return re.sub(r"[^A-Za-z0-9_.:/+@-]", "?", str(text))[:100]


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def is_narrow(name):
    return name.lower().replace("_", "-") in NARROW_NAMES


def is_wide(repo):
    return isinstance(repo, str) and repo.startswith(WIDE_REPOSITORY_PREFIX)


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
    """記録に載せる、作業ツリーの HEAD のコミット（変更が在れば -dirty）を返す。取れなければ Misuse。"""
    rev = _git(root, ["rev-parse", "--verify", "HEAD^{commit}"])
    sha = rev.strip() if rev is not None else ""
    need(re.fullmatch(r"[0-9a-f]{40}([0-9a-f]{24})?", sha) is not None, "cannot resolve the source commit")
    st = _git(root, ["status", "--porcelain", "--untracked-files=normal", "--ignore-submodules=all"])
    need(st is not None, "cannot resolve the source commit")
    return sha + ("-dirty" if st.strip() else "")


def run_cargo(root, cmd):
    """cargo を引数の並びで起動して標準出力を返す。失敗は Misuse（標準エラーは出さない）。"""
    env = dict(os.environ, CARGO_TERM_COLOR="never")
    try:
        done = subprocess.run(
            cmd, cwd=root, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    except OSError:
        raise Misuse("cannot run cargo")
    need(done.returncode == 0, "%s failed" % " ".join(cmd[:2]))
    return done.stdout.decode("utf-8", "replace")


def read_text(path):
    try:
        with open(path, "rb") as f:
            return f.read().decode("utf-8")
    except (OSError, UnicodeDecodeError):
        raise Misuse("cannot read input file")


def parse_tree(text):
    """cargo tree --prefix none の出力から (名前, 版) の一意の集合を返す。"""
    found = set()
    for line in text.splitlines():
        if not line.strip():
            continue
        if line.endswith(" (*)"):
            line = line[:-4]
        m = TREE_LINE.match(line)
        need(m is not None, "unreadable cargo tree line")
        found.add((m.group(1), m.group(2)))
    return found


def reachable(md):
    """metadata から (workspace member 数, 到達できる package の一覧) を返す。"""
    need(isinstance(md, dict), "metadata top level is not an object")
    pkgs, members, resolve = md.get("packages"), md.get("workspace_members"), md.get("resolve")
    need(isinstance(pkgs, list), "missing or malformed 'packages'")
    need(isinstance(members, list) and members, "missing or empty 'workspace_members'")
    need(isinstance(resolve, dict) and isinstance(resolve.get("nodes"), list), "missing or malformed 'resolve'")
    by_id = {}
    for p in pkgs:
        need(isinstance(p, dict) and isinstance(p.get("id"), str), "malformed package")
        need(isinstance(p.get("name"), str) and isinstance(p.get("version"), str), "malformed package")
        repo = p.get("repository")
        need(repo is None or isinstance(repo, str), "malformed repository")
        by_id[p["id"]] = p
    nodes = {}
    for n in resolve["nodes"]:
        need(isinstance(n, dict) and isinstance(n.get("id"), str), "malformed resolve node")
        deps = n.get("deps")
        need(isinstance(deps, list), "malformed resolve node deps")
        ids = []
        for d in deps:
            need(isinstance(d, dict) and isinstance(d.get("pkg"), str), "malformed resolve dep")
            ids.append(d["pkg"])
        nodes[n["id"]] = ids
    for m in members:
        need(isinstance(m, str), "malformed workspace member")
    seen, stack = set(), list(members)
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        need(i in by_id, "package id not found in packages")
        stack.extend(nodes.get(i, []))
    return len(members), [by_id[i] for i in sorted(seen)]


def main():
    ap = argparse.ArgumentParser(prog="dep-graph-wgpu-check.sh", add_help=True)
    ap.add_argument("root")
    ap.add_argument("--metadata-json")
    ap.add_argument("--tree-text")
    a = ap.parse_args()
    if a.tree_text is not None and a.metadata_json is None:
        raise Misuse("--tree-text requires --metadata-json")

    tree_set = None
    if a.metadata_json is not None:
        commit = "-"
        try:
            md = json.loads(read_text(a.metadata_json))
        except ValueError:
            raise Misuse("metadata input is not JSON")
        if a.tree_text is not None:
            tree_set = parse_tree(read_text(a.tree_text))
    else:
        commit = source_commit(a.root)
        run_cargo(a.root, ["cargo", "--version"])
        try:
            md = json.loads(run_cargo(a.root, METADATA_CMD))
        except ValueError:
            raise Misuse("cargo metadata output is not JSON")
        tree_set = parse_tree(run_cargo(a.root, TREE_CMD))

    n_members, pkgs = reachable(md)
    need(pkgs, "no crates found in the dependency graph")
    # 一意の (名前, 版) にまとめる。同じ組のどれかが当たれば当たり。
    uniq = {}
    for p in pkgs:
        key = (p["name"], p["version"])
        cur = uniq.setdefault(key, {"narrow": False, "wide": False, "repos": []})
        cur["narrow"] = cur["narrow"] or is_narrow(p["name"])
        cur["wide"] = cur["wide"] or is_wide(p.get("repository"))
        cur["repos"].append(p.get("repository"))
    if tree_set is not None:
        need(set(uniq) == tree_set, "cargo metadata and cargo tree disagree on the crate set")

    narrow = sorted(k for k, v in uniq.items() if v["narrow"])
    wide = sorted(k for k, v in uniq.items() if v["wide"])
    print("scanned: reading=metadata crates=%d narrow=%d wide=%d" % (len(uniq), len(narrow), len(wide)))
    if tree_set is not None:
        print("scanned: reading=tree crates=%d" % len(tree_set))
    print("record: commit=%s crates_total=%d workspace_members=%d narrow_total=%d wide_total=%d"
          % (commit, len(uniq), n_members, len(narrow), len(wide)))
    for label, hits in (("narrow", narrow), ("wide", wide)):
        for name, ver in hits:
            repos = [r for r in uniq[(name, ver)]["repos"] if isinstance(r, str)]
            print("violation: definition=%s crate=%s version=%s repository=%s"
                  % (label, clean(name), clean(ver), clean(repos[0]) if repos else "-"), file=sys.stderr)
    if narrow or wide:
        print("FAILED: %d crate(s)" % len(set(narrow) | set(wide)), file=sys.stderr)
        return 1
    print("ok: %d crate(s) in the dependency graph checked, no wgpu-project crates (narrow=0 wide=0)" % len(uniq))
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
