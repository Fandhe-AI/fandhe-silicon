#!/usr/bin/env bash
# wasm-build-check.sh の回帰テスト（spec TASK-237・BUILD-21・BUILD-25 のポインタ。親 Issue #462 の 3/4）。
# 合成の wasm・合成の rustdoc JSON・合成のソース・PATH の先頭に置く偽の cargo で各行の判定を確かめ、
# 最後に一時 workspace（本体と lib.sh を写したもの）で実物の cargo による 2 版のビルドと rustdoc を通す。
# 型の名前はダミーだけを使い、リポジトリのファイルは変更しない・ネットワーク不要。
# 呼び出し元: `make test-wasm-build-check`（直接実行も可。手元で流す。CI への組み込みは後続の sub-issue）。
# 前提: python3・cargo・wasm32-unknown-unknown の target（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う（先頭が `!` の語は含まれないことの確認）。
# 期待値を緩めない。途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd cargo

TOOL="${DIR}/wasm-build-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-wasm-build-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

mkdir "${work}/f" "${work}/src"
# 合成の入力: wasm（custom section target_features を手で組む）、rustdoc JSON、ソース。
python3 -I - "${work}" <<'PY'
import json
import os
import sys

out = sys.argv[1]
HEADER = b"\x00asm\x01\x00\x00\x00"


def leb(n):
    r = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            r.append(b | 0x80)
        else:
            r.append(b)
            return bytes(r)


def custom(name, payload):
    body = leb(len(name)) + name + payload
    return b"\x00" + leb(len(body)) + body


def features(items):
    payload = leb(len(items))
    for it in items:
        payload += b"+" + leb(len(it)) + it
    return custom(b"target_features", payload)


TYPE_SEC = b"\x01\x01\x00"
BASE = [b"bulk-memory", b"mutable-globals"]


def put(rel, data, mode="wb"):
    with open(os.path.join(out, rel), mode) as fh:
        fh.write(data)


put("f/scalar.wasm", HEADER + TYPE_SEC + features(BASE))
put("f/simd.wasm", HEADER + TYPE_SEC + features(BASE + [b"simd128"]))
put("f/nosection.wasm", HEADER + TYPE_SEC)
put("f/badmagic.wasm", b"\x00asn\x01\x00\x00\x00" + TYPE_SEC)
put("f/badleb.wasm", HEADER + b"\x00\x80\x80\x80\x80\x80\x00")
put("f/trunc.wasm", HEADER + b"\x00\x7f\x00")
put("f/dup.wasm", HEADER + features(BASE) + features(BASE))
put("f/empty.wasm", b"")

doc = {
    "root": 0,
    "paths": {
        "1": {"crate_id": 0, "path": ["fandhe_silicon_wasm_abi", "dummy_mod", "Alpha"], "kind": "struct"},
        "2": {"crate_id": 1, "path": ["core", "option", "Option"], "kind": "enum"},
        "3": {"crate_id": 0, "path": ["fandhe_silicon_wasm_abi", "dummy_mod", "Hidden"], "kind": "struct"},
        "4": {"crate_id": 0, "path": ["fandhe_silicon_wasm_abi", "dummy_mod", "make"], "kind": "function"},
        "5": {"crate_id": 0, "path": ["fandhe_silicon_wasm_abi", "dummy_mod"], "kind": "module"},
        "6": {"crate_id": 0, "path": ["fandhe_silicon_wasm_abi", "dummy_mod", "Kind"], "kind": "enum"},
        "7": {"crate_id": 0, "path": ["fandhe_silicon_wasm_abi", "dummy_mod", "Shape"], "kind": "trait"},
        "8": {"crate_id": 0, "path": ["fandhe_silicon_wasm_abi", "dummy_mod", "PathsOnly"], "kind": "struct"},
    },
    "index": {"1": {"visibility": "public"}, "3": {"visibility": "crate"}, "4": {"visibility": "public"},
              "5": {"visibility": "public"}, "6": {"visibility": "public"}, "7": {"visibility": "public"}},
}
put("f/doc.json", json.dumps(doc), "w")
put("f/baddoc.json", "{}", "w")
put("f/notjson.json", "not json", "w")

