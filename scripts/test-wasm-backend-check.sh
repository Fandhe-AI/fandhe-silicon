#!/usr/bin/env bash
# wasm-backend-check.sh の回帰テスト（spec TASK-238・BUILD-22・BUILD-23 のポインタ。親 Issue #1155 の 1/2）。
# 合成の rustdoc JSON・合成の tree・PATH の先頭に置く偽の cargo / rustup で各行の判定を確かめ、
# 最後に一時 workspace（本体と lib.sh と契約の crate の写し）で実物の cargo による検査と陽性対照を通す。
# リポジトリのファイルは変更しない・ネットワーク不要。
# 呼び出し元: `make test-wasm-backend-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
# 前提: python3・cargo・git・wasm32-unknown-unknown の target（無ければ失敗にし、skip しない）。
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
require_cmd git

TOOL="${DIR}/wasm-backend-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-wasm-backend-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

mkdir "${work}/f" "${work}/bin"
# 合成の入力: rustdoc JSON（モジュールの有無・公開の子）と tree。
python3 -I - "${work}/f" <<'PY'
import json
import os
import sys

out = sys.argv[1]


def doc(modules):
    """modules: [(name, [(child_name, visibility)])]。root の module に並べる。"""
    index = {"0": {"name": "c", "inner": {"module": {"items": []}}}}
    nid = 1
    for name, children in modules:
        cids = []
        for cname, vis in children:
            index[str(nid)] = {"name": cname, "visibility": vis, "inner": {"function": {}}}
            cids.append(nid)
            nid += 1
        index[str(nid)] = {"name": name, "visibility": "public", "inner": {"module": {"items": cids}}}
        index["0"]["inner"]["module"]["items"].append(nid)
        nid += 1
    return {"root": 0, "index": index}


def put(name, text):
    with open(os.path.join(out, name), "w") as fh:
        fh.write(text)


def putj(name, obj):
    put(name, json.dumps(obj))


putj("metal-ok.json", doc([("backend_metal", [])]))
putj("metal-child.json", doc([("backend_metal", [("f", "public")])]))
putj("metal-private-child.json", doc([("backend_metal", [("f", "crate")])]))
putj("none.json", doc([]))
putj("vulkan-present.json", doc([("backend_vulkan", [])]))
putj("cuda-present.json", doc([("backend_cuda", [])]))
putj("hidden-other.json", doc([("cfg_gate_doctest", [("g", "public")])]))
putj("no-root.json", {"index": {}})
putj("no-index.json", {"root": 0})
putj("root-not-module.json", {"root": 0, "index": {"0": {"name": "c", "inner": {"function": {}}}}})
put("not-json.json", "not json")


def tree(extra=()):
    lines = ["fandhe-silicon-contract v0.0.0 (/x/crates/contract)", "fandhe-silicon-core v0.0.0 (/x/crates/core)"]
    lines += list(extra)
    return "\n".join(lines) + "\n"


put("tree-clean.txt", tree())
put("tree-ash.txt", tree(["ash v0.38.0"]))
put("tree-ash-upper.txt", tree(["ASH v0.38.0"]))
put("tree-ash-sys.txt", tree(["ash_window v0.13.0", "cudarc-sys v0.1.0", "objc2-metal-extra v0.3.0"]))
put("tree-metal-underscore.txt", tree(["objc2_metal v0.3.0"]))
put("tree-native.txt", tree(["ash v0.38.0", "cudarc v0.12.0"]))
put("tree-no-crate.txt", "fandhe-silicon-core v0.0.0 (/x/crates/core)\n")
put("tree-empty.txt", "")
PY

# expect <name> <rc> <word>...: 直前の実行（OUT_F / ERR_F / rc）を確かめる。
expect() {
  local name="$1" want="$2" word ok=1
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    ok=0
    echo "FAIL ${name}: exit ${rc}, want ${want}" >&2
  fi
  for word in "$@"; do
    case "${word}" in
      '!'*)
        if cat "${OUT_F}" "${ERR_F}" | grep -q -F -e "${word#!}"; then
          ok=0
          echo "FAIL ${name}: unexpected: ${word#!}" >&2
        fi
        ;;
      *)
        if ! cat "${OUT_F}" "${ERR_F}" | grep -q -F -e "${word}"; then
          ok=0
          echo "FAIL ${name}: missing: ${word}" >&2
        fi
        ;;
    esac
  done
  if [ "${ok}" -eq 1 ]; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    sed 's/^/  out: /' "${OUT_F}" >&2
    sed 's/^/  err: /' "${ERR_F}" >&2
  fi
}

