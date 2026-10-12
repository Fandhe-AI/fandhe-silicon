#!/usr/bin/env bash
# metal-session.sh の回帰テスト（spec TASK-19・TASK-28 のポインタ）。
# macOS のコマンド（uname・date・pmset・sysctl・caffeinate・xcode-select・cargo）は、一時ディレクトリに置く
# 偽のコマンドを PATH の先頭に入れて差し替えるので、Linux の CI で全部流れる。
# 偽のデータは example と分かる形（ホスト名・ユーザー名・プロセス名に見える文字列は、出力に現れないことの確認用）。
# 呼び出し元: `make test-metal-session`（直接実行も可。CI の `deny` ジョブと `test-hooks` ジョブ〔macOS を含む〕も呼ぶ。
# macOS では /bin/bash で直接実行する step もある。本体はこのテストを動かしている bash で起動する）。
#
# 判定は「終了コード + 標準出力・標準エラーの部分一致（先頭が ! の語は含まれないこと）」で行う。
# 終了コードだけでは別の理由で落ちても気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。途中で止めず、最後に件数を出す。リポジトリのファイルは変更しない。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
# 偽のコマンドの本文と、展開させない合成の引数（`$(...)` の形）を単引用符で書くため。
# shellcheck disable=SC2016
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd git

MS="${DIR}/metal-session.sh"
SETTING_RS="${ROOT_DIR}/crates/core/src/test_frame/setting.rs"
COMBINATION_RS="${ROOT_DIR}/crates/core/src/test_frame/combination.rs"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-metal-session.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
bin="${work}/bin"
fix="${work}/fix"
mkdir -p "${bin}" "${fix}"
passed=0
failed=0
rc=0

# 呼び出し側の環境の値を引き継がない（実機の CI 環境に同名の変数がありうる）。
unset DEVELOPER_DIR FANDHE_SILICON_TEST_MACHINE_COMBINATION FANDHE_SILICON_TEST_PERMISSIONS

# ---- 偽のコマンド ----

mkfake() {
  printf '#!/bin/sh\n%s\n' "$2" >"${bin}/$1"
  chmod +x "${bin}/$1"
}

export FAKE_FIX="${fix}"
export FAKE_REC="${work}/child.txt"
export FAKE_LOG="${work}/calls.log"
mkfake uname 'echo "${FAKE_UNAME:-Darwin}"'
mkfake date 'echo "date $*" >>"$FAKE_LOG"
[ -z "${FAKE_DATE_FAIL:-}" ] || exit 1
echo "${FAKE_DATE:-2000-01-01T00:00:00Z}"'
mkfake pmset 'echo "pmset $*" >>"$FAKE_LOG"
case "$*" in
  "-g") f=pmset_g ;;
  "-g batt") f=pmset_batt ;;
  "-g therm") f=pmset_therm ;;
  *) exit 2 ;;
esac
[ -f "$FAKE_FIX/$f" ] || exit 1
cat "$FAKE_FIX/$f"'
mkfake sysctl 'echo "sysctl $*" >>"$FAKE_LOG"
[ "$*" = "-n vm.loadavg" ] || exit 2
[ -f "$FAKE_FIX/loadavg" ] || exit 1
cat "$FAKE_FIX/loadavg"'
mkfake xcode-select 'echo "xcode-select $*" >>"$FAKE_LOG"; exit 0'
mkfake caffeinate 'echo "caffeinate $*" >>"$FAKE_LOG"
shift
exec "$@"'
mkfake cargo '{
  printf "COMB=%s\n" "${FANDHE_SILICON_TEST_MACHINE_COMBINATION-UNSET}"
  printf "PERM=%s\n" "${FANDHE_SILICON_TEST_PERMISSIONS-UNSET}"
  printf "DEV=%s\n" "${DEVELOPER_DIR-UNSET}"
  printf "ARGC=%s\n" "$#"
  for a in "$@"; do printf "ARG=%s\n" "$a"; done
} >"$FAKE_REC"
exit "${FAKE_CARGO_RC:-0}"'
PATH="${bin}:${PATH}"
export PATH

# ---- 土台 ----

pass() {
  passed=$((passed + 1))
  echo "ok: $1"
}

fail() {
  failed=$((failed + 1))
  echo "FAIL: $1: $2" >&2
  echo "--- stderr ---" >&2
  sed 's/^/    /' "${ERR_F}" >&2 || true
}

# ms <引数...>: 本体を実行し rc・標準出力・標準エラーを控える。
ms() {
  rc=0
  "${BASH}" "${MS}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# expect <名前> <期待コード> [語...]: 先頭が ! の語は標準出力・標準エラーのどちらにも無いこと。
expect() {
  local name="$1" want="$2" s
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    fail "${name}" "expected exit ${want}, got ${rc}"
    return 0
  fi
  for s in "$@"; do
    case "${s}" in
      '!'*)
        if grep -qF -e "${s#!}" "${ERR_F}" "${OUT_F}"; then
          fail "${name}" "output must not contain: ${s#!}"
          return 0
        fi
        ;;
      *)
        if ! grep -qF -e "${s}" "${ERR_F}" "${OUT_F}"; then
          fail "${name}" "output lacks: ${s}"
          return 0
        fi
        ;;
    esac
  done
  pass "${name}"
}

# pj <名前> <python の式>: 標準出力の JSON を d として式が真になること。式は合成の固定文字列。
pj() {
  if python3 -I -c 'import json, sys
d = json.load(open(sys.argv[1]))
sys.exit(0 if eval(sys.argv[2]) else 1)' "${OUT_F}" "$2" 2>/dev/null; then
    pass "$1"
  else
    fail "$1" "json check failed: $2"
  fi
}

# truthy <名前> <シェルの条件を表す終了コード>: 条件を直接渡せないので、呼び出し側が [ ] の結果を渡す。
check() {
  if [ "$2" -eq 0 ]; then pass "$1"; else fail "$1" "condition false"; fi
}

sparse() {
  python3 -I -c 'import sys
f = open(sys.argv[1], "wb")
f.truncate(int(sys.argv[2]))
f.close()' "$1" "$2"
}

