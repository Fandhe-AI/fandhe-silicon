#!/usr/bin/env bash
# profile-dep-graph-check.sh の回帰テスト（spec TASK-228 のポインタ）。
# 一時ディレクトリの合成 cargo tree のテキスト（ダミーの名前）を --tree-text / --profile で渡し、
# 終了コード・標準出力・標準エラーの文言を確かめる。合成の節は cargo を呼ばない。
# ComputeOnly（Metal）の組は、features の辺を含む tree（--features-text）も渡して per-header feature を数える。
# 「偽のコマンド」の節は、PATH の先頭に置いた偽の cargo / git で引数なしの経路を通す。
# 「実物」の節は、実行時に一時ディレクトリへ作る合成 path crate の一時 workspace に検査を写し、
# 実物の cargo tree の出力で通す（feature の組と全 target が効いていることもここで確かめる）。
# 末尾で、実物のリポジトリに対する引数なしの検査も流す。ネットワークは使わず、リポジトリのファイルを変更しない。
# 呼び出し元: `make test-profile-dep-graph-check`（直接実行も可）。
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

CHECK="${DIR}/profile-dep-graph-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-profile-dep-graph-check.XXXXXX")"
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

# count_is <名前> <ファイル> <固定文字列> <件数>: 語を含む行数が一致すること。
count_is() {
  local name="$1" file="$2" word="$3" want="$4" got
  got="$(grep -cF -e "${word}" "${file}" || true)"
  if [ "${got}" = "${want}" ]; then
    report_ok "${name}"
  else
    report_fail "${name}" "expected ${want} line(s) containing '${word}', got ${got}"
  fi
}

