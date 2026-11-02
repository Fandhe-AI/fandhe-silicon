#!/usr/bin/env bash
# profile-test-count-check.sh の回帰テスト（spec TASK-227 のポインタ。親 Issue #1149 の 2/4）。
# 合成の build-profiles.sh（ダミーの feature 名）と、PATH の先頭に置く偽の cargo で各判定を確かめる。
# 実物の cargo は流さない（実物は本体の `make profile-test-count-check` が持つ）。
# リポジトリのファイルは変更しない・ネットワーク不要。
# 呼び出し元: `make test-profile-test-count-check`（直接実行も可）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」（先頭が `!` の語は含まれないこと）。
# 期待値を緩めない。途中で止めず、最後に件数を出す。bash 3.2 互換。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

TOOL="${DIR}/profile-test-count-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-profile-test-count-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
LOG_F="${work}/fake.log"
passed=0
failed=0
rc=0

mkdir "${work}/bin" "${work}/nocargo"

# 合成の build-profiles.sh。
cat >"${work}/bp.sh" <<'EOF'
SETS=(
  "aa,bb"          # spec BUILD-1
  "aa,bb,cc"       # spec BUILD-2
  "aa,bb,dd,gg"    # spec BUILD-3
  "zz"             # spec BUILD-4
)
EOF
sed 's/^  "aa,bb,cc".*$//' "${work}/bp.sh" >"${work}/bp-zero.sh"
{
  cat "${work}/bp.sh"
  echo '  "aa,bb,cc" # spec BUILD-2'
} >"${work}/bp-dup.sh"
sed 's/^  "aa,bb".*$/  aa,bb # spec BUILD-1/' "${work}/bp.sh" >"${work}/bp-noquote.sh"
sed 's/"aa,bb,cc"/"aa;bb"/' "${work}/bp.sh" >"${work}/bp-badchar.sh"

# 偽の cargo。FAKE_MODE で出力を切り替える。--features の値の先頭で組を見分ける。
cat >"${work}/bin/cargo" <<'EOF'
#!/usr/bin/env bash
echo "ARGS: $*" >>"${FAKE_LOG}"
echo "RECORD_DIR=${FANDHE_SILICON_TEST_RECORD_DIR-unset}" >>"${FAKE_LOG}"
feat=""
prev=""
for a in "$@"; do
  [ "${prev}" = "--features" ] && feat="${a}"
  prev="${a}"
done
case "${feat}" in
  aa,bb,dd,gg,*) prof=G; n=9; g=3 ;;
  aa,bb,cc,*) prof=C; n=6; g=0 ;;
  *) prof=P; n=6; g=0 ;;
esac
mode="${FAKE_MODE:-good}"
rcode=0
sum_extra=""
case "${mode}" in
  count) [ "${prof}" = G ] && n=10 || n=7 ;;
  low) n=$((n - 1)); [ "${prof}" = G ] && g=2 ;;
  cpu-graphics) [ "${prof}" = P ] && g=1 ;;
  gfx2) [ "${prof}" = G ] && g=2 ;;
  cargo-fail) rcode=101 ;;
esac
i=0
while [ "${i}" -lt "${n}" ]; do
  if [ "${i}" -lt "${g}" ]; then echo "test graphics_case_${i} ... ok"; else echo "test plain_case_${i} ... ok"; fi
  i=$((i + 1))
done
[ "${mode}" = nonok ] && echo "test bad_case ... FAILED"
res="test result: ok. ${n} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s"
case "${mode}" in
  failed) res="test result: FAILED. ${n} passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s" ;;
  ignored) res="test result: ok. ${n} passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s" ;;
  filtered) res="test result: ok. ${n} passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s" ;;
esac
[ "${mode}" = nores ] || echo "${res}"
[ "${mode}" = tworesults ] && echo "${res}"
exit "${rcode}"
EOF
chmod +x "${work}/bin/cargo"

# run_fake <mode> <args...>: 偽の cargo を PATH の先頭に入れて流す。
run_fake() {
  local mode="$1"
  shift
  rc=0
  : >"${LOG_F}"
  PATH="${work}/bin:${PATH}" FAKE_LOG="${LOG_F}" FAKE_MODE="${mode}" \
    "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
}

record() {
  local name="$1" ok="$2"
  if [ "${ok}" -eq 1 ]; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    echo "FAIL ${name}" >&2
  fi
}

# expect <name> <終了コード> <語>...
expect() {
  local name="$1" want_rc="$2" word ok=1
  shift 2
  if [ "${rc}" -ne "${want_rc}" ]; then
    echo "FAIL ${name}: exit ${rc}, expected ${want_rc}" >&2
    ok=0
  fi
  if grep -q -F -e "${work}" "${OUT_F}" "${ERR_F}"; then
    echo "FAIL ${name}: output contains a temp path" >&2
    ok=0
  fi
  for word in "$@"; do
    case "${word}" in
      '!'*)
        if grep -q -F -e "${word#!}" "${OUT_F}" "${ERR_F}"; then
          echo "FAIL ${name}: unexpected text: ${word#!}" >&2
          ok=0
        fi
        ;;
      *)
        if ! grep -q -F -e "${word}" "${OUT_F}" "${ERR_F}"; then
          echo "FAIL ${name}: missing text: ${word}" >&2
          ok=0
        fi
        ;;
    esac
  done
  record "${name}" "${ok}"
}

