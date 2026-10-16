#!/usr/bin/env bash
# rerun-compare.sh の回帰テスト（spec TASK-29 のポインタ。親 Issue #135 の 5/8）。
# 一時ディレクトリの合成の部品（ダミーの値）を並べて 2 つのディレクトリとして渡し、終了コード・標準出力・
# 標準エラーの文言を確かめる。cargo もネットワークも `docs/spec` の取得も使わず、リポジトリのファイルを変更しない。
# 呼び出し元: `make test-rerun-compare`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
# 前提: python3（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致（並びは完全一致）」で行う。終了コードだけでは
# 別の理由で落ちても気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。現行の道具と食い違ったらテストでなく道具側の挙動として直す。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

TOOL="${DIR}/rerun-compare.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-rerun-compare.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

machine_of() {
  case "$1" in
    metal-mac-mini) echo mac-mini ;;
    vulkan-node-b | cuda-node-b) echo node-b ;;
    vulkan-node-a | cuda-node-a) echo node-a ;;
    vulkan-amd) echo amd ;;
    vulkan-intel) echo intel ;;
    *) echo unknown ;;
  esac
}

# put <ディレクトリ名> <試験の ID> <組み合わせ> <ラベル> [理由]: record.rs と同じ欄の順の 1 行の JSON を置く。
# 値はダミー。behavior_ids と threshold_id は出力に出ないことの確かめ用の目印を入れる。
put() {
  local d="${work}/$1" id="$2" combo="$3" label="$4" reason="${5:-}" rf="" extra=""
  mkdir -p "${d}"
  if [ -n "${reason}" ]; then
    rf=",\"label_reason\":\"${reason}\""
  fi
  if [ "${label}" = "pending" ]; then
    extra=",\"threshold_id\":\"t-777\""
  fi
  printf '{"behavior_ids":["MARKER-BEHAVIOR-9"],"machine_combination":"%s","machine":"%s","label":"%s"%s,"body_kind":"contract-test","body":{"test_id":"%s"%s}}\n' \
    "${combo}" "$(machine_of "${combo}")" "${label}" "${rf}" "${id}" "${extra}" >"${d}/${id}.${combo}.json"
}

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

run_tool() {
  rc=0
  "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# judge <名前> <期待コード> [語...]: o:<語> は標準出力に、e:<語> は標準エラーに含まれること。!<語> はどちらにも含まれないこと。
# どのケースでも作業ディレクトリの絶対パス（引数のパス）と目印の値が出力に出ないことも確かめる。
judge() {
  local name="$1" want="$2" s
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    report_fail "${name}" "expected exit ${want}, got ${rc}"
    return 0
  fi
  if grep -qF -e "${work}" "${ERR_F}" "${OUT_F}"; then
    report_fail "${name}" "output must not contain the argument path"
    return 0
  fi
  if grep -qF -e "MARKER-BEHAVIOR-9" -e "t-777" "${ERR_F}" "${OUT_F}"; then
    report_fail "${name}" "output must not contain values other than test id, combination, label and reason"
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
      o:*)
        if ! grep -qF -e "${s#o:}" "${OUT_F}"; then
          report_fail "${name}" "stdout lacks: ${s#o:}"
          return 0
        fi
        ;;
      e:*)
        if ! grep -qF -e "${s#e:}" "${ERR_F}"; then
          report_fail "${name}" "stderr lacks: ${s#e:}"
          return 0
        fi
        ;;
    esac
  done
  report_ok "${name}"
}

# differ <名前> <前> <新> [語...]: 差分あり（終了コード 1）。
differ() {
  local name="$1" a="$2" b="$3"
  shift 3
  run_tool "${work}/${a}" "${work}/${b}"
  judge "${name}" 1 "e:DIFFERENT: " "o:summary: " "!ok: " "$@"
}

# misuse <名前> <前> <新> [語...]: 読めない記録（終了コード 2・error の行・DIFFERENT なし）。
misuse() {
  local name="$1" a="$2" b="$3"
  shift 3
  run_tool "${work}/${a}" "${work}/${b}"
  judge "${name}" 2 "e:error: " "!ok: " "!summary: " "!DIFFERENT" "$@"
}

