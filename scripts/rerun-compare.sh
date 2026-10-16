#!/usr/bin/env bash
# 流し直しの照合（spec TASK-29 のポインタ。親 Issue #135 の 5/8）で、前の記録と新しい記録を比べる。
# 記録 = 試験の枠が書き出す部品のディレクトリ（`<試験の ID>.<組み合わせの名前>.json`。
# crates/core/src/rerun/run.rs の `export_outcomes` と crates/core/src/test_frame の `export_parts` が作る）。
# 呼び出し元: 手元の直接実行と、回帰テスト `scripts/test-rerun-compare.sh`（`make test-rerun-compare`。CI の `deny` ジョブも呼ぶ）。
# make ターゲットは持たない: 比べる記録のパスの引数が要るため。既定の入力を持たせると、
# 実物を比べたと誤解させる（scripts/perf-record-check.sh・scripts/public-api-diff.sh と同じ扱い）。
# `measure-perf` の run ごとの記録（scripts/perf-record-check.sh の対象）は扱わない。役割を重ねない。
#
# 使い方:
#   scripts/rerun-compare.sh <前の記録のディレクトリ> <新しい記録のディレクトリ>
# 終了コード（diff(1) の意味にそろえる）:
#   0 差分なし / 1 差分あり（1 件以上） / 2 誤用・前提の不足・読めない記録・想定外の例外
#   （2 を成功扱いにしない。読めない入力を「差分なし」と読ませない）
# 標準出力（終了コード 0 と 1）: 決定的な順の行。分類の順は regressed → improved → changed → removed → added、
#   各分類の中は (試験の ID, 組み合わせ) の昇順。締めに
#   `summary: same=<n> regressed=<n> improved=<n> changed=<n> removed=<n> added=<n>`。0 のときは続けて
#   `ok: no differences from the previous record`。変わらなかった行は件数（same）だけで、行としては出さない。
# 標準エラー: 終了コード 1 で `DIFFERENT: <n> difference(s)`、終了コード 2 で `error: <固定の文言>`
#   （誤用は `usage: ...`）。
#
# 形の検査は定義表を二重に持たず、各ディレクトリを先に scripts/record-summary.sh に通す（定数表の唯一の正）。
#   その終了コードが 0 以外なら、本道具は 2 にする（record-summary の 1 を、本道具の 1 =「差分あり」へ流さない）。
#   関門を通ったあと、比べるのに要る欄だけを防御的に読み直す（関門との間の差し替えにも 2 に倒す）。
# 突き合わせの鍵は (body.test_id, machine_combination)。ファイル名でなく中身から取る。
# 比べる値は (label, label_reason) の組。理由だけが変わっても差として出す。
#   regressed: 前が ok で新が ok 以外 / improved: 前が ok 以外で新が ok /
#   changed: 上の 2 つ以外で値が変わった / removed: 前にだけ在る鍵 / added: 新にだけ在る鍵。
#   分類は出力の見出しで、合否の判定ではない。どの差を許すかは呼び出し側が決める。
# 比べないもの: behavior_ids・machine・body の他の値（threshold_id を含む）。出力にも出さない。
# 出すのは検査済みのトークン（試験の ID・組み合わせ・ラベル・理由）と件数だけ。引数のパス・ホストの絶対パス・
#   環境変数の値は出さない。入力は読むだけで、ファイルを作らず、ネットワークを使わない。
# 実行環境: macOS の bash 3.2 と同梱の python3（3.8 以上の標準ライブラリだけ）でも動く。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "${DIR}/record-summary.sh" "$@" <<'PY'
import json
import os
import re
import stat
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

USAGE = "usage: rerun-compare.sh <previous record directory> <new record directory>"

# record-summary.sh と同じ上限。
MAX_BYTES = 64 * 1024
MAX_ENTRIES = 10000

TEST_ID_RE = re.compile(r"[a-z0-9_-]{1,128}", re.ASCII)
TOKEN_RE = re.compile(r"[a-z0-9-]{1,64}", re.ASCII)

CLASSES = ("regressed", "improved", "changed", "removed", "added")


class Misuse(Exception):
    """誤用・前提の不足・読めない記録（終了コード 2）。"""


class Dup(Exception):
    pass


class Const(Exception):
    pass


def clean(text):
    """出力に出す値から想定外の文字を落とす（検査済みのトークンに対する念のための置き換え）。"""
    return re.sub(r"[^A-Za-z0-9_.:/-]", "?", str(text))[:256]


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def pairs_hook(pairs):
    seen = set()
    for k, _ in pairs:
        if k in seen:
            raise Dup()
        seen.add(k)
    return dict(pairs)


def bad_const(_):
    raise Const()


def normalize(p):
    """末尾の区切りと "." の要素を字句で取り除く（`link/`・`link/.` でもリンクの検査をすり抜けさせない）。"""
    lead = "/" if p.startswith("/") else ""
    parts = [c for c in p.split("/") if c not in ("", ".")]
    r = lead + "/".join(parts)
    return r if r else "."


