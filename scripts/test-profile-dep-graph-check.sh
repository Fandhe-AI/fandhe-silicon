#!/usr/bin/env bash
# profile-dep-graph-check.sh の回帰テスト（spec TASK-228、TASK-229・BUILD-3〜BUILD-5 のポインタ）。
# 一時ディレクトリの合成 cargo tree のテキスト（ダミーの名前）を --tree-text / --profile で渡し、
# 終了コード・標準出力・標準エラーの文言を確かめる。合成の節は cargo を呼ばない。
# Graphics（Vulkan）の組は SDK のうち ash・libloading だけを許し、それ以外の SDK と描画・surface の名前を数える。
# Graphics と相互運用の組は SDK のうち ash・cudarc・libloading だけを許し、それ以外の SDK と描画・surface の名前を数える。
# ComputeOnly（Metal）の組は SDK のうち objc2-metal だけを許し、features の辺を含む tree（--features-text）も渡して per-header feature を数える。
# 比較基準の単一ビルド（--all-features）は判定せず、baseline:・baseline-count:・diff: の行で記録する。
# 5/6: プロファイルごとの許す SDK と判定の名前の表（テスト側に独立に持つ）、比較基準の端のケース、全 backend の一時 workspace を網羅する。
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
count_is "cm_ok_crate_counts" "${OUT_F}" "count: profile=compute-only-metal crate=" 6
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

# ---- 合成の節（graphics-vulkan: SDK は ash・libloading だけを許す）----
mk_tree gv_ok "${BASE1}" "${BASE2}" "ash v0.38.0" "libloading v0.8.1"
mk_tree gv_metal "${BASE1}" "objc2-metal v0.3.0"
mk_tree gv_cudarc "${BASE1}" "cudarc v0.12.0"
mk_tree gv_surface "${BASE1}" "raw_window_handle v0.6.2"
mk_tree gv_prefix "${BASE1}" "cudarc-x v1.0.0"

run_tree --tree-text "${work}/gv_ok.tree" --profile graphics-vulkan
judge "gv_ok" 0 "o:ok: " "o:scanned: profile=graphics-vulkan crates=4" \
  "o:record: commit=- profile=graphics-vulkan crates_total=4 judged_total=0" "!violation" "!FAILED" "!error:" "!header_crate_n" "!${work}"
count_is "gv_ok_counts" "${OUT_F}" "count: profile=graphics-vulkan crate=" 5
count_is "gv_ok_counts_zero" "${OUT_F}" " n=0" 5
for n in objc2-metal cudarc objc2-quartz-core raw-window-handle winit; do
  count_is "gv_ok_count_${n}" "${OUT_F}" "count: profile=graphics-vulkan crate=${n} n=0" 1
done
count_is "gv_ok_ash_not_judged" "${OUT_F}" "crate=ash" 0
count_is "gv_ok_libloading_not_judged" "${OUT_F}" "crate=libloading" 0

run_tree --tree-text "${work}/gv_metal.tree" --profile graphics-vulkan
judge "gv_metal" 1 "e:violation: profile=graphics-vulkan crate=objc2-metal version=0.3.0" "!ok: "
run_tree --tree-text "${work}/gv_cudarc.tree" --profile graphics-vulkan
judge "gv_cudarc" 1 "e:violation: profile=graphics-vulkan crate=cudarc version=0.12.0" "!ok: "
run_tree --tree-text "${work}/gv_surface.tree" --profile graphics-vulkan
judge "gv_surface_folded" 1 "e:violation: profile=graphics-vulkan crate=raw-window-handle" "!ok: "
run_tree --tree-text "${work}/gv_prefix.tree" --profile graphics-vulkan
judge "gv_prefix_only" 0 "o:ok: " "o:judged_total=0" "!violation"

