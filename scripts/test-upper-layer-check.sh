#!/usr/bin/env bash
# upper-layer-check.sh の回帰テスト（spec TASK-15 のポインタ。親 Issue #367 の 1/4）。
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
    cat >"${d}/md.json" <<JSON
{"workspace_root":"${d}","workspace_members":["u"],"packages":[{"id":"u","manifest_path":"${d}/crates/upper/Cargo.toml","dependencies":[],"targets":[{"kind":["lib"],"src_path":"${d}/crates/upper/src/lib.rs"}]}]}
JSON
    "${CHECK}" --metadata-json "${d}/md.json" >"${d}/out" 2>"${d}/err" || rc=$?
    if [ "${rc}" -eq "${want}" ]; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        echo "FAIL: ${name}: exit ${rc} (want ${want})" >&2
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

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
