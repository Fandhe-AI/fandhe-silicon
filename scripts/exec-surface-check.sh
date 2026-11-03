#!/usr/bin/env bash
# 下-2 の crate（crates/exec）のソースの環境変数とファイルの読み取りの字句、および既定の feature の公開の面にある
# 試験用の口の名前を数える検査（spec TASK-923 のポインタ。親 Issue #867）。
# 呼び出し元: `make exec-surface-check`、回帰テスト `scripts/test-exec-surface-check.sh`。
#
# 使い方:
#   scripts/exec-surface-check.sh                      既定の crates/exec を走査し、cargo で rustdoc の JSON を 2 組（default / all-features）作って名前を数える
#   scripts/exec-surface-check.sh --crate-dir <dir>    <dir> の crate のソースを走査する（JSON は同様に cargo で作る）
#   scripts/exec-surface-check.sh [--crate-dir <dir>] --json-default <path> --json-all <path>
#                                                      渡した 2 つの rustdoc の JSON だけで名前を数える（cargo を呼ばない。2 つ揃えて 1 回ずつ）
# 終了コード: 0 期待どおり（違反 0 件）/ 1 違反あり /
#   2 誤用・前提の不足・読み切れない入力（python3 が無い、未知の引数、--json-default と --json-all の片方だけ・重複、
#     crate ディレクトリが無い、.rs が 0 件、UTF-8 でない、シンボリックリンク、ディレクトリやファイルが読めない、
#     `use std::{` の対応する `}` が無い、既定の走査でリビジョンを読めない、cargo・rustdoc が使えない、
#     rustdoc の JSON を作れない、JSON が読めない・形が崩れている・root が fandhe_silicon_exec でない、想定外の例外）
# 標準出力: 読んだファイルごとに `scanned: file=<crate からの相対パス>`、JSON の組ごとに
#   `scanned: features=<default|all-features> items=<crate_id 0 のアイテムの数>`、数え終えたとき（終了コード 0 と 1）だけ
#   `record: commit=<rev|-> rs_files=<n> build_rs=<yes|no> env_fs=<n> items_default=<n> items_all=<n>
#   port_names_default=<n> port_names_all=<n> port_names_missing_all=<n>`、合格時の `ok:` の 1 行。
#   rs_files は build.rs を含む見つけた .rs の数。commit は既定の crate を走査したときだけ HEAD
#   （変更が在れば末尾に `-dirty`）、--crate-dir のときは `-`。
# 標準エラー: 違反ごとに `violation: check=env-fs file=<相対パス> line=<n> word=<字句>`、
#   `violation: check=port-name features=default name=<名前> count=<n>`、
#   `violation: check=port-name-positive features=all-features name=<名前>`、
#   違反があれば `FAILED: <n> violation(s)`、前提の不足は `error:`。
#
# 数え方: 対象のファイル、字句、名前、期待は、Python 部の先頭の「数え方の定義」ブロックにだけ置く。
# 検査しないもの: 構文の解釈（マクロが生成するパス、別名の取り込み）、target/ と `.` 始まりのディレクトリ、.rs 以外のファイル、
#   core・upper の公開の面、wasm32、型の中身。
# JSON を作る段は専用の target ディレクトリだけに書き、cargo の出力は出さない。`RUSTC_BOOTSTRAP=1` は rustdoc を流す子プロセスにだけ渡す。
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
# 既定の対象の crate（リポジトリのルートからの相対）。
DEFAULT_CRATE_REL = "crates/exec"
# 走査するソース: crate ディレクトリの下で見つかったすべての .rs（build.rs を含む）。
BUILD_FILE = "build.rs"
SOURCE_EXT = ".rs"
# 辿らないディレクトリ名（`.` 始まりも辿らない）。
SKIP_DIRS = ("target",)
# 識別子の一部になる文字（前後がこれでないものを識別子としての一致とする）。
IDENT = "A-Za-z0-9_"
# 数えるのは各ファイルの生のテキスト（コメント・文字列を除かない）。期待は 0 件。
# 環境変数とファイルの読み取り（FB-13）: 次の字句を部分文字列としてそれぞれ独立に数える（重なりは引かない）。
ENV_FS_LITERALS = (
    "std::env", "env::var", "option_env!", "env!", "std::fs", "File::open",
    "include_str!", "include_bytes!", "include!",
)
# 加えて、`use` 文の `std::{…}`（入れ子を含む）の中に識別子として現れる語。
USE_STD_BRACE = r"\buse\s+(?:::\s*)?std\s*::\s*\{"
USE_STD_WORDS = r"(?<![%s])(env|fs)(?![%s])" % (IDENT, IDENT)
# リビジョンを読む外部コマンドの待ち時間の上限（秒）。
REV_TIMEOUT = 20
# 公開の面の名前（FB-10）: rustdoc の JSON の crate_id 0 のアイテム（種類・visibility を問わない）の名前と完全一致で数える。
PORT_NAMES = ("force_unsupported", "inject_denormal", "ForcedUnsupported", "DenormalInjection", "Injected")
# JSON を作る対象: package 名・lib の名前・target ディレクトリの下位の名前。
EXEC_PACKAGE = "fandhe-silicon-exec"
EXEC_LIB = "fandhe_silicon_exec"
TARGET_SUBDIR = "exec-surface-check"
# JSON の組: (ラベル, cargo の feature の引数)。default は期待 0 件、all-features は各名前 1 件以上。
JSON_SETS = (("default", ()), ("all-features", ("--all-features",)))
# cargo の子プロセスの待ち時間の上限（秒）。
CARGO_TIMEOUT = 580
# ==== 数え方の定義ここまで ====


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def collect_files(crate):
    """走査するファイルの crate からの相対パス（ソート済み、区切りは /）と、直下の build.rs の有無を返す。"""
    need(os.path.isdir(crate) and not os.path.islink(crate), "crate directory not found")
    rs = []

    def walk_error(_exc):
        raise Misuse("source directory cannot be read")

    for base, dirs, files in os.walk(crate, followlinks=False, onerror=walk_error):
        for name in dirs:
            need(not os.path.islink(os.path.join(base, name)), "symbolic link in source tree")
        dirs[:] = sorted(d for d in dirs if not d.startswith(".") and d not in SKIP_DIRS)
        for name in sorted(files):
            full = os.path.join(base, name)
            if name.endswith(SOURCE_EXT):
                need(not os.path.islink(full), "symbolic link in source tree")
                rs.append(os.path.relpath(full, crate).replace(os.sep, "/"))
    need(len(rs) > 0, "no .rs file under crate directory")
    rs.sort()
    return rs, BUILD_FILE in rs