def gate(summary, path, which):
    """record-summary.sh で形を確かめる。0 以外は 2（標準エラーの違反の種類はそのまま通す）。"""
    try:
        r = subprocess.run(
            [summary, path],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            check=False,
        )
    except OSError:
        raise Misuse("record-summary.sh cannot be run")
    need(r.returncode == 0, "%s record is not readable" % which)


def read_entry(path, which):
    """通常ファイルだけを、上限つきで読む。"""
    msg = "%s record is not readable" % which
    try:
        st = os.lstat(path)
    except OSError:
        raise Misuse(msg)
    need(stat.S_ISREG(st.st_mode) and st.st_size <= MAX_BYTES, msg)
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    try:
        fd = os.open(path, flags)
    except OSError:
        raise Misuse(msg)
    try:
        chunks = []
        total = 0
        while True:
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            total += len(chunk)
            need(total <= MAX_BYTES, msg)
            chunks.append(chunk)
    except OSError:
        raise Misuse(msg)
    finally:
        os.close(fd)
    return b"".join(chunks)


def is_str(v):
    return type(v) is str


def load(summary, arg, which):
    """記録のディレクトリを読み、{(test_id, 組み合わせ): (label, reason)} を返す。"""
    msg = "%s record is not readable" % which
    path = normalize(arg)
    try:
        st = os.lstat(path)
    except OSError:
        raise Misuse(msg)
    need(stat.S_ISDIR(st.st_mode), msg)
    gate(summary, path, which)
    try:
        names = sorted(e.name for e in os.scandir(path))
    except OSError:
        raise Misuse(msg)
    need(0 < len(names) <= MAX_ENTRIES, msg)
    out = {}
    for name in names:
        need(name.endswith(".json"), msg)
        raw = read_entry(os.path.join(path, name), which)
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError:
            raise Misuse(msg)
        line = text[:-1] if text.endswith("\n") else text
        need("\n" not in line and "\r" not in line, msg)
        try:
            rec = json.loads(line, object_pairs_hook=pairs_hook, parse_constant=bad_const)
        except (Dup, Const, ValueError, RecursionError):
            raise Misuse(msg)
        need(isinstance(rec, dict) and isinstance(rec.get("body"), dict), msg)
        tid = rec["body"].get("test_id")
        combo = rec.get("machine_combination")
        label = rec.get("label")
        reason = rec.get("label_reason", "")
        need(is_str(tid) and TEST_ID_RE.fullmatch(tid), msg)
        need(is_str(combo) and TOKEN_RE.fullmatch(combo), msg)
        need(is_str(label) and TOKEN_RE.fullmatch(label), msg)
        need(is_str(reason) and (reason == "" or TOKEN_RE.fullmatch(reason)), msg)
        key = (tid, combo)
        need(key not in out, msg)
        out[key] = (label, reason)
    return out


def show(v):
    return v[0] + ("/" + v[1] if v[1] else "")


def main():
    summary = sys.argv[1]
    argv = sys.argv[2:]
    if argv == ["--help"]:
        print(USAGE)
        return 0
    if len(argv) != 2 or any(a.startswith("-") for a in argv):
        print(USAGE, file=sys.stderr)
        return 2
    before = load(summary, argv[0], "previous")
    after = load(summary, argv[1], "new")
    lines = {c: [] for c in CLASSES}
    same = 0
    for key in sorted(set(before) | set(after)):
        head = "test=%s combination=%s" % (clean(key[0]), clean(key[1]))
        if key not in after:
            lines["removed"].append("removed: %s before=%s" % (head, clean(show(before[key]))))
        elif key not in before:
            lines["added"].append("added: %s after=%s" % (head, clean(show(after[key]))))
        elif before[key] == after[key]:
            same += 1
        else:
            b, a = before[key], after[key]
            if b[0] == "ok":
                cls = "regressed"
            elif a[0] == "ok":
                cls = "improved"
            else:
                cls = "changed"
            lines[cls].append(
                "%s: %s before=%s after=%s" % (cls, head, clean(show(b)), clean(show(a)))
            )
    total = 0
    for c in CLASSES:
        for ln in lines[c]:
            print(ln)
        total += len(lines[c])
    print(
        "summary: same=%d %s"
        % (same, " ".join("%s=%d" % (c, len(lines[c])) for c in CLASSES))
    )
    if total:
        print("DIFFERENT: %d difference(s)" % total, file=sys.stderr)
        return 1
    print("ok: no differences from the previous record")
    return 0


try:
    code = main()
except Misuse as e:
    print("error: %s" % e, file=sys.stderr)
    code = 2
except SystemExit:
    raise
except BaseException as e:
    # 想定外の例外（深すぎる入れ子の RecursionError など）を差分ありの 1 と読ませない。
    print("error: unexpected %s" % type(e).__name__, file=sys.stderr)
    code = 2
sys.exit(code)
PY
