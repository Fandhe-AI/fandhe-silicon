#!/usr/bin/env bash
# CI の定義（.github/workflows/ci.yml）が、デバイスを使わない検査の枠の事実を保っているかを確かめる
# （spec TASK-112・TASK-18 のポインタ。親 Issue #393 の 3/4）。
# 呼び出し元: `make ci-frame-check`、CI の `deny` ジョブ、回帰テスト `scripts/test-ci-frame-check.sh`。
#
# 使い方: scripts/ci-frame-check.sh [--workflow <path>]
#   既定の入力は <repo>/.github/workflows/ci.yml。--workflow は回帰テストが合成 fixture を渡すために使う。
# 終了コード: 0 合格 / 1 違反 / 2 誤用・前提の不足（未知の引数・パスが無い・python3 が無い。成功扱いにしない）
#
# 検査する規則（許可ラベルの定数はこのファイルの 1 か所だけ）:
#   F1 集約ジョブ ci-complete があり、ジョブ単位の if が always() である
#   F2 ci-complete の needs が、ci-complete 以外の全ジョブ ID と過不足なく一致する
#   F3 各ジョブの runs-on が許可ラベル（GitHub ホステッドのみ）か、リテラルの並びの matrix 参照である
#   F4 continue-on-error のキーがどこにも無い（値は問わない）
#   F5 コメント行以外に「|| true」「|| :」の綴りが無い
#   F6 always() が F1 の 1 か所以外に現れない
# 読めない形（タブのインデント・アンカー・マージキー・対応外の needs / runs-on / matrix の書き方・
# ジョブ ID の重複・jobs の欠落）は違反にする（fail-closed）。YAML の汎用パーサではなく、
# 行とインデントだけを読む。行の途中の # 以降は読み飛ばさない（行中コメントの綴りも違反になる）。
#
# 検査しないもの: ci-complete の判定式の中身、ci.yml 以外の workflow、ruleset の登録、
# ステップ単位の if、F5 の綴り以外の握りつぶし（set +e など）。
# 入力を読むだけで、ファイルを作らず、ネットワークを使わない。Python 3.8 以上の標準ライブラリだけ。
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
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# 実行場所として許す runs-on のラベル（GitHub ホステッド）。
ALLOWED_LABELS = ("ubuntu-latest", "macos-latest")
AGGREGATE = "ci-complete"

root = sys.argv[1]
parser = argparse.ArgumentParser(prog="ci-frame-check.sh")
parser.add_argument("--workflow", default=os.path.join(root, ".github", "workflows", "ci.yml"))
args = parser.parse_args(sys.argv[2:])

path = args.workflow
if not os.path.isfile(path):
    print("error: workflow file not found or not a regular file", file=sys.stderr)
    sys.exit(2)
try:
    with open(path, encoding="utf-8") as f:
        raw = f.read()
except (OSError, UnicodeDecodeError):
    print("error: cannot read workflow file as UTF-8", file=sys.stderr)
    sys.exit(2)

violations = []  # (line, rule, job, message)


def viol(line, rule, job, msg):
    violations.append((line, rule, job or "-", msg))


lines = [(i + 1, l.rstrip("\r")) for i, l in enumerate(raw.split("\n"))]
if lines and lines[-1][1] == "":
    lines.pop()

# 意味のある行: (行番号, インデント, 本文)。空行・コメント行は除く。タブのインデントは違反。
body = []
for no, text in lines:
    stripped = text.strip()
    if not stripped or stripped.startswith("#"):
        continue
    lead = text[: len(text) - len(text.lstrip())]
    if "\t" in lead:
        viol(no, "READ", None, "tab in indentation")
        continue
    body.append((no, len(lead), text.strip()))

# F4・F5・F6 と読めない形の検出は、ジョブの帰属によらず全行に当てる。
F5_RE = re.compile(r"\|\|\s*(true|:)(?![\w-])")
F4_RE = re.compile(r"^(-\s+)?continue-on-error\s*:")
MERGE_RE = re.compile(r"^(-\s+)?<<\s*:")
ANCHOR_RE = re.compile(r":\s+[&*][A-Za-z0-9_-]+\s*$")
always_lines = []
for no, ind, t in body:
    if F4_RE.search(t):
        viol(no, "F4", None, "continue-on-error key")
    if F5_RE.search(t):
        viol(no, "F5", None, "failure swallowed with '|| true' or '|| :'")
    if MERGE_RE.search(t) or ANCHOR_RE.search(t):
        viol(no, "READ", None, "yaml anchor/alias/merge key is not supported")
    if "always()" in t:
        always_lines.append((no, ind, t))

# jobs: の範囲をジョブごとに切り出す。
jobs = {}  # id -> {"line": n, "rows": [(no, ind, text)]}
order = []
in_jobs = False
seen_jobs_key = False
cur = None
JOBS_RE = re.compile(r"^jobs:\s*(#.*)?$")
JOBID_RE = re.compile(r"^([A-Za-z0-9_-]+):\s*(#.*)?$")
for no, ind, t in body:
    if ind == 0:
        in_jobs = False
        cur = None
        if JOBS_RE.match(t):
            seen_jobs_key = True
            in_jobs = True
        continue
    if not in_jobs:
        continue
    if ind == 2:
        m = JOBID_RE.match(t)
        if not m:
            viol(no, "READ", None, "unreadable job id line")
            cur = None
            continue
        jid = m.group(1)
        if jid in jobs:
            viol(no, "READ", jid, "duplicate job id")
            cur = None
            continue
        jobs[jid] = {"line": no, "rows": []}
        order.append(jid)
        cur = jid
        continue
    if cur is not None:
        jobs[cur]["rows"].append((no, ind, t))