GOOD = 'cfg(all(target_arch = "wasm32", target_feature = "simd128"))'
srcs = {
    "good": '#[%s]\npub fn a() {}\n' % GOOD,
    "good_multiline": '#[cfg(\n    all(target_arch = "wasm32",\n        target_feature = "simd128")\n)]\npub fn a() {}\n',
    "not_form": '#[%s]\npub fn a() {}\n#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]\npub fn a() {}\n' % GOOD,
    "only_feature": '#[cfg(target_feature = "simd128")]\npub fn a() {}\n',
    "any_form": '#[cfg(any(target_arch = "wasm32", target_feature = "simd128"))]\npub fn a() {}\n',
    "runtime_spaced": '#[%s]\npub fn a() {}\npub fn b() { if is_x86_feature_detected\n  ! (\n"simd128") {} }\n' % GOOD,
    "debug_cfg": '#[%s]\npub fn a() {}\n#[cfg(debug_assertions)]\npub struct Alpha;\n' % GOOD,
    "runtime": '#[%s]\npub fn a() {}\npub fn b() { if is_x86_feature_detected!("simd128") {} }\n' % GOOD,
    "comments_only": '//! target_feature = "simd128" の話\n// target_feature = "simd128"\n/* target_feature = "simd128" */\n/// target_feature = "simd128"\npub fn a() {}\n',
    "none": 'pub fn a() {}\n',
    "strings_only": 'pub const A: &str = "target_feature = \\"simd128\\"";\npub const B: &str = r#"#[cfg(target_feature = "simd128")] x"#;\n'
                    'pub const C: &str = "is_x86_feature_detected!(\\"simd128\\")";\npub const D: char = \'"\';\n',
    "string_then_real": 'pub const A: &str = r#"target_feature = "simd128""#;\npub const D: char = \'"\';\n#[%s]\npub fn a() {}\n' % GOOD,
    "string_then_bad": 'pub const A: &str = "cfg(all(target_arch = \\"wasm32\\", target_feature = \\"simd128\\"))";\n#[cfg(target_feature = "simd128")]\npub fn a() {}\n',
}
for name, text in srcs.items():
    os.makedirs(os.path.join(out, "src", name))
    put("src/%s/x.rs" % name, text, "w")
PY

# 偽の cargo。引数と rustflags 系の環境を FAKE_LOG へ残し、metadata / build / rustdoc を合成の出力で済ませる。
mkdir "${work}/bin" "${work}/td"
cat >"${work}/bin/cargo" <<'EOF'
#!/usr/bin/env bash
echo "ARGS $*" >>"${FAKE_LOG}"
echo "RUSTFLAGS=${RUSTFLAGS-unset} ENC=$(printf %s "${CARGO_ENCODED_RUSTFLAGS-unset}" | tr '\037' '|') BOOT=${RUSTC_BOOTSTRAP-unset}" >>"${FAKE_LOG}"
echo "RDENC=$(printf %s "${CARGO_ENCODED_RUSTDOCFLAGS-unset}" | tr '\037' '|') BOOT=${RUSTC_BOOTSTRAP-unset}" >>"${FAKE_LOG}"
td=""
prev=""
for a in "$@"; do
  if [ "${prev}" = "--target-dir" ]; then td="${a}"; fi
  prev="${a}"
