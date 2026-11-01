#!/usr/bin/env bash
# texture-format-key-check.sh の回帰テスト（spec TASK-1313 のポインタ。親 Issue #1015 の 2/4）。
# 一時ディレクトリの fixture（合成の架空の名前）で、終了コード・標準出力・標準エラーの文言・
# 違反の件数・鍵の秘匿・入力を変更しないことを確かめる。`docs/spec` の取得は要らない。
# 実物の example の出力の形を確かめる 1 塊だけは cargo を使う（cargo が無ければその塊だけ飛ばし、標準エラーに出す）。
# 呼び出し元: `make test-texture-format-key-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致 + FAILED の件数」で行う。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として報告する。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

work="$(mktemp -d "${TMPDIR:-/tmp}/test-texture-format-key-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
TAB="$(printf '\t')"
passed=0
failed=0
rc=0

# fx: 架空の 3 行（鍵 1 件）と 1 行（鍵 3 件）の表・CSV・列挙の JSON をケースごとに作り直す。
fx() {
  rm -f "${work}/table.tsv" "${work}/inv.csv" "${work}/enum.json"
  {
    printf 'Alpha\ttexformat:Alpha\tAlpha\n'
    printf 'Beta\ttexformat:Beta\tBeta\n'
    printf 'Gamma\ttexformat:Gamma\tGamma\n'
    printf 'Multi\ttexformat:Multi.X\tMulti { v: X }\n'
    printf 'Multi\ttexformat:Multi.Y\tMulti { v: Y }\n'
    printf 'Multi\ttexformat:Multi.Z\tMulti { v: Z }\n'
  } >"${work}/table.tsv"
  printf 'key,note\ntexformat:Alpha,a\ntexformat:Beta,b\ntexformat:Gamma,c\ntexformat:Multi,m\nother:Zeta,z\n' >"${work}/inv.csv"
  printf '{"items":[{"name":"Alpha"},{"name":"Beta"},{"name":"Gamma"},{"name":"Multi { v: X }"},{"name":"Multi { v: Y }"},{"name":"Multi { v: Z }"}]}\n' >"${work}/enum.json"
}

