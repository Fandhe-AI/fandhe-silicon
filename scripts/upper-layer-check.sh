#!/usr/bin/env bash
# 上の層の crate（crates/upper）が、特別な裏口を持たないことを機械で確かめる検査
# （spec TASK-15・BUILD-56・BUILD-57・BUILD-58・D-33 のポインタ。親 Issue #367 の 1/4・2/4）。
# 入れている検査は 4 つ（(1)(2) は 1/4、(3)(4) は 2/4）。
#   (1) unsafe の不在: crate root の `#![forbid(unsafe_code)]` の宣言と、crate ディレクトリ配下の
#       すべての .rs（src/・tests/・benches/・examples/・build.rs を含む）での `unsafe` の出現 0 件。
#       数え方は(2)と同じ識別子の分割で、コメント・文字列を除いた識別子トークンのうち `r#` の付かない `unsafe` だけ。
#       forbid は lib の crate root にしか効かず、tests/ などは別の crate なのでコンパイラが止めない。その穴を埋める。
#   (2) チップの crate の不在: 上の層の宣言された直接依存（通常・build・dev。全 target・全 feature）と、
#       ソース中のチップの crate 名と一致する識別子トークンが 0 件。
#       ソース側の契約: コメント・文字列を除いたテキストを Rust の識別子（XID_Start/XID_Continue と `_`、`r#` は外す）に
#       分け、名前（`-` は `_`）と一致するものを位置に関係なくすべて数える（`::`・use 文・`.`・キーワードの判定はしない）。
#       曖昧なものは数える側に倒す。ローカルの同名のモジュール・関数・メソッド・フィールドも数える（意図した過検出）。
#       根拠: 2018 edition 以降は直接依存に無い crate の名前をソースから解決できないので、本来の守りは
#       metadata の直接依存の検査で、ソース側の件数は重ねた守り。
#       依存は resolve でなく宣言（metadata の dependencies）を見る。feature・target の解決に依らず宣言を全部拾うため。
#   (3) 直接依存の許可リスト（BUILD-57 のポインタ）: 上の層の宣言された直接依存（(2) と同じ範囲）のうち、
#       package 名（`package =` の改名は実体の名前）が許可の名前に無いものを違反にする。
#       チップの crate は (2) の chip-dep と (3) の dep-allow の両方で報告される（独立した 2 つの検査なので二重報告は意図どおり）。
#   (4) 測定のビルドで検証用 feature が有効でないこと（BUILD-58・TASK-6 のポインタ）: 製品の外の測定の workspace の
#       member のうち契約の crate に依存するもの（通常・build・dev。path か名前で特定し、曖昧なら数える側に倒す）について、
#       契約の crate で有効になる feature に禁止の feature が含まれないことを確かめる。resolve でなく宣言を読む
#       （resolve の feature は workspace 全体で統一され package に帰属できないため）。有効な feature は、依存の宣言の
#       features・default（uses_default_features のとき）・member 自身の [features] からの転送（`<キー>/<feat>`・`<キー>?/<feat>`。
#       測定の検査は --all-features で流すので全 member の feature を有効とみなす）を、契約の crate の feature の表で閉包して求める。
# 呼び出し元: `make upper-layer-check`、CI の `deny` ジョブ、回帰テスト `scripts/test-upper-layer-check.sh`（`make deny` には含めない）。
#
# 使い方:
#   scripts/upper-layer-check.sh                            cargo metadata を流して数える
#   scripts/upper-layer-check.sh --metadata-json <path> --measure-metadata-json <path>
#                                                           渡した 2 つの metadata の JSON だけを読む（cargo・git を呼ばない。
#                                                           ソースは JSON の manifest_path が指す crate ディレクトリから読む。
#                                                           片方だけの指定は誤用。(4) を黙ってスキップしない）
# 終了コード: 0 合格 / 1 違反あり
#   2 誤用・前提の不足（python3・cargo・git が無い、cargo の失敗、metadata が JSON でない・形が崩れている、
#     crates/upper が見つからない・読めない、.rs が 0 件、UTF-8 でない、閉じていないコメント・文字列、
#     シンボリックリンク、契約の crate が製品の member にちょうど 1 件でない、測定の workspace の manifest が読めない・
#     その metadata の cargo の失敗・形の崩れ、--metadata-json と --measure-metadata-json の片方だけの指定、想定外の例外）
# 標準出力: `scanned:` の行（読んだ入力ごと）、数え終えたとき（終了コード 0 と 1）だけ `record:` の行、合格時の `ok:` の 1 行。
#   `record: commit=<rev|-> rs_files=<n> direct_deps=<n> forbid=<yes|no> unsafe_total=<n> chip_deps=<n> chip_paths=<n> other_deps=<n> measure_pkgs=<n> measure_contract_pkgs=<n> measure_forbidden=<n>`
#   other_deps は許可の名前に無い直接依存の数。measure_* は測定の workspace の member 数・契約に依存する member 数・禁止の feature の違反数。
#   commit は cargo を流す経路では HEAD のコミット（変更が在れば末尾に `-dirty`）、--metadata-json の経路では `-`。
# 標準エラー: `violation: check=<forbid|unsafe|chip-dep|chip-path|dep-allow|measure-feature> ...`・`FAILED:`・`error:`。
#   パスは crate ディレクトリからの相対パスだけを出す。
#
# 数え方: 対象の位置、チップの crate の名前、依存の種類、名前の折りたたみは、Python 部の先頭の
#   「数え方の定義」ブロックにだけ置く（値をここへ書き写さない）。
# 検査しないもの: マクロが生成するパス、`include!`・`#[path]` で crate ディレクトリの外から読むファイル、
#   `target/` と隠しディレクトリ（`.` 始まり）、`package =` で改名した依存のソース側の別名（依存側は package 名で拾う）、
#   bin target の root の forbid、`cfg_attr` で包んだ forbid、件数の増減、
#   測定の workspace の member でない path の crate を経由した間接の依存、resolve による feature の統一、
#   `cfg` で絞った target 依存の除外（宣言は全部拾う）。
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

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# ==== 数え方の定義（数え方を変えるときはここだけを変える） ====
# 何を数えるかの決定は private spec 側の記録に従う（TASK-15・BUILD-56・BUILD-57 のポインタのみ）。
# コマンドの組み立て・走査・判定はこのブロックの値から作るので、値はこのブロックの外に書かない。

