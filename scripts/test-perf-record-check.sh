#!/usr/bin/env bash
# perf-record-check.sh の回帰テスト（spec TASK-27・TASK-28・TASK-2947 のポインタ）。
# 一時ディレクトリの fixture（合成のダミー値）で、終了コード・標準出力・標準エラーの文言・
# 入力の値の秘匿・入力を変更しないことを確かめる。cargo は呼ばない。
# 呼び出し元: `make test-perf-record-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
#
# 判定は「終了コード + 標準出力の完全一致 + 標準エラーの部分一致」で行う。終了コードだけでは
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

CHECK="${DIR}/perf-record-check.sh"
RUN_EX="${ROOT_DIR}/docs/run-record.example.json"
SESSION_EX="${ROOT_DIR}/docs/session-record.example.json"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-perf-record-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
IN_F="${work}/run.json"
IN2_F="${work}/run2.json"
SES_F="${work}/session.json"
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
MUT_PY="${work}/mut.py"
TXT_PY="${work}/txt.py"
passed=0
failed=0
rc=0

# 記入例から 1 か所だけ変えた fixture を作る Python。式は合成の固定文字列（d は辞書）。
cat >"${MUT_PY}" <<'PY'
import json, re, sys
# 記入例の小数（loadavg の固定 3 桁）は綴りのまま書き戻す。式で足す小数は通常の書き方になる。
d = json.load(open(sys.argv[1]), object_pairs_hook=lambda p: dict(p), parse_float=lambda t: "__F:" + t)
exec(sys.argv[3])
text = re.sub(r'"__F:([^"]*)"', r"\1", json.dumps(d))
with open(sys.argv[2], "w", encoding="utf-8") as f:
    f.write(text)
PY

# 記入例の文字列を 1 か所だけ置き換える Python（辞書を経由しない生の文字列の fixture 用）。
cat >"${TXT_PY}" <<'PY'
import sys
b = open(sys.argv[1], "rb").read()
b = b.replace(sys.argv[2].encode("utf-8", "surrogateescape"), sys.argv[3].encode("utf-8", "surrogateescape"), 1)
sys.stdout.buffer.write(b)
PY

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

# mut <src> <dst> <python 文>
mut() {
  python3 -I "${MUT_PY}" "$1" "$2" "$3"
}

# good <名前> <python 文>: 変えた run が形の検査に合格すること。
good() {
  mut "${RUN_EX}" "${IN_F}" "$2"
  run_check "${IN_F}"
  judge "$1" 0 "ok: run record shape"
}

# bad <名前> <python 文> [部分一致...]: 変えた run が違反（1）になること。
bad() {
  local name="$1" expr="$2"
  shift 2
  mut "${RUN_EX}" "${IN_F}" "${expr}"
  run_check "${IN_F}"
  judge "${name}" 1 "" "FAILED:" "$@"
}

# bad_raw <名前> <部分一致...>: ${IN_F} に在る内容が違反（1）になること。
bad_raw() {
  local name="$1"
  shift
  run_check "${IN_F}"
  judge "${name}" 1 "" "$@"
}

# raw <置き換え前> <置き換え後>: 記入例の文字列を 1 か所置き換えた fixture を ${IN_F} に作る。
raw() {
  python3 -I "${TXT_PY}" "${RUN_EX}" "$1" "$2" >"${IN_F}"
}

misuse() {
  local name="$1"
  shift
  run_check "$@"
  judge "${name}" 2 ""
}

# sess <名前> <期待コード> <期待の標準出力> <python 文(回)> <python 文(run)> [部分一致...]
# 記入例の回と run を変えて --session で流す。
sess() {
  local name="$1" want="$2" out_want="$3" sexpr="$4" rexpr="$5"
  shift 5
  mut "${SESSION_EX}" "${SES_F}" "${sexpr}"
  mut "${RUN_EX}" "${IN_F}" "${rexpr}"
  run_check --session "${SES_F}" "${IN_F}"
  judge "${name}" "${want}" "${out_want}" "$@"
}

NL=$'\n'

