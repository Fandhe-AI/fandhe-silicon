#!/usr/bin/env bash
# record-summary.sh と、verify.sh の「試験の記録から集計を出す」部分の回帰テスト（spec TASK-23 のポインタ）。
# 一時ディレクトリの合成の部品（ダミー値）で、終了コード・標準出力の完全一致・標準エラーの文言・違反の件数・
# 入力の値の秘匿・入力を変更しないことを確かめる。verify.sh の関数は source して、偽の cargo で確かめる。
# 呼び出し元: `make test-record-summary`（直接実行も可。CI の `deny` ジョブと、macOS の `test-hooks` ジョブ〔標準の bash 3.2〕が呼ぶ）。
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

TOOL="${DIR}/record-summary.sh"
VERIFY="${DIR}/verify.sh"
COMBINATION_SRC="${ROOT_DIR}/crates/core/src/test_frame/combination.rs"
LABEL_SRC="${ROOT_DIR}/crates/core/src/test_frame/label.rs"
DECISION_SRC="${ROOT_DIR}/crates/core/src/test_frame/decision.rs"
RECORD_SRC="${ROOT_DIR}/crates/core/src/test_frame/record.rs"
TABLE_SRC="${ROOT_DIR}/crates/contract/tests/test_support_test_frame.rs"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-record-summary.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
D="${work}/records"
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

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

# fresh: 空の部品のディレクトリを作り直す。
fresh() {
  rm -rf "${D:?}"
  mkdir "${D}"
}

# machine_of <組み合わせ>: 組み合わせに対応する機材の呼び名（combination.rs の対応のとおり）。
machine_of() {
  case "$1" in
    metal-mac-mini) echo mac-mini ;;
    *-node-b) echo node-b ;;
    *-node-a) echo node-a ;;
    vulkan-amd) echo amd ;;
    vulkan-intel) echo intel ;;
    *) echo unknown ;;
  esac
}

# part_json <試験の ID> <組み合わせ> <ラベル> [理由]: record.rs と同じ欄の順の 1 行の JSON を標準出力へ。
# 理由が空でなければ label_reason を足す。判定保留のときは body に threshold_id を足す。
part_json() {
  local id="$1" combo="$2" label="$3" reason="${4:-}" extra="" reason_field=""
  if [ -n "${reason}" ]; then
    reason_field=",\"label_reason\":\"${reason}\""
  fi
  if [ "${label}" = "pending" ]; then
    extra=",\"threshold_id\":\"t-4\""
  fi
  printf '{"behavior_ids":["BUILD-54"],"machine_combination":"%s","machine":"%s","label":"%s"%s,"body_kind":"contract-test","body":{"test_id":"%s"%s}}\n' \
    "${combo}" "$(machine_of "${combo}")" "${label}" "${reason_field}" "${id}" "${extra}"
}

# put_part <試験の ID> <組み合わせ> <ラベル> [理由]: 部品を export.rs のファイル名で ${D} に置く。
put_part() {
  part_json "$@" >"${D}/$1.$2.json"
}

# put_raw <ファイル名> <内容>: 内容をそのまま（末尾の改行なし）置く。
put_raw() {
  printf '%s' "$2" >"${D}/$1"
}

