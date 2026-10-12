#!/usr/bin/env bash
# 試験の枠（spec TASK-23 のポインタ）が書き出した記録の部品のディレクトリから、
# 機材の組み合わせ × ラベルの件数の表（Markdown）を作って標準出力に出す。
# 呼び出し元: `scripts/verify.sh`（`make verify` の試験の後。CI の verify ジョブのログに出る）と、
# 回帰テスト `scripts/test-record-summary.sh`。引数（読むディレクトリ）が要るので make ターゲットを持たない。
#
# 使い方: scripts/record-summary.sh <部品のディレクトリ>
# 終了コード: 0 集計を出した / 1 違反（読めない・形が合わない・重複・部品が 0 件。表は出さない）
#             2 誤用・前提の不足（引数・ディレクトリが無い・python3 が無い。成功扱いにしない）
# 未実施・判定保留・NG・判定不能が何件あっても、部品が形どおりなら終了コードは 0（NG で失敗にするのは試験の側）。
# 部品が 0 件のディレクトリは違反にする（試験が何も書かなかったことを成功に見せないため）。
#
# 入力は crates/core/src/test_frame/record.rs が作る 1 ファイル 1 行の JSON オブジェクト。
# 欄・綴りの定義はこのファイルの定数表が唯一の正で、Rust 側の綴りとの一致は回帰テストが確かめる。
# 表の形は crates/core/src/test_frame/summary.rs の `Summary` の `Display` と同じ（行は末尾に改行を足す）。
# 違反の表示は種類だけで、入力の値（試験の ID・ファイル名・未知の欄の名前）は出さない。表も件数だけ。
# ファイルを作らず、ネットワークを使わず、入力を変更しない。
# 実行環境: macOS の bash 3.2 と同梱の python3（3.8 以上の標準ライブラリだけ）でも動く。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "$@" <<'PY'
import json
import os
import re
import stat
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# 1 ファイルの上限（部品は 1 KiB に満たない）と、ディレクトリの項目数の上限。
MAX_BYTES = 64 * 1024
MAX_ENTRIES = 10000

# 綴りは crates/core/src/test_frame/combination.rs の MachineCombination::name / Machine::name に揃える。
# 順は MachineCombination::ALL の順（表の行の順）。値は機材の呼び名。
COMBINATIONS = (
    ("metal-mac-mini", "mac-mini"),
    ("vulkan-node-b", "node-b"),
    ("vulkan-node-a", "node-a"),
    ("vulkan-amd", "amd"),
    ("vulkan-intel", "intel"),
    ("cuda-node-b", "node-b"),
    ("cuda-node-a", "node-a"),
)
# 綴りは label.rs の LabelKind::name と、理由のトークン（label.rs・decision.rs）に揃える。
# 順は LabelKind::ALL の順（表の列の順）。値は理由のトークン（None は理由を持たないラベル）。
LABELS = (
    ("ok", None),
    ("ng", None),
    ("pending", ("threshold-unapproved",)),
    (
        "not-run",
        (
            "not-configured",
            "invalid-combination-setting",
            "invalid-permission-setting",
            "combination-not-declared",
            "permission-missing",
        ),
    ),
    ("indeterminate", ("device-mismatch", "machine-unreachable")),
)
# record.rs の BODY_KIND。
BODY_KINDS = ("contract-test",)

# 識別子の綴りは identifier.rs・threshold.rs の検査に揃える。
TEST_ID_RE = re.compile(r"[a-z0-9_-]{1,128}", re.ASCII)
BEHAVIOR_ID_RE = re.compile(r"[A-Z0-9-]{1,32}", re.ASCII)
THRESHOLD_ID_RE = re.compile(r"t-[1-9][0-9]{0,2}", re.ASCII)
MAX_BEHAVIORS = 16

REQUIRED = ("behavior_ids", "machine_combination", "machine", "label", "body_kind", "body")
OPTIONAL = ("label_reason",)


class Dup(Exception):
    pass


class Const(Exception):
    pass


def pairs_hook(pairs):
    seen = set()
    for k, _ in pairs:
        if k in seen:
            raise Dup()
        seen.add(k)
    return dict(pairs)


def bad_const(_):
    raise Const()


def read_entry(path):
    """通常ファイルだけを、上限つきで読む。(本文, 違反の種類) を返す。"""
    try:
        st = os.lstat(path)
    except OSError:
        return None, "unreadable-entry"
    if not stat.S_ISREG(st.st_mode):
        return None, "not-a-regular-file"
    if st.st_size > MAX_BYTES:
        return None, "too-large"
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        fd = os.open(path, flags)
    except OSError:
        return None, "unreadable-entry"
    try:
        chunks = []
        total = 0
        while True:
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            total += len(chunk)
            if total > MAX_BYTES:
                return None, "too-large"
            chunks.append(chunk)
    except OSError:
        return None, "unreadable-entry"
    finally:
        os.close(fd)
    return b"".join(chunks), None


def is_str(v):
    return type(v) is str