# 回の記録と同じディレクトリに runs/<ID>.json を置いた並び（実在の検査の前提）。
mkdir "${work}/runs"
cp "${RUN_EX}" "${work}/runs/example-run-a.json"
mut "${RUN_EX}" "${work}/runs/example-run-b.json" 'd["run_id"] = "example-run-b"'

# ---- 形の検査: 合格 ----

run_check "${RUN_EX}"
judge "pass: example in repo" 0 "ok: run record shape"

good "pass: field order changed" 'd = dict(reversed(list(d.items())))'
good "pass: label with reason" 'd["label"] = "pending"; d["label_reason"] = "threshold-unapproved"'
good "pass: label not-run" 'd["label"] = "not-run"; d["label_reason"] = "permission-missing"'
good "pass: label indeterminate" 'd["label"] = "indeterminate"; d["label_reason"] = "machine-unreachable"'
good "pass: session none" 'd["session_ref"] = "_none"'
good "pass: baseline crates-io" 'd["wgpu_baseline"] = {"version": "0.0.0", "source": "crates-io", "lockfile_sha256": "_unavailable", "basis": "BASIS-0"}'
good "pass: baseline git" 'd["wgpu_baseline"] = {"version": "0.0.0", "source": "git", "git_repository": "https://example.invalid/w.git", "git_tag": "example-tag", "git_commit": "_unavailable", "lockfile_sha256": "_unavailable", "basis": "BASIS-0"}'
good "pass: vulkan env" 'e = d["environment"]; del e["xcode_version"], e["metal_toolchain"]; e["driver_version"] = "0.0.1"; e["sdk_versions"] = {"vulkan-sdk": "0.0.2"}; d["machine_combination"] = "vulkan-node-a"; d["machine"] = "node-a"'
good "pass: cuda env nulls" 'e = d["environment"]; del e["xcode_version"], e["metal_toolchain"]; e["driver_version"] = "_unavailable"; e["sdk_versions"] = None; d["machine_combination"] = "cuda-node-b"; d["machine"] = "node-b"'
good "pass: gpu sample unavailable" 'd["body"]["gpu_ns"]["measured"][1] = None; d["body"]["stats"]["gpu_ns"] = None'
good "pass: unavailable strings and nulls" 'e = d["environment"]; e["gpu"] = "_unavailable"; e["feature_set"] = None; e["before"]["low_power_mode"] = None; e["before"]["loadavg"] = None; d["evidence"]["output_sha256"] = "_no-output"; d["evidence"]["kernel_sha256"] = None'
good "pass: processed count and instructions" 'd["body"]["processed_count"] = 7; d["body"]["instructions_found"] = {"example-insn": True}'
good "pass: median with half" 'd["body"]["stats"]["cpu_ns"]["median"] = 20.5'
good "pass: equal start and end" 'd["ended_utc"] = d["started_utc"]'

# ---- 形の検査: 欄の集合 ----

for f in schema_version run_id task_ids behavior_ids machine_combination machine label body_kind \
  started_utc ended_utc session_ref evidence environment body; do
  bad "set: missing ${f}" "del d['${f}']" "field ${f}: missing"
done
for f in adapter_backend adapter_name kernel_sha256 artifact_sha256 input_sha256 output_sha256; do
  bad "set: missing evidence.${f}" "del d['evidence']['${f}']" "field evidence.${f}: missing"
done
for f in gpu cpu os_name os_version rustc_version repo_commit cargo_lock_sha256 build_profile \
  feature_set target_features shader_compile_options library_versions validation_layers \
  xcode_version metal_toolchain before after; do
  bad "set: missing environment.${f}" "del d['environment']['${f}']" "field environment.${f}: missing"
done
for f in utc power_source low_power_mode thermal_warning loadavg; do
  bad "set: missing environment.before.${f}" "del d['environment']['before']['${f}']" "field environment.before.${f}: missing"
done
for f in sample_counts submission_count dispatch_count processed_count instructions_found cpu_ns gpu_ns stats; do
  bad "set: missing body.${f}" "del d['body']['${f}']" "field body.${f}: missing"