if not seen_jobs_key:
    viol(1, "READ", None, "no top-level 'jobs:' key")


def strip_comment(v):
    return re.sub(r"\s+#.*$", "", v).strip()


def unquote(v):
    v = v.strip()
    if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
        return v[1:-1]
    return v


NAME_RE = re.compile(r"^[A-Za-z0-9_.-]+$")


def parse_list(rows, idx, key_indent, value):
    """スカラー・フロー形式・ブロック形式の 3 つだけを読む。読めなければ None。"""
    value = strip_comment(value)
    if value.startswith("["):
        if not value.endswith("]"):
            return None
        inner = value[1:-1].strip()
        items = [unquote(x) for x in inner.split(",")] if inner else []
        return items if all(NAME_RE.match(x) for x in items) else None
    if value:
        v = unquote(value)
        return [v] if NAME_RE.match(v) else None
    items = []
    j = idx + 1
    while j < len(rows) and rows[j][1] > key_indent:
        m = re.match(r"^-\s+(\S+?)\s*(\s#.*)?$", rows[j][2])
        if not m:
            return None
        v = unquote(m.group(1))
        if not NAME_RE.match(v):
            return None
        items.append(v)
        j += 1
    return items


def job_keys(rows):
    out = {}
    for i, (no, ind, t) in enumerate(rows):
        if ind != 4:
            continue
        m = re.match(r"^([A-Za-z0-9_-]+):\s*(.*)$", t)
        if m and m.group(1) not in out:
            out[m.group(1)] = (i, no, m.group(2))
    return out


def matrix_values(rows, key):
    """strategy.matrix.<key> のリテラルの並びを返す。読めなければ None。"""
    start = None
    for i, (no, ind, t) in enumerate(rows):
        if ind == 6 and re.match(r"^matrix:\s*(#.*)?$", t):
            start = i
            break
    if start is None:
        return None
    j = start + 1
    while j < len(rows) and rows[j][1] > 6:
        no, ind, t = rows[j]
        m = re.match(r"^" + re.escape(key) + r":\s*(.*)$", t)
        if ind == 8 and m:
            return parse_list(rows, j, 8, m.group(1))
        j += 1
    return None


ids = list(order)
agg_if_line = None
for jid in ids:
    rows = jobs[jid]["rows"]
    keys = job_keys(rows)
    # F3 実行場所
    if "runs-on" not in keys:
        viol(jobs[jid]["line"], "F3", jid, "no runs-on")
    else:
        i, no, val = keys["runs-on"]
        v = unquote(strip_comment(val))
        m = re.match(r"^\$\{\{\s*matrix\.([A-Za-z0-9_-]+)\s*\}\}$", v)
        if v in ALLOWED_LABELS:
            pass
        elif m:
            vals = matrix_values(rows, m.group(1))
            if vals is None:
                viol(no, "F3", jid, "matrix for runs-on is not a literal list")
            else:
                bad = [x for x in vals if x not in ALLOWED_LABELS]
                if not vals or bad:
                    viol(no, "F3", jid, "matrix runs-on has a disallowed or empty label")
        else:
            viol(no, "F3", jid, "runs-on is not an allowed label")

if AGGREGATE not in jobs:
    viol(jobs[ids[0]]["line"] if ids else 1, "F1", AGGREGATE, "aggregate job is missing")
else:
    rows = jobs[AGGREGATE]["rows"]
    keys = job_keys(rows)
    # F1 ジョブ単位の if
    if "if" not in keys:
        viol(jobs[AGGREGATE]["line"], "F1", AGGREGATE, "job-level if is missing")
    else:
        i, no, val = keys["if"]
        agg_if_line = no
        if strip_comment(val) != "always()":
            viol(no, "F1", AGGREGATE, "job-level if is not exactly always()")
    # F2 needs の網羅
    if "needs" not in keys:
        viol(jobs[AGGREGATE]["line"], "F2", AGGREGATE, "needs is missing")
    else:
        i, no, val = keys["needs"]
        got = parse_list(rows, i, 4, val)
        if got is None:
            viol(no, "F2", AGGREGATE, "needs is not scalar, flow list or block list")
        else:
            want = set(x for x in ids if x != AGGREGATE)
            have = set(got)
            for x in sorted(want - have):
                viol(no, "F2", AGGREGATE, "needs lacks job: " + x)
            for x in sorted(have - want):
                viol(no, "F2", AGGREGATE, "needs names unknown job: " + x)

# F6 always() の場所
for no, ind, t in always_lines:
    if no == agg_if_line:
        continue
    viol(no, "F6", None, "always() outside the aggregate job-level if")

if violations:
    for no, rule, job, msg in sorted(violations):
        print("violation: %s line %d job %s: %s" % (rule, no, job, msg), file=sys.stderr)
    print("FAILED: %d violation(s)" % len(violations), file=sys.stderr)
    sys.exit(1)

print("ok: ci workflow frame (%d jobs, rules F1-F6)" % len(ids))
PY