# ---- 基準の記録 ----

for d in base base_copy; do
  put "${d}" case_a metal-mac-mini ok
  put "${d}" case_b vulkan-amd ng
  put "${d}" case_c cuda-node-a not-run not-configured
  put "${d}" case_d vulkan-intel pending threshold-unapproved
done

# ---- 差分なし ----

run_tool "${work}/base" "${work}/base_copy"
judge "identical" 0 "o:ok: no differences from the previous record" "o:summary: same=4 regressed=0 improved=0 changed=0 removed=0 added=0" "!DIFFERENT"
run_tool "${work}/base" "${work}/base"
judge "same_dir_twice" 0 "o:ok: no differences from the previous record" "o:same=4"

# ---- 差分あり ----

cp -R "${work}/base" "${work}/reg"
put reg case_a metal-mac-mini ng
differ "regressed" base reg "o:regressed: test=case_a combination=metal-mac-mini before=ok after=ng" "o:regressed=1" "o:same=3" "e:DIFFERENT: 1 difference(s)"

run_tool "${work}/reg" "${work}/base"
judge "improved" 1 "o:improved: test=case_a combination=metal-mac-mini before=ng after=ok" "o:improved=1" "!regressed:" "e:DIFFERENT: 1 difference(s)"

cp -R "${work}/base" "${work}/reason"
put reason case_c cuda-node-a not-run permission-missing
differ "reason_changed" base reason "o:changed: test=case_c combination=cuda-node-a before=not-run/not-configured after=not-run/permission-missing" "o:changed=1"

cp -R "${work}/base" "${work}/label_nonok"
put label_nonok case_d vulkan-intel indeterminate device-mismatch
differ "label_changed_non_ok" base label_nonok "o:changed: test=case_d combination=vulkan-intel before=pending/threshold-unapproved after=indeterminate/device-mismatch" "o:changed=1" "!regressed:" "!improved:"

cp -R "${work}/base" "${work}/added"
put added case_e cuda-node-b ok
differ "added" base added "o:added: test=case_e combination=cuda-node-b after=ok" "o:added=1" "!removed:"

run_tool "${work}/added" "${work}/base"
judge "removed" 1 "o:removed: test=case_e combination=cuda-node-b before=ok" "o:removed=1" "!added:"

# 同じ試験の ID でも組み合わせが違えば別の鍵
cp -R "${work}/base" "${work}/other_combo"
put other_combo case_a vulkan-amd ok
differ "same_id_other_combination_is_added" base other_combo "o:added: test=case_a combination=vulkan-amd after=ok" "o:added=1" "o:same=4"

# 退行と改善と他の差が同時にあっても、分類ごとに数える
cp -R "${work}/base" "${work}/mixed"
put mixed case_a metal-mac-mini ng
put mixed case_b vulkan-amd ok
put mixed case_c cuda-node-a not-run permission-missing
put mixed case_z cuda-node-b ok
rm "${work}/mixed/case_d.vulkan-intel.json"
differ "mixed" base mixed "o:regressed=1" "o:improved=1" "o:changed=1" "o:removed=1" "o:added=1" "o:same=0" "e:DIFFERENT: 5 difference(s)"

# 出力の並び（分類の順 → 鍵の昇順）の完全一致
cp -R "${work}/base" "${work}/order_new"
put order_new case_a metal-mac-mini ng
put order_new case_b vulkan-amd ok
put order_new case_c cuda-node-a not-run permission-missing
put order_new case_y cuda-node-b ok
put order_new case_x cuda-node-a ok
rm "${work}/order_new/case_d.vulkan-intel.json"
run_tool "${work}/base" "${work}/order_new"
want="regressed: test=case_a combination=metal-mac-mini before=ok after=ng
improved: test=case_b combination=vulkan-amd before=ng after=ok
changed: test=case_c combination=cuda-node-a before=not-run/not-configured after=not-run/permission-missing
removed: test=case_d combination=vulkan-intel before=pending/threshold-unapproved
added: test=case_x combination=cuda-node-a after=ok
added: test=case_y combination=cuda-node-b after=ok
summary: same=0 regressed=1 improved=1 changed=1 removed=1 added=2"
if [ "${rc}" -eq 1 ] && [ "$(cat "${OUT_F}")" = "${want}" ]; then
  report_ok "order_deterministic"
