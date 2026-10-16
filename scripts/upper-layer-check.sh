#!/usr/bin/env bash
# 上の層の crate（crates/upper）が、特別な裏口を持たないことを機械で確かめる検査
# （spec TASK-15・BUILD-56・BUILD-57・D-33 のポインタ。親 Issue #367 の 1/4）。
# この段で入れる検査は 2 つ。
#   (1) unsafe の不在: crate root の `#![forbid(unsafe_code)]` の宣言と、crate ディレクトリ配下の
#       すべての .rs（src/・tests/・benches/・examples/・build.rs を含む）での `unsafe` の出現 0 件。
#       forbid は lib の crate root にしか効かず、tests/ などは別の crate なのでコンパイラが止めない。その穴を埋める。
#   (2) チップの crate の不在: 上の層の宣言された直接依存（通常・build・dev。全 target・全 feature）と、
#       ソース中の `use`・パスに、チップの crate が 0 件。
#       依存は resolve でなく宣言（metadata の dependencies）を見る。feature・target の解決に依らず宣言を全部拾うため。
# 呼び出し元: 現時点では直接実行のみ（make ターゲットと CI への組み込みは後の段。Issue #3411 以降）。
#
# 使い方:
#   scripts/upper-layer-check.sh                            cargo metadata を流して数える
#   scripts/upper-layer-check.sh --metadata-json <path>     渡した metadata の JSON だけを読む（cargo・git を呼ばない。
#                                                           ソースは JSON の manifest_path が指す crate ディレクトリから読む）
# 終了コード: 0 合格 / 1 違反あり
#   2 誤用・前提の不足（python3・cargo・git が無い、cargo の失敗、metadata が JSON でない・形が崩れている、
#     crates/upper が見つからない・読めない、.rs が 0 件、UTF-8 でない、閉じていないコメント・文字列、
#     シンボリックリンク、想定外の例外）
# 標準出力: `scanned:` の行（読んだ入力ごと）、数え終えたとき（終了コード 0 と 1）だけ `record:` の行、合格時の `ok:` の 1 行。
#   `record: commit=<rev|-> rs_files=<n> direct_deps=<n> forbid=<yes|no> unsafe_total=<n> chip_deps=<n> chip_paths=<n>`
#   commit は cargo を流す経路では HEAD のコミット（変更が在れば末尾に `-dirty`）、--metadata-json の経路では `-`。
# 標準エラー: `violation: check=<forbid|unsafe|chip-dep|chip-path> ...`・`FAILED:`・`error:`。
#   パスは crate ディレクトリからの相対パスだけを出す。
#
# 数え方: 対象の位置、チップの crate の名前、依存の種類、名前の折りたたみは、Python 部の先頭の
#   「数え方の定義」ブロックにだけ置く（値をここへ書き写さない）。
# 検査しないもの: マクロが生成するパス、`include!`・`#[path]` で crate ディレクトリの外から読むファイル、
#   `target/` と隠しディレクトリ（`.` 始まり）、`package =` で改名した依存のソース側の別名（依存側は package 名で拾う）、
#   bin target の root の forbid、`cfg_attr` で包んだ forbid、件数の増減。
# ホストの絶対パス・環境変数の値は出力に出さない。ソースを変更しない。ネットワークを使わない。
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
import unicodedata

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# ==== 数え方の定義（数え方を変えるときはここだけを変える） ====
# 何を数えるかの決定は private spec 側の記録に従う（TASK-15・BUILD-56・BUILD-57 のポインタのみ）。
# コマンドの組み立て・走査・判定はこのブロックの値から作るので、値はこのブロックの外に書かない。

# 対象の crate の manifest（workspace_root からの相対）。公開名は未決のため package 名でなく位置で特定する。
TARGET_MANIFEST_REL = "crates/upper/Cargo.toml"
# 数える依存の種類（metadata の dependencies の kind。通常の依存は null）。
DEP_KINDS = ("normal", "build", "dev")
# チップの crate の package 名（折りたたんだ形で完全一致）。
CHIP_CRATES = ("objc2", "objc2-foundation", "objc2-metal", "objc2-core-foundation", "ash", "cudarc")
# 名前の折りたたみ（小文字化のあとに適用）。
NAME_REPLACE = (("_", "-"),)
# 数える feature の指定（cargo の引数）。target は絞らない（全 target）。
FEATURE_ARGS = ("--all-features",)
# 走査するソースの拡張子と、走査しないディレクトリ名（`.` 始まりも走査しない）。
SOURCE_EXT = ".rs"
SKIP_DIRS = ("target",)
# ==== 数え方の定義ここまで ====

# --offline で取得が要る場合は失敗（前提の不足）にする。--no-deps は workspace の package だけを読む（宣言は残る）。
METADATA_CMD = ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1", *FEATURE_ARGS]
# 識別子の文字（Unicode の単語文字。Rust の識別子は Unicode を含む）。
IDENT = r"\w"
# raw identifier の接頭辞（`r#unsafe` はキーワードでなく識別子）。
RAW = r"(?<!r#)"
# crate 名の判定でだけ許す raw identifier の接頭辞（`r#ash` は `ash` と同じ crate を指す）。
RAW_OPT = r"(?:r#)?"


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


