#!/usr/bin/env bash
# inventory-trace-check.sh の回帰テスト（spec TASK-16 のポインタ。親 Issue #60 の 4/4）。
# 追随分の CSV（--follow-csv）の節は spec TASK-17 のポインタ（親 Issue #65 の 2/5）。
# 一時ディレクトリの fixture（合成のダミー値）で、終了コード・標準出力・標準エラーの文言・
# 違反の件数・鍵の秘匿・入力を変更しないことを確かめる。実物の一覧・項目は使わず、
# `docs/spec` の取得も要らない。
# 呼び出し元: `make test-inventory-trace-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致 + FAILED の件数」で行う。終了コードだけでは
# 別の理由で落ちても気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として報告する。
# fixture は合成の値だけで、spec 由来の語・形を書かない。途中で止めず、最後に件数を出す。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

work="$(mktemp -d "${TMPDIR:-/tmp}/test-inventory-trace-check.XXXXXX")"
trap 'chmod 700 "${work}/items" 2>/dev/null || true; rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# ---- 土台 ----

# fx: ケースごとに fixture を作り直して順序依存を無くす（鍵 alpha を持つ一覧と、それを含む項目ファイル）。
fx() {
  chmod 700 "${work}/items" 2>/dev/null || true
  rm -rf "${work:?}/items" "${work:?}/inv.csv" "${work:?}/follow.csv"
  mkdir "${work}/items"
  printf 'key\nalpha\n' >"${work}/inv.csv"
  printf 'alpha\n' >"${work}/items/a.md"
}

# item <name> <printf の書式> [引数...]: 項目ファイルを作る。
item() {
  local name="$1"
  shift
  # shellcheck disable=SC2059 # 書式はテスト側の固定文字列
  printf "$@" >"${work}/items/${name}"
}

