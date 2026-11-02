#!/usr/bin/env bash
# feature-key-check.sh の回帰テスト（spec TASK-1114 のポインタ。親 Issue #948 の 5/6）。
# 一時ディレクトリの fixture（合成の架空の名前）で、終了コード・標準出力・標準エラーの文言・
# 違反の件数・鍵の秘匿・入力を変更しないことを確かめる。`docs/spec` の取得は要らない。
# 実物の example の出力の形を確かめる 1 塊だけは cargo を使う（cargo が無ければその塊だけ飛ばし、標準エラーに出す）。
# 呼び出し元: `make test-feature-key-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
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

work="$(mktemp -d "${TMPDIR:-/tmp}/test-feature-key-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# fx: 架空の 3 鍵の一覧と、対象の行 3 件と対象外の行 1 件の CSV をケースごとに作り直す。
fx() {
  rm -f "${work}/keys.txt" "${work}/inv.csv"
  printf 'feature:ALPHA\nfeature:BETA\nfeature:GAMMA\n' >"${work}/keys.txt"
  printf 'key,note\nfeature:ALPHA,a\nfeature:BETA,b\nfeature:GAMMA,c\nother:ZETA,z\n' >"${work}/inv.csv"
}

# run_check <引数...>: 既定の入力を渡して実行する。後ろの引数が前を上書きする。
run_check() {
  rc=0
  "${DIR}/feature-key-check.sh" --keys "${work}/keys.txt" --csv "${work}/inv.csv" "$@" \
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

OK_LINE='[feature-key-check.sh] ok: keys=3 csv=3'

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
printf 'key,note\nfeature:ALPHA,a\nfeature:BETA,b\nfeature:GAMMA,c\nother:ZETA,z\nother:ETA,e\n' >"${work}/inv.csv"
run_check
expect_out "pass: rows outside the prefix are ignored" "${OK_LINE}"

fx
printf 'pfx:ALPHA\npfx:BETA\npfx:GAMMA\n' >"${work}/keys.txt"
printf 'key\npfx:ALPHA\npfx:BETA\npfx:GAMMA\nfeature:STRAY\n' >"${work}/inv.csv"
run_check --key-prefix pfx:
expect_out "pass: custom --key-prefix" "[feature-key-check.sh] ok: keys=3 csv=3"

# ---- (a) 一覧の形 ----

fx
printf 'feature:ALPHA\n' >>"${work}/keys.txt"
run_check
expect_violations "keys: duplicate key" 1 "duplicate key at key list line 4 (first at line 1)"

fx
printf '\n' >>"${work}/keys.txt"
run_check
expect_violations "keys: empty line" 1 "empty key at key list line 4"

fx
printf 'other:DELTA\n' >>"${work}/keys.txt"
run_check
expect_violations "keys: key outside the prefix" 2 "does not start with the prefix" "key at key list line 4 has no CSV key"

fx
printf 'feature:DELTA\textra\n' >>"${work}/keys.txt"
run_check
expect_violations "keys: line with a tab" 1 "key list line 4 has more than 1 column"

# ---- (b) 機能一覧との対応 ----

fx
printf 'key,note\nfeature:ALPHA,a\nfeature:GAMMA,c\n' >"${work}/inv.csv"
run_check
expect_violations "csv: missing row" 1 "key at key list line 2 has no CSV key"

fx
printf 'key,note\nfeature:ALPHA,a\nfeature:BETA,b\nfeature:GAMMA,c\nfeature:EXTRA,x\n' >"${work}/inv.csv"
run_check
expect_violations "csv: extra row" 1 "CSV key at line 5 is not in the key list"

fx
printf 'key,note\nfeature:ALPHA,a\nfeature:BETA,b\nfeature:GAMMA,c\nfeature:BETA,b\n' >"${work}/inv.csv"
run_check
expect_violations "csv: duplicate key" 1 "duplicate CSV key at line 5 (first at line 3)"

fx
printf 'key,note\nfeature:ALPHA,a\nfeature:BETA,b\nfeature:GAMMA\n' >"${work}/inv.csv"
run_check
expect_violations "csv: column count differs from header" 2 "column count differs from the header at CSV line 4"

# ---- 読めない・空の入力 ----

fx
: >"${work}/keys.txt"
run_check
expect_unreadable "empty key list" "the key list is empty"

fx
: >"${work}/inv.csv"
run_check
expect_unreadable "empty CSV" "the input CSV is empty"

fx
printf 'key,note\nother:ZETA,z\n' >"${work}/inv.csv"
run_check
expect_unreadable "CSV without rows of the prefix" "no rows with the key prefix"

# ---- 誤用 ----

fx
run_check --bogus
expect_usage "unknown option"

fx
run_check --keys "${work}/none.txt"
expect_usage "missing --keys file" "key list file not found:"

fx
run_check --csv "${work}/none.csv"
expect_usage "missing --csv file" "input CSV not found"

# --keys 省略で cargo が PATH に無い（python3 だけを置いた一時ディレクトリを PATH にする）。
fx
mkdir "${work}/bin"
ln -s "$(command -v python3)" "${work}/bin/python3"
ln -s "$(command -v dirname)" "${work}/bin/dirname"
rc=0
PATH="${work}/bin" "$(command -v bash)" "${DIR}/feature-key-check.sh" --csv "${work}/inv.csv" \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect_usage "no --keys and no cargo" "cargo not found"

# --example の誤用。名前の検証は cargo の検索より前なので、cargo に届かない。
fx
for bad in 'bad-name' ''; do
  rc=0
  "${DIR}/feature-key-check.sh" --csv "${work}/inv.csv" --example "${bad}" \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
  expect_usage "invalid --example name '${bad}'" "invalid --example name" \
    '!the key list example failed to run' '!cargo not found'
done

fx
run_check --example limit_keys
expect_usage "--keys and --example together" "--keys and --example cannot be used together"

# ---- 秘匿 ----

fx
printf 'feature:ALPHA\n' >>"${work}/keys.txt"
run_check
expect_violations "diagnostics hide keys by default" 1 "duplicate key at key list line 4" '!feature:ALPHA'
run_check --show-keys
expect_violations "--show-keys prints the key" 1 ": feature:ALPHA"

# ---- 入力を変えない ----

snapshot() {
  (cd "${work}" && find . -type f ! -name stdout.txt ! -name stderr.txt | sort && cksum keys.txt inv.csv)
}
fx
s1="$(snapshot)"
run_check
run_check --show-keys
printf 'feature:ALPHA\n' >>"${work}/keys.txt"
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
  real="${work}/real.txt"
  rc=0
  (cd "${DIR}/.." && cargo run -q -p fandhe-silicon-core --example feature_keys) >"${real}" 2>"${ERR_F}" || rc=$?
  : >"${OUT_F}"
  if [ "${rc}" -ne 0 ]; then
    report_fail "real example: runs" "exit ${rc}"
  else
    lines="$(wc -l <"${real}" | tr -d ' ')"
    uniq_lines="$(sort -u "${real}" | wc -l | tr -d ' ')"
    others="$(grep -vc '^feature:' "${real}" || true)"
    if [ "${lines}" = 79 ] && [ "${uniq_lines}" = 79 ] && [ "${others}" = 0 ]; then
      report_ok "real example: 79 lines, all distinct, all start with feature:"
    else
      report_fail "real example: shape" "lines=${lines} distinct=${uniq_lines} others=${others}"
    fi
    # 同じ出力から合成の CSV を作り、--keys 省略（cargo 経由）で合格することを確かめる。
    {
      printf 'key\n'
      cat "${real}"
    } >"${work}/real.csv"
    rc=0
    "${DIR}/feature-key-check.sh" --csv "${work}/real.csv" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    judge "real example: passes without --keys" 0 '[feature-key-check.sh] ok: keys=79 csv=79'
  fi
  for name in limit_keys downlevel_keys; do
    case "${name}" in
      limit_keys) pfx='limit:' want=57 ;;
      *) pfx='downlevel:' want=26 ;;
    esac
    rc=0
    (cd "${DIR}/.." && cargo run -q -p fandhe-silicon-core --example "${name}") >"${real}" 2>"${ERR_F}" || rc=$?
    : >"${OUT_F}"
    if [ "${rc}" -ne 0 ]; then
      report_fail "real ${name}: runs" "exit ${rc}"
      continue
    fi
    lines="$(wc -l <"${real}" | tr -d ' ')"
    uniq_lines="$(sort -u "${real}" | wc -l | tr -d ' ')"
    others="$(grep -vc "^${pfx}" "${real}" || true)"
    if [ "${lines}" = "${want}" ] && [ "${uniq_lines}" = "${want}" ] && [ "${others}" = 0 ]; then
      report_ok "real ${name}: ${want} lines, all distinct, all start with ${pfx}"
    else
      report_fail "real ${name}: shape" "lines=${lines} distinct=${uniq_lines} others=${others}"
    fi
    {
      printf 'key\n'
      cat "${real}"
    } >"${work}/real.csv"
    rc=0
    "${DIR}/feature-key-check.sh" --csv "${work}/real.csv" --example "${name}" --key-prefix "${pfx}" \
      >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    judge "real ${name}: passes with --example" 0 "[feature-key-check.sh] ok: keys=${want} csv=${want}"
    if [ "${name}" = limit_keys ]; then
      rc=0
      "${DIR}/feature-key-check.sh" --csv "${work}/real.csv" --example "${name}" \
        >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
      judge "real ${name}: default prefix is rejected" 1 "" \
        "does not start with the prefix" "no rows with the key prefix"
    fi
  done
else
  echo "skip: cargo not found; the real example block was not run" >&2
fi

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