# run_offline <metal.json> <vulkan.json> <cuda.json> <wasm tree> <native tree>
run_offline() {
  local f="${work}/f"
  set +e
  "${TOOL}" --rustdoc-json "backend-metal=${f}/$1" --rustdoc-json "backend-vulkan=${f}/$2" --rustdoc-json "backend-cuda=${f}/$3" \
    --tree-text "backend-metal=${f}/$4" --tree-text "backend-vulkan=${f}/$4" --tree-text "backend-cuda=${f}/$4" \
    --native-tree-text "backend-metal=${f}/$5" --native-tree-text "backend-vulkan=${f}/$5" --native-tree-text "backend-cuda=${f}/$5" \
    >"${OUT_F}" 2>"${ERR_F}"
  rc=$?
  set -e
}

# run_args <args...>: 引数をそのまま渡す。
run_args() {
  set +e
  "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}"
  rc=$?
  set -e
}

# 1. 合成の入力（オフラインの経路）。check: は not-run なので全部合格でも終了コードは 1。
run_offline metal-ok.json none.json none.json tree-clean.txt tree-clean.txt
expect "offline-all-match-but-check-not-run" 1 "check: feature=backend-metal target=wasm32-unknown-unknown result=not-run" \
  "module: feature=backend-metal module=backend_metal expect=present-empty found=present public_items=0 result=match" \
  "module: feature=backend-vulkan module=backend_vulkan expect=absent found=absent public_items=- result=match" \
  "deps: feature=backend-cuda name=ash wasm32_n=0 native_n=0 result=match" \
  "record: commit=- configs=3 check=-/3 module=3/3 sdk_wasm32_total=0 sdk_native_total=0" "!ok:"
run_offline metal-child.json none.json none.json tree-clean.txt tree-clean.txt
expect "metal-public-child" 1 "module: feature=backend-metal module=backend_metal expect=present-empty found=present public_items=1 result=mismatch" \
  "violation: module: feature=backend-metal"
run_offline metal-private-child.json none.json none.json tree-clean.txt tree-clean.txt
expect "metal-private-child-not-counted" 1 "public_items=0 result=match" "module=3/3"
run_offline none.json none.json none.json tree-clean.txt tree-clean.txt
expect "metal-missing" 1 "module: feature=backend-metal module=backend_metal expect=present-empty found=absent public_items=- result=mismatch" "module=2/3"
run_offline metal-ok.json vulkan-present.json none.json tree-clean.txt tree-clean.txt
expect "vulkan-present" 1 "module: feature=backend-vulkan module=backend_vulkan expect=absent found=present public_items=0 result=mismatch"
run_offline metal-ok.json none.json cuda-present.json tree-clean.txt tree-clean.txt
expect "cuda-present" 1 "module: feature=backend-cuda module=backend_cuda expect=absent found=present public_items=0 result=mismatch"
run_offline metal-ok.json hidden-other.json none.json tree-clean.txt tree-clean.txt
expect "other-module-not-counted" 1 "module: feature=backend-vulkan module=backend_vulkan expect=absent found=absent public_items=- result=match"
run_offline no-root.json none.json none.json tree-clean.txt tree-clean.txt
expect "json-no-root" 2 "error: cannot read the rustdoc JSON" "!record:"
run_offline no-index.json none.json none.json tree-clean.txt tree-clean.txt
expect "json-no-index" 2 "error: cannot read the rustdoc JSON"
run_offline root-not-module.json none.json none.json tree-clean.txt tree-clean.txt
expect "json-root-not-module" 2 "error: cannot read the rustdoc JSON"
run_offline not-json.json none.json none.json tree-clean.txt tree-clean.txt
expect "json-not-json" 2 "error: cannot read the rustdoc JSON"
run_offline missing.json none.json none.json tree-clean.txt tree-clean.txt
expect "json-missing-file" 2 "error: cannot read an input file"