def fold_ident_chars(s):
    """Rust の識別子継続文字（XID_Continue）のうち `\\w` に入らないもの（結合文字・連結用句読点）を `_` に置き換える。

    同じ長さを保つので位置と行番号は変わらない。`fn unsafé() {}` のような有効な識別子を境界の判定で割らない。
    """
    return "".join(
        "_" if (ord(c) > 127 and unicodedata.category(c) in ("Mn", "Mc", "Nd", "Pc")) else c
        for c in s
    )


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


def find_target(md):
    """metadata から (crate ディレクトリ, crate root, 対象 package) を返す。"""
    need(isinstance(md, dict), "metadata top level is not an object")
    pkgs, members, wroot = md.get("packages"), md.get("workspace_members"), md.get("workspace_root")
    need(isinstance(pkgs, list), "missing or malformed 'packages'")
    need(isinstance(members, list) and members, "missing or empty 'workspace_members'")
    need(isinstance(wroot, str) and wroot, "missing or malformed 'workspace_root'")
    want = os.path.normpath(os.path.join(wroot, TARGET_MANIFEST_REL))
    hits = []
    for p in pkgs:
        need(isinstance(p, dict) and isinstance(p.get("id"), str), "malformed package")
        mp = p.get("manifest_path")
        need(isinstance(mp, str), "malformed package manifest_path")
        if os.path.normpath(mp) == want and p["id"] in members:
            hits.append(p)
    need(len(hits) == 1, "the upper-layer crate is not found exactly once in the workspace members")
    pkg = hits[0]
    crate_dir = os.path.dirname(os.path.normpath(pkg["manifest_path"]))
    need(os.path.isdir(crate_dir), "the upper-layer crate directory is not readable")
    targets = pkg.get("targets")
    need(isinstance(targets, list), "malformed package targets")
    libs = []
    for t in targets:
        need(isinstance(t, dict) and isinstance(t.get("kind"), list), "malformed target")
        if "lib" in t["kind"]:
            need(isinstance(t.get("src_path"), str), "malformed target src_path")
            libs.append(os.path.normpath(t["src_path"]))
    need(len(libs) == 1, "the upper-layer crate must have exactly one lib target")
    return crate_dir, libs[0], pkg


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


FORBID_RE = re.compile(r"#\s*!\s*\[\s*forbid\s*\(([^()]*)\)\s*\]")
UNSAFE_RE = re.compile(r"(?<![%s])%sunsafe(?![%s])" % (IDENT, RAW, IDENT))


def has_forbid(stripped):
    """crate root のトップレベル（丸・角・波括弧のどれの中でもない位置）の inner attribute だけを宣言として数える。

    マクロの入力（`m!(...)`・`macro_rules!` の本体）や mod の中の宣言は crate の属性ではないので数えない。
    """
    depth_at = {}
    depth = 0
    for i, ch in enumerate(stripped):
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            need(depth >= 0, "unbalanced brackets in the crate root")
        elif ch == "#":
            depth_at[i] = depth
    need(depth == 0, "unbalanced brackets in the crate root")
    for m in FORBID_RE.finditer(stripped):
        if depth_at.get(m.start()) != 0:
            continue
        if "unsafe_code" in [s.strip() for s in m.group(1).split(",")]:
            return True
    return False


# 式・パターンの前に置かれ、直後の `::` を絶対パスの先頭にするキーワード。
PATH_KEYWORDS = ("return", "break", "continue", "in", "as", "else", "if", "match", "while", "move", "yield", "await", "let", "mut", "ref", "box", "dyn", "impl", "where", "for", "loop", "unsafe", "async", "const", "static", "pub", "fn", "type", "use")

TOKEN_RE = re.compile(r"%s%s+|::|[{},;*#\[\]]" % (RAW_OPT, IDENT))


def use_hits(stripped, name):
    """`use` 文の use tree（グループ・別名を含む）で、先頭の区間が name の出現位置の集合を返す。

    tree の中の属性（`#[...]`）は読み飛ばし、先頭の区間の判定に影響させない。
    """
    hits = set()
    for m in re.finditer(r"(?<![%s])%suse(?![%s])" % (IDENT, RAW, IDENT), stripped):
        empty = True  # 現在の tree がまだ区間を消費していない（先頭に在る）
        stack = []
        attr = 0  # 属性の角括弧の深さ（0 より大きい間は読み飛ばす）
        pending = False  # 直前が `#`（次の `[` が属性の開始）
        for t in TOKEN_RE.finditer(stripped, m.end()):
            tok = t.group()
            if attr:
                if tok == "[":
                    attr += 1
                elif tok == "]":
                    attr -= 1
                continue
            if tok == ";":
                break
            if tok == "#":
                pending = True
                continue
            if tok == "[":
                if pending:
                    attr = 1
                pending = False
                continue
            pending = False
            if tok == "]":
                continue
            if tok == "{":
                stack.append(empty)
            elif tok == ",":
                empty = stack[-1] if stack else False
            elif tok == "}":
                if stack:
                    stack.pop()
                empty = False
            elif tok[0].isalnum() or tok[0] == "_":
                if empty and (tok[2:] if tok.startswith("r#") else tok) == name:
                    hits.add(t.start())
                empty = False
    return hits