run_tree --tree-text "${work}/gv_ok.tree" --profile graphics-vulkan --extra-judged-crate fandhe-silicon-core
judge "gv_extra" 1 "e:violation: profile=graphics-vulkan crate=fandhe-silicon-core version=0.0.0" "o:extra_judged_crate=fandhe-silicon-core" "!ok: "
run_tree --tree-text "${work}/gv_ok.tree" --profile graphics-vulkan --extra-judged-crate cudarc
judge "gv_extra_already_judged" 2 "e:error: --extra-judged-crate is already judged" "!ok: " "!record: "
run_tree --tree-text "${work}/gv_ok.tree" --profile graphics-vulkan --features-text "${work}/cm_f_none.tree"
judge "gv_misuse_with_features_text" 2 "e:error: --features-text is required" "!ok: " "!record: "

# ---- 合成の節（graphics-interop: SDK は ash・cudarc・libloading だけを許す）----
mk_tree gi_ok "${BASE1}" "${BASE2}" "ash v0.38.0" "cudarc v0.12.0" "libloading v0.8.1"
mk_tree gi_metal "${BASE1}" "objc2-metal v0.3.0"
mk_tree gi_surface "${BASE1}" "objc2_quartz_core v0.2.0"
mk_tree gi_prefix "${BASE1}" "objc2-metal-x v1.0.0"

run_tree --tree-text "${work}/gi_ok.tree" --profile graphics-interop
judge "gi_ok" 0 "o:ok: " "o:scanned: profile=graphics-interop crates=5" \
  "o:record: commit=- profile=graphics-interop crates_total=5 judged_total=0" "!violation" "!FAILED" "!error:" "!header_crate_n" "!${work}"
count_is "gi_ok_counts" "${OUT_F}" "count: profile=graphics-interop crate=" 4
count_is "gi_ok_counts_zero" "${OUT_F}" " n=0" 4
for n in objc2-metal objc2-quartz-core raw-window-handle winit; do
  count_is "gi_ok_count_${n}" "${OUT_F}" "count: profile=graphics-interop crate=${n} n=0" 1
done
count_is "gi_ok_ash_not_judged" "${OUT_F}" "crate=ash" 0
count_is "gi_ok_cudarc_not_judged" "${OUT_F}" "crate=cudarc" 0
count_is "gi_ok_libloading_not_judged" "${OUT_F}" "crate=libloading" 0

run_tree --tree-text "${work}/gv_cudarc.tree" --profile graphics-interop
judge "gi_cudarc_allowed" 0 "o:ok: " "o:judged_total=0" "!violation"
run_tree --tree-text "${work}/gi_metal.tree" --profile graphics-interop
judge "gi_metal" 1 "e:violation: profile=graphics-interop crate=objc2-metal version=0.3.0" "!ok: "
run_tree --tree-text "${work}/gi_surface.tree" --profile graphics-interop
judge "gi_surface_folded" 1 "e:violation: profile=graphics-interop crate=objc2-quartz-core" "!ok: "
run_tree --tree-text "${work}/gi_prefix.tree" --profile graphics-interop
judge "gi_prefix_only" 0 "o:ok: " "o:judged_total=0" "!violation"

run_tree --tree-text "${work}/gi_ok.tree" --profile graphics-interop --extra-judged-crate fandhe-silicon-core
judge "gi_extra" 1 "e:violation: profile=graphics-interop crate=fandhe-silicon-core version=0.0.0" "o:extra_judged_crate=fandhe-silicon-core" "!ok: "
run_tree --tree-text "${work}/gi_ok.tree" --profile graphics-interop --extra-judged-crate objc2-metal
judge "gi_extra_already_judged" 2 "e:error: --extra-judged-crate is already judged" "!ok: " "!record: "
run_tree --tree-text "${work}/gi_ok.tree" --profile graphics-interop --features-text "${work}/cm_f_none.tree"
judge "gi_misuse_with_features_text" 2 "e:error: --features-text is required" "!ok: " "!record: "

# ---- 合成の節（compute-only-metal: 選んでいない SDK）----
mk_tree cm_ash "${BASE1}" "ash v0.38.0"
mk_tree cm_cudarc "${BASE1}" "cudarc v0.12.0"
mk_tree cm_libloading "${BASE1}" "libloading v0.8.1"
mk_tree cm_metal_ok "${BASE1}" "objc2-metal v0.3.0"
for n in ash cudarc libloading; do
  run_tree --tree-text "${work}/cm_${n}.tree" --profile compute-only-metal --features-text "${work}/cm_f_none.tree"
  judge "cm_${n}" 1 "e:violation: profile=compute-only-metal crate=${n} version=" "e:FAILED: 1 judged crate(s) found" "!ok: "