# run_tree <ケース名> [引数...]: 合成 tree を --tree-text で渡す。引数が無ければ cpu-only。
run_tree() {
  rc=0
  "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# ---- 合成の tree ----
mk_tree() { # mk_tree <ケース名> <行...>
  local f="${work}/$1.tree"
  shift
  : >"${f}"
  local l
  for l in "$@"; do
    printf '%s\n' "${l}" >>"${f}"
  done
}
BASE1="fandhe-silicon-contract v0.0.0 (/dummy/crates/contract)"
BASE2="fandhe-silicon-core v0.0.0 (/dummy/crates/core)"
mk_tree ok_basic "${BASE1}" "${BASE2}"
mk_tree bad_ash "${BASE1}" "${BASE2}" "ash v0.38.0"
mk_tree bad_underscore "${BASE1}" "raw_window_handle v0.6.2"
mk_tree bad_fold_case "${BASE1}" "WinIt v0.30.0"
mk_tree bad_two "${BASE1}" "ash v0.38.0" "libloading v0.8.1" "libloading v0.7.0"
mk_tree ok_decorated "${BASE1}" "${BASE2} (*)" "macro-dummy v1.0.0 (proc-macro) (/dummy/macro)" "other-dummy v2.0.0 (*)"
mk_tree ok_prefix_only "${BASE1}" "ash-window-x v1.0.0" "winit-foo v1.0.0" "xash v1.0.0"
mk_tree err_empty
mk_tree err_blank "" "  "
mk_tree err_no_package "${BASE2}" "other-dummy v1.0.0"
mk_tree err_bad_line "${BASE1}" "this line has no version"
mk_tree err_no_v "${BASE1}" "ash 0.38.0"

# ---- 合成の節 ----
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only
judge "tree_ok" 0 "o:ok: " "o:scanned: profile=cpu-only crates=2" \
  "o:record: commit=- profile=cpu-only crates_total=2 judged_total=0" "!violation" "!FAILED" "!error:" "!${work}"
count_is "tree_ok_counts" "${OUT_F}" "count: profile=cpu-only crate=" 7
count_is "tree_ok_counts_zero" "${OUT_F}" " n=0" 7
for n in objc2-metal ash cudarc libloading objc2-quartz-core raw-window-handle winit; do
  count_is "tree_ok_count_${n}" "${OUT_F}" "count: profile=cpu-only crate=${n} n=0" 1
done

run_tree --tree-text "${work}/bad_ash.tree" --profile cpu-only
judge "tree_ash" 1 "e:violation: profile=cpu-only crate=ash version=0.38.0" "e:FAILED: 1 judged crate(s) found" \
  "o:judged_total=1" "o:crate=ash n=1" "!ok: " "!error:" "!${work}"

run_tree --tree-text "${work}/bad_underscore.tree" --profile cpu-only
judge "tree_underscore" 1 "e:violation: profile=cpu-only crate=raw-window-handle" "!ok: "

run_tree --tree-text "${work}/bad_fold_case.tree" --profile cpu-only
judge "tree_case" 1 "e:violation: profile=cpu-only crate=winit" "!ok: "

run_tree --tree-text "${work}/bad_two.tree" --profile cpu-only
judge "tree_two" 1 "e:FAILED: 3 judged crate(s) found" "o:crate=libloading n=2" "!ok: "

run_tree --tree-text "${work}/ok_decorated.tree" --profile cpu-only
judge "tree_decorated" 0 "o:crates_total=4" "o:ok: " "!macro-dummy" "!/dummy" "!proc-macro"

run_tree --tree-text "${work}/ok_prefix_only.tree" --profile cpu-only
judge "tree_prefix_only" 0 "o:judged_total=0" "o:ok: " "!violation"

# ---- 合成の節（--extra-judged-crate: 一覧に 1 件足すと落ちる）----
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate fandhe-silicon-core
judge "extra_listed_name_fails" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core version=0.0.0" \
  "e:FAILED: 1 judged crate(s) found" "o:count: profile=cpu-only crate=fandhe-silicon-core n=1" \
  "o:judged_total=1 extra_judged_crate=fandhe-silicon-core" "!ok: " "!error:" "!${work}"
count_is "extra_count_lines" "${OUT_F}" "count: profile=cpu-only crate=" 8

run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate dummy-absent
judge "extra_absent_name_passes" 0 "o:ok: 1 profile(s) checked, no judged crates (judged=0) extra_judged_crate=dummy-absent" \
  "o:count: profile=cpu-only crate=dummy-absent n=0" "o:judged_total=0 extra_judged_crate=dummy-absent" "!violation" "!FAILED"

run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate Fandhe_Silicon_Core
judge "extra_name_is_folded" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core" "!ok: "

run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate fandhe-silicon
judge "extra_name_is_exact" 0 "o:ok: " "o:crate=fandhe-silicon n=0" "!violation"

run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only
judge "no_extra_leaves_record_unchanged" 0 "o:ok: 1 profile(s) checked, no judged crates (judged=0)" "!extra_judged_crate"
count_is "no_extra_ok_line_is_exact" "${OUT_F}" "judged=0)" 1

# 誤用: 空・不正な文字・長すぎる・既に判定の名前にある・2 回の指定・値なし
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate ""
judge "extra_misuse_empty" 2 "e:error: invalid --extra-judged-crate" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate "a b"
judge "extra_misuse_space" 2 "e:error: invalid --extra-judged-crate" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate "../x"
judge "extra_misuse_path" 2 "e:error: invalid --extra-judged-crate" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate "$(printf 'a%.0s' $(seq 1 65))"
judge "extra_misuse_too_long" 2 "e:error: invalid --extra-judged-crate" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate ash
judge "extra_misuse_already_judged" 2 "e:error: --extra-judged-crate is already judged" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate ASH
judge "extra_misuse_already_judged_folded" 2 "e:error: --extra-judged-crate is already judged" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate winit
judge "extra_misuse_already_judged_other_profile" 2 "e:error: --extra-judged-crate is already judged" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate a --extra-judged-crate b
judge "extra_misuse_twice" 2 "e:error: --extra-judged-crate takes exactly one name" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --extra-judged-crate
judge "extra_misuse_no_value" 2 "!ok: " "!record: "

run_tree --tree-text "${work}/err_empty.tree" --profile cpu-only
judge "tree_empty" 2 "e:error: no crates found" "!ok: " "!record: "
run_tree --tree-text "${work}/err_blank.tree" --profile cpu-only
judge "tree_blank_only" 2 "e:error: no crates found" "!ok: " "!record: "
run_tree --tree-text "${work}/err_no_package.tree" --profile cpu-only
judge "tree_no_package" 2 "e:error: the checked crate is missing" "!ok: " "!record: "
run_tree --tree-text "${work}/err_bad_line.tree" --profile cpu-only
judge "tree_bad_line" 2 "e:error: unreadable cargo tree line" "!ok: " "!record: "
run_tree --tree-text "${work}/err_no_v.tree" --profile cpu-only
judge "tree_no_v" 2 "e:error: unreadable cargo tree line" "!ok: " "!record: "

# ---- 合成の節（compute-only-metal: crate と per-header feature）----
mk_tree cm_plain "${BASE1}" "${BASE2}"
mk_tree cm_plain_header "${BASE1}" "objc2-metal v0.3.0"
mk_tree cm_plain_winit "${BASE1}" "winit v0.30.0"
mk_tree cm_f_none "${BASE1}" "objc2-metal v0.3.0"
mk_tree cm_f_judged "${BASE1}" "objc2-metal v0.3.0" 'objc2-metal feature "MTLDrawable"'
mk_tree cm_f_recorded "${BASE1}" "objc2-metal v0.3.0" 'objc2-metal feature "MTLPixelFormat"'
mk_tree cm_f_decorated "${BASE1}" 'objc2-metal v0.3.0 (*)' 'objc2-metal feature "MTLRenderPass" (command-line)' 'objc2-metal feature "MTLTexture" (*)'
mk_tree cm_f_fold "${BASE1}" "objc2_metal v0.3.0" 'OBJC2-Metal feature "MTLRenderPipeline"'
mk_tree cm_f_case "${BASE1}" "objc2-metal v0.3.0" 'objc2-metal feature "mtldrawable"' 'objc2-metal feature "MTLDrawableX"'
mk_tree cm_f_other_pkg "${BASE1}" "objc2-metal v0.3.0" "other-dummy v1.0.0" 'other-dummy feature "MTLDrawable"'
mk_tree cm_f_dup "${BASE1}" "objc2-metal v0.3.0" 'objc2-metal feature "MTLDrawable"' 'objc2-metal feature "MTLDrawable"'
mk_tree cm_f_bad "${BASE1}" 'objc2-metal feature MTLDrawable'
mk_tree cm_f_nopkg 'objc2-metal feature "MTLDrawable"'
mk_tree cm_f_nocheck "${BASE2}" 'objc2-metal feature "MTLDrawable"'

run_tree --tree-text "${work}/cm_plain.tree" --profile compute-only-metal --features-text "${work}/cm_f_none.tree"
judge "cm_ok" 0 "o:ok: " "o:scanned: profile=compute-only-metal crates=2" \
  "o:record: commit=- profile=compute-only-metal crates_total=2 judged_total=0 header_crate_n=0 features_judged_total=0 features_recorded_total=0" \
  "!violation" "!FAILED" "!error:" "!${work}"
count_is "cm_ok_crate_counts" "${OUT_F}" "count: profile=compute-only-metal crate=" 3
count_is "cm_ok_feature_counts" "${OUT_F}" "count: profile=compute-only-metal feature=" 8
count_is "cm_ok_feature_judged" "${OUT_F}" " judged=yes n=0" 5
count_is "cm_ok_feature_recorded" "${OUT_F}" " judged=no n=0" 3

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_none.tree"
judge "cm_header_crate_is_not_judged" 0 "o:ok: " "o:header_crate_n=1" "!violation"

run_tree --tree-text "${work}/cm_plain_winit.tree" --profile compute-only-metal --features-text "${work}/cm_f_none.tree"
judge "cm_winit" 1 "e:violation: profile=compute-only-metal crate=winit version=0.30.0" "e:FAILED: 1 judged crate(s) found" "!ok: "

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_judged.tree"
judge "cm_feature_judged" 1 "e:violation: profile=compute-only-metal crate=objc2-metal feature=MTLDrawable" \
  "e:FAILED: 1 judged feature(s) found" "o:feature=MTLDrawable judged=yes n=1" "o:features_judged_total=1" "!ok: " "!error:"

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_recorded.tree"
judge "cm_feature_recorded_only" 0 "o:ok: " "o:feature=MTLPixelFormat judged=no n=1" "o:features_recorded_total=1" "!violation" "!FAILED"

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_decorated.tree"
judge "cm_feature_decorated" 1 "e:FAILED: 2 judged feature(s) found" "o:feature=MTLRenderPass judged=yes n=1" "o:feature=MTLTexture judged=yes n=1"

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_fold.tree"
judge "cm_feature_package_fold" 1 "e:violation: profile=compute-only-metal crate=objc2-metal feature=MTLRenderPipeline" "o:header_crate_n=1"

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_case.tree"
judge "cm_feature_case_exact" 0 "o:ok: " "o:features_judged_total=0" "!violation"

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_other_pkg.tree"
judge "cm_feature_other_package" 0 "o:ok: " "o:features_judged_total=0" "!violation"

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_dup.tree"
judge "cm_feature_dedup" 1 "o:feature=MTLDrawable judged=yes n=1" "e:FAILED: 1 judged feature(s) found"

run_tree --tree-text "${work}/cm_plain.tree" --profile compute-only-metal --features-text "${work}/cm_f_none.tree" --extra-judged-crate fandhe-silicon-core
judge "extra_compute_only" 1 "e:violation: profile=compute-only-metal crate=fandhe-silicon-core version=0.0.0" \
  "o:count: profile=compute-only-metal crate=fandhe-silicon-core n=1" "o:features_recorded_total=0 extra_judged_crate=fandhe-silicon-core" "!ok: "

run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_bad.tree"
judge "cm_feature_unreadable" 2 "e:error: unreadable cargo tree line" "!ok: " "!record: "
run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_nopkg.tree"
judge "cm_feature_no_package" 2 "e:error: no crates found" "!ok: " "!record: "
run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/cm_f_nocheck.tree"
judge "cm_feature_checked_crate_missing" 2 "e:error: the checked crate is missing" "!ok: " "!record: "
run_tree --tree-text "${work}/cm_plain_header.tree" --profile compute-only-metal --features-text "${work}/missing.tree"
judge "cm_feature_missing_file" 2 "e:error: cannot read input file" "!ok: " "!record: " "!${work}"

run_tree --tree-text "${work}/cm_plain.tree" --profile compute-only-metal
judge "misuse_compute_only_without_features_text" 2 "e:error: --features-text is required" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --features-text "${work}/cm_f_none.tree"
judge "misuse_cpu_only_with_features_text" 2 "e:error: --features-text is required" "!ok: " "!record: "
run_tree --features-text "${work}/cm_f_none.tree"
judge "misuse_features_text_only" 2 "e:error: --features-text needs" "!ok: " "!record: "

run_tree --tree-text "${work}/ok_basic.tree"
judge "misuse_tree_only" 2 "e:--tree-text and --profile must be given together" "!ok: " "!record: "
run_tree --profile cpu-only
judge "misuse_profile_only" 2 "e:--tree-text and --profile must be given together" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile unknown-profile
judge "misuse_unknown_profile" 2 "e:error: unknown profile" "!ok: " "!record: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --bogus
judge "misuse_unknown_arg" 2 "!ok: " "!record: "
run_tree --tree-text "${work}/missing.tree" --profile cpu-only
judge "misuse_missing_file" 2 "e:error: cannot read input file" "!ok: " "!record: " "!${work}"

# ---- 偽の cargo / git（引数なしの経路。PATH の先頭に置く）----
fakebin="${work}/fakebin"
mkdir -p "${fakebin}"
cat >"${fakebin}/cargo" <<'FAKE'
#!/usr/bin/env bash
if [ "${FAKE_CARGO_MODE:-ok}" = "fail" ]; then
  exit 1
fi
if [ -n "${FAKE_ARGS_LOG:-}" ]; then
  echo "$*" >>"${FAKE_ARGS_LOG}"
fi
case "$1" in
  --version) echo "cargo 0.0.0 (fake)" ;;
  tree)
    # features の引数（--features contract-core,...）はどの呼び出しにも入るので、辺の種類と -i で振り分ける
    case "$*" in
      *" -i "*) cat "${FAKE_INV_TREE:-${FAKE_TREE}}" ;;
      *"-e normal,build,dev,features"*) cat "${FAKE_FEATURES_TREE:-${FAKE_TREE}}" ;;
      *) cat "${FAKE_TREE}" ;;
    esac
    ;;
  *) exit 1 ;;