done
bad "set: extra top-level field" 'd["zzz-extra-key"] = 1' "1 field(s) not in the table" "!zzz-extra-key"
bad "set: extra nested field" 'd["environment"]["zzz-extra-key"] = 1' "1 field(s) not in the table at environment" "!zzz-extra-key"
bad "set: label_reason without reason label" 'd["label_reason"] = "threshold-unapproved"' "1 field(s) not in the table"
bad "set: reason label without label_reason" 'd["label"] = "pending"' "field label_reason: missing"
bad "set: wrong reason for label" 'd["label"] = "pending"; d["label_reason"] = "permission-missing"' "field label_reason: invalid value"
bad "set: vulkan fields on metal combination" 'd["environment"]["driver_version"] = "0.0.1"' "1 field(s) not in the table at environment"
bad "set: metal fields on vulkan combination" 'd["machine_combination"] = "vulkan-node-a"' "field environment.driver_version: missing"
bad "set: git fields on crates-io baseline" 'd["wgpu_baseline"] = {"version": "0.0.0", "source": "crates-io", "git_tag": "t", "lockfile_sha256": "_unavailable", "basis": "BASIS-0"}' "1 field(s) not in the table at wgpu_baseline"
bad "set: git baseline lacks git fields" 'd["wgpu_baseline"] = {"version": "0.0.0", "source": "git", "lockfile_sha256": "_unavailable", "basis": "BASIS-0"}' "field wgpu_baseline.git_tag: missing"
bad "set: baseline basis unavailable word" 'd["wgpu_baseline"] = {"version": "0.0.0", "source": "crates-io", "lockfile_sha256": "_unavailable", "basis": "_unavailable"}' "field wgpu_baseline.basis: invalid value"
bad "set: stats key missing" 'del d["body"]["stats"]["cpu_ns"]["p95"]' "field body.stats.cpu_ns.p95: missing"
bad "set: sample_counts extra" 'd["body"]["sample_counts"]["x"] = 1' "1 field(s) not in the table at body.sample_counts"

# ---- 形の検査: 型・値の形 ----