done
run_tree --tree-text "${work}/cm_metal_ok.tree" --profile compute-only-metal --features-text "${work}/cm_f_none.tree"
judge "cm_metal_ok" 0 "o:ok: " "o:judged_total=0" "!violation"

# ---- 合成の節（比較基準 --all-features: 判定せず記録する）----
mk_tree bl_three "${BASE1}" "objc2-metal v0.3.0" "ash v0.38.0" "cudarc v0.12.0" "libloading v0.8.1"
mk_tree bl_none "${BASE1}" "${BASE2}"
mk_tree bl_empty
mk_tree bl_no_package "${BASE2}"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_three.tree"
judge "bl_three" 0 "o:ok: " \
  "o:baseline: commit=- profile=all-features crates_total=5 sdk_total=3" \
  "o:baseline-count: profile=all-features crate=objc2-metal n=1" "o:baseline-count: profile=all-features crate=ash n=1" \
  "o:baseline-count: profile=all-features crate=cudarc n=1" \
  "o:diff: profile=cpu-only baseline=all-features sdk_baseline=3 sdk_profile=0 diff=3" "!violation" "!error:" "!${work}"
count_is "bl_three_count_lines" "${OUT_F}" "baseline-count: " 3
count_is "bl_three_diff_lines" "${OUT_F}" "diff: " 1
run_tree --tree-text "${work}/cm_metal_ok.tree" --profile compute-only-metal --features-text "${work}/cm_f_none.tree" --baseline-text "${work}/bl_three.tree"
judge "bl_diff_compute_only" 0 "o:diff: profile=compute-only-metal baseline=all-features sdk_baseline=3 sdk_profile=1 diff=2"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_none.tree"
judge "bl_none" 0 "o:baseline: commit=- profile=all-features crates_total=2 sdk_total=0" "o:diff: profile=cpu-only baseline=all-features sdk_baseline=0 sdk_profile=0 diff=0"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_three.tree" --extra-judged-crate fandhe-silicon-core
judge "bl_extra_not_in_baseline" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core" "o:baseline-count: profile=all-features crate=ash n=1" "!baseline-count: profile=all-features crate=fandhe-silicon-core"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only
judge "bl_absent_without_flag" 0 "o:ok: " "!baseline" "!diff:"
run_tree --baseline-text "${work}/bl_three.tree"
judge "bl_misuse_alone" 2 "e:error: --baseline-text needs" "!ok: " "!baseline:"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_empty.tree"
judge "bl_empty" 2 "e:error: no crates found" "!ok: " "!baseline:"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_no_package.tree"
judge "bl_no_package" 2 "e:error: the checked crate is missing" "!ok: "
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/nonexistent.tree"
judge "bl_unreadable" 2 "e:error: cannot read input file" "!ok: " "!${work}"

# ---- 合成の節（プロファイルと判定の名前の表。TASK-229・BUILD-3〜BUILD-5 のポインタ）----
# 期待の表はテスト側に独立に持つ（検査の定数から導かない）。定義のブロックが黙って変わったときにも落とすため。
allowed_for() { # allowed_for <プロファイル>: そのプロファイルが許す SDK・ローダの名前
  case "$1" in
    cpu-only) echo "" ;;
    compute-only-metal) echo "objc2-metal" ;;
    graphics-vulkan) echo "ash libloading" ;;
    graphics-interop) echo "ash cudarc libloading" ;;
  esac
}
want_name_lines() { # want_name_lines <プロファイル>: count: の行数（7 つの名前から許す数を引いた値）
  case "$1" in
    cpu-only) echo 7 ;;
    compute-only-metal) echo 6 ;;
    graphics-vulkan) echo 5 ;;
    graphics-interop) echo 4 ;;
  esac
}
want_all_sdk() { # want_all_sdk <プロファイル>: 4 つの SDK・ローダを載せたときの判定数
  case "$1" in
    cpu-only) echo 4 ;;
    compute-only-metal) echo 3 ;;
    graphics-vulkan) echo 2 ;;
    graphics-interop) echo 1 ;;
  esac
}
ALL_NAMES="objc2-metal ash cudarc libloading objc2-quartz-core raw-window-handle winit"
mk_tree all_sdk "${BASE1}" "${BASE2}" "objc2-metal v9.9.9" "ash v9.9.9" "cudarc v9.9.9" "libloading v9.9.9"
for n in ${ALL_NAMES}; do
  mk_tree "matrix_${n}" "${BASE1}" "${BASE2}" "${n} v9.9.9"
