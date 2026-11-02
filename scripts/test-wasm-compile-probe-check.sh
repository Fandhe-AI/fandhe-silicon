#!/usr/bin/env bash
# wasm-compile-probe-check.sh の回帰テスト（spec TASK-239 のポインタ。親 Issue #463 の 1/4）。
# 合成の build-profiles.sh（ダミーの feature 名）と、PATH の先頭に置く偽の cargo で各判定を確かめる。
# 実物の cargo は流さない（実物は本体の `make wasm-compile-probe-check` が持つ）。
# リポジトリのファイルは変更しない・ネットワーク不要。
# 呼び出し元: `make test-wasm-compile-probe-check`（直接実行も可）。CI の deny ジョブも同じ make ターゲットを呼ぶ。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」（先頭が `!` の語は含まれないこと）。
# 期待値を緩めない。途中で止めず、最後に件数を出す。bash 3.2 互換。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

TOOL="${DIR}/wasm-compile-probe-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-wasm-compile-probe-check.XXXXXX")"
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
  "xx"             # spec BUILD-1
)
WASM_SETS=(
  "aa,bb,cc"       # spec BUILD-19
  "aa,bb,cc,dd"    # spec BUILD-22
)
EOF
sed 's/^  "aa,bb,cc".*$//' "${work}/bp.sh" >"${work}/bp-zero.sh"
{
  cat "${work}/bp.sh"
  echo '  "aa,bb,cc" # spec BUILD-19'
} >"${work}/bp-dup.sh"
sed 's/^  "aa,bb,cc".*$/  aa,bb,cc # spec BUILD-19/' "${work}/bp.sh" >"${work}/bp-noquote.sh"
sed 's/"aa,bb,cc"/"aa;bb"/' "${work}/bp.sh" >"${work}/bp-badchar.sh"

# 偽の cargo。--manifest-path の Cargo.toml の package 名で行を見分け、FAKE_MODE で場面を切り替える。
cat >"${work}/bin/cargo" <<'EOF'
#!/usr/bin/env bash
echo "ARGS: $*" >>"${FAKE_LOG}"
echo "RUSTFLAGS=${RUSTFLAGS-unset}" >>"${FAKE_LOG}"
echo "CARGO_TARGET_DIR=${CARGO_TARGET_DIR-unset}" >>"${FAKE_LOG}"
manifest=""
prev=""
for a in "$@"; do
  [ "${prev}" = "--manifest-path" ] && manifest="${a}"
  prev="${a}"
done
pkg="$(sed -n 's/^name = "\(.*\)"$/\1/p' "${manifest}")"
sed -n '/^\[dependencies\]/,/^$/p' "${manifest}" | grep -v '^\[' | grep -v '^$' | sed "s|^|MANIFEST ${pkg}: |" >>"${FAKE_LOG}"
mode="${FAKE_MODE:-good}"
[ "${mode}" = no-target ] && { echo "error: the wasm32-unknown-unknown target may not be installed" >&2; exit 101; }
fail_with() { # <コード> <識別子>
  echo "error[$1]: no method named \`$2\` found for reference" >&2
  echo "  --> ${manifest}:3:1" >&2
  exit 101
}
fail_bound() { # <型>
  echo "error[E0277]: \`$1\` cannot be shared between threads safely" >&2
  echo "  --> ${manifest}:3:1" >&2
  exit 101
}
# TASK-553 の行の分岐。
case "${pkg}" in
  wasm-compile-probe-native-handle-rejects-not-send|wasm-compile-probe-encoder-closer-rejects-not-send)
    [ "${mode}" = bounds-relaxed ] && exit 0
    case "${pkg}" in *native-handle*) fail_bound 'Rc<u8>' ;; *) fail_bound 'Rc<()>' ;; esac ;;
  wasm-compile-probe-shared-closer-rejects-not-sync)
    case "${mode}" in
      bounds-relaxed) exit 0 ;;
      bound-other-type) fail_bound 'Rc<u8>' ;;
      *) fail_bound 'Cell<u32>' ;;
    esac ;;
  wasm-compile-probe-native-handle-send-sync|wasm-compile-probe-command-encoder-send-sync)
    [ "${mode}" = bounds-relaxed ] && { echo "error[E0277]: \`(dyn Any + 'static)\` cannot be sent between threads safely" >&2; exit 101; }
    exit 0 ;;
  wasm-compile-probe-download-blocking)
    case "${mode}" in
      fail-row-passes) exit 0 ;;
      fail-row-other-code) fail_with E0433 download_blocking ;;
      *) fail_with E0599 download_blocking ;;
    esac ;;
  wasm-compile-probe-wait-blocking)
    case "${mode}" in
      fail-row-other-ident) fail_with E0599 other_name ;;
      *) fail_with E0599 wait_blocking ;;
    esac ;;
  wasm-compile-probe-nonblocking-readback)
    [ "${mode}" = pass-row-fails ] && { echo "error: boom" >&2; exit 101; }
    exit 0 ;;
  wasm-compile-probe-*-gate-open)
    [ "${mode}" = control-row-fails ] && { echo "error: boom" >&2; exit 101; }
    exit 0 ;;