# 対象の crate の manifest（workspace_root からの相対）。公開名は未決のため package 名でなく位置で特定する。
TARGET_MANIFEST_REL = "crates/upper/Cargo.toml"
# (3) 上の層の直接依存に許してよい package 名（折りたたんだ形で完全一致）。
ALLOWED_DEPS = ("fandhe-silicon-core", "fandhe-silicon-exec")
# 契約の crate の manifest（製品の workspace_root からの相対）。(4) で名前でなく位置で特定する。
CONTRACT_MANIFEST_REL = "crates/contract/Cargo.toml"
# 測定の workspace の manifest（製品の workspace_root からの相対）。
MEASURE_MANIFEST_REL = "measure/Cargo.toml"
# (4) 測定のビルドで契約の crate に対して有効になってはいけない feature。
FORBIDDEN_FEATURES = ("fault-injection", "test-support")
# 数える依存の種類（metadata の dependencies の kind。通常の依存は null）。
DEP_KINDS = ("normal", "build", "dev")
# チップの crate の package 名（折りたたんだ形で完全一致）。
CHIP_CRATES = ("objc2", "objc2-foundation", "objc2-metal", "objc2-core-foundation", "ash", "cudarc")
# 名前の折りたたみ（小文字化のあとに適用）。
NAME_REPLACE = (("_", "-"),)
# 数える feature の指定（cargo の引数）。target は絞らない（全 target）。
FEATURE_ARGS = ("--all-features",)
# ソース側で比べる識別子の形（`-` を `_` にしたもの）から crate 名への対応。
CHIP_IDENTS = {c.replace("-", "_"): c for c in CHIP_CRATES}
# 走査するソースの拡張子と、走査しないディレクトリ名（`.` 始まりも走査しない）。
SOURCE_EXT = ".rs"
SKIP_DIRS = ("target",)
# ==== 数え方の定義ここまで ====

# --offline で取得が要る場合は失敗（前提の不足）にする。--no-deps は workspace の package だけを読む（宣言は残る）。
METADATA_CMD = ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1", *FEATURE_ARGS]
MEASURE_METADATA_CMD = METADATA_CMD + ["--manifest-path", None]  # None は実行時に測定の manifest の位置で置き換える


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


def find_contract(md):
    """製品の metadata から契約の crate の package を返す（workspace member にちょうど 1 件。位置で特定）。"""
    want = os.path.normpath(os.path.join(md["workspace_root"], CONTRACT_MANIFEST_REL))
    hits = [p for p in md["packages"]
            if os.path.normpath(p["manifest_path"]) == want and p["id"] in md["workspace_members"]]
    need(len(hits) == 1, "the contract crate is not found exactly once in the workspace members")
    need(isinstance(hits[0].get("name"), str), "malformed contract package name")
    feats = hits[0].get("features")
    need(isinstance(feats, dict), "missing or malformed contract 'features'")
    for v in feats.values():
        need(isinstance(v, list) and all(isinstance(x, str) for x in v), "malformed contract feature table")
    return hits[0]