done
# run_profile <プロファイル> <tree>: compute-only-metal だけ features の tree を足して流す。
run_profile() {
  if [ "$1" = "compute-only-metal" ]; then
    run_tree --tree-text "$2" --profile "$1" --features-text "${work}/cm_f_none.tree"
  else
    run_tree --tree-text "$2" --profile "$1"
  fi
}
for p in cpu-only compute-only-metal graphics-vulkan graphics-interop; do
  allowed=" $(allowed_for "${p}") "
  for n in ${ALL_NAMES}; do
    run_profile "${p}" "${work}/matrix_${n}.tree"
    case "${allowed}" in
      *" ${n} "*)
        judge "matrix_${p}_${n}" 0 "o:ok: " "o:judged_total=0" "!violation" "!FAILED" "!${work}"
        count_is "matrix_${p}_${n}_no_count" "${OUT_F}" "count: profile=${p} crate=${n} " 0
        ;;
      *)
        judge "matrix_${p}_${n}" 1 "e:violation: profile=${p} crate=${n} version=9.9.9" "e:FAILED: 1 judged crate(s) found" \
          "o:count: profile=${p} crate=${n} n=1" "o:judged_total=1" "!ok: " "!${work}"
        ;;
    esac
  done
  run_profile "${p}" "${work}/ok_basic.tree"
  count_is "matrix_${p}_count_lines" "${OUT_F}" "count: profile=${p} crate=" "$(want_name_lines "${p}")"
  run_profile "${p}" "${work}/all_sdk.tree"
  k="$(want_all_sdk "${p}")"
  judge "matrix_${p}_all_sdk" 1 "e:FAILED: ${k} judged crate(s) found" "o:judged_total=${k}" "!ok: " "!${work}"
  count_is "matrix_${p}_all_sdk_violation_lines" "${ERR_F}" "violation: profile=${p} crate=" "${k}"
  for n in objc2-metal ash cudarc libloading; do
    case "${allowed}" in
      *" ${n} "*) count_is "matrix_${p}_all_sdk_allowed_${n}" "${ERR_F}" "violation: profile=${p} crate=${n} " 0 ;;
      *) count_is "matrix_${p}_all_sdk_judged_${n}" "${ERR_F}" "violation: profile=${p} crate=${n} version=9.9.9" 1 ;;
    esac
  done
done

# ---- 合成の節（比較基準の端のケース）----
mk_tree bl_two_versions "${BASE1}" "ash v0.37.0" "ash v0.38.0"
mk_tree bl_partial "${BASE1}" "objc2-metal v0.3.0"
mk_tree bl_not_sdk "${BASE1}" "libloading v0.8.1" "winit v0.30.0" "raw_window_handle v0.6.2"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_two_versions.tree"
judge "bl_two_versions" 0 "o:ok: " "o:baseline-count: profile=all-features crate=ash n=2" "o:baseline-count: profile=all-features crate=objc2-metal n=0" \
  "o:baseline-count: profile=all-features crate=cudarc n=0" "o:sdk_total=2" "o:diff: profile=cpu-only baseline=all-features sdk_baseline=2 sdk_profile=0 diff=2" "!violation"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_partial.tree"
judge "bl_partial" 0 "o:sdk_total=1" "o:baseline-count: profile=all-features crate=objc2-metal n=1" \
  "o:baseline-count: profile=all-features crate=cudarc n=0" "o:baseline-count: profile=all-features crate=ash n=0" \
  "o:diff: profile=cpu-only baseline=all-features sdk_baseline=1 sdk_profile=0 diff=1" "!violation"
