#!/usr/bin/env bash
# 製品の workspace の依存グラフに載る crate のうち、
# wgpu プロジェクトの crate を「狭い定義」と「広い定義」の 2 通りで数えて確かめる
# （spec TASK-11・BUILD-48・T-14 のポインタ。親 Issue #364 の 2/6・3/6・4/6）。
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
#   `record: commit=<rev|-> crates_total=<n> workspace_members=<n> narrow_total=<n> wide_total=<n> no_repository=<n>`
#   commit は cargo を流す経路では HEAD のコミット（変更が在れば末尾に `-dirty`）、--metadata-json の経路では `-`。
#   repository が無い crate は 1 件ごとに `no-repository: crate=<name> version=<ver>`（`record:` の後。違反ではない）。
# 標準エラー: `violation:`・`FAILED:`・`error:`。
#
# 数え方: 数える依存の種類・target・feature、名前の一覧、広い定義の形は、Python 部の先頭の
#   「数え方の定義」ブロックにだけ置く（値をここへ書き写さない）。変える手順と出力の読み方は docs/dep-graph-wgpu-check.md。
#   数えるのは一意の package（metadata の workspace_members から、定義の依存の種類の辺をたどって届くもの）。
#   tree 側の (名前, 版) の集合と一致しなければ終了コード 2。
#   repository が無い（null か空）crate は広い定義に当たらない扱いで、総数には含め、件数と crate を標準出力に出す。
#   同じ (名前, 版) が複数あるときは、広い定義はどれか 1 つでも当たれば当たり、repository の欠落は全部が欠けるときだけ欠落。
# 検査しないもの: `www.` 付きのホスト、`git://`・`ssh://`・`git@github.com:` の形、github.com 以外のホスト、
#   組織の URL そのもの（`github.com/gfx-rs` で終わるもの）、別 workspace、件数の増減。
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

# ==== 数え方の定義（数え方を変えるときはここだけを変える） ====
# 何を数えるかの決定は private spec 側の記録に従う（TASK-82 のポインタのみ）。
# 狭い定義の名前は deny.toml の [bans] deny と同じ PR で一緒に変える。手順は docs/dep-graph-wgpu-check.md。
# コマンドの組み立て・走査・判定はこのブロックの値から作るので、値はこのブロックの外に書かない。

# 数える依存の種類（cargo の dep_kinds の kind。通常の依存は normal）。
DEP_KINDS = ("normal", "build", "dev")
# 数える target（"all" か target triple）。
TARGET = "all"
# 数える feature の指定（cargo の引数）。
FEATURE_ARGS = ("--all-features",)

# 狭い定義: package 名の完全一致。比較は名前の折りたたみ（小文字化のあと NARROW_NAME_REPLACE を適用）の後。
NARROW_NAMES = ("wgpu", "wgpu-core", "wgpu-hal", "wgpu-types", "naga")
NARROW_NAME_REPLACE = (("_", "-"),)

# 広い定義: metadata の repository を正規化した結果がこの接頭辞で始まる crate。
WIDE_REPOSITORY_PREFIX = "github.com/gfx-rs/"
# repository の正規化の規則（この順: 空白除去・小文字化・先頭の除去・scheme の除去・末尾の除去）。
REPOSITORY_STRIP_PREFIXES = ("git+",)
REPOSITORY_SCHEMES = ("https://", "http://")
REPOSITORY_STRIP_SUFFIXES = ("/", ".git")
# ==== 数え方の定義ここまで ====

# 定義から組み立てるコマンド（値はここに書かない）。target は metadata では --filter-platform に任せる。
METADATA_CMD = (
    ["cargo", "metadata", "--locked", "--format-version", "1", *FEATURE_ARGS]
    + ([] if TARGET == "all" else ["--filter-platform", TARGET])
)
TREE_CMD = [
    "cargo", "tree", "--locked", "--workspace", *FEATURE_ARGS, "--target", TARGET,
    "-e", ",".join(DEP_KINDS), "--prefix", "none", "--no-dedupe",
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
    folded = name.lower()
    for old, new in NARROW_NAME_REPLACE:
        folded = folded.replace(old, new)
    return folded in NARROW_NAMES


def normalize_repository(repo):
    """repository の書き方の揺れを吸収した形を返す（純粋な文字列操作だけ）。"""
    r = repo.strip().lower()
    for pre in REPOSITORY_STRIP_PREFIXES:
        if r.startswith(pre):
            r = r[len(pre):]
            break
    for pre in REPOSITORY_SCHEMES:
        if r.startswith(pre):
            r = r[len(pre):]
            break
    changed = True
    while changed:
        changed = False
        for suf in REPOSITORY_STRIP_SUFFIXES:
            if r.endswith(suf):
                r = r[:-len(suf)]
                changed = True
    return r


def is_wide(repo):
    return isinstance(repo, str) and normalize_repository(repo).startswith(WIDE_REPOSITORY_PREFIX)


def is_missing(repo):
    return repo is None or (isinstance(repo, str) and not repo.strip())


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
            kinds = d.get("dep_kinds")
            need(isinstance(kinds, list) and kinds, "malformed resolve dep kinds")
            counted = False
            for k in kinds:
                need(isinstance(k, dict), "malformed resolve dep kinds")
                kind = k.get("kind")
                # cargo が出す kind は null（通常）・normal・build・dev だけ。それ以外は辺を捨てず誤りとして止める。
                need(kind is None or kind in ("normal", "build", "dev"), "unknown resolve dep kind")
                # kind が null は通常の依存。定義の依存の種類に入る辺だけをたどる。
                if (kind or "normal") in DEP_KINDS:
                    counted = True
            if counted:
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
        # 到達した package の resolve node が欠けていたら、依存なしと読まずに誤りとして止める。
        need(i in nodes, "package id not found in resolve nodes")
        stack.extend(nodes[i])
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
        cur = uniq.setdefault(key, {"narrow": False, "wide": False, "missing": True, "repos": []})
        cur["narrow"] = cur["narrow"] or is_narrow(p["name"])
        cur["wide"] = cur["wide"] or is_wide(p.get("repository"))
        cur["missing"] = cur["missing"] and is_missing(p.get("repository"))
        cur["repos"].append(p.get("repository"))
    if tree_set is not None:
        need(set(uniq) == tree_set, "cargo metadata and cargo tree disagree on the crate set")

    narrow = sorted(k for k, v in uniq.items() if v["narrow"])
    wide = sorted(k for k, v in uniq.items() if v["wide"])
    missing = sorted(k for k, v in uniq.items() if v["missing"] and not v["wide"])
    print("scanned: reading=metadata crates=%d narrow=%d wide=%d no_repository=%d"
          % (len(uniq), len(narrow), len(wide), len(missing)))
    if tree_set is not None:
        print("scanned: reading=tree crates=%d" % len(tree_set))
    print("record: commit=%s crates_total=%d workspace_members=%d narrow_total=%d wide_total=%d no_repository=%d"
          % (commit, len(uniq), n_members, len(narrow), len(wide), len(missing)))
    for name, ver in missing:
        print("no-repository: crate=%s version=%s" % (clean(name), clean(ver)))
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
