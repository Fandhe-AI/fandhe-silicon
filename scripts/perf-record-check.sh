#!/usr/bin/env bash
# run ごとの記録（`measure/` の測定の道具が書く JSON 1 ファイル）の形を検査し、
# 回の記録と突き合わせて「判定に使えるか」を確かめる（spec TASK-27・TASK-28 のポインタ）。
# 呼び出し元: 手元の確認（実機の回の記録を持ち帰ったあと）と、回帰テスト
# `scripts/test-perf-record-check.sh`・`measure/crates/perf/tests/repeat.rs`。
#
# 使い方:
#   scripts/perf-record-check.sh <run の記録のパス>
#   scripts/perf-record-check.sh --session <回の記録のパス> [<run の記録のパス> ...]
# 終了コード: 0 合格 / 1 違反または判定に使えない / 2 誤用・前提の不足（パスが無い場合を含む）
# 判定に使えない記録を成功扱いにしない（fail-closed）。
#
# make ターゲットを持たない理由: 引数（検査する記録）が要る。既定の入力を持たせると
# 「実物を検査した」と誤解させるため、直接実行だけにする。
#
# 形の検査: 欄の集合（過不足）・型・値の形・配列の長さの一致・条件付きの欄の有無・時刻順・
# 組み合わせと環境の背後実装の整合。欄の定義はこのファイルの定数表が唯一の正で、
# 書く側は measure/crates/perf の record・evidence・json。食い違いは repeat.rs が検出する。
# 判定に使えるかの検査（--session）: 回の記録の形（session-record-check.sh）を先に通し、
# 回の側の除外条件と run ごとの除外条件（回への参照・時間帯・電源・熱・検証レイヤー・取得不能）を見る。
# 入れない検査: 統計値の再計算、run 間の突き合わせ、ホスト情報の置き換えの有無、
# 回の run_ids との突き合わせ、機材と組み合わせの対応。
#
# 入力の値と表に無い欄の名前は出力に出さない。ファイルを作らず、ネットワークを使わない。
# 実行環境: 実機（macOS）でも流すので bash 3.2 互換・Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う（先頭の引数はこのディレクトリ）。
exec python3 -I - "${DIR}" "$@" <<'PY'
import argparse
import datetime
import json
import os
import re
import stat
import subprocess
import sys

# 標本の配列が入るため回の記録より大きく取る。標本数の上限を道具が持たないので、
# 読み込む量の上限をここで決める（資源の枯渇を防ぐ）。
MAX_BYTES = 8 * 1024 * 1024
U64_MAX = (1 << 64) - 1
# 中央値の上限。書き出し側（measure の stats の MAX_EXACT_NS）と同じ 1 << 52。
MAX_EXACT_NS = 1 << 52

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

UNAV = "_unavailable"
NO_OUTPUT = "_no-output"
NO_SESSION = "_none"

SCHEMA_VERSIONS = ("1",)
# 綴りは measure/crates/perf/src/record.rs・evidence.rs に揃える（core の test_frame と突き合わせ済み）。
MACHINES = ("mac-mini", "node-a", "node-b", "amd", "intel")
COMBO_BACKEND = {
    "metal-mac-mini": "metal",
    "vulkan-node-b": "vulkan",
    "vulkan-node-a": "vulkan",
    "vulkan-amd": "vulkan",
    "vulkan-intel": "vulkan",
    "cuda-node-b": "cuda",
    "cuda-node-a": "cuda",
}
LABELS = ("ok", "ng", "pending", "not-run", "indeterminate")
REASONS = {
    "pending": ("threshold-unapproved",),
    "not-run": (
        "not-configured",
        "invalid-combination-setting",
        "invalid-permission-setting",
        "combination-not-declared",
        "permission-missing",
    ),
    "indeterminate": ("device-mismatch", "machine-unreachable"),
}
BODY_KINDS = ("perf-samples",)
BACKENDS = ("metal", "vulkan", "cuda")
OS_NAMES = ("macos", "linux", "windows")
PROFILES = ("dev", "release", "test", "bench")
VALIDATION = ("enabled", "disabled")
POWER = ("ac", "battery", "ups")
PHASES = ("cold", "warmup", "measured")
STAT_KEYS = ("median", "q1", "q3", "p95", "min", "max")