bad "type: submission_count bool" 'd["body"]["submission_count"] = True' "field body.submission_count: invalid value"
bad "type: submission_count string" 'd["body"]["submission_count"] = "5"' "field body.submission_count: invalid value"
bad "type: submission_count float" 'd["body"]["submission_count"] = 5.5' "field body.submission_count: invalid value"
bad "type: submission_count negative" 'd["body"]["submission_count"] = -1' "field body.submission_count: invalid value"
bad "type: submission_count too large" 'd["body"]["submission_count"] = 1 << 64' "field body.submission_count: invalid value"
bad "type: sample bool" 'd["body"]["cpu_ns"]["measured"][0] = True' "field body.cpu_ns.measured: invalid value"
bad "type: cpu sample null" 'd["body"]["cpu_ns"]["measured"][0] = None' "field body.cpu_ns.measured: invalid value"
bad "type: gpu sample float" 'd["body"]["gpu_ns"]["measured"][0] = 1.5' "field body.gpu_ns.measured: invalid value"
good "pass: median at limit" 'd["body"]["stats"]["cpu_ns"]["median"] = 4503599627370496'
good "pass: median half below limit" 'd["body"]["stats"]["cpu_ns"]["median"] = 4503599627370495.5'
bad "type: median int over limit" 'd["body"]["stats"]["cpu_ns"]["median"] = 4503599627370497' "field body.stats.cpu_ns.median: invalid value"
bad "type: median half over limit" 'd["body"]["stats"]["cpu_ns"]["median"] = 4503599627370496.5' "field body.stats.cpu_ns.median: invalid value"
bad "type: median half over u64" 'd["body"]["stats"]["cpu_ns"]["median"] = 18446744073709551616.5' "field body.stats.cpu_ns.median: invalid value"
bad "type: median not half" 'd["body"]["stats"]["cpu_ns"]["median"] = 20.3' "field body.stats.cpu_ns.median: invalid value"
bad "type: median string" 'd["body"]["stats"]["cpu_ns"]["median"] = "20"' "field body.stats.cpu_ns.median: invalid value"
bad "type: cpu stats null" 'd["body"]["stats"]["cpu_ns"] = None' "field body.stats.cpu_ns: not an object"
bad "type: instructions value" 'd["body"]["instructions_found"] = {"a": 1}' "field body.instructions_found: invalid value"
bad "type: processed_count string" 'd["body"]["processed_count"] = "1"' "field body.processed_count: invalid value"
bad "type: bool field string" 'd["environment"]["before"]["low_power_mode"] = "false"' "field environment.before.low_power_mode: invalid value"
bad "type: loadavg wrong length" 'd["environment"]["before"]["loadavg"] = [1.000, 2.000]' "field environment.before.loadavg: invalid value"
bad "type: loadavg three digits needed" 'd["environment"]["before"]["loadavg"] = [1.0, 2.0, 3.0]' "field environment.before.loadavg: invalid value"
bad "type: body not object" 'd["body"] = []' "field body: invalid value"
bad "form: schema_version" 'd["schema_version"] = "2"' "field schema_version: invalid value"
bad "form: run_id upper" 'd["run_id"] = "Run-1"' "field run_id: invalid value"
bad "form: run_id reserved" 'd["run_id"] = "_none"' "field run_id: invalid value"
bad "form: session_ref upper" 'd["session_ref"] = "_NONE"' "field session_ref: invalid value"
bad "form: task id lower" 'd["task_ids"] = ["task-27"]' "field task_ids: invalid value"
bad "form: machine" 'd["machine"] = "laptop"' "field machine: invalid value"
bad "form: combination" 'd["machine_combination"] = "metal-node-a"' "field machine_combination: invalid value"
bad "form: label" 'd["label"] = "maybe"' "field label: invalid value"
bad "form: body_kind" 'd["body_kind"] = "other"' "field body_kind: invalid value"
bad "form: time not a date" 'd["started_utc"] = "2000-02-30T00:00:00Z"' "field started_utc: invalid value"
bad "form: time no Z" 'd["started_utc"] = "2000-01-01T00:00:00"' "field started_utc: invalid value"
bad "form: sha upper" 'd["evidence"]["output_sha256"] = "A" * 64' "field evidence.output_sha256: invalid value"
bad "form: sha short" 'd["evidence"]["kernel_sha256"] = {"k": "00"}' "field evidence.kernel_sha256: invalid value"
bad "form: map key reserved" 'd["evidence"]["kernel_sha256"] = {"_k": "0" * 64}' "field evidence.kernel_sha256: invalid value"
bad "form: no_output outside output_sha256" 'd["environment"]["gpu"] = "_no-output"' "field environment.gpu: invalid value"
bad "form: unavailable in label" 'd["label"] = "_unavailable"' "field label: invalid value"
bad "form: commit length" 'd["environment"]["repo_commit"] = "0" * 64' "field environment.repo_commit: invalid value"
bad "form: os_name" 'd["environment"]["os_name"] = "plan9"' "field environment.os_name: invalid value"
bad "form: power_source" 'd["environment"]["before"]["power_source"] = "solar"' "field environment.before.power_source: invalid value"
bad "form: option token slash" 'd["environment"]["feature_set"] = ["a/b"]' "field environment.feature_set: invalid value"
bad "form: device name leading space" 'd["environment"]["gpu"] = " x"' "field environment.gpu: invalid value"
bad "form: baseline version leading zero" 'd["wgpu_baseline"] = {"version": "01.0.0", "source": "crates-io", "lockfile_sha256": "_unavailable", "basis": "B"}' "field wgpu_baseline.version: invalid value"
bad "form: baseline repo http" 'd["wgpu_baseline"] = {"version": "0.0.0", "source": "git", "git_repository": "http://example.invalid/w.git", "git_tag": "t", "git_commit": "_unavailable", "lockfile_sha256": "_unavailable", "basis": "B"}' "field wgpu_baseline.git_repository: invalid value"
bad "form: unknown reserved word" 'd["environment"]["os_version"] = "_other"' "field environment.os_version: invalid value"

# ---- 形の検査: 配列の長さ・時刻順・整合 ----