# 2. 合成の tree。
run_offline metal-ok.json none.json none.json tree-ash.txt tree-clean.txt
expect "tree-ash" 1 "deps: feature=backend-metal name=ash wasm32_n=1 native_n=0 result=mismatch" "sdk_wasm32_total=3"
run_offline metal-ok.json none.json none.json tree-ash-upper.txt tree-clean.txt
expect "tree-ash-case-folded" 1 "name=ash wasm32_n=1"
run_offline metal-ok.json none.json none.json tree-metal-underscore.txt tree-clean.txt
expect "tree-underscore-folded" 1 "name=objc2-metal wasm32_n=1"
run_offline metal-ok.json none.json none.json tree-ash-sys.txt tree-clean.txt
expect "tree-prefix-not-counted" 1 "name=ash wasm32_n=0" "name=cudarc wasm32_n=0" "name=objc2-metal wasm32_n=0" "sdk_wasm32_total=0"
run_offline metal-ok.json none.json none.json tree-clean.txt tree-native.txt
expect "tree-native-only-recorded" 1 "name=ash wasm32_n=0 native_n=1 result=match" "sdk_native_total=6" "sdk_wasm32_total=0"
run_offline metal-ok.json none.json none.json tree-no-crate.txt tree-clean.txt
expect "tree-crate-missing" 2 "error: the checked crate is missing from the dependency tree" "!record:"
run_offline metal-ok.json none.json none.json tree-empty.txt tree-clean.txt
expect "tree-empty" 2 "error: the checked crate is missing from the dependency tree"

# 3. 誤用。
f="${work}/f"
run_args --bogus
expect "misuse-unknown-arg" 2 "usage:"
run_args --rustdoc-json
expect "misuse-no-value" 2 "usage:"
run_args --rustdoc-json "backend-unknown=${f}/none.json"
expect "misuse-unknown-feature" 2 "error: unknown feature name"
run_args --rustdoc-json "backend-metal=${f}/none.json" --rustdoc-json "backend-metal=${f}/none.json"
expect "misuse-duplicate" 2 "error: duplicate feature"
run_args --rustdoc-json "backend-metal=${f}/none.json"
expect "misuse-incomplete" 2 "error: offline mode needs all of"
run_args --rustdoc-json "backend-metal"
expect "misuse-no-equals" 2 "error: expected <feature>=<path>"

# 4. 偽の cargo / rustup（PATH の先頭）。引数・環境変数をログに残し、振る舞いを環境変数で切り替える。
cat >"${work}/bin/cargo" <<'FAKE'
#!/usr/bin/env bash
sub="$1"
feat=""
td=""
prev=""
for a in "$@"; do
  [ "${prev}" = "--features" ] && feat="${a}"
  [ "${prev}" = "--target-dir" ] && td="${a}"
  prev="${a}"
done
{
  echo "ARGS $*"
  echo "ENV sub=${sub} BOOT=${RUSTC_BOOTSTRAP:-unset} RUSTFLAGS=${RUSTFLAGS:-unset} ENC=${CARGO_ENCODED_RUSTFLAGS:-unset}"
} >>"${FAKE_LOG}"
case "${sub}" in
  metadata) printf '{"target_directory":"%s"}\n' "${FAKE_TD}" ;;
  check)
    [ -n "${FAKE_FAIL_CHECK:-}" ] && exit 101
    exit 0
    ;;
  rustdoc)
    [ -n "${FAKE_FAIL_RUSTDOC:-}" ] && exit 101
    mkdir -p "${td}/wasm32-unknown-unknown/doc"
    case "${feat}" in
      backend-metal) src=metal-ok.json ;;
      backend-vulkan) src=vulkan-present.json ;;
      *) src=cuda-present.json ;;
    esac
    cp "${FAKE_F}/${src}" "${td}/wasm32-unknown-unknown/doc/fandhe_silicon_contract.json"
    ;;
  tree)
    [ -n "${FAKE_FAIL_TREE:-}" ] && exit 101
    cat "${FAKE_F}/tree-clean.txt"
    ;;
  *) exit 99 ;;
