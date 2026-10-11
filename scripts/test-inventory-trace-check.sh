#!/usr/bin/env bash
# inventory-trace-check.sh の回帰テスト（spec TASK-16 のポインタ）。
# 項目ディレクトリが列挙できない場合に、誤用の 2 ではなく読み取り失敗の 1 で終えることを
# 一時ディレクトリの fixture（ダミーの鍵）で確かめる。直接実行する（Make のターゲットは無い）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-inventory-trace-check.XXXXXX")"
trap 'chmod 700 "${work}/items" 2>/dev/null || true; rm -rf "${work:?}"' EXIT
mkdir "${work}/items"
printf 'key\nalpha\n' >"${work}/inv.csv"
printf 'alpha\n' >"${work}/items/a.md"

rc=0
"${DIR}/inventory-trace-check.sh" --csv "${work}/inv.csv" --items-dir "${work}/items" 2>/dev/null || rc=$?
if [ "${rc}" -ne 0 ]; then
  echo "FAIL: baseline expected exit 0, got ${rc}" >&2
  exit 1
fi

chmod 000 "${work}/items"
if [ -r "${work}/items" ] && ls "${work}/items" >/dev/null 2>&1; then
  chmod 700 "${work}/items"
  echo "error: permissions are not enforced here (running as root?); cannot test unreadable directory" >&2
  exit 2
fi
rc=0
"${DIR}/inventory-trace-check.sh" --csv "${work}/inv.csv" --items-dir "${work}/items" 2>/dev/null || rc=$?
chmod 700 "${work}/items"
if [ "${rc}" -ne 1 ]; then
  echo "FAIL: unreadable items directory expected exit 1, got ${rc}" >&2
  exit 1
fi
echo "ok: unreadable items directory exits 1"
