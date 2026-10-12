#!/usr/bin/env bash
# session-record-check.sh の回帰テスト（spec TASK-19 のポインタ）。
# 一時ディレクトリの fixture（合成のダミー値）で、終了コード・標準出力・標準エラーの文言・
# 違反の件数・入力の値の秘匿・入力を変更しないことを確かめる。
# 呼び出し元: `make test-session-record-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致 + FAILED の件数」で行う。終了コードだけでは
# 別の理由で落ちても気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として直す。
# 途中で止めず、最後に件数を出す。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

CHECK="${DIR}/session-record-check.sh"
EXAMPLE="${ROOT_DIR}/docs/session-record.example.json"
RUST_SRC="${ROOT_DIR}/crates/core/src/test_frame/combination.rs"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-session-record-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
IN_F="${work}/rec.json"
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
MUT_PY="${work}/mut.py"
passed=0
failed=0
rc=0

# 記入例から 1 か所だけ変えた fixture を作る Python。式は合成の固定文字列（d は辞書）。
cat >"${MUT_PY}" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]), object_pairs_hook=lambda p: dict(p))
exec(sys.argv[3])
with open(sys.argv[2], "w", encoding="utf-8") as f:
    json.dump(d, f)
PY

# ---- 土台 ----

report_ok() {
  passed=$((passed + 1))
  echo "ok: $1"
}

report_fail() {
  failed=$((failed + 1))
  echo "FAIL: $1: $2" >&2
  echo "--- stderr ---" >&2
  sed 's/^/    /' "${ERR_F}" >&2 || true
}

# run_check <引数...>: 検査を実行し rc・標準出力・標準エラーを控える。
run_check() {
  rc=0
  "${BASH}" "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# judge <名前> <期待コード> <期待の標準出力> [部分一致...]: 先頭が ! の語は出力に含まれないこと。
judge() {
  local name="$1" want="$2" out_want="$3" s out
  shift 3
  out="$(cat "${OUT_F}")"
  if [ "${rc}" -ne "${want}" ]; then
    report_fail "${name}" "expected exit ${want}, got ${rc}"
    return 0
  fi
  if [ "${out}" != "${out_want}" ]; then
    report_fail "${name}" "unexpected stdout"
    return 0
  fi
  for s in "$@"; do
    case "${s}" in
      '!'*)
        if grep -qF -e "${s#!}" "${ERR_F}" "${OUT_F}"; then
          report_fail "${name}" "output must not contain: ${s#!}"
          return 0
        fi
        ;;
      *)
        if ! grep -qF -e "${s}" "${ERR_F}"; then
          report_fail "${name}" "stderr lacks: ${s}"
          return 0
        fi
        ;;
    esac
  done
  report_ok "${name}"
}

# mut <python 文>: 記入例を変えた fixture を ${IN_F} に作る。
mut() {
  python3 -I "${MUT_PY}" "${EXAMPLE}" "${IN_F}" "$1"
}

# bad <名前> <python 文> <件数> [部分一致...]: 変えた fixture が違反（1・件数一致）になること。
bad() {
  local name="$1" expr="$2" n="$3"
  shift 3
  mut "${expr}"
  run_check "${IN_F}"
  judge "${name}" 1 "" "FAILED: ${n} violation(s)" "$@"
}

# bad_raw <名前> <件数> [部分一致...]: 既に ${IN_F} に在る内容が違反になること。
bad_raw() {
  local name="$1" n="$2"
  shift 2
  run_check "${IN_F}"
  judge "${name}" 1 "" "FAILED: ${n} violation(s)" "$@"
}

misuse() {
  local name="$1"
  shift
  run_check "$@"
  judge "${name}" 2 ""
}

# ---- 合格 ----

run_check "${EXAMPLE}"
judge "pass: example in repo" 0 "ok: session record shape"

mut 'd["run_ids"] = []'
run_check "${IN_F}"
judge "pass: empty run_ids" 0 "ok: session record shape"

mut 'd = dict(reversed(list(d.items())))'
run_check "${IN_F}"
judge "pass: field order changed" 0 "ok: session record shape"

# ---- 欄の集合 ----

for f in schema_version session machine asked_utc answered_utc started_utc ended_utc \
  owner_answer run_ids interrupted low_power_mode resident_process_stopped gui_login_state; do
  bad "set: missing ${f}" "del d['${f}']" 1 "field ${f}: missing"
