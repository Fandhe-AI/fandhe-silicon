#!/usr/bin/env bash
# 上の層の crate（crates/upper）のソースを字句で数える検査（spec TASK-2387 のポインタ。親 Issue #1059 の 2/4）。
# 入れている走査は 4 つ。
#   (1) グローバルな状態: `static`・`thread_local!`・`Mutex` の出現。
#   (2) 環境変数とファイルの読み取り: 6 つの字句と、`use std::{…}` の中の `env`・`fs` の出現。
#   (3) 契約の呼び出し: 関数の表の各関数の本体での、core の `CommandQueue` の操作の呼び出しの数。
#   (4) 失敗の構築: 同じ本体での `Err(`・panic 系のマクロ・`.unwrap(`・`.expect(` の出現。
#   (3)(4) はコメント・文字列・文字のリテラルを空白にした写しの上で数える。
# 呼び出し元: `make upper-source-check`、回帰テスト `scripts/test-upper-source-check.sh`。
#
# 使い方:
#   scripts/upper-source-check.sh                      既定の crates/upper を走査する
#   scripts/upper-source-check.sh --crate-dir <dir>    <dir> の crate を走査する（回帰テストが一時ディレクトリへ写した crate を渡す）
#   scripts/upper-source-check.sh --queue-trait-file <path>   契約の操作の名前を読む trait のファイルを差し替える
# 終了コード: 0 期待どおり（違反 0 件）/ 1 違反あり /
#   2 誤用・前提の不足・読み切れない入力（python3 が無い、未知の引数、crate ディレクトリや src/ が無い、
#     src/ の .rs が 0 件、UTF-8 でない、シンボリックリンク、`use std::{` の対応する `}` が無い、
#     閉じていないコメント・文字列・文字のリテラル、関数の表の関数が 0 件・2 件以上・本体が無い・閉じない、
#     trait のファイルが読めない・trait が無い・操作の名前が 0 件・関数の表の操作が trait に無い、想定外の例外）
# 標準出力: 読んだファイルごとに `scanned: file=<crate からの相対パス>`、数え終えたとき（終了コード 0 と 1）だけ
#   `record: commit=<rev|-> rs_files=<n> build_rs=<yes|no> global_state=<n> env_fs=<n> table_fns=<n> contract_ops=<n>
#   contract_calls=<n> contract_call_mismatch=<n> dispatch_ex=<n> failure_constructs=<n>`、合格時の `ok:` の 1 行。
#   rs_files は build.rs を除く src/ 配下の .rs の数。commit は既定の crate を走査したときだけ HEAD
#   （変更が在れば末尾に `-dirty`）、リビジョンを読めないときと --crate-dir・--queue-trait-file のときは `-`。
# 標準エラー: 違反ごとに `violation: check=<global-state|env-fs> file=<相対パス> line=<n> word=<字句>`、
#   `violation: check=contract-call fn=<関数> op=<名前> expected=<n> found=<n>`、
#   `violation: check=failure-construct fn=<関数> line=<n> word=<字句>`、
#   違反があれば `FAILED: <n> violation(s)`、前提の不足は `error:`。
#
# 数え方: 対象のファイル、字句、期待は、Python 部の先頭の「数え方の定義」ブロックにだけ置く。
# 検査しないもの: 構文の解釈（マクロが生成するパス、別名の取り込み）、tests/・benches/・examples/・target/・
#   `.` 始まりのディレクトリ、`src/` と `build.rs` の外のファイル、関数の impl の型、関数の表に無い関数、
#   `build.rs` への (3)(4)。
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
# (3) 契約の呼び出し（UPPER-1）: 関数の表の各関数の本体で、契約の trait の操作の呼び出しを数える。
# 操作の名前は trait のファイルから実行時に読む（trait の本体の直下の `fn` の名前の集合）。
DEFAULT_QUEUE_TRAIT_REL = "crates/core/src/execution/submit.rs"
QUEUE_TRAIT = "CommandQueue"
# 関数の表: 関数の名前 -> {操作の名前: 期待する呼び出しの数}。書いていない操作の期待は 0。行を足すだけで関数を足せる。
FUNCTION_TABLE = {
    "create_command_encoder": {"create_encoder": 1},
    "submit": {"submit": 1},
    "begin_compute_pass": {},
    "finish": {},
}
# 呼び出しの形: `.` または `::` の直後（空白・改行は任意）に名前、続けて `(`（空白は任意）。
CALL_PREFIX = r"(?:\.|::)\s*"
# record の dispatch_ex 欄に使う名前。
DISPATCH_EX = "dispatch_ex"
# (4) 失敗の構築と panic（UPPER-2）: 関数の表の各関数の本体の写しで、識別子としての字句を数える。期待は 0 件。
FAILURE_PATTERNS = (
    ("Err(", r"(?<![%s])Err\s*\(" % IDENT),
    ("panic!", r"(?<![%s])panic\s*!" % IDENT),
    ("unreachable!", r"(?<![%s])unreachable\s*!" % IDENT),
    ("todo!", r"(?<![%s])todo\s*!" % IDENT),
    ("unimplemented!", r"(?<![%s])unimplemented\s*!" % IDENT),
    ("assert!", r"(?<![%s])assert\s*!" % IDENT),
    ("assert_eq!", r"(?<![%s])assert_eq\s*!" % IDENT),
    ("assert_ne!", r"(?<![%s])assert_ne\s*!" % IDENT),
    ("debug_assert!", r"(?<![%s])debug_assert\s*!" % IDENT),
    ("debug_assert_eq!", r"(?<![%s])debug_assert_eq\s*!" % IDENT),
    ("debug_assert_ne!", r"(?<![%s])debug_assert_ne\s*!" % IDENT),
    (".unwrap(", r"\.\s*unwrap\s*\("),
    (".expect(", r"\.\s*expect\s*\("),
)
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

    def walk_error(_exc):
        raise Misuse("source directory cannot be read")

    for base, dirs, files in os.walk(src, followlinks=False, onerror=walk_error):
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