def enabled_features(dep, member_features, contract_features):
    """測定の member の 1 件の契約への依存について、契約の crate で有効になる feature の集合を返す。"""
    key = dep.get("rename") or dep["name"]
    need(isinstance(key, str), "malformed dependency rename")
    dep_feats = dep.get("features") or []
    need(isinstance(dep_feats, list) and all(isinstance(x, str) for x in dep_feats), "malformed dependency features")
    on = set(dep_feats)
    if dep.get("uses_default_features", True):
        on.add("default")
    for values in member_features.values():
        need(isinstance(values, list), "malformed member feature table")
        for v in values:
            need(isinstance(v, str), "malformed member feature table")
            for prefix in (key + "/", key + "?/"):
                if v.startswith(prefix):
                    on.add(v[len(prefix):])
    seen = set()
    todo = list(on)
    while todo:
        f = todo.pop()
        if f in seen:
            continue
        seen.add(f)
        for v in contract_features.get(f, []):
            if "/" not in v and not v.startswith("dep:"):
                todo.append(v)
    return seen


def check_measure(mmd, contract):
    """測定の workspace の member のうち契約の crate に依存するものの、禁止の feature の有効化を返す。

    戻り値は (member 数, 契約に依存する member 数, [(package, 依存の種類, feature)])。
    """
    need(isinstance(mmd, dict), "measure metadata top level is not an object")
    pkgs, members = mmd.get("packages"), mmd.get("workspace_members")
    need(isinstance(pkgs, list), "measure metadata: missing or malformed 'packages'")
    need(isinstance(members, list) and members, "measure metadata: missing or empty 'workspace_members'")
    for m in members:
        need(isinstance(m, str) and m, "measure metadata: malformed workspace member id")
    ids = [p.get("id") for p in pkgs if isinstance(p, dict)]
    for m in members:
        need(ids.count(m) == 1, "measure metadata: a workspace member is not found exactly once in 'packages'")
    contract_dir = os.path.dirname(os.path.normpath(contract["manifest_path"]))
    contract_name = fold(contract["name"])
    n_members, dependents, bad = 0, 0, []
    for p in pkgs:
        need(isinstance(p, dict) and isinstance(p.get("id"), str) and isinstance(p.get("name"), str),
        "measure metadata: malformed package")
        if p["id"] not in members:
            continue
        n_members += 1
        deps = p.get("dependencies")
        need(isinstance(deps, list), "measure metadata: missing or malformed 'dependencies'")
        mfeat = p.get("features", {})
        need(isinstance(mfeat, dict), "measure metadata: malformed package features")
        hit = False
        for d in deps:
            need(isinstance(d, dict) and isinstance(d.get("name"), str), "measure metadata: malformed dependency")
            kind = d.get("kind")
            need(kind is None or kind in ("normal", "build", "dev"), "unknown dependency kind")
            label = kind or "normal"
            need(label in DEP_KINDS, "dependency kind outside the counted kinds")
            path = d.get("path")
            need(path is None or isinstance(path, str), "measure metadata: malformed dependency path")
            # 位置か名前のどちらかが当たれば契約への依存とみなす（曖昧なら数える側）。
            if not ((path and os.path.normpath(path) == contract_dir) or fold(d["name"]) == contract_name):
                continue
            hit = True
            on = enabled_features(d, mfeat, contract["features"])
            for f in FORBIDDEN_FEATURES:
                if f in on:
                    bad.append((p["name"], label, f))
        if hit:
            dependents += 1
    bad.sort()
    return n_members, dependents, bad


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


def has_forbid(plain):
    """crate root のトップレベル（丸・角・波括弧のどれの中でもない位置）の inner attribute だけを宣言として数える。

    マクロの入力（`m!(...)`・`macro_rules!` の本体）や mod の中の宣言は crate の属性ではないので数えない。
    """
    depth_at = {}
    depth = 0
    for i, ch in enumerate(plain):
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            need(depth >= 0, "unbalanced brackets in the crate root")
        elif ch == "#":
            depth_at[i] = depth
    need(depth == 0, "unbalanced brackets in the crate root")
    for m in FORBID_RE.finditer(plain):
        if depth_at.get(m.start()) != 0:
            continue
        if "unsafe_code" in [s.strip() for s in m.group(1).split(",")]:
            return True
    return False


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


def unsafe_hits_in(stripped):
    """キーワードの `unsafe` の位置を返す。`r#unsafe` は識別子なので数えない。

    チップの名前の一致と同じ識別子の分割を使うので、`℘unsafe`・`unsafe·x` のように
    識別子の一部に `unsafe` を含むだけのものは数えない。
    """
    return [pos for pos, tok, raw in ident_tokens(stripped) if tok == "unsafe" and not raw]


def chip_path_hits(stripped, idents):
    """チップの crate の名前（識別子の形）と一致する識別子トークンの (位置, 名前) を、位置に関係なくすべて返す。"""
    return [(pos, tok) for pos, tok, _raw in ident_tokens(stripped) if tok in idents]


