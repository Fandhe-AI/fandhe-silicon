#!/usr/bin/env bash
# Metal の実機（Mac mini）で試験を流す回の手順を助けるスクリプト（spec TASK-19・TASK-28 のポインタ）。
# 呼び出し元: `docs/metal-run-procedure.md` の手順（実機に届く端末のセッションが直接実行する）と、
# 回帰テスト `scripts/test-metal-session.sh`。make ターゲットは持たない（サブコマンドと引数が要るため。
# 既定の入力を持たせると「実物を流した」と誤解させる）。
#
# 使い方:
#   scripts/metal-session.sh probe
#   scripts/metal-session.sh run --combination <name> [--permissions <csv>] [--developer-dir <dir>] [--caffeinate] -- <command> [args...]
#   scripts/metal-session.sh redact <words-file> <input> <output>
#   scripts/metal-session.sh scan <words-file> <file>...
# 終了コード: 0 成功 / 1 違反（置き換えの後に語が残る・入力が不適）/ 2 誤用・前提の不足。
# `run` は起動したコマンドの終了コードをそのまま返す。
#
# probe: 専有の確かめに要る環境の集計値（低電力モード・電源・省電力の設定・熱・負荷の平均）を
#   JSON で標準出力へ出す。macOS 以外では 2 で終わる。取れなかった値は null にし、
#   `unavailable` にその名前を並べる（黙って既定値を入れない）。コマンドの出力は全文を流さず、
#   正規表現で数値と決まった語だけを取り出す（ホスト名・ユーザー名・プロセス名・識別子は出力に出ない）。
#   常駐プロセスと GUI のログインの状態は人が記入する欄で、触れない（`not_probed` に名前を出す）。
# run: 試験の枠が読む環境変数 2 つと DEVELOPER_DIR を設定して、`--` 以降のコマンドを exec する。
#   組み合わせの名前は既定を持たず必須。許可は --permissions で明示したときだけ設定し、
#   呼び出し側の環境に同名の変数があっても引き継がない。DEVELOPER_DIR は --developer-dir か
#   呼び出し側の環境の値（実在するディレクトリ）が要る。既定の開発者ディレクトリは切り替えない。
#   スリープの抑止は --caffeinate を付けたときだけ（`caffeinate -i` の下で起動する）。
# redact / scan: 持ち帰るテキストの置き換えと漏れの検索。語の一覧は機材の側に置き、リポの中は拒否する。
#   語は固定の文字列（正規表現として解釈しない・大文字小文字を区別する）。出力とエラーに語を出さず、
#   件数と行番号だけを出す。置き換えは別のファイルへ書く（入力と既存のファイルを上書きしない）。
#
# 入れていないもの: 設定の変更・プロセスの停止（低電力モードの切り替え・常駐プロセスの停止は人の作業）、
#   `xcode-select` の呼び出し、ネットワーク、プロセスの一覧の取得。
# 実行環境: 実機（macOS）でも流すので bash 3.2 互換・Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# 試験の枠が読む環境変数の名前。綴りは crates/core/src/test_frame/setting.rs に揃える
# （回帰テストが突き合わせる）。
COMBINATION_ENV="FANDHE_SILICON_TEST_MACHINE_COMBINATION"
PERMISSIONS_ENV="FANDHE_SILICON_TEST_PERMISSIONS"

usage() {
  cat >&2 <<'EOF'
usage:
  metal-session.sh probe
  metal-session.sh run --combination <name> [--permissions <csv>] [--developer-dir <dir>] [--caffeinate] -- <command> [args...]
      example name: metal-mac-mini
  metal-session.sh redact <words-file> <input> <output>
  metal-session.sh scan <words-file> <file>...
EOF
  exit 2
}

die() {
  echo "error: $*" >&2
  exit 2
}

# ---- probe ----

