#!/usr/bin/env bash
# ci-frame-check.sh の回帰テスト（spec TASK-112 のポインタ。親 Issue #393 の 3/4）。
# 一時ディレクトリの合成 workflow（ダミーの値）と、実物の ci.yml の写しで、終了コード・標準出力・
# 標準エラーの文言・違反の件数・入力を変更しないことを確かめる。リポジトリのファイルは変更しない。
# 呼び出し元: `make test-ci-frame-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致 + FAILED の件数」で行う。終了コードだけでは
# 別の理由で落ちても気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として直す。
# 途中で止めず、最後に件数を出す。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
# shellcheck disable=SC2016 # sed 式の ${{ }} は GitHub Actions の式の綴りで、シェルに展開させない
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

CHECK="${DIR}/ci-frame-check.sh"
REAL="${ROOT_DIR}/.github/workflows/ci.yml"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-ci-frame-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
BASE="${work}/base.yml"
IN_F="${work}/in.yml"
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成の基準: 通常のジョブ a・matrix のジョブ m・条件つきのジョブ c・集約ジョブ ci-complete。
cat >"${BASE}" <<'YML'
name: T
on: push
jobs:
  a:
    name: a
    runs-on: ubuntu-latest
    steps:
      - run: echo a
  m:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - run: echo m
  c:
    if: github.event_name == 'pull_request'
    runs-on: ubuntu-latest
    steps:
      - run: echo c
  ci-complete:
    if: always()
    runs-on: ubuntu-latest
    needs:
      - a
      - m
      - c
    steps:
      - run: echo done
YML

# ---- 土台 ----

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