run_tool() {
  rc=0
  "${BASH}" "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
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

# bad <名前> <件数> [部分一致...]: ${D} の内容が違反になること（標準出力は空。表は出さない）。
bad() {
  local name="$1" n="$2"
  shift 2
  run_tool "${D}"
  judge "${name}" 1 "" "FAILED: ${n} violation(s)" "$@"
}

# bad1 <名前> <JSON 1 行> <違反の種類>: 良い部品 1 件と、与えた 1 行（ファイル名 bad.json）で、違反がちょうど 1 件になること。
bad1() {
  fresh
  put_part good_test metal-mac-mini ok
  put_raw bad.json "$2"
  bad "$1" 1 "violation: $3"
}

# ---- 合格: 表の完全一致 ----

fresh
put_part case_a metal-mac-mini ok
put_part case_b metal-mac-mini ng
put_part case_c vulkan-node-a pending threshold-unapproved
put_part case_d vulkan-node-a ok
put_part case_d vulkan-amd not-run combination-not-declared
put_part case_e cuda-node-b not-run not-configured
put_part case_f cuda-node-b not-run permission-missing
put_part case_g cuda-node-a indeterminate device-mismatch
put_part case_h vulkan-intel indeterminate machine-unreachable
put_part case_i vulkan-node-b not-run invalid-combination-setting
put_part case_j vulkan-node-b not-run invalid-permission-setting
EXPECTED_MIXED='| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 1 | 1 | 0 | 0 | 0 |
| vulkan-node-b | 0 | 0 | 0 | 2 | 0 |
| vulkan-node-a | 1 | 0 | 1 | 0 | 0 |
| vulkan-amd | 0 | 0 | 0 | 1 | 0 |
| vulkan-intel | 0 | 0 | 0 | 0 | 1 |
| cuda-node-b | 0 | 0 | 0 | 2 | 0 |
| cuda-node-a | 0 | 0 | 0 | 0 | 1 |'
run_tool "${D}"
judge "pass: mixed labels give the exact table" 0 "${EXPECTED_MIXED}" "!case_"

# NG や判定保留・判定不能・未実施が何件あっても 0（失敗にするのは試験の側）。
fresh
put_part only_ng metal-mac-mini ng
put_part only_ng2 vulkan-amd ng
run_tool "${D}"
judge "pass: ng records alone still exit 0" 0 "| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 0 | 1 | 0 | 0 | 0 |
| vulkan-node-b | 0 | 0 | 0 | 0 | 0 |
| vulkan-node-a | 0 | 0 | 0 | 0 | 0 |
| vulkan-amd | 0 | 1 | 0 | 0 | 0 |
| vulkan-intel | 0 | 0 | 0 | 0 | 0 |
| cuda-node-b | 0 | 0 | 0 | 0 | 0 |
| cuda-node-a | 0 | 0 | 0 | 0 | 0 |"

# 判定不能の理由 4 語は、どれも indeterminate の列に数えられる。
fresh
put_part reason_a vulkan-amd indeterminate evidence-missing
put_part reason_b vulkan-amd indeterminate environment-differs
run_tool "${D}"
judge "pass: new indeterminate reasons are counted as indeterminate" 0 "| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 0 | 0 | 0 | 0 | 0 |
| vulkan-node-b | 0 | 0 | 0 | 0 | 0 |
| vulkan-node-a | 0 | 0 | 0 | 0 | 0 |
| vulkan-amd | 0 | 0 | 0 | 0 | 2 |
| vulkan-intel | 0 | 0 | 0 | 0 | 0 |
| cuda-node-b | 0 | 0 | 0 | 0 | 0 |
| cuda-node-a | 0 | 0 | 0 | 0 | 0 |"

# CI の機材なしの形: 宣言した組み合わせごとに not-run だけが並ぶ。
fresh
put_part ci_shape metal-mac-mini not-run not-configured
put_part ci_shape vulkan-node-b not-run not-configured
put_part ci_shape cuda-node-a not-run not-configured
run_tool "${D}"
judge "pass: device-free shape counts only not-run and exits 0" 0 "| combination | ok | ng | pending | not-run | indeterminate |
| --- | ---: | ---: | ---: | ---: | ---: |
| metal-mac-mini | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-b | 0 | 0 | 0 | 1 | 0 |
| vulkan-node-a | 0 | 0 | 0 | 0 | 0 |
| vulkan-amd | 0 | 0 | 0 | 0 | 0 |
| vulkan-intel | 0 | 0 | 0 | 0 | 0 |
| cuda-node-b | 0 | 0 | 0 | 0 | 0 |
| cuda-node-a | 0 | 0 | 0 | 1 | 0 |"

# 末尾の改行の有無・空の behavior_ids・複数の behavior_ids は受ける。
fresh
put_raw a.json '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"a"}}'
put_raw b.json '{"behavior_ids":["BUILD-54","TASK-2"],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"b"}}
'
run_tool "${D}"
if [ "${rc}" -eq 0 ] && grep -qF '| vulkan-amd | 2 | 0 | 0 | 0 | 0 |' "${OUT_F}"; then
  report_ok "pass: with or without a trailing newline, empty and several behavior ids"
else
  report_fail "pass: with or without a trailing newline, empty and several behavior ids" "unexpected result"
fi

# ---- 部品が 0 件 ----

fresh
run_tool "${D}"
judge "empty: an empty directory is a violation, not an all-zero table" 1 "" "violation: no-records" "FAILED: 1 violation(s)"

# ---- 形の違反（良い部品 1 件 + 悪い 1 件で、違反がちょうど 1 件） ----

bad1 "json: not json" 'not json' invalid-json
bad1 "json: truncated" '{"behavior_ids":[' invalid-json
bad1 "json: empty file" '' invalid-json
bad1 "json: NaN constant" '{"x":NaN}' invalid-json
bad1 "json: duplicate key" '{"label":"ok","label":"ok"}' duplicate-key
bad1 "json: array" '[]' not-an-object
bad1 "json: number" '1' not-an-object
bad1 "json: string" '"x"' not-an-object
bad1 "json: null" 'null' not-an-object
bad1 "json: two lines" '{"a":1}
{"b":2}' not-one-line

OK_BASE='{"behavior_ids":["BUILD-54"],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}'

# 必須の欄が 1 つ欠ける
for f in behavior_ids machine_combination machine label body_kind body; do
  json="$(printf '%s' "${OK_BASE}" | python3 -I -c "
import json,sys
d=json.load(sys.stdin)
del d[sys.argv[1]]
print(json.dumps(d))" "${f}")"
  bad1 "missing: required field ${f}" "${json}" missing-field
done

# 一覧に無い欄
json="$(printf '%s' "${OK_BASE}" | python3 -I -c "
import json,sys
d=json.load(sys.stdin)
d['secret_marker_field']='v'
print(json.dumps(d))")"
bad1 "unknown: extra top-level field" "${json}" unknown-field
fresh
put_part good_test metal-mac-mini ok
put_raw bad.json "${json}"
run_tool "${D}"
judge "secret: unknown field name is not echoed" 1 "" "FAILED: 1 violation(s)" "!secret_marker_field" "!bad.json"

# 値の違反
bad1 "value: unknown combination" '{"behavior_ids":[],"machine_combination":"vulkan-other","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' unknown-combination
bad1 "value: combination with different case" '{"behavior_ids":[],"machine_combination":"Vulkan-AMD","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' unknown-combination
bad1 "value: combination not a string" '{"behavior_ids":[],"machine_combination":1,"machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' unknown-combination
bad1 "value: machine differs from combination" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"intel","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' machine-mismatch
bad1 "value: machine not a string" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":null,"label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' machine-mismatch
bad1 "value: unknown label" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"skipped","body_kind":"contract-test","body":{"test_id":"x"}}' unknown-label
bad1 "value: label with different case" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"OK","body_kind":"contract-test","body":{"test_id":"x"}}' unknown-label
bad1 "value: unknown body kind" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"run","body":{"test_id":"x"}}' unknown-body-kind
bad1 "value: behavior ids not a list" '{"behavior_ids":"BUILD-54","machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' bad-behavior-ids
bad1 "value: behavior id with lowercase" '{"behavior_ids":["build-54"],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' bad-behavior-ids
bad1 "value: duplicate behavior ids" '{"behavior_ids":["BUILD-54","BUILD-54"],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x"}}' bad-behavior-ids