done

bad "set: extra field name not echoed" "d['zzextra-marker'] = 1" 1 "1 field(s) not in the table" "!zzextra-marker"

printf '{"schema_version":"1","schema_version":"1"}' >"${IN_F}"
bad_raw "set: duplicate field" 1 "duplicate field"

# ---- 型 ----

bad "type: string field is number" "d['session'] = 1" 1 "field session: invalid id"
bad "type: bool field is 0" "d['interrupted'] = 0" 1 "field interrupted: invalid bool"
bad "type: bool field is string" "d['low_power_mode'] = 'true'" 1 "field low_power_mode: invalid bool"
bad "type: run_ids not array" "d['run_ids'] = 'example-run'" 1 "field run_ids: invalid ids"
bad "type: run_ids element not string" "d['run_ids'] = [1]" 1 "field run_ids: invalid ids"
bad "type: owner_answer true" "d['owner_answer'] = True" 1 "field owner_answer: invalid posint"
bad "type: owner_answer 0" "d['owner_answer'] = 0" 1 "field owner_answer: invalid posint"
bad "type: owner_answer negative" "d['owner_answer'] = -1" 1 "field owner_answer: invalid posint"
bad "type: owner_answer float" "d['owner_answer'] = 1.5" 1 "field owner_answer: invalid posint"
bad "type: owner_answer string" "d['owner_answer'] = '1'" 1 "field owner_answer: invalid posint"

# ---- 形 ----

bad "form: unknown schema_version" "d['schema_version'] = '2'" 1 "field schema_version: invalid version"
bad "form: schema_version number" "d['schema_version'] = 1" 1 "field schema_version: invalid version"
bad "form: id uppercase" "d['session'] = 'Example'" 1 "field session: invalid id"
bad "form: id space" "d['session'] = 'a b'" 1 "field session: invalid id"
bad "form: id underscore" "d['session'] = 'a_b'" 1 "field session: invalid id"
bad "form: id leading hyphen" "d['session'] = '-a'" 1 "field session: invalid id"
bad "form: id trailing hyphen" "d['session'] = 'a-'" 1 "field session: invalid id"
bad "form: id empty" "d['session'] = ''" 1 "field session: invalid id"
bad "form: id 65 bytes" "d['session'] = 'a' * 65" 1 "field session: invalid id"
bad "form: run_ids duplicate" "d['run_ids'] = ['example-a', 'example-a']" 1 "field run_ids: invalid ids"
bad "form: run_ids bad element" "d['run_ids'] = ['Example']" 1 "field run_ids: invalid ids"
# 孤立サロゲート（JSON の \ud800 エスケープ）は traceback ではなく欄ごとの違反になる
bad "form: lone surrogate in run_ids" "d['run_ids'] = ['\ud800']" 1 "field run_ids: invalid ids" "!Traceback"
bad "form: lone surrogate in session" "d['session'] = '\ud800'" 1 "field session: invalid id" "!Traceback"
bad "form: lone surrogate in word" "d['gui_login_state'] = '\ud800'" 1 "field gui_login_state: invalid word" "!Traceback"
bad "form: machine unknown" "d['machine'] = 'example-other'" 1 "field machine: invalid machine"

for t in '2000-01-01T00:00:00' '2000-01-01T00:00:00+09:00' '2000-01-01T00:00:00.5Z' '2000-01-01' \
  '2000-1-01T00:00:00Z' '2000-01-1T00:00:00Z' '2000-01-01T0:00:00Z' '2000-02-30T00:00:00Z' \
  '2000-01-01T24:00:00Z' '2000-01-01T00:00:60Z' '2000-01-01t00:00:00Z' '2000-01-01T00:00:00z' \
  '2000-13-01T00:00:00Z' ''; do
  bad "form: time '${t}'" "d['asked_utc'] = '${t}'" 1 "field asked_utc: invalid time"
done
bad "form: time number" "d['ended_utc'] = 946684800" 1 "field ended_utc: invalid time"

