#!/usr/bin/env bash
# 下-2 の crate（crates/exec）の依存の向きを機械で確かめる検査
# （spec TASK-259・BUILD-55 のポインタ。親 Issue #474 の 1/4）。
# 入れている検査は 3 つ。
#   (a) 依存の宣言: `cargo metadata` の対象 crate の dependencies（通常・build・dev。target 付き・optional を含む。
#       `package =` で改名した依存は実体の package 名で見る）が、許す名前だけであること。
#       許す名前でも path が製品の crates/core を指さない（registry・git・別の path）ものは違反。
#   (b) 依存グラフ: `cargo tree`（全 feature・全 target・通常/build/dev）の深さ 0 が対象 crate であること、
#       深さ 1 の package 名が許す名前だけであること、深さ 1 以上のどこにも禁じる名前が出ないこと。
#       禁じる名前に当たらない外部の crate の有無は判定しない。行は (深さ, 名前) の集合として読む。
#   (c) ソース: crate ディレクトリ配下のすべての .rs の、コメント・文字列を除いた識別子トークンのうち、
#       禁じる名前（`-` は `_`）と一致するものを位置に関係なくすべて数える（ローカルの同名も数える過検出の側）。
# 禁じる名前は、製品の workspace の member のうち対象 crate と共通の crate 以外のすべて（metadata から実行時に作る）と、
# チップの crate の名前（定義ブロック）。
# 呼び出し元: `make exec-deps-check`（`make deny` には含めない）。
#
# 使い方:
#   scripts/exec-deps-check.sh                       cargo と git を流して検査する
#   scripts/exec-deps-check.sh --metadata-json <path> --tree-text <path>
#                                                    渡した 2 つだけを読む（cargo・git を呼ばない。ソースは JSON の
#                                                    manifest_path が指す crate ディレクトリから読む。片方だけの指定・
#                                                    同じオプションの重複・未知のオプションは誤用）
# 終了コード: 0 合格 / 1 違反あり
#   2 誤用・前提の不足（python3・cargo・git が無い、cargo の失敗、metadata が JSON でない・形が崩れている、
#     tree の行が読めない・深さ 0 が対象でない、対象が一意でない、crate ディレクトリが読めない、.rs が 0 件、
#     UTF-8 でない、閉じていないコメント・文字列、シンボリックリンク、想定外の例外）
# 標準出力: `scanned:` の行（読んだ入力ごと）、数え終えたとき（終了コード 0 と 1）だけ `record:` の行、合格時の `ok:` の 1 行。
#   `record: commit=<rev|-> rs_files=<n> declared_deps=<n> other_deps=<n> dep_source=<n> tree_pkgs=<n> tree_direct=<n> tree_forbidden=<n> source_hits=<n>`
#   commit は cargo を流す経路では HEAD のコミット（変更が在れば末尾に `-dirty`）、--metadata-json の経路では `-`。
# 標準エラー: `violation: check=<dep-allow|dep-source|tree-direct|tree-forbidden|source-name> ...`・`FAILED:`・`error:`。
#   パスは crate ディレクトリからの相対パスだけを出す。
#
# 数え方: 対象の位置、許す依存、チップの crate の名前、依存の種類、名前の折りたたみ、cargo の引数は、
#   Python 部の先頭の「数え方の定義」ブロックにだけ置く（値をここへ書き写さない）。
# 検査しないもの: マクロが生成するパス、`include!`・`#[path]` で crate ディレクトリの外から読むファイル、
#   `target/` と隠しディレクトリ（`.` 始まり）、resolve による feature の統一、対象の feature の形、`unsafe` の有無。
# ホストの絶対パス・環境変数の値は出力に出さない。ソースと Cargo.lock を変更しない。ネットワークを使わない。
# Python 3.8 以上の標準ライブラリだけ。
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
# 何を数えるかの決定は private spec 側の記録に従う（TASK-259・BUILD-55 のポインタのみ）。
# コマンドの組み立て・走査・判定はこのブロックの値から作るので、値はこのブロックの外に書かない。