# 理由
bad1 "reason: pending without reason" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"pending","body_kind":"contract-test","body":{"test_id":"x","threshold_id":"t-4"}}' missing-reason
bad1 "reason: not-run without reason" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"not-run","body_kind":"contract-test","body":{"test_id":"x"}}' missing-reason
bad1 "reason: indeterminate without reason" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"indeterminate","body_kind":"contract-test","body":{"test_id":"x"}}' missing-reason
bad1 "reason: ok with a reason" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","label_reason":"not-configured","body_kind":"contract-test","body":{"test_id":"x"}}' unexpected-reason
bad1 "reason: ng with a reason" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ng","label_reason":"device-mismatch","body_kind":"contract-test","body":{"test_id":"x"}}' unexpected-reason
bad1 "reason: unknown reason" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"not-run","label_reason":"because","body_kind":"contract-test","body":{"test_id":"x"}}' unknown-reason
bad1 "reason: reason of another label" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"not-run","label_reason":"device-mismatch","body_kind":"contract-test","body":{"test_id":"x"}}' unknown-reason
bad1 "reason: reason not a string" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"not-run","label_reason":1,"body_kind":"contract-test","body":{"test_id":"x"}}' unknown-reason

# 本体
bad1 "body: not an object" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":"x"}' bad-body
bad1 "body: test id missing" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{}}' bad-body
bad1 "body: test id with uppercase" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"X"}}' bad-body
bad1 "body: test id with a dot" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"a.b"}}' bad-body
bad1 "body: unknown key" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x","note":"y"}}' bad-body
bad1 "body: pending without threshold id" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"pending","label_reason":"threshold-unapproved","body_kind":"contract-test","body":{"test_id":"x"}}' bad-body
bad1 "body: pending with a malformed threshold id" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"pending","label_reason":"threshold-unapproved","body_kind":"contract-test","body":{"test_id":"x","threshold_id":"t-04"}}' bad-body
bad1 "body: threshold id on a label that is not pending" '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"x","threshold_id":"t-4"}}' bad-body

# ---- 重複・ファイルの種類・大きさ ----

fresh
put_part dup_test vulkan-amd ok
part_json dup_test vulkan-amd ok >"${D}/copy.json"
bad "duplicate: same test id and combination in two files" 1 "violation: duplicate-record"