def check_record(rec):
    """部品 1 件の形を検査する。(キー, 違反の種類の列) を返す（キーは重複の検出用）。"""
    kinds = []
    if not isinstance(rec, dict):
        return None, ["not-an-object"]
    for k in REQUIRED:
        if k not in rec:
            kinds.append("missing-field")
    for k in rec:
        if k not in REQUIRED and k not in OPTIONAL:
            kinds.append("unknown-field")
    if kinds:
        return None, kinds

    ids = rec["behavior_ids"]
    if not (
        type(ids) is list
        and len(ids) <= MAX_BEHAVIORS
        and all(is_str(x) and BEHAVIOR_ID_RE.fullmatch(x) for x in ids)
        and len(set(ids)) == len(ids)
    ):
        kinds.append("bad-behavior-ids")

    combo = rec["machine_combination"]
    machine = rec["machine"]
    expected_machine = None
    for name, m in COMBINATIONS:
        if is_str(combo) and combo == name:
            expected_machine = m
    if expected_machine is None:
        kinds.append("unknown-combination")
    elif not is_str(machine) or machine != expected_machine:
        kinds.append("machine-mismatch")

    label = rec["label"]
    reasons = None
    known_label = False
    for name, rs in LABELS:
        if is_str(label) and label == name:
            known_label = True
            reasons = rs
    if not known_label:
        kinds.append("unknown-label")
    elif reasons is None:
        if "label_reason" in rec:
            kinds.append("unexpected-reason")
    elif "label_reason" not in rec:
        kinds.append("missing-reason")
    else:
        r = rec["label_reason"]
        if not (is_str(r) and r in reasons):
            kinds.append("unknown-reason")

    if not (is_str(rec["body_kind"]) and rec["body_kind"] in BODY_KINDS):
        kinds.append("unknown-body-kind")

    body = rec["body"]
    test_id = None
    if not isinstance(body, dict):
        kinds.append("bad-body")
    else:
        for k in body:
            if k not in ("test_id", "threshold_id"):
                kinds.append("bad-body")
                break
        tid = body.get("test_id")
        if not (is_str(tid) and TEST_ID_RE.fullmatch(tid)):
            kinds.append("bad-body")
        else:
            test_id = tid
        if known_label and label == "pending":
            th = body.get("threshold_id")
            if not (is_str(th) and THRESHOLD_ID_RE.fullmatch(th)):
                kinds.append("bad-body")
        elif "threshold_id" in body:
            kinds.append("bad-body")

    if kinds:
        return None, kinds
    return (test_id, combo, label), []


def fail(kinds):
    for k in kinds:
        print("violation: %s" % k, file=sys.stderr)
    print("FAILED: %d violation(s)" % len(kinds), file=sys.stderr)
    sys.exit(1)


def main():
    argv = sys.argv[1:]
    if len(argv) != 1 or argv[0].startswith("-"):
        print("usage: record-summary.sh <record directory>", file=sys.stderr)
        sys.exit(2)
    path = argv[0]
    try:
        st = os.lstat(path)
    except OSError:
        print("error: record directory not found or unreadable", file=sys.stderr)
        sys.exit(2)
    if not stat.S_ISDIR(st.st_mode):
        print("error: record directory is not a directory", file=sys.stderr)
        sys.exit(2)
    try:
        names = sorted(e.name for e in os.scandir(path))
    except OSError:
        print("error: record directory not found or unreadable", file=sys.stderr)
        sys.exit(2)

    if len(names) > MAX_ENTRIES:
        fail(["too-many-entries"])
    if not names:
        fail(["no-records"])

    violations = []
    seen = set()
    counts = {}
    for name in names:
        if not name.endswith(".json"):
            violations.append("not-a-record-name")
            continue
        raw, kind = read_entry(os.path.join(path, name))
        if kind is not None:
            violations.append(kind)
            continue
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError:
            violations.append("not-utf8")
            continue
        line = text[:-1] if text.endswith("\n") else text
        if "\n" in line or "\r" in line:
            violations.append("not-one-line")
            continue
        try:
            rec = json.loads(line, object_pairs_hook=pairs_hook, parse_constant=bad_const)
        except Dup:
            violations.append("duplicate-key")
            continue
        except (ValueError, Const, RecursionError):
            violations.append("invalid-json")
            continue
        key, kinds = check_record(rec)
        if kinds:
            violations.extend(kinds)
            continue
        pair = (key[0], key[1])
        if pair in seen:
            violations.append("duplicate-record")
            continue
        seen.add(pair)
        counts[(key[1], key[2])] = counts.get((key[1], key[2]), 0) + 1

    if violations:
        fail(violations)

    out = ["| combination |" + "".join(" %s |" % n for n, _ in LABELS)]
    out.append("| --- |" + " ---: |" * len(LABELS))
    for cname, _ in COMBINATIONS:
        out.append(
            "| %s |" % cname
            + "".join(" %d |" % counts.get((cname, lname), 0) for lname, _ in LABELS)
        )
    print("\n".join(out))


main()
PY