# run_check <引数...>: 既定の入力を渡して実行する。後ろの引数が前を上書きする。
run_check() {
  rc=0
  "${DIR}/texture-format-key-check.sh" --table "${work}/table.tsv" --csv "${work}/inv.csv" \
    --enumerate-json "${work}/enum.json" --enumerate-list items --enumerate-item name "$@" \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
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

# judge <名前> <期待コード> <期待の標準出力> [部分一致...]: 先頭が ! の語は標準エラーに含まれないこと。
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
        if grep -qF -e "${s#!}" "${ERR_F}"; then
          report_fail "${name}" "stderr must not contain: ${s#!}"
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

OK_LINE='[texture-format-key-check.sh] ok: table=6 rows=4 multi=Multi:3 csv=4 enumerate=6'

expect_out() {
  local name="$1" out_want="$2"
  shift 2
  judge "${name}" 0 "${out_want}" "$@"
}
expect_violations() {
  local name="$1" n="$2"
  shift 2
  judge "${name}" 1 "" "FAILED: ${n} violation(s)" "$@"
}
expect_unreadable() {
  local name="$1"
  shift
  judge "${name}" 1 "" '!FAILED:' "$@"
}
expect_usage() {
  local name="$1"
  shift
  judge "${name}" 2 "" '!FAILED:' "$@"
}

# ---- 合格 ----

fx
run_check
expect_out "pass: base set" "${OK_LINE}" '!FAILED:'

fx
printf 'key,note\ntexformat:Alpha,a\ntexformat:Beta,b\ntexformat:Gamma,c\ntexformat:Multi,m\nother:Zeta,z\nother:Eta,e\n' >"${work}/inv.csv"
run_check
expect_out "pass: rows outside the prefix are ignored" "${OK_LINE}"

fx
printf 'key\npfx:Alpha\npfx:Beta\npfx:Gamma\npfx:Multi\ntexformat:Stray\n' >"${work}/inv.csv"
run_check --key-prefix pfx:
expect_out "pass: custom --key-prefix" "${OK_LINE}"

# ---- (a) 表の形 ----

fx
printf 'Delta\ttexformat:Delta\n' >>"${work}/table.tsv"
run_check
expect_violations "table: row with 2 columns" 1 "table line 7 does not have 3 columns"

fx
printf 'Delta\ttexformat:Alpha\tDelta\n' >>"${work}/table.tsv"
run_check
expect_violations "table: duplicate key" 2 "duplicate key at table line 7"

fx
printf 'Delta\ttexformat:Delta\tAlpha\n' >>"${work}/table.tsv"
run_check
expect_violations "table: duplicate format" 1 "duplicate format at table line 7"

fx
printf 'Delta\t\tDelta\n' >>"${work}/table.tsv"
run_check
expect_violations "table: empty key" 2 "empty key at table line 7"

# ---- (b) 機能一覧との対応 ----

fx
printf 'key,note\ntexformat:Alpha,a\ntexformat:Gamma,c\ntexformat:Multi,m\n' >"${work}/inv.csv"
run_check
expect_violations "csv: missing row" 1 "table row at line 2 has no CSV key"

fx
printf 'key,note\ntexformat:Alpha,a\ntexformat:Beta,b\ntexformat:Gamma,c\ntexformat:Multi,m\ntexformat:Extra,x\n' >"${work}/inv.csv"
run_check
expect_violations "csv: extra row" 1 "CSV key at line 6 has no table row"

fx
printf 'key,note\ntexformat:Alpha,a\ntexformat:Beta,b\ntexformat:Gamma,c\ntexformat:Multi,m\ntexformat:Beta,b\n' >"${work}/inv.csv"
run_check
expect_violations "csv: duplicate key" 1 "duplicate CSV key at line 6 (first at line 3)"

fx
printf 'key,note\ntexformat:Alpha,a\ntexformat:Beta,b\ntexformat:Gamma,c\n' >"${work}/inv.csv"
run_check
expect_violations "csv: missing row for the multi-key row" 1 "table row at line 4 has no CSV key"

fx
printf 'key,note\ntexformat:Alpha,a\ntexformat:Beta,b\ntexformat:Gamma,c\ntexformat:Multi\n' >"${work}/inv.csv"
run_check
expect_violations "csv: column count differs from header" 2 "column count differs from the header at CSV line 5"

# ---- (c) 実行時列挙との対応 ----

fx
printf '{"items":[{"name":"Alpha"},{"name":"Beta"},{"name":"Gamma"},{"name":"Multi { v: X }"},{"name":"Multi { v: Y }"}]}\n' >"${work}/enum.json"
run_check
expect_violations "enum: missing spelling" 1 "table format at line 6 is not enumerated"

fx
printf '{"items":[{"name":"Alpha"},{"name":"Beta"},{"name":"Gamma"},{"name":"Multi { v: X }"},{"name":"Multi { v: Y }"},{"name":"Multi { v: Z }"},{"name":"Extra"}]}\n' >"${work}/enum.json"
run_check
expect_violations "enum: extra spelling" 1 "enumeration item 6 has no table format"

fx
printf '{"items":[{"name":"Alpha"},{"name":"Beta"},{"name":"Gamma"},{"name":"Multi { v: X }"},{"name":"Multi { v: Y }"},{"name":"Multi { v: Z }"},{"name":"Beta"}]}\n' >"${work}/enum.json"
run_check
expect_violations "enum: duplicate spelling" 1 "duplicate enumeration spelling at item 6 (first at item 1)"

# ---- 読めない・空の入力 ----

fx
: >"${work}/table.tsv"
run_check
expect_unreadable "empty table" "the table is empty"

fx
: >"${work}/inv.csv"
run_check
expect_unreadable "empty CSV" "the input CSV is empty"

fx
printf 'key,note\nother:Zeta,z\n' >"${work}/inv.csv"
run_check
expect_unreadable "CSV without rows of the prefix" "no rows with the key prefix"

fx
printf '{"items":[]}\n' >"${work}/enum.json"
run_check
expect_unreadable "empty enumeration" "the enumeration is empty"

fx
printf '{"items":[' >"${work}/enum.json"
run_check
expect_unreadable "broken JSON" "cannot parse the enumeration JSON"

fx
printf '{"items":{"name":"Alpha"}}\n' >"${work}/enum.json"
run_check
expect_unreadable "enumeration is not a list" "no list under the given list key"

fx
printf '{"items":[{"other":"Alpha"}]}\n' >"${work}/enum.json"
run_check
expect_unreadable "enumeration item without the spelling" "enumeration item 0 has no string"

# ---- 誤用 ----

fx
run_check --bogus
expect_usage "unknown option"

fx
run_check --table "${work}/none.tsv"
expect_usage "missing --table file" "table file not found:"

fx
run_check --csv "${work}/none.csv"
expect_usage "missing --csv file" "input CSV not found:"

fx
run_check --enumerate-json "${work}/none.json"
expect_usage "missing JSON file" "enumeration JSON not found:"

for missing in json list item; do
  fx
  rc=0
  case "${missing}" in
    json)
      "${DIR}/texture-format-key-check.sh" --table "${work}/table.tsv" --csv "${work}/inv.csv" \
        --enumerate-list items --enumerate-item name >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
      ;;
    list)
      "${DIR}/texture-format-key-check.sh" --table "${work}/table.tsv" --csv "${work}/inv.csv" \
        --enumerate-json "${work}/enum.json" --enumerate-item name >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
      ;;
    item)
      "${DIR}/texture-format-key-check.sh" --table "${work}/table.tsv" --csv "${work}/inv.csv" \
        --enumerate-json "${work}/enum.json" --enumerate-list items >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
      ;;
  esac
  expect_usage "missing enumerate argument: ${missing}" "are all required"