def main():
    ap = argparse.ArgumentParser(prog="upper-layer-check.sh", add_help=True)
    ap.add_argument("root")
    ap.add_argument("--metadata-json")
    ap.add_argument("--measure-metadata-json")
    a = ap.parse_args()

    # 片方だけの指定は誤用（(4) を黙ってスキップしない）。
    need((a.metadata_json is None) == (a.measure_metadata_json is None),
        "--metadata-json and --measure-metadata-json must be given together")

    if a.metadata_json is not None:
        commit = "-"
        try:
            md = json.loads(read_text(a.metadata_json))
            mmd = json.loads(read_text(a.measure_metadata_json))
        except ValueError:
            raise Misuse("metadata input is not JSON")
    else:
        commit = source_commit(a.root)
        run_cargo(a.root, ["cargo", "--version"])
        mpath = os.path.join(a.root, MEASURE_MANIFEST_REL)
        need(os.path.isfile(mpath) and not os.path.islink(mpath), "the measure workspace manifest is not readable")
        mcmd = [mpath if c is None else c for c in MEASURE_METADATA_CMD]
        try:
            md = json.loads(run_cargo(a.root, METADATA_CMD))
            mmd = json.loads(run_cargo(a.root, mcmd))
        except ValueError:
            raise Misuse("cargo metadata output is not JSON")

    crate_dir, crate_root, pkg = find_target(md)
    contract = find_contract(md)

    deps = pkg.get("dependencies")
    need(isinstance(deps, list), "missing or malformed 'dependencies'")
    chip_deps = []
    other_deps = []
    allowed = set(ALLOWED_DEPS)
    for d in deps:
        need(isinstance(d, dict) and isinstance(d.get("name"), str), "malformed dependency")
        kind = d.get("kind")
        need(kind is None or kind in ("normal", "build", "dev"), "unknown dependency kind")
        label = kind or "normal"
        need(label in DEP_KINDS, "dependency kind outside the counted kinds")
        if fold(d["name"]) in CHIP_CRATES:
            chip_deps.append((fold(d["name"]), label))
        if fold(d["name"]) not in allowed:
            other_deps.append((fold(d["name"]), label))
    chip_deps.sort()
    other_deps.sort()
    print("scanned: reading=metadata packages=%d target_deps=%d" % (len(md["packages"]), len(deps)))

    n_mpkgs, n_mdep, mbad = check_measure(mmd, contract)
    print("scanned: reading=measure-metadata packages=%d contract_dependents=%d" % (n_mpkgs, n_mdep))

    sources = collect_sources(crate_dir)
    need(sources, "no source files found in the upper-layer crate")
    print("scanned: reading=source rs_files=%d" % len(sources))

    root_rel = None
    declared = False
    unsafe_hits, chip_hits = [], []
    for rel, full in sources:
        plain = strip_code(read_text(full))
        if os.path.realpath(full) == os.path.realpath(crate_root):
            root_rel = rel
            declared = has_forbid(plain)
        # unsafe もチップの名前も、コメント・文字列を除いた同じ文字列を同じ識別子の分割で数える。
        for pos in unsafe_hits_in(plain):
            unsafe_hits.append((rel, line_of(plain, pos)))
        for pos, tok in chip_path_hits(plain, CHIP_IDENTS):
            chip_hits.append((rel, line_of(plain, pos), CHIP_IDENTS[tok]))
    need(root_rel is not None, "the lib crate root is not among the scanned sources")
    unsafe_hits.sort()
    chip_hits.sort()

    forbid = "yes" if declared else "no"
    print("record: commit=%s rs_files=%d direct_deps=%d forbid=%s unsafe_total=%d chip_deps=%d chip_paths=%d "
          "other_deps=%d measure_pkgs=%d measure_contract_pkgs=%d measure_forbidden=%d"
          % (commit, len(sources), len(deps), forbid, len(unsafe_hits), len(chip_deps), len(chip_hits),
            len(other_deps), n_mpkgs, n_mdep, len(mbad)))
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
    for name, kind in other_deps:
        total += 1
        print("violation: check=dep-allow crate=%s kind=%s" % (clean(name), kind), file=sys.stderr)
    for name, kind, feat in mbad:
        total += 1
        print("violation: check=measure-feature package=%s kind=%s feature=%s"
              % (clean(name), kind, clean(feat)), file=sys.stderr)
    if total:
        print("FAILED: %d violation(s)" % total, file=sys.stderr)
        return 1
    print("ok: %d .rs file(s) and %d direct dependenc(ies) checked, forbid(unsafe_code) declared, "
          "unsafe=0 chip_deps=0 chip_paths=0 other_deps=0 measure_contract_dependents=%d forbidden_features=0"
          % (len(sources), len(deps), n_mdep))
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