# 対象の crate の manifest（workspace_root からの相対）。位置で特定する。
TARGET_MANIFEST_REL = "crates/exec/Cargo.toml"
# 対象の直接依存に許す package 名（折りたたんだ形で完全一致）。
ALLOWED_DIRECT = ("fandhe-silicon-core",)
# 許す依存の path の先（workspace_root からの相対。crate のディレクトリ）。
ALLOWED_DIRECT_DIR_REL = "crates/core"
# 許す依存の manifest（禁じる名前から除く member の位置）。
ALLOWED_MANIFEST_REL = "crates/core/Cargo.toml"
# 数える依存の種類（metadata の dependencies の kind。通常の依存は null）。
DEP_KINDS = ("normal", "build", "dev")
# チップの crate の package 名（折りたたんだ形で完全一致）。
CHIP_CRATES = ("objc2", "objc2-foundation", "objc2-metal", "objc2-core-foundation", "ash", "cudarc")
# 名前の折りたたみ（小文字化のあとに適用）。
NAME_REPLACE = (("_", "-"),)
# 数える feature の指定（cargo の引数）。
FEATURE_ARGS = ("--all-features",)
# cargo tree の辺の種類と target（target は絞らない）。
TREE_EDGES = ",".join(DEP_KINDS)
TREE_TARGET = "all"
# 走査するソースの拡張子と、走査しないディレクトリ名（`.` 始まりも走査しない）。
SOURCE_EXT = ".rs"
SKIP_DIRS = ("target",)
# ==== 数え方の定義ここまで ====

# --offline で取得が要る場合は失敗（前提の不足）にする。--no-deps は workspace の package だけを読む（宣言は残る）。
METADATA_CMD = ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1", *FEATURE_ARGS]


def tree_cmd(package):
    """対象 package の依存グラフを深さつきで出すコマンド（--no-dedupe は付けない）。"""
    return ["cargo", "tree", "--locked", "--offline", "-p", package, *FEATURE_ARGS,
            "-e", TREE_EDGES, "--target", TREE_TARGET, "--prefix", "depth"]


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def clean(text):
    """出力に出す名前から制御文字や想定外の文字を落とす。"""
    return re.sub(r"[^A-Za-z0-9_.:/+@-]", "?", str(text))[:100]


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def fold(name):
    folded = name.lower()
    for old, new in NAME_REPLACE:
        folded = folded.replace(old, new)
    return folded


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


def strip_code(text):
    """コメントと文字列・文字リテラルを空白に置き換えた同じ長さの文字列を返す（改行は残し、行番号を保つ）。

    閉じていないコメント・文字列は Misuse（読み切れない入力を合格にしない）。
    """
    n = len(text)
    out = []
    i = 0

    def blank(s):
        return "".join(c if c == "\n" else " " for c in s)

    def is_ident(c):
        return c.isalnum() or c == "_"

    while i < n:
        c = text[i]
        two = text[i:i + 2]
        if two == "//":
            j = text.find("\n", i)
            j = n if j < 0 else j
            out.append(blank(text[i:j]))
            i = j
            continue
        if two == "/*":
            depth, j = 1, i + 2
            while j < n and depth > 0:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            need(depth == 0, "unterminated block comment")
            out.append(blank(text[i:j]))
            i = j
            continue
        prev_ident = i > 0 and is_ident(text[i - 1])
        # 文字列（通常・生・b/c 接頭辞つき）
        if c == '"' or (c in "rbc" and not prev_ident):
            j = i
            if text[j] in "bc":
                j += 1
            raw = False
            hashes = 0
            if j < n and text[j] == "r":
                raw = True
                j += 1
                while j < n and text[j] == "#":
                    hashes += 1
                    j += 1
            if j < n and text[j] == '"':
                j += 1
                if raw:
                    close = '"' + "#" * hashes
                    k = text.find(close, j)
                    need(k >= 0, "unterminated string literal")
                    end = k + len(close)
                else:
                    k = j
                    while k < n and text[k] != '"':
                        k += 2 if text[k] == "\\" else 1
                    need(k < n, "unterminated string literal")
                    end = k + 1
                out.append(blank(text[i:end]))
                i = end
                continue
        # 文字リテラル（b'x' を含む）とライフタイム・ラベルの区別
        if c == "'" or (c == "b" and not prev_ident and text[i + 1:i + 2] == "'"):
            q = i if c == "'" else i + 1
            end = -1
            if text[q + 1:q + 2] == "\\":
                k = q + 3
                while k < n and k < q + 14 and text[k] != "'" and text[k] != "\n":
                    k += 1
                if k < n and text[k] == "'":
                    end = k + 1
            elif q + 2 < n and text[q + 2] == "'" and text[q + 1] not in "'\n":
                end = q + 3
            if end > 0:
                out.append(blank(text[i:end]))
                i = end
                continue
        out.append(c)
        i += 1
    return "".join(out)


def line_of(text, pos):
    return text.count("\n", 0, pos) + 1




def collect_sources(crate_dir):
    """crate ディレクトリ配下の .rs を (相対パス, 絶対パス) の並びで返す。リンクは追わず Misuse。"""
    found = []
    def walk_failed(err):
        raise Misuse("cannot read a directory in the crate")

    for cur, dirs, files in os.walk(crate_dir, followlinks=False, onerror=walk_failed):
        keep = []
        for d in sorted(dirs):
            if d.startswith(".") or d in SKIP_DIRS:
                continue
            need(not os.path.islink(os.path.join(cur, d)), "symbolic link in the crate directory")
            keep.append(d)
        dirs[:] = keep
        for f in sorted(files):
            if not f.endswith(SOURCE_EXT):
                continue
            full = os.path.join(cur, f)
            need(not os.path.islink(full), "symbolic link in the crate directory")
            found.append((os.path.relpath(full, crate_dir).replace(os.sep, "/"), full))
    found.sort()
    return found