done
case "$1" in
  metadata) printf '{"target_directory":"%s"}\n' "${FAKE_TD}" ;;
  build)
    if [ -n "${FAKE_FAIL-}" ]; then exit 101; fi
    mkdir -p "${td}/wasm32-unknown-unknown/release"
    case "${td}" in
      */scalar) cp "${FAKE_SCALAR}" "${td}/wasm32-unknown-unknown/release/fandhe_silicon_wasm_abi.wasm" ;;
      *) cp "${FAKE_SIMD}" "${td}/wasm32-unknown-unknown/release/fandhe_silicon_wasm_abi.wasm" ;;
    esac ;;
  rustdoc)
    mkdir -p "${td}/wasm32-unknown-unknown/doc"
    cp "${FAKE_JSON}" "${td}/wasm32-unknown-unknown/doc/fandhe_silicon_wasm_abi.json" ;;
esac
EOF
chmod +x "${work}/bin/cargo"

# run_case <name> <args...>: ツールを流し、終了コードを rc に、出力を OUT_F / ERR_F に残す。
run_case() {
  rc=0
  "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
}

# run_fake <args...>: 偽の cargo を PATH の先頭に入れて流す。
run_fake() {
  rc=0
  : >"${work}/fake.log"
  PATH="${work}/bin:${PATH}" FAKE_LOG="${work}/fake.log" FAKE_TD="${work}/td" \
    FAKE_SCALAR="${work}/f/scalar.wasm" FAKE_SIMD="${work}/f/simd.wasm" FAKE_JSON="${work}/f/doc.json" \
    RUSTFLAGS="-C target-cpu=host" CARGO_ENCODED_RUSTFLAGS="-Cdebuginfo=2" \
    "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
}

# expect <name> <終了コード> <語>...: 語は標準出力か標準エラーに含まれること。先頭が `!` の語は含まれないこと。
expect() {
  local name="$1" want_rc="$2" word ok=1
  shift 2
  if [ "${rc}" -ne "${want_rc}" ]; then
    echo "FAIL ${name}: exit ${rc}, expected ${want_rc}" >&2
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
  # どの出力にも一時ディレクトリの絶対パスを出さない。
  if grep -q -F -e "${work}" "${OUT_F}" "${ERR_F}"; then
    echo "FAIL ${name}: host path in output" >&2
    ok=0
  fi
  if [ "${ok}" -eq 1 ]; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    sed 's/^/  out: /' "${OUT_F}" >&2
    sed 's/^/  err: /' "${ERR_F}" >&2
  fi
}

# expect_log <name> <語>...: 偽の cargo の記録に語が含まれること（先頭が `!` なら含まれないこと）。
expect_log() {
  local name="$1" word ok=1
  shift
  for word in "$@"; do
    case "${word}" in
      '!'*)
        if grep -q -F -e "${word#!}" "${work}/fake.log"; then
          echo "FAIL ${name}: unexpected log text: ${word#!}" >&2
          ok=0
        fi
        ;;
      *)
        if ! grep -q -F -e "${word}" "${work}/fake.log"; then
          echo "FAIL ${name}: missing log text: ${word}" >&2
          ok=0
        fi
        ;;
    esac
  done
  if [ "${ok}" -eq 1 ]; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    sed 's/^/  log: /' "${work}/fake.log" >&2
  fi
}

F="${work}/f"
S="${work}/src"

# 1. 成果物の判別（--from-dir。型は --require-type 無しで not-run、cfg は none の起点で not-run になるので、ここでは artifact の行だけを見る）
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/none"
expect "artifact-ok" 1 "artifact: kind=scalar label=match simd128=absent" "artifact: kind=simd128 label=match simd128=present" \
  "record: commit=" "artifact=2/2"