esac
FAKE
cat >"${work}/bin/rustup" <<'FAKE'
#!/usr/bin/env bash
[ -n "${FAKE_NO_TARGET:-}" ] && exit 0
echo "wasm32-unknown-unknown"
echo "x86_64-unknown-linux-gnu"
FAKE
chmod +x "${work}/bin/cargo" "${work}/bin/rustup"

# run_fake <env assignments...>: 偽の cargo で既定の経路を流す。
run_fake() {
  mkdir -p "${work}/td"
  : >"${work}/fake.log"
  set +e
  env "$@" PATH="${work}/bin:${PATH}" FAKE_LOG="${work}/fake.log" FAKE_TD="${work}/td" FAKE_F="${work}/f" \
    RUSTFLAGS="-Cdebuginfo=2" CARGO_ENCODED_RUSTFLAGS="-Cdebuginfo=2" RUSTC_BOOTSTRAP="1" \
    "${TOOL}" >"${OUT_F}" 2>"${ERR_F}"
  rc=$?
  set -e
}

# expect_log <name> <word>...: 偽の cargo のログに語が含まれる（先頭が `!` なら含まれない）。
expect_log() {
  local name="$1" word ok=1
  shift
  for word in "$@"; do
    case "${word}" in
      '!'*)
        if grep -q -F -e "${word#!}" "${work}/fake.log"; then
          ok=0
          echo "FAIL ${name}: unexpected in log: ${word#!}" >&2
        fi
        ;;
      *)
        if ! grep -q -F -e "${word}" "${work}/fake.log"; then
          ok=0
          echo "FAIL ${name}: missing in log: ${word}" >&2
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

# 偽の rustdoc は vulkan / cuda の module が在る JSON を返すので、既定の経路は mismatch で終了コード 1 になる。
run_fake X=1
expect "fake-default" 1 "check: feature=backend-metal target=wasm32-unknown-unknown result=match" \
  "check: feature=backend-cuda target=wasm32-unknown-unknown result=match" \
  "module: feature=backend-vulkan module=backend_vulkan expect=absent found=present public_items=0 result=mismatch" \
  "module: feature=backend-cuda module=backend_cuda expect=absent found=present public_items=0 result=mismatch" \
  "module: feature=backend-metal module=backend_metal expect=present-empty found=present public_items=0 result=match" \
  "record: commit=" "check=3/3 module=1/3"
expect_log "fake-args-metal" "ARGS check -p fandhe-silicon-contract --target wasm32-unknown-unknown --features backend-metal --locked"
expect_log "fake-args-vulkan" "ARGS check -p fandhe-silicon-contract --target wasm32-unknown-unknown --features backend-vulkan --locked"
expect_log "fake-args-cuda" "ARGS check -p fandhe-silicon-contract --target wasm32-unknown-unknown --features backend-cuda --locked"
expect_log "fake-default-features-kept" "!--no-default-features"
expect_log "fake-bootstrap-only-rustdoc" "ENV sub=rustdoc BOOT=1" "ENV sub=check BOOT=unset" "ENV sub=tree BOOT=unset" "ENV sub=metadata BOOT=unset"
expect_log "fake-flags-dropped" "!RUSTFLAGS=-C" "!ENC=-C"
expect_log "fake-tree-targets" "--target wasm32-unknown-unknown" "--target all" "-e normal,build,dev"
expect_log "fake-rustdoc-json" "--output-format json" "/wasm-backend-check/rustdoc"
# 構成ごとの JSON が別名で残り、上書きされない。
kept=0
for feat in backend-metal backend-vulkan backend-cuda; do
  if [ -f "${work}/td/wasm-backend-check/rustdoc/wasm32-unknown-unknown/doc/fandhe_silicon_contract.${feat}.json" ]; then
    kept=$((kept + 1))
  fi
done
if [ "${kept}" -eq 3 ]; then
  passed=$((passed + 1))
