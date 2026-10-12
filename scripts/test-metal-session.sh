#!/usr/bin/env bash
# metal-session.sh の回帰テスト（spec TASK-19・TASK-28 のポインタ）。
# macOS のコマンド（uname・pmset・sysctl・caffeinate・xcode-select・cargo）は、一時ディレクトリに置く
# 偽のコマンドを PATH の先頭に入れて差し替えるので、Linux の CI で全部流れる。
# 偽のデータは example と分かる形（ホスト名・ユーザー名・プロセス名に見える文字列は、出力に現れないことの確認用）。
# 呼び出し元: `make test-metal-session`（直接実行も可。CI の `deny` ジョブと `test-hooks` ジョブ〔macOS を含む〕も呼ぶ）。
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
  "${MS}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
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
  "low_power_mode": False, "sleep_minutes": 0, "display_sleep_minutes": 10,
  "disk_sleep_minutes": 10, "power_source": "ac", "thermal_warning_recorded": False,
  "load_average": [1.5, 2.25, 3.0], "unavailable": [],
  "not_probed": ["resident_process_stopped", "gui_login_state"]}'
expect "probe: noise from commands not echoed" 0 '!example-host' '!example-user' '!example-daemon' '!example-serial' '!invalid'
bad_calls=0
while IFS= read -r line; do
  case "${line}" in
    "pmset -g" | "pmset -g batt" | "pmset -g therm" | "sysctl -n vm.loadavg") ;;
    *) bad_calls=1 ;;
  esac
done <"${FAKE_LOG}"
check "probe: only read-only queries issued" "${bad_calls}"
check "probe: never calls xcode-select" "$(grep -c '^xcode-select' "${FAKE_LOG}" | sed 's/^[1-9].*/1/')"

sed 's/lowpowermode         0/lowpowermode         1/' "${fix}/pmset_g" >"${work}/g.tmp"
mv "${work}/g.tmp" "${fix}/pmset_g"
ms probe
pj "probe: low power mode on" 'd["low_power_mode"] is True and d["unavailable"] == []'

sed 's/lowpowermode         1/lowpowermode         2/' "${fix}/pmset_g" >"${work}/g.tmp"
mv "${work}/g.tmp" "${fix}/pmset_g"
ms probe
pj "probe: out-of-range low power value is unavailable" 'd["low_power_mode"] is None and d["unavailable"] == ["low_power_mode"]'

sed 's/lowpowermode         2/lowpowermode         0/' "${fix}/pmset_g" >"${work}/g.tmp"
mv "${work}/g.tmp" "${fix}/pmset_g"
echo "Now drawing from 'Battery Power'" >"${fix}/pmset_batt"
ms probe
pj "probe: battery power" 'd["power_source"] == "battery"'
echo "Now drawing from 'Example Power'" >"${fix}/pmset_batt"
ms probe
pj "probe: unknown power source is unavailable" 'd["power_source"] is None and d["unavailable"] == ["power_source"]'

echo "Note: something else example-host" >"${fix}/pmset_therm"
ms probe
pj "probe: therm without the none note is unavailable" 'd["thermal_warning_recorded"] is None and d["unavailable"] == ["power_source", "thermal_warning_recorded"]'
expect "probe: warns on stderr when values are unavailable" 0 "warning: 2 value(s) unavailable" '!example-host'

echo "{ 1.0 abc 3.0 }" >"${fix}/loadavg"
ms probe
pj "probe: malformed load average is unavailable" 'd["load_average"] is None and "load_average" in d["unavailable"]'
echo "{ 1.0 2.0 }" >"${fix}/loadavg"
ms probe
pj "probe: short load average is unavailable" 'd["load_average"] is None'

rm -f "${fix}/pmset_g"
ms probe
pj "probe: pmset failure marks its values unavailable" 'd["low_power_mode"] is None and d["sleep_minutes"] is None and d["display_sleep_minutes"] is None and d["disk_sleep_minutes"] is None'