cmd_probe() {
  [ "$#" -eq 0 ] || usage
  if [ "$(uname -s 2>/dev/null || true)" != "Darwin" ]; then
    die "probe runs on macOS only"
  fi
  require_cmd python3 || exit 2
  PROBE_TMP="$(mktemp -d "${TMPDIR:-/tmp}/metal-session-probe.XXXXXX")"
  # 取得したコマンドの出力は一時ディレクトリに置き、Python が必要な値だけを取り出す。
  trap 'rm -rf "${PROBE_TMP:?}"' EXIT
  # cap <名前> <コマンド...>: 成功したときだけ ${PROBE_TMP}/<名前> に標準出力を残す。
  cap() {
    local name="$1"
    shift
    if ! "$@" >"${PROBE_TMP}/${name}" 2>/dev/null </dev/null; then
      rm -f "${PROBE_TMP:?}/${name}"
    fi
  }
  cap pmset_g pmset -g
  cap pmset_batt pmset -g batt
  cap pmset_therm pmset -g therm
  cap loadavg sysctl -n vm.loadavg
  python3 -I - "${PROBE_TMP}" <<'PY'
import json
import os
import re
import sys

MAX_BYTES = 64 * 1024
d = sys.argv[1]


def read(name):
    p = os.path.join(d, name)
    try:
        with open(p, "rb") as f:
            return f.read(MAX_BYTES).decode("utf-8", errors="replace")
    except OSError:
        return None


def first_int(text, key):
    m = re.search(r"^[ \t]*" + key + r"[ \t]+([0-9]{1,9})\b", text, re.M)
    return int(m.group(1)) if m else None


out = {}
unavailable = []


def put(key, value):
    out[key] = value
    if value is None:
        unavailable.append(key)


g = read("pmset_g")
if g is None:
    for k in ("low_power_mode", "sleep_minutes", "display_sleep_minutes", "disk_sleep_minutes"):
        put(k, None)
else:
    m = re.search(r"^[ \t]*lowpowermode[ \t]+([01])\b", g, re.M)
    put("low_power_mode", None if m is None else m.group(1) == "1")
    put("sleep_minutes", first_int(g, "sleep"))
    put("display_sleep_minutes", first_int(g, "displaysleep"))
    put("disk_sleep_minutes", first_int(g, "disksleep"))

b = read("pmset_batt")
src = None
if b is not None:
    m = re.search(r"^Now drawing from '(AC Power|Battery Power|UPS Power)'", b, re.M)
    if m:
        src = {"AC Power": "ac", "Battery Power": "battery", "UPS Power": "ups"}[m.group(1)]
put("power_source", src)

t = read("pmset_therm")
# 「警告の記録なし」の注記があるときだけ false。注記が無ければ言い切れないので null。
if t is not None and re.search(r"^Note: No thermal warning level has been recorded", t, re.M):
    put("thermal_warning_recorded", False)
else:
    put("thermal_warning_recorded", None)

la = read("loadavg")
num = r"([0-9]+(?:\.[0-9]+)?)"
m = None if la is None else re.fullmatch(r"\s*\{?\s*" + num + r"\s+" + num + r"\s+" + num + r"\s*\}?\s*", la)
put("load_average", None if m is None else [float(x) for x in m.groups()])

out["unavailable"] = unavailable
out["not_probed"] = ["resident_process_stopped", "gui_login_state"]
print(json.dumps(out, indent=2))
if unavailable:
    print("warning: %d value(s) unavailable" % len(unavailable), file=sys.stderr)
PY
}

# ---- run ----