# ---- probe ----

write_probe_fixtures() {
  rm -f "${fix}"/*
  cat >"${fix}/pmset_g" <<'EOF'
System-wide power settings:
Currently in use:
  standby              1
  hibernatemode        0
  powermode            0
  lowpowermode         0
  displaysleep         10
  sleep                0 (sleep prevented by powerd, example-daemon)
  disksleep            10
  womp                 1
  note                 Hostname example-host.example.invalid user example-user
EOF
  cat >"${fix}/pmset_batt" <<'EOF'
Now drawing from 'AC Power'
  -InternalBattery-0 (id=0)	100%; charged; 0:00 remaining present: true example-serial-0001
EOF
  cat >"${fix}/pmset_therm" <<'EOF'
Note: No thermal warning level has been recorded
Note: No performance warning level has been recorded
Note: No CPU power status has been recorded
EOF
  echo "{ 1.50 2.25 3 }" >"${fix}/loadavg"
}

write_probe_fixtures
: >"${FAKE_LOG}"
ms probe
expect "probe: exits 0" 0 '!value(s) unavailable'
pj "probe: values from fixed input" 'd == {
  "utc": "2000-01-01T00:00:00Z", "power_source": "ac", "low_power_mode": False,
  "thermal_warning": False, "loadavg": [1.5, 2.25, 3.0],
  "reference": {"sleep_minutes": 0, "display_sleep_minutes": 10, "disk_sleep_minutes": 10},
  "unavailable": [],
  "not_probed": ["resident_process_stopped", "gui_login_state"]}'
expect "probe: noise from commands not echoed" 0 '!example-host' '!example-user' '!example-daemon' '!example-serial' '!invalid'
bad_calls=0
while IFS= read -r line; do
  case "${line}" in
    "pmset -g" | "pmset -g batt" | "pmset -g therm" | "sysctl -n vm.loadavg" | "date -u +%Y-%m-%dT%H:%M:%SZ") ;;
    *) bad_calls=1 ;;
  esac
done <"${FAKE_LOG}"
check "probe: only read-only queries issued" "${bad_calls}"
check "probe: never calls xcode-select" "$(grep -c '^xcode-select' "${FAKE_LOG}" | sed 's/^[1-9].*/1/')"

FAKE_DATE="2024-02-29T23:59:59Z" ms probe
pj "probe: time comes from date" 'd["utc"] == "2024-02-29T23:59:59Z"'

sed 's/lowpowermode         0/lowpowermode         1/' "${fix}/pmset_g" >"${work}/g.tmp"
mv "${work}/g.tmp" "${fix}/pmset_g"
ms probe
expect "probe: low power mode on exits 0" 0
pj "probe: low power mode on" 'd["low_power_mode"] is True and d["unavailable"] == []'

sed 's/lowpowermode         1/lowpowermode         2/' "${fix}/pmset_g" >"${work}/g.tmp"
mv "${work}/g.tmp" "${fix}/pmset_g"
ms probe
expect "probe: out-of-range low power value exits 1" 1 "1 value(s) unavailable"
pj "probe: out-of-range low power value is unavailable" 'd["low_power_mode"] is None and d["unavailable"] == ["low_power_mode"]'

sed 's/lowpowermode         2/lowpowermode         0/' "${fix}/pmset_g" >"${work}/g.tmp"
mv "${work}/g.tmp" "${fix}/pmset_g"
echo "Now drawing from 'Battery Power'" >"${fix}/pmset_batt"
ms probe
expect "probe: battery power exits 0" 0
pj "probe: battery power" 'd["power_source"] == "battery"'
echo "Now drawing from 'Example Power'" >"${fix}/pmset_batt"
ms probe
expect "probe: unknown power source exits 1" 1
pj "probe: unknown power source is unavailable" 'd["power_source"] is None and d["unavailable"] == ["power_source"]'

echo "Note: something else example-host" >"${fix}/pmset_therm"
ms probe
expect "probe: therm without the none note exits 1" 1 "2 value(s) unavailable" '!example-host'
pj "probe: therm without the none note is unavailable" 'd["thermal_warning"] is None and d["unavailable"] == ["power_source", "thermal_warning"]'

echo "{ 1.0 abc 3.0 }" >"${fix}/loadavg"
ms probe
expect "probe: malformed load average exits 1" 1
pj "probe: malformed load average is unavailable" 'd["loadavg"] is None and "loadavg" in d["unavailable"]'
echo "{ 1.0 2.0 }" >"${fix}/loadavg"
ms probe
pj "probe: short load average is unavailable" 'd["loadavg"] is None'

rm -f "${fix}/pmset_g"
ms probe
expect "probe: pmset failure exits 1" 1
pj "probe: pmset failure marks its values unavailable" 'd["low_power_mode"] is None and d["reference"] == {"sleep_minutes": None, "display_sleep_minutes": None, "disk_sleep_minutes": None}'

write_probe_fixtures
FAKE_DATE="2000-13-01T00:00:00Z" ms probe
expect "probe: impossible date exits 1" 1
pj "probe: impossible date is unavailable" 'd["utc"] is None and d["unavailable"] == ["utc"]'
FAKE_DATE="not-a-time example-host" ms probe
expect "probe: malformed date exits 1 and is not echoed" 1 '!example-host' '!not-a-time'
FAKE_DATE_FAIL=1 ms probe
expect "probe: date failure exits 1" 1
pj "probe: date failure is unavailable" 'd["utc"] is None and d["unavailable"] == ["utc"]'

rm -f "${fix}"/*
FAKE_DATE_FAIL=1 ms probe
expect "probe: all commands failing exits 1" 1 "8 value(s) unavailable"
pj "probe: all values null and listed" 'len(d["unavailable"]) == 8 and d["utc"] is None and d["power_source"] is None and d["loadavg"] is None and all(v is None for v in d["reference"].values())'

write_probe_fixtures
FAKE_UNAME=Linux ms probe
expect "probe: non-macOS fails" 2 "macOS only" '!{'
check "probe: non-macOS prints no JSON" "$([ ! -s "${OUT_F}" ] && echo 0 || echo 1)"
ms probe extra
expect "probe: extra argument is misuse" 2 "usage:"

# ---- run ----

devdir="${work}/dev"
mkdir "${devdir}"
devdir2="${work}/dev2"
mkdir "${devdir2}"
: >"${FAKE_LOG}"

rm -f "${FAKE_REC}"
ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test -p example
expect "run: child exit 0" 0
check "run: env reaches child" "$(printf 'COMB=metal-mac-mini\nPERM=UNSET\nDEV=%s\nARGC=3\nARG=test\nARG=-p\nARG=example\n' "${devdir}" | cmp -s - "${FAKE_REC}" && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
FANDHE_SILICON_TEST_PERMISSIONS=caller-granted ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test
expect "run: caller's permissions not passed on" 0
check "run: permissions stay unset without the option" "$(grep -qx 'PERM=UNSET' "${FAKE_REC}" && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
ms run --combination metal-mac-mini --permissions fault-injection,exclusive-gpu --developer-dir "${devdir}" -- cargo test
expect "run: permissions passed when given" 0
check "run: permissions value reaches child" "$(grep -qx 'PERM=fault-injection,exclusive-gpu' "${FAKE_REC}" && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
FANDHE_SILICON_TEST_MACHINE_COMBINATION=metal-mac-mini ms run --developer-dir "${devdir}" -- cargo test
expect "run: no default combination even if the caller's env has one" 2 "--combination is required"
check "run: child not started without combination" "$([ ! -e "${FAKE_REC}" ] && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
FANDHE_SILICON_TEST_MACHINE_COMBINATION=caller-value ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test
check "run: option overrides caller's combination" "$(grep -qx 'COMB=metal-mac-mini' "${FAKE_REC}" && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
DEVELOPER_DIR="${devdir}" ms run --combination metal-mac-mini -- cargo test
expect "run: developer dir from caller's env" 0
check "run: env developer dir reaches child" "$(grep -qx "DEV=${devdir}" "${FAKE_REC}" && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
DEVELOPER_DIR="${devdir}" ms run --combination metal-mac-mini --developer-dir "${devdir2}" -- cargo test
check "run: option overrides env developer dir" "$(grep -qx "DEV=${devdir2}" "${FAKE_REC}" && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
ms run --combination metal-mac-mini -- cargo test
expect "run: developer dir required" 2 "developer directory is required"
check "run: child not started without developer dir" "$([ ! -e "${FAKE_REC}" ] && echo 0 || echo 1)"

rm -f "${FAKE_REC}"
ms run --combination metal-mac-mini --developer-dir "${work}/no-such-dir" -- cargo test
expect "run: missing developer dir" 2 "does not exist"
check "run: child not started with missing developer dir" "$([ ! -e "${FAKE_REC}" ] && echo 0 || echo 1)"

: >"${work}/plain-file"
ms run --combination metal-mac-mini --developer-dir "${work}/plain-file" -- cargo test
expect "run: developer dir that is a file" 2 "does not exist"
ms run --combination metal-mac-mini --developer-dir "relative/dir" -- cargo test
expect "run: relative developer dir" 2 "absolute"
ms run --combination metal-mac-mini --developer-dir "" -- cargo test
expect "run: empty developer dir option falls through to required" 2 "developer directory is required"

check "run: never calls xcode-select" "$(grep -c '^xcode-select' "${FAKE_LOG}" | sed 's/^[1-9].*/1/')"

for c in 'Metal-Mac' 'metal_mac' 'metal mac' 'a;b' 'a$(b)' ''; do
  ms run --combination "${c}" --developer-dir "${devdir}" -- cargo test
  expect "run: bad combination '${c}'" 2 "error:"
done
ms run --combination "$(python3 -I -c "print('a' * 65)")" --developer-dir "${devdir}" -- cargo test
expect "run: combination over 64 bytes" 2 "too long"

for p in '' ',a' 'a,' 'a,,b' 'A' 'a b' 'a;b' 'a$(b)'; do
  ms run --combination metal-mac-mini --permissions "${p}" --developer-dir "${devdir}" -- cargo test
  expect "run: bad permissions '${p}'" 2 "error:"
done
many="$(python3 -I -c "print(','.join('p%d' % i for i in range(17)))")"
ms run --combination metal-mac-mini --permissions "${many}" --developer-dir "${devdir}" -- cargo test
expect "run: 17 permission names" 2 "too many"
ms run --combination metal-mac-mini --permissions "$(python3 -I -c "print('a' * 65)")" --developer-dir "${devdir}" -- cargo test
expect "run: permission name over 64 bytes" 2 "too long"

rm -f "${FAKE_REC}"
FAKE_CARGO_RC=7 ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test
expect "run: child exit code propagates" 7
FAKE_CARGO_RC=1 ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test
expect "run: child exit code 1 propagates" 1

rm -f "${FAKE_REC}" "${work}/pwned"
ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo 'a b' ';' "\$(touch ${work}/pwned)" '' '*' '--flag' "it's"
expect "run: odd arguments accepted" 0
check "run: odd arguments arrive as single arguments" "$(printf 'COMB=metal-mac-mini\nPERM=UNSET\nDEV=%s\nARGC=7\nARG=a b\nARG=;\nARG=$(touch %s)\nARG=\nARG=*\nARG=--flag\nARG=it'"'"'s\n' "${devdir}" "${work}/pwned" | cmp -s - "${FAKE_REC}" && echo 0 || echo 1)"
check "run: nothing was evaluated" "$([ ! -e "${work}/pwned" ] && echo 0 || echo 1)"

# スリープの抑止は既定で入り、--no-caffeinate で外す。
: >"${FAKE_LOG}"
rm -f "${FAKE_REC}"
ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test x
expect "run: runs under caffeinate by default" 0
check "run: caffeinate -i wraps the command by default" "$(grep -qx 'caffeinate -i cargo test x' "${FAKE_LOG}" && [ -e "${FAKE_REC}" ] && echo 0 || echo 1)"
: >"${FAKE_LOG}"
rm -f "${FAKE_REC}"
ms run --combination metal-mac-mini --developer-dir "${devdir}" --no-caffeinate -- cargo test x
expect "run: --no-caffeinate runs the command" 0
check "run: caffeinate not called with --no-caffeinate" "$([ ! -s "${FAKE_LOG}" ] && [ -e "${FAKE_REC}" ] && echo 0 || echo 1)"
ms run --combination metal-mac-mini --developer-dir "${devdir}" --caffeinate -- cargo test
expect "run: the old --caffeinate option is misuse" 2 "usage:"
FAKE_CARGO_RC=9 ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test
expect "run: exit code propagates through caffeinate" 9
# caffeinate が PATH に無い環境（dirname と偽の cargo だけを置いた PATH）。
bin_nocaf="${work}/bin-nocaf"
mkdir "${bin_nocaf}"
ln -s "$(command -v dirname)" "${bin_nocaf}/dirname"
cp "${bin}/cargo" "${bin_nocaf}/cargo"
rm -f "${FAKE_REC}"
PATH="${bin_nocaf}" ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test
expect "run: missing caffeinate refuses to start" 2 "caffeinate not found"
check "run: child not started without caffeinate" "$([ ! -e "${FAKE_REC}" ] && echo 0 || echo 1)"
PATH="${bin_nocaf}" ms run --combination metal-mac-mini --developer-dir "${devdir}" --no-caffeinate -- cargo test
expect "run: --no-caffeinate starts without caffeinate" 0
check "run: child started without caffeinate" "$([ -e "${FAKE_REC}" ] && echo 0 || echo 1)"

ms run --combination metal-mac-mini --developer-dir "${devdir}" -- no-such-command-example
expect "run: command not found" 2 "command not found"
ms run --combination metal-mac-mini --developer-dir "${devdir}" --
expect "run: no command after --" 2 "usage:"
ms run --combination metal-mac-mini --developer-dir "${devdir}" cargo test
expect "run: missing --" 2 "usage:"
ms run --combination
expect "run: option without value" 2 "usage:"
ms run --no-such-option -- cargo
expect "run: unknown option" 2 "usage:"

# ---- redact / scan ----

# 語の一覧は「語 TAB 印」。印は作り物（実物の語彙は機材の側の一覧が持つ）。
words="${work}/words.txt"
printf 'example-host\t<example-machine>\nexample-user\t<example-person>\na.b*c\t<example-path>\n(x|y)\t<example-pair>\n' >"${words}"
log="${work}/log.txt"
printf 'line1 example-host connected\naXbbc is not a.b*c\nuser=example-user (x|y) x y\nEXAMPLE-HOST stays\n' >"${log}"
before="$(cksum <"${log}")"
red="${work}/red.txt"

ms redact "${words}" "${log}" "${red}"
expect "redact: replaces words" 0 "ok: replaced 4 occurrence(s)" '!example-host' '!example-user' '!a.b*c' '!(x|y)' '!<example'
check "redact: each word gets its own mark" "$(grep -qF 'line1 <example-machine> connected' "${red}" && grep -qF 'is not <example-path>' "${red}" && grep -qF 'user=<example-person> <example-pair> x y' "${red}" && echo 0 || echo 1)"
check "redact: listed words are gone from output" "$(grep -qF -e 'example-host' -e 'example-user' -e 'a.b*c' -e '(x|y)' "${red}" && echo 1 || echo 0)"
check "redact: words are fixed strings, not regexes" "$(grep -qF -e 'aXbbc' "${red}" && echo 0 || echo 1)"
check "redact: matching is case sensitive" "$(grep -qF 'EXAMPLE-HOST stays' "${red}" && echo 0 || echo 1)"
check "redact: input is not modified" "$([ "$(cksum <"${log}")" = "${before}" ] && echo 0 || echo 1)"
check "redact: output mode is 600" "$(python3 -I -c 'import os, stat, sys
sys.exit(0 if stat.S_IMODE(os.stat(sys.argv[1]).st_mode) == 0o600 else 1)' "${red}" && echo 0 || echo 1)"

ms scan "${words}" "${red}"
expect "scan: passes after redaction" 0 "ok: no listed word or IP address/UUID-shaped value remains in 1 file(s)"

ms scan "${words}" "${log}"
expect "scan: finds remaining words" 1 "input 1: 4 residual occurrence(s) on line(s) 1, 2, 3" "FAILED: 4 residual occurrence(s)" '!example-host' '!example-user' '!a.b*c' '!(x|y)' '!<example'
ms scan "${words}" "${red}" "${log}"
expect "scan: reports the file index of the dirty one" 1 "input 2:" '!input 1:'
printf 'aXbbc only\n' >"${work}/regexonly.txt"
ms scan "${words}" "${work}/regexonly.txt"
expect "scan: regex characters in words are literal" 0 "ok:"
printf 'EXAMPLE-HOST only\n' >"${work}/upper.txt"
ms scan "${words}" "${work}/upper.txt"
expect "scan: case sensitive" 0 "ok:"
printf 'bin\000ary example-user bytes\n' >"${work}/bin.dat"
ms scan "${words}" "${work}/bin.dat"
expect "scan: also searches binary files" 1 "input 1: 1 residual occurrence(s) on line(s) 1"

# 件数と先頭の行番号は、多数の行に同じ語があっても正しい（行番号は上限まで）。
python3 -I -c 'import sys
open(sys.argv[1], "w").write("example-host\n" * 5000)
open(sys.argv[2], "w").write("example-host example-host\nplain\n" * 25)' "${work}/many.txt" "${work}/many2.txt"
ms scan "${words}" "${work}/many.txt"
expect "scan: many matching lines" 1 "input 1: 5000 residual occurrence(s) on line(s) 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20" "FAILED: 5000 residual occurrence(s)" '!, 21'
ms scan "${words}" "${work}/many2.txt"
expect "scan: several matches per line counted once per line" 1 "input 1: 50 residual occurrence(s) on line(s) 1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 39" '!, 41'

ms redact "${words}" "${log}" "${red}"
expect "redact: existing output is not overwritten" 2 "output already exists"
check "redact: existing output kept" "$(grep -qF 'user=<example-person>' "${red}" && echo 0 || echo 1)"
ms redact "${words}" "${log}" "${log}"
expect "redact: output equal to input is refused" 2 "output already exists"
check "redact: input kept when output equals input" "$([ "$(cksum <"${log}")" = "${before}" ] && echo 0 || echo 1)"
ln -s "${work}/dangling-target" "${work}/link-out"
ms redact "${words}" "${log}" "${work}/link-out"
expect "redact: output symlink is refused" 2 "output already exists"
check "redact: symlink target not created" "$([ ! -e "${work}/dangling-target" ] && echo 0 || echo 1)"
ms redact "${words}" "${log}" "${work}/no-such-dir/out.txt"
expect "redact: output directory must exist" 2 "output directory does not exist"

printf 'ab\t<m-a>\nabc\t<m-b>\n' >"${work}/w-overlap.txt"
printf 'abcd\n' >"${work}/in-overlap.txt"
ms redact "${work}/w-overlap.txt" "${work}/in-overlap.txt" "${work}/out-overlap.txt"
check "redact: longest word wins" "$([ "$(cat "${work}/out-overlap.txt")" = "<m-b>d" ] && echo 0 || echo 1)"

printf 'b\t<m-x>\n>c\t<m-y>\n' >"${work}/w-edge.txt"
printf 'bc\n' >"${work}/in-edge.txt"
ms redact "${work}/w-edge.txt" "${work}/in-edge.txt" "${work}/out-edge.txt"
expect "redact: a word formed at the mark boundary is refused" 1 "would remain" '!>c' '!<m-'
check "redact: no output when a word would remain" "$([ ! -e "${work}/out-edge.txt" ] && echo 0 || echo 1)"

printf 'example-host\t<example-machine>\r\nexample-user\t<example-person>\r\n' >"${work}/w-crlf.txt"
ms redact "${work}/w-crlf.txt" "${log}" "${work}/out-crlf.txt"
expect "redact: CRLF word list" 0 "ok: replaced 2 occurrence(s)"

printf 'example-host\t<example-machine>\n\n\nexample-user\t<example-machine>\n\n' >"${work}/w-blank.txt"
ms redact "${work}/w-blank.txt" "${log}" "${work}/out-blank.txt"
expect "redact: blank lines in the list are skipped, marks may repeat" 0 "ok: replaced 2 occurrence(s)"
check "redact: two words with the same mark" "$(grep -qF 'user=<example-machine>' "${work}/out-blank.txt" && echo 0 || echo 1)"

# 語の一覧の形の拒否
: >"${work}/w-empty.txt"
ms redact "${work}/w-empty.txt" "${log}" "${work}/o1.txt"
expect "words: empty list refused" 2 "no words"
printf '\n\n\n' >"${work}/w-nl.txt"
ms redact "${work}/w-nl.txt" "${log}" "${work}/o2.txt"
expect "words: newline-only list refused" 2 "no words"
ms scan "${work}/w-empty.txt" "${log}"
expect "words: empty list refused by scan" 2 "no words"
check "words: refused list leaves no output" "$([ ! -e "${work}/o1.txt" ] && [ ! -e "${work}/o2.txt" ] && echo 0 || echo 1)"
printf 'example-host\n' >"${work}/w-nomark.txt"
ms scan "${work}/w-nomark.txt" "${log}"
expect "words: line without a mark refused by scan" 2 "without a mark" '!example-host'
ms redact "${work}/w-nomark.txt" "${log}" "${work}/o7.txt"
expect "words: line without a mark refused by redact" 2 "without a mark" '!example-host'
check "words: no default mark is used" "$([ ! -e "${work}/o7.txt" ] && echo 0 || echo 1)"
printf 'example-host\t<example-machine>\nexample-user\n' >"${work}/w-nomark2.txt"
ms scan "${work}/w-nomark2.txt" "${log}"
expect "words: one line without a mark refuses the whole list" 2 "without a mark"
for m in 'x' '<>' '<a b>' '<a_b>' '<a>x' ' <a>' '<a>	<b>' '<a.b>'; do
  printf 'example-host\t%s\n' "${m}" >"${work}/w-badmark.txt"
  ms scan "${work}/w-badmark.txt" "${log}"
  expect "words: malformed mark '${m}'" 2 "malformed mark" '!example-host'
done
printf '\t<a>\n' >"${work}/w-noword.txt"
ms scan "${work}/w-noword.txt" "${log}"
expect "words: empty word refused" 2 "empty or whitespace-only word"
printf '  \t<a>\n' >"${work}/w-ws.txt"
ms scan "${work}/w-ws.txt" "${log}"
expect "words: whitespace-only word refused" 2 "empty or whitespace-only word"
printf 'example-host\t<example-machine>\nexample-host\t<example-machine>\n' >"${work}/w-dup1.txt"
ms scan "${work}/w-dup1.txt" "${log}"
expect "words: duplicate word with the same mark refused" 2 "duplicate word" '!example-host'
printf 'example-host\t<example-machine>\nexample-host\t<example-other>\n' >"${work}/w-dup2.txt"
ms scan "${work}/w-dup2.txt" "${log}"
expect "words: duplicate word with another mark refused" 2 "duplicate word" '!example-host'
printf 'example\t<example-machine>\n' >"${work}/w-mark.txt"
ms scan "${work}/w-mark.txt" "${log}"
expect "words: a word inside a mark refused" 2 "replacement mark" '!example' '!<example'
printf 'example-host\t<example-machine>\nm\t<a-b>\n' >"${work}/w-mark2.txt"
ms redact "${work}/w-mark2.txt" "${log}" "${work}/o8.txt"
expect "words: a word inside another word's mark refused" 2 "replacement mark"
check "words: refused lists leave no output" "$([ ! -e "${work}/o8.txt" ] && echo 0 || echo 1)"
ms scan "${work}/no-such-words.txt" "${log}"
expect "words: missing file" 2 "not found"
ms scan "${work}" "${log}"
expect "words: directory" 2 "not a regular file"
printf '\377\t<a>\n' >"${work}/w-bad.txt"
ms scan "${work}/w-bad.txt" "${log}"
expect "words: invalid UTF-8" 2 "not UTF-8"
printf '\357\273\277example-host\t<example-machine>\n' >"${work}/w-bom.txt"
ms scan "${work}/w-bom.txt" "${log}"
expect "words: BOM" 2 "BOM"
sparse "${work}/w-big.txt" 1048577
ms scan "${work}/w-big.txt" "${log}"
expect "words: over size limit" 2 "too large"

# git の作業ツリーの中の一覧と出力先の拒否。
gitq() {
  git -c user.name=example -c user.email=example -c commit.gpgsign=false "$@"
}
repo_a="${work}/repo-a"
gitq init -q "${repo_a}"
mkdir "${repo_a}/sub"
for f in tracked untracked ignored; do
  cp "${words}" "${repo_a}/${f}.txt"
done
cp "${words}" "${repo_a}/sub/nested.txt"
printf 'ignored.txt\n' >"${repo_a}/.gitignore"
gitq -C "${repo_a}" add tracked.txt .gitignore
gitq -C "${repo_a}" commit -q -m example
for f in tracked untracked ignored sub/nested; do
  ms scan "${repo_a}/${f}.txt" "${log}"
  expect "words: ${f} file inside a git work tree refused" 2 "outside any git work tree"
done
ln -s "${repo_a}/tracked.txt" "${work}/w-link.txt"
ms scan "${work}/w-link.txt" "${log}"
expect "words: symlink into a git work tree refused" 2 "outside any git work tree"
gitq -C "${repo_a}" worktree add -q "${work}/repo-wt" -b example-branch
ms scan "${work}/repo-wt/tracked.txt" "${log}"
expect "words: file in another worktree refused" 2 "outside any git work tree"
cp "${words}" "${repo_a}/.git/words.txt"
ms scan "${repo_a}/.git/words.txt" "${log}"
expect "words: file inside the .git directory refused" 2 "outside any git work tree"
mkdir "${work}/fake-repo"
: >"${work}/fake-repo/.git"
cp "${words}" "${work}/fake-repo/words.txt"
ms scan "${work}/fake-repo/words.txt" "${log}"
expect "words: ancestor with a .git entry refused even when git cannot tell" 2 "outside any git work tree"
ms scan "${ROOT_DIR}/README.md" "${log}"
expect "words: file inside this repository refused" 2 "outside any git work tree"
ms redact "${words}" "${log}" "${repo_a}/out.txt"
expect "redact: output inside a git work tree refused" 2 "output must be outside any git work tree"
check "redact: nothing written inside the work tree" "$([ ! -e "${repo_a}/out.txt" ] && echo 0 || echo 1)"
ms redact "${words}" "${log}" "${work}/repo-wt/out.txt"
expect "redact: output inside another worktree refused" 2 "output must be outside any git work tree"
ms redact "${words}" "${log}" "${work}/fake-repo/out.txt"
expect "redact: output below a .git entry refused" 2 "output must be outside any git work tree"
ms scan "${words}" "${red}"
expect "words: file outside any work tree accepted" 0 "ok:"

# 入力の拒否
ms scan "${words}" "${work}/no-such-input.txt"
expect "input: missing" 2 "not found"
ms scan "${words}" "${work}"
expect "input: directory" 1 "not a regular file" "FAILED: 1 violation(s)"
mkfifo "${work}/fifo"
ms scan "${words}" "${work}/fifo"
expect "input: fifo" 1 "not a regular file"
ms redact "${words}" "${work}/bin.dat" "${work}/o3.txt"
expect "input: redact refuses binary" 1 "not text"
printf '\377\n' >"${work}/in-bad.txt"
ms redact "${words}" "${work}/in-bad.txt" "${work}/o4.txt"
expect "input: redact refuses invalid UTF-8" 1 "not text"
sparse "${work}/in-big.txt" $((64 * 1024 * 1024 + 1))
ms redact "${words}" "${work}/in-big.txt" "${work}/o5.txt"
expect "input: redact over size limit" 1 "too large"
ms scan "${words}" "${work}/in-big.txt"
expect "input: scan over size limit" 1 "too large"
check "input: refused inputs leave no output" "$([ ! -e "${work}/o3.txt" ] && [ ! -e "${work}/o4.txt" ] && [ ! -e "${work}/o5.txt" ] && echo 0 || echo 1)"

# ---- 印の英大文字 ----

# 機材の別名は英大文字を含む（印は英字の大小・数字・ハイフンだけで作る）。
printf 'example-host\t<Node-A2>\nexample-user\t<example-person>\n' >"${work}/w-upper.txt"
ms redact "${work}/w-upper.txt" "${log}" "${work}/out-upper.txt"
expect "marks: a mark with upper-case letters is accepted" 0 "ok: replaced 2 occurrence(s)" '!Node-A2'
check "marks: the upper-case mark is written as given" "$(grep -qF 'line1 <Node-A2> connected' "${work}/out-upper.txt" && echo 0 || echo 1)"
ms scan "${work}/w-upper.txt" "${work}/out-upper.txt"
expect "marks: scan passes on output with an upper-case mark" 0 "ok:"
for m in '<<a>>' '<a>>' '<a-B_c>' '<A B>' '<Ä>'; do
  printf 'example-host\t%s\n' "${m}" >"${work}/w-badmark2.txt"
  ms scan "${work}/w-badmark2.txt" "${log}"
  expect "marks: malformed mark '${m}' stays refused" 2 "malformed mark" '!example-host'
done
printf 'example-host\t<00000000-0000-4000-8000-0000000000ab>\n' >"${work}/w-uuidmark.txt"
ms scan "${work}/w-uuidmark.txt" "${log}"
expect "marks: a mark shaped like a UUID is refused" 2 "mark with the shape" '!example-host'

# ---- 形で拾う値（IPv4・IPv6・UUID）----

# 値は文書用の予約済みの範囲と作り物の UUID。リテラルを避けるため、DOT と COL と FW（全角の点）を
# 実行時に置き換える。表の「拾う」行は種類ごとの行番号が出力に出ること、「拾わない」行は止まらないことを固定する。
shapes="${work}/shapes.txt"
passfile="${work}/passes.txt"
exp="${work}/shapes-expected.txt"
python3 -I - "${shapes}" "${passfile}" "${exp}" <<'PY'
import sys

HIT = [
    ("ipv4", "peer 192DOT0DOT2DOT7 up"),
    ("ipv4", "peer 198DOT51DOT100DOT9COL8080 up"),
    ("ipv4", "see 203DOT0DOT113DOT5."),
    ("ipv4", "version 1DOT2DOT3DOT4"),
    ("ipv4", "build 1DOT2DOT3DOT4DOT5"),
    ("ipv4", "port by dot 192DOT0DOT2DOT7DOT8080"),
    ("ipv4", "glued x1DOT192DOT0DOT2DOT7"),
    ("ipv4", "zero padded 010DOT000DOT002DOT007"),
    ("ipv4", "over range 256DOT1DOT1DOT1"),
    ("ipv4", "long digits 1234DOT5DOT6DOT7"),
    ("ipv6", "addr 2001COLdb8COLCOL1"),
    ("ipv6", "addr 2001COLdb8COL0COL0COL0COL0COL0COL1"),
    ("ipv6", "lo COLCOL1"),
    ("ipv6", "[2001COLdb8COLCOL1]COL443"),
    ("ipv6", "link fe80COLCOL1%en0"),
    ("ipv6", "eui 00COL11COL22COL33COL44COL55COL66COL77"),
    ("ipv6", "id aCOLCOLb"),
    ("ipv6", "addrCOL2001COLdb8COLCOL1"),
    ("ipv6", "IPv6COL2001COLdb8COLCOL1"),
    ("ipv6", "peer 2001COLdb8COLCOL1COL refused"),
    ("ipv6", "peer COL 2001COLdb8COLCOL1COL"),
    ("ipv6", "use stdCOLCOL{fmt}"),
    ("uuid", "gpu 00000000-0000-4000-8000-0000000000ab"),
    ("uuid", "GPU-AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"),
    ("uuid", "hex 0x00000000-0000-4000-8000-0000000000ab"),
    ("uuid", "tail 00000000-0000-4000-8000-0000000000ab0"),
    ("uuid", "head 000000000-0000-4000-8000-0000000000ab"),
    ("uuid", "head x00000000-0000-4000-8000-0000000000ab"),
]
MISS = [
    "three parts 1DOT2DOT3 and two 12DOT34",
    "names aDOTbDOTcDOTd and 1DOTDOT2DOT3DOT4 and 1 DOT2DOT3DOT4",
    "fullwidth 192FWFW0FW2FW7 and 2001FWdb8FWFW1",
    "t 12COL34COL56 and 23COL59COL59",
    "mac 00COL11COL22COL33COL44COL55",
    "path stdCOLCOLfmtCOLCOLDisplay and FooCOLCOLBar",
    "two compressions 1COLCOL2COLCOL3 and COLCOLCOL and COLCOL",
    "keyCOL value and a COL b",
    "sha 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "uuid without hyphens 0123456789abcdef0123456789abcdef",
    "short uuid 00000000-0000-4000-8000 and 0000000-0000-4000-8000-0000000000ab",
    "plain text only",
]


def fx(t):
    return t.replace("DOT", ".").replace("COL", ":").replace("FW", "．")


lines = [t for _, t in HIT] + MISS
with open(sys.argv[1], "w", encoding="utf-8") as f:
    f.write(fx("\n".join(lines)) + "\n")
with open(sys.argv[2], "w", encoding="utf-8") as f:
    f.write(fx("\n".join(MISS)) + "\n")
out = []
for kind in ("ipv4", "ipv6", "uuid"):
    ln = [i + 1 for i, (k, _) in enumerate(HIT) if k == kind]
    out.append("input 1: %d %s-shaped value(s) on line(s) %s" % (len(ln), kind, ", ".join(map(str, ln))))
out.append("FAILED: %d residual occurrence(s)" % len(HIT))
open(sys.argv[3], "w").write("\n".join(out) + "\n")
PY
ms scan "${words}" "${shapes}"
expect "shapes: each kind reports its own lines" 1 "$(sed -n 1p "${exp}")" "$(sed -n 2p "${exp}")" "$(sed -n 3p "${exp}")" "$(sed -n 4p "${exp}")" '!word' '!192' '!2001' '!fe80' '!0000'
ms scan "${words}" "${shapes}" "${log}"
expect "shapes: file index of the shaped file" 1 "input 2: 4 residual occurrence(s) on line(s) 1, 2, 3" '!input 2: 4 ipv4' "input 1: 10 ipv4-shaped"
ms scan "${words}" "${passfile}"
expect "shapes: look-alikes and unsupported spellings pass" 0 "ok: no listed word or IP address/UUID-shaped value remains in 1 file(s)"

# 形の検出は語の一覧の検索と併せて数える。
python3 -I - "${work}/mixed.txt" <<'PY'
import sys

open(sys.argv[1], "w").write("example-host at 192DOT0DOT2DOT7\n".replace("DOT", "."))
PY
ms scan "${words}" "${work}/mixed.txt"
expect "shapes: words and shapes are both reported" 1 "input 1: 1 residual occurrence(s) on line(s) 1" "input 1: 1 ipv4-shaped value(s) on line(s) 1" "FAILED: 2 residual occurrence(s)" '!example-host' '!192'

# redact は形で拾う値を置き換えない。残りを知らせ、scan が止める。
ms redact "${words}" "${work}/mixed.txt" "${work}/mixed-out.txt"
expect "shapes: redact leaves shapes and notes them without values" 0 "ok: replaced 1 occurrence(s)" "note: values of IP address or UUID shape are not replaced (ipv4: 1)" '!192'
check "shapes: redact output keeps the shaped value" "$(grep -q '^<example-machine> at 192\.0\.2\.7$' "${work}/mixed-out.txt" && echo 0 || echo 1)"
ms scan "${words}" "${work}/mixed-out.txt"
expect "shapes: scan stops on redact output with a shape" 1 "input 1: 1 ipv4-shaped value(s) on line(s) 1" '!192'

# 多数の行・大きな入力でも結果が正しい（行番号は上限まで。数え方は線形）。
python3 -I - "${work}/ipmany.txt" "${work}/big.txt" <<'PY'
import sys

open(sys.argv[1], "w").write("x 198DOT51DOT100DOT1 y\n".replace("DOT", ".") * 5000)
chunk = ("ts 12COL34COL56 mac 00COL11COL22COL33COL44COL55 v 1DOT2DOT3 std COLCOLx " + "ab" * 20 + "\n").replace("DOT", ".").replace("COL", ":")
n = (60 * 1024 * 1024) // len(chunk)
with open(sys.argv[2], "w") as f:
    f.write(chunk * n)
    f.write("aaaa:" * 1000 + "\n")
    f.write("last 198DOT51DOT100DOT7\n".replace("DOT", "."))
PY
ms scan "${words}" "${work}/ipmany.txt"
expect "shapes: many shaped lines" 1 "input 1: 5000 ipv4-shaped value(s) on line(s) 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20" "FAILED: 5000 residual occurrence(s)" '!, 21'
ms scan "${words}" "${work}/big.txt"
expect "shapes: 60 MiB input finds the one shaped value on its line" 1 "input 1: 1 ipv4-shaped value(s) on line(s)" "FAILED: 1 residual occurrence(s)" '!ipv6' '!uuid'

# 止まった値を語の一覧に印つきで足して redact をやり直すと、置き換わって scan が通る。
python3 -I - "${work}/flow.txt" "${work}/flow-words.txt" <<'PY'
import sys

v = "192DOT0DOT2DOT7".replace("DOT", ".")
open(sys.argv[1], "w").write("example-host at %s\n" % v)
open(sys.argv[2], "w").write("example-host\t<example-machine>\n%s\t<example-address>\n" % v)
PY
ms scan "${work}/flow-words.txt" "${work}/flow.txt"
expect "flow: scan stops while the value is not in the list" 1 "FAILED:"
ms redact "${work}/flow-words.txt" "${work}/flow.txt" "${work}/flow-out.txt"
expect "flow: redact with the value added replaces it" 0 "ok: replaced 2 occurrence(s)" '!note:' '!192'
check "flow: the value became the mark" "$([ "$(cat "${work}/flow-out.txt")" = "<example-machine> at <example-address>" ] && echo 0 || echo 1)"
ms scan "${work}/flow-words.txt" "${work}/flow-out.txt"
expect "flow: scan passes after the rerun" 0 "ok:"

# 長い 1 行（16 MiB）とコロン・ドットだらけの入力で、結果が正しい（後戻りで止まらない）。
python3 -I - "${work}" <<'PY'
import os
import sys

d = sys.argv[1]
n = 16 * 1024 * 1024
open(os.path.join(d, "long-colon-a.txt"), "w").write("a:" * (n // 2) + "\n")
open(os.path.join(d, "long-colons.txt"), "w").write(":" * n + "\n")
open(os.path.join(d, "long-hex.txt"), "w").write("a" * n + "\n")
open(os.path.join(d, "long-dots.txt"), "w").write("1." * (n // 2) + "\n")
open(os.path.join(d, "long-dotnum.txt"), "w").write("9" * n + "." * 3 + "\n")
open(os.path.join(d, "long-dash.txt"), "w").write("0000000-" * (n // 8) + "\n")
PY
for f in long-colon-a long-colons long-hex long-dotnum long-dash; do
  ms scan "${words}" "${work}/${f}.txt"
  expect "perf: 16 MiB line ${f} is not shaped" 0 "ok:"
done
ms scan "${words}" "${work}/long-dots.txt"
expect "perf: 16 MiB line of dotted numbers is stopped once per line" 1 "ipv4-shaped value(s) on line(s) 1" '!, 2'

# 誤用
ms redact "${words}" "${log}"
expect "misuse: redact with 2 arguments" 2 "usage:"
ms redact "${words}" "${log}" "${work}/o6.txt" extra
expect "misuse: redact with 4 arguments" 2 "usage:"
ms scan "${words}"
expect "misuse: scan without files" 2 "usage:"
ms
expect "misuse: no subcommand" 2 "usage:"
ms no-such-subcommand
expect "misuse: unknown subcommand" 2 "usage:"

# ---- 綴りの同期 ----

# 環境変数の名前と組み合わせの名前が、Rust 側（試験の枠）と本体で揃っていること。
# 文字列の存在だけを見る片方向の検査で、どちらかの改名でこのテストが落ちることが目的。
c_env="FANDHE_SILICON_TEST_MACHINE_COMBINATION"
p_env="FANDHE_SILICON_TEST_PERMISSIONS"
check "sync: combination env name matches setting.rs" "$(grep -qF "pub const COMBINATION_ENV: &str = \"${c_env}\";" "${SETTING_RS}" && grep -qF "COMBINATION_ENV=\"${c_env}\"" "${MS}" && echo 0 || echo 1)"
check "sync: permissions env name matches setting.rs" "$(grep -qF "pub const PERMISSIONS_ENV: &str = \"${p_env}\";" "${SETTING_RS}" && grep -qF "PERMISSIONS_ENV=\"${p_env}\"" "${MS}" && echo 0 || echo 1)"
check "sync: usage example name matches combination.rs" "$(grep -qF 'MachineCombination::MetalMacMini => "metal-mac-mini"' "${COMBINATION_RS}" && grep -qF 'example name: metal-mac-mini' "${MS}" && echo 0 || echo 1)"

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