# run_check <引数...>: 検査を実行し rc・標準出力・標準エラーを控える。
run_check() {
  rc=0
  "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
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

# ins <入力> <出力> <行に含まれる語> <挿入する行>: 語を含む最初の行の直後に 1 行足す。
ins() {
  awk -v pat="$3" -v txt="$4" '{ print } !done && index($0, pat) { print txt; done = 1 }' "$1" >"$2"
}

# sub <入力> <出力> <sed 式>: 基準を 1 か所だけ変える。
sub() {
  sed -e "$3" "$1" >"$2"
}

# pass <名前> [入力]: 合格（0・ok の 1 行）。入力を省くと ${IN_F}。
pass() {
  run_check --workflow "${2:-${IN_F}}"
  judge "$1" 0 "ok: ci workflow frame (4 jobs, rules F1-F6)" "!violation"
}

# bad <名前> <件数> [部分一致...]: ${IN_F} が違反（1・件数一致）になること。
bad() {
  local name="$1" n="$2"
  shift 2
  run_check --workflow "${IN_F}"
  judge "${name}" 1 "" "FAILED: ${n} violation(s)" "$@"
}

misuse() {
  local name="$1"
  shift
  run_check "$@"
  judge "${name}" 2 ""
}

# ---- 合格 ----

run_check
judge "pass: real ci.yml (no arguments)" 0 "ok: ci workflow frame (7 jobs, rules F1-F6)"

pass "pass: synthetic base" "${BASE}"

sub "${BASE}" "${IN_F}" 's/^    needs:$/    needs: [a, m, c]/;/^      - [amc]$/d'
pass "pass: needs in flow form"

sub "${BASE}" "${IN_F}" 's/^    needs:$/    needs: [a, m, c] # comment/;/^      - [amc]$/d'
pass "pass: needs in flow form with trailing comment"

ins "${BASE}" "${IN_F}" "echo a" "        run: echo ok || { echo x; exit 1; }"
pass "pass: '|| { ...; exit 1; }' form"

{
  cat "${BASE}"
  printf '%s\n' '# continue-on-error: true' '# echo x || true' '# runs-on: self-hosted' '# if: always()'
} >"${IN_F}"
pass "pass: spellings only in comment lines"

ins "${BASE}" "${IN_F}" "echo a" "      # echo x || true and always() and continue-on-error"
pass "pass: indented comment line"

# ---- F1 ----

sub "${BASE}" "${IN_F}" 's/^    if: always()$/    timeout-minutes: 5/'
bad "F1: ci-complete without if" 1 "F1" "job-level if is missing"

sub "${BASE}" "${IN_F}" 's/^    if: always()$/    if: success()/'
bad "F1: ci-complete with other condition" 1 "F1" "job-level if is not exactly always()"

sub "${BASE}" "${IN_F}" 's/^    if: always()$/    if: always() \&\& true/'
bad "F1: always() with extra condition" 1 "F1"

sub "${BASE}" "${IN_F}" 's/ci-complete/ci-done/'
bad "F1: ci-complete missing" 2 "F1" "aggregate job is missing" "F6"

# ---- F2 ----

sub "${BASE}" "${IN_F}" '/^      - c$/d'
bad "F2: needs lacks a job" 1 "F2" "needs lacks job: c"

sub "${BASE}" "${IN_F}" 's/^      - m$/      - zz/'
bad "F2: needs names unknown job and lacks one" 2 "needs lacks job: m" "needs names unknown job: zz"

sub "${BASE}" "${IN_F}" 's/^    needs:$/    needs: a/;/^      - [amc]$/d'
bad "F2: needs scalar covers only one job" 2 "needs lacks job: m" "needs lacks job: c"

sub "${BASE}" "${IN_F}" 's/^    needs:$/    needs: ${{ fromJSON(x) }}/;/^      - [amc]$/d'
bad "F2: needs in unsupported form" 1 "F2" "needs is not scalar"

sub "${BASE}" "${IN_F}" '/^    needs:$/d;/^      - [amc]$/d'
bad "F2: needs missing" 1 "F2" "needs is missing"

sub "${REAL}" "${IN_F}" '/^      - pr-title$/d'
bad "F2: real ci.yml copy without one needs line" 1 "F2" "needs lacks job: pr-title"

# ---- F3 ----

sub "${BASE}" "${IN_F}" '/^  a:$/,/^  m:$/s/ubuntu-latest/self-hosted/'
bad "F3: self-hosted" 1 "F3" "job a" "runs-on is not an allowed label"

sub "${BASE}" "${IN_F}" '/^  a:$/,/^  m:$/s/runs-on: ubuntu-latest/runs-on: [ubuntu-latest]/'
bad "F3: runs-on as array" 1 "F3" "job a"

sub "${BASE}" "${IN_F}" '/^  a:$/,/^  m:$/s/runs-on: ubuntu-latest/runs-on: ubuntu-22.04/'
bad "F3: label outside the allowed set" 1 "F3" "job a"

sub "${BASE}" "${IN_F}" 's/\[ubuntu-latest, macos-latest\]/[ubuntu-latest, self-hosted]/'
bad "F3: matrix element outside the allowed set" 1 "F3" "job m" "disallowed or empty label"

sub "${BASE}" "${IN_F}" 's/runs-on: ${{ matrix.os }}/runs-on: ${{ inputs.runner }}/'
bad "F3: other expression" 1 "F3" "job m"

sub "${BASE}" "${IN_F}" 's/runs-on: ${{ matrix.os }}/runs-on: ${{ matrix.nokey }}/'
bad "F3: matrix key not found" 1 "F3" "job m" "not a literal list"

sub "${BASE}" "${IN_F}" '/^  c:$/,/^  ci-complete:$/{/runs-on/d;}'
bad "F3: job without runs-on" 1 "F3" "job c" "no runs-on"

sub "${BASE}" "${IN_F}" 's/\[ubuntu-latest, macos-latest\]/${{ fromJSON(x) }}/'
bad "F3: matrix from an expression" 1 "F3" "job m"

awk '{ print } /os: \[ubuntu-latest, macos-latest\]/ { print "        include:"; print "          - os: self-hosted" }' "${BASE}" >"${IN_F}"
bad "F3: matrix include is rejected" 1 "F3" "job m" "not a literal list"

awk '{ print } /os: \[ubuntu-latest, macos-latest\]/ { print "        exclude:"; print "          - os: macos-latest" }' "${BASE}" >"${IN_F}"
bad "F3: matrix exclude is rejected" 1 "F3" "job m" "not a literal list"

# ---- F4 ----

ins "${BASE}" "${IN_F}" "echo a" "        continue-on-error: true"
bad "F4: step-level continue-on-error" 1 "F4" "continue-on-error key"

ins "${BASE}" "${IN_F}" "name: a" "    continue-on-error: false"
bad "F4: job-level continue-on-error with false" 1 "F4"

ins "${BASE}" "${IN_F}" "name: a" "    'continue-on-error': true"
bad "F4: single-quoted continue-on-error key" 1 "F4" "continue-on-error key"

ins "${BASE}" "${IN_F}" "name: a" '    "continue-on-error": true'
bad "F4: double-quoted continue-on-error key" 1 "F4" "continue-on-error key"

# ---- F5 ----

sub "${BASE}" "${IN_F}" 's/echo a$/echo a || true/'
bad "F5: '|| true'" 1 "F5" "failure swallowed"

sub "${BASE}" "${IN_F}" 's/echo a$/echo a || :/'
bad "F5: '|| :'" 1 "F5"

sub "${BASE}" "${IN_F}" 's/echo a$/echo a # || true/'
bad "F5: spelling in a trailing comment is still a violation" 1 "F5"

# ---- F6 ----

ins "${BASE}" "${IN_F}" "echo c" "        if: always()"
bad "F6: step-level always() in another job" 1 "F6" "always() outside"

sub "${BASE}" "${IN_F}" "s/^    if: github.event_name == 'pull_request'$/    if: always()/"
bad "F6: job-level always() in another job" 1 "F6" "always() outside"

# ---- 複数 ----

sub "${BASE}" "${IN_F}" '/^  a:$/,/^  m:$/s/ubuntu-latest/self-hosted/;s/echo c$/echo c || true/'
bad "multiple: F3 and F5" 2 "F3" "F5"

# ---- 読めない形 ----

printf 'name: T\non: push\n' >"${IN_F}"
bad "unreadable: no jobs key" 2 "no top-level 'jobs:' key" "aggregate job is missing"

ins "${BASE}" "${IN_F}" "name: a" "$(printf '\t')x: 1"
bad "unreadable: tab in indentation" 1 "tab in indentation"

{
  cat "${BASE}"
  printf '%s\n' '  a:' '    runs-on: ubuntu-latest'
} >"${IN_F}"
bad "unreadable: duplicate job id" 1 "duplicate job id"

ins "${BASE}" "${IN_F}" "name: a" "    env: &anc"
bad "unreadable: anchor" 1 "anchor"

ins "${BASE}" "${IN_F}" "name: a" "    <<: *base"
bad "unreadable: merge key" 1 "merge key"

# ---- 誤用 ----

misuse "misuse: unknown argument" --bogus
misuse "misuse: stray positional" "${BASE}"
misuse "misuse: --workflow without value" --workflow
misuse "misuse: path does not exist" --workflow "${work}/missing.yml"
misuse "misuse: directory instead of file" --workflow "${work}"

# ---- 不変 ----

cp "${BASE}" "${IN_F}"
before="$(cksum <"${IN_F}")"
files_before="$(find "${work}" | wc -l)"
run_check --workflow "${IN_F}"
after="$(cksum <"${IN_F}")"
files_after="$(find "${work}" | wc -l)"
# 控え（stdout/stderr）は既に在るので、検査の前後でファイル数は変わらない。
if [ "${before}" = "${after}" ] && [ "${files_before}" = "${files_after}" ] && [ "${rc}" -eq 0 ]; then
  report_ok "invariant: input and directory unchanged"
else
  report_fail "invariant: input and directory unchanged" "changed"
fi

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
