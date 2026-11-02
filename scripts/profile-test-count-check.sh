#!/usr/bin/env bash
# プロファイルごとの試験の本数の検査（spec TASK-227・BUILD-10 のポインタ。親 Issue #1149 の 2/4）。
#
# 契約の crate の試験 `stub_device_profiles` を、`scripts/build-profiles.sh` の 3 つのプロファイルの組
# （`# spec BUILD-1`〜`BUILD-3` の注記の行）に `test-support` を足した組で流し、
# 通った本数と `graphics_` で始まる試験の本数を期待と照らす。
# `build-profiles.sh` は読むだけで、実行も source もしない。組の定義は注記の行の 1 か所に保つ。
#
# 呼び出し元: `make profile-test-count-check`（回帰テストは `make test-profile-test-count-check`）。
# 要るもの: bash・cargo・grep・sed・wc。リポジトリのファイルは変更しない。
#
# 使い方: scripts/profile-test-count-check.sh [--build-profiles <path>]
#   --build-profiles: 読む build-profiles.sh の差し替え（回帰テスト用）。
# 終了コード:
#   0: 3 組とも合格
#   1: どれかの組が不合格
#   2: 誤用・前提の不足（cargo が無い）・読めない入力（注記の行が 0 行か 2 行以上・引用符が無い・組の文字が不正）
# 出力: 組ごとに標準出力へ `record: profile=<名前> features=<組>,test-support passed=<N> graphics=<数>`、
#   全組合格なら最後に `profile-test-count-check: all profiles passed`。不合格の理由は標準エラーの `FAILED:` の行。
#
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# プロファイルの並び。
NAMES="CpuOnly ComputeOnly Graphics"
# 期待の定数（1 か所）。`crates/contract/tests/stub_device_profiles.rs` の `#[test]` を
# 足す・消すときは、同じ変更でここを直す。
expected_passed() {
  case "$1" in
    CpuOnly) echo 6 ;;
    ComputeOnly) echo 6 ;;
    Graphics) echo 9 ;;
  esac
}
expected_graphics() {
  case "$1" in
    CpuOnly) echo 0 ;;
    ComputeOnly) echo 0 ;;
    Graphics) echo 3 ;;
  esac
}
# build-profiles.sh の注記の行の番号（`# spec BUILD-<n>`）。
note_number() {
  case "$1" in
    CpuOnly) echo 1 ;;
    ComputeOnly) echo 2 ;;
    Graphics) echo 3 ;;
  esac
}

usage() {
  echo "usage: $(basename "$0") [--build-profiles <path>]" >&2
  exit 2
}

SCRIPT_PATH="${ROOT_DIR}/scripts/build-profiles.sh"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --build-profiles)
      [ "$#" -ge 2 ] || usage
      SCRIPT_PATH="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done
if [ ! -f "${SCRIPT_PATH}" ] || [ ! -r "${SCRIPT_PATH}" ]; then
  echo "error: cannot read the profile script" >&2
  exit 2
fi
# 相対パスは cd の前に絶対にしておく。
case "${SCRIPT_PATH}" in
  /*) ;;
  *) SCRIPT_PATH="$(pwd)/${SCRIPT_PATH}" ;;
esac
require_cmd cargo || exit 2

work="$(mktemp -d "${TMPDIR:-/tmp}/profile-test-count-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT

# 3 組とも先に取り出して検証する（読めない入力で途中まで流さない）。
for name in ${NAMES}; do
  n="$(note_number "${name}")"
  grep -E "# spec BUILD-${n}\$" "${SCRIPT_PATH}" >"${work}/line.${name}" || :
  lines="$(wc -l <"${work}/line.${name}" | tr -d ' ')"
  if [ "${lines}" -ne 1 ]; then
    echo "error: expected exactly 1 note line for profile ${name}, found ${lines}" >&2
    exit 2
  fi
  set_value="$(sed -n 's/^[^"]*"\([^"]*\)".*$/\1/p' "${work}/line.${name}")"
  if [ -z "${set_value}" ]; then
    echo "error: no quoted feature set on the note line for profile ${name}" >&2
    exit 2
  fi
  if ! printf '%s\n' "${set_value}" | grep -Eq '^[a-z0-9][a-z0-9,-]*$'; then
    echo "error: invalid characters in the feature set for profile ${name}" >&2
    exit 2
  fi
  echo "${set_value}" >"${work}/set.${name}"
done

unset FANDHE_SILICON_TEST_RECORD_DIR
cd "${ROOT_DIR}"

status=0
fail() {
  echo "FAILED: profile=$1 reason=$2" >&2
  status=1
}

for name in ${NAMES}; do
  features="$(cat "${work}/set.${name}"),test-support"
  want_passed="$(expected_passed "${name}")"
  want_graphics="$(expected_graphics "${name}")"
  out="${work}/out.${name}"
  err="${work}/err.${name}"
  log "cargo test -p fandhe-silicon-contract --no-default-features --features ${features} --test stub_device_profiles"
  rc=0
  cargo test -p fandhe-silicon-contract --no-default-features --features "${features}" \
    --test stub_device_profiles >"${out}" 2>"${err}" || rc=$?

  ok=1
  if [ "${rc}" -ne 0 ]; then
    fail "${name}" cargo-exit
    ok=0
  fi
  grep -E '^test result: ' "${out}" >"${work}/res.${name}" || :
  res_lines="$(wc -l <"${work}/res.${name}" | tr -d ' ')"
  passed="-"
  if [ "${res_lines}" -ne 1 ]; then
    fail "${name}" result-lines
    ok=0
  else
    passed="$(sed -n 's/^test result: ok\. \([0-9][0-9]*\) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out.*$/\1/p' "${work}/res.${name}")"
    if [ -z "${passed}" ]; then
      fail "${name}" result-format
      passed="-"
      ok=0
    elif [ "${passed}" -ne "${want_passed}" ]; then
      fail "${name}" passed-count
      ok=0
    fi
  fi
  grep -E '^test [^ ]+ \.\.\. ' "${out}" >"${work}/tests.${name}" || :
  grep -E '^test [^ ]+ \.\.\. ok$' "${work}/tests.${name}" >"${work}/ok.${name}" || :
  ok_lines="$(wc -l <"${work}/ok.${name}" | tr -d ' ')"
  all_lines="$(wc -l <"${work}/tests.${name}" | tr -d ' ')"
  if [ "${all_lines}" -ne "${ok_lines}" ]; then
    fail "${name}" non-ok-lines
    ok=0
  fi
  if [ "${passed}" != "-" ] && [ "${ok_lines}" -ne "${passed}" ]; then
    fail "${name}" ok-lines
    ok=0
  fi
  grep -E '^test graphics_[^ ]* \.\.\. ok$' "${work}/ok.${name}" >"${work}/gr.${name}" || :
  graphics="$(wc -l <"${work}/gr.${name}" | tr -d ' ')"
  if [ "${graphics}" -ne "${want_graphics}" ]; then
    fail "${name}" graphics-count
    ok=0
  fi
  echo "record: profile=${name} features=${features} passed=${ok_lines} graphics=${graphics}"
  if [ "${ok}" -eq 0 ]; then
    # 判定に使った行だけを出す（cargo の生の出力は絶対パスを含みうるので流さない）。
    cat "${work}/res.${name}" "${work}/tests.${name}" >&2
  fi
done

if [ "${status}" -ne 0 ]; then
  exit 1
fi
echo "profile-test-count-check: all profiles passed"
