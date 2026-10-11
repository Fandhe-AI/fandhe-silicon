#!/usr/bin/env bash
# inventory-trace-check.sh の回帰テスト（spec TASK-16 のポインタ）。
# 一時ディレクトリの fixture（ダミーの鍵・ラベル）で、項目の行への帰属・--list の出力・
# --item-id-column・項目ディレクトリが列挙できない場合の終了コードを確かめる。
# 直接実行する（Make のターゲットは無い）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-inventory-trace-check.XXXXXX")"
trap 'chmod 700 "${work}/items" 2>/dev/null || true; rm -rf "${work:?}"' EXIT
mkdir "${work}/items"

# 終了コードと標準出力を確かめる。引数: 名前 期待コード 期待の標準出力 追加の引数...
expect() {
  local name="$1" want="$2" out_want="$3"
  shift 3
  local rc=0 out
  out="$("${DIR}/inventory-trace-check.sh" --csv "${work}/inv.csv" --items-dir "${work}/items" "$@" 2>/dev/null)" || rc=$?
  if [ "${rc}" -ne "${want}" ]; then
    echo "FAIL: ${name}: expected exit ${want}, got ${rc}" >&2
    exit 1
  fi
  if [ "${out}" != "${out_want}" ]; then
    echo "FAIL: ${name}: unexpected stdout" >&2
    exit 1
  fi
  echo "ok: ${name}"
}

printf 'key\nalpha\n' >"${work}/inv.csv"
printf '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n' >"${work}/items/a.md"
expect "key in a table row" 0 ""

printf 'prose with alpha only\n\n| id | text |\n| --- | --- |\n| ITEM-A | other |\n' >"${work}/items/a.md"
expect "key only in prose" 1 ""

printf '| id | text |\n| --- | --- |\n| ITEM-A | x \\| alpha |\n' >"${work}/items/a.md"
expect "escaped pipe stays in the cell" 0 ""

printf '| id | text |\n| --- | --- |\n| ITEM-A | alpha | extra |\n' >"${work}/items/a.md"
expect "row with a column mismatch is not an item" 1 ""

printf '| id | text |\n| --- | --- |\n| alpha | other |\n' >"${work}/items/a.md"
expect "key only in the label cell" 1 ""

# --list: 2 ファイル・複数項目。合格のときだけ TSV を出す。
printf 'key\nalpha\nbeta\n' >"${work}/inv.csv"
printf '| id | text |\n| --- | --- |\n| ITEM-B | alpha beta |\n| ITEM-A | alpha |\n' >"${work}/items/b.md"
printf '| id | text |\n| --- | --- |\n| ITEM-C | alpha |\n' >"${work}/items/a.md"
want="$(printf 'line\tkey\tfile\titem\n2\talpha\ta.md\tITEM-C\n2\talpha\tb.md\tITEM-B\n2\talpha\tb.md\tITEM-A\n3\tbeta\tb.md\tITEM-B')"
expect "--list output" 0 "${want}" --list
expect "no stdout without --list" 0 ""

printf 'key\nalpha\ngamma\n' >"${work}/inv.csv"
expect "--list prints nothing on violation" 1 "" --list

# --item-id-column
printf 'key\nalpha\n' >"${work}/inv.csv"
rm "${work}/items/b.md"
printf '| id | name | text |\n| --- | --- | --- |\n| 1 | ITEM-A | alpha |\n' >"${work}/items/a.md"
want="$(printf 'line\tkey\tfile\titem\n2\talpha\ta.md\tITEM-A')"
expect "--item-id-column selects another column" 0 "${want}" --item-id-column name --list
expect "--item-id-column that matches no table" 2 "" --item-id-column nothing

printf 'key\nalpha\n' >"${work}/inv.csv"
printf '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n' >"${work}/items/a.md"
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
