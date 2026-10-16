#!/usr/bin/env bash
# upper-layer-check.sh の回帰テスト（spec TASK-15 のポインタ。親 Issue #367 の 1/4・2/4）。
# 一時ディレクトリに合成の crate と metadata の JSON を作り、--metadata-json で渡して終了コードを確かめる。
# cargo・git を呼ばず、リポジトリのファイルを変更しない。
# 呼び出し元: 直接実行のみ（make ターゲットと CI への組み込みは後の段）。
# 途中で止めず、最後に件数を出す。bash 3.2 互換。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
require_cmd python3

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
LAST_DIR=""

# run_case <名前> <期待する終了コード> <lib.rs の内容> [追加ファイル src/x.rs の内容]
run_case() {
    local name="$1" want="$2" lib="$3" extra="${4:-}" d rc=0
    d="${work}/${name}"
    mkdir -p "${d}/crates/upper/src"
    printf '[package]\nname = "u"\nversion = "0.1.0"\n' >"${d}/crates/upper/Cargo.toml"
    printf '%s\n' "${lib}" >"${d}/crates/upper/src/lib.rs"
    if [ -n "${extra}" ]; then
        printf '%s\n' "${extra}" >"${d}/crates/upper/src/x.rs"
    fi
    local contract="" members='["u"]'
    if [ -n "${CONTRACT_PKG}" ]; then
        contract=",${CONTRACT_PKG//@D@/${d}}"
        members='["u","c"]'
    fi
    cat >"${d}/md.json" <<JSON
{"workspace_root":"${d}","workspace_members":${members},"packages":[{"id":"u","name":"u","manifest_path":"${d}/crates/upper/Cargo.toml","dependencies":${UPPER_DEPS},"targets":[{"kind":["lib"],"src_path":"${d}/crates/upper/src/lib.rs"}]}${contract}]}
JSON
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
UPPER_DEPS="[$(dep fandhe-silicon-core),$(dep fandhe-silicon-contract '"build"'),$(dep fandhe-silicon-exec '"dev"')]"
run_case allow-ok 0 "${FB}"
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

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