ID_RE = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")
REF_RE = re.compile(r"[A-Z]+(-[A-Z0-9]+)+")
TIME_RE = re.compile(r"([0-9]{4})-([0-9]{2})-([0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})Z")
SHA_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
SEMVER_RE = re.compile(r"(0|[1-9][0-9]{0,8})(\.(0|[1-9][0-9]{0,8})){2}")
VERSION_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9.+-]{0,63}")
NAME_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}")
DEVICE_RE = re.compile(
    r"[A-Za-z0-9.()+,-]([A-Za-z0-9 ._()+,-]{0,126}[A-Za-z0-9._()+,-])?"
)
MEDIAN_RE = re.compile(r"[0-9]+\.5")
MILLI_RE = re.compile(r"[0-9]{1,6}\.[0-9]{3}")
OPT_BAD = set("\"'`\\/~")
REPO_BAD = set("\"'`\\@?#")


class Dec(str):
    """JSON の小数。文字列のまま持ち、float に丸めない。"""


class Dup(Exception):
    pass


class Const(Exception):
    pass


class Fatal(Exception):
    def __init__(self, code, msg):
        Exception.__init__(self)
        self.code = code
        self.msg = msg


def pairs_hook(pairs):
    seen = set()
    for k, _ in pairs:
        if k in seen:
            raise Dup()
        seen.add(k)
    return dict(pairs)


def bad_const(_):
    raise Const()


def load(path, what):
    """ファイルを厳格に読んで JSON の値を返す。失敗は Fatal（値は出さない）。"""
    try:
        st = os.stat(path)
    except OSError:
        raise Fatal(2, "error: %s not found or unreadable" % what)
    if not stat.S_ISREG(st.st_mode):
        raise Fatal(1, "error: %s is not a regular file" % what)
    if st.st_size > MAX_BYTES:
        raise Fatal(1, "error: %s too large" % what)
    try:
        with open(path, "rb") as f:
            raw = f.read(MAX_BYTES + 1)
    except OSError:
        raise Fatal(2, "error: %s not found or unreadable" % what)
    if len(raw) > MAX_BYTES:
        raise Fatal(1, "error: %s too large" % what)
    try:
        text = raw.decode("ascii")
    except UnicodeDecodeError:
        raise Fatal(1, "error: %s is not ASCII" % what)
    try:
        return json.loads(
            text,
            object_pairs_hook=pairs_hook,
            parse_constant=bad_const,
            parse_float=Dec,
        )
    except Dup:
        raise Fatal(1, "error: %s has a duplicate field" % what)
    except (Const, ValueError, RecursionError):
        raise Fatal(1, "error: %s is not valid JSON" % what)


# ---- 値の形の述語（型を `type(v) is` で厳密に見る。bool は int として通さない）----

def s_re(rx):
    return lambda v: type(v) is str and rx.fullmatch(v) is not None


def s_in(values):
    return lambda v: type(v) is str and v in values


def unav(pred):
    return lambda v: type(v) is str and (v == UNAV or pred(v))


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


def is_uint(v):
    return type(v) is int and 0 <= v <= U64_MAX


def is_opt_uint(v):
    return v is None or is_uint(v)


def is_bool(v):
    return type(v) is bool


def is_opt_bool(v):
    return v is None or type(v) is bool


def is_median(v):
    if type(v) is int:
        return 0 <= v <= MAX_EXACT_NS
    if type(v) is Dec and MEDIAN_RE.fullmatch(v) is not None:
        # 整数部で比べる（浮動小数にしない）。N.5 は N < 上限のときだけ上限以下。
        return int(v.split(".")[0]) < MAX_EXACT_NS
    return False


def is_option(v):
    return (
        type(v) is str
        and 1 <= len(v) <= 128
        and v[0] != "_"
        and all(0x21 <= ord(c) <= 0x7E and c not in OPT_BAD for c in v)
    )