fresh
put_part same_test vulkan-amd ok
put_part same_test vulkan-intel ok
put_part other_test vulkan-amd ok
run_tool "${D}"
if [ "${rc}" -eq 0 ] && grep -qF '| vulkan-amd | 2 | 0 | 0 | 0 | 0 |' "${OUT_F}" && grep -qF '| vulkan-intel | 1 | 0 | 0 | 0 | 0 |' "${OUT_F}"; then
  report_ok "duplicate: the same test id on other combinations is not a duplicate"
else
  report_fail "duplicate: the same test id on other combinations is not a duplicate" "unexpected result"
fi

fresh
put_part good_test metal-mac-mini ok
printf '%s\n' "$(part_json x vulkan-amd ok)" >"${D}/x.vulkan-amd.txt"
bad "entry: a file whose name does not end with .json" 1 "violation: not-a-record-name"

fresh
put_part good_test metal-mac-mini ok
mkdir "${D}/sub.json"
bad "entry: a directory" 1 "violation: not-a-regular-file"

fresh
put_part good_test metal-mac-mini ok
ln -s "${D}/good_test.metal-mac-mini.json" "${D}/link.json"
bad "entry: a symbolic link" 1 "violation: not-a-regular-file"

fresh
put_part good_test metal-mac-mini ok
python3 -I -c "
import sys
sys.stdout.write('{\"pad\":\"' + 'a' * (70 * 1024) + '\"}')" >"${D}/big.json"
bad "entry: a file over the size limit" 1 "violation: too-large"

fresh
put_part good_test metal-mac-mini ok
printf '\377\376{}' >"${D}/binary.json"
bad "entry: not utf-8" 1 "violation: not-utf8"

fresh
put_part good_test metal-mac-mini ok
put_raw bad.json "$(printf '{"a":1}\r\n')"
bad "entry: a carriage return" 1 "violation: not-one-line"

# 複数の違反は件数に出る
fresh
put_raw a.json 'x'
put_raw b.json '[]'
put_raw c.txt 'y'
bad "multiple: every violation is counted" 3 "violation: invalid-json" "violation: not-an-object" "violation: not-a-record-name"

# 入力の値の秘匿（試験の ID・ファイル名）
fresh
put_part good_test metal-mac-mini ok
put_raw secret_marker_name.json 'not json'
run_tool "${D}"
judge "secret: file name is not echoed" 1 "" "FAILED: 1 violation(s)" "!secret_marker_name"
fresh
printf '{"behavior_ids":[],"machine_combination":"vulkan-amd","machine":"amd","label":"ok","body_kind":"contract-test","body":{"test_id":"secret_marker_id","x":1}}' >"${D}/a.json"
run_tool "${D}"
judge "secret: test id is not echoed" 1 "" "FAILED: 1 violation(s)" "!secret_marker_id"

# ---- 誤用 ----

run_tool
judge "misuse: no argument" 2 "" "usage:"
run_tool "${D}" "${D}"
judge "misuse: two arguments" 2 "" "usage:"
run_tool --help
judge "misuse: option-like argument" 2 "" "usage:"
run_tool "${work}/no-such-dir"
judge "misuse: missing directory" 2 "" "not found"
fresh
put_part good_test metal-mac-mini ok
run_tool "${D}/good_test.metal-mac-mini.json"
judge "misuse: a file instead of a directory" 2 "" "not a directory"
ln -s "${D}" "${work}/link-to-records"
run_tool "${work}/link-to-records"
judge "misuse: a symbolic link to a directory" 2 "" "not a directory"
run_tool "${work}/link-to-records/"
judge "misuse: a symbolic link to a directory with a trailing separator" 2 "" "not a directory"
run_tool "${work}/link-to-records/."
judge "misuse: a symbolic link to a directory with a trailing dot" 2 "" "not a directory"
run_tool "${work}/link-to-records//./"
judge "misuse: a symbolic link to a directory with repeated separators and dots" 2 "" "not a directory"
run_tool "${D}/"
if [ "${rc}" -eq 0 ] && grep -qF '| metal-mac-mini | 1 |' "${OUT_F}"; then
  report_ok "pass: a real directory with a trailing separator"
else
  report_fail "pass: a real directory with a trailing separator" "unexpected result"
fi

# ---- 入力を変更しない・ファイルを作らない ----