else
  failed=$((failed + 1))
  echo "FAIL fake-json-kept-per-feature: ${kept}/3" >&2
fi
run_fake FAKE_FAIL_CHECK=1
expect "fake-check-fails" 1 "check: feature=backend-metal target=wasm32-unknown-unknown result=mismatch" "violation: check: feature=backend-metal" "check=0/3"
run_fake FAKE_FAIL_RUSTDOC=1
expect "fake-rustdoc-fails" 2 "error: cargo rustdoc failed for backend-metal" "!record:"
run_fake FAKE_FAIL_TREE=1
expect "fake-tree-fails" 2 "error: cargo tree failed for backend-metal" "!record:"
run_fake FAKE_NO_TARGET=1
expect "fake-target-missing" 2 "error: rust target not installed: wasm32-unknown-unknown" "!record:"

# 5. 実物: 契約の crate と core だけの一時 workspace で実物の cargo を通す。
W="${work}/ws"
mkdir -p "${W}/scripts" "${W}/crates"
cp "${TOOL}" "${W}/scripts/wasm-backend-check.sh"
cp "${DIR}/lib.sh" "${W}/scripts/lib.sh"
cp "${ROOT_DIR}/rust-toolchain.toml" "${W}/rust-toolchain.toml"
cp -R "${ROOT_DIR}/crates/core" "${W}/crates/core"
cp -R "${ROOT_DIR}/crates/contract" "${W}/crates/contract"
cp "${ROOT_DIR}/Cargo.toml" "${W}/Cargo.toml"
python3 -I - "${W}/Cargo.toml" <<'PY'
import re
import sys

p = sys.argv[1]
t = open(p).read()
t = re.sub(r'^members = \[.*\]$', 'members = ["crates/core", "crates/contract"]', t, count=1, flags=re.M)
t = re.sub(r'^exclude = \[.*\]$', 'exclude = []', t, count=1, flags=re.M)
open(p, "w").write(t)
PY
git -C "${W}" init -q
(cd "${W}" && cargo generate-lockfile --offline >/dev/null 2>&1)
git -C "${W}" add -A
git -C "${W}" -c user.name=t -c user.email=t@example.invalid commit -q -m init

set +e
(cd "${W}" && scripts/wasm-backend-check.sh) >"${OUT_F}" 2>"${ERR_F}"
rc=$?
set -e
expect "real-all-match" 0 "check: feature=backend-metal target=wasm32-unknown-unknown result=match" \
  "module: feature=backend-metal module=backend_metal expect=present-empty found=present public_items=0 result=match" \
  "module: feature=backend-vulkan module=backend_vulkan expect=absent found=absent public_items=- result=match" \
  "module: feature=backend-cuda module=backend_cuda expect=absent found=absent public_items=- result=match" \
  "record: commit=" "check=3/3 module=3/3 sdk_wasm32_total=0 sdk_native_total=0" "ok:" "!violation:"
# ホストの絶対パスが出力に出ない。
if cat "${OUT_F}" "${ERR_F}" | grep -q -F -e "${work}"; then
  failed=$((failed + 1))
  echo "FAIL real-no-host-path: the work directory appears in the output" >&2
else
  passed=$((passed + 1))
fi

# 陽性対照: 写しの Vulkan のゲートから wasm32 の除外を外すと module: が mismatch になる。
python3 -I - "${W}/crates/contract/src/lib.rs" <<'PY'
import sys

p = sys.argv[1]
t = open(p).read()
old = '#[cfg(all(feature = "backend-vulkan", not(target_arch = "wasm32")))]'
if old not in t:
    sys.exit("gate line not found")
open(p, "w").write(t.replace(old, '#[cfg(feature = "backend-vulkan")]', 1))
PY
set +e
(cd "${W}" && scripts/wasm-backend-check.sh) >"${OUT_F}" 2>"${ERR_F}"
rc=$?
set -e
expect "real-positive-control" 1 "module: feature=backend-vulkan module=backend_vulkan expect=absent found=present public_items=0 result=mismatch" \
  "violation: module: feature=backend-vulkan" "FAILED:" "!ok:"

echo "passed=${passed} failed=${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