run_case --from-dir "${F}/scalar.wasm" "${F}/scalar.wasm.copy" --source-root "${S}/none"
expect "artifact-missing-file" 2 "error:" "!record:"
cp "${F}/scalar.wasm" "${F}/scalar.wasm.copy"
run_case --from-dir "${F}/scalar.wasm" "${F}/scalar.wasm.copy" --source-root "${S}/none"
expect "artifact-identical" 1 "artifact: kind=scalar label=mismatch" "reason=identical-artifacts" "artifact=0/2"
run_case --from-dir "${F}/scalar.wasm" "${F}/scalar.wasm" --source-root "${S}/none"
expect "artifact-same-path" 2 "error:" "!record:"
run_case --from-dir "${F}/scalar.wasm" "${F}/nosection.wasm" --source-root "${S}/none"
expect "artifact-simd-lacks-flag" 1 "artifact: kind=simd128 label=mismatch simd128=absent reason=missing-target-features" "artifact=1/2"
run_case --from-dir "${F}/nosection.wasm" "${F}/simd.wasm" --source-root "${S}/none"
expect "artifact-scalar-no-section" 1 "artifact: kind=scalar label=mismatch" "reason=missing-target-features" "artifact: kind=simd128 label=match"
run_case --from-dir "${F}/simd.wasm" "${F}/scalar.wasm" --source-root "${S}/none"
expect "artifact-swapped" 1 "artifact: kind=scalar label=mismatch simd128=present" "artifact: kind=simd128 label=mismatch simd128=absent" "artifact=0/2"
run_case --from-dir "${F}/badmagic.wasm" "${F}/simd.wasm" --source-root "${S}/none"
expect "artifact-bad-magic" 2 "error: not a wasm module" "!record:"
run_case --from-dir "${F}/scalar.wasm" "${F}/badleb.wasm" --source-root "${S}/none"
expect "artifact-bad-leb" 2 "error: malformed LEB128" "!record:"
run_case --from-dir "${F}/scalar.wasm" "${F}/trunc.wasm" --source-root "${S}/none"
expect "artifact-truncated-section" 2 "error:" "!record:"
run_case --from-dir "${F}/scalar.wasm" "${F}/dup.wasm" --source-root "${S}/none"
expect "artifact-duplicate-section" 2 "error: duplicate target_features section"
run_case --from-dir "${F}/scalar.wasm" "${F}/empty.wasm" --source-root "${S}/none"
expect "artifact-empty-file" 2 "error: empty wasm input"

# 2. 型の確かめ（合成 rustdoc JSON とダミーの名前）
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
  --require-type dummy_mod::Alpha
expect "type-present" 0 "type: name=dummy_mod::Alpha label=match" "cfg: sites=1 label=match" "types=1/1" "ok:" "!FAILED"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
  --require-type fandhe_silicon_wasm_abi::dummy_mod::Alpha
expect "type-present-full-path" 0 "label=match" "types=1/1"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
  --require-type dummy_mod::Alpha --require-type dummy_mod::Missing
expect "type-one-missing" 1 "type: name=dummy_mod::Missing label=mismatch" "types=1/2" "FAILED:"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
  --require-type dummy_mod::Hidden
expect "type-non-public" 1 "type: name=dummy_mod::Hidden label=mismatch"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
  --require-type dummy_mod::PathsOnly
expect "type-paths-only-not-in-index" 1 "type: name=dummy_mod::PathsOnly label=mismatch"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
  --require-type core::option::Option
expect "type-external-crate" 1 "label=mismatch"
for n in make Kind Shape; do
  run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
    --require-type "dummy_mod::${n}"
  case "${n}" in
    make) expect "type-kind-function-is-not-type" 1 "type: name=dummy_mod::make label=mismatch build=scalar" "types=0/1" ;;
    *) expect "type-kind-${n}-ok" 0 "type: name=dummy_mod::${n} label=match build=simd128" "types=1/1" ;;
  esac
done
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json" \
  --require-type dummy_mod
expect "type-kind-module-is-not-type" 1 "type: name=dummy_mod label=mismatch build=scalar" "types=0/1"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/doc.json"
expect "type-none-required" 1 "type: label=not-run reason=no-required-types" "artifact: kind=simd128 label=match" "not_run=1" "!ok:"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/baddoc.json" \
  --require-type dummy_mod::Alpha