def is_repo(v):
    return (
        type(v) is str
        and len(v) <= 256
        and v.startswith("https://")
        and len(v) > len("https://")
        and all(0x21 <= ord(c) <= 0x7E and c not in REPO_BAD for c in v)
    )


def opt_list(item):
    return lambda v: v is None or (type(v) is list and all(item(x) for x in v))


def opt_map(item):
    return lambda v: v is None or (
        type(v) is dict and all(NAME_RE.fullmatch(k) for k in v) and all(item(x) for x in v.values())
    )


def ref_list(v):
    return type(v) is list and all(s_re(REF_RE)(x) and len(x) <= 64 for x in v)


def is_id(v):
    return s_re(ID_RE)(v) and len(v) <= 64


def is_loadavg(v):
    return v is None or (
        type(v) is list and len(v) == 3 and all(type(x) is Dec and MILLI_RE.fullmatch(x) for x in v)
    )


VERSION = s_re(VERSION_RE)
DEVICE = s_re(DEVICE_RE)
SHA = s_re(SHA_RE)
COMMIT = s_re(COMMIT_RE)

# ---- 欄の定数表（欄名 -> 述語、入れ子は表）----

SNAPSHOT = {
    "utc": unav(is_time),
    "power_source": unav(s_in(POWER)),
    "low_power_mode": is_opt_bool,
    "thermal_warning": is_opt_bool,
    "loadavg": is_loadavg,
}

ENV_COMMON = {
    "gpu": unav(DEVICE),
    "cpu": unav(DEVICE),
    "os_name": unav(s_in(OS_NAMES)),
    "os_version": unav(VERSION),
    "rustc_version": unav(VERSION),
    "repo_commit": unav(COMMIT),
    "cargo_lock_sha256": unav(SHA),
    "build_profile": unav(s_in(PROFILES)),
    "feature_set": opt_list(is_option),
    "target_features": opt_list(is_option),
    "shader_compile_options": opt_list(is_option),
    "library_versions": opt_map(VERSION),
    "validation_layers": unav(s_in(VALIDATION)),
    "before": SNAPSHOT,
    "after": SNAPSHOT,
}
ENV_BACKEND = {
    "metal": {"xcode_version": unav(VERSION), "metal_toolchain": unav(VERSION)},
    "vulkan": {"driver_version": unav(VERSION), "sdk_versions": opt_map(VERSION)},
    "cuda": {"driver_version": unav(VERSION), "sdk_versions": opt_map(VERSION)},
}

EVIDENCE = {
    "adapter_backend": unav(s_in(BACKENDS)),
    "adapter_name": unav(DEVICE),
    "kernel_sha256": opt_map(SHA),
    "artifact_sha256": opt_map(SHA),
    "input_sha256": opt_map(SHA),
    "output_sha256": lambda v: type(v) is str and (v in (UNAV, NO_OUTPUT) or SHA(v)),
}

BASELINE_COMMON_HEAD = {"version": unav(s_re(SEMVER_RE)), "source": s_in(("crates-io", "git"))}
BASELINE_GIT = {
    "git_repository": unav(is_repo),
    "git_tag": unav(s_re(NAME_RE)),
    "git_commit": unav(COMMIT),
}
BASELINE_TAIL = {"lockfile_sha256": unav(SHA), "basis": s_re(NAME_RE)}

ENVELOPE = {
    "schema_version": s_in(SCHEMA_VERSIONS),
    "run_id": is_id,
    "task_ids": ref_list,
    "behavior_ids": ref_list,
    "machine_combination": s_in(tuple(COMBO_BACKEND)),
    "machine": s_in(MACHINES),
    "label": s_in(LABELS),
    "body_kind": s_in(BODY_KINDS),
    "started_utc": is_time,
    "ended_utc": is_time,
    "session_ref": lambda v: v == NO_SESSION or is_id(v),
}