def ident_tokens(text):
    """コメント・文字列を除いたソースから、Rust の識別子トークンを (位置, 名前, raw か) で返す。

    識別子は XID_Start か `_` で始まり XID_Continue が続く並び。`r#name` は `name` として返し、raw は True にする
    （チップの名前の一致は raw でも数え、unsafe の一致は raw を数えない。呼び出し側が使い分ける）。
    数値リテラル（`1u8`・`0x1f`）は継続文字ごと 1 つの塊として読み飛ばし、接尾辞を識別子に数えない。
    `::`・`.`・`use` などの周りの文字は見ない（位置による判定をしない）。
    """
    def start(c):
        return c == "_" or c.isidentifier()

    def cont(c):
        return c == "_" or ("a" + c).isidentifier()

    n = len(text)
    i = 0
    while i < n:
        c = text[i]
        if c.isdigit():
            j = i + 1
            while j < n and cont(text[j]):
                j += 1
            i = j
            continue
        if not start(c):
            i += 1
            continue
        j = i + 1
        while j < n and cont(text[j]):
            j += 1
        if text[i:j] == "r" and text[j:j + 1] == "#" and j + 1 < n and start(text[j + 1]):
            k = j + 2
            while k < n and cont(text[k]):
                k += 1
            yield j + 1, text[j + 1:k], True
            i = k
            continue
        yield i, text[i:j], False
        i = j




TREE_LINE_RE = re.compile(r"^(\d+)([A-Za-z_][A-Za-z0-9_-]*) v\S+( \(.*\))?$")
# cargo tree が種別ごとに挟む節の見出し行（[dev-dependencies]・[build-dependencies] など）。節の区切りとして読み飛ばす。
TREE_HEADER_RE = re.compile(r"^\d*\[[A-Za-z][A-Za-z0-9 _-]*\]$")


def find_target(md):
    """metadata から (crate ディレクトリ, 対象 package, 禁じる名前の集合, 許す依存の crate ディレクトリ) を返す。"""
    need(isinstance(md, dict), "metadata top level is not an object")
    pkgs, members, wroot = md.get("packages"), md.get("workspace_members"), md.get("workspace_root")
    need(isinstance(pkgs, list), "missing or malformed 'packages'")
    need(isinstance(members, list) and members, "missing or empty 'workspace_members'")
    need(isinstance(wroot, str) and wroot, "missing or malformed 'workspace_root'")
    want = os.path.normpath(os.path.join(wroot, TARGET_MANIFEST_REL))
    common = os.path.normpath(os.path.join(wroot, ALLOWED_MANIFEST_REL))
    hits = []
    forbidden = set(CHIP_CRATES)
    for p in pkgs:
        need(isinstance(p, dict) and isinstance(p.get("id"), str), "malformed package")
        mp, name = p.get("manifest_path"), p.get("name")
        need(isinstance(mp, str) and isinstance(name, str), "malformed package")
        if p["id"] not in members:
            continue
        norm = os.path.normpath(mp)
        if norm == want:
            hits.append(p)
        elif norm != common:
            forbidden.add(fold(name))
    need(len(hits) == 1, "the target crate is not found exactly once in the workspace members")
    pkg = hits[0]
    need(isinstance(pkg.get("name"), str), "malformed target package name")
    crate_dir = os.path.dirname(os.path.normpath(pkg["manifest_path"]))
    need(os.path.isdir(crate_dir), "the target crate directory is not readable")
    return crate_dir, pkg, forbidden, os.path.normpath(os.path.join(wroot, ALLOWED_DIRECT_DIR_REL))


def check_declared(pkg, allowed_dir):
    """宣言の検査。戻り値は (宣言数, [(check, 種類, 名前)])。"""
    deps = pkg.get("dependencies")
    need(isinstance(deps, list), "missing or malformed 'dependencies'")
    bad = []
    for d in deps:
        need(isinstance(d, dict) and isinstance(d.get("name"), str), "malformed dependency")
        kind = d.get("kind")
        need(kind is None or kind in ("normal", "build", "dev"), "unknown dependency kind")
        label = kind or "normal"
        need(label in DEP_KINDS, "dependency kind outside the counted kinds")
        path = d.get("path")
        need(path is None or isinstance(path, str), "malformed dependency path")
        name = fold(d["name"])
        if name not in ALLOWED_DIRECT:
            bad.append(("dep-allow", label, name))
        elif path is None or os.path.normpath(path) != allowed_dir:
            bad.append(("dep-source", label, name))
    bad.sort()
    return len(deps), bad


