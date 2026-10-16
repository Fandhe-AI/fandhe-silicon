#!/usr/bin/env bash
# scripts/baseline-positive-control.sh の回帰テスト（最小版）。
# 呼び出し元: `make test-baseline-positive-control` と CI の `deny` ジョブ。
# 本体と lib.sh を一時ディレクトリの scripts/ へ写し、隣に偽の検査 2 本を置いて終了コードの写しを確かめる
# （本体は兄弟のスクリプトを DIR 経由で呼ぶため、写しが要る）。cargo・ネットワーク不要。
# リポジトリのファイルは変更しない。判定は終了コード + 標準出力 + 標準エラーの部分一致。bash 3.2 互換。
set -uo pipefail
SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/scripts" "${TMP}/baseline-wgpu"
cp "${SRC_DIR}/baseline-positive-control.sh" "${SRC_DIR}/lib.sh" "${TMP}/scripts/"
: > "${TMP}/baseline-wgpu/Cargo.toml"

# 本体の前提ツール確認（require_cmd）を通すためのダミー。実物がある環境では実物を優先し（PATH の末尾に足す）、
# 無い環境でも合成入力だけで回る。偽の検査が本物の cargo 等を呼ぶことはない。
mkdir -p "${TMP}/stub-bin"
for tool in python3 cargo git; do
  printf '#!/bin/sh\nexit 0\n' > "${TMP}/stub-bin/${tool}"
  chmod +x "${TMP}/stub-bin/${tool}"
done
export PATH="${PATH}:${TMP}/stub-bin"

# 偽の検査: FAKE_<名前>_RC の終了コードで終わり、record: を出し、受け取った引数を記録する。
for name in dep-graph-wgpu-check public-api-check; do
  var="FAKE_$(echo "${name}" | tr 'a-z-' 'A-Z_')_RC"
  cat > "${TMP}/scripts/${name}.sh" <<FAKE
#!/usr/bin/env bash
echo "\$*" >> "${TMP}/args-${name}"
echo "record: fake=${name}"
exit "\${${var}:-1}"
FAKE
  chmod +x "${TMP}/scripts/${name}.sh"
done

passed=0
failed=0
OUT=""
RC=0
# run_case <名前> <期待終了コード> <出力に含むべき部分文字列> [引数...]（環境変数は呼び出し側で設定）
run_case() {
  local label="$1" want_rc="$2" want_text="$3"
  shift 3
  rm -f "${TMP}/args-dep-graph-wgpu-check" "${TMP}/args-public-api-check"
  OUT="$("${TMP}/scripts/baseline-positive-control.sh" "$@" 2>&1)"
  RC=$?
  if [ "${RC}" -eq "${want_rc}" ] && printf '%s' "${OUT}" | grep -qF -- "${want_text}"; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    echo "FAIL: ${label} (rc=${RC}, want=${want_rc}, text=${want_text})" >&2
    printf '%s\n' "${OUT}" >&2
  fi
}
check_args() {
  local label="$1" file="$2" want="$3" unwant="$4"
  local got
  got="$(cat "${TMP}/${file}" 2>/dev/null || true)"
  if printf '%s' "${got}" | grep -qF -- "${want}" && { [ -z "${unwant}" ] || ! printf '%s' "${got}" | grep -qF -- "${unwant}"; }; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    echo "FAIL: ${label} (args=${got})" >&2
  fi
}

export FAKE_DEP_GRAPH_WGPU_CHECK_RC=1 FAKE_PUBLIC_API_CHECK_RC=1
run_case "both detect -> 0" 0 "ok: positive control detected"
check_args "manifest passed to dep-graph" args-dep-graph-wgpu-check "--manifest-path ${TMP}/baseline-wgpu/Cargo.toml" ""
check_args "superset flag only on dep-graph" args-dep-graph-wgpu-check "--allow-metadata-superset" ""
check_args "manifest passed to public-api" args-public-api-check "--manifest-path ${TMP}/baseline-wgpu/Cargo.toml" "--allow-metadata-superset"

FAKE_DEP_GRAPH_WGPU_CHECK_RC=0
run_case "dep-graph zero -> 1" 1 "positive-control-failed: check=dep-graph-wgpu-check inner_exit=0 result=no-detection"
check_args "public-api still ran after dep-graph zero" args-public-api-check "--manifest-path" ""

FAKE_DEP_GRAPH_WGPU_CHECK_RC=1
FAKE_PUBLIC_API_CHECK_RC=0
run_case "public-api zero -> 1" 1 "check=public-api-check inner_exit=0 result=no-detection"

FAKE_PUBLIC_API_CHECK_RC=2
run_case "public-api error -> 2" 2 "check=public-api-check inner_exit=2 result=error"

FAKE_DEP_GRAPH_WGPU_CHECK_RC=0
FAKE_PUBLIC_API_CHECK_RC=2
run_case "error beats zero -> 2" 2 "result=error"

FAKE_DEP_GRAPH_WGPU_CHECK_RC=1
FAKE_PUBLIC_API_CHECK_RC=1
run_case "unknown argument -> 2" 2 "usage:" --bogus

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
