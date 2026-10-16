#!/usr/bin/env bash
# upper-layer-check.sh の回帰テスト（spec TASK-15 のポインタ。親 Issue #367 の 3/4）。
# 節の構成:
#   1. 合成の節: 一時ディレクトリに合成の crate と metadata の JSON を作り、--metadata-json /
#      --measure-metadata-json で渡す。4 つの検査（unsafe の不在・チップの crate の不在・直接依存の許可リスト・
#      測定の feature）それぞれの合格・違反と、対象が 0 件の合格、前提が足りないときの終了コード 2 を確かめる。cargo・git を呼ばない。
#   2. ツールが無いとき: PATH を絞った環境で python3・cargo・git の未導入が終了コード 2 になることを確かめる。
#   3. 偽のコマンド: PATH の先頭に置いた偽の cargo / git で、引数なしの経路（cargo を流す経路）を通す。
#   4. 一時 workspace の実物: 実行時に作る合成 path crate の一時 workspace に検査を写し、実物の cargo metadata で通す
#      （依存は path だけ。ネットワークは使わない）。
#   5. 実物のリポジトリ: crates/upper と measure/ に当てた合格を 1 件。件数の値は固定しない。
# リポジトリのファイルを変更しない（5 は git の読み取りと cargo metadata --locked --offline --no-deps だけ）。
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」。終了コードだけでは別の理由で落ちても気付けず、
# 検査が素通りする退行（fail-open）を見逃すため、違反のケースでは `ok:` が出ないことも確かめる。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として直す。
# 前提: python3・cargo・git・rust-toolchain.toml の toolchain（無ければ失敗にし、skip しない）。
# 呼び出し元: 直接実行のみ（make ターゲットと CI への組み込みは後の段）。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
require_cmd python3
require_cmd cargo
require_cmd git

CHECK="${DIR}/upper-layer-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-upper-layer-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
passed=0
failed=0

# (3)(4) 用の入力。run_case が読み、実行のあとに既定へ戻す。
# 上の層の dependencies の JSON、測定の packages の JSON、製品の metadata に載せる契約の package の JSON、
# 渡す引数の組（both / product-only / measure-only）、測定の metadata の全体を差し替える文字列（空なら組み立てる）。
DEF_UPPER_DEPS='[]'
DEF_MEASURE_PKGS='[{"id":"m","name":"m","dependencies":[],"features":{}}]'
DEF_CONTRACT='{"id":"c","name":"fandhe-silicon-contract","manifest_path":"@D@/crates/contract/Cargo.toml","dependencies":[],"features":{"default":["contract-core"],"contract-core":[],"x":["test-support"],"test-support":[],"fault-injection":[]},"targets":[]}'
UPPER_DEPS="${DEF_UPPER_DEPS}"
MEASURE_PKGS="${DEF_MEASURE_PKGS}"
CONTRACT_PKG="${DEF_CONTRACT}"
ARGS_MODE=both
MEASURE_RAW=""
# 追加ファイルの置き場（crate ディレクトリからの相対）、製品の metadata の全体を差し替える文字列（@D@ は置換）、
# 準備のあとに呼ぶ関数名（引数は crate ディレクトリ）。run_case のあとに既定へ戻す。
EXTRA_REL="src/x.rs"
MD_RAW=""
PRE_HOOK=""
LAST_DIR=""

# run_case <名前> <期待する終了コード> <lib.rs の内容> [追加ファイル src/x.rs の内容]
run_case() {
    local name="$1" want="$2" lib="$3" extra="${4:-}" d rc=0
    d="${work}/${name}"
    mkdir -p "${d}/crates/upper/src"
    printf '[package]\nname = "u"\nversion = "0.1.0"\n' >"${d}/crates/upper/Cargo.toml"
    printf '%s\n' "${lib}" >"${d}/crates/upper/src/lib.rs"
    if [ -n "${extra}" ]; then
        mkdir -p "$(dirname "${d}/crates/upper/${EXTRA_REL}")"
        printf '%s\n' "${extra}" >"${d}/crates/upper/${EXTRA_REL}"
    fi
    if [ -n "${PRE_HOOK}" ]; then
        "${PRE_HOOK}" "${d}/crates/upper"
    fi
    local contract="" members='["u"]'
    if [ -n "${CONTRACT_PKG}" ]; then
        contract=",${CONTRACT_PKG//@D@/${d}}"
        members='["u","c"]'
    fi
    if [ -n "${MD_RAW}" ]; then
        printf '%s' "${MD_RAW//@D@/${d}}" >"${d}/md.json"
    else
        cat >"${d}/md.json" <<JSON
{"workspace_root":"${d}","workspace_members":${members},"packages":[{"id":"u","name":"u","manifest_path":"${d}/crates/upper/Cargo.toml","dependencies":${UPPER_DEPS},"targets":[{"kind":["lib"],"src_path":"${d}/crates/upper/src/lib.rs"}]}${contract}]}
JSON
    fi
    if [ -n "${MEASURE_RAW}" ]; then
        printf '%s' "${MEASURE_RAW}" >"${d}/mm.json"
    else
        printf '{"workspace_root":"%s/measure","workspace_members":["m"],"packages":%s}' "${d}" "${MEASURE_PKGS//@D@/${d}}" >"${d}/mm.json"
    fi
    case "${ARGS_MODE}" in
        both) "${CHECK}" --metadata-json "${d}/md.json" --measure-metadata-json "${d}/mm.json" >"${d}/out" 2>"${d}/err" || rc=$? ;;
        product-only) "${CHECK}" --metadata-json "${d}/md.json" >"${d}/out" 2>"${d}/err" || rc=$? ;;
        measure-only) "${CHECK}" --measure-metadata-json "${d}/mm.json" >"${d}/out" 2>"${d}/err" || rc=$? ;;
    esac
    UPPER_DEPS="${DEF_UPPER_DEPS}"
    MEASURE_PKGS="${DEF_MEASURE_PKGS}"
    CONTRACT_PKG="${DEF_CONTRACT}"
    ARGS_MODE=both
    MEASURE_RAW=""
    EXTRA_REL="src/x.rs"
    MD_RAW=""
    PRE_HOOK=""
    if [ "${rc}" -eq "${want}" ]; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        echo "FAIL: ${name}: exit ${rc} (want ${want})" >&2
    fi
    LAST_DIR="${d}"
}

# expect_text <名前> <out|err> <含まれるべき文字列>: 直前の run_case の出力を確かめる。
expect_text() {
    if grep -qF -- "$3" "${LAST_DIR}/$2"; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        echo "FAIL: $1: '$3' not in $2" >&2
    fi
}