PHASE_UINT = {p: lambda v: type(v) is list and all(is_uint(x) for x in v) for p in PHASES}
PHASE_OPT = {p: lambda v: type(v) is list and all(is_opt_uint(x) for x in v) for p in PHASES}
STATS = {k: (is_median if k == "median" else is_uint) for k in STAT_KEYS}
BODY = {
    "sample_counts": {p: is_uint for p in PHASES},
    "submission_count": is_uint,
    "dispatch_count": is_uint,
    "processed_count": is_opt_uint,
    "instructions_found": lambda v: type(v) is dict
    and all(NAME_RE.fullmatch(k) for k in v)
    and all(type(x) is bool for x in v.values()),
    "cpu_ns": PHASE_UINT,
    "gpu_ns": PHASE_OPT,
    "stats": {"cpu_ns": STATS, "gpu_ns": lambda v: v is None},
}


def check_obj(v, spec, path, viol):
    """欄の集合が表と過不足なく一致し、各値が述語を満たすことを見る。表に無い欄は件数だけ報告。"""
    if type(v) is not dict:
        viol.append("error: field %s: not an object" % path)
        return False
    ok = True
    for name, pred in spec.items():
        sub = "%s.%s" % (path, name) if path else name
        if name not in v:
            viol.append("error: field %s: missing" % sub)
            ok = False
        elif isinstance(pred, dict):
            ok = check_obj(v[name], pred, sub, viol) and ok
        elif pred is not None and not pred(v[name]):
            viol.append("error: field %s: invalid value" % sub)
            ok = False
    extra = sum(1 for k in v if k not in spec)
    if extra:
        viol.append("error: %d field(s) not in the table%s" % (extra, " at " + path if path else ""))
        ok = False
    return ok


def check_run(data):
    """run ごとの記録の形を検査して違反の文言の一覧を返す。"""
    viol = []
    if type(data) is not dict:
        return ["error: top level is not an object"]

    label = data.get("label")
    combo = data.get("machine_combination")
    backend = COMBO_BACKEND.get(combo) if type(combo) is str else None

    spec = dict(ENVELOPE)
    if type(label) is str and label in REASONS:
        spec["label_reason"] = s_in(REASONS[label])
    spec.update({"evidence": EVIDENCE, "body": lambda v: type(v) is dict})
    spec_env = dict(ENV_COMMON)
    if backend is not None:
        spec_env.update(ENV_BACKEND[backend])
    else:
        # 組み合わせが不正なら背後実装ごとの欄を特定できない。環境の欄は共通部分だけ見る。
        for fields in ENV_BACKEND.values():
            spec_env.update(fields)
    spec["environment"] = spec_env
    if "wgpu_baseline" in data:
        base = dict(BASELINE_COMMON_HEAD)
        wb = data["wgpu_baseline"]
        if type(wb) is dict and wb.get("source") == "git":
            base.update(BASELINE_GIT)
        base.update(BASELINE_TAIL)
        spec["wgpu_baseline"] = base
    check_obj(data, spec, "", viol)

    # 時刻順（両方が正しい形のときだけ。固定長なので文字列順が時刻順）。
    s, e = data.get("started_utc"), data.get("ended_utc")
    if is_time(s) and is_time(e) and s > e:
        viol.append("error: started_utc is after ended_utc")
    # 環境の背後実装の欄が組み合わせの背後実装に合うこと（欄の集合の過不足で既に見ている）。

    body = data.get("body")
    if isinstance(body, dict):
        check_body(body, viol)
    return viol


def check_body(body, viol):
    spec = dict(BODY)
    # 統計の gpu_ns は null か統計の表。構造だけ先に見る。
    stats = body.get("stats")
    if isinstance(stats, dict) and isinstance(stats.get("gpu_ns"), dict):
        spec["stats"] = {"cpu_ns": STATS, "gpu_ns": STATS}
    sub = []
    ok = check_obj(body, spec, "body", sub)
    viol.extend(sub)
    if not ok:
        return
    counts = body["sample_counts"]
    if counts["measured"] < 1:
        viol.append("error: field body.sample_counts.measured: invalid value")
    for key in ("cpu_ns", "gpu_ns"):
        for p in PHASES:
            if len(body[key][p]) != counts[p]:
                viol.append("error: field body.%s.%s: length differs from sample_counts" % (key, p))
    gpu_measured = body["gpu_ns"]["measured"]
    has_null = any(x is None for x in gpu_measured)
    gpu_stats = body["stats"]["gpu_ns"]
    if has_null and gpu_stats is not None:
        viol.append("error: field body.stats.gpu_ns: must be null when a measured gpu sample is null")
    if not has_null and gpu_stats is None and len(gpu_measured) > 0:
        viol.append("error: field body.stats.gpu_ns: must not be null")