esac
exit 0
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
  "record: probe=download_blocking target=wasm32-unknown-unknown features=aa,bb,cc expect=fail code=E0599 result=ok" \
  "record: probe=wait_blocking target=wasm32-unknown-unknown features=aa,bb,cc expect=fail code=E0599 result=ok" \
  "record: probe=nonblocking_readback target=wasm32-unknown-unknown features=aa,bb,cc expect=pass code=- result=ok" \
  "record: probe=download_blocking_gate_open target=wasm32-unknown-unknown features=aa,bb,cc,native-blocking-wait expect=pass code=- result=ok" \
  "record: probe=wait_blocking_gate_open target=wasm32-unknown-unknown features=aa,bb,cc,native-blocking-wait expect=pass code=- result=ok" \
  "record: probe=native_handle_send_sync target=wasm32-unknown-unknown features=aa,bb,cc expect=pass code=- result=ok" \
  "record: probe=native_handle_rejects_not_send target=wasm32-unknown-unknown features=aa,bb,cc expect=fail code=E0277 result=ok" \
  "record: probe=encoder_closer_send_sync target=wasm32-unknown-unknown features=aa,bb,cc expect=pass code=- result=ok" \
  "record: probe=encoder_closer_rejects_not_send target=wasm32-unknown-unknown features=aa,bb,cc expect=fail code=E0277 result=ok" \
  "record: probe=shared_closer_rejects_not_sync target=wasm32-unknown-unknown features=aa,bb,cc expect=fail code=E0277 result=ok" \
  "record: probe=command_encoder_send_sync target=wasm32-unknown-unknown features=aa,bb,cc expect=pass code=- result=ok" \
  "wasm-compile-probe-check: all probes passed" "!result=ng"
expect_log "good-args" \
  "ARGS: check" "--offline" "--target wasm32-unknown-unknown" "--manifest-path" "--target-dir" "!--locked" "!-D"
expect_log "good-manifest" \
  "MANIFEST wasm-compile-probe-download-blocking: fandhe-silicon-contract = { path = " \
  "default-features = false, features = [\"aa\", \"bb\", \"cc\"] }" \
  "default-features = false, features = [\"aa\", \"bb\", \"cc\", \"native-blocking-wait\"] }" \
  "MANIFEST wasm-compile-probe-wait-blocking-gate-open: fandhe-silicon-contract" \
  "MANIFEST wasm-compile-probe-nonblocking-readback: fandhe-silicon-core = { path = " \
  "!fandhe-silicon-core = { path = \"${ROOT_DIR}/crates/core\", "

# 環境に置いた RUSTFLAGS・出力先は子に渡らない。
rc=0
: >"${LOG_F}"
RUSTFLAGS="-D warnings" CARGO_TARGET_DIR="${work}/tgt" PATH="${work}/bin:${PATH}" FAKE_LOG="${LOG_F}" FAKE_MODE=good \
  "${TOOL}" --build-profiles "${BP}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "env-unset" 0 "all probes passed"
expect_log "env-unset-log" "RUSTFLAGS=unset" "CARGO_TARGET_DIR=unset" "!RUSTFLAGS=-D"

run_fake fail-row-passes --build-profiles "${BP}"
expect "fail-row-passes" 1 "probe=download_blocking target=wasm32-unknown-unknown features=aa,bb,cc expect=fail code=E0599 result=ng" "1 probe(s) failed" "!all probes passed"
run_fake fail-row-other-code --build-profiles "${BP}"
expect "fail-row-other-code" 1 "probe=download_blocking" "result=ng" "1 probe(s) failed"
run_fake fail-row-other-ident --build-profiles "${BP}"
expect "fail-row-other-ident" 1 "probe=wait_blocking target" "result=ng" "1 probe(s) failed"
run_fake pass-row-fails --build-profiles "${BP}"
expect "pass-row-fails" 1 "probe=nonblocking_readback target=wasm32-unknown-unknown features=aa,bb,cc expect=pass code=- result=ng" "error: boom" "1 probe(s) failed"
run_fake control-row-fails --build-profiles "${BP}"
expect "control-row-fails" 1 "2 probe(s) failed" "!all probes passed"
run_fake bound-other-type --build-profiles "${BP}"
expect "bound-other-type" 1 "probe=shared_closer_rejects_not_sync target=wasm32-unknown-unknown features=aa,bb,cc expect=fail code=E0277 result=ng" "1 probe(s) failed"
run_fake bounds-relaxed --build-profiles "${BP}"
expect "bounds-relaxed" 1 "probe=native_handle_send_sync target" "probe=command_encoder_send_sync target" "5 probe(s) failed" "!all probes passed"
run_fake no-target --build-profiles "${BP}"
expect "no-target" 2 "target is not installed" "!all probes passed" "!record:"

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
for t in grep sed wc tr cat mktemp rm dirname basename env mkdir; do
  p="$(command -v "${t}")"
  ln -s "${p}" "${work}/nocargo/${t}"
done
rc=0
PATH="${work}/nocargo" "$(command -v bash)" "${TOOL}" --build-profiles "${BP}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "no-cargo" 2 "required command not found"

echo "test-wasm-compile-probe-check: ${passed} passed, ${failed} failed"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