expect "type-bad-shape" 2 "error: cannot read rustdoc JSON" "!record:"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/notjson.json" \
  --require-type dummy_mod::Alpha
expect "type-not-json" 2 "error: cannot read rustdoc JSON"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/good" --rustdoc-json "${F}/missing.json" \
  --require-type dummy_mod::Alpha
expect "type-json-missing" 2 "error: cannot read rustdoc JSON"

# 3. cfg の確かめ（合成のソース）。型は成立させ、cfg の行だけが結果を変えるようにする。
cfg_case() {
  run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/$1" --rustdoc-json "${F}/doc.json" \
    --require-type dummy_mod::Alpha
}
cfg_case good_multiline
expect "cfg-multiline-ok" 0 "cfg: sites=1 label=match" "ok:"
cfg_case not_form
expect "cfg-not-all-form-ok" 0 "cfg: sites=2 label=match" "ok:"
cfg_case only_feature
expect "cfg-only-feature" 1 "cfg: sites=1 label=mismatch reason=unsupported-cfg-form" "violation: file=" "x.rs line=1 reason=unsupported-cfg-form"
cfg_case any_form
expect "cfg-any-form" 1 "cfg: sites=1 label=mismatch" "reason=unsupported-cfg-form"
cfg_case runtime
expect "cfg-runtime-detection" 1 "label=mismatch reason=runtime-detection" "line=3 reason=runtime-detection"
cfg_case runtime_spaced
expect "cfg-runtime-detection-spaced" 1 "label=mismatch reason=runtime-detection" "line=3 reason=runtime-detection"
cfg_case debug_cfg
expect "cfg-debug-assertions" 1 "label=mismatch reason=debug-assertions-cfg" "line=3 reason=debug-assertions-cfg"
cfg_case strings_only
expect "cfg-strings-only" 1 "cfg: sites=0 label=not-run reason=no-simd128-sites" "!violation:"
cfg_case string_then_real
expect "cfg-string-then-real" 0 "cfg: sites=1 label=match" "ok:"
cfg_case string_then_bad
expect "cfg-string-then-bad" 1 "cfg: sites=1 label=mismatch reason=unsupported-cfg-form" "line=2 reason=unsupported-cfg-form"
cfg_case comments_only
expect "cfg-comments-only" 1 "cfg: sites=0 label=not-run reason=no-simd128-sites" "!violation:"
cfg_case none
expect "cfg-zero-sites" 1 "cfg: sites=0 label=not-run reason=no-simd128-sites" "cfg_sites=0" "not_run=1"
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm" --source-root "${S}/missing_dir" --rustdoc-json "${F}/doc.json" \
  --require-type dummy_mod::Alpha
expect "cfg-source-root-missing" 2 "error: source root is not a directory"

# 実物のリポジトリの crates/ の `//!` にある simd128 の語は、コメントなので数えない。
run_case --from-dir "${F}/scalar.wasm" "${F}/simd.wasm"
expect "cfg-real-crates-comments-not-counted" 1 "cfg: sites=0 label=not-run reason=no-simd128-sites" "!violation:"

# 4. 偽の cargo（既定の経路）
run_fake --source-root "${S}/good"
expect "fake-default" 1 "artifact: kind=scalar label=match" "artifact: kind=simd128 label=match" "type: label=not-run reason=no-required-types" "cfg: sites=1 label=match"
expect_log "fake-scalar-flags" "ENC=unset" "!ENC=-Cdebuginfo=2" "!RUSTFLAGS=-C" "!BOOT=1"
expect_log "fake-simd-flags" "ENC=-C|target-feature=+simd128"
expect_log "fake-target-dirs" "/wasm-build-check/scalar" "/wasm-build-check/simd128" "--locked" "--target wasm32-unknown-unknown" \
  "--no-default-features --features contract-core,cpu-isa,cpu-ops"
