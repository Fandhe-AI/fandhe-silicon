#!/usr/bin/env bash
# public-api-check.sh の回帰テスト（spec TASK-10 のポインタ。親 Issue #363）。
# 一時ディレクトリの合成 JSON だけを `--json` で渡し、cargo は呼ばない。リポジトリのファイルは変更しない。
# 呼び出し元: 直接実行（make ターゲットは持たない）。
# 判定は「終了コード + 標準エラーの部分一致」で行う。要る構造が欠けた入力を成功扱いにしない
# （fail-open）退行を見逃さないため。途中で止めず、最後に件数を出す。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

CHECK="${DIR}/public-api-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-public-api-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
passed=0
failed=0

# run_case <名前> <期待する終了コード> <標準エラーに含まれるべき語（空なら不問）> <JSON>
run_case() {
    local name="$1" want="$2" needle="$3" json="$4" rc=0
    printf '%s' "${json}" >"${work}/in.json"
    "${CHECK}" --json "${work}/in.json" >"${work}/out.txt" 2>"${work}/err.txt" || rc=$?
    if [ "${rc}" -ne "${want}" ] || { [ -n "${needle}" ] && ! grep -q -- "${needle}" "${work}/err.txt"; }; then
        echo "FAIL: ${name} (exit ${rc}, want ${want})" >&2
        failed=$((failed + 1))
    else
        passed=$((passed + 1))
    fi
}

ROOT='"0":{"crate_id":0,"name":"c","visibility":"public","inner":{"module":{}}}'
HEAD='{"root":0,"includes_private":false,"format_version":57,"index":{'
run_case "valid, no references" 0 "" \
    "${HEAD}${ROOT}},\"paths\":{},\"external_crates\":{}}"
run_case "public item without inner" 2 "without a valid 'inner'" \
    "${HEAD}${ROOT},\"1\":{\"crate_id\":0,\"visibility\":\"public\"}},\"paths\":{},\"external_crates\":{}}"
run_case "root missing from index" 2 "'root'" \
    "${HEAD}\"1\":{\"crate_id\":0,\"visibility\":\"public\",\"inner\":{}}},\"paths\":{},\"external_crates\":{}}"
run_case "root without inner" 2 "'root'" \
    "${HEAD}\"0\":{\"crate_id\":0,\"visibility\":\"public\"}},\"paths\":{},\"external_crates\":{}}"
run_case "wgpu reference" 1 "violation:" \
    "${HEAD}${ROOT},\"1\":{\"crate_id\":0,\"name\":\"f\",\"visibility\":\"public\",\"inner\":{\"x\":{\"id\":7}}}},\"paths\":{\"7\":{\"crate_id\":1}},\"external_crates\":{\"1\":{\"name\":\"wgpu\"}}}"

run_case "unresolved reference id" 2 "unresolved reference" \
    "${HEAD}${ROOT},\"1\":{\"crate_id\":0,\"name\":\"f\",\"visibility\":\"public\",\"inner\":{\"x\":{\"id\":7}}}},\"paths\":{},\"external_crates\":{}}"
run_case "internal reference resolved by index" 0 "" \
    "${HEAD}${ROOT},\"1\":{\"crate_id\":0,\"name\":\"f\",\"visibility\":\"public\",\"inner\":{\"x\":{\"id\":0}}}},\"paths\":{},\"external_crates\":{}}"
run_case "string reference id (old format)" 2 "non-integer" \
    "${HEAD}${ROOT},\"1\":{\"crate_id\":0,\"name\":\"f\",\"visibility\":\"public\",\"inner\":{\"x\":{\"id\":\"0:7\"}}}},\"paths\":{},\"external_crates\":{}}"
run_case "format_version missing" 2 "format_version" \
    '{"root":0,"includes_private":false,"index":{'"${ROOT}"'},"paths":{},"external_crates":{}}'
run_case "format_version is a string" 2 "format_version" \
    '{"root":0,"includes_private":false,"format_version":"1","index":{'"${ROOT}"'},"paths":{},"external_crates":{}}'

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
