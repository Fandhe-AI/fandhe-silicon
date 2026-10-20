#!/usr/bin/env bash
# scripts/profile-dep-graph-positive-control.sh の回帰テスト（spec TASK-228 のポインタ。親 Issue #459 の 3/6）。
# 本体と lib.sh を一時ディレクトリの scripts/ へ写し、隣に偽の profile-dep-graph-check.sh を置いて、
# 内側の終了コード・`record:`・`count:`・`violation:` の行から、プロファイルごとの判定が出ることを確かめる
# （本体は兄弟のスクリプトを DIR 経由で呼ぶため、写しが要る）。環境変数で偽の振る舞いを切り替える。
# 末尾で、実物のリポジトリに対して本物の本体を流す（実物の検査に 1 件足すと落ちることの確認）。
# ネットワークは使わず、リポジトリのファイルを変更しない。
# 呼び出し元: `make test-profile-dep-graph-positive-control`（直接実行も可）。
# 前提: python3・cargo・git・rust-toolchain.toml の toolchain（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う。期待値を緩めない。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd cargo
require_cmd git

work="$(mktemp -d "${TMPDIR:-/tmp}/test-profile-dep-graph-positive-control.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

report_ok() {
  passed=$((passed + 1))
  echo "ok: $1"
}
report_fail() {
  failed=$((failed + 1))
  echo "FAIL: $1: $2" >&2
}

# judge <名前> <期待コード> [語...]: o:<語> は標準出力に、e:<語> は標準エラーに含まれること。!<語> はどちらにも含まれないこと。
judge() {
  local name="$1" want="$2" s
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    report_fail "${name}" "expected exit ${want}, got ${rc}"
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
      o:*)
        if ! grep -qF -e "${s#o:}" "${OUT_F}"; then
          report_fail "${name}" "stdout lacks: ${s#o:}"
          return 0
        fi
        ;;
      e:*)
        if ! grep -qF -e "${s#e:}" "${ERR_F}"; then
          report_fail "${name}" "stderr lacks: ${s#e:}"
          return 0
        fi
        ;;
    esac
  done
  report_ok "${name}"
}

# ---- 偽の内側の検査を置いた写し ----
fx="${work}/fx"
mkdir -p "${fx}/scripts"
cp "${DIR}/profile-dep-graph-positive-control.sh" "${DIR}/lib.sh" "${fx}/scripts/"
chmod +x "${fx}/scripts/profile-dep-graph-positive-control.sh"
cat >"${fx}/scripts/profile-dep-graph-check.sh" <<'FAKE'
#!/usr/bin/env bash
# 偽の内側の検査。引数なしは基準、--extra-judged-crate 付きは足した組。FAKE_* の環境変数で振る舞いを切り替える。
set -u
extra=""
if [ "${1:-}" = "--extra-judged-crate" ]; then
  extra="$2"
fi
# 4/6 で足した比較基準の行。陽性対照の読み方を乱さないことを確かめるため、足した名前を含む形で出す。
baseline_lines() {
  [ "${FAKE_BASELINE_LINES:-}" = "1" ] || return 0
  echo "baseline: commit=- profile=all-features crates_total=2 sdk_total=0"
  echo "baseline-count: profile=all-features crate=${extra:-ash} n=1"
  for q in ${profiles}; do
    echo "diff: profile=${q} baseline=all-features sdk_baseline=0 sdk_profile=0 diff=0"
  done
}
if [ -z "${extra}" ]; then
  profiles="${FAKE_BASE_PROFILES:-cpu-only compute-only-metal}"
  for p in ${profiles}; do
    echo "scanned: profile=${p} crates=2"
    echo "record: commit=- profile=${p} crates_total=2 judged_total=${FAKE_BASE_TOTAL:-0}"
    echo "count: profile=${p} crate=ash n=${FAKE_BASE_TOTAL:-0}"
  done
  baseline_lines
  exit "${FAKE_BASE_RC:-0}"