def chip_path_hits(stripped, ident):
    """チップの crate の名前（識別子の形）の出現位置の集合を返す。"""
    name = re.escape(ident)
    end = r"(?![%s])" % IDENT
    hits = set()
    for m in re.finditer(r"(?<![%s.#])%s%s%s\s*::" % (IDENT, RAW_OPT, name, end), stripped):
        # `foo::name::` のように別のパスの途中に出るものは数えない（先頭の `::name::` は数える）。
        before = stripped[:m.start()].rstrip()
        # 空白を挟んだメソッド呼び出し（`x . ash::<u32>()`）も crate のパスではない（`..` の範囲は除く）。
        if before.endswith(".") and not before.endswith(".."):
            continue
        if before.endswith("::"):
            prev = re.search(r"(%s+)\s*$" % IDENT, before[:-2])
            # 直前がキーワードなら、先頭の絶対パス（`return ::ash::f()`）なので数える。
            if prev is not None and prev.group(1) not in PATH_KEYWORDS:
                continue
        hits.add(m.start())
    hits.update(use_hits(stripped, name))
    for m in re.finditer(r"(?<![%s])extern\s+crate\s+%s(%s)%s" % (IDENT, RAW_OPT, name, end), stripped):
        hits.add(m.start(1))
    return hits


def main():
    ap = argparse.ArgumentParser(prog="upper-layer-check.sh", add_help=True)
    ap.add_argument("root")
    ap.add_argument("--metadata-json")
    a = ap.parse_args()

    if a.metadata_json is not None:
        commit = "-"
        try:
            md = json.loads(read_text(a.metadata_json))
        except ValueError:
            raise Misuse("metadata input is not JSON")
    else:
        commit = source_commit(a.root)
        run_cargo(a.root, ["cargo", "--version"])
        try:
            md = json.loads(run_cargo(a.root, METADATA_CMD))
        except ValueError:
            raise Misuse("cargo metadata output is not JSON")

    crate_dir, crate_root, pkg = find_target(md)

    deps = pkg.get("dependencies")
    need(isinstance(deps, list), "missing or malformed 'dependencies'")
    chip_deps = []
    for d in deps:
        need(isinstance(d, dict) and isinstance(d.get("name"), str), "malformed dependency")
        kind = d.get("kind")
        need(kind is None or kind in ("normal", "build", "dev"), "unknown dependency kind")
        label = kind or "normal"
        need(label in DEP_KINDS, "dependency kind outside the counted kinds")
        if fold(d["name"]) in CHIP_CRATES:
            chip_deps.append((fold(d["name"]), label))
    chip_deps.sort()
    print("scanned: reading=metadata packages=%d target_deps=%d" % (len(md["packages"]), len(deps)))

    sources = collect_sources(crate_dir)
    need(sources, "no source files found in the upper-layer crate")
    print("scanned: reading=source rs_files=%d" % len(sources))

    root_rel = None
    declared = False
    unsafe_hits, chip_hits = [], []
    for rel, full in sources:
        stripped = fold_ident_chars(strip_code(read_text(full)))
        if os.path.realpath(full) == os.path.realpath(crate_root):
            root_rel = rel
            declared = has_forbid(stripped)
        for m in UNSAFE_RE.finditer(stripped):
            unsafe_hits.append((rel, line_of(stripped, m.start())))
        for chip in CHIP_CRATES:
            for pos in chip_path_hits(stripped, chip.replace("-", "_")):
                chip_hits.append((rel, line_of(stripped, pos), chip))
    need(root_rel is not None, "the lib crate root is not among the scanned sources")
    unsafe_hits.sort()
    chip_hits.sort()

    forbid = "yes" if declared else "no"
    print("record: commit=%s rs_files=%d direct_deps=%d forbid=%s unsafe_total=%d chip_deps=%d chip_paths=%d"
          % (commit, len(sources), len(deps), forbid, len(unsafe_hits), len(chip_deps), len(chip_hits)))
    total = 0
    if not declared:
        total += 1
        print("violation: check=forbid file=%s" % clean(root_rel), file=sys.stderr)
    for rel, line in unsafe_hits:
        total += 1
        print("violation: check=unsafe file=%s line=%d" % (clean(rel), line), file=sys.stderr)
    for name, kind in chip_deps:
        total += 1
        print("violation: check=chip-dep crate=%s kind=%s" % (clean(name), kind), file=sys.stderr)
    for rel, line, name in chip_hits:
        total += 1
        print("violation: check=chip-path file=%s line=%d crate=%s" % (clean(rel), line, clean(name)), file=sys.stderr)
    if total:
        print("FAILED: %d violation(s)" % total, file=sys.stderr)
        return 1
    print("ok: %d .rs file(s) and %d direct dependenc(ies) checked, forbid(unsafe_code) declared, "
          "unsafe=0 chip_deps=0 chip_paths=0" % (len(sources), len(deps)))
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