done

# --table 省略で cargo が PATH に無い（python3 だけを置いた一時ディレクトリを PATH にする）。
fx
mkdir "${work}/bin"
ln -s "$(command -v python3)" "${work}/bin/python3"
ln -s "$(command -v dirname)" "${work}/bin/dirname"
rc=0
PATH="${work}/bin" "$(command -v bash)" "${DIR}/texture-format-key-check.sh" --csv "${work}/inv.csv" \
  --enumerate-json "${work}/enum.json" --enumerate-list items --enumerate-item name \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect_usage "no --table and no cargo" "cargo not found"

# ---- 秘匿 ----

fx
printf 'Delta\ttexformat:Alpha\tDelta\n' >>"${work}/table.tsv"
run_check
expect_violations "diagnostics hide keys by default" 2 "duplicate key at table line 7" '!texformat:Alpha'
run_check --show-keys
expect_violations "--show-keys prints the key" 2 ": texformat:Alpha"

# ---- 入力を変えない ----

snapshot() {
  (cd "${work}" && find . -type f ! -name stdout.txt ! -name stderr.txt | sort && cksum table.tsv inv.csv enum.json)
}
fx
s1="$(snapshot)"
run_check
run_check --show-keys
printf 'Delta\ttexformat:Alpha\tDelta\n' >>"${work}/table.tsv"
s2="$(snapshot)"
run_check --show-keys
s3="$(snapshot)"
fx
if [ "${s1}" = "$(snapshot)" ] && [ "${s2}" = "${s3}" ]; then
  report_ok "inputs are not modified and no files are created"
else
  report_fail "inputs are not modified and no files are created" "file list or checksums changed"
fi

# ---- 実物の example の出力 ----

if command -v cargo >/dev/null 2>&1; then
  real="${work}/real.tsv"
  rc=0
  (cd "${DIR}/.." && cargo run -q -p fandhe-silicon-core --example texture_format_keys) >"${real}" 2>"${ERR_F}" || rc=$?
  : >"${OUT_F}"
  if [ "${rc}" -ne 0 ]; then
    report_fail "real example: runs" "exit ${rc}"
  else
    lines="$(wc -l <"${real}" | tr -d ' ')"
    badcols="$(awk -F "${TAB}" 'NF != 3' "${real}" | wc -l | tr -d ' ')"
    rows="$(cut -f1 "${real}" | sort -u | wc -l | tr -d ' ')"
    astc="$(awk -F "${TAB}" '$1 == "Astc"' "${real}" | wc -l | tr -d ' ')"
    if [ "${lines}" = 118 ] && [ "${badcols}" = 0 ] && [ "${rows}" = 77 ] && [ "${astc}" = 42 ]; then
      report_ok "real example: 118 lines, 3 columns, 77 rows, 42 for Astc"
    else
      report_fail "real example: shape" "lines=${lines} badcols=${badcols} rows=${rows} astc=${astc}"
    fi
    # 同じ出力から合成の CSV・JSON を作り、--table 省略（cargo 経由）で合格することを確かめる。
    {
      printf 'key\n'
      cut -f1 "${real}" | awk '!s[$0]++ { print "texformat:" $0 }'
    } >"${work}/real.csv"
    {
      printf '{"items":['
      cut -f3 "${real}" | awk 'BEGIN { first = 1 } { gsub(/"/, "\\\""); printf "%s{\"name\":\"%s\"}", (first ? "" : ","), $0; first = 0 }'
      printf ']}\n'
    } >"${work}/real.json"
    rc=0
    "${DIR}/texture-format-key-check.sh" --csv "${work}/real.csv" --enumerate-json "${work}/real.json" \
      --enumerate-list items --enumerate-item name >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    judge "real example: passes without --table" 0 \
      '[texture-format-key-check.sh] ok: table=118 rows=77 multi=Astc:42 csv=77 enumerate=118'
  fi
else
  echo "skip: cargo not found; the real example block was not run" >&2
fi

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