fi
profiles="${FAKE_EXTRA_PROFILES:-cpu-only compute-only-metal}"
for p in ${profiles}; do
  case "${p}" in
    cpu-only) n="${FAKE_N_CPU:-1}" ;;
    *) n="${FAKE_N_CM:-1}" ;;
  esac
  total=$(( ${FAKE_BASE_TOTAL:-0} + ${FAKE_TOTAL_DELTA:-0} ))
  if [ "${n}" != "bad" ]; then
    total=$((total + n))
  fi
  echo "scanned: profile=${p} crates=2"
  if [ "${FAKE_OMIT_RECORD:-}" != "${p}" ]; then
    echo "record: commit=- profile=${p} crates_total=2 judged_total=${total} extra_judged_crate=${extra}"
  fi
  if [ "${FAKE_DUP_RECORD:-}" = "${p}" ]; then
    echo "record: commit=- profile=${p} crates_total=2 judged_total=${total} extra_judged_crate=${extra}"
  fi
  if [ "${FAKE_OMIT_COUNT:-}" != "${p}" ]; then
    if [ "${n}" = "bad" ]; then
      echo "count: profile=${p} crate=${extra} n=-1"
    else
      echo "count: profile=${p} crate=${extra} n=${n}"
    fi
  fi
  if [ "${FAKE_DUP_COUNT:-}" = "${p}" ]; then
    echo "count: profile=${p} crate=${extra} n=1"
  fi
  if [ "${n}" != "bad" ] && [ "${n}" -ge 1 ]; then
    echo "violation: profile=${p} crate=${extra} version=0.0.0" >&2
  fi
done
baseline_lines
if [ -n "${FAKE_VIOLATION_OTHER:-}" ]; then
  echo "violation: profile=cpu-only crate=ash version=0.38.0" >&2
fi
exit "${FAKE_EXTRA_RC:-1}"
FAKE
chmod +x "${fx}/scripts/profile-dep-graph-check.sh"
POSITIVE="${fx}/scripts/profile-dep-graph-positive-control.sh"

