#!/usr/bin/env bash
# 上の層の crate（crates/upper）のソースを字句で数える検査（spec TASK-2387 のポインタ。親 Issue #1059 の 1/4）。
# 入れている走査は 2 つ。
#   (1) グローバルな状態: `static`・`thread_local!`・`Mutex` の出現。
#   (2) 環境変数とファイルの読み取り: 6 つの字句と、`use std::{…}` の中の `env`・`fs` の出現。
# 呼び出し元: `make upper-source-check`、回帰テスト `scripts/test-upper-source-check.sh`。
#
# 使い方:
#   scripts/upper-source-check.sh                      既定の crates/upper を走査する
#   scripts/upper-source-check.sh --crate-dir <dir>    <dir> の crate を走査する（回帰テストが一時ディレクトリへ写した crate を渡す）
# 終了コード: 0 期待どおり（違反 0 件）/ 1 違反あり /
#   2 誤用・前提の不足・読み切れない入力（python3 が無い、未知の引数、crate ディレクトリや src/ が無い、
#     src/ の .rs が 0 件、UTF-8 でない、シンボリックリンク、`use std::{` の対応する `}` が無い、想定外の例外）
# 標準出力: 読んだファイルごとに `scanned: file=<crate からの相対パス>`、数え終えたとき（終了コード 0 と 1）だけ
#   `record: commit=<rev|-> rs_files=<n> build_rs=<yes|no> global_state=<n> env_fs=<n>`、合格時の `ok:` の 1 行。
#   rs_files は build.rs を除く src/ 配下の .rs の数。commit は既定の crate を走査したときだけ HEAD
#   （変更が在れば末尾に `-dirty`）、リビジョンを読めないときと --crate-dir のときは `-`。
# 標準エラー: 違反ごとに `violation: check=<global-state|env-fs> file=<相対パス> line=<n> word=<字句>`、
#   違反があれば `FAILED: <n> violation(s)`、前提の不足は `error:`。
#
# 数え方: 対象のファイル、字句、期待は、Python 部の先頭の「数え方の定義」ブロックにだけ置く。
# 検査しないもの: 構文の解釈（マクロが生成するパス、別名の取り込み）、tests/・benches/・examples/・target/・
#   `.` 始まりのディレクトリ、`src/` と `build.rs` の外のファイル。
# cargo を呼ばない。ホストの絶対パス・環境変数の値は出力に出さない。ソースを変更しない。ネットワークを使わない。
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
DEFAULT_CRATE_REL = "crates/upper"
# 走査するソース: crate 直下の src/ 以下のすべての .rs と、crate 直下の build.rs。
SRC_DIR = "src"
BUILD_FILE = "build.rs"
SOURCE_EXT = ".rs"
# src/ の中で辿らないディレクトリ名（`.` 始まりも辿らない）。
SKIP_DIRS = ("target",)
# 識別子の一部になる文字（前後がこれでないものを識別子としての一致とする）。
IDENT = "A-Za-z0-9_"
# 数えるのは各ファイルの生のテキスト（コメント・文字列を除かない）。期待はどちらも 0 件。
# (1) グローバルな状態（UPPER-32）: 識別子としての static / Mutex と、thread_local に続く `!`。
GLOBAL_STATE_PATTERNS = (
    ("static", r"(?<![%s])static(?![%s])" % (IDENT, IDENT)),
    ("thread_local!", r"(?<![%s])thread_local\s*!" % IDENT),
    ("Mutex", r"(?<![%s])Mutex(?![%s])" % (IDENT, IDENT)),
)
# (2) 環境変数とファイルの読み取り（UPPER-40）: 次の字句を部分文字列としてそれぞれ独立に数える（重なりは引かない）。
ENV_FS_LITERALS = ("std::env", "env::var", "option_env!", "env!", "std::fs", "File::open")
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
    """走査するファイルの crate からの相対パスの一覧（ソート済み）と、build.rs の有無を返す。"""
    need(os.path.isdir(crate) and not os.path.islink(crate), "crate directory not found")
    src = os.path.join(crate, SRC_DIR)
    need(not os.path.islink(src), "symbolic link in source tree")
    need(os.path.isdir(src), "src directory not found")
    rs = []
    for base, dirs, files in os.walk(src, followlinks=False):
        for name in dirs:
            need(not os.path.islink(os.path.join(base, name)), "symbolic link in source tree")
        dirs[:] = sorted(d for d in dirs if not d.startswith(".") and d not in SKIP_DIRS)
        for name in sorted(files):
            full = os.path.join(base, name)
            need(not os.path.islink(full), "symbolic link in source tree")
            if name.endswith(SOURCE_EXT):
                rs.append(os.path.relpath(full, crate))
    need(len(rs) > 0, "no .rs file under src")
    rs.sort()
    build = os.path.join(crate, BUILD_FILE)
    need(not os.path.islink(build), "symbolic link in source tree")
    has_build = os.path.isfile(build)
    return rs, has_build


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


def scan_global_state(text):
    """(語, 行) の一覧。"""
    hits = []
    for word, pat in GLOBAL_STATE_PATTERNS:
        for m in re.finditer(pat, text):
            hits.append((word, line_of(text, m.start())))
    return hits


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
    rev = vcs_out(["rev-parse", "--verify", "HEAD^{commit}"])
    if not rev or not re.fullmatch(r"[0-9a-f]{7,64}", rev):
        return "-"
    status = vcs_out(["status", "--porcelain"])
    if status is None:
        return "-"
    return rev[:12] + ("-dirty" if status else "")


def run(argv):
    parser = argparse.ArgumentParser(prog="upper-source-check.sh", allow_abbrev=False)
    parser.add_argument("--crate-dir", default=None)
    args = parser.parse_args(argv)
    default_run = args.crate_dir is None
    crate = os.path.join(ROOT, DEFAULT_CRATE_REL) if default_run else args.crate_dir

    rs, has_build = collect_files(crate)
    files = list(rs) + ([BUILD_FILE] if has_build else [])
    violations = []
    global_total = 0
    env_fs_total = 0
    for rel in files:
        text = read_text(crate, rel)
        print("scanned: file=%s" % rel)
        for word, line in scan_global_state(text):
            global_total += 1
            violations.append(("global-state", rel, line, word))
        for word, line in scan_env_fs(text):
            env_fs_total += 1
            violations.append(("env-fs", rel, line, word))

    commit = commit_label() if default_run else "-"
    print(
        "record: commit=%s rs_files=%d build_rs=%s global_state=%d env_fs=%d"
        % (commit, len(rs), "yes" if has_build else "no", global_total, env_fs_total)
    )
    for check, rel, line, word in violations:
        print("violation: check=%s file=%s line=%d word=%s" % (check, rel, line, word), file=sys.stderr)
    if violations:
        print("FAILED: %d violation(s)" % len(violations), file=sys.stderr)
        return 1
    print("ok: upper source scan passed")
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