def read_text(crate, rel):
    try:
        with open(os.path.join(crate, rel), "rb") as fh:
            return fh.read().decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        raise Misuse("source is not valid UTF-8")
    except OSError:
        raise Misuse("source cannot be read")


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def scan_env_fs(text):
    """(語, 行) の一覧。"""
    hits = []
    for lit in ENV_FS_LITERALS:
        pos = text.find(lit)
        while pos != -1:
            hits.append((lit, line_of(text, pos)))
            pos = text.find(lit, pos + 1)
    for m in re.finditer(USE_STD_BRACE, text):
        depth = 1
        i = m.end()
        while i < len(text) and depth > 0:
            ch = text[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            i += 1
        need(depth == 0, "unclosed use std brace")
        inner_start = m.end()
        inner = text[inner_start : i - 1]
        for w in re.finditer(USE_STD_WORDS, inner):
            hits.append(("std::{%s}" % w.group(1), line_of(text, inner_start + w.start())))
    return hits


def vcs_out(args):
    env = dict(os.environ)
    env["GIT_OPTIONAL_LOCKS"] = "0"
    try:
        res = subprocess.run(
            ["git"] + args,
            cwd=ROOT,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=REV_TIMEOUT,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if res.returncode != 0:
        return None
    return res.stdout.decode("utf-8", errors="replace").strip()


def tool_env():
    env = dict(os.environ)
    env["RUSTC_BOOTSTRAP"] = "1"
    return env


def run_quiet(args, env=None, timeout=CARGO_TIMEOUT):
    """出力を捨てて終了コードを返す。起動できなければ None。"""
    try:
        res = subprocess.run(args, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=timeout)
    except (OSError, subprocess.SubprocessError):
        return None
    return res.returncode


def build_json_sets():
    """cargo rustdoc で JSON を組ごとに作り、ラベル -> パスの辞書を返す。"""
    need(run_quiet(["cargo", "--version"], timeout=REV_TIMEOUT) == 0, "cannot run cargo")
    try:
        meta = subprocess.run(
            ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
            cwd=ROOT, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            timeout=CARGO_TIMEOUT)
    except (OSError, subprocess.SubprocessError):
        raise Misuse("cargo metadata failed")
    need(meta.returncode == 0, "cargo metadata failed")
    try:
        doc = json.loads(meta.stdout.decode("utf-8"))
        target_dir = doc["target_directory"]
        pkgs = [p for p in doc["packages"] if p["name"] == EXEC_PACKAGE]
        has_lib = any("lib" in t["kind"] for p in pkgs for t in p["targets"])
    except (ValueError, KeyError, TypeError, AttributeError):
        raise Misuse("cargo metadata output is not usable")
    need(isinstance(target_dir, str) and has_lib, "exec package not found in workspace")
    need(run_quiet(["rustdoc", "-Z", "unstable-options", "--version"], env=tool_env(), timeout=REV_TIMEOUT) == 0,
          "rustdoc cannot emit JSON output")
    out_dir = os.path.join(target_dir, TARGET_SUBDIR)
    produced = os.path.join(out_dir, "doc", EXEC_LIB + ".json")
    paths = {}
    for label, feat in JSON_SETS:
        try:
            if os.path.exists(produced):
                os.remove(produced)
        except OSError:
            raise Misuse("cannot clear previous JSON output")
        cmd = (["cargo", "rustdoc", "--locked", "--offline", "--target-dir", out_dir, "-p", EXEC_PACKAGE]
                + list(feat)
                + ["--lib", "--", "-Z", "unstable-options", "--output-format", "json", "--document-hidden-items"])
        rc = run_quiet(cmd, env=tool_env())
        need(rc == 0 and os.path.isfile(produced), "cargo rustdoc failed (%s)" % label)
        dest = os.path.join(out_dir, "doc", "%s.%s.json" % (EXEC_LIB, label))
        try:
            os.replace(produced, dest)
        except OSError:
            raise Misuse("cannot store JSON output (%s)" % label)
        paths[label] = dest
    return paths


def load_doc(path):
    """rustdoc の JSON を読み、形を検査して crate_id 0 のアイテムの一覧を返す。"""
    try:
        with open(path, "rb") as fh:
            doc = json.loads(fh.read().decode("utf-8", errors="strict"))
    except (OSError, ValueError):
        raise Misuse("cannot read JSON input")
    need(isinstance(doc, dict), "JSON top level is not an object")
    for key in ("index", "paths", "external_crates"):
        need(isinstance(doc.get(key), dict), "JSON field %s is not an object" % key)
    need("root" in doc, "JSON has no root")
    need(doc.get("includes_private") is False, "JSON includes private items")
    fv = doc.get("format_version")
    need(isinstance(fv, int) and not isinstance(fv, bool), "JSON format_version is not an integer")
    items = []
    for item in doc["index"].values():
        need(isinstance(item, dict), "JSON index entry is not an object")
        cid = item.get("crate_id")
        need(isinstance(cid, int) and not isinstance(cid, bool), "JSON item has no integer crate_id")
        if cid == 0:
            items.append(item)
    root = doc["index"].get(str(doc["root"]))
    need(isinstance(root, dict) and root.get("crate_id") == 0 and root.get("name") == EXEC_LIB,
          "root item is not %s" % EXEC_LIB)
    return items


def count_port_names(items):
    """名前 -> 件数。use アイテムは inner.use.name も見る（1 アイテムは 1 回だけ数える）。"""
    counts = dict((n, 0) for n in PORT_NAMES)
    for item in items:
        names = [item.get("name")]
        inner = item.get("inner")
        if isinstance(inner, dict) and isinstance(inner.get("use"), dict):
            names.append(inner["use"].get("name"))
        for n in PORT_NAMES:
            if n in names:
                counts[n] += 1
    return counts


def commit_label():
    """HEAD のコミット（変更が在れば -dirty）。取れなければ Misuse。"""
    rev = vcs_out(["rev-parse", "--verify", "HEAD^{commit}"])
    need(rev is not None and re.fullmatch(r"[0-9a-f]{40}([0-9a-f]{24})?", rev) is not None,
          "cannot resolve the source commit")
    status = vcs_out(["status", "--porcelain", "--untracked-files=normal", "--ignore-submodules=all"])
    need(status is not None, "cannot resolve the source commit")
    return rev + ("-dirty" if status else "")


def run(argv):
    parser = argparse.ArgumentParser(prog="exec-surface-check.sh", allow_abbrev=False)
    parser.add_argument("--crate-dir", default=None)
    parser.add_argument("--json-default", action="append", default=[])
    parser.add_argument("--json-all", action="append", default=[])
    args = parser.parse_args(argv)
    need(len(args.json_default) <= 1, "duplicate --json-default")
    need(len(args.json_all) <= 1, "duplicate --json-all")
    need(bool(args.json_default) == bool(args.json_all), "--json-default and --json-all must be given together")
    default_run = args.crate_dir is None
    crate = os.path.join(ROOT, DEFAULT_CRATE_REL) if default_run else args.crate_dir

    rs, has_build = collect_files(crate)
    violations = []
    env_fs_total = 0
    for rel in rs:
        text = read_text(crate, rel)
        print("scanned: file=%s" % rel)
        for word, line in scan_env_fs(text):
            env_fs_total += 1
            violations.append(("env-fs", rel, line, word))

    commit = commit_label() if default_run else "-"
    if args.json_default:
        paths = {"default": args.json_default[0], "all-features": args.json_all[0]}
    else:
        paths = build_json_sets()
    docs = {}
    for label, _feat in JSON_SETS:
        docs[label] = load_doc(paths[label])
        print("scanned: features=%s items=%d" % (label, len(docs[label])))
    cnt_def = count_port_names(docs["default"])
    cnt_all = count_port_names(docs["all-features"])
    default_total = sum(cnt_def.values())
    all_total = sum(cnt_all.values())
    missing = [n for n in PORT_NAMES if cnt_all[n] == 0]
    print("record: commit=%s rs_files=%d build_rs=%s env_fs=%d items_default=%d items_all=%d "
          "port_names_default=%d port_names_all=%d port_names_missing_all=%d"
          % (commit, len(rs), "yes" if has_build else "no", env_fs_total,
              len(docs["default"]), len(docs["all-features"]), default_total, all_total, len(missing)))
    for check, rel, line, word in violations:
        print("violation: check=%s file=%s line=%d word=%s" % (check, rel, line, word), file=sys.stderr)
    total = len(violations)
    for n in PORT_NAMES:
        if cnt_def[n] > 0:
            total += 1
            print("violation: check=port-name features=default name=%s count=%d" % (n, cnt_def[n]), file=sys.stderr)
    for n in missing:
        total += 1
        print("violation: check=port-name-positive features=all-features name=%s" % n, file=sys.stderr)
    if total:
        print("FAILED: %d violation(s)" % total, file=sys.stderr)
        return 1
    print("ok: exec surface check passed")
    return 0


ROOT = sys.argv[1]
try:
    code = run(sys.argv[2:])
except Misuse as exc:
    print("error: %s" % exc, file=sys.stderr)
    code = 2
except SystemExit:
    raise
except Exception:
    print("error: unexpected failure", file=sys.stderr)
    code = 2
sys.exit(code)
PY