# ---- 判定に使えるかの検査 ----

def walk_unavailable(v):
    """None または予約語 `_unavailable` が 1 つでもあれば真。"""
    if v is None or (type(v) is str and v == UNAV):
        return True
    if isinstance(v, dict):
        return any(walk_unavailable(x) for x in v.values())
    if isinstance(v, list):
        return any(walk_unavailable(x) for x in v)
    return False


def exclusion(run, session):
    """判定に使えない最初の理由（固定の語）。使えるなら None。"""
    ref = run["session_ref"]
    if ref == NO_SESSION:
        return "no-session"
    if ref != session["session"]:
        return "session-mismatch"
    if run["started_utc"] < session["started_utc"] or run["ended_utc"] > session["ended_utc"]:
        return "outside-session-window"
    env = run["environment"]
    for snap in (env["before"], env["after"]):
        if snap["power_source"] != "ac":
            return "power-not-ac"
    if env["before"]["low_power_mode"] is True or env["after"]["low_power_mode"] is True:
        return "low-power-mode"
    if env["before"]["thermal_warning"] is True or env["after"]["thermal_warning"] is True:
        return "thermal-warning"
    if env["validation_layers"] == "enabled":
        return "validation-layers-enabled"
    if walk_unavailable(run["evidence"]) or walk_unavailable(env):
        return "unavailable-field"
    return None


class Parser(argparse.ArgumentParser):
    def error(self, message):
        # argparse の既定は入力の語を含む文言を出すので、固定の文言に置き換える。
        print("usage: perf-record-check.sh <run.json> | --session <session.json> [<run.json> ...]", file=sys.stderr)
        sys.exit(2)


def report(viol):
    for m in viol:
        print(m, file=sys.stderr)
    print("FAILED: %d violation(s)" % len(viol), file=sys.stderr)


def run_shape(path):
    return check_run(load(path, "run record"))


def main():
    here = sys.argv[1]
    ap = Parser(prog="perf-record-check.sh", add_help=True)
    ap.add_argument("--session")
    ap.add_argument("paths", nargs="*")
    try:
        args = ap.parse_args(sys.argv[2:])
    except SystemExit as e:
        sys.exit(0 if e.code in (0, None) else 2)

    if args.session is None:
        if len(args.paths) != 1:
            ap.error("")
        viol = run_shape(args.paths[0])
        if viol:
            report(viol)
            sys.exit(1)
        print("ok: run record shape")
        return

    # 回の記録の形は session-record-check.sh に任せる（同じ検査を二重に書かない）。
    checker = os.path.join(here, "session-record-check.sh")
    r = subprocess.run(["bash", checker, args.session], stdout=subprocess.DEVNULL)
    if r.returncode != 0:
        sys.exit(r.returncode)
    session = load(args.session, "session record")

    bad = 0
    if session["low_power_mode"] is True:
        print("session: excluded: low-power-mode")
        bad += 1
    elif session["interrupted"] is True:
        print("session: excluded: interrupted")
        bad += 1
    else:
        print("session: eligible")
    session_ok = bad == 0

    seen = set()
    for i, path in enumerate(args.paths, 1):
        data = load(path, "run record")
        viol = check_run(data)
        if viol:
            print("run %d: excluded: invalid-shape" % i)
            report(viol)
            bad += 1
            continue
        reason = None
        if not session_ok:
            reason = "session-excluded"
        elif data["run_id"] in seen:
            reason = "duplicate-run-id"
        else:
            reason = exclusion(data, session)
        seen.add(data["run_id"])
        if reason is None:
            print("run %d: eligible" % i)
        else:
            print("run %d: excluded: %s" % (i, reason))
            bad += 1
    if bad:
        sys.exit(1)


try:
    main()
except Fatal as f:
    print(f.msg, file=sys.stderr)
    sys.exit(f.code)
PY