bad "len: cpu cold differs" 'd["body"]["cpu_ns"]["cold"] = [1, 2]' "field body.cpu_ns.cold: length differs"
bad "len: gpu measured differs" 'd["body"]["gpu_ns"]["measured"] = [1]' "field body.gpu_ns.measured: length differs"
bad "len: measured zero" 'd["body"]["sample_counts"]["measured"] = 0; d["body"]["cpu_ns"]["measured"] = []; d["body"]["gpu_ns"]["measured"] = []' "field body.sample_counts.measured: invalid value"
bad "gpu: stats present with null sample" 'd["body"]["gpu_ns"]["measured"][1] = None' "field body.stats.gpu_ns: must be null"
bad "gpu: stats null without null sample" 'd["body"]["stats"]["gpu_ns"] = None' "field body.stats.gpu_ns: must not be null"
bad "order: started after ended" 'd["ended_utc"] = "2000-01-01T00:01:00Z"' "started_utc is after ended_utc"

# ---- 入力の読み ----

printf '%s' '{"a":1,"a":2}' >"${IN_F}"
bad_raw "input: duplicate field" "duplicate field"
raw '"run_id":"example-run-a"' '"run_id":"example-run-a","run_id":"x"'
bad_raw "input: duplicate field in example" "duplicate field"
raw '"median":20' '"median":NaN'
bad_raw "input: NaN" "not valid JSON"
raw '"median":20' '"median":Infinity'
bad_raw "input: Infinity" "not valid JSON"
raw '"median":20' '"median":2e1'
bad_raw "input: exponent number" "field body.stats.cpu_ns.median: invalid value"
raw 'Example CPU' 'Exampl'$'\xc3\xa9'
bad_raw "input: non-ascii bytes" "not ASCII"
printf '%s' '[]' >"${IN_F}"
bad_raw "input: top level array" "top level is not an object"
printf '%s' '{' >"${IN_F}"
bad_raw "input: truncated" "not valid JSON"
: >"${IN_F}"
bad_raw "input: empty file" "not valid JSON"
printf '%s' '"x"' >"${IN_F}"
bad_raw "input: top level string" "top level is not an object"
python3 -I -c 'print("[" * 100000)' >"${IN_F}"
bad_raw "input: deep nesting" "not valid JSON"
head -c 9437184 /dev/zero | tr '\0' ' ' >"${IN_F}"
bad_raw "input: too large" "too large"

# ---- 誤用 ----

misuse "misuse: no arguments"
misuse "misuse: unknown option" --bogus "${RUN_EX}"
misuse "misuse: two run paths without --session" "${RUN_EX}" "${RUN_EX}"
misuse "misuse: --session without value" --session
misuse "misuse: missing path" "${work}/missing.json"
run_check "${work}"
judge "input: directory" 1 "" "not a regular file"
misuse "misuse: session path missing" --session "${work}/missing.json" "${RUN_EX}"
mut "${SESSION_EX}" "${SES_F}" 'pass'
run_check --session "${SES_F}" "${work}/missing.json"
judge "misuse: run path missing in session mode" 2 "session: eligible"

# ---- 秘匿 ----

bad "secret: value not echoed" 'd["environment"]["gpu"] = "SECRET/MARKER-VALUE"' "field environment.gpu: invalid value" "!MARKER-VALUE"
bad "secret: extra key name not echoed" 'd["SECRET-MARKER-KEY"] = 1' "1 field(s) not in the table" "!SECRET-MARKER-KEY"
bad "secret: id value not echoed" 'd["run_id"] = "SECRET-MARKER-ID"' "field run_id: invalid value" "!SECRET-MARKER-ID"

# ---- 判定に使えるか（--session）----

mut "${SESSION_EX}" "${SES_F}" 'pass'
run_check --session "${SES_F}" "${RUN_EX}"
judge "session: examples are eligible" 0 "session: eligible${NL}run 1: eligible"
run_check --session "${SES_F}"
judge "session: no runs" 0 "session: eligible"
run_check --session "${SES_F}" "${work}/runs/example-run-a.json"
judge "session: run record from runs dir" 0 "session: eligible${NL}run 1: eligible"
run_check --session "${SESSION_EX}" "${RUN_EX}"
judge "session: docs examples have no runs dir" 1 "session: excluded: run-record-missing${NL}run 1: excluded: session-excluded" "run_ids[1]: run record not found" "!example-run-a"