# run_check <引数...>: 検査を実行し rc・標準出力・標準エラーを控える。後ろの引数が前を上書きする。
run_check() {
  rc=0
  "${DIR}/inventory-trace-check.sh" --csv "${work}/inv.csv" --items-dir "${work}/items" "$@" \
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

# expect_ok <名前> [部分一致...]: 0・標準出力は空。
expect_ok() {
  local name="$1"
  shift
  judge "${name}" 0 "" "$@"
}

# expect_out <名前> <期待の標準出力> [部分一致...]: 0・標準出力が一致。
expect_out() {
  local name="$1" out_want="$2"
  shift 2
  judge "${name}" 0 "${out_want}" "$@"
}

# expect_violations <名前> <件数> [部分一致...]: 1・標準出力は空・FAILED の件数が一致。
expect_violations() {
  local name="$1" n="$2"
  shift 2
  judge "${name}" 1 "" "FAILED: ${n} violation(s)" "$@"
}

# expect_unreadable <名前> [部分一致...]: 1（読み取りの失敗）・標準出力は空。FAILED の集計行は出さない経路。
expect_unreadable() {
  local name="$1"
  shift
  judge "${name}" 1 "" '!FAILED:' "$@"
}

# expect_usage <名前> [部分一致...]: 2・標準出力は空・FAILED は出さない（検査を終えていない）。
expect_usage() {
  local name="$1"
  shift
  judge "${name}" 2 "" '!FAILED:' "$@"
}

# ---- 合格・既定（ファイル単位） ----

fx
run_check
expect_ok "default: key in an item file" "ok: every row key was found in at least one item file"

fx
item a.md 'prose with alpha only\n'
run_check
expect_ok "default: key in prose passes"

fx
item a.md '| id | text |\n| --- | --- |\n| alpha | other |\n'
run_check
expect_ok "default: key in label cell passes"

fx
item a.md 'other\n'
run_check
expect_violations "default: absent key fails" 1 "no item file contains the key at line 2"

fx
run_check --item-id-column id
expect_usage "--item-id-column without --item-rows is misuse" "--item-id-column requires --item-rows"

# ---- 誤用（2） ----

fx
run_check --bogus
expect_usage "unknown option"

fx
run_check --ite "${work}/items"
expect_usage "abbreviated option is rejected"

fx
run_check extra-positional
expect_usage "extra positional argument"

fx
run_check --csv "${work}/none.csv"
expect_usage "missing --csv path" "input CSV not found:"

fx
run_check --items-dir "${work}/none"
expect_usage "missing --items-dir path" "items directory not found:"

fx
: >"${work}/inv.csv"
run_check
expect_usage "empty CSV file" "input CSV is empty"

fx
printf 'a,b\nalpha,1\n' >"${work}/inv.csv"
run_check --key-column nothing
expect_usage "--key-column matches no header" "--key-column must match exactly one header name"

fx
printf 'a,a\nalpha,1\n' >"${work}/inv.csv"
run_check --key-column a
expect_usage "--key-column matches two headers" "--key-column must match exactly one header name"

fx
run_check --exclude missing.md
expect_usage "--exclude names a missing file" "--exclude names a file that does not exist"

fx
mkdir "${work}/items/sub"
printf 'x\n' >"${work}/items/sub/b.md"
run_check --exclude sub/b.md
expect_usage "--exclude with a path separator" "--exclude names a file that does not exist"

fx
item a.md '| id | id | text |\n| --- | --- | --- |\n| 1 | 2 | alpha |\n'
run_check --item-rows --item-id-column id
expect_usage "--item-id-column matches two headers in a table" "--item-id-column matches more than one header"

fx
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n'
run_check --item-rows --item-id-column nothing
expect_usage "--item-id-column matches no table" "--item-id-column matches no table header"

# python3 が PATH に無い（dirname だけを置いた PATH で bash を絶対パスから起動する）
fx
nobin="${work}/nobin"
mkdir "${nobin}"
ln -s "$(command -v dirname)" "${nobin}/dirname"
bash_bin="$(command -v bash)"
rc=0
PATH="${nobin}" "${bash_bin}" "${DIR}/inventory-trace-check.sh" \
  --csv "${work}/inv.csv" --items-dir "${work}/items" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect_usage "python3 missing from PATH" "required command not found on PATH: python3"

# ---- 違反（1） ----

fx
printf 'key\n' >"${work}/inv.csv"
run_check
expect_violations "header-only CSV" 1 "the input CSV has no data rows"

fx
printf 'key,v\nalpha,1\nbeta\n,2\nalpha,3\n' >"${work}/inv.csv"
run_check
expect_violations "column mismatch, empty key and duplicate key" 3 \
  "column count differs from the header at line 3" \
  "empty key at line 4" \
  "duplicate key at line 5 (first seen at line 2)"

fx
printf 'key\n"abc\n' >"${work}/inv.csv"
run_check
expect_unreadable "unterminated quote in the CSV" "cannot parse the input CSV"

fx
printf 'key\n\377\n' >"${work}/inv.csv"
run_check
expect_unreadable "invalid UTF-8 in the CSV" "cannot parse the input CSV"

fx
printf '\377\n' >"${work}/items/a.md"
run_check
expect_unreadable "invalid UTF-8 in an item file" "cannot read an item file"

fx
rm "${work}/items/a.md"
printf 'alpha\n' >"${work}/items/README.md"
run_check
expect_violations "only README.md to search" 2 "no item files (*.md) to search"

fx
item a.md 'other\n'
printf 'alpha\n' >"${work}/items/README.md"
printf 'alpha\n' >"${work}/items/b.txt"
mkdir "${work}/items/sub"
printf 'alpha\n' >"${work}/items/sub/c.md"
run_check
expect_violations "key only in README, non-md and subdirectory files" 1 \
  "no item file contains the key at line 2"

fx
item a.md 'other\n'
item b.md 'alpha\n'
run_check --exclude b.md
expect_violations "key only in an excluded file" 1 "no item file contains the key at line 2"
run_check
expect_ok "same key passes when the file is not excluded"

fx
printf 'key\nalpha\nbeta\ngamma\n' >"${work}/inv.csv"
item a.md 'alpha gamma\n'
run_check
expect_violations "one row of several is absent" 1 \
  "no item file contains the key at line 3" \
  "!at line 2" "!at line 4"

# ---- 境界（直前・直後の文字） ----

fx
printf 'key\nabc\nabcd\n' >"${work}/inv.csv"
item a.md 'abcd\n'
run_check
expect_violations "key only as a prefix of another key" 1 "no item file contains the key at line 2"

fx
printf 'key\nabc\nxabc\n' >"${work}/inv.csv"
item a.md 'xabc\n'
run_check
expect_violations "key only as a suffix of another key" 1 "no item file contains the key at line 2"

fx
item a.md 'see (alpha), then\n'
run_check
expect_ok "punctuation outside the key alphabet is a boundary"

fx
printf 'alpha' >"${work}/items/a.md"
run_check
expect_ok "key at the very start and end of a file"

# ---- CSV の読み方 ----

fx
printf 'key,note\nalpha,"a,b\nc"\nbeta,x\n' >"${work}/inv.csv"
item a.md 'alpha beta\n'
run_check --list
want="$(printf 'line\tkey\tfile\titem\n3\talpha\ta.md\t\n4\tbeta\ta.md\t')"
expect_out "quoted multi-line cell: --list line is the record's last line" "${want}"

fx
printf '\357\273\277key,other\r\nalpha,beta\r\n' >"${work}/inv.csv"
item a.md 'alpha beta\n'
run_check --key-column key
expect_ok "BOM and CRLF with --key-column by name"
run_check --key-column other
expect_ok "BOM and CRLF with a non-first key column"

fx
printf 'key\n"a\tb"\n' >"${work}/inv.csv"
printf 'a\tb\n' >"${work}/items/a.md"
run_check --list
want="$(printf 'line\tkey\tfile\titem\n2\t"a\tb"\ta.md\t')"
expect_out "--list quotes a key containing a tab" "${want}"

# ---- --item-rows（項目の行まで絞る） ----

fx
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n'
run_check --item-rows
expect_ok "item rows: key in a table row"

fx
item a.md 'prose with alpha only\n\n| id | text |\n| --- | --- |\n| ITEM-A | other |\n'
run_check --item-rows
expect_violations "item rows: key only in prose" 1 "no item row contains the key at line 2"

fx
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | x \\| alpha |\n'
run_check --item-rows
expect_ok "item rows: escaped pipe stays in the cell"

fx
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | alpha | extra |\n'
run_check --item-rows
expect_violations "item rows: column mismatch row is not an item" 2 \
  "note: 1 table row(s) were not treated as items" "error: no item rows found" \
  "!error: 1 table row(s)"

# 読み飛ばしの注記は違反ではない（項目の行があり対応が取れていれば 0・接頭辞は note:）。
fx
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n| X | y | extra |\n|  | z |\n'
run_check --item-rows
expect_ok "item rows: skipped-row note is not an error" \
  "note: 2 table row(s) were not treated as items" "!error:"

fx
item a.md '| id | text |\n| --- | --- |\n| alpha | other |\n'
run_check --item-rows
expect_violations "item rows: key only in the label cell" 1 "no item row contains the key at line 2"

fx
item a.md '| id | text |\n| :--- | ---: |\n| ITEM-A | alpha |\n'
run_check --item-rows
expect_ok "item rows: aligned separator row"

fx
item a.md '| id | text |\n| --- |\n| ITEM-A | alpha |\n'
run_check --item-rows
expect_violations "item rows: header and separator widths differ" 2 \
  "no item rows found" "no item row contains the key at line 2"

fx
item a.md '| id | text |\n| --- | --- |\n|  | alpha |\n| X | alpha | extra |\n'
run_check --item-rows
expect_violations "item rows: only empty-label and mismatched rows" 2 \
  "note: 2 table row(s) were not treated as items" "error: no item rows found"

bt='```'
fx
item a.md '%s\n| id | text |\n| --- | --- |\n| EXAMPLE | alpha |\n%s\n' "${bt}markdown" "${bt}"
run_check --item-rows
expect_violations "item rows: key only in a backtick fence" 2 "no item rows found"

fx
item a.md '~~~\n| id | text |\n| --- | --- |\n| EXAMPLE | alpha |\n~~~\n'
run_check --item-rows
expect_violations "item rows: key only in a tilde fence" 2 "no item rows found"

fx
bt4="${bt}\`"
item a.md '%s\n%s\n| id | text |\n| --- | --- |\n| EXAMPLE | alpha |\n%s\n' "${bt4}" "${bt}" "${bt4}"
run_check --item-rows
expect_violations "item rows: shorter inner fence does not close" 2 "no item rows found"

fx
item a.md '%s\nexample\n%s\n\n| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n' "${bt}" "${bt}"
run_check --item-rows --list
want="$(printf 'line\tkey\tfile\titem\n2\talpha\ta.md\tITEM-A')"
expect_out "item rows: table after a closed fence still counts" "${want}"

fx
item a.md '| id | name | text |\n| --- | --- | --- |\n| 1 | ITEM-A | alpha |\n'
run_check --item-id-column name --item-rows --list
want="$(printf 'line\tkey\tfile\titem\n2\talpha\ta.md\tITEM-A')"
expect_out "item rows: --item-id-column selects another column" "${want}"

# ---- --list ----

fx
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n'
run_check --list
want="$(printf 'line\tkey\tfile\titem\n2\talpha\ta.md\t')"
expect_out "default: --list has an empty item column" "${want}"

fx
printf 'key\nalpha\nbeta\n' >"${work}/inv.csv"
item b.md '| id | text |\n| --- | --- |\n| ITEM-B | alpha beta |\n| ITEM-A | alpha |\n'
item a.md '| id | text |\n| --- | --- |\n| ITEM-C | alpha |\n'
run_check --item-rows --list
want="$(printf 'line\tkey\tfile\titem\n2\talpha\ta.md\tITEM-C\n2\talpha\tb.md\tITEM-B\n2\talpha\tb.md\tITEM-A\n3\tbeta\tb.md\tITEM-B')"
expect_out "--list order: CSV row, file name, line in file" "${want}"
run_check --item-rows
expect_ok "no stdout without --list"

fx
printf 'key\nalpha\ngamma\n' >"${work}/inv.csv"
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n'
run_check --list --item-rows
expect_violations "--list prints nothing on violation" 1 "no item row contains the key at line 3"
fx
printf 'key\nalpha\ngamma\n' >"${work}/inv.csv"
run_check --list
expect_violations "--list prints nothing on violation (file unit)" 1 "no item file contains the key at line 3"

fx
run_check --list --bogus
expect_usage "--list prints nothing on misuse"

# ---- 診断での鍵の秘匿 ----

fx
printf 'key\nzzkey\n' >"${work}/inv.csv"
run_check
expect_violations "keys are hidden by default" 1 \
  "no item file contains the key at line 2" "!zzkey"

fx
printf 'key\nzzkey\n' >"${work}/inv.csv"
run_check --show-keys
expect_violations "--show-keys prints the key" 1 \
  "no item file contains the key at line 2: zzkey"

fx
printf 'key\nzzkey\n' >"${work}/inv.csv"
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | other |\n'
run_check --item-rows
expect_violations "item rows: keys are hidden by default" 1 \
  "no item row contains the key at line 2" "!zzkey"

# ---- 追随分（--follow-csv） ----
# 追随分のファイルは節の中で ${work}/follow.csv に作る（fx が消す）。値は合成のダミーだけ。

# fol <printf の書式> [引数...]: 追随分の CSV を作る。
fol() {
  # shellcheck disable=SC2059 # 書式はテスト側の固定文字列
  printf "$@" >"${work}/follow.csv"
}
FOL="--follow-csv ${work}/follow.csv"

fx
fol 'key\nbeta\n'
item b.md 'beta\n'
# shellcheck disable=SC2086 # FOL は固定の 2 語
run_check ${FOL}
expect_ok "follow-up: key only in the follow-up CSV passes" "ok: every row key was found"

fx
fol 'key\nbeta\n'
item a.md '| id | text |\n| --- | --- |\n| ITEM-A | alpha |\n| ITEM-B | beta |\n'
# shellcheck disable=SC2086
run_check ${FOL} --item-rows
expect_ok "follow-up: --item-rows key in a table row"

fx
printf '\357\273\277key,other\r\nalpha,beta\r\n' >"${work}/inv.csv"
printf '\357\273\277key,other\r\ngamma,delta\r\n' >"${work}/follow.csv"
item a.md 'alpha gamma\n'
# shellcheck disable=SC2086
run_check ${FOL} --key-column other
expect_violations "follow-up: --key-column by name with BOM and CRLF" 2 \
  "no item file contains the key at line 2" "no item file contains the key at follow-up line 2"
item a.md 'beta delta\n'
# shellcheck disable=SC2086
run_check ${FOL} --key-column other
expect_ok "follow-up: --key-column by name passes with BOM and CRLF"

fx
fol 'key\nzzkey\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: absent follow-up key fails" 1 \
  "no item file contains the key at follow-up line 2" "!at line 2" "!zzkey"

fx
fol 'key\nalpha\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: key in both CSVs is a duplicate" 1 \
  "duplicate key at follow-up line 2 (first seen at line 2)"

fx
fol 'key\nbeta\nbeta\n'
item b.md 'beta\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: duplicate inside the follow-up CSV" 1 \
  "duplicate key at follow-up line 3 (first seen at follow-up line 2)"

fx
fol 'id\nalpha\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: header name differs from the base" 1 \
  "follow-up CSV header differs from the base CSV header"

fx
fol 'key,extra\nalpha2,x\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: header column count differs from the base" 1 \
  "follow-up CSV header differs from the base CSV header" "!follow-up line"

fx
fol 'key\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: header-only CSV" 1 "the follow-up CSV has no data rows"

fx
printf 'key,other\nalpha,x\n' >"${work}/inv.csv"
fol 'key,other\nbeta,y,z\n,w\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: column mismatch and empty key are labelled" 2 \
  "column count differs from the header at follow-up line 2" "empty key at follow-up line 3"

fx
printf 'key\nabc\n' >"${work}/inv.csv"
fol 'key\nabcd\n'
item a.md 'abcd\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: boundary uses the keys of both CSVs" 1 \
  "no item file contains the key at line 2" "!follow-up line"

fx
printf '\377\n' >"${work}/follow.csv"
# shellcheck disable=SC2086
run_check ${FOL}
expect_unreadable "follow-up: invalid UTF-8 in the follow-up CSV" "cannot parse the follow-up CSV"

fx
fol 'key\n"alpha\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_unreadable "follow-up: unterminated quote in the follow-up CSV" "cannot parse the follow-up CSV"

fx
run_check --follow-csv "${work}/nope.csv"
expect_usage "follow-up: missing --follow-csv path" "follow-up CSV not found"

fx
: >"${work}/follow.csv"
# shellcheck disable=SC2086
run_check ${FOL}
expect_usage "follow-up: empty follow-up CSV file" "follow-up CSV is empty"

fx
fol 'key\nalpha\n'
# shellcheck disable=SC2086
run_check ${FOL} ${FOL}
expect_usage "follow-up: --follow-csv given twice" "--follow-csv must be given at most once"

fx
fol 'key\nbeta\n'
item b.md 'beta\n'
# shellcheck disable=SC2086
run_check ${FOL} --list
want="$(printf 'csv\tline\tkey\tfile\titem\nbase\t2\talpha\ta.md\t\nfollow-up\t2\tbeta\tb.md\t')"
expect_out "follow-up: --list adds the csv column" "${want}"

fx
fol 'key\nzzkey\n'
# shellcheck disable=SC2086
run_check ${FOL} --list
expect_violations "follow-up: --list prints nothing on violation" 1

fx
fol 'key\nzzkey\n'
# shellcheck disable=SC2086
run_check ${FOL}
expect_violations "follow-up: keys are hidden by default" 1 "!zzkey"
# shellcheck disable=SC2086
run_check ${FOL} --show-keys
expect_violations "follow-up: --show-keys prints the key" 1 \
  "no item file contains the key at follow-up line 2: zzkey"

fx
run_check
expect_ok "follow-up: without the option the output is unchanged" \
  "[inventory-trace-check.sh] ok: every row key was found in at least one item file"

# ---- 入力を変更しない ----

snapshot() {
  (cd "${work}" && find . -path ./stdout.txt -prune -o -path ./stderr.txt -prune -o -type f -exec cksum {} + | sort)
}
fx
printf 'key\nalpha\ngamma\n' >"${work}/inv.csv"
item b.md '| id | text |\n| --- | --- |\n| ITEM-B | alpha |\n'
before="$(snapshot)"
run_check
run_check --item-rows --list
fx_after_violation="$(snapshot)"
printf 'key\nalpha\n' >"${work}/inv.csv"
before_ok="$(snapshot)"
run_check --list
run_check --item-rows --list
after_ok="$(snapshot)"
if [ "${before}" = "${fx_after_violation}" ] && [ "${before_ok}" = "${after_ok}" ]; then
  report_ok "inputs are not modified and no files are created"
else
  report_fail "inputs are not modified and no files are created" "file list or checksums changed"
fi

# 追随分を渡した実行（合格・違反）でも入力を変更せず、ファイルを作らない。
printf 'key\nbeta\n' >"${work}/follow.csv"
item c.md 'beta\n'
before_follow="$(snapshot)"
run_check --follow-csv "${work}/follow.csv" --list
printf 'key\nzzkey\n' >"${work}/follow.csv"
before_follow_bad="$(snapshot)"
run_check --follow-csv "${work}/follow.csv" --item-rows --list
after_follow_bad="$(snapshot)"
printf 'key\nbeta\n' >"${work}/follow.csv"
after_follow="$(snapshot)"
if [ "${before_follow_bad}" = "${after_follow_bad}" ] && [ "${before_follow}" = "${after_follow}" ]; then
  report_ok "follow-up: inputs are not modified and no files are created"
else
  report_fail "follow-up: inputs are not modified and no files are created" "file list or checksums changed"
fi

# ---- 読めない項目ディレクトリ ----

fx
chmod 000 "${work}/items"
if [ -r "${work}/items" ] && ls "${work}/items" >/dev/null 2>&1; then
  chmod 700 "${work}/items"
  echo "error: permissions are not enforced here (running as root?); cannot test unreadable directory" >&2
  exit 2
fi
run_check
chmod 700 "${work}/items"
expect_unreadable "unreadable items directory exits 1" "cannot list the items directory"

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