fresh
put_part inv_a metal-mac-mini ok
put_part inv_b cuda-node-a not-run not-configured
before="$(cat "${D}"/* | cksum)"
files_before="$(find "${work}" | wc -l)"
run_tool "${D}"
after="$(cat "${D}"/* | cksum)"
files_after="$(find "${work}" | wc -l)"
# find の件数は標準出力・標準エラーの控えが既にある前提で、実行の前後で変わらない。
if [ "${rc}" -eq 0 ] && [ "${before}" = "${after}" ] && [ "${files_before}" = "${files_after}" ]; then
  report_ok "invariant: input and directory unchanged"
else
  report_fail "invariant: input and directory unchanged" "changed"
fi

# ---- 綴りの同期（Rust 側の綴りとの一致） ----

# Rust 側の綴りと食い違ったとき、途中の失敗で全体が止まらず、FAIL の行と件数が出るようにする。
set +e

# Rust のソースから引用符付きの綴りを順に取り出す（片方向ではなく、表の行と列の順まで見る）。
rust_names() {
  python3 -I -c "
import re,sys
src=open(sys.argv[1],encoding='utf-8').read()
for m in re.finditer(sys.argv[2], src):
    print(m.group(1))" "$1" "$2"
}

COMBOS="$(rust_names "${COMBINATION_SRC}" 'MachineCombination::\w+ => "([a-z0-9-]+)",')"
MACHINES="$(rust_names "${COMBINATION_SRC}" 'Machine::\w+ => "([a-z0-9-]+)",')"
KINDS="$(rust_names "${LABEL_SRC}" 'LabelKind::\w+ => "([a-z0-9-]+)",')"
NOT_RUN_REASONS="$(rust_names "${DECISION_SRC}" 'NotRunReason::\w+ => "([a-z0-9-]+)",')"
INDETERMINATE_REASONS="$(rust_names "${LABEL_SRC}" 'IndeterminateReason::\w+ => "([a-z0-9-]+)",')"
PENDING_REASON_VALUE="$(rust_names "${LABEL_SRC}" 'PENDING_REASON: &str = "([a-z0-9-]+)"')"
BODY_KIND_VALUE="$(rust_names "${RECORD_SRC}" 'BODY_KIND: &str = "([a-z0-9-]+)"')"

count_lines() { printf '%s\n' "$1" | grep -c . || true; }
if [ "$(count_lines "${COMBOS}")" = 7 ] && [ "$(count_lines "${MACHINES}")" = 5 ] \
  && [ "$(count_lines "${KINDS}")" = 5 ] && [ "$(count_lines "${NOT_RUN_REASONS}")" = 7 ] \
  && [ "$(count_lines "${INDETERMINATE_REASONS}")" = 4 ] && [ "${PENDING_REASON_VALUE}" = "threshold-unapproved" ] \
  && [ "${BODY_KIND_VALUE}" = "contract-test" ]; then
  report_ok "sync: the Rust sources still yield the expected number of spellings"
else
  echo "FAIL: sync: the Rust sources still yield the expected number of spellings" >&2
  failed=$((failed + 1))
fi

# 表の行と列が Rust の順で並ぶこと: Rust の綴りの組ごとに部品を作り、表の行頭の並びを比べる。
fresh
for c in ${COMBOS}; do
  put_part "sync_${c}" "${c}" ok
done
run_tool "${D}"
rows="$(sed -n '3,$p' "${OUT_F}" | sed 's/^| \([a-z0-9-]*\) |.*/\1/')"
cols="$(sed -n '1p' "${OUT_F}" | tr '|' '\n' | sed 's/^ *//; s/ *$//' | grep -v '^$' | sed '1d')"
if [ "${rc}" -eq 0 ] && [ "${rows}" = "${COMBOS}" ] && [ "${cols}" = "${KINDS}" ]; then
  report_ok "sync: table rows follow MachineCombination::ALL and columns follow LabelKind::ALL"
else
  echo "FAIL: sync: table rows follow MachineCombination::ALL and columns follow LabelKind::ALL" >&2
  failed=$((failed + 1))
fi

# 組み合わせ → 機材: 組み合わせの名前は機材の名前で終わる（Rust の試験 combination_name_ends_with_its_machine_name と同じ規則）。
sync_ok=1
for c in ${COMBOS}; do
  want=""
  for m in ${MACHINES}; do
    case "${c}" in *-"${m}") want="${m}" ;; esac
  done
  if [ -z "${want}" ]; then
    sync_ok=0
    continue
  fi
  fresh
  put_part sync_machine "${c}" ok
  run_tool "${D}"
  [ "${rc}" -eq 0 ] || sync_ok=0
  # 同じ部品の machine だけを別の機材にすると拒否される
  for m in ${MACHINES}; do
    [ "${m}" != "${want}" ] || continue
    fresh
    part_json sync_machine "${c}" ok | sed "s/\"machine\":\"${want}\"/\"machine\":\"${m}\"/" >"${D}/x.json"
    run_tool "${D}"
    [ "${rc}" -eq 1 ] || sync_ok=0
  done