rm -f "${fix}"/*
ms probe
expect "probe: all commands failing still exits 0 with a warning" 0 "warning: 7 value(s) unavailable"
pj "probe: all values null and listed" 'len(d["unavailable"]) == 7 and all(d[k] is None for k in d["unavailable"])'

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

: >"${FAKE_LOG}"
rm -f "${FAKE_REC}"
ms run --combination metal-mac-mini --developer-dir "${devdir}" --caffeinate -- cargo test x
expect "run: with caffeinate" 0
check "run: caffeinate -i wraps the command" "$(grep -qx 'caffeinate -i cargo test x' "${FAKE_LOG}" && [ -e "${FAKE_REC}" ] && echo 0 || echo 1)"
: >"${FAKE_LOG}"
ms run --combination metal-mac-mini --developer-dir "${devdir}" -- cargo test
check "run: caffeinate unused without the option" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"
FAKE_CARGO_RC=9 ms run --combination metal-mac-mini --developer-dir "${devdir}" --caffeinate -- cargo test
expect "run: exit code propagates through caffeinate" 9

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

words="${work}/words.txt"
printf 'example-host\nexample-user\na.b*c\n(x|y)\n' >"${words}"
log="${work}/log.txt"
printf 'line1 example-host connected\naXbbc is not a.b*c\nuser=example-user (x|y) x y\nEXAMPLE-HOST stays\n' >"${log}"
before="$(cksum <"${log}")"
red="${work}/red.txt"

ms redact "${words}" "${log}" "${red}"
expect "redact: replaces words" 0 "ok: replaced 4 occurrence(s)" '!example-host' '!example-user' '!a.b*c' '!(x|y)'
check "redact: output has the mark" "$(grep -qF '<redacted>' "${red}" && echo 0 || echo 1)"
check "redact: listed words are gone from output" "$(grep -qF -e 'example-host' -e 'example-user' -e 'a.b*c' -e '(x|y)' "${red}" && echo 1 || echo 0)"
check "redact: words are fixed strings, not regexes" "$(grep -qF -e 'aXbbc' "${red}" && grep -qF 'user=<redacted> <redacted> x y' "${red}" && echo 0 || echo 1)"
check "redact: matching is case sensitive" "$(grep -qF 'EXAMPLE-HOST stays' "${red}" && echo 0 || echo 1)"
check "redact: input is not modified" "$([ "$(cksum <"${log}")" = "${before}" ] && echo 0 || echo 1)"
check "redact: output mode is 600" "$(python3 -I -c 'import os, stat, sys
sys.exit(0 if stat.S_IMODE(os.stat(sys.argv[1]).st_mode) == 0o600 else 1)' "${red}" && echo 0 || echo 1)"

ms scan "${words}" "${red}"
expect "scan: passes after redaction" 0 "ok: no listed word remains in 1 file(s)"

ms scan "${words}" "${log}"
expect "scan: finds remaining words" 1 "input 1: 4 residual occurrence(s) on line(s) 1, 2, 3" "FAILED: 4 residual occurrence(s)" '!example-host' '!example-user' '!a.b*c' '!(x|y)'
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

ms redact "${words}" "${log}" "${red}"
expect "redact: existing output is not overwritten" 2 "output already exists"
check "redact: existing output kept" "$(grep -qF 'user=<redacted>' "${red}" && echo 0 || echo 1)"
ms redact "${words}" "${log}" "${log}"
expect "redact: output equal to input is refused" 2 "output already exists"
check "redact: input kept when output equals input" "$([ "$(cksum <"${log}")" = "${before}" ] && echo 0 || echo 1)"
ln -s "${work}/dangling-target" "${work}/link-out"
ms redact "${words}" "${log}" "${work}/link-out"
expect "redact: output symlink is refused" 2 "output already exists"
check "redact: symlink target not created" "$([ ! -e "${work}/dangling-target" ] && echo 0 || echo 1)"
ms redact "${words}" "${log}" "${work}/no-such-dir/out.txt"
expect "redact: output directory must exist" 2 "output directory does not exist"

printf 'ab\nabc\n' >"${work}/w-overlap.txt"
printf 'abcd\n' >"${work}/in-overlap.txt"
ms redact "${work}/w-overlap.txt" "${work}/in-overlap.txt" "${work}/out-overlap.txt"
check "redact: longest word wins" "$([ "$(cat "${work}/out-overlap.txt")" = "<redacted>d" ] && echo 0 || echo 1)"

printf 'b\n>c\n' >"${work}/w-edge.txt"
printf 'bc\n' >"${work}/in-edge.txt"
ms redact "${work}/w-edge.txt" "${work}/in-edge.txt" "${work}/out-edge.txt"
expect "redact: a word formed at the mark boundary is refused" 1 "would remain" '!>c'
check "redact: no output when a word would remain" "$([ ! -e "${work}/out-edge.txt" ] && echo 0 || echo 1)"

printf 'example-host\r\nexample-user\r\n' >"${work}/w-crlf.txt"
ms redact "${work}/w-crlf.txt" "${log}" "${work}/out-crlf.txt"
expect "redact: CRLF word list" 0 "ok: replaced 2 occurrence(s)"

printf 'example-host\n\n\nexample-user\n\n' >"${work}/w-blank.txt"
ms redact "${work}/w-blank.txt" "${log}" "${work}/out-blank.txt"
expect "redact: blank lines in the list are skipped" 0 "ok: replaced 2 occurrence(s)"

# 語の一覧の拒否
: >"${work}/w-empty.txt"
ms redact "${work}/w-empty.txt" "${log}" "${work}/o1.txt"
expect "words: empty list refused" 2 "no words"
printf '\n\n\n' >"${work}/w-nl.txt"
ms redact "${work}/w-nl.txt" "${log}" "${work}/o2.txt"
expect "words: newline-only list refused" 2 "no words"
ms scan "${work}/w-empty.txt" "${log}"
expect "words: empty list refused by scan" 2 "no words"
check "words: refused list leaves no output" "$([ ! -e "${work}/o1.txt" ] && [ ! -e "${work}/o2.txt" ] && echo 0 || echo 1)"
printf 'example-host\n  \n' >"${work}/w-ws.txt"
ms scan "${work}/w-ws.txt" "${log}"
expect "words: whitespace-only line refused" 2 "whitespace-only"
printf 'red\n' >"${work}/w-mark.txt"
ms scan "${work}/w-mark.txt" "${log}"
expect "words: a word inside the mark refused" 2 "replacement mark" '!red'
ms scan "${work}/no-such-words.txt" "${log}"
expect "words: missing file" 2 "not found"
ms scan "${work}" "${log}"
expect "words: directory" 2 "not a regular file"
printf '\377\n' >"${work}/w-bad.txt"
ms scan "${work}/w-bad.txt" "${log}"
expect "words: invalid UTF-8" 2 "not UTF-8"
printf '\357\273\277example-host\n' >"${work}/w-bom.txt"
ms scan "${work}/w-bom.txt" "${log}"
expect "words: BOM" 2 "BOM"
sparse "${work}/w-big.txt" 1048577
ms scan "${work}/w-big.txt" "${log}"
expect "words: over size limit" 2 "too large"

# リポの中の一覧は拒否する（既存のファイルを読むだけ。リポのファイルは変更しない）。
ms scan "${ROOT_DIR}/README.md" "${log}"
expect "words: file inside the repository refused" 2 "outside the repository"
ln -s "${ROOT_DIR}/README.md" "${work}/w-link.txt"
ms scan "${work}/w-link.txt" "${log}"
expect "words: symlink into the repository refused" 2 "outside the repository"

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