else
  report_fail "order_deterministic" "stdout is not the expected ordered lines (exit ${rc})"
fi

# ---- 読めない記録 ----

mkdir "${work}/empty"
misuse "empty_dir" base empty "e:violation: no-records" "e:error: new record is not readable"
misuse "empty_dir_previous" empty base "e:violation: no-records" "e:error: previous record is not readable"

cp -R "${work}/base" "${work}/malformed"
printf '{"behavior_ids":[]}\n' >"${work}/malformed/bad.json"
misuse "malformed_part" base malformed "e:violation: missing-field"
misuse "malformed_in_new_only" base malformed "e:error: new record is not readable"
misuse "malformed_in_previous_only" malformed base "e:error: previous record is not readable"

cp -R "${work}/base" "${work}/notjson"
printf 'not json\n' >"${work}/notjson/bad.json"
misuse "invalid_json_part" base notjson "e:violation: invalid-json"

cp -R "${work}/base" "${work}/badname"
printf '%s\n' "x" >"${work}/badname/readme.txt"
misuse "non_record_name" base badname "e:violation: not-a-record-name"

run_tool "${work}/base" "${work}/base/case_a.metal-mac-mini.json"
judge "not_a_directory" 2 "e:error: " "!ok: " "!summary: "
run_tool "${work}/no-such-dir" "${work}/base"
judge "missing_dir" 2 "e:error: " "!ok: " "!summary: " "!no-such-dir"
ln -s "${work}/base" "${work}/link_to_base"
run_tool "${work}/base" "${work}/link_to_base"
judge "symlink_dir" 2 "e:error: " "!ok: " "!summary: "
run_tool "${work}/base" "${work}/link_to_base/"
judge "symlink_dir_trailing_separator" 2 "e:error: " "!ok: " "!summary: "

# ---- 誤用 ----

run_tool
judge "args_0" 2 "e:usage: " "!ok: " "!summary: "
run_tool "${work}/base"
judge "args_1" 2 "e:usage: " "!ok: " "!summary: "
run_tool "${work}/base" "${work}/base" "${work}/base"
judge "args_3" 2 "e:usage: " "!ok: " "!summary: "
run_tool -x "${work}/base"
judge "dash_arg" 2 "e:usage: " "!ok: " "!summary: "
run_tool --bogusflagword "${work}/base" "${work}/base"
judge "unknown_option" 2 "e:usage: " "!bogusflagword" "!ok: " "!summary: "
run_tool --help
judge "help" 0 "o:usage: " "!summary: "

# ---- 入力を変更しない・ファイルを作らない ----

before="$(cat "${work}"/base/*.json "${work}"/mixed/*.json | cksum)"
empty_cwd="${work}/empty_cwd"
mkdir "${empty_cwd}"
rc=0
(cd "${empty_cwd}" && "${TOOL}" "${work}/base" "${work}/mixed" >"${OUT_F}" 2>"${ERR_F}" </dev/null) || rc=$?
after="$(cat "${work}"/base/*.json "${work}"/mixed/*.json | cksum)"
if [ "${rc}" -eq 1 ] && [ -z "$(ls -A "${empty_cwd}")" ]; then
  report_ok "creates_no_files"
else
  report_fail "creates_no_files" "expected exit 1 and an empty working directory"
fi
if [ "${before}" = "${after}" ]; then
  report_ok "inputs_unchanged"
else
  report_fail "inputs_unchanged" "input parts changed"
fi

echo "test-rerun-compare: passed=${passed} failed=${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