done
if [ "${sync_ok}" = 1 ]; then
  report_ok "sync: every combination maps to the machine its name ends with"
else
  echo "FAIL: sync: every combination maps to the machine its name ends with" >&2
  failed=$((failed + 1))
fi

# ラベルと理由: Rust の理由を、それぞれのラベルで受け、別のラベルの理由として使うと拒否する。
sync_ok=1
fresh
n=0
for r in ${NOT_RUN_REASONS}; do
  n=$((n + 1))
  put_part "sync_nr${n}" vulkan-amd not-run "${r}"
done
n=0
for r in ${INDETERMINATE_REASONS}; do
  n=$((n + 1))
  put_part "sync_in${n}" vulkan-amd indeterminate "${r}"
done
put_part sync_pending vulkan-amd pending "${PENDING_REASON_VALUE}"
run_tool "${D}"
[ "${rc}" -eq 0 ] || sync_ok=0
for r in ${INDETERMINATE_REASONS} ${PENDING_REASON_VALUE}; do
  fresh
  put_part sync_cross vulkan-amd not-run "${r}"
  run_tool "${D}"
  [ "${rc}" -eq 1 ] || sync_ok=0
done
for r in ${NOT_RUN_REASONS}; do
  fresh
  put_part sync_cross vulkan-amd indeterminate "${r}"
  run_tool "${D}"
  [ "${rc}" -eq 1 ] || sync_ok=0
done
if [ "${sync_ok}" = 1 ]; then
  report_ok "sync: reason tokens match the Rust sources per label"
else
  echo "FAIL: sync: reason tokens match the Rust sources per label" >&2
  failed=$((failed + 1))
fi

# 表の見出しと区切りの行が、Rust の端から端の試験の期待の表（test_support_test_frame.rs）と一致する。
rust_header="$(grep -m1 '^| combination |' "${TABLE_SRC}" || true)"
rust_rule="$(grep -m1 '^| --- |' "${TABLE_SRC}" || true)"
fresh
put_part table_sync metal-mac-mini ok
run_tool "${D}"
if [ -n "${rust_header}" ] && [ "$(sed -n 1p "${OUT_F}")" = "${rust_header}" ] && [ "$(sed -n 2p "${OUT_F}")" = "${rust_rule}" ]; then
  report_ok "sync: table header and rule lines match the Rust expected table"
else
  echo "FAIL: sync: table header and rule lines match the Rust expected table" >&2
  failed=$((failed + 1))
fi

set -e

# ---- verify.sh の記録の扱い（偽の cargo） ----

BIN="${work}/bin"
mkdir "${BIN}"
cat >"${BIN}/cargo" <<'FAKE'
#!/usr/bin/env bash
# 偽の cargo: 引数と、渡された書き出し先を控え、FAKE_MODE に応じて部品を書く。
echo "$*" >>"${FAKE_ARGS_LOG}"
if [ "${FANDHE_SILICON_TEST_RECORD_DIR+set}" = set ]; then
  printf '%s\n' "${FANDHE_SILICON_TEST_RECORD_DIR}" >>"${FAKE_DIR_LOG}"
else
  echo UNSET >>"${FAKE_DIR_LOG}"
  exit "${FAKE_EXIT:-0}"
fi
d="${FANDHE_SILICON_TEST_RECORD_DIR}"
case "${FAKE_MODE:-write}" in
  write)
    printf '%s\n' '{"behavior_ids":["BUILD-54"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"not-run","label_reason":"not-configured","body_kind":"contract-test","body":{"test_id":"fake_a"}}' >"${d}/fake_a.metal-mac-mini.json"
    printf '%s\n' '{"behavior_ids":[],"machine_combination":"cuda-node-a","machine":"node-a","label":"ng","body_kind":"contract-test","body":{"test_id":"fake_b"}}' >"${d}/fake_b.cuda-node-a.json"
    ;;
  nothing) ;;
  malformed) echo 'not json' >"${d}/bad.json" ;;
  term) kill -TERM "$PPID" ;;
  int) kill -INT "$PPID" ;;
  hup) kill -HUP "$PPID" ;;
esac
exit "${FAKE_EXIT:-0}"
FAKE
chmod +x "${BIN}/cargo"