# expect_log <name> <語>...: 偽の cargo の記録に対する部分一致。
expect_log() {
  local name="$1" word ok=1
  shift
  for word in "$@"; do
    case "${word}" in
      '!'*)
        if grep -q -F -e "${word#!}" "${LOG_F}"; then
          echo "FAIL ${name}: unexpected log text: ${word#!}" >&2
          ok=0
        fi
        ;;
      *)
        if ! grep -q -F -e "${word}" "${LOG_F}"; then
          echo "FAIL ${name}: missing log text: ${word}" >&2
          ok=0
        fi
        ;;
    esac
  done
  record "${name}" "${ok}"
}

BP="${work}/bp.sh"

run_fake good --build-profiles "${BP}"
expect "good" 0 \
  "record: profile=CpuOnly features=aa,bb,test-support passed=6 graphics=0" \
  "record: profile=ComputeOnly features=aa,bb,cc,test-support passed=6 graphics=0" \
  "record: profile=Graphics features=aa,bb,dd,gg,test-support passed=9 graphics=3" \
  "profile-test-count-check: all profiles passed" "!FAILED"
expect_log "good-args" \
  "ARGS: test -p fandhe-silicon-contract --no-default-features --features aa,bb,test-support --test stub_device_profiles" \
  "ARGS: test -p fandhe-silicon-contract --no-default-features --features aa,bb,cc,test-support --test stub_device_profiles" \
  "ARGS: test -p fandhe-silicon-contract --no-default-features --features aa,bb,dd,gg,test-support --test stub_device_profiles" \
  "!-q"

# 環境に置いた記録先は子に渡らない。
rc=0
: >"${LOG_F}"
FANDHE_SILICON_TEST_RECORD_DIR="${work}/rec" PATH="${work}/bin:${PATH}" FAKE_LOG="${LOG_F}" FAKE_MODE=good \
  "${TOOL}" --build-profiles "${BP}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "record-dir-unset" 0 "all profiles passed"
expect_log "record-dir-unset-log" "RECORD_DIR=unset" "!RECORD_DIR=${work}"

run_fake count --build-profiles "${BP}"
expect "count-differs" 1 "reason=passed-count" "FAILED: profile=CpuOnly" "FAILED: profile=Graphics" "!all profiles passed"
run_fake low --build-profiles "${BP}"
expect "count-low" 1 "reason=passed-count" "reason=graphics-count"
run_fake failed --build-profiles "${BP}"
expect "failed-nonzero" 1 "reason=result-format" "!all profiles passed"
run_fake ignored --build-profiles "${BP}"
expect "ignored-nonzero" 1 "reason=result-format"
run_fake filtered --build-profiles "${BP}"
expect "filtered-out-nonzero" 1 "reason=result-format"
run_fake nores --build-profiles "${BP}"
expect "result-zero-lines" 1 "reason=result-lines"
run_fake tworesults --build-profiles "${BP}"
expect "result-two-lines" 1 "reason=result-lines"
run_fake cargo-fail --build-profiles "${BP}"
expect "cargo-exit" 1 "reason=cargo-exit"
run_fake cpu-graphics --build-profiles "${BP}"
expect "cpu-has-graphics" 1 "reason=graphics-count" "FAILED: profile=CpuOnly"
run_fake gfx2 --build-profiles "${BP}"
expect "graphics-two" 1 "reason=graphics-count" "FAILED: profile=Graphics"
run_fake nonok --build-profiles "${BP}"
expect "non-ok-line" 1 "reason=non-ok-lines"

run_fake good --build-profiles "${work}/bp-zero.sh"
expect "note-zero" 2 "exactly 1 note line"
expect_log "note-zero-no-cargo" "!ARGS"
run_fake good --build-profiles "${work}/bp-dup.sh"
expect "note-two" 2 "exactly 1 note line"
expect_log "note-two-no-cargo" "!ARGS"
run_fake good --build-profiles "${work}/bp-noquote.sh"
expect "no-quote" 2 "no quoted feature set"
expect_log "no-quote-no-cargo" "!ARGS"
run_fake good --build-profiles "${work}/bp-badchar.sh"
expect "bad-char" 2 "invalid characters"
expect_log "bad-char-no-cargo" "!ARGS"

run_fake good --bogus
expect "unknown-arg" 2 "usage:"
run_fake good --build-profiles
expect "missing-value" 2 "usage:"
run_fake good --build-profiles "${work}/none.sh"
expect "missing-path" 2 "cannot read"

# cargo の無い PATH。
ln -s "$(command -v bash)" "${work}/nocargo/bash"
for t in grep sed wc tr cat mktemp rm dirname basename; do
  p="$(command -v "${t}")"
  ln -s "${p}" "${work}/nocargo/${t}"
done
rc=0
PATH="${work}/nocargo" "$(command -v bash)" "${TOOL}" --build-profiles "${BP}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "no-cargo" 2 "required command not found"

echo "test-profile-test-count-check: ${passed} passed, ${failed} failed"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