run_tree --tree-text "${work}/ok_basic.tree" --profile cpu-only --baseline-text "${work}/bl_not_sdk.tree"
judge "bl_not_sdk" 0 "o:sdk_total=0" "o:baseline-count: profile=all-features crate=objc2-metal n=0" \
  "o:diff: profile=cpu-only baseline=all-features sdk_baseline=0 sdk_profile=0 diff=0" "!violation"
count_is "bl_not_sdk_count_lines" "${OUT_F}" "baseline-count: " 3
count_is "bl_not_sdk_no_loader_count" "${OUT_F}" "baseline-count: profile=all-features crate=libloading" 0
count_is "bl_not_sdk_no_surface_count" "${OUT_F}" "baseline-count: profile=all-features crate=winit" 0
run_tree --tree-text "${work}/gv_ok.tree" --profile graphics-vulkan --baseline-text "${work}/bl_three.tree"
judge "bl_diff_graphics_vulkan" 0 "o:diff: profile=graphics-vulkan baseline=all-features sdk_baseline=3 sdk_profile=1 diff=2" "!violation"
run_tree --tree-text "${work}/gi_ok.tree" --profile graphics-interop --baseline-text "${work}/bl_three.tree"
judge "bl_diff_graphics_interop" 0 "o:diff: profile=graphics-interop baseline=all-features sdk_baseline=3 sdk_profile=2 diff=1" "!violation"
run_tree --tree-text "${work}/bad_ash.tree" --profile cpu-only --baseline-text "${work}/bl_three.tree"
judge "bl_violation_still_records" 1 "e:violation: profile=cpu-only crate=ash version=0.38.0" "e:FAILED: 1 judged crate(s) found" \
  "o:baseline: commit=- profile=all-features crates_total=5 sdk_total=3" "o:diff: profile=cpu-only baseline=all-features sdk_baseline=3 sdk_profile=1 diff=2" "!ok: "

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
judge "fake_baseline_lines" 0 "o:baseline: commit=0123456789abcdef0123456789abcdef01234567 profile=all-features crates_total=2 sdk_total=0" \
  "o:diff: profile=cpu-only baseline=all-features sdk_baseline=0 sdk_profile=0 diff=0" \
  "o:diff: profile=graphics-interop baseline=all-features sdk_baseline=0 sdk_profile=0 diff=0"
count_is "fake_tree_args" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops -e normal,build,dev --target all --prefix none" 1
count_is "fake_tree_args_compute_only" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-metal -e normal,build,dev --target all --prefix none" 1
count_is "fake_tree_args_graphics_vulkan" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics -e normal,build,dev --target all --prefix none" 1
count_is "fake_no_features_edges_graphics_vulkan" "${args_log}" "backend-vulkan,graphics -e normal,build,dev,features" 0
count_is "fake_tree_args_graphics_interop" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-vulkan,backend-cuda,graphics,ext-interop -e normal,build,dev --target all --prefix none" 1
count_is "fake_no_features_edges_graphics_interop" "${args_log}" "ext-interop -e normal,build,dev,features" 0
count_is "fake_features_args" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-metal -e normal,build,dev,features --target all --prefix none" 1
count_is "fake_tree_args_baseline" "${args_log}" \
  "tree --locked -p fandhe-silicon-contract --all-features -e normal,build,dev --target all --prefix none" 1