FV_OUT="${work}/fv-stdout.txt"
FV_ERR="${work}/fv-stderr.txt"
FAKE_ARGS_LOG="${work}/fake-args.log"
FAKE_DIR_LOG="${work}/fake-dir.log"
# run_verify <KEY=VALUE...>: verify.sh を source して関数を偽の cargo で動かす（サブシェル。env -i で呼び出し側の環境を持ち込まない）。
run_verify() {
  : >"${FAKE_ARGS_LOG}"
  : >"${FAKE_DIR_LOG}"
  rc=0
  cd "${work}"
  # shellcheck disable=SC2016 # bash -c の本文は子のシェルが展開する（$1 は source する verify.sh のパス）
  env -i HOME="${HOME:-/}" PATH="${BIN}:${PATH}" TMPDIR="${work}" \
    FAKE_ARGS_LOG="${FAKE_ARGS_LOG}" FAKE_DIR_LOG="${FAKE_DIR_LOG}" "$@" \
    "${BASH}" -c 'set -euo pipefail; source "$1"; run_cargo_test_with_record_summary' _ "${VERIFY}" \
    >"${FV_OUT}" 2>"${FV_ERR}" </dev/null || rc=$?
}

fv_fail() {
  failed=$((failed + 1))
  echo "FAIL: $1" >&2
  sed 's/^/    /' "${FV_ERR}" >&2 || true
}

leftover_dirs() { find "${work}" -maxdepth 1 -name 'fandhe-silicon-test-records.*' | grep -c . || true; }
# 偽の cargo が呼ばれた回数と、渡された書き出し先の行数。
dir_log_lines() { wc -l <"${FAKE_DIR_LOG}" | tr -d ' '; }

# 環境変数が無いとき: 新しい一時ディレクトリを渡し、表を出し、ディレクトリを消す。cargo test は 1 回だけ。
run_verify FAKE_MODE=write
first_dir="$(cat "${FAKE_DIR_LOG}")"
name="verify: unset env gives cargo one fresh directory, prints the table, and removes the directory"
if [ "${rc}" -eq 0 ] && [ "$(cat "${FAKE_ARGS_LOG}")" = "test --workspace --all-features" ] \
  && [ "$(dir_log_lines)" = 1 ] && [ "${first_dir#"${work}"/fandhe-silicon-test-records.}" != "${first_dir}" ] \
  && [ ! -e "${first_dir}" ] && [ "$(leftover_dirs)" = 0 ] \
  && [ "$(sed -n 3p "${FV_OUT}")" = "| metal-mac-mini | 0 | 0 | 0 | 1 | 0 |" ] \
  && [ "$(sed -n 9p "${FV_OUT}")" = "| cuda-node-a | 0 | 1 | 0 | 0 | 0 |" ]; then
  report_ok "${name}"
else
  fv_fail "${name}"
fi

run_verify FAKE_MODE=write
second_dir="$(cat "${FAKE_DIR_LOG}")"
name="verify: a second run uses a different directory"
if [ "${rc}" -eq 0 ] && [ "${first_dir}" != "${second_dir}" ]; then report_ok "${name}"; else fv_fail "${name}"; fi

# 試験が失敗しても、その終了コードを返し、ディレクトリを消し、出せる集計は出す。
run_verify FAKE_MODE=write FAKE_EXIT=101
name="verify: a failing cargo test keeps its exit code, still prints the table and removes the directory"
if [ "${rc}" -eq 101 ] && [ "$(grep -c '^| combination |' "${FV_OUT}")" = 1 ] && [ "$(leftover_dirs)" = 0 ]; then
  report_ok "${name}"
else
  fv_fail "${name}"
fi

# 試験が失敗して部品が無い・壊れているときも、試験の終了コードが勝つ。
run_verify FAKE_MODE=nothing FAKE_EXIT=101
name="verify: a failing cargo test with no records returns the test exit code"
if [ "${rc}" -eq 101 ] && [ "$(leftover_dirs)" = 0 ]; then report_ok "${name}"; else fv_fail "${name}"; fi
run_verify FAKE_MODE=malformed FAKE_EXIT=3
name="verify: a failing cargo test with a malformed record returns the test exit code"
if [ "${rc}" -eq 3 ] && [ "$(leftover_dirs)" = 0 ]; then report_ok "${name}"; else fv_fail "${name}"; fi

# 試験が通っても、部品が 0 件・形の違反なら失敗にする（表は出さない）。
run_verify FAKE_MODE=nothing
name="verify: passing tests that wrote no records fail"
if [ "${rc}" -eq 1 ] && [ ! -s "${FV_OUT}" ] && [ "$(leftover_dirs)" = 0 ]; then report_ok "${name}"; else fv_fail "${name}"; fi
run_verify FAKE_MODE=malformed
name="verify: passing tests that wrote a malformed record fail"
if [ "${rc}" -eq 1 ] && [ ! -s "${FV_OUT}" ] && [ "$(leftover_dirs)" = 0 ]; then report_ok "${name}"; else fv_fail "${name}"; fi