def read_abs(path):
    try:
        with open(path, "rb") as fh:
            return fh.read().decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        raise Misuse("queue trait file is not valid UTF-8")
    except OSError:
        raise Misuse("queue trait file cannot be read")


def char_literal_end(text, i):
    """text[i] が `'`。文字のリテラルなら閉じの次の位置、寿命なら None。"""
    n = len(text)
    if i + 1 < n and text[i + 1] == "\\":
        j = text.find("'", i + 3)
        need(j != -1, "unclosed comment or literal")
        return j + 1
    if i + 2 < n and text[i + 2] == "'" and text[i + 1] != "'":
        return i + 3
    return None


def mask(text):
    """コメント・文字列・文字のリテラルを同じ長さの空白にした写し（改行は残す）。寿命の `'` は残す。"""
    n = len(text)
    out = list(text)
    ident = re.compile("[%s]" % IDENT)
    raw_re = re.compile(r'(?:br|cr|r)(#*)"')
    plain_re = re.compile(r'(?:b|c)"')

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    def end_plain_string(j):
        # j は開きの `"` の次。閉じの `"` の次の位置を返す。
        while j < n:
            if text[j] == "\\":
                j += 2
            elif text[j] == '"':
                return j + 1
            else:
                j += 1
        raise Misuse("unclosed comment or literal")

    i = 0
    while i < n:
        c = text[i]
        prev_ident = i > 0 and ident.match(text[i - 1]) is not None
        if c == "/" and text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j == -1 else j
            blank(i, j)
            i = j
        elif c == "/" and text.startswith("/*", i):
            depth = 1
            j = i + 2
            while j < n and depth > 0:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            need(depth == 0, "unclosed comment or literal")
            blank(i, j)
            i = j
        elif c == '"':
            j = end_plain_string(i + 1)
            blank(i, j)
            i = j
        elif c in "brc" and not prev_ident:
            m = raw_re.match(text, i)
            m2 = plain_re.match(text, i)
            if m:
                close = '"' + m.group(1)
                j = text.find(close, m.end())
                need(j != -1, "unclosed comment or literal")
                j += len(close)
                blank(i, j)
                i = j
            elif m2:
                j = end_plain_string(m2.end())
                blank(i, j)
                i = j
            elif c == "b" and text.startswith("b'", i):
                j = char_literal_end(text, i + 1)
                if j is None:
                    i += 1
                else:
                    blank(i, j)
                    i = j
            else:
                i += 1
        elif c == "'":
            j = char_literal_end(text, i)
            if j is None:
                i += 1
            else:
                blank(i, j)
                i = j
        else:
            i += 1
    return "".join(out)


def match_brace(masked, open_idx, what):
    """masked[open_idx] は `{`。対応する `}` の位置を返す。"""
    depth = 0
    for k in range(open_idx, len(masked)):
        ch = masked[k]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return k
    raise Misuse("%s is not closed" % what)


def find_fn_bodies(masked_files, names):
    """表の各関数の (ファイル, 写し, 本体の開始位置, 終了位置)。ちょうど 1 件でなければ Misuse。"""
    found = {}
    for name in names:
        pat = re.compile(r"(?<![%s])fn\s+%s(?![%s])" % (IDENT, re.escape(name), IDENT))
        hits = []
        for rel, m in masked_files:
            for h in pat.finditer(m):
                hits.append((rel, m, h.end()))
        need(len(hits) == 1, "function %s must be found exactly once (found %d)" % (name, len(hits)))
        rel, m, pos = hits[0]
        depth = 0
        start = None
        for k in range(pos, len(m)):
            ch = m[k]
            if ch in "([":
                depth += 1
            elif ch in ")]":
                depth -= 1
            elif ch == ";" and depth <= 0:
                raise Misuse("function %s has no body" % name)
            elif ch == "{" and depth <= 0:
                start = k
                break
        need(start is not None, "function %s has no body" % name)
        end = match_brace(m, start, "function %s body" % name)
        found[name] = (rel, m, start, end)
    return found