def parse_tree(text, target_name):
    """tree の出力を (深さ, 折りたたんだ名前) の集合にする。形が崩れていれば Misuse。"""
    entries = set()
    nlines = 0
    for line in text.splitlines():
        if not line.strip():
            continue
        if TREE_HEADER_RE.match(line.strip()):
            continue
        m = TREE_LINE_RE.match(line.rstrip())
        need(m is not None, "cannot parse a dependency tree line")
        nlines += 1
        entries.add((int(m.group(1)), fold(m.group(2))))
    need(entries, "the dependency tree is empty")
    roots = {n for d, n in entries if d == 0}
    need(roots == {fold(target_name)}, "the dependency tree root is not the target crate")
    return entries, nlines


def main():
    ap = argparse.ArgumentParser(prog="exec-deps-check.sh", add_help=False, allow_abbrev=False)
    ap.add_argument("root")
    ap.add_argument("--metadata-json")
    ap.add_argument("--tree-text")
    a = ap.parse_args()
    # 同じオプションの重複は誤用（argparse は後勝ちにするので数え直す）。
    for opt in ("--metadata-json", "--tree-text"):
        n = sum(1 for x in sys.argv[2:] if x == opt or x.startswith(opt + "="))
        need(n <= 1, "duplicate option")
    need((a.metadata_json is None) == (a.tree_text is None),
        "--metadata-json and --tree-text must be given together")

    if a.metadata_json is not None:
        commit = "-"
        try:
            md = json.loads(read_text(a.metadata_json))
        except ValueError:
            raise Misuse("metadata input is not JSON")
        tree_text = read_text(a.tree_text)
        crate_dir, pkg, forbidden, allowed_dir = find_target(md)
    else:
        commit = source_commit(a.root)
        run_cargo(a.root, ["cargo", "--version"])
        try:
            md = json.loads(run_cargo(a.root, METADATA_CMD))
        except ValueError:
            raise Misuse("cargo metadata output is not JSON")
        crate_dir, pkg, forbidden, allowed_dir = find_target(md)
        tree_text = run_cargo(a.root, tree_cmd(pkg["name"]))

    n_decl, decl_bad = check_declared(pkg, allowed_dir)
    print("scanned: reading=metadata packages=%d target_deps=%d forbidden_names=%d"
          % (len(md["packages"]), n_decl, len(forbidden)))

    entries, nlines = parse_tree(tree_text, pkg["name"])
    print("scanned: reading=tree lines=%d" % nlines)
    direct = sorted({n for d, n in entries if d == 1})
    tree_direct_bad = [n for n in direct if n not in ALLOWED_DIRECT]
    tree_forbidden = sorted((d, n) for d, n in entries if d >= 1 and n in forbidden)
    tree_pkgs = len({n for d, n in entries if d >= 1})

    sources = collect_sources(crate_dir)
    need(sources, "no source files found in the target crate")
    print("scanned: reading=source rs_files=%d" % len(sources))
    idents = {f.replace("-", "_"): f for f in forbidden}
    hits = []
    for rel, full in sources:
        plain = strip_code(read_text(full))
        for pos, tok, _raw in ident_tokens(plain):
            if tok in idents:
                hits.append((rel, line_of(plain, pos), idents[tok]))
    hits.sort()

    dep_allow = [b for b in decl_bad if b[0] == "dep-allow"]
    dep_source = [b for b in decl_bad if b[0] == "dep-source"]
    print("record: commit=%s rs_files=%d declared_deps=%d other_deps=%d dep_source=%d tree_pkgs=%d "
          "tree_direct=%d tree_forbidden=%d source_hits=%d"
          % (commit, len(sources), n_decl, len(dep_allow), len(dep_source), tree_pkgs,
            len(direct), len(tree_forbidden), len(hits)))
    total = 0
    for check, kind, name in decl_bad:
        total += 1
        print("violation: check=%s kind=%s name=%s" % (check, kind, clean(name)), file=sys.stderr)
    for name in tree_direct_bad:
        total += 1
        print("violation: check=tree-direct name=%s" % clean(name), file=sys.stderr)
    for depth, name in tree_forbidden:
        total += 1
        print("violation: check=tree-forbidden depth=%d name=%s" % (depth, clean(name)), file=sys.stderr)
    for rel, line, name in hits:
        total += 1
        print("violation: check=source-name file=%s line=%d name=%s" % (clean(rel), line, clean(name)),
              file=sys.stderr)
    if total:
        print("FAILED: %d violation(s)" % total, file=sys.stderr)
        return 1
    print("ok: %d .rs file(s), %d declared dependenc(ies) and %d tree package(s) checked, "
          "other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0" % (len(sources), n_decl, tree_pkgs))
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