count_is "fake_no_features_edges_baseline" "${args_log}" "--all-features -e normal,build,dev,features" 0
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
  "e:violation: profile=graphics-vulkan crate=fandhe-silicon-core" "e:violation: profile=graphics-interop crate=fandhe-silicon-core" \
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
judge "fake_violation" 1 "e:violation: profile=cpu-only crate=ash" "!violation: profile=graphics-vulkan" "!violation: profile=graphics-interop" "!ok: "
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
for n in ash winit cudarc libloading; do
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
cudarc = { path = "../../../ext/cudarc", optional = true }

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-vulkan = ["dep:ash"]
backend-cuda = ["dep:cudarc"]
ext-interop = []
backend-metal = []
graphics = []'
if real_git init -q >/dev/null 2>"${ERR_F}" && real_contract "${BASE_FEATURES}"; then
  # (a) 選ばない feature の裏の依存は数えない
  run_real
  judge "real_clean" 0 "o:ok: " "o:scanned: profile=cpu-only crates=2" "o:scanned: profile=compute-only-metal crates=2" "o:scanned: profile=graphics-vulkan crates=3" "o:scanned: profile=graphics-interop crates=4" "o:header_crate_n=0" "o:judged_total=0" "!violation" "!error:" "!${rw}"
  if grep -Eq '^record: commit=[0-9a-f]{40,64} ' "${OUT_F}"; then
    report_ok "real_clean_commit_is_clean_sha"
  else
    report_fail "real_clean_commit_is_clean_sha" "commit must be a full hex sha without -dirty"
  fi

  # (a2) 実物の tree に載っている名前を 1 件足すと、両プロファイルで違反になる（検査が空振りしていない）
  run_real --extra-judged-crate fandhe-silicon-core
  judge "real_extra_fails" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core version=0.0.0" \
    "e:violation: profile=compute-only-metal crate=fandhe-silicon-core version=0.0.0" \
    "e:violation: profile=graphics-vulkan crate=fandhe-silicon-core version=0.0.0" \
    "e:violation: profile=graphics-interop crate=fandhe-silicon-core version=0.0.0" "e:FAILED: 4 judged crate(s) found" \
    "o:crate=fandhe-silicon-core n=1" "o:extra_judged_crate=fandhe-silicon-core" "!ok: " "!error:" "!${rw}"

  # (b) 選んだ feature の裏の依存は数える
  real_contract "${BASE_FEATURES/cpu-ops = []/cpu-ops = [\"dep:ash\"]}" || report_fail "real_feature_setup" "cannot prepare"
  run_real
  judge "real_feature_selected" 1 "e:violation: profile=cpu-only crate=ash version=0.1.0" "e:violation: profile=compute-only-metal crate=ash version=0.1.0" "e:FAILED: 2 judged crate(s) found" "!violation: profile=graphics-vulkan" "!violation: profile=graphics-interop" "!ok: " "!${rw}"

  # (c) 別 target に限った依存も数える（--target all）
  real_contract "[target.'cfg(target_os = \"windows\")'.dependencies]