def queue_op_names(path):
    """trait の本体の直下の `fn` の名前の集合（ソート済みの一覧）。"""
    need(not os.path.islink(path), "queue trait file is a symbolic link")
    need(os.path.isfile(path), "queue trait file not found")
    m = mask(read_abs(path))
    heads = list(re.finditer(r"(?<![%s])trait\s+%s(?![%s])" % (IDENT, QUEUE_TRAIT, IDENT), m))
    need(len(heads) == 1, "queue trait must be found exactly once")
    brace = m.find("{", heads[0].end())
    need(brace != -1, "queue trait has no body")
    end = match_brace(m, brace, "queue trait body")
    top = []
    depth = 0
    for k in range(brace + 1, end):
        ch = m[k]
        if ch == "{":
            depth += 1
            top.append(" ")
        elif ch == "}":
            depth -= 1
            top.append(" ")
        else:
            top.append(ch if depth == 0 or ch == "\n" else " ")
    names = sorted(set(re.findall(r"(?<![%s])fn\s+([A-Za-z_][%s]*)" % (IDENT, IDENT), "".join(top))))
    need(len(names) > 0, "no operation found in queue trait")
    return names


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
    parser.add_argument("--queue-trait-file", default=None)
    args = parser.parse_args(argv)
    default_run = args.crate_dir is None and args.queue_trait_file is None
    trait_path = os.path.join(ROOT, DEFAULT_QUEUE_TRAIT_REL) if args.queue_trait_file is None else args.queue_trait_file
    crate = os.path.join(ROOT, DEFAULT_CRATE_REL) if args.crate_dir is None else args.crate_dir

    rs, has_build = collect_files(crate)
    files = list(rs) + ([BUILD_FILE] if has_build else [])
    violations = []
    global_total = 0
    env_fs_total = 0
    texts = {}
    for rel in files:
        text = read_text(crate, rel)
        texts[rel] = text
        print("scanned: file=%s" % rel)
        for word, line in scan_global_state(text):
            global_total += 1
            violations.append(("global-state", rel, line, word))
        for word, line in scan_env_fs(text):
            env_fs_total += 1
            violations.append(("env-fs", rel, line, word))

    # (3)(4): build.rs は対象にしない。マスクの失敗・表の関数・trait の問題はここで終了コード 2 になる。
    masked_files = [(rel, mask(texts[rel])) for rel in rs]
    bodies = find_fn_bodies(masked_files, list(FUNCTION_TABLE))
    ops = queue_op_names(trait_path)
    for expect in FUNCTION_TABLE.values():
        for op in expect:
            need(op in ops, "table operation %s is not in the queue trait" % op)
    contract_calls = 0
    mismatch = 0
    dispatch_ex = 0
    failure_total = 0
    for fn, (rel, m, start, end) in bodies.items():
        body = m[start : end + 1]
        dispatch_ex += len(re.findall(CALL_PREFIX + re.escape(DISPATCH_EX) + r"\s*\(", body))
        for op in ops:
            found = len(re.findall(CALL_PREFIX + re.escape(op) + r"\s*\(", body))
            contract_calls += found
            want = FUNCTION_TABLE[fn].get(op, 0)
            if found != want:
                mismatch += 1
                violations.append(("contract-call", None, None, "fn=%s op=%s expected=%d found=%d" % (fn, op, want, found)))
        for word, pat in FAILURE_PATTERNS:
            for h in re.finditer(pat, body):
                failure_total += 1
                violations.append(
                    ("failure-construct", None, None, "fn=%s line=%d word=%s" % (fn, line_of(m, start + h.start()), word))
                )

    commit = commit_label() if default_run else "-"
    print(
        "record: commit=%s rs_files=%d build_rs=%s global_state=%d env_fs=%d table_fns=%d contract_ops=%d"
        " contract_calls=%d contract_call_mismatch=%d dispatch_ex=%d failure_constructs=%d"
        % (commit, len(rs), "yes" if has_build else "no", global_total, env_fs_total, len(FUNCTION_TABLE), len(ops),
            contract_calls, mismatch, dispatch_ex, failure_total)
    )
    for check, rel, line, word in violations:
        if rel is None:
            print("violation: check=%s %s" % (check, word), file=sys.stderr)
        else:
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