# expect_absent <名前> <out|err> <含まれてはいけない文字列>: expect_text の否定形。
expect_absent() {
    if grep -qF -- "$3" "${LAST_DIR}/$2"; then
        failed=$((failed + 1))
        echo "FAIL: $1: '$3' must not be in $2" >&2
    else
        passed=$((passed + 1))
    fi
}

FB='#![forbid(unsafe_code)]'
run_case ok 0 "${FB}"
# shellcheck disable=SC2016
run_case macro-args 1 'macro_rules! discard { ($($t:tt)*) => {}; } discard!(#![forbid(unsafe_code)]);'
run_case macro-brackets 1 'm![#![forbid(unsafe_code)]];'
run_case in-mod 1 'mod a { #![forbid(unsafe_code)] }'
run_case raw-ident 0 "${FB}
fn r#unsafe() {}"
run_case unicode-ident 0 "${FB}
fn éunsafe() {}"
run_case real-unsafe 1 "${FB}
fn f() { unsafe {} }"
run_case chip-grouped 1 "${FB}
use ash::{self};"
run_case chip-grouped-attr 1 "${FB}
use {#[cfg(all(a, b))] ash::b, c};"
run_case chip-grouped-attr-late 1 "${FB}
use {#[cfg(x)] c, ash};"
run_case attr-chip 1 "${FB}
use std::{#[cfg(ash)] a, b};"
run_case raw-chip-path 1 "${FB}
fn f() { r#ash::x(); }"
run_case raw-chip-use 1 "${FB}
use r#ash::b;"
run_case raw-chip-use-group 1 "${FB}
use {r#ash};"
run_case raw-chip-extern 1 "${FB}
extern crate r#ash;"
run_case combining-ident 0 "${FB}
fn unsafé() {}"
run_case combining-ident-lead 0 "${FB}
fn áunsafe() {}"
run_case combining-decomposed 0 "${FB}
fn unsafe$(printf '\xcc\x81')() {}"
run_case other-id-start 0 "${FB}
fn ℘unsafe() {}"
run_case other-id-continue 0 "${FB}
fn unsafe·x() {}"
run_case return-abs-chip 1 "${FB}
fn f() { return ::ash::foo(); }"
run_case break-abs-chip 1 "${FB}
fn f() { loop { break ::ash::foo(); } }"
run_case mid-path-chip 1 "${FB}
fn f() { foo::ash::bar(); }"
run_case use-group-local-chip 1 "${FB}
use crate::{ash::Thing};"
run_case assoc-type-path-chip 1 "${FB}
fn f<T: Tr>() { let _ = <T as Tr>::ash::Thing::new(); }"
run_case method-spaced-chip 1 "${FB}
fn f(x: X) { x . ash::<u32>(); }"
run_case method-adjacent-chip 1 "${FB}
fn f(x: X) { x.ash::<u32>(); }"
run_case method-newline-chip 1 "${FB}
fn f(x: X) { x
    .
    ash::<u32>(); }"
run_case range-lhs-chip 1 "${FB}
fn f() { let _ = 0..ash::LIMIT; }"
run_case range-open-chip 1 "${FB}
fn f() { let _ = ..ash::LIMIT; }"
run_case break-label-abs-chip 1 "${FB}
fn f() { 'done: loop { break 'done ::ash::foo(); } }"
run_case mid-path-label-chip 1 "${FB}
fn f() { foo::bar::ash::baz(); }"

run_case gt-abs-chip 1 "${FB}
fn f(x: u32) { if x > ::ash::LIMIT {} }"
# shellcheck disable=SC2016
run_case macro-use-var-chip 1 "${FB}
macro_rules! m { (\$use:ident) => {} }
fn f() { ash::g(); }"
run_case return-abs-chip2 1 "${FB}
fn f() { return ::ash::f(); }"
run_case break-label-spaced-chip 1 "${FB}
fn f() { 'l: loop { break 'l ::ash::f(); } }"
run_case range-chip 1 "${FB}
fn f() { let _ = 0..ash::X; }"
run_case closure-abs-chip 1 "${FB}
fn f() { let _ = |x: u32| ::ash::f(); }"
run_case raw-use-chip 1 "${FB}
use r#ash::Device;"
run_case use-braced-alias-chip 1 "${FB}
use {ash as vk};"
run_case underscore-chip 1 "${FB}
fn f() { objc2_metal::g(); }"
run_case underscore-use-chip 1 "${FB}
use objc2_foundation::NSString;"
run_case chip-in-extra-file 1 "${FB}" "fn f() { cudarc::g(); }"
run_case comment-not-chip 0 "${FB}
// ash::f() and /* cudarc */
/* ash */ fn f() {}"
run_case string-not-chip 0 "${FB}
fn f() { let _ = \"ash::f\"; let _ = r#\"cudarc\"#; }"
run_case partial-not-chip 0 "${FB}
fn ashes() { let _ = cash::x; let _ = ash_tray; let _ = objc2_metalx; }"
run_case prefix-objc2-not-metal 1 "${FB}
fn f() { objc2::g(); }"
run_case number-suffix-not-chip 0 "${FB}
fn f() { let _ = 1ash; }"
run_case unicode-continue-not-chip 0 "${FB}
fn ashé() {}"

run_case raw-string-inner-quote-chip 1 "${FB}
fn f() { r#\"a\"b\"#; ash::f(); }"
run_case nested-block-comment-chip 1 "${FB}
/* /* */ ash::f(); */ ash::g();"
run_case lifetime-then-chip 1 "${FB}
fn f<'a>(x: &'a u8) {}
fn g() { ash::f(); }"
run_case lifetime-name-chip 1 "${FB}
fn f<'ash>() {}"
run_case byte-string-escape-chip 1 "${FB}
fn f() { let _ = b\"x\\\"y\"; ash::f(); }"
run_case byte-raw-string-chip 1 "${FB}
fn f() { let _ = br#\"x\"y\"#; ash::f(); }"
run_case byte-quote-char-chip 1 "${FB}
fn f() { let _ = b'\"'; ash::f(); }"
run_case float-exponent-chip 1 "${FB}
fn f() { 1e5; ash::x(); }"
run_case hex-literal-chip 1 "${FB}
fn f() { 0x1f; ash::x(); }"
run_case raw-chip-extra-file 1 "${FB}" "fn f() { r#ash::x(); }"

# ---- (3) 直接依存の許可リスト ----
dep() { printf '{"name":"%s","kind":%s,"rename":%s}' "$1" "${2:-null}" "${3:-null}"; }
UPPER_DEPS="[$(dep fandhe-silicon-core),$(dep fandhe-silicon-core '"build"'),$(dep fandhe-silicon-exec '"dev"')]"
run_case allow-ok 0 "${FB}"
UPPER_DEPS="[$(dep fandhe-silicon-contract)]"
run_case allow-contract-normal 1 "${FB}"
expect_text allow-contract-normal-msg err "violation: check=dep-allow crate=fandhe-silicon-contract kind=normal"
UPPER_DEPS="[$(dep fandhe-silicon-contract '"dev"')]"
run_case allow-contract-dev 1 "${FB}"
UPPER_DEPS="[$(dep fandhe-silicon-contract '"build"')]"
run_case allow-contract-build 1 "${FB}"
UPPER_DEPS="[$(dep serde)]"
run_case allow-extra-normal 1 "${FB}"
expect_text allow-extra-normal-msg err "violation: check=dep-allow crate=serde kind=normal"
UPPER_DEPS="[$(dep serde '"build"')]"
run_case allow-extra-build 1 "${FB}"
UPPER_DEPS="[$(dep serde '"dev"')]"
run_case allow-extra-dev 1 "${FB}"
UPPER_DEPS="[$(dep serde null '"fandhe-silicon-core"')]"
run_case allow-renamed-extra 1 "${FB}"
UPPER_DEPS="[$(dep ash)]"
run_case allow-chip 1 "${FB}"
expect_text allow-chip-chipdep err "violation: check=chip-dep crate=ash kind=normal"
expect_text allow-chip-depallow err "violation: check=dep-allow crate=ash kind=normal"
UPPER_DEPS="[$(dep Fandhe_Silicon_Core)]"
run_case allow-fold-ok 0 "${FB}"

# ---- (4) 測定の workspace の feature ----
EMPTY_OBJ='{}'
mpkg() { printf '[{"id":"m","name":"m","dependencies":[%s],"features":%s}]' "$1" "${2:-${EMPTY_OBJ}}"; }
cdep() { printf '{"name":"fandhe-silicon-contract","kind":%s,"features":%s,"uses_default_features":%s,"rename":%s,"path":%s}' "${1:-null}" "${2:-[]}" "${3:-true}" "${4:-null}" "${5:-null}"; }
run_case measure-no-contract 0 "${FB}"
expect_text measure-no-contract-count out "contract_dependents=0"
MEASURE_PKGS="$(mpkg "$(cdep)")"
run_case measure-default 0 "${FB}"
expect_text measure-default-count out "contract_dependents=1"
MEASURE_PKGS="$(mpkg "$(cdep null '[]' false)")"
run_case measure-no-default 0 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep null '["test-support"]')")"
run_case measure-test-support 1 "${FB}"
expect_text measure-test-support-msg err "violation: check=measure-feature package=m kind=normal feature=test-support"
MEASURE_PKGS="$(mpkg "$(cdep null '["fault-injection"]')")"
run_case measure-fault-injection 1 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep '"dev"' '["test-support"]')")"
run_case measure-dev-test-support 1 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep '"build"' '["test-support"]')")"
run_case measure-build-test-support 1 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep)" '{"f":["fandhe-silicon-contract/test-support"]}')"
run_case measure-forward 1 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep)" '{"f":["fandhe-silicon-contract?/fault-injection"]}')"
run_case measure-forward-optional 1 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep null '[]' true '"k"')" '{"f":["k/test-support"]}')"
run_case measure-forward-renamed 1 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep)" '{"f":["other/test-support"]}')"
run_case measure-forward-other-dep 0 "${FB}"
MEASURE_PKGS="$(mpkg "$(cdep null '["x"]')")"
run_case measure-indirect 1 "${FB}"
MEASURE_PKGS="$(mpkg '{"name":"alias-name","kind":null,"features":["test-support"],"uses_default_features":true,"rename":null,"path":"@D@/crates/contract"}')"
run_case measure-path-by-location 1 "${FB}"