sess "session: mismatch" 1 "session: eligible${NL}run 1: excluded: session-mismatch" 'pass' 'd["session_ref"] = "other-session"'
sess "session: run without session" 1 "session: eligible${NL}run 1: excluded: no-session" 'pass' 'd["session_ref"] = "_none"'
sess "session: window start" 1 "session: eligible${NL}run 1: excluded: outside-session-window" 'pass' 'd["started_utc"] = "2000-01-01T00:01:00Z"'
sess "session: window end" 1 "session: eligible${NL}run 1: excluded: outside-session-window" 'pass' 'd["ended_utc"] = "2000-01-01T00:04:00Z"'
sess "session: battery before" 1 "session: eligible${NL}run 1: excluded: power-not-ac" 'pass' 'd["environment"]["before"]["power_source"] = "battery"'
sess "session: power unavailable after" 1 "session: eligible${NL}run 1: excluded: power-not-ac" 'pass' 'd["environment"]["after"]["power_source"] = "_unavailable"'
sess "session: low power before" 1 "session: eligible${NL}run 1: excluded: low-power-mode" 'pass' 'd["environment"]["before"]["low_power_mode"] = True'
sess "session: low power after" 1 "session: eligible${NL}run 1: excluded: low-power-mode" 'pass' 'd["environment"]["after"]["low_power_mode"] = True'
sess "session: thermal warning" 1 "session: eligible${NL}run 1: excluded: thermal-warning" 'pass' 'd["environment"]["after"]["thermal_warning"] = True'
sess "session: validation layers" 1 "session: eligible${NL}run 1: excluded: validation-layers-enabled" 'pass' 'd["environment"]["validation_layers"] = "enabled"'
sess "session: unavailable env string" 1 "session: eligible${NL}run 1: excluded: unavailable-field" 'pass' 'd["environment"]["gpu"] = "_unavailable"'
sess "session: unavailable env null" 1 "session: eligible${NL}run 1: excluded: unavailable-field" 'pass' 'd["environment"]["before"]["thermal_warning"] = None'
sess "session: unavailable evidence null" 1 "session: eligible${NL}run 1: excluded: unavailable-field" 'pass' 'd["evidence"]["input_sha256"] = None'
sess "session: unavailable output" 1 "session: eligible${NL}run 1: excluded: unavailable-field" 'pass' 'd["evidence"]["output_sha256"] = "_unavailable"'
sess "session: no output is usable" 0 "session: eligible${NL}run 1: eligible" 'pass' 'd["evidence"]["output_sha256"] = "_no-output"'
sess "session: invalid run shape" 1 "session: eligible${NL}run 1: excluded: invalid-shape" 'pass' 'del d["body"]' "field body: missing"
sess "session: low power mode" 1 "session: excluded: low-power-mode${NL}run 1: excluded: session-excluded" 'd["low_power_mode"] = True' 'pass'
sess "session: interrupted" 1 "session: excluded: interrupted${NL}run 1: excluded: session-excluded" 'd["interrupted"] = True' 'pass'

mut "${SESSION_EX}" "${SES_F}" 'd["low_power_mode"] = True'
run_check --session "${SES_F}"
judge "session: low power mode without runs" 1 "session: excluded: low-power-mode"
mut "${SESSION_EX}" "${SES_F}" 'del d["session"]'
run_check --session "${SES_F}" "${RUN_EX}"
judge "session: invalid session record" 1 "" "field session: missing"
mut "${SESSION_EX}" "${SES_F}" 'pass'
mut "${RUN_EX}" "${IN_F}" 'pass'
run_check --session "${SES_F}" "${IN_F}" "${IN_F}"
judge "session: duplicate run id" 1 "session: eligible${NL}run 1: eligible${NL}run 2: excluded: duplicate-run-id"
mut "${RUN_EX}" "${IN2_F}" 'd["run_id"] = "example-run-b"'
run_check --session "${SES_F}" "${IN_F}" "${IN2_F}"
judge "session: two eligible runs" 0 "session: eligible${NL}run 1: eligible${NL}run 2: eligible"
mut "${RUN_EX}" "${IN2_F}" 'd["run_id"] = "example-run-b"; d["environment"]["before"]["power_source"] = "ups"'
run_check --session "${SES_F}" "${IN_F}" "${IN2_F}"
judge "session: one of two excluded" 1 "session: eligible${NL}run 1: eligible${NL}run 2: excluded: power-not-ac"
run_check --session "${SES_F}" "${IN_F}" "${work}/missing.json"
judge "session: stops with 2 on missing run path" 2 "session: eligible${NL}run 1: eligible"