winit = { path = \"../../../ext/winit\" }

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = []
backend-vulkan = []
backend-cuda = []
ext-interop = []
graphics = []" || report_fail "real_target_setup" "cannot prepare"
  run_real
  judge "real_other_target" 1 "e:violation: profile=cpu-only crate=winit version=0.1.0" "e:violation: profile=compute-only-metal crate=winit version=0.1.0" "e:violation: profile=graphics-vulkan crate=winit version=0.1.0" "e:violation: profile=graphics-interop crate=winit version=0.1.0" "e:FAILED: 4 judged crate(s) found" "!ok: " "!${rw}"

  # (d) dev-dependencies の判定の名前も数える
  real_contract "[dev-dependencies]
ash = { path = \"../../../ext/ash\" }

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = []
backend-vulkan = []
backend-cuda = []
ext-interop = []
graphics = []" || report_fail "real_dev_setup" "cannot prepare"
  run_real
  judge "real_dev_dep" 1 "e:violation: profile=cpu-only crate=ash" "e:violation: profile=compute-only-metal crate=ash" "e:FAILED: 2 judged crate(s) found" "!violation: profile=graphics-vulkan" "!violation: profile=graphics-interop" "!ok: " "!${rw}"

  # (e)〜(i) の契約の crate の後半。objc2-metal の依存の宣言と feature の表を引数で差し替える
  METAL_COMMON='[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-vulkan = []
backend-cuda = []
ext-interop = []
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
backend-vulkan = []
backend-cuda = []
ext-interop = []
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

  # (j) graphics が組に入っている: graphics が引く描画の依存は Graphics の組（graphics-vulkan・graphics-interop）だけで違反になる
  real_contract "ash = { path = \"../../../ext/ash\", optional = true }
winit = { path = \"../../../ext/winit\", optional = true }

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = []
backend-vulkan = [\"dep:ash\"]
backend-cuda = []
ext-interop = []
graphics = [\"dep:winit\"]" || report_fail "real_graphics_setup" "cannot prepare"
  run_real
  judge "real_graphics_selected" 1 "e:violation: profile=graphics-vulkan crate=winit version=0.1.0" "e:violation: profile=graphics-interop crate=winit version=0.1.0" "e:FAILED: 2 judged crate(s) found" \
    "!violation: profile=cpu-only" "!violation: profile=compute-only-metal" "!ok: " "!${rw}"

  # (k) backend-vulkan の裏に判定の SDK（objc2-metal）があれば Graphics の組で違反になる
  real_contract "[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = []
backend-vulkan = [\"dep:objc2-metal\"]
backend-cuda = []
ext-interop = []
graphics = []" || report_fail "real_vulkan_sdk_setup" "cannot prepare"
  run_real
  judge "real_vulkan_judged_sdk" 1 "e:violation: profile=graphics-vulkan crate=objc2-metal version=0.1.0" "e:violation: profile=graphics-interop crate=objc2-metal version=0.1.0" "e:FAILED: 2 judged crate(s) found" \
    "!violation: profile=cpu-only" "!violation: profile=compute-only-metal" "!ok: " "!${rw}"

  # (l) cpu-ops の裏に cudarc があれば cpu-only・compute-only-metal・graphics-vulkan で違反になる（graphics-interop は cudarc を許す）
  real_contract "cudarc = { path = \"../../../ext/cudarc\", optional = true }

[features]
contract-core = []
cpu-isa = []
cpu-ops = [\"dep:cudarc\"]
backend-metal = []
backend-vulkan = []
backend-cuda = []
ext-interop = []
graphics = []" || report_fail "real_cpu_ops_cudarc_setup" "cannot prepare"
  run_real
  judge "real_cpu_ops_cudarc" 1 "e:violation: profile=cpu-only crate=cudarc version=0.1.0" "e:violation: profile=graphics-vulkan crate=cudarc version=0.1.0" \
    "e:violation: profile=compute-only-metal crate=cudarc version=0.1.0" "e:FAILED: 3 judged crate(s) found" "!violation: profile=graphics-interop" "!ok: " "!${rw}"

  # (m) ext-interop の裏に判定の SDK（objc2-metal）があれば graphics-interop だけで違反になる
  real_contract "[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = []
backend-vulkan = []
backend-cuda = []
ext-interop = [\"dep:objc2-metal\"]
graphics = []" || report_fail "real_interop_only_setup" "cannot prepare"
  run_real
  judge "real_interop_only" 1 "e:violation: profile=graphics-interop crate=objc2-metal version=0.1.0" "e:FAILED: 1 judged crate(s) found" \
    "!violation: profile=cpu-only" "!violation: profile=compute-only-metal" "!violation: profile=graphics-vulkan" "!ok: " "!${rw}"

  # (n) 各 backend の feature の裏に SDK があれば、比較基準（--all-features）に 3 つが載り、4 プロファイルは合格して差が出る
  real_contract "ash = { path = \"../../../ext/ash\", optional = true }
cudarc = { path = \"../../../ext/cudarc\", optional = true }

[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = [\"dep:objc2-metal\"]
backend-vulkan = [\"dep:ash\"]
backend-cuda = [\"dep:cudarc\"]
ext-interop = []
graphics = []" || report_fail "real_baseline_setup" "cannot prepare"
  run_real
  judge "real_baseline" 0 "o:ok: 4 profile(s) checked" \
    "o:baseline: commit=" "o:profile=all-features crates_total=5 sdk_total=3" \
    "o:baseline-count: profile=all-features crate=objc2-metal n=1" "o:baseline-count: profile=all-features crate=ash n=1" \
    "o:baseline-count: profile=all-features crate=cudarc n=1" \
    "o:diff: profile=cpu-only baseline=all-features sdk_baseline=3 sdk_profile=0 diff=3" \
    "o:diff: profile=compute-only-metal baseline=all-features sdk_baseline=3 sdk_profile=1 diff=2" \
    "o:diff: profile=graphics-vulkan baseline=all-features sdk_baseline=3 sdk_profile=1 diff=2" \
    "o:diff: profile=graphics-interop baseline=all-features sdk_baseline=3 sdk_profile=2 diff=1" \
    "!violation" "!error:" "!${rw}"

  # (o) backend-vulkan の裏に ash に加えて libloading があっても Graphics の組は合格する（libloading は数えず、比較基準の SDK にも入らない）
  real_contract "ash = { path = \"../../../ext/ash\", optional = true }
libloading = { path = \"../../../ext/libloading\", optional = true }
cudarc = { path = \"../../../ext/cudarc\", optional = true }

[dependencies.objc2-metal]
path = \"../../../ext/objc2-metal\"
optional = true
default-features = false

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
backend-metal = [\"dep:objc2-metal\"]
backend-vulkan = [\"dep:ash\", \"dep:libloading\"]
backend-cuda = [\"dep:cudarc\"]
ext-interop = []
graphics = []" || report_fail "real_all_backends_setup" "cannot prepare"
  run_real
  judge "real_all_backends" 0 "o:ok: 4 profile(s) checked" \
    "o:scanned: profile=graphics-vulkan crates=4" "o:scanned: profile=graphics-interop crates=5" \
    "o:profile=all-features crates_total=6 sdk_total=3" \
    "o:baseline-count: profile=all-features crate=objc2-metal n=1" "o:baseline-count: profile=all-features crate=ash n=1" \
    "o:baseline-count: profile=all-features crate=cudarc n=1" \
    "o:diff: profile=cpu-only baseline=all-features sdk_baseline=3 sdk_profile=0 diff=3" \
    "o:diff: profile=compute-only-metal baseline=all-features sdk_baseline=3 sdk_profile=1 diff=2" \
    "o:diff: profile=graphics-vulkan baseline=all-features sdk_baseline=3 sdk_profile=1 diff=2" \
    "o:diff: profile=graphics-interop baseline=all-features sdk_baseline=3 sdk_profile=2 diff=1" \
    "!violation" "!error:" "!${rw}"
  count_is "real_all_backends_baseline_lines" "${OUT_F}" "baseline-count: " 3
  count_is "real_all_backends_no_baseline_loader_count" "${OUT_F}" "baseline-count: profile=all-features crate=libloading" 0
  count_is "real_all_backends_vulkan_loader_not_judged" "${OUT_F}" "count: profile=graphics-vulkan crate=libloading" 0
  count_is "real_all_backends_interop_loader_not_judged" "${OUT_F}" "count: profile=graphics-interop crate=libloading" 0
else
  report_fail "real_fixture_setup" "cannot prepare the temporary workspace"
fi

# ---- 実物のリポジトリ（引数なしの本物の検査）----
rc=0
"${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "repo_real" 0 "o:ok: " "o:record: commit=" "o:profile=cpu-only" "o:profile=compute-only-metal" "o:profile=graphics-vulkan" "o:profile=graphics-interop" "o:header_crate_n=0" "o:baseline: commit=" "o:profile=all-features" "!violation" "!error:" "!${ROOT_DIR}"
count_is "repo_real_baseline_count_lines" "${OUT_F}" "baseline-count: " 3
count_is "repo_real_diff_lines" "${OUT_F}" "diff: profile=" 4
# 行の数は名前の一覧から決まる（値は固定しない。SDK の依存が後で入っても変わらない形だけを確かめる）
for p in cpu-only compute-only-metal graphics-vulkan graphics-interop; do
  count_is "repo_real_count_lines_${p}" "${OUT_F}" "count: profile=${p} crate=" "$(want_name_lines "${p}")"
  count_is "repo_real_diff_line_${p}" "${OUT_F}" "diff: profile=${p} baseline=all-features " 1
done

rc=0
"${CHECK}" --extra-judged-crate fandhe-silicon-core >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "repo_real_extra_fails" 1 "e:violation: profile=cpu-only crate=fandhe-silicon-core" \
  "e:violation: profile=compute-only-metal crate=fandhe-silicon-core" "e:violation: profile=graphics-vulkan crate=fandhe-silicon-core" \
  "e:violation: profile=graphics-interop crate=fandhe-silicon-core" "e:FAILED: 4 judged crate(s) found" "o:extra_judged_crate=fandhe-silicon-core" "!ok: " "!error:" "!${ROOT_DIR}"

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