esac
FAKE
cat >"${fakebin}/git" <<'FAKE'
#!/usr/bin/env bash
case "${FAKE_GIT_MODE:-ok}" in
  fail) exit 1 ;;
  garbage)
    echo "/secret/host/path/not-a-sha"
    exit 0
    ;;
esac
case "$1" in
  rev-parse) echo "0123456789abcdef0123456789abcdef01234567" ;;
  status)
    if [ "${FAKE_GIT_MODE:-ok}" = "dirty" ]; then
      echo " M tracked.rs"
    fi
    ;;
  *) exit 1 ;;
esac
FAKE
chmod +x "${fakebin}/cargo" "${fakebin}/git"

# run_fake <cargo モード> <git モード> <tree>
run_fake() {
  rc=0
  PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE="$1" FAKE_GIT_MODE="$2" FAKE_TREE="$3" \
    "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

args_log="${work}/fake-args.log"
rc=0
PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE=ok FAKE_GIT_MODE=ok FAKE_ARGS_LOG="${args_log}" FAKE_TREE="${work}/ok_basic.tree" \
  "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "fake_ok" 0 "o:record: commit=0123456789abcdef0123456789abcdef01234567 profile=cpu-only crates_total=2 " "o:ok: " "!-dirty"
count_is "fake_tree_args" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops -e normal,build,dev --target all --prefix none" 1
count_is "fake_tree_args_compute_only" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-metal -e normal,build,dev --target all --prefix none" 1
count_is "fake_features_args" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-metal -e normal,build,dev,features --target all --prefix none" 1
count_is "fake_no_inverse_without_header_crate" "${args_log}" " -i " 0

# objc2-metal が載る tree では版ごとに逆向きの tree を 1 回流し、逆向きにだけある判定の feature を数える
mk_tree fake_with_header "${BASE1}" "objc2-metal v0.0.1"
mk_tree fake_inv_judged "${BASE1}" "objc2-metal v0.0.1" 'objc2-metal feature "MTLRenderPass"'
inv_log="${work}/fake-inv-args.log"
rc=0
PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE=ok FAKE_GIT_MODE=ok FAKE_ARGS_LOG="${inv_log}" FAKE_TREE="${work}/fake_with_header.tree" \
  FAKE_FEATURES_TREE="${work}/fake_with_header.tree" FAKE_INV_TREE="${work}/fake_inv_judged.tree" \
  "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "fake_inverse_feature" 1 "e:violation: profile=compute-only-metal crate=objc2-metal feature=MTLRenderPass" \
  "e:FAILED: 1 judged feature(s) found" "o:header_crate_n=1" "!${work}"
count_is "fake_inverse_args" "${inv_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-metal -e normal,build,dev,features -i objc2-metal@0.0.1 --target all --prefix none" 1
# --extra-judged-crate は cargo の引数を変えない（名前は判定の比較にだけ使い、cargo へ渡さない）
extra_log="${work}/fake-extra-args.log"
rc=0
PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE=ok FAKE_GIT_MODE=ok FAKE_ARGS_LOG="${extra_log}" FAKE_TREE="${work}/ok_basic.tree" \
  "${CHECK}" --extra-judged-crate fandhe-silicon-core >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "fake_extra" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core" "e:violation: profile=compute-only-metal crate=fandhe-silicon-core" \
  "o:extra_judged_crate=fandhe-silicon-core" "!ok: "
if cmp -s <(grep -v '^$' "${args_log}") <(grep -v '^$' "${extra_log}"); then
  report_ok "fake_extra_cargo_args_unchanged"
else
  report_fail "fake_extra_cargo_args_unchanged" "cargo arguments differ when --extra-judged-crate is given"
fi
count_is "fake_extra_name_not_passed_to_cargo" "${extra_log}" "fandhe-silicon-core" 0
run_fake ok dirty "${work}/ok_basic.tree"
judge "fake_dirty" 0 "o:commit=0123456789abcdef0123456789abcdef01234567-dirty " "o:ok: "
run_fake ok ok "${work}/bad_ash.tree"
judge "fake_violation" 1 "e:violation: profile=cpu-only crate=ash" "!ok: "
run_fake fail ok "${work}/ok_basic.tree"
judge "fake_cargo_fails" 2 "e:error: cargo --version failed" "!ok: " "!record: "
run_fake ok fail "${work}/ok_basic.tree"
judge "fake_git_fails" 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"
run_fake ok garbage "${work}/ok_basic.tree"
judge "fake_git_garbage" 2 "e:error: cannot resolve the source commit" "!/secret/host/path" "!ok: " "!record: "
run_fake ok ok "${work}/err_empty.tree"
judge "fake_empty_tree" 2 "e:error: no crates found" "!ok: " "!record: "

# python3 が無い環境（PATH を空のディレクトリだけにする）
nopy="${work}/nopy"
mkdir -p "${nopy}"
ln -s "$(command -v bash)" "${nopy}/bash"
for t in dirname env; do
  ln -s "$(command -v "${t}")" "${nopy}/${t}"
done
rc=0
PATH="${nopy}" "${nopy}/bash" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_python3" 2 "!ok: " "!record: "

# ---- 実物（一時 workspace の実物の cargo tree）----
rw="${work}/real"
mkdir -p "${rw}/ws/crates/contract/src" "${rw}/ws/crates/core/src" "${rw}/ws/scripts" "${rw}/ext"
cat >"${rw}/ws/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/contract", "crates/core"]
resolver = "2"
TOML
printf '%s\n' '// fixture' >"${rw}/ws/crates/contract/src/lib.rs"
printf '%s\n' '// fixture' >"${rw}/ws/crates/core/src/lib.rs"
printf '[package]\nname = "fandhe-silicon-core"\nversion = "0.0.0"\nedition = "2021"\n' >"${rw}/ws/crates/core/Cargo.toml"
cp "${DIR}/profile-dep-graph-check.sh" "${DIR}/lib.sh" "${rw}/ws/scripts/"
chmod +x "${rw}/ws/scripts/profile-dep-graph-check.sh"
if [ -f "${DIR}/../rust-toolchain.toml" ]; then
  cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi
for n in ash winit; do
  mkdir -p "${rw}/ext/${n}/src"
  printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2021"\n' "${n}" >"${rw}/ext/${n}/Cargo.toml"
  printf '%s\n' '// fixture' >"${rw}/ext/${n}/src/lib.rs"
done

# 8 個の per-header feature と無害な feature 1 個を宣言する合成の objc2-metal
mkdir -p "${rw}/ext/objc2-metal/src"
cat >"${rw}/ext/objc2-metal/Cargo.toml" <<'TOML'
[package]
name = "objc2-metal"
version = "0.1.0"
edition = "2021"

[features]
MTLRenderPipeline = []
MTLRenderCommandEncoder = []
MTLRenderPass = []
MTLTexture = []
MTLDrawable = []
MTLDepthStencil = []
MTLBlitCommandEncoder = []
MTLPixelFormat = []
MTLDevice = []
TOML
printf '%s\n' '// fixture' >"${rw}/ext/objc2-metal/src/lib.rs"

real_git() {
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
    -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# real_contract <契約の crate の Cargo.toml の後半>: 書き換え、ロックを作り直してコミットする。
real_contract() {
  {
    printf '[package]\nname = "fandhe-silicon-contract"\nversion = "0.0.0"\nedition = "2021"\n\n'
    printf '[dependencies]\nfandhe-silicon-core = { path = "../core" }\n\n'
    printf '%s\n' "$1"
  } >"${rw}/ws/crates/contract/Cargo.toml"
  (cd "${rw}/ws" && cargo generate-lockfile --offline) >/dev/null 2>"${ERR_F}" \
    && real_git add -A >/dev/null 2>>"${ERR_F}" \
    && real_git commit -q -m fixture >/dev/null 2>>"${ERR_F}"
}
run_real() {
  rc=0
  (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR ./scripts/profile-dep-graph-check.sh "$@") \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

BASE_FEATURES='ash = { path = "../../../ext/ash", optional = true }
winit = { path = "../../../ext/winit", optional = true }

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-vulkan = ["dep:ash"]
backend-metal = []
graphics = []'
if real_git init -q >/dev/null 2>"${ERR_F}" && real_contract "${BASE_FEATURES}"; then
  # (a) 選ばない feature の裏の依存は数えない
  run_real
  judge "real_clean" 0 "o:ok: " "o:scanned: profile=cpu-only crates=2" "o:scanned: profile=compute-only-metal crates=2" "o:header_crate_n=0" "o:judged_total=0" "!violation" "!error:" "!${rw}"
  if grep -Eq '^record: commit=[0-9a-f]{40,64} ' "${OUT_F}"; then
    report_ok "real_clean_commit_is_clean_sha"
  else
    report_fail "real_clean_commit_is_clean_sha" "commit must be a full hex sha without -dirty"
  fi

  # (a2) 実物の tree に載っている名前を 1 件足すと、両プロファイルで違反になる（検査が空振りしていない）
  run_real --extra-judged-crate fandhe-silicon-core
  judge "real_extra_fails" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core version=0.0.0" \
    "e:violation: profile=compute-only-metal crate=fandhe-silicon-core version=0.0.0" "e:FAILED: 2 judged crate(s) found" \
    "o:crate=fandhe-silicon-core n=1" "o:extra_judged_crate=fandhe-silicon-core" "!ok: " "!error:" "!${rw}"

  # (b) 選んだ feature の裏の依存は数える
  real_contract "${BASE_FEATURES/cpu-ops = []/cpu-ops = [\"dep:ash\"]}" || report_fail "real_feature_setup" "cannot prepare"
  run_real
  judge "real_feature_selected" 1 "e:violation: profile=cpu-only crate=ash version=0.1.0" "e:FAILED: 1 judged crate(s) found" "!ok: " "!${rw}"

  # (c) 別 target に限った依存も数える（--target all）
  real_contract "[target.'cfg(target_os = \"windows\")'.dependencies]
winit = { path = \"../../../ext/winit\" }

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = []
graphics = []" || report_fail "real_target_setup" "cannot prepare"
  run_real
  judge "real_other_target" 1 "e:violation: profile=cpu-only crate=winit version=0.1.0" "e:violation: profile=compute-only-metal crate=winit version=0.1.0" "e:FAILED: 2 judged crate(s) found" "!ok: " "!${rw}"

  # (d) dev-dependencies の判定の名前も数える
  real_contract "[dev-dependencies]
ash = { path = \"../../../ext/ash\" }

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = []
graphics = []" || report_fail "real_dev_setup" "cannot prepare"
  run_real
  judge "real_dev_dep" 1 "e:violation: profile=cpu-only crate=ash" "!ok: " "!${rw}"

  # (e)〜(i) の契約の crate の後半。objc2-metal の依存の宣言と feature の表を引数で差し替える
  METAL_COMMON='[features]
contract-core = []
cpu-isa = []
cpu-ops = []
graphics = []'

  # (e) objc2-metal が入っていても、無害な feature だけなら 0 件（SDK の有無はこの検査の判定ではない）
  real_contract "[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false
features = [\"MTLDevice\"]

${METAL_COMMON}
backend-metal = [\"dep:objc2-metal\"]" || report_fail "real_metal_setup_e" "cannot prepare"
  run_real
  judge "real_metal_harmless" 0 "o:ok: " "o:profile=compute-only-metal" "o:header_crate_n=1" "o:features_judged_total=0" "!violation" "!error:" "!${rw}"

  # (f) graphics だけが有効にする per-header feature は、graphics がプロファイルに入っていないので数えない
  real_contract "[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = [\"dep:objc2-metal\"]
graphics = [\"objc2-metal?/MTLRenderPass\"]" || report_fail "real_metal_setup_f" "cannot prepare"
  run_real
  judge "real_metal_graphics_not_selected" 0 "o:ok: " "o:header_crate_n=1" "o:features_judged_total=0" "!violation" "!${rw}"

  # (g) 契約の crate 自身の feature が有効にする per-header feature（順方向の tree には出ない経路。逆向きの tree で拾う）
  real_contract "[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false

${METAL_COMMON}
backend-metal = [\"dep:objc2-metal\", \"objc2-metal/MTLTexture\"]" || report_fail "real_metal_setup_g" "cannot prepare"
  run_real
  judge "real_metal_own_feature" 1 "e:violation: profile=compute-only-metal crate=objc2-metal feature=MTLTexture" \
    "e:FAILED: 1 judged feature(s) found" "o:feature=MTLTexture judged=yes n=1" "!ok: " "!${rw}"

  # (h) 依存の宣言の features に書いた per-header feature
  real_contract "[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false
features = [\"MTLDrawable\"]

${METAL_COMMON}
backend-metal = [\"dep:objc2-metal\"]" || report_fail "real_metal_setup_h" "cannot prepare"
  run_real
  judge "real_metal_declared_feature" 1 "e:violation: profile=compute-only-metal crate=objc2-metal feature=MTLDrawable" \
    "e:FAILED: 1 judged feature(s) found" "!ok: " "!${rw}"

  # (i) 判定に使わない per-header feature は数えるだけで落とさない
  real_contract "[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false

${METAL_COMMON}
backend-metal = [\"dep:objc2-metal\", \"objc2-metal/MTLPixelFormat\"]" || report_fail "real_metal_setup_i" "cannot prepare"
  run_real
  judge "real_metal_record_only_feature" 0 "o:ok: " "o:feature=MTLPixelFormat judged=no n=1" "o:features_recorded_total=1" "o:features_judged_total=0" "!violation" "!${rw}"
else
  report_fail "real_fixture_setup" "cannot prepare the temporary workspace"
fi

# ---- 実物のリポジトリ（引数なしの本物の検査）----
rc=0
"${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "repo_real" 0 "o:ok: " "o:record: commit=" "o:profile=cpu-only" "o:profile=compute-only-metal" "o:header_crate_n=0" "!violation" "!error:" "!${ROOT_DIR}"

rc=0
"${CHECK}" --extra-judged-crate fandhe-silicon-core >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "repo_real_extra_fails" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core" \
  "e:violation: profile=compute-only-metal crate=fandhe-silicon-core" "e:FAILED: 2 judged crate(s) found" \
  "o:extra_judged_crate=fandhe-silicon-core" "!ok: " "!error:" "!${ROOT_DIR}"

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