expect_log "fake-no-rustdoc-without-types" "!rustdoc"
run_fake --source-root "${S}/good" --require-type dummy_mod::Alpha
expect "fake-with-types" 0 "type: name=dummy_mod::Alpha label=match" "ok:"
expect_log "fake-rustdoc-bootstrap" "ARGS rustdoc" "BOOT=1" "--output-format json" "--target wasm32-unknown-unknown" "/wasm-build-check/rustdoc-scalar" \
  "/wasm-build-check/rustdoc-simd128"
expect_log "fake-rustdoc-release" "ARGS rustdoc" "--release"
expect_log "fake-rustdoc-simd-flags" "RDENC=-C|target-feature=+simd128"
rc=0
FAKE_FAIL=1 PATH="${work}/bin:${PATH}" FAKE_LOG="${work}/fake.log" FAKE_TD="${work}/td" "${TOOL}" --source-root "${S}/good" \
  >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "fake-build-fails" 2 "error: cargo build failed for the scalar artifact" "!record:"

# 6. 誤用
run_case --bogus
expect "misuse-unknown-arg" 2 "usage:"
run_case --from-dir "${F}/scalar.wasm"
expect "misuse-from-dir-missing-arg" 2 "usage:"
run_case --require-type
expect "misuse-require-type-no-value" 2 "usage:"

# 5. 実物の 1 組: 本体と lib.sh を一時 workspace へ写し、実物の cargo で 2 版のビルドと rustdoc を通す。
ws="${work}/ws"
mkdir -p "${ws}/scripts" "${ws}/crates/wasm-abi/src"
cp "${DIR}/wasm-build-check.sh" "${DIR}/lib.sh" "${ws}/scripts/"
cat >"${ws}/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/wasm-abi"]
resolver = "2"
EOF
cat >"${ws}/crates/wasm-abi/Cargo.toml" <<'EOF'
[package]
name = "fandhe-silicon-wasm-abi"
version = "0.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[features]
contract-core = []
cpu-isa = []
cpu-ops = []
EOF
cat >"${ws}/crates/wasm-abi/src/lib.rs" <<'EOF'
//! 実物の確かめ用のダミー crate。
pub mod dummy_mod {
    /// ダミーの公開型。
    pub struct Alpha;
}

/// 命令の有無で在る型が変わるダミー。
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub struct SimdOnly;

/// scalar 版にだけ在るダミーの型。
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
pub struct ScalarOnly;

/// 命令の有無をビルド時の cfg で決めるダミーの関数。
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub fn lanes() -> u32 {
    4
}

/// scalar 版のダミーの関数。
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
pub fn lanes() -> u32 {
    1
}
EOF
(cd "${ws}" && cargo generate-lockfile --offline >/dev/null 2>&1)
rc=0
(cd "${ws}" && env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS scripts/wasm-build-check.sh --require-type dummy_mod::Alpha) \
  >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "real-all-match" 0 "artifact: kind=scalar label=match simd128=absent" "artifact: kind=simd128 label=match simd128=present" \
  "type: name=dummy_mod::Alpha label=match" "cfg: sites=4 label=match" "artifact=2/2 types=1/1 cfg_sites=4 not_run=0" "ok:"
rc=0
(cd "${ws}" && env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS scripts/wasm-build-check.sh --require-type dummy_mod::Nope) \
  >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "real-type-missing" 1 "type: name=dummy_mod::Nope label=mismatch" "FAILED:"
rc=0
(cd "${ws}" && env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS scripts/wasm-build-check.sh --require-type SimdOnly --require-type ScalarOnly) \
  >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "real-cfg-split-types" 1 "type: name=SimdOnly label=match build=simd128" "type: name=SimdOnly label=mismatch build=scalar" \
  "type: name=ScalarOnly label=match build=scalar" "type: name=ScalarOnly label=mismatch build=simd128" "types=0/2"

echo "test-wasm-build-check: ${passed} passed, ${failed} failed" >&2
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