for f in resident_process_stopped gui_login_state; do
  bad "word: ${f} empty" "d['${f}'] = ''" 1 "field ${f}: invalid word"
  bad "word: ${f} space" "d['${f}'] = 'a b'" 1 "field ${f}: invalid word"
  bad "word: ${f} uppercase" "d['${f}'] = 'Abc'" 1 "field ${f}: invalid word"
  bad "word: ${f} 65 bytes" "d['${f}'] = 'a' * 65" 1 "field ${f}: invalid word"
  bad "word: ${f} number" "d['${f}'] = 1" 1 "field ${f}: invalid word"
done

# 複数の違反は件数が合うこと。
bad "count: two violations" "d['session'] = 1; d['machine'] = 'x'" 2

# ---- 入力そのもの ----

: >"${IN_F}"
bad_raw "input: empty file" 1 "not valid JSON"
printf '{' >"${IN_F}"
bad_raw "input: broken JSON" 1 "not valid JSON"
cp "${EXAMPLE}" "${IN_F}"
printf 'x' >>"${IN_F}"
bad_raw "input: trailing garbage" 1 "not valid JSON"
printf '[]' >"${IN_F}"
bad_raw "input: top level array" 1 "top level is not an object"
printf '"a"' >"${IN_F}"
bad_raw "input: top level string" 1 "top level is not an object"
printf 'null' >"${IN_F}"
bad_raw "input: top level null" 1 "top level is not an object"
printf '{"owner_answer": NaN}' >"${IN_F}"
bad_raw "input: NaN" 1 "not valid JSON"
printf '{"owner_answer": Infinity}' >"${IN_F}"
bad_raw "input: Infinity" 1 "not valid JSON"
printf '{"a": "\377"}' >"${IN_F}"
bad_raw "input: invalid UTF-8" 1 "UTF-8"
printf '\357\273\277' >"${IN_F}"
cat "${EXAMPLE}" >>"${IN_F}"
bad_raw "input: BOM" 1 "UTF-8"
python3 -I -c "print('[' * 100000)" >"${IN_F}"
bad_raw "input: deep nesting" 1 "not valid JSON"
python3 -I -c "print('{\"a\": \"' + 'x' * (1024 * 1024 + 10) + '\"}')" >"${IN_F}"
bad_raw "input: over size limit" 1 "too large"
mkdir "${work}/dir"
run_check "${work}/dir"
judge "input: directory" 1 "" "not a regular file" "FAILED: 1 violation(s)"

# ---- 誤用（2） ----

misuse "misuse: no argument"
misuse "misuse: two arguments" "${EXAMPLE}" "${EXAMPLE}"
misuse "misuse: missing path" "${work}/no-such-file"
misuse "misuse: unknown option" --no-such-option

# ---- 秘匿 ----

mut "d['session'] = 'SECRETMARK1'; d['zzsecretkey'] = 'SECRETMARK2'; d['resident_process_stopped'] = 'SECRETMARK3'"
run_check "${IN_F}"
judge "secret: values and unknown names not echoed" 1 "" "FAILED: 3 violation(s)" "!SECRETMARK" "!zzsecretkey"

# ---- 不変 ----

cp "${EXAMPLE}" "${IN_F}"
before="$(cksum <"${IN_F}")"
files_before="$(find "${work}" | wc -l)"
run_check "${IN_F}"
after="$(cksum <"${IN_F}")"
files_after="$(find "${work}" | wc -l)"
# find の件数は stdout/stderr の控えが既にある前提で、検査の前後で変わらない。
if [ "${before}" = "${after}" ] && [ "${files_before}" = "${files_after}" ] && [ "${rc}" -eq 0 ]; then
  report_ok "invariant: input and directory unchanged"
else
  report_fail "invariant: input and directory unchanged" "changed"
fi

# ---- 綴りの同期 ----

# 検査が受ける machine の値が Rust 側の Machine::name に引用符付きで在ること。
# 文字列の存在だけを見る片方向の検査で、Rust 側の改名でこのテストが落ちることが目的（逆向きは見ない）。
machine_val="$(python3 -I -c "
import json,sys
print(json.load(open(sys.argv[1]))['machine'])" "${EXAMPLE}")"
if grep -qF "\"${machine_val}\"" "${RUST_SRC}" && grep -qF "MACHINES = (\"${machine_val}\",)" "${CHECK}"; then
  report_ok "sync: machine spelling matches Machine::name"
else
  echo "FAIL: sync: machine spelling matches Machine::name" >&2
  failed=$((failed + 1))
fi

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