# run_fake [環境変数...]: 偽の内側で本体を流す。
run_fake() {
  rc=0
  env "$@" "${POSITIVE}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# ---- 合格 ----
run_fake FAKE_X=1
judge "detected_in_every_profile" 0 \
  "o:positive-control: profile=cpu-only crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=compute-only-metal crate=fandhe-silicon-core n=1 result=detected" \
  "o:ok: positive control detected" "o:record: commit=- profile=cpu-only" "!positive-control-failed" "!${work}"

run_fake FAKE_N_CPU=2 FAKE_N_CM=3
judge "detected_counts_are_not_compared_to_a_fixed_value" 0 "o:profile=cpu-only crate=fandhe-silicon-core n=2 result=detected" \
  "o:profile=compute-only-metal crate=fandhe-silicon-core n=3 result=detected"

run_fake FAKE_BASE_PROFILES=cpu-only FAKE_EXTRA_PROFILES=cpu-only
judge "single_profile" 0 "o:profile=cpu-only crate=fandhe-silicon-core n=1 result=detected" "!compute-only-metal"

run_fake FAKE_BASELINE_LINES=1
judge "baseline_lines_are_not_read" 0 \
  "o:positive-control: profile=cpu-only crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=compute-only-metal crate=fandhe-silicon-core n=1 result=detected" \
  "o:ok: positive control detected" "!positive-control-failed" "!${work}"

run_fake FAKE_BASELINE_LINES=1 \
  "FAKE_BASE_PROFILES=cpu-only compute-only-metal graphics-vulkan graphics-interop" \
  "FAKE_EXTRA_PROFILES=cpu-only compute-only-metal graphics-vulkan graphics-interop"
judge "four_profiles" 0 \
  "o:positive-control: profile=cpu-only crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=compute-only-metal crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=graphics-vulkan crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=graphics-interop crate=fandhe-silicon-core n=1 result=detected" \
  "o:ok: positive control detected" "!positive-control-failed" "!${work}"

# ---- 失敗（1）: 足しても落ちない ----
run_fake FAKE_N_CPU=0 FAKE_N_CM=0 FAKE_EXTRA_RC=0
judge "no_detection_anywhere" 1 "e:positive-control-failed: profile=cpu-only crate=fandhe-silicon-core n=0 result=no-detection" \
  "e:positive-control-failed: profile=compute-only-metal crate=fandhe-silicon-core n=0 result=no-detection" "!ok: positive control" "!result=detected"

run_fake FAKE_N_CM=0
judge "no_detection_in_one_profile" 1 "o:positive-control: profile=cpu-only crate=fandhe-silicon-core n=1 result=detected" \
  "e:positive-control-failed: profile=compute-only-metal crate=fandhe-silicon-core n=0 result=no-detection" "!ok: positive control"

run_fake FAKE_BASE_RC=1 FAKE_BASE_TOTAL=1
judge "baseline_has_a_violation" 1 "e:positive-control-failed: crate=fandhe-silicon-core result=baseline-violation" "!ok: positive control" "!result=detected"

# ---- fail-closed（2）----
run_fake FAKE_BASE_RC=2
judge "baseline_inner_error" 2 "e:positive-control-failed: crate=fandhe-silicon-core result=inner-error" "!ok: positive control"
run_fake FAKE_BASE_RC=7
judge "baseline_unexpected_exit" 2 "e:result=inner-error" "!ok: positive control"
run_fake FAKE_EXTRA_RC=2
judge "extra_inner_error" 2 "e:result=inner-error" "!ok: positive control" "!result=detected"
run_fake FAKE_BASE_RC=1 FAKE_BASE_TOTAL=1 FAKE_EXTRA_RC=2
judge "baseline_violation_and_extra_inner_error" 2 "e:result=inner-error" "!result=baseline-violation" "!ok: positive control"
run_fake FAKE_EXTRA_RC=0
judge "extra_exit_zero_but_counted" 2 "e:result=inconsistent" "!ok: positive control" "!result=detected"
run_fake FAKE_N_CPU=0 FAKE_N_CM=0
judge "extra_exit_one_but_not_counted" 2 "e:result=inconsistent" "!ok: positive control"
run_fake FAKE_OMIT_COUNT=cpu-only
judge "count_line_missing" 2 "e:result=profile-mismatch" "!ok: positive control" "!result=detected"
run_fake FAKE_DUP_COUNT=cpu-only
judge "count_line_duplicated" 2 "e:result=unreadable-count" "!ok: positive control" "!result=detected"
run_fake FAKE_N_CPU=bad
judge "count_value_not_an_integer" 2 "e:result=unreadable-count" "!ok: positive control" "!result=detected"
run_fake FAKE_OMIT_RECORD=compute-only-metal
judge "record_line_missing" 2 "e:result=profile-mismatch" "!ok: positive control"
run_fake FAKE_DUP_RECORD=cpu-only
judge "record_line_duplicated" 2 "e:result=unreadable-record" "!ok: positive control"
run_fake FAKE_EXTRA_PROFILES=cpu-only
judge "profile_set_differs" 2 "e:result=profile-mismatch" "!ok: positive control" "!result=detected"
run_fake FAKE_TOTAL_DELTA=1
judge "judged_total_difference_mismatch" 2 "e:result=inconsistent" "!ok: positive control" "!result=detected"
run_fake FAKE_VIOLATION_OTHER=1
judge "violation_of_another_name" 2 "e:result=unexpected-violation" "!ok: positive control" "!result=detected"

# ---- 前提の不足 ----
rc=0
"${POSITIVE}" extra-arg >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "misuse_arguments" 2 "e:usage:" "!ok: positive control"

# python3 が無い環境（PATH を最小のディレクトリだけにする）
nopy="${work}/nopy"
mkdir -p "${nopy}"
ln -s "$(command -v bash)" "${nopy}/bash"
for t in dirname env; do
  ln -s "$(command -v "${t}")" "${nopy}/${t}"
done
rc=0
PATH="${nopy}" "${nopy}/bash" "${POSITIVE}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_python3" 2 "e:required command not found" "!ok: positive control" "!result=detected"

# ---- 実物のリポジトリ（本物の内側の検査に足して落ちることを確かめる）----
rc=0
"${DIR}/profile-dep-graph-positive-control.sh" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "repo_real" 0 "o:positive-control: profile=cpu-only crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=compute-only-metal crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=graphics-vulkan crate=fandhe-silicon-core n=1 result=detected" \
  "o:positive-control: profile=graphics-interop crate=fandhe-silicon-core n=1 result=detected" \
  "o:ok: positive control detected" "!positive-control-failed" "!error:" "!${ROOT_DIR}"

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