# ---- 回の run_ids との突き合わせ（所属・実在。spec TASK-2947 のポインタ）----

# 走査しないことの確かめ用: run_ids に無いファイルと、ディレクトリの entry。
printf '%s' '{}' >"${work}/runs/unlisted.json"
mkdir "${work}/runs/dir-entry.json"
sess "session: run_id not in run_ids" 1 "session: eligible${NL}run 1: excluded: not-in-run-ids" 'pass' 'd["run_id"] = "orphan-run"'
mut "${SESSION_EX}" "${SES_F}" 'd["run_ids"] = []'
mut "${RUN_EX}" "${IN_F}" 'pass'
run_check --session "${SES_F}"
judge "session: empty run_ids and no run" 0 "session: eligible"
run_check --session "${SES_F}" "${IN_F}"
judge "session: empty run_ids with a run" 1 "session: eligible${NL}run 1: excluded: not-in-run-ids"
sess "session: mismatch wins over not in run_ids" 1 "session: eligible${NL}run 1: excluded: session-mismatch" 'pass' 'd["session_ref"] = "other-session"; d["run_id"] = "orphan-run"'
sess "session: not in run_ids wins over window" 1 "session: eligible${NL}run 1: excluded: not-in-run-ids" 'pass' 'd["run_id"] = "orphan-run"; d["started_utc"] = "2000-01-01T00:01:00Z"'
sess "session: subset of runs is enough" 0 "session: eligible${NL}run 1: eligible" 'pass' 'pass'
sess "session: missing run record" 1 "session: excluded: run-record-missing${NL}run 1: excluded: session-excluded" 'd["run_ids"] = ["example-run-a", "example-run-b", "secret-marker-id"]' 'pass' "run_ids[3]: run record not found" "!secret-marker-id"
sess "session: low power wins over missing record" 1 "session: excluded: low-power-mode${NL}run 1: excluded: session-excluded" 'd["low_power_mode"] = True; d["run_ids"] = ["secret-marker-id"]' 'pass' "run_ids[1]: run record not found" "!secret-marker-id"
sess "session: interrupted wins over missing record" 1 "session: excluded: interrupted${NL}run 1: excluded: session-excluded" 'd["interrupted"] = True; d["run_ids"] = ["secret-marker-id"]' 'pass' "run_ids[1]: run record not found"
sess "session: extra file in runs is ignored" 0 "session: eligible${NL}run 1: eligible" 'pass' 'pass'
sess "session: runs entry is a directory" 1 "session: excluded: run-record-missing${NL}run 1: excluded: session-excluded" 'd["run_ids"] = ["example-run-a", "dir-entry"]' 'pass' "run_ids[2]: run record not found"
mut "${SESSION_EX}" "${SES_F}" 'pass'
mut "${RUN_EX}" "${IN_F}" 'pass'
mkdir "${work}/sub"
rc=0
(cd "${work}/sub" && "${BASH}" "${CHECK}" --session ../session.json ../run.json) >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "session: relative paths from another cwd" 0 "session: eligible${NL}run 1: eligible"
rmdir "${work}/sub"

# ---- 入力を変更しない ----

before="$(cksum <"${RUN_EX}")"
files_before="$(find "${work}" | wc -l)"
run_check "${RUN_EX}"
after="$(cksum <"${RUN_EX}")"
files_after="$(find "${work}" | wc -l)"
if [ "${before}" = "${after}" ] && [ "${files_before}" = "${files_after}" ] && [ "${rc}" -eq 0 ]; then
  report_ok "invariant: input and directory unchanged"
else
  report_fail "invariant: input and directory unchanged" "changed"
fi

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