# ---- 入力の組・形の崩れ ----
ARGS_MODE=product-only
run_case args-product-only 2 "${FB}"
ARGS_MODE=measure-only
run_case args-measure-only 2 "${FB}"
MEASURE_RAW='not json'
run_case measure-not-json 2 "${FB}"
MEASURE_RAW='{"workspace_members":[]}'
run_case measure-no-packages 2 "${FB}"
MEASURE_RAW='{"workspace_root":"/x","workspace_members":["missing"],"packages":[]}'
run_case measure-member-missing 2 "${FB}"
MEASURE_RAW='{"workspace_root":"/x","workspace_members":[],"packages":[{"id":"m","name":"m","dependencies":[],"features":{}}]}'
run_case measure-members-empty 2 "${FB}"
MEASURE_RAW='{"workspace_root":"/x","workspace_members":[""],"packages":[{"id":"","name":"m","dependencies":[],"features":{}}]}'
run_case measure-member-empty-id 2 "${FB}"
MEASURE_RAW='{"workspace_root":"/x","workspace_members":[1],"packages":[{"id":"m","name":"m","dependencies":[],"features":{}}]}'
run_case measure-member-non-string 2 "${FB}"
MEASURE_RAW='{"workspace_root":"/x","workspace_members":["m"],"packages":[{"id":"m","name":"m","dependencies":[],"features":{}},{"id":"m","name":"m2","dependencies":[],"features":{}}]}'
run_case measure-package-duplicate 2 "${FB}"
CONTRACT_PKG=""
run_case product-no-contract 2 "${FB}"

# ---- 判定の型（違反・誤用のケースで成功の行が出ないことまで確かめる）----
# good <名前> <lib.rs> [追加ファイル]: 合格（終了コード 0・ok の行・違反なし・誤りなし・ホストのパスなし）。
good() {
    local n="$1"
    shift
    run_case "${n}" 0 "$@"
    expect_text "${n}-ok" out "ok:"
    expect_absent "${n}-no-violation" err "violation"
    expect_absent "${n}-no-error" err "error:"
    expect_absent "${n}-no-host-path" out "${work}"
}
# bad <名前> <lib.rs> [追加ファイル]: 違反（終了コード 1・FAILED の行・record は出る・ok は出ない）。
bad() {
    local n="$1"
    shift
    run_case "${n}" 1 "$@"
    expect_text "${n}-failed" err "FAILED:"
    expect_text "${n}-record" out "record:"
    expect_absent "${n}-no-ok" out "ok:"
    expect_absent "${n}-no-error" err "error:"
    expect_absent "${n}-no-host-path" err "${work}"
}
# misuse <名前> <lib.rs> [追加ファイル]: 誤用・前提の不足（終了コード 2・error の行・成功の行も record もなし）。
misuse() {
    local n="$1"
    shift
    run_case "${n}" 2 "$@"
    expect_text "${n}-error" err "error:"
    expect_absent "${n}-no-ok" out "ok:"
    expect_absent "${n}-no-record" out "record:"
    expect_absent "${n}-no-violation" err "violation"
    expect_absent "${n}-no-host-path" err "${work}"
}

