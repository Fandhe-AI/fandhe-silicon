#!/usr/bin/env bash
# 回ごとの記録（専有の確かめと時間帯の調整の記録。JSON 1 ファイル）の形を検査する
# （spec TASK-19・TASK-28 のポインタ）。
# 呼び出し元: `docs/metal-run-procedure.md` の手順（記録を持ち帰る前の確認）と、
# 回帰テスト `scripts/test-session-record-check.sh`。
#
# 使い方: scripts/session-record-check.sh <記録のパス>
# 終了コード: 0 合格 / 1 違反 / 2 誤用・前提の不足（パスが無い場合を含む。成功扱いにしない）
#
# make ターゲットを持たない理由: 引数（検査する記録）が要る。既定の入力を持たせると
# 「実物を検査した」と誤解させるため、直接実行だけにする。
#
# 検査するのは形（欄の集合・型・値の形）だけで、欄の定義はこのファイルの定数表が唯一の正。
# 欄の意味と決定の所在は spec 側（TASK-19・TASK-28）。値の一覧を検査していない欄がある
# （定数表で値の形だけを見る欄。resident_process_stopped・gui_login_state）。
# 入れない検査: 欄どうしの前後関係、run ごとの記録との突き合わせ、低電力モードの値による採否。
#
# 入力の値と表に無い欄の名前は出力に出さない。ファイルを作らず、ネットワークを使わない。
# 実行環境: 実機（macOS）でも流すので bash 3.2 互換・Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "$@" <<'PY'
import argparse
import datetime
import json
import os
import re
import stat
import sys

MAX_BYTES = 1024 * 1024

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

ID_RE = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")
TIME_RE = re.compile(r"([0-9]{4})-([0-9]{2})-([0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})Z")

SCHEMA_VERSIONS = ("1",)
# 綴りは crates/core/src/test_frame/combination.rs の Machine::name に揃える。
MACHINES = ("mac-mini",)

# 欄名 -> 種別。種別: version / id / machine / time / posint / ids / bool / word
FIELDS = {
    "schema_version": "version",
    "session": "id",
    "machine": "machine",
    "asked_utc": "time",
    "answered_utc": "time",
    "started_utc": "time",
    "ended_utc": "time",
    "owner_answer": "posint",
    "run_ids": "ids",
    "interrupted": "bool",
    "low_power_mode": "bool",
    "resident_process_stopped": "word",
    "gui_login_state": "word",
}


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


def is_id(v):
    return (
        type(v) is str
        and ID_RE.fullmatch(v) is not None
        and 1 <= len(v) <= 64
    )


def is_time(v):
    if type(v) is not str or len(v) != 20:
        return False
    m = TIME_RE.fullmatch(v)
    if m is None:
        return False
    try:
        datetime.datetime(*[int(g) for g in m.groups()])
    except ValueError:
        return False
    return True


def check(kind, v):
    if kind == "version":
        return type(v) is str and v in SCHEMA_VERSIONS
    if kind == "machine":
        return type(v) is str and v in MACHINES
    if kind in ("id", "word"):
        return is_id(v)
    if kind == "time":
        return is_time(v)
    if kind == "posint":
        return type(v) is int and v >= 1
    if kind == "bool":
        return type(v) is bool
    if kind == "ids":
        return (
            type(v) is list
            and all(is_id(x) for x in v)
            and len(set(v)) == len(v)
        )
    return False


def fail(messages):
    for m in messages:
        print(m, file=sys.stderr)
    print("FAILED: %d violation(s)" % len(messages), file=sys.stderr)
    sys.exit(1)


def main():
    ap = argparse.ArgumentParser(prog="session-record-check.sh")
    ap.add_argument("path")
    try:
        args = ap.parse_args(sys.argv[1:])
    except SystemExit as e:
        sys.exit(0 if e.code in (0, None) else 2)
    path = args.path
    try:
        st = os.stat(path)
    except OSError:
        print("error: input not found or unreadable", file=sys.stderr)
        sys.exit(2)
    if not stat.S_ISREG(st.st_mode):
        fail(["error: input is not a regular file"])
    if st.st_size > MAX_BYTES:
        fail(["error: input too large"])
    try:
        with open(path, "rb") as f:
            raw = f.read(MAX_BYTES + 1)
    except OSError:
        print("error: input not found or unreadable", file=sys.stderr)
        sys.exit(2)
    if len(raw) > MAX_BYTES:
        fail(["error: input too large"])
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        text = None
    if text is None or text.startswith("﻿"):
        fail(["error: input is not strict UTF-8 without BOM"])
    try:
        data = json.loads(text, object_pairs_hook=pairs_hook, parse_constant=bad_const)
    except Dup:
        fail(["error: duplicate field"])
    except (Const, ValueError, RecursionError):
        fail(["error: input is not valid JSON"])
    if type(data) is not dict:
        fail(["error: top level is not an object"])
    viol = []
    for name, kind in FIELDS.items():
        if name not in data:
            viol.append("error: field %s: missing" % name)
        elif not check(kind, data[name]):
            viol.append("error: field %s: invalid %s" % (name, kind))
    extra = sum(1 for k in data if k not in FIELDS)
    if extra:
        viol.append("error: %d field(s) not in the table" % extra)
    if viol:
        fail(viol)
    print("ok: session record shape")


main()
PY