# 呼び出し側の環境に既に在るとき: 上書きせず、そのまま渡し、集計も片づけもしない。
preset="${work}/preset"
mkdir "${preset}"
echo keep >"${preset}/keep.txt"
run_verify FAKE_MODE=write FANDHE_SILICON_TEST_RECORD_DIR="${preset}"
name="verify: a preset directory is passed through, kept, and not summarized"
if [ "${rc}" -eq 0 ] && [ "$(cat "${FAKE_DIR_LOG}")" = "${preset}" ] && [ -f "${preset}/keep.txt" ] \
  && [ -f "${preset}/fake_a.metal-mac-mini.json" ] && [ ! -s "${FV_OUT}" ] && [ "$(leftover_dirs)" = 0 ]; then
  report_ok "${name}"
else
  fv_fail "${name}"
fi
run_verify FAKE_MODE=write FAKE_EXIT=101 FANDHE_SILICON_TEST_RECORD_DIR="${preset}2"
name="verify: a preset value is passed through as is, and the test exit code is kept"
if [ "${rc}" -eq 101 ] && [ "$(cat "${FAKE_DIR_LOG}")" = "${preset}2" ] && [ ! -s "${FV_OUT}" ]; then
  report_ok "${name}"
else
  fv_fail "${name}"
fi
run_verify FAKE_MODE=nothing FANDHE_SILICON_TEST_RECORD_DIR=
name="verify: an empty preset value counts as set and is passed through unchanged"
if [ "${rc}" -eq 0 ] && [ "$(dir_log_lines)" = 1 ] && [ "$(cat "${FAKE_DIR_LOG}")" = "" ] && [ ! -s "${FV_OUT}" ]; then
  report_ok "${name}"
else
  fv_fail "${name}"
fi

# TMPDIR が相対パスでも、渡す書き出し先は絶対パスで、試験の後に消える。
mkdir "${work}/reltmp"
run_verify FAKE_MODE=write TMPDIR=reltmp
rel_dir="$(cat "${FAKE_DIR_LOG}")"
name="verify: a relative TMPDIR still gives cargo an absolute directory, which is removed afterwards"
if [ "${rc}" -eq 0 ] && [ "${rel_dir#/}" != "${rel_dir}" ] && [ ! -e "${rel_dir}" ] \
  && [ "$(grep -c '^| combination |' "${FV_OUT}")" = 1 ]; then
  report_ok "${name}"
else
  fv_fail "${name}"
fi

# シグナルのときは慣習の終了コードで、ディレクトリを消す。
for sig_case in "term 143" "int 130" "hup 129"; do
  mode="${sig_case% *}"
  want="${sig_case#* }"
  run_verify FAKE_MODE="${mode}"
  name="verify: ${mode} exits with ${want} and removes the directory"
  if [ "${rc}" -eq "${want}" ] && [ "$(leftover_dirs)" = 0 ]; then
    report_ok "${name}"
  else
    fv_fail "${name}"
  fi
done

# ---- build-profiles.sh は呼び出し側の書き出し先を引き継がない（偽の cargo と rustup） ----

BIN2="${work}/bin2"
mkdir "${BIN2}"
BP_LOG="${work}/bp.log"
cat >"${BIN2}/cargo" <<'FAKE'
#!/usr/bin/env bash
if [ "${FANDHE_SILICON_TEST_RECORD_DIR+set}" = set ]; then echo SET >>"${BP_LOG}"; else echo UNSET >>"${BP_LOG}"; fi
FAKE
cat >"${BIN2}/rustup" <<'FAKE'
#!/usr/bin/env bash
echo wasm32-unknown-unknown
FAKE
chmod +x "${BIN2}/cargo" "${BIN2}/rustup"
: >"${BP_LOG}"
rc=0
env BP_LOG="${BP_LOG}" FANDHE_SILICON_TEST_RECORD_DIR="${work}/inherited" PATH="${BIN2}:${PATH}" \
  "${BASH}" "${DIR}/build-profiles.sh" >"${FV_OUT}" 2>"${FV_ERR}" </dev/null || rc=$?
name="build-profiles: the caller's record directory is not passed to any cargo call"
if [ "${rc}" -eq 0 ] && [ "$(grep -c UNSET "${BP_LOG}")" -gt 10 ] && ! grep -q '^SET$' "${BP_LOG}"; then
  report_ok "${name}"
else
  fv_fail "${name}"
fi

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