cmd_run() {
  local combination="" permissions="" have_perm=0 devdir="" caffeinate=0
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --combination)
        [ "$#" -ge 2 ] || usage
        combination="$2"
        shift 2
        ;;
      --permissions)
        [ "$#" -ge 2 ] || usage
        permissions="$2"
        have_perm=1
        shift 2
        ;;
      --developer-dir)
        [ "$#" -ge 2 ] || usage
        devdir="$2"
        shift 2
        ;;
      --caffeinate)
        caffeinate=1
        shift
        ;;
      --)
        shift
        break
        ;;
      *) usage ;;
    esac
  done
  [ "$#" -ge 1 ] || usage
  [ -n "${combination}" ] || die "--combination is required (no default)"
  # 値の形は core の検査と同じ文字種に揃える（知らない名前かどうかは枠が決める）。
  case "${combination}" in
    *[!abcdefghijklmnopqrstuvwxyz0123456789-]*) die "--combination has characters outside [a-z0-9-]" ;;
  esac
  [ "${#combination}" -le 64 ] || die "--combination is too long"
  if [ "${have_perm}" -eq 1 ]; then
    [ -n "${permissions}" ] || die "--permissions is empty (omit the option to grant nothing)"
    case "${permissions}" in
      *[!abcdefghijklmnopqrstuvwxyz0123456789,-]* | ,* | *, | *,,*) die "--permissions must be comma-separated names of [a-z0-9-]" ;;
    esac
    local commas part
    commas="$(printf '%s' "${permissions}" | tr -cd ',' | wc -c | tr -d ' ')"
    [ "${commas}" -le 15 ] || die "--permissions has too many names"
    local parts
    IFS=, read -r -a parts <<<"${permissions}"
    for part in "${parts[@]}"; do
      [ "${#part}" -le 64 ] || die "--permissions has a too long name"
    done
  fi
  if [ -z "${devdir}" ]; then
    devdir="${DEVELOPER_DIR:-}"
  fi
  [ -n "${devdir}" ] || die "developer directory is required (--developer-dir or DEVELOPER_DIR)"
  case "${devdir}" in
    /*) ;;
    *) die "developer directory must be an absolute path" ;;
  esac
  [ -d "${devdir}" ] || die "developer directory does not exist"
  command -v "$1" >/dev/null 2>&1 || die "command not found: $1"
  if [ "${caffeinate}" -eq 1 ]; then
    command -v caffeinate >/dev/null 2>&1 || die "caffeinate not found on PATH"
  fi

  export "${COMBINATION_ENV}=${combination}"
  if [ "${have_perm}" -eq 1 ]; then
    export "${PERMISSIONS_ENV}=${permissions}"
  else
    unset "${PERMISSIONS_ENV}"
  fi
  export DEVELOPER_DIR="${devdir}"
  if [ "${caffeinate}" -eq 1 ]; then
    exec caffeinate -i "$@"
  fi
  exec "$@"
}

# ---- redact / scan ----

# mode(redact|scan) と引数を Python に渡す。シェルは入力の値に触れない。
text_tool() {
  require_cmd python3 || exit 2
  python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import os
import re
import stat
import sys

MAX_WORDS_BYTES = 1024 * 1024
MAX_INPUT_BYTES = 64 * 1024 * 1024
MAX_WORDS = 10000
MAX_LINES_SHOWN = 20
MARK = "<redacted>"

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)


def misuse(msg):
    print("error: " + msg, file=sys.stderr)
    sys.exit(2)


def reject(msg):
    print("error: " + msg, file=sys.stderr)
    print("FAILED: 1 violation(s)", file=sys.stderr)
    sys.exit(1)


def load_words(path, root):
    try:
        st = os.stat(path)
    except OSError:
        misuse("words file not found or unreadable")
    rp = os.path.realpath(path)
    rr = os.path.realpath(root)
    if rp == rr or rp.startswith(rr + os.sep):
        misuse("words file must be outside the repository")
    if not stat.S_ISREG(st.st_mode):
        misuse("words file is not a regular file")
    if st.st_size > MAX_WORDS_BYTES:
        misuse("words file too large")
    try:
        with open(path, "rb") as f:
            raw = f.read(MAX_WORDS_BYTES + 1)
        text = raw.decode("utf-8")
    except (OSError, UnicodeDecodeError):
        misuse("words file is unreadable or not UTF-8")
    if text.startswith("\ufeff"):
        misuse("words file has a BOM")
    words = []
    for line in text.split("\n"):
        if line.endswith("\r"):
            line = line[:-1]
        if line == "":
            continue
        if line.strip() == "":
            misuse("words file has a whitespace-only line")
        if line not in words:
            words.append(line)
    if not words:
        misuse("words file has no words")
    if len(words) > MAX_WORDS:
        misuse("words file has too many words")
    for w in words:
        if w in MARK:
            misuse("a word occurs in the replacement mark")
    return words


def read_input(path, as_text):
    try:
        st = os.stat(path)
    except OSError:
        misuse("input not found or unreadable")
    if not stat.S_ISREG(st.st_mode):
        reject("input is not a regular file")
    if st.st_size > MAX_INPUT_BYTES:
        reject("input too large")
    try:
        with open(path, "rb") as f:
            raw = f.read(MAX_INPUT_BYTES + 1)
    except OSError:
        misuse("input not found or unreadable")
    if len(raw) > MAX_INPUT_BYTES:
        reject("input too large")
    if not as_text:
        return raw
    if b"\x00" in raw:
        reject("input is not text")
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        reject("input is not text")


def byte_pattern(words):
    ws = sorted((w.encode("utf-8") for w in words), key=len, reverse=True)
    return re.compile(b"|".join(re.escape(w) for w in ws))


def residual(pattern, data):
    lines = []
    n = 0
    for m in pattern.finditer(data):
        n += 1
        ln = data.count(b"\n", 0, m.start()) + 1
        if ln not in lines and len(lines) < MAX_LINES_SHOWN:
            lines.append(ln)
    return n, lines


def cmd_redact(root, words_path, in_path, out_path):
    words = load_words(words_path, root)
    text = read_input(in_path, True)
    if os.path.lexists(out_path):
        misuse("output already exists")
    parent = os.path.dirname(os.path.abspath(out_path))
    if not os.path.isdir(parent):
        misuse("output directory does not exist")
    ordered = sorted(words, key=len, reverse=True)
    pat = re.compile("|".join(re.escape(w) for w in ordered))
    result, count = pat.subn(MARK, text)
    data = result.encode("utf-8")
    left, lines = residual(byte_pattern(words), data)
    if left:
        reject("a listed word would remain after replacement (%d occurrence(s))" % left)
    try:
        fd = os.open(out_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        misuse("cannot create output")
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
    except OSError:
        try:
            os.unlink(out_path)
        except OSError:
            pass
        misuse("cannot write output")
    print("ok: replaced %d occurrence(s)" % count)


def cmd_scan(root, words_path, paths):
    pat = byte_pattern(load_words(words_path, root))
    total = 0
    for i, p in enumerate(paths, 1):
        n, lines = residual(pat, read_input(p, False))
        if n:
            total += n
            print(
                "error: input %d: %d residual occurrence(s) on line(s) %s"
                % (i, n, ", ".join(str(x) for x in lines)),
                file=sys.stderr,
            )
    if total:
        print("FAILED: %d residual occurrence(s)" % total, file=sys.stderr)
        sys.exit(1)
    print("ok: no listed word remains in %d file(s)" % len(paths))


def main():
    root, mode, rest = sys.argv[1], sys.argv[2], sys.argv[3:]
    if mode == "redact" and len(rest) == 3:
        cmd_redact(root, *rest)
    elif mode == "scan" and len(rest) >= 2:
        cmd_scan(root, rest[0], rest[1:])
    else:
        misuse("bad arguments")


main()
PY
}

cmd_redact() {
  [ "$#" -eq 3 ] || usage
  text_tool redact "$@"
}

cmd_scan() {
  [ "$#" -ge 2 ] || usage
  text_tool scan "$@"
}

[ "$#" -ge 1 ] || usage
sub="$1"
shift
case "${sub}" in
  probe) cmd_probe "$@" ;;
  run) cmd_run "$@" ;;
  redact) cmd_redact "$@" ;;
  scan) cmd_scan "$@" ;;
  *) usage ;;
esac