# 代表のケースを、成功の行・違反の行の有無まで確かめる型で流し直す。
good ok-again "${FB}"
run_case real-unsafe-again 1 "${FB}
fn f() { unsafe {} }"
expect_text real-unsafe-where err "violation: check=unsafe file=src/lib.rs line=2"
expect_absent real-unsafe-no-ok out "ok:"

# ---- (1) unsafe の不在: forbid の宣言と、forbid が届かない位置 ----
bad no-forbid ""
expect_text no-forbid-msg err "violation: check=forbid file=src/lib.rs"
bad forbid-in-other-file "" "${FB}"
expect_text forbid-in-other-file-msg err "violation: check=forbid file=src/lib.rs"
bad forbid-other-lint "#![forbid(missing_docs)]"
expect_text forbid-other-lint-msg err "violation: check=forbid file=src/lib.rs"
bad forbid-deny-only "#![deny(unsafe_code)]"
EXTRA_REL="tests/t.rs"
bad unsafe-in-tests "${FB}" "fn f() { unsafe {} }"
expect_text unsafe-in-tests-msg err "violation: check=unsafe file=tests/t.rs line=1"
expect_absent unsafe-in-tests-forbid-ok err "check=forbid"
EXTRA_REL="build.rs"
bad unsafe-in-build-rs "${FB}" "fn main() { unsafe {} }"
expect_text unsafe-in-build-rs-msg err "violation: check=unsafe file=build.rs line=1"
EXTRA_REL="benches/b.rs"
bad unsafe-in-benches "${FB}" "fn f() { unsafe {} }"
expect_text unsafe-in-benches-msg err "violation: check=unsafe file=benches/b.rs line=1"
EXTRA_REL="examples/e.rs"
bad unsafe-in-examples "${FB}" "fn main() { unsafe {} }"
expect_text unsafe-in-examples-msg err "violation: check=unsafe file=examples/e.rs line=1"
EXTRA_REL="src/deep/mod.rs"
bad unsafe-in-nested-src "${FB}" "unsafe fn f() {}"
expect_text unsafe-in-nested-src-msg err "violation: check=unsafe file=src/deep/mod.rs line=1"
# 2 件あれば 2 件の違反になる（件数を数える）。
bad unsafe-two "${FB}
fn f() { unsafe {} }
fn g() { unsafe {} }"
expect_text unsafe-two-count err "FAILED: 2 violation(s)"
expect_text unsafe-two-total out "unsafe_total=2"
# コメント・文字列の中の unsafe は数えない。
good unsafe-in-comment-string "${FB}
// unsafe { }
/* unsafe */
fn f() { let _ = \"unsafe\"; let _ = r#\"unsafe\"#; }"
expect_text unsafe-in-comment-string-count out "unsafe_total=0"
# 走査しない位置（target/ と隠しディレクトリ）の unsafe は数えない。
EXTRA_REL="target/x.rs"
good unsafe-in-target-dir "${FB}" "fn f() { unsafe {} }"
EXTRA_REL=".hidden/x.rs"
good unsafe-in-hidden-dir "${FB}" "fn f() { unsafe {} }"
expect_text unsafe-in-hidden-dir-files out "rs_files=1 "

# ---- (2) チップの crate: 全部の名前 × 依存の種類 × use × パス ----
CHIP_NAMES="objc2 objc2-foundation objc2-metal objc2-core-foundation ash cudarc"
# 検査側の CHIP_CRATES とこのテストの名前の一覧が一致していること（一致しなければ、名前を足した側に合わせてテストを直す）。
cat >"${work}/chip-sync.py" <<'PY'
import ast
import sys

names = None
with open(sys.argv[1], encoding="utf-8") as f:
    for line in f:
        if line.startswith("CHIP_CRATES = "):
            names = set(ast.literal_eval(line.split("=", 1)[1].strip()))
            break
print("same" if names is not None and names == set(sys.argv[2].split()) else "differ")
PY
if [ "$(python3 -I "${work}/chip-sync.py" "${CHECK}" "${CHIP_NAMES}")" = "same" ]; then
    passed=$((passed + 1))
else
    failed=$((failed + 1))
    echo "FAIL: chip-names-in-sync: CHIP_CRATES in upper-layer-check.sh differs from CHIP_NAMES in this test" >&2
fi
for n in ${CHIP_NAMES}; do
    id="${n//-/_}"
    for pair in "null:normal" '"build":build' '"dev":dev'; do
        kj="${pair%%:*}"
        kl="${pair##*:}"
        UPPER_DEPS="[$(dep "${n}" "${kj}")]"
        bad "chip-dep-${n}-${kl}" "${FB}"
        expect_text "chip-dep-${n}-${kl}-chip" err "violation: check=chip-dep crate=${n} kind=${kl}"
        expect_text "chip-dep-${n}-${kl}-allow" err "violation: check=dep-allow crate=${n} kind=${kl}"
        expect_text "chip-dep-${n}-${kl}-count" err "FAILED: 2 violation(s)"
    done
    bad "chip-use-${n}" "${FB}
use ${id}::x;"
    expect_text "chip-use-${n}-msg" err "violation: check=chip-path file=src/lib.rs line=2 crate=${n}"
    bad "chip-path-${n}" "${FB}
fn f() { ${id}::g(); }"
    expect_text "chip-path-${n}-msg" err "violation: check=chip-path file=src/lib.rs line=2 crate=${n}"
    bad "chip-path-extra-${n}" "${FB}" "fn f() { ${id}::g(); }"
    expect_text "chip-path-extra-${n}-msg" err "violation: check=chip-path file=src/x.rs line=1 crate=${n}"
done
# 依存の名前の大文字・下線は折りたたんで当てる。改名の依存は package の名前（実体）で当てる。
UPPER_DEPS="[$(dep ASH)]"
bad chip-dep-upper-case "${FB}"
expect_text chip-dep-upper-case-msg err "violation: check=chip-dep crate=ash kind=normal"
UPPER_DEPS="[$(dep objc2_metal)]"
bad chip-dep-underscore "${FB}"
expect_text chip-dep-underscore-msg err "violation: check=chip-dep crate=objc2-metal kind=normal"
UPPER_DEPS="[$(dep ash null '"vk"')]"
bad chip-dep-renamed "${FB}"
expect_text chip-dep-renamed-msg err "violation: check=chip-dep crate=ash kind=normal"
# 依存もソースも無ければ 0 件で合格する。
good chip-none "${FB}"
expect_text chip-none-count out "chip_deps=0 chip_paths=0"

# ---- (3) 直接依存: 対象が 0 件でも合格する ----
good allow-zero-deps "${FB}"
expect_text allow-zero-deps-count out "direct_deps=0 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0"
UPPER_DEPS="[$(dep fandhe-silicon-core),$(dep fandhe-silicon-exec)]"
good allow-both-ok "${FB}"
expect_text allow-both-ok-count out "direct_deps=2 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0"
UPPER_DEPS="[$(dep fandhe-silicon-core),$(dep serde)]"
bad allow-one-of-two "${FB}"
expect_text allow-one-of-two-msg err "violation: check=dep-allow crate=serde kind=normal"
expect_absent allow-one-of-two-core err "crate=fandhe-silicon-core"
expect_text allow-one-of-two-count err "FAILED: 1 violation(s)"
UPPER_DEPS="[$(dep serde '"dev"'),$(dep serde '"build"')]"
bad allow-dev-and-build "${FB}"
expect_text allow-dev-and-build-count err "FAILED: 2 violation(s)"
expect_text allow-dev-and-build-dev err "violation: check=dep-allow crate=serde kind=dev"
expect_text allow-dev-and-build-build err "violation: check=dep-allow crate=serde kind=build"

# ---- (4) 測定の feature: 対象が 0 件の合格・片方の member だけの違反・複数の feature ----
good measure-zero-contract-deps "${FB}"
expect_text measure-zero-contract-deps-count out "contract_dependents=0"
expect_text measure-zero-contract-deps-forbidden out "forbidden_features=0"
MEASURE_PKGS="$(mpkg "$(cdep null '[]' false)")"
good measure-no-default-ok "${FB}"
expect_text measure-no-default-ok-count out "contract_dependents=1"
MEASURE_RAW="{\"workspace_root\":\"/x\",\"workspace_members\":[\"m\",\"n\"],\"packages\":[{\"id\":\"m\",\"name\":\"clean-member\",\"dependencies\":[$(cdep)],\"features\":{}},{\"id\":\"n\",\"name\":\"bad-member\",\"dependencies\":[$(cdep null '["test-support"]')],\"features\":{}}]}"
bad measure-one-of-two "${FB}"
expect_text measure-one-of-two-msg err "violation: check=measure-feature package=bad-member kind=normal feature=test-support"
expect_absent measure-one-of-two-clean err "package=clean-member"
expect_text measure-one-of-two-count err "FAILED: 1 violation(s)"
expect_text measure-one-of-two-dependents out "contract_dependents=2"
MEASURE_PKGS="$(mpkg "$(cdep null '["test-support","fault-injection"]')")"
bad measure-both-features "${FB}"
expect_text measure-both-features-count err "FAILED: 2 violation(s)"
expect_text measure-both-features-a err "feature=fault-injection"
expect_text measure-both-features-b err "feature=test-support"
# member でない package の依存は見ない（workspace_members の外）。
MEASURE_RAW="{\"workspace_root\":\"/x\",\"workspace_members\":[\"m\"],\"packages\":[{\"id\":\"m\",\"name\":\"m\",\"dependencies\":[],\"features\":{}},{\"id\":\"z\",\"name\":\"z\",\"dependencies\":[$(cdep null '["test-support"]')],\"features\":{}}]}"
good measure-nonmember-ignored "${FB}"
expect_text measure-nonmember-ignored-count out "contract_dependents=0"
# 上の層の側の違反があっても測定の側の判定は独立に出る（両方の違反が出る）。
MEASURE_PKGS="$(mpkg "$(cdep null '["test-support"]')")"
UPPER_DEPS="[$(dep serde)]"
bad measure-and-upper "${FB}
fn f() { unsafe {} }"
expect_text measure-and-upper-count err "FAILED: 3 violation(s)"

# ---- 終了コード 2: 前提の不足・読み切れない入力 ----
UPPER_DEPS="${DEF_UPPER_DEPS}"
MD_RAW='{"workspace_root":"@D@","workspace_members":["x"],"packages":[{"id":"x","name":"x","manifest_path":"@D@/crates/other/Cargo.toml","dependencies":[],"targets":[]}]}'
misuse upper-missing "${FB}"
expect_text upper-missing-msg err "the upper-layer crate is not found exactly once"
MD_RAW='{"workspace_root":"@D@","workspace_members":["x"],"packages":[{"id":"x","name":"x","manifest_path":"@D@/crates/upper/Cargo.toml","dependencies":[],"targets":[{"kind":["bin"],"src_path":"@D@/crates/upper/src/main.rs"}]}]}'
misuse upper-no-lib "${FB}"
expect_text upper-no-lib-msg err "exactly one lib target"
MD_RAW='not json'
misuse product-not-json "${FB}"
expect_text product-not-json-msg err "metadata input is not JSON"
MD_RAW='{"workspace_root":"@D@","workspace_members":[],"packages":[]}'
misuse product-no-members "${FB}"
expect_text product-no-members-msg err "missing or empty 'workspace_members'"
MD_RAW='[]'
misuse product-not-object "${FB}"
expect_text product-not-object-msg err "metadata top level is not an object"
hook_rm_dir() { rm -rf "$1"; }
PRE_HOOK=hook_rm_dir
misuse upper-dir-missing "${FB}"
expect_text upper-dir-missing-msg err "the upper-layer crate directory is not readable"
hook_no_rs() { rm -f "$1/src/lib.rs"; }
PRE_HOOK=hook_no_rs
misuse no-rs-files "${FB}"
expect_text no-rs-files-msg err "no source files found"
hook_non_utf8() { printf '\377\376' >"$1/src/x.rs"; }
PRE_HOOK=hook_non_utf8
misuse non-utf8 "${FB}"
expect_text non-utf8-msg err "cannot read input file"
hook_symlink_rs() { ln -s lib.rs "$1/src/link.rs"; }
PRE_HOOK=hook_symlink_rs
misuse symlink-rs "${FB}"
expect_text symlink-rs-msg err "symbolic link in the crate directory"
hook_symlink_dir() { mkdir -p "$1/real" && ln -s real "$1/linkdir"; }
PRE_HOOK=hook_symlink_dir
misuse symlink-dir "${FB}"
expect_text symlink-dir-msg err "symbolic link in the crate directory"
misuse unterminated-comment "${FB}
/* never closed"
expect_text unterminated-comment-msg err "unterminated block comment"
misuse unterminated-string "${FB}
fn f() { let _ = \"never closed; }"
expect_text unterminated-string-msg err "unterminated string literal"
misuse unbalanced-root "${FB}
fn f() {"
expect_text unbalanced-root-msg err "unbalanced brackets in the crate root"
misuse unterminated-comment-extra "${FB}" "/* never closed"
expect_text unterminated-comment-extra-msg err "unterminated block comment"
ARGS_MODE=product-only
misuse args-product-only-again "${FB}"
expect_text args-product-only-again-msg err "must be given together"
ARGS_MODE=measure-only
misuse args-measure-only-again "${FB}"
CONTRACT_PKG=""
misuse product-no-contract-again "${FB}"
expect_text product-no-contract-again-msg err "the contract crate is not found exactly once"

# ---- 以降は引数なしの経路（cargo・git を呼ぶ経路）の節。標準出力・標準エラーは作業ディレクトリの 2 ファイルに控える ----
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
rc=0

report_ok() {
    passed=$((passed + 1))
}

report_fail() {
    failed=$((failed + 1))
    echo "FAIL: $1: $2" >&2
    echo "--- stderr ---" >&2
    sed 's/^/    /' "${ERR_F}" >&2 || true
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

# count_is <名前> <ファイル> <固定文字列> <件数>: 固定文字列を含む行数が一致すること。
count_is() {
    local name="$1" file="$2" word="$3" want="$4" got
    got="$(grep -cF -e "${word}" "${file}" || true)"
    if [ "${got}" = "${want}" ]; then
        report_ok "${name}"
    else
        report_fail "${name}" "expected ${want} line(s) containing '${word}', got ${got}"
    fi
}

# ---- ツール未導入（成功にせず終了コード 2）----
# PATH を絞った環境で、検査を絶対パスの bash から起動する。絞った PATH に入れるのは必要なコマンドだけ。
bash_bin="$(command -v bash)"
dirname_bin="$(command -v dirname)"
git_bin="$(command -v git)"
py_real="$(python3 -I -c 'import sys; print(sys.executable)')"

nopy="${work}/nopy"
mkdir -p "${nopy}"
ln -s "${dirname_bin}" "${nopy}/dirname"
rc=0
PATH="${nopy}" "${bash_bin}" "${CHECK}" --metadata-json "${work}/ok/md.json" --measure-metadata-json "${work}/ok/mm.json" \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge no_python3 2 "e:required command not found on PATH: python3" "!ok: " "!record: " "!scanned:"

nocargo="${work}/nocargo"
mkdir -p "${nocargo}"
ln -s "${dirname_bin}" "${nocargo}/dirname"
ln -s "${py_real}" "${nocargo}/python3"
ln -s "${git_bin}" "${nocargo}/git"
rc=0
PATH="${nocargo}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge no_cargo 2 "e:error: cannot run cargo" "!ok: " "!record: " "!scanned:"

nogit="${work}/nogit"
mkdir -p "${nogit}"
ln -s "${dirname_bin}" "${nogit}/dirname"
ln -s "${py_real}" "${nogit}/python3"
rc=0
PATH="${nogit}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge no_git 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"

# 未知のオプションは誤用（引数 --metadata-json の片方だけは合成の節で確かめ済み）。
rc=0
"${CHECK}" --no-such-option >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge unknown_option 2 "!ok: " "!record: "

# ---- 偽の cargo / git（引数なしの経路。PATH の先頭に置く）----
# 偽の cargo は FAKE_CARGO_MODE で、偽の git は FAKE_GIT_MODE で振る舞いを変える。偽のコマンドは ${work} の中だけに置く。
fakebin="${work}/fakebin"
mkdir -p "${fakebin}"
cat >"${fakebin}/cargo" <<'FAKE'
#!/usr/bin/env bash
mode="${FAKE_CARGO_MODE:-ok}"
if [ -n "${FAKE_ARGS_LOG:-}" ]; then
  echo "$*" >>"${FAKE_ARGS_LOG}"
fi
case "$1" in
  --version)
    if [ "${mode}" = "fail" ]; then
      exit 1
    fi
    echo "cargo 0.0.0 (fake)"
    ;;
  metadata)
    is_measure=0
    case "$*" in
      *--manifest-path*) is_measure=1 ;;
    esac
    if [ "${mode}" = "metafail" ]; then
      exit 1
    fi
    if [ "${mode}" = "measurefail" ] && [ "${is_measure}" = 1 ]; then
      exit 1
    fi
    if [ "${mode}" = "badjson" ]; then
      echo "{not json"
    elif [ "${mode}" = "measurebadjson" ] && [ "${is_measure}" = 1 ]; then
      echo "{not json"
    elif [ "${is_measure}" = 1 ]; then
      cat "${FAKE_MM}"
    else
      cat "${FAKE_MD}"
    fi
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

# run_fake <cargo モード> <git モード>: 合成の節の ok ケースの metadata を偽の cargo に返させる。
args_log="${work}/args.log"
run_fake() {
    rc=0
    rm -f "${args_log}"
    PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE="$1" FAKE_GIT_MODE="$2" FAKE_ARGS_LOG="${args_log}" \
        FAKE_MD="${work}/ok/md.json" FAKE_MM="${work}/ok/mm.json" \
        "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

run_fake ok ok
judge fake_ok 0 "o:record: commit=0123456789abcdef0123456789abcdef01234567 " "o:ok: " \
    "!-dirty" "!violation" "!error:" "!${work}"
count_is fake_ok_two_metadata "${args_log}" "metadata --locked --offline --no-deps --format-version 1 --all-features" 2
count_is fake_ok_measure_manifest "${args_log}" "measure/Cargo.toml" 1
count_is fake_ok_version "${args_log}" "--version" 1
run_fake ok dirty
judge fake_dirty 0 "o:commit=0123456789abcdef0123456789abcdef01234567-dirty " "o:ok: " "!violation"
run_fake fail ok
judge fake_cargo_version_fails 2 "e:error: cargo --version failed" "!ok: " "!record: "
run_fake metafail ok
judge fake_metadata_fails 2 "e:error: cargo metadata failed" "!ok: " "!record: "
run_fake measurefail ok
judge fake_measure_metadata_fails 2 "e:error: cargo metadata failed" "!ok: " "!record: "
run_fake badjson ok
judge fake_metadata_not_json 2 "e:error: cargo metadata output is not JSON" "!ok: " "!record: "
run_fake measurebadjson ok
judge fake_measure_metadata_not_json 2 "e:error: cargo metadata output is not JSON" "!ok: " "!record: "
run_fake ok fail
judge fake_git_fails 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"
run_fake ok garbage
judge fake_git_garbage 2 "e:error: cannot resolve the source commit" "!/secret/host/path" "!ok: " "!record: "

# ---- 一時 workspace の実物（実物の cargo metadata。引数なしの経路）----
# 合成 path crate だけの一時 workspace（製品の 4 crate 相当 + measure/ の別 workspace）を作り、検査を写して引数なしで流す。
# ネットワークは使わない（--offline）。リポジトリのファイルは変更しない。
rw="${work}/real"
mkdir -p "${rw}/ws/scripts" "${rw}/ext" "${rw}/ws/measure/harness/src"
for c in core exec contract upper; do
    mkdir -p "${rw}/ws/crates/${c}/src"
done
cat >"${rw}/ws/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/core", "crates/exec", "crates/contract", "crates/upper"]
exclude = ["measure"]
resolver = "2"
TOML
for c in core exec; do
    printf '[package]\nname = "fandhe-silicon-%s"\nversion = "0.1.0"\nedition = "2021"\n' "${c}" >"${rw}/ws/crates/${c}/Cargo.toml"
    printf '%s\n' '// fixture' >"${rw}/ws/crates/${c}/src/lib.rs"
done
cat >"${rw}/ws/crates/contract/Cargo.toml" <<'TOML'
[package]
name = "fandhe-silicon-contract"
version = "0.1.0"
edition = "2021"

[features]
default = []
test-support = []
fault-injection = []
TOML
printf '%s\n' '// fixture' >"${rw}/ws/crates/contract/src/lib.rs"
printf '%s\n' '// fixture' >"${rw}/ws/measure/harness/src/lib.rs"
cp "${DIR}/upper-layer-check.sh" "${DIR}/lib.sh" "${rw}/ws/scripts/"
chmod +x "${rw}/ws/scripts/upper-layer-check.sh"
if [ -f "${DIR}/../rust-toolchain.toml" ]; then
    cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi

# mk_ext <名前>: ${rw}/ext/<名前> に空の合成 crate を作る（ws の外。path 依存だけで取得は要らない）。
mk_ext() {
    mkdir -p "${rw}/ext/$1/src"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2021"\n' "$1" >"${rw}/ext/$1/Cargo.toml"
    printf '%s\n' '// fixture' >"${rw}/ext/$1/src/lib.rs"
}
mk_ext ash
mk_ext objc2-metal
mk_ext plain-dummy

# real_git: 開発者の全体設定とフックを効かせない git。
real_git() {
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
        -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# real_commit: 製品と measure/ のロックを作り直してコミットする（--locked で流すため）。
real_commit() {
    (cd "${rw}/ws" && cargo generate-lockfile --offline && cargo generate-lockfile --offline --manifest-path measure/Cargo.toml) \
        >/dev/null 2>"${ERR_F}" \
        && real_git add -A >/dev/null 2>>"${ERR_F}" \
        && real_git commit -q --allow-empty -m fixture >/dev/null 2>>"${ERR_F}"
}
# 既定の形（forbid あり・追加ファイルなし）。real_state が読み、呼んだあとに既定へ戻す。
DEF_RS_LIB='#![forbid(unsafe_code)]'
RS_LIB="${DEF_RS_LIB}"
RS_TEST=""
RS_SRC_X=""
HARNESS_PLAIN='[dependencies]'
# real_state <上の層の Cargo.toml の追記> [測定の harness の Cargo.toml の追記]: 全体を既定から組み直してコミットする。
real_state() {
    rm -rf "${rw}/ws/crates/upper/tests" "${rw}/ws/crates/upper/src/x.rs"
    {
        printf '[package]\nname = "fandhe-silicon-upper"\nversion = "0.1.0"\nedition = "2021"\n\n'
        printf '[dependencies]\nfandhe-silicon-core = { path = "../core" }\nfandhe-silicon-exec = { path = "../exec" }\n'
        printf '%s\n' "$1"
    } >"${rw}/ws/crates/upper/Cargo.toml"
    printf '%s\n' "${RS_LIB}" >"${rw}/ws/crates/upper/src/lib.rs"
    if [ -n "${RS_TEST}" ]; then
        mkdir -p "${rw}/ws/crates/upper/tests"
        printf '%s\n' "${RS_TEST}" >"${rw}/ws/crates/upper/tests/t.rs"
    fi
    if [ -n "${RS_SRC_X}" ]; then
        printf '%s\n' "${RS_SRC_X}" >"${rw}/ws/crates/upper/src/x.rs"
    fi
    {
        printf '[package]\nname = "harness"\nversion = "0.1.0"\nedition = "2021"\n\n'
        printf '%s\n' "${2:-${HARNESS_PLAIN}}"
    } >"${rw}/ws/measure/harness/Cargo.toml"
    printf '[workspace]\nmembers = ["harness"]\nresolver = "2"\n' >"${rw}/ws/measure/Cargo.toml"
    RS_LIB="${DEF_RS_LIB}"
    RS_TEST=""
    RS_SRC_X=""
    real_commit
}
# run_real: 写した検査を引数なしで流す。共有の target ディレクトリへ書かせない。
run_real() {
    rc=0
    (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR ./scripts/upper-layer-check.sh) \
        >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}
# real_case <名前> <期待コード> [語...]: run_real + judge。ホストの一時パスが出ないことも確かめる。
real_case() {
    local n="$1" w="$2"
    shift 2
    run_real
    judge "${n}" "${w}" "!${rw}" "$@"
}
HARNESS_CONTRACT='[dependencies]
fandhe-silicon-contract = { path = "../../crates/contract", default-features = false }'

if real_git init -q >/dev/null 2>"${ERR_F}" && real_state ""; then
    # (a) 既定: 合格・対象 0 件（測定が契約に依存しない）
    real_case real_clean 0 "o:ok: " "o:contract_dependents=0" "o:direct_deps=2 " "o:forbid=yes" \
        "!-dirty" "!violation" "!error:"
    if grep -Eq '^record: commit=[0-9a-f]{40,64} ' "${OUT_F}"; then
        report_ok real_clean_commit_is_clean_sha
    else
        report_fail real_clean_commit_is_clean_sha "commit must be a full hex sha without -dirty"
    fi
    count_is real_clean_three_scanned "${OUT_F}" "scanned: " 3

    # 追跡済みのファイルを触ると -dirty が付く
    printf '%s\n' '// touched' >>"${rw}/ws/crates/upper/src/lib.rs"
    real_case real_dirty 0 "o:ok: " "!violation"
    if grep -Eq '^record: commit=[0-9a-f]{40,64}-dirty ' "${OUT_F}"; then
        report_ok real_dirty_suffix
    else
        report_fail real_dirty_suffix "commit must end with -dirty"
    fi
    real_git checkout -- . >/dev/null 2>&1 || true

    # (1) unsafe
    RS_LIB=""
    real_state "" || report_fail real_no_forbid_setup "cannot prepare"
    real_case real_no_forbid 1 "e:violation: check=forbid file=src/lib.rs" "!ok: " "!error:"
    RS_TEST='fn f() { unsafe {} }'
    real_state "" || report_fail real_unsafe_tests_setup "cannot prepare"
    real_case real_unsafe_tests 1 "e:violation: check=unsafe file=tests/t.rs line=1" "!check=forbid" "!ok: " "!error:"

    # (2)(3) チップの crate と許可外の依存
    real_state '[dependencies.ash]
path = "../../../ext/ash"' || report_fail real_chip_dep_normal_setup "cannot prepare"
    real_case real_chip_dep_normal 1 "e:violation: check=chip-dep crate=ash kind=normal" \
        "e:violation: check=dep-allow crate=ash kind=normal" "e:FAILED: 2 violation(s)" "!ok: "
    real_state '[build-dependencies]
ash = { path = "../../../ext/ash" }' || report_fail real_chip_dep_build_setup "cannot prepare"
    real_case real_chip_dep_build 1 "e:violation: check=chip-dep crate=ash kind=build" \
        "e:violation: check=dep-allow crate=ash kind=build" "!ok: "
    real_state '[dev-dependencies]
ash = { path = "../../../ext/ash" }' || report_fail real_chip_dep_dev_setup "cannot prepare"
    real_case real_chip_dep_dev 1 "e:violation: check=chip-dep crate=ash kind=dev" \
        "e:violation: check=dep-allow crate=ash kind=dev" "!ok: "
    real_state 'objc2-metal = { path = "../../../ext/objc2-metal" }' || report_fail real_chip_dep_metal_setup "cannot prepare"
    real_case real_chip_dep_metal 1 "e:violation: check=chip-dep crate=objc2-metal kind=normal" "!ok: "
    real_state '[dependencies.vk]
package = "ash"
path = "../../../ext/ash"' || report_fail real_chip_dep_renamed_setup "cannot prepare"
    real_case real_chip_dep_renamed 1 "e:violation: check=chip-dep crate=ash kind=normal" "!ok: "
    RS_SRC_X='use ash::x;'
    real_state "" || report_fail real_chip_use_setup "cannot prepare"
    real_case real_chip_use 1 "e:violation: check=chip-path file=src/x.rs line=1 crate=ash" "!check=chip-dep" "!ok: "
    real_state '[dev-dependencies]
plain-dummy = { path = "../../../ext/plain-dummy" }' || report_fail real_other_dep_dev_setup "cannot prepare"
    real_case real_other_dep_dev 1 "e:violation: check=dep-allow crate=plain-dummy kind=dev" "!check=chip-dep" "!ok: "
    real_state '[build-dependencies]
plain-dummy = { path = "../../../ext/plain-dummy" }' || report_fail real_other_dep_build_setup "cannot prepare"
    real_case real_other_dep_build 1 "e:violation: check=dep-allow crate=plain-dummy kind=build" "!ok: "
    real_state 'fandhe-silicon-contract = { path = "../contract" }' || report_fail real_contract_dep_setup "cannot prepare"
    real_case real_contract_dep_upper 1 "e:violation: check=dep-allow crate=fandhe-silicon-contract kind=normal" "!ok: "

    # (4) 測定の feature
    real_state "" "${HARNESS_CONTRACT}" || report_fail real_measure_ok_setup "cannot prepare"
    real_case real_measure_ok 0 "o:ok: " "o:contract_dependents=1" "o:measure_contract_pkgs=1" "!violation" "!error:"
    real_state "" '[dependencies]
fandhe-silicon-contract = { path = "../../crates/contract", features = ["test-support"] }' \
        || report_fail real_measure_test_support_setup "cannot prepare"
    real_case real_measure_test_support 1 \
        "e:violation: check=measure-feature package=harness kind=normal feature=test-support" "!ok: "
    real_state "" '[dependencies]
fandhe-silicon-contract = { path = "../../crates/contract", features = ["fault-injection"] }' \
        || report_fail real_measure_fault_injection_setup "cannot prepare"
    real_case real_measure_fault_injection 1 \
        "e:violation: check=measure-feature package=harness kind=normal feature=fault-injection" "!ok: "
    real_state "" "${HARNESS_CONTRACT}
[features]
x = [\"fandhe-silicon-contract/test-support\"]" || report_fail real_measure_forward_setup "cannot prepare"
    real_case real_measure_forward 1 \
        "e:violation: check=measure-feature package=harness kind=normal feature=test-support" "!ok: "
    real_state "" '[dev-dependencies]
fandhe-silicon-contract = { path = "../../crates/contract", features = ["test-support"] }' \
        || report_fail real_measure_dev_setup "cannot prepare"
    real_case real_measure_dev 1 \
        "e:violation: check=measure-feature package=harness kind=dev feature=test-support" "!ok: "
    real_state "" '[build-dependencies]
fandhe-silicon-contract = { path = "../../crates/contract", features = ["fault-injection"] }' \
        || report_fail real_measure_build_setup "cannot prepare"
    real_case real_measure_build 1 \
        "e:violation: check=measure-feature package=harness kind=build feature=fault-injection" "!ok: "

    # 実物の cargo metadata の出力をファイルに落として --metadata-json で渡しても同じ判定になる（commit は `-`）。
    real_files() {
        local n="$1" w="$2"
        shift 2
        if (cd "${rw}/ws" \
            && cargo metadata --locked --offline --no-deps --format-version 1 --all-features >"${rw}/md.json" \
            && cargo metadata --locked --offline --no-deps --format-version 1 --all-features \
                --manifest-path measure/Cargo.toml >"${rw}/mm.json") 2>"${ERR_F}"; then
            rc=0
            "${rw}/ws/scripts/upper-layer-check.sh" --metadata-json "${rw}/md.json" --measure-metadata-json "${rw}/mm.json" \
                >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
            judge "${n}" "${w}" "o:record: commit=- " "!${rw}" "$@"
        else
            report_fail "${n}" "cannot produce cargo metadata"
        fi
    }
    real_state "" "${HARNESS_CONTRACT}" || report_fail real_files_ok_setup "cannot prepare"
    real_files real_files_ok 0 "o:ok: " "o:contract_dependents=1" "!violation"
    real_state "" '[dependencies]
fandhe-silicon-contract = { path = "../../crates/contract", features = ["test-support"] }' \
        || report_fail real_files_bad_setup "cannot prepare"
    real_files real_files_bad 1 "e:violation: check=measure-feature package=harness kind=normal feature=test-support" "!ok: "

    # 測定の workspace の manifest が無い（前提の不足）
    real_state "" || report_fail real_measure_missing_setup "cannot prepare"
    rm -f "${rw}/ws/measure/Cargo.toml"
    if ! { real_git add -A && real_git commit -q -m remove-measure; } >/dev/null 2>&1; then
        report_fail real_measure_missing_setup "cannot commit"
    fi
    real_case real_measure_missing 2 "e:error: the measure workspace manifest is not readable" "!ok: " "!record: "
else
    report_fail real_workspace_setup "cannot prepare the temporary workspace"
fi

# ---- 実物のリポジトリ ----
# 本リポの crates/upper と measure/ に引数なしで当てる。起きることは git の読み取り（rev-parse・status）と
# cargo metadata --locked --offline --no-deps だけで、リポジトリのファイルは書かない。
# 確かめるのは合格・ok の行・record の行・違反なしだけ。件数と commit の値は固定しない（作業ツリーが dirty でもよい）。
rc=0
"${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge real_repo 0 "o:ok: " "o:record: " "!violation" "!error:" "!${ROOT_DIR}"

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
