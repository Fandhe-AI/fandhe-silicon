#!/usr/bin/env bash
# 下-2 の crate（crates/exec）のソースの環境変数とファイルの読み取りの字句を数える検査（spec TASK-923 のポインタ。親 Issue #867）。
# 呼び出し元: `make exec-surface-check`、回帰テスト `scripts/test-exec-surface-check.sh`。
#
# 使い方:
#   scripts/exec-surface-check.sh                      既定の crates/exec を走査する
#   scripts/exec-surface-check.sh --crate-dir <dir>    <dir> の crate を走査する（回帰テストが一時ディレクトリへ写した crate を渡す）
# 終了コード: 0 期待どおり（違反 0 件）/ 1 違反あり /
#   2 誤用・前提の不足・読み切れない入力（python3 が無い、未知の引数、crate ディレクトリが無い、.rs が 0 件、
#     UTF-8 でない、シンボリックリンク、ディレクトリやファイルが読めない、`use std::{` の対応する `}` が無い、
#     既定の走査でリビジョンを読めない、想定外の例外）
# 標準出力: 読んだファイルごとに `scanned: file=<crate からの相対パス>`、数え終えたとき（終了コード 0 と 1）だけ
#   `record: commit=<rev|-> rs_files=<n> build_rs=<yes|no> env_fs=<n>`、合格時の `ok:` の 1 行。
#   rs_files は build.rs を含む見つけた .rs の数。commit は既定の crate を走査したときだけ HEAD
#   （変更が在れば末尾に `-dirty`）、--crate-dir のときは `-`。
# 標準エラー: 違反ごとに `violation: check=env-fs file=<相対パス> line=<n> word=<字句>`、
#   違反があれば `FAILED: <n> violation(s)`、前提の不足は `error:`。
#
# 数え方: 対象のファイル、字句、期待は、Python 部の先頭の「数え方の定義」ブロックにだけ置く。
# 検査しないもの: 構文の解釈（マクロが生成するパス、別名の取り込み）、target/ と `.` 始まりのディレクトリ、.rs 以外のファイル。
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
    args = parser.parse_args(argv)
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
    print("record: commit=%s rs_files=%d build_rs=%s env_fs=%d"
          % (commit, len(rs), "yes" if has_build else "no", env_fs_total))
    for check, rel, line, word in violations:
        print("violation: check=%s file=%s line=%d word=%s" % (check, rel, line, word), file=sys.stderr)
    if violations:
        print("FAILED: %d violation(s)" % len(violations), file=sys.stderr)
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
