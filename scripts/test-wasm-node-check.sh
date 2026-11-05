#!/usr/bin/env bash
# wasm-node-check.sh の回帰テスト（spec TASK-22 のポインタ）。
# node・wasm-bindgen・cargo は、一時ディレクトリに置く偽のコマンドを PATH の先頭に入れて差し替える。
# 偽の node は合成の結果のファイルを書き出すので、判定（一致・不一致・未実施・版違い・結果の欠け）の
# 全経路を機材なし・ネットワークなしで確かめられる。run-glue.cjs 自体は本物の node で偽の glue を読ませて確かめる。
# 呼び出し元: `make test-wasm-node-check`（直接実行も可。CI の `deny` ジョブが呼ぶ）。
#
# 判定は「終了コード + 標準出力・標準エラーの部分一致（先頭が ! の語は含まれないこと）」で行う。
# 期待値を緩めない。途中で止めず、最後に件数を出す。リポジトリのファイルは変更しない。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
# 偽のコマンドの本文と、展開させない合成の値（`$(...)` の形）を単引用符で書くため。
# shellcheck disable=SC2016
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd git
require_cmd node

WNC="${DIR}/wasm-node-check.sh"
NODE_SCRIPT="${ROOT_DIR}/wasm/node/run-glue.cjs"
CASES_REAL="${ROOT_DIR}/wasm/node/cases.json"
GLUE_MANIFEST="${ROOT_DIR}/wasm/crates/glue/Cargo.toml"
REAL_NODE="$(command -v node)"
WB_VER="$(sed -n 's/^wasm-bindgen = "=\([0-9][0-9.]*\)".*/\1/p' "${GLUE_MANIFEST}" | head -n 1)"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-wasm-node-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
bin="${work}/bin"
fix="${work}/fix"
mkdir -p "${bin}" "${fix}"
passed=0
failed=0
rc=0

unset FAKE_ROW_MAP FAKE_OUTPUTS FAKE_HOST_FIXTURE FAKE_HOST_RC FAKE_NODE_VERSION FAKE_NODE_RC FAKE_WB_VERSION FAKE_WB_NO_GLUE FAKE_CARGO_RC FAKE_CARGO_NO_ARTIFACT

# ---- 偽のコマンド ----

mkfake() {
  printf '#!/bin/sh\n%s\n' "$2" >"${bin}/$1"
  chmod +x "${bin}/$1"
}

export FAKE_LOG="${work}/calls.log"
export FAKE_RESULTS="${fix}/results.jsonl"
export FAKE_TARGET="${work}/target"
mkfake node 'if [ "$1" = "--version" ]; then echo "${FAKE_NODE_VERSION:-v24.13.0}"; exit 0; fi
echo "node $*" >>"$FAKE_LOG"
[ -f "$FAKE_RESULTS" ] && cp "$FAKE_RESULTS" "$4"
if [ -n "${FAKE_ROW_MAP:-}" ] && [ -f "$FAKE_ROW_MAP" ]; then
  while read -r rid cname; do cp "$6/$cname.native-scalar.bin" "$7/$rid.bin"; done <"$FAKE_ROW_MAP"
fi
if [ -n "${FAKE_OUTPUTS:-}" ] && [ -d "$FAKE_OUTPUTS" ]; then cp "$FAKE_OUTPUTS"/* "$7"/; fi
exit "${FAKE_NODE_RC:-0}"'
mkfake wasm-bindgen 'if [ "$1" = "--version" ]; then echo "${FAKE_WB_VERSION:-wasm-bindgen '"${WB_VER}"'}"; exit 0; fi
echo "wasm-bindgen $*" >>"$FAKE_LOG"
[ -z "${FAKE_WB_NO_GLUE:-}" ] || exit 0
out=""
prev=""
for a in "$@"; do
  [ "$prev" = "--out-dir" ] && out="$a"
  prev="$a"
  last="$a"
done
b="$(basename "$last" .wasm)"
echo "// fake glue" >"$out/$b.js"'
mkfake cargo 'echo "cargo $*" >>"$FAKE_LOG"
case "$1" in
  metadata) printf "{\"target_directory\":\"%s\"}\n" "$FAKE_TARGET" ;;
  run)
    for a in "$@"; do last="$a"; done
    [ -z "${FAKE_HOST_FIXTURE:-}" ] || cp "$FAKE_HOST_FIXTURE"/* "$last"/
    exit "${FAKE_HOST_RC:-0}"
    ;;
  build)
    echo "env rustflags=$(printf %s "${CARGO_ENCODED_RUSTFLAGS-unset}" | tr "\037" "|") RUSTFLAGS=${RUSTFLAGS-unset}" >>"$FAKE_LOG"
    [ -z "${FAKE_CARGO_NO_ARTIFACT:-}" ] || exit "${FAKE_CARGO_RC:-0}"
    td=""
    prev=""
    for a in "$@"; do
      [ "$prev" = "--target-dir" ] && td="$a"
      prev="$a"
    done
    d="${td:-$FAKE_TARGET}/wasm32-unknown-unknown/wasm-size"
    mkdir -p "$d"
    echo wasm >"$d/wasm_glue.wasm"
    exit "${FAKE_CARGO_RC:-0}"
    ;;
esac'
PATH="${bin}:${PATH}"
export PATH

# ---- 土台 ----

pass() {
  passed=$((passed + 1))
  echo "ok: $1"
}

fail() {
  failed=$((failed + 1))
  echo "FAIL: $1: $2" >&2
  echo "--- stderr ---" >&2
  sed 's/^/    /' "${ERR_F}" >&2 || true
}

# w <引数...>: 本体を実行し rc・標準出力・標準エラーを控える。
w() {
  rc=0
  : >"${FAKE_LOG}"
  "${BASH}" "${WNC}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# expect <名前> <期待コード> [語...]: 先頭が ! の語は標準出力・標準エラーのどちらにも無いこと。
expect() {
  local name="$1" want="$2" s
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    fail "${name}" "expected exit ${want}, got ${rc}"
    return 0
  fi
  for s in "$@"; do
    case "${s}" in
      '!'*)
        if grep -qF -e "${s#!}" "${ERR_F}" "${OUT_F}"; then
          fail "${name}" "output must not contain: ${s#!}"
          return 0
        fi
        ;;
      *)
        if ! grep -qF -e "${s}" "${ERR_F}" "${OUT_F}"; then
          fail "${name}" "output lacks: ${s}"
          return 0
        fi
        ;;
    esac
  done
  pass "${name}"
}

# check <名前> <0 なら成功>: 静的な性質の確認。
check() {
  if [ "$2" = "0" ]; then pass "$1"; else fail "$1" "check failed"; fi
}

# 最小 PATH: 指定したコマンドだけを symlink した bin（コマンドが無い状況を作る）。
mkbin() {
  local d="$1" c p
  shift
  mkdir -p "${d}"
  for c in "$@"; do
    p="$(command -v "${c}")"
    ln -sf "${p}" "${d}/${c}"
  done
}

# ---- 入力の fixture ----

dummy_wasm="${fix}/wasm_glue.wasm"
echo wasm >"${dummy_wasm}"
cases="${fix}/cases.json"
cat >"${cases}" <<'EOF'
{"rows": [
  {"id": "row-a", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]},
  {"id": "row-b", "export": "roundtrip_f32", "input": ["80000000", "7fc00000"], "reference_source": "fixed", "reference": ["80000000", "7fc00000"]}
]}
EOF
res_ok() {
  printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-b","output":["80000000","7fc00000"]}' >"${FAKE_RESULTS}"
}
res_ok

# ---- 前提 ----

# 1. node が PATH に無い（python3・wasm-bindgen だけある最小 PATH）
mkbin "${work}/nonode" python3 wasm-bindgen dirname basename sed head mktemp rm cat grep
rc=0
PATH="${work}/nonode" "${BASH}" "${WNC}" --wasm "${dummy_wasm}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "1 node missing" 2 "node"
check "1 node body not started" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"

# 2. node の版違い
for v in v22.0.0 v24.13.0-rc.1 v24.13.1; do
  FAKE_NODE_VERSION="${v}" w --wasm "${dummy_wasm}"
  expect "2 node version ${v}" 2 "expected v24.13.0" "got ${v}" "!summary:"
  check "2 node body not started (${v})" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"
done

# 3. wasm-bindgen が無い
mkbin "${work}/nowb" python3 dirname basename sed head mktemp rm cat grep
cp "${bin}/node" "${work}/nowb/node"
rc=0
PATH="${work}/nowb" "${BASH}" "${WNC}" --wasm "${dummy_wasm}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "3 wasm-bindgen missing" 2 "wasm-bindgen"

# 4. wasm-bindgen の版違い
FAKE_WB_VERSION="wasm-bindgen 0.2.127" w --wasm "${dummy_wasm}"
expect "4 wasm-bindgen version" 2 "expected wasm-bindgen ${WB_VER}" "got wasm-bindgen 0.2.127" "!summary:"
FAKE_WB_VERSION="wasm-bindgen ${WB_VER}-rc.1" w --wasm "${dummy_wasm}"
expect "4 wasm-bindgen prerelease" 2 "version mismatch"

# ---- 判定 ----

# 5. 全行一致
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "5 all match" 0 "row: row-a match" "row: row-b match" "summary: rows=2 match=2 mismatch=0 not-run=0"

# 6. 1 つだけビット列が違う
printf '%s\n' '{"id":"row-a","output":["3f800001"]}' '{"id":"row-b","output":["80000000","7fc00000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "6 one bit differs" 1 "row: row-a mismatch" "row: row-b match" "mismatch=1" "!mismatch=0"

# 7. -0.0 と +0.0、NaN のペイロード違いは不一致（ビット列の比較）
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-b","output":["00000000","7fc00000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "7 signed zero differs" 1 "row: row-b mismatch" "index 0"
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-b","output":["80000000","7fc00001"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "7 NaN payload differs" 1 "row: row-b mismatch" "index 1"

# 8. 長さの違う結果
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-b","output":["80000000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "8 length differs" 1 "row: row-b mismatch" "length expected 2 got 1"

# 9. 参照の無い行は not-run（Node の結果があっても一致にしない）
cat >"${fix}/noref.json" <<'EOF'
{"rows": [
  {"id": "row-a", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]},
  {"id": "row-b", "export": "roundtrip_f32", "input": ["80000000"], "reference_source": "fixed"},
  {"id": "row-c", "export": "roundtrip_f32", "input": ["80000000"], "reference_source": "fixed", "reference": null}
]}
EOF
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-b","output":["80000000"]}' '{"id":"row-c","output":["80000000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${fix}/noref.json"
expect "9 no reference is not-run" 1 "row: row-a match" "row: row-b not-run (reference-missing)" "row: row-c not-run (reference-missing)" "match=1 mismatch=0 not-run=2"

# 10. 行が 0 件
echo '{"rows": []}' >"${fix}/empty.json"
: >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${fix}/empty.json"
expect "10 zero rows" 1 "rows=0" "no rows"

# 11. 結果の error 行は不一致（参照のある行）
printf '%s\n' '{"id":"row-a","error":"call failed"}' '{"id":"row-b","output":["80000000","7fc00000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "11 error row" 1 "row: row-a mismatch" "call error"

# 12. Node が非 0 で終わる
res_ok
FAKE_NODE_RC=3 w --wasm "${dummy_wasm}" --cases "${cases}"
expect "12 node nonzero" 2 "node run failed" "!summary:"

# 13. 結果のファイルが不正
printf '%s\n' 'not json' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "13 broken results" 2 "!summary:"
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "13 missing row" 2 "lack a row" "!summary:"
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-z","output":[]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "13 unknown id" 2 "unknown or missing id"
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-b","output":[]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "13 repeated id" 2 "repeat an id"
printf '%s\n' '{"id":"row-a","output":["zz"]}' '{"id":"row-b","output":[]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "13 bad bit pattern in results" 2 "invalid bit pattern"
rm -f "${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "13 no results file" 2 "cannot read results"
res_ok

# 14. 入力が不正
w --wasm "${dummy_wasm}" --cases "${fix}/does-not-exist.json"
expect "14 unreadable cases" 2 "cannot read cases"
echo '{"rows": [' >"${fix}/bad.json"
w --wasm "${dummy_wasm}" --cases "${fix}/bad.json"
expect "14 broken JSON" 2 "cannot read cases"
echo '{"rows": [{"id":"a","export":"roundtrip_f32","input":[],"reference_source":"fixed"},{"id":"a","export":"roundtrip_f32","input":[],"reference_source":"fixed"}]}' >"${fix}/dup.json"
w --wasm "${dummy_wasm}" --cases "${fix}/dup.json"
expect "14 duplicate id" 2 "duplicate row id"
echo '{"rows": [{"id":"a","export":"roundtrip_f32","input":["3F800000"],"reference_source":"fixed"}]}' >"${fix}/hex.json"
w --wasm "${dummy_wasm}" --cases "${fix}/hex.json"
expect "14 bad bit pattern" 2 "invalid bit pattern"
echo '{"rows": [{"id":"a","export":"mystery","input":[],"reference_source":"fixed"}]}' >"${fix}/exp.json"
w --wasm "${dummy_wasm}" --cases "${fix}/exp.json"
expect "14 unknown export" 2 "unknown export"
echo '{"rows": [{"id":"a","export":"roundtrip_f32","input":[],"reference_source":"fixed","extra":1}]}' >"${fix}/key.json"
w --wasm "${dummy_wasm}" --cases "${fix}/key.json"
expect "14 unknown key" 2 "missing or unknown keys"
check "14 node body not started on invalid cases" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"

# 15. 誤用
w --bogus
expect "15 unknown argument" 2 "usage:"
w --wasm
expect "15 missing --wasm value" 2 "usage:"
w --cases
expect "15 missing --cases value" 2 "usage:"
w --wasm "${fix}/no-such.wasm" --cases "${cases}"
expect "15 missing wasm file" 2 "missing or empty wasm file"
: >"${fix}/empty.wasm"
w --wasm "${fix}/empty.wasm" --cases "${cases}"
expect "15 empty wasm file" 2 "missing or empty wasm file"

# 16. glue が生成されない
FAKE_WB_NO_GLUE=1 w --wasm "${dummy_wasm}" --cases "${cases}"
expect "16 no glue" 2 "glue was not generated" "!summary:"

# 17. シェルに展開されない合成の入力
mkdir -p "${work}/inj"
printf '%s\n' '{"rows": [{"id":"$(touch '"${work}"'/inj/pwned)","export":"roundtrip_f32","input":[],"reference_source":"fixed"},{"id":"a;touch","export":"roundtrip_f32","input":[],"reference_source":"fixed"}]}' >"${fix}/inj.json"
w --wasm "${dummy_wasm}" --cases "${fix}/inj.json"
expect "17 injection-shaped id" 2 "invalid form"
check "17 nothing executed" "$([ ! -e "${work}/inj/pwned" ] && [ ! -e "${work}/inj/touch" ] && echo 0 || echo 1)"

# 18, 19. 偽の node・wasm-bindgen が受け取った引数
res_ok
w --wasm "${dummy_wasm}" --cases "${cases}"
expect "18 node args" 0 "summary: rows=2 match=2"
check "18 node receives script, glue, cases" "$(grep -E "^node ${NODE_SCRIPT} .*/wasm_glue\.js ${cases} " "${FAKE_LOG}" >/dev/null && echo 0 || echo 1)"
check "19 wasm-bindgen --target nodejs" "$(grep -E '^wasm-bindgen .*--target nodejs ' "${FAKE_LOG}" >/dev/null && echo 0 || echo 1)"

# 既定の経路（cargo を呼ぶ）
w --cases "${cases}"
expect "cargo path" 0 "summary: rows=2 match=2"
check "cargo build uses wasm-size on wasm32" "$(grep -E '^cargo build .*--locked .*--target wasm32-unknown-unknown --profile wasm-size' "${FAKE_LOG}" >/dev/null && echo 0 || echo 1)"
FAKE_CARGO_RC=7 w --cases "${cases}"
expect "cargo build failure propagates" 7 "!summary:"
rm -rf "${FAKE_TARGET}"
FAKE_CARGO_NO_ARTIFACT=1 w --cases "${cases}"
expect "cargo artifact missing" 2 "missing or empty wasm file"

# 呼び出し元がルート以外でも、相対の --cases / --wasm は呼び出し元の cwd 基準で解決される
res_ok
rc=0
: >"${FAKE_LOG}"
(cd "${fix}" && "${BASH}" "${WNC}" --cases cases.json >"${OUT_F}" 2>"${ERR_F}" </dev/null) || rc=$?
expect "24 relative --cases from non-root cwd (cargo path)" 0 "summary: rows=2 match=2"
check "24 node receives the absolute cases path" "$(grep -E "^node ${NODE_SCRIPT} .*/wasm_glue\.js ${cases} " "${FAKE_LOG}" >/dev/null && echo 0 || echo 1)"
rc=0
: >"${FAKE_LOG}"
(cd "${fix}" && "${BASH}" "${WNC}" --wasm wasm_glue.wasm --cases cases.json >"${OUT_F}" 2>"${ERR_F}" </dev/null) || rc=$?
expect "24 relative --wasm and --cases from non-root cwd" 0 "summary: rows=2 match=2"
check "24 wasm-bindgen receives the absolute wasm path" "$(grep -F "${dummy_wasm}" "${FAKE_LOG}" >/dev/null && echo 0 || echo 1)"
rc=0

# 20. 版の宣言点
check "20 wasm-bindgen version read from Cargo.toml" "$([ -n "${WB_VER}" ] && ! grep -qF "${WB_VER}" "${WNC}" && echo 0 || echo 1)"
check "20 node version constant form" "$(grep -qE '^NODE_EXPECTED="v[0-9]+\.[0-9]+\.[0-9]+"$' "${WNC}" && grep -qF 'NODE_EXPECTED="v24.13.0"' "${WNC}" && echo 0 || echo 1)"

# 21. 静的な性質（Node 標準だけ）
bad_req="$(grep -oE '(require|import)[ (][^)]*' "${NODE_SCRIPT}" | grep -vE '^require\("node:[a-z/_]+"|^require\(gluePath$' || true)"
check "21 run-glue requires only node: builtins and the glue" "$([ -z "${bad_req}" ] && echo 0 || echo 1)"
check "21 no eval / child_process" "$(grep -qE 'eval\(|new Function|child_process' "${NODE_SCRIPT}" && echo 1 || echo 0)"
check "21 no npm package files" "$(git -C "${ROOT_DIR}" ls-files -- wasm scripts package.json package-lock.json node_modules | grep -qE '(^|/)(package\.json|package-lock\.json|node_modules/)' && echo 1 || echo 0)"

# 22. 同梱の入力（偽 node が参照どおりの結果を返すと 0）。固定の行と生成の行（TASK-994）の両方を扱う
# （生成の行が足されても、この項目を直さずに通る）。
# mk_host_fixture <dir> <ケース名...>: 偽の host 側の道具が書く小さな合成のケース（2x3x2）の manifest と .bin。
mk_host_fixture() {
  mkdir -p "$1"
  python3 -I - "$@" <<'PY'
import hashlib, json, os, struct, sys
d = sys.argv[1]
cases = []
def wr(fn, words):
    data = b"".join(struct.pack("<I", w) for w in words)
    with open(os.path.join(d, fn), "wb") as f:
        f.write(data)
    return hashlib.sha256(data).hexdigest()
for name in sys.argv[2:]:
    a = [0x3f800000, 0x80000000, 0x7fc00001, 0x40000000, 0xc0000000, 0x00000000]
    b = [0x3f800000] * 6
    nat = [0x3f800000, 0x80000000, 0x7fc00001, 0x40490fdb]
    cases.append({
        "name": name, "kind": "gemm", "m": 2, "k": 3, "n": 2,
        "a_file": name + ".a.bin", "b_file": name + ".b.bin", "native_scalar_file": name + ".native-scalar.bin",
        "a_sha256": wr(name + ".a.bin", a), "b_sha256": wr(name + ".b.bin", b),
        "native_scalar_sha256": wr(name + ".native-scalar.bin", nat),
    })
with open(os.path.join(d, "manifest.json"), "w") as f:
    json.dump({"cases": cases}, f, indent=2)
PY
}
# reset_gen: 生成の行の場面で足した環境の値を戻す。
reset_gen() {
  unset FAKE_ROW_MAP FAKE_OUTPUTS FAKE_HOST_FIXTURE FAKE_HOST_RC
}
# bundled_check <名前> <cases>: 行ごとに参照どおりの結果（生成の行は偽の host 側の道具の参照の写し）を用意して流す。
bundled_check() {
  local name="$1" cf="$2" info names n
  rm -rf "${fix}/hf-${name}" "${fix}/rowmap-${name}"
  info="$(python3 -I -c 'import json,sys
rows = json.load(open(sys.argv[1]))["rows"]
res = []
rm = []
cs = []
for r in rows:
    if "case" in r:
        res.append({"id": r["id"], "output_file": r["id"] + ".bin"})
        rm.append(r["id"] + " " + r["case"])
        if r["case"] not in cs:
            cs.append(r["case"])
    else:
        res.append({"id": r["id"], "output": r["reference"]})
open(sys.argv[2], "w").write("".join(json.dumps(x) + "\n" for x in res))
open(sys.argv[3], "w").write("".join(x + "\n" for x in rm))
print(" ".join(cs))
print(len(rows))' "${cf}" "${FAKE_RESULTS}" "${fix}/rowmap-${name}")"
  names="$(printf '%s\n' "${info}" | head -n 1)"
  n="$(printf '%s\n' "${info}" | tail -n 1)"
  # ケース名は空白区切りで渡す（意図した分割）。
  # shellcheck disable=SC2086
  mk_host_fixture "${fix}/hf-${name}" ${names}
  export FAKE_HOST_FIXTURE="${fix}/hf-${name}" FAKE_ROW_MAP="${fix}/rowmap-${name}"
  w --wasm "${dummy_wasm}" --wasm-simd128 "${dummy_wasm}" --cases "${cf}"
  reset_gen
  expect "22 ${name}" 0 "summary: rows=${n} match=${n} mismatch=0 not-run=0"
}
bundled_check bundled "${CASES_REAL}"
check "22 every bundled fixed row has a reference, every generated row has the exact five keys" "$(python3 -I -c 'import json,sys
d = json.load(open(sys.argv[1]))
def ok(r):
    if "case" in r or str(r.get("export", "")).startswith("gemm_"):
        return set(r) == {"id", "export", "build", "case", "reference_source"} and r["reference_source"] == "native-scalar"
    return r.get("reference_source") == "fixed" and isinstance(r.get("reference"), list)
sys.exit(0 if d["rows"] and all(ok(r) for r in d["rows"]) else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
check "22 bundled isa rows pin the scalar and simd128 values" "$(python3 -I -c 'import json,sys
rows = {r["id"]: r for r in json.load(open(sys.argv[1]))["rows"]}
a, b = rows.get("isa-scalar"), rows.get("isa-simd128")
sys.exit(0 if a and b and a["build"] == "scalar" and b["build"] == "simd128" and a["reference"] == ["00000000"] and b["reference"] == ["00000001"] else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
# 生成の行を 3 ケース × 3 行で足した形の合成の入力でも、この回帰テストを直さずに通る
python3 -I - "${fix}/nine.json" <<'PY'
import json, sys
rows = [{"id": "keep-fixed", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]}]
for c in ("gemm-a", "gemm-b", "gemm-c"):
    for exp, build in (("gemm_scalar", "scalar"), ("gemm_simd", "scalar"), ("gemm_simd", "simd128")):
        rows.append({"id": "%s-%s-%s" % (c, exp.split("_")[1], build), "export": exp, "build": build, "case": c, "reference_source": "native-scalar"})
json.dump({"rows": rows}, open(sys.argv[1], "w"))
PY
bundled_check nine "${fix}/nine.json"

# 23. run-glue.cjs 自体（本物の node で偽の glue を読ませる）。引数は 6 つ（TASK-994）
mkdir -p "${work}/glue"
echo 'exports.roundtrip_f32 = (a) => a; exports.detect_isa_bits = () => 0;' >"${work}/glue/id.js"
echo 'exports.roundtrip_f32 = (a) => a.map((x) => x + 1);' >"${work}/glue/inc.js"
echo 'exports.roundtrip_f32 = () => { throw new Error("boom"); };' >"${work}/glue/throw.js"
echo 'exports.roundtrip_f32 = (a) => a; exports.detect_isa_bits = () => 1;' >"${work}/glue/simd.js"
mkdir -p "${work}/glue/out"
python3 -I - "${CASES_REAL}" "${work}/glue/fixed-cases.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d["rows"] = [r for r in d["rows"] if "case" not in r]
json.dump(d, open(sys.argv[2], "w"))
PY
FIXED_CASES="${work}/glue/fixed-cases.json"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/id.js" "${FIXED_CASES}" "${work}/glue/r1.jsonl" "${work}/glue/simd.js" - "${work}/glue/out" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue identity glue" 0
if python3 -I -c 'import json,sys
rows = {r["id"]: r for r in json.load(open(sys.argv[1]))["rows"]}
out = [json.loads(l) for l in open(sys.argv[2])]
sys.exit(0 if len(out) == len(rows) and all(o["output"] == rows[o["id"]]["reference"] for o in out) else 1)' "${FIXED_CASES}" "${work}/glue/r1.jsonl"; then
  pass "23 run-glue keeps bit patterns and picks the glue by build (NaN, -0.0, Inf, isa bits)"
else
  fail "23 run-glue keeps bit patterns and picks the glue by build" "output differs from reference"
fi
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/inc.js" "${FIXED_CASES}" "${work}/glue/r2.jsonl" - - "${work}/glue/out" >"${OUT_F}" 2>"${ERR_F}" || true
check "23 run-glue reports a changed value" "$(grep -q '"id":"roundtrip-finite","output":\["3f800000"' "${work}/glue/r2.jsonl" && echo 1 || echo 0)"
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/throw.js" "${FIXED_CASES}" "${work}/glue/r3.jsonl" - - "${work}/glue/out" >"${OUT_F}" 2>"${ERR_F}" || true
check "23 run-glue turns a throw into an error row" "$(grep -qc 'call failed' "${work}/glue/r3.jsonl" && echo 0 || echo 1)"
echo '{"rows":[{"id":"a","export":"constructor","input":[]}]}' >"${fix}/ctor.json"
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/id.js" "${fix}/ctor.json" "${work}/glue/r4.jsonl" - - "${work}/glue/out" >"${OUT_F}" 2>"${ERR_F}" || true
check "23 run-glue rejects a non-listed export" "$(grep -q 'unknown export' "${work}/glue/r4.jsonl" && echo 0 || echo 1)"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue usage" 2 "usage:"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/id.js" "${FIXED_CASES}" "${work}/glue/r5.jsonl" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue old three-argument form is a usage error" 2 "usage:"

# 23b. gemm の呼び方: out は 0x7fc0dead で埋めて渡す・戻り 0 のときだけ .bin を書く・ビット列を保つ
mk_host_fixture "${work}/glue/in" gemm-a
cat >"${work}/glue/gemm.js" <<'JS'
exports.gemm_scalar = (a, b, m, k, n, out) => {
  out[0] = a[0];
  out[1] = a[1];
  return 0;
};
exports.gemm_simd = () => 7;
JS
echo 'exports.gemm_simd = () => { throw new Error("boom"); };' >"${work}/glue/gemmthrow.js"
cat >"${work}/glue/gemm-cases.json" <<'JSON'
{"rows": [
  {"id": "g1", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "g2", "export": "gemm_simd", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "g3", "export": "gemm_simd", "build": "simd128", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "g4", "export": "gemm_scalar", "build": "scalar", "case": "no-such-case", "reference_source": "native-scalar"}
]}
JSON
rm -rf "${work}/glue/gout"
mkdir -p "${work}/glue/gout"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/gemm.js" "${work}/glue/gemm-cases.json" "${work}/glue/rg.jsonl" "${work}/glue/gemmthrow.js" "${work}/glue/in" "${work}/glue/gout" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue gemm rows" 0
check "23 gemm: a zero return writes the output with the fill word kept and the bit patterns intact" "$(python3 -I -c 'import struct,sys
data = open(sys.argv[1], "rb").read()
sys.exit(0 if struct.unpack("<4I", data) == (0x3f800000, 0x80000000, 0x7fc0dead, 0x7fc0dead) else 1)' "${work}/glue/gout/g1.bin" && echo 0 || echo 1)"
check "23 gemm: nonzero return, throw and unknown case are error rows and write nothing" "$(python3 -I -c 'import json,os,sys
rows = {json.loads(l)["id"]: json.loads(l) for l in open(sys.argv[1])}
ok = rows["g1"] == {"id": "g1", "output_file": "g1.bin"} and rows["g2"] == {"id": "g2", "error": "nonzero return"} and rows["g3"] == {"id": "g3", "error": "call failed"} and rows["g4"] == {"id": "g4", "error": "call failed"}
ok = ok and sorted(os.listdir(sys.argv[2])) == ["g1.bin"]
sys.exit(0 if ok else 1)' "${work}/glue/rg.jsonl" "${work}/glue/gout" && echo 0 || echo 1)"

# 25. 参照の出どころ（TASK-29）。欠け・未知の値は入力の不正（Node の本体を起動しない）
res_ok
echo '{"rows": [{"id":"a","export":"roundtrip_f32","input":[],"reference":[]}]}' >"${fix}/nosrc.json"
w --wasm "${dummy_wasm}" --cases "${fix}/nosrc.json"
expect "25 reference_source missing" 2 "missing or unknown keys" "!summary:"
check "25 node body not started (missing source)" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"
n25=0
for bad in '"Fixed"' '"FIXED"' '"fixed "' '"neon"' '""' '1' 'null' '["fixed"]'; do
  n25=$((n25 + 1))
  printf '%s\n' '{"rows": [{"id":"a","export":"roundtrip_f32","input":[],"reference_source":'"${bad}"',"reference":[]}]}' >"${fix}/badsrc.json"
  w --wasm "${dummy_wasm}" --cases "${fix}/badsrc.json"
  expect "25 reference_source invalid (${n25})" 2 "unknown reference_source" "!summary:"
done
check "25 node body not started (invalid source)" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"

# 26. native-neon の行は、Node の結果が参照と一致していても not-run (reference-unavailable)。参照が無くても出どころが先
cat >"${fix}/neon.json" <<'EOF'
{"rows": [
  {"id": "row-a", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "native-neon", "reference": ["3f800000"]},
  {"id": "row-b", "export": "roundtrip_f32", "input": ["80000000"], "reference_source": "native-neon"},
  {"id": "row-c", "export": "roundtrip_f32", "input": ["80000000"], "reference_source": "fixed"},
  {"id": "row-d", "export": "roundtrip_f32", "input": ["80000000"], "reference_source": "fixed", "reference": ["80000000"]}
]}
EOF
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' '{"id":"row-b","output":["80000000"]}' '{"id":"row-c","output":["80000000"]}' '{"id":"row-d","output":["80000000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${fix}/neon.json"
expect "26 mixed sources" 1 "row: row-a not-run (reference-unavailable)" "row: row-b not-run (reference-unavailable)" "row: row-c not-run (reference-missing)" "row: row-d match" "summary: rows=4 match=1 mismatch=0 not-run=3"
cat >"${fix}/neon1.json" <<'EOF'
{"rows": [
  {"id": "row-a", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "native-neon", "reference": ["3f800000"]}
]}
EOF
printf '%s\n' '{"id":"row-a","output":["3f800000"]}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${fix}/neon1.json"
expect "26 native-neon only is never success" 1 "match=0 mismatch=0 not-run=1"
printf '%s\n' '{"id":"row-a","error":"call failed"}' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${fix}/neon1.json"
expect "26 unavailable source wins over a call error" 1 "row: row-a not-run (reference-unavailable)" "!mismatch (call error)"
res_ok

# 27. 綴りの一致: 出どころと理由のトークンが core のソースに在る（一方向。core 側の綴りが正）
REF_SRC="${ROOT_DIR}/crates/core/src/rerun/reference.rs"
DEC_SRC="${ROOT_DIR}/crates/core/src/test_frame/decision.rs"
sp=0
for t in fixed native-neon native-scalar; do
  grep -qF "=> \"${t}\"," "${REF_SRC}" || sp=1
  grep -qF "\"${t}\"" "${WNC}" || sp=1
done
for t in reference-missing reference-unavailable; do
  grep -qF "=> \"${t}\"," "${DEC_SRC}" || sp=1
  grep -qF "${t}" "${WNC}" || sp=1
done
LBL_SRC="${ROOT_DIR}/crates/core/src/test_frame/label.rs"
for t in ok ng not-run; do
  grep -qF "=> \"${t}\"," "${LBL_SRC}" || sp=1
  grep -qF "\"${t}\"" "${WNC}" || sp=1
done
check "27 source and reason spellings exist in the core sources" "${sp}"

# ---- 部品の書き出し（TASK-29 の追加の葉。#7188） ----

RS="${DIR}/record-summary.sh"
RC="${DIR}/rerun-compare.sh"
cat >"${fix}/rec.json" <<'JSON'
{"rows": [
  {"id": "rec-match", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]},
  {"id": "rec-mismatch", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]},
  {"id": "rec-missing", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed"},
  {"id": "rec-unavail", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "native-neon", "reference": ["3f800000"]}
]}
JSON
res_rec() {
  printf '%s\n' '{"id":"rec-match","output":["3f800000"]}' "{\"id\":\"rec-mismatch\",\"output\":[\"$1\"]}" '{"id":"rec-missing","output":["3f800000"]}' '{"id":"rec-unavail","output":["3f800000"]}' >"${FAKE_RESULTS}"
}
res_rec 3f800001
newdir() {
  rm -rf "${work:?}/$1"
  mkdir -p "${work}/$1"
}
nfiles() {
  find "$1" -type f | wc -l | tr -d ' '
}

# 28. 書き出し: 名前・中身・record-summary の表
newdir rec1
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec1" --record-combination metal-mac-mini
expect "28 record written" 1 "row: rec-match match" "row: rec-mismatch mismatch (first difference at index 0)" "match=1 mismatch=1 not-run=2" "!${work}"
cp "${OUT_F}" "${work}/out-with.txt"
cat >"${work}/expect-rec-match.json" <<'JSON'
{"behavior_ids":["BUILD-41"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"ok","body_kind":"contract-test","body":{"test_id":"rec-match"}}
JSON
cat >"${work}/expect-rec-mismatch.json" <<'JSON'
{"behavior_ids":["BUILD-41"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"ng","body_kind":"contract-test","body":{"test_id":"rec-mismatch"}}
JSON
cat >"${work}/expect-rec-missing.json" <<'JSON'
{"behavior_ids":["BUILD-41"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"not-run","label_reason":"reference-missing","body_kind":"contract-test","body":{"test_id":"rec-missing"}}
JSON
cat >"${work}/expect-rec-unavail.json" <<'JSON'
{"behavior_ids":["BUILD-41"],"machine_combination":"metal-mac-mini","machine":"mac-mini","label":"not-run","label_reason":"reference-unavailable","body_kind":"contract-test","body":{"test_id":"rec-unavail"}}
JSON
okf=0
for t in rec-match rec-mismatch rec-missing rec-unavail; do
  cmp -s "${work}/expect-${t}.json" "${work}/rec1/${t}.metal-mac-mini.json" || okf=1
done
check "28 four parts with the exact bytes" "$([ "${okf}" = 0 ] && [ "$(nfiles "${work}/rec1")" = 4 ] && echo 0 || echo 1)"
rc=0
"${BASH}" "${RS}" "${work}/rec1" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "28 record-summary accepts the parts" 0 "| metal-mac-mini | 1 | 1 | 0 | 2 | 0 |"

# 29. 7 つの組み合わせすべてで record-summary が通る（machine の写しの食い違いは machine-mismatch で落ちる）
for c in metal-mac-mini vulkan-node-b vulkan-node-a vulkan-amd vulkan-intel cuda-node-b cuda-node-a; do
  newdir rec29
  w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec29" --record-combination "${c}"
  rc29=${rc}
  rc=0
  "${BASH}" "${RS}" "${work}/rec29" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
  expect "29 record every combination (${c})" 0 "| ${c} | 1 | 1 | 0 | 2 | 0 |"
  [ "${rc29}" = 1 ] || fail "29 wasm-node-check exit (${c})" "expected 1, got ${rc29}"
done

# 30. 選ばないときの出力と終了コードは変わらない
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json"
rc_without=${rc}
cp "${OUT_F}" "${work}/out-without.txt"
check "30 unselected output is byte-identical" "$(cmp -s "${work}/out-with.txt" "${work}/out-without.txt" && [ "${rc_without}" = 1 ] && echo 0 || echo 1)"
before="$(find "${work}" -maxdepth 1 | wc -l | tr -d ' ')"
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json"
check "30 unselected run leaves no files" "$([ "$(find "${work}" -maxdepth 1 | wc -l | tr -d ' ')" = "${before}" ] && echo 0 || echo 1)"

# 31. rerun-compare: 同じなら差なし、変われば分類が出る
newdir recA
newdir recB
res_rec 3f800001
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/recA" --record-combination metal-mac-mini
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/recB" --record-combination metal-mac-mini
rc=0
"${BASH}" "${RC}" "${work}/recA" "${work}/recB" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "31 rerun-compare same record" 0 "ok: no differences from the previous record"
newdir recC
res_rec 3f800000
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/recC" --record-combination metal-mac-mini
rc=0
"${BASH}" "${RC}" "${work}/recC" "${work}/recA" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "31 rerun-compare match to mismatch is regressed" 1 "regressed" "rec-mismatch" "regressed=1"
rc=0
"${BASH}" "${RC}" "${work}/recA" "${work}/recC" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "31 rerun-compare mismatch to match is improved" 1 "improved" "rec-mismatch" "improved=1"
# not-run (reference-missing) → match は improved、理由だけの変化は changed
cat >"${fix}/rec-b.json" <<'JSON'
{"rows": [
  {"id": "rec-x", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed"}
]}
JSON
cat >"${fix}/rec-c.json" <<'JSON'
{"rows": [
  {"id": "rec-x", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]}
]}
JSON
cat >"${fix}/rec-d.json" <<'JSON'
{"rows": [
  {"id": "rec-x", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "native-neon"}
]}
JSON
printf '%s\n' '{"id":"rec-x","output":["3f800000"]}' >"${FAKE_RESULTS}"
newdir recM
newdir recK
newdir recU
w --wasm "${dummy_wasm}" --cases "${fix}/rec-b.json" --record-dir "${work}/recM" --record-combination metal-mac-mini
w --wasm "${dummy_wasm}" --cases "${fix}/rec-c.json" --record-dir "${work}/recK" --record-combination metal-mac-mini
w --wasm "${dummy_wasm}" --cases "${fix}/rec-d.json" --record-dir "${work}/recU" --record-combination metal-mac-mini
rc=0
"${BASH}" "${RC}" "${work}/recM" "${work}/recK" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "31 rerun-compare not-run to match is improved" 1 "improved" "rec-x"
rc=0
"${BASH}" "${RC}" "${work}/recM" "${work}/recU" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "31 rerun-compare reason-only change is changed" 1 "changed" "rec-x" "changed=1"

# 32. 誤用: 片方だけ・値の欠け・未知の組み合わせは終了コード 2 で、Node の本体を起動しない
res_rec 3f800001
newdir rec32
n32=0
set_misuse() {
  n32=$((n32 + 1))
  w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" "$@"
}
set_misuse --record-dir "${work}/rec32"
expect "32 --record-dir alone (${n32})" 2 "usage:" "!summary:"
set_misuse --record-combination metal-mac-mini
expect "32 --record-combination alone (${n32})" 2 "usage:" "!summary:"
set_misuse --record-dir
expect "32 --record-dir without value (${n32})" 2 "usage:"
set_misuse --record-dir "${work}/rec32" --record-combination
expect "32 --record-combination without value (${n32})" 2 "usage:"
set_misuse --record-dir "" --record-combination metal-mac-mini
expect "32 empty --record-dir (${n32})" 2 "usage:"
for bad in Metal-mac-mini "" metal "metal-mac-mini " vulkan-mac-mini; do
  set_misuse --record-dir "${work}/rec32" --record-combination "${bad}"
  expect "32 unknown combination '${bad}'" 2 "unknown record combination" "!summary:"
  check "32 node body not started ('${bad}')" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"
done
check "32 nothing written on misuse" "$([ "$(nfiles "${work}/rec32")" = 0 ] && echo 0 || echo 1)"

# 33. 書き出し先が不正: 終了コード 2、Node を起動しない、引数のパスを出さない
mkdir -p "${work}/dirs"
: >"${work}/dirs/afile"
ln -sfn "${work}/rec32" "${work}/dirs/link"
bad_dir() {
  w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "$1" --record-combination metal-mac-mini
  expect "33 invalid record dir ($2)" 2 "error:" "!summary:" "!$1"
  check "33 node body not started ($2)" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"
}
bad_dir "${work}/dirs/missing" "missing"
bad_dir "${work}/dirs/afile" "file"
bad_dir "${work}/dirs/link" "symlink"
bad_dir "${work}/dirs/link/" "symlink with slash"
bad_dir "${work}/dirs/link/." "symlink with dot"
bad_dir "${work}/dirs/../rec32" "parent component"
check "33 nothing written through the symlink" "$([ "$(nfiles "${work}/rec32")" = 0 ] && echo 0 || echo 1)"
# 相対パスは呼び出し元の cwd 基準
newdir rec33
rc=0
(cd "${work}" && "${BASH}" "${WNC}" --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir rec33 --record-combination metal-mac-mini >"${OUT_F}" 2>"${ERR_F}" </dev/null) || rc=$?
expect "33 relative record dir resolves from the caller cwd" 1 "summary: rows=4"
check "33 relative record dir got the parts" "$([ "$(nfiles "${work}/rec33")" = 4 ] && echo 0 || echo 1)"
# 末尾の区切りと . は字句で落ちる（通常のディレクトリなら通る）
newdir rec33b
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec33b/./" --record-combination metal-mac-mini
expect "33 trailing slash and dot are accepted for a real directory" 1 "summary: rows=4"

# 34. 上書きしない・後始末
newdir rec34
echo keep >"${work}/rec34/rec-mismatch.metal-mac-mini.json"
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec34" --record-combination metal-mac-mini
expect "34 existing file is not overwritten" 2 "error:" "!summary:" "!row:" "!${work}"
check "34 existing content kept and nothing else left" "$([ "$(cat "${work}/rec34/rec-mismatch.metal-mac-mini.json")" = keep ] && [ "$(nfiles "${work}/rec34")" = 1 ] && echo 0 || echo 1)"
newdir rec34b
echo target >"${work}/rec34b-target"
ln -s "${work}/rec34b-target" "${work}/rec34b/rec-missing.metal-mac-mini.json"
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec34b" --record-combination metal-mac-mini
expect "34 symlink with the same name is not followed" 2 "error:" "!summary:"
check "34 symlink target untouched and nothing else left" "$([ "$(cat "${work}/rec34b-target")" = target ] && [ "$(nfiles "${work}/rec34b")" = 0 ] && echo 0 || echo 1)"

# 35. 失敗したときは何も書かない
newdir rec35
res_rec 3f800001
FAKE_NODE_RC=3 w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec35" --record-combination metal-mac-mini
expect "35 node failure" 2 "node run failed" "!summary:"
printf '%s\n' 'not json' >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec35" --record-combination metal-mac-mini
expect "35 broken results" 2 "!summary:"
w --wasm "${dummy_wasm}" --cases "${fix}/does-not-exist.json" --record-dir "${work}/rec35" --record-combination metal-mac-mini
expect "35 broken cases" 2 "cannot read cases"
: >"${FAKE_RESULTS}"
w --wasm "${dummy_wasm}" --cases "${fix}/empty.json" --record-dir "${work}/rec35" --record-combination metal-mac-mini
expect "35 zero rows" 1 "rows=0" "no rows"
check "35 nothing written on failure" "$([ "$(nfiles "${work}/rec35")" = 0 ] && echo 0 || echo 1)"

# 36. mismatch の詳細は部品に入れない・ng に label_reason は付かない
res_rec 3f800001
newdir rec36
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec36" --record-combination metal-mac-mini
leak="$(cat "${work}"/rec36/*.json | grep -cE 'call error|first difference|length expected|mismatches' || true)"
check "36 mismatch detail is not in the parts" "$([ "${leak}" = 0 ] && echo 0 || echo 1)"
check "36 ng part has no label_reason" "$(grep -q 'label_reason' "${work}/rec36/rec-mismatch.metal-mac-mini.json" && echo 1 || echo 0)"
printf '%s\n' '{"id":"rec-match","output":["3f800000"]}' '{"id":"rec-mismatch","error":"call failed"}' '{"id":"rec-missing","output":["3f800000"]}' '{"id":"rec-unavail","output":["3f800000"]}' >"${FAKE_RESULTS}"
newdir rec36b
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec36b" --record-combination metal-mac-mini
leak="$(cat "${work}"/rec36b/*.json | grep -cE 'call error|call failed' || true)"
check "36 call error detail is not in the parts" "$([ "${leak}" = 0 ] && echo 0 || echo 1)"
res_ok

# ---- 生成の入力の行と 2 つのビルド（TASK-994 の 2/5） ----

DW="${dummy_wasm}"
cat >"${fix}/gen.json" <<'JSON'
{"rows": [
  {"id": "g-scalar", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "g-simd", "export": "gemm_simd", "build": "simd128", "case": "gemm-b", "reference_source": "native-scalar"}
]}
JSON
# gen_setup: 2 ケースの偽の host 側の道具の出力と、全行一致の結果を用意する。
gen_setup() {
  rm -rf "${fix}/hf" "${fix}/outs"
  mk_host_fixture "${fix}/hf" gemm-a gemm-b
  printf '%s\n' 'g-scalar gemm-a' 'g-simd gemm-b' >"${fix}/gen-rowmap"
  printf '%s\n' '{"id":"g-scalar","output_file":"g-scalar.bin"}' '{"id":"g-simd","output_file":"g-simd.bin"}' >"${FAKE_RESULTS}"
  export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
}
# gw <引数...>: 2 つの .wasm を渡して生成の行の入力を流す。
gw() {
  w --wasm "${DW}" --wasm-simd128 "${DW}" "$@"
}
no_node_started() {
  grep -q '^node ' "${FAKE_LOG}" && echo 1 || echo 0
}
# mut_out <モード> <出力のファイル>: 参照の写しを変えた出力を ${fix}/outs に書く。
mut_out() {
  mkdir -p "${fix}/outs"
  python3 -I - "$1" "${fix}/hf/gemm-a.native-scalar.bin" "${fix}/outs/$2" <<'PY'
import struct, sys
mode, src, dst = sys.argv[1:4]
w = list(struct.unpack("<4I", open(src, "rb").read()))
if mode == "nan-payload":
    w[2] = 0x7fc00002
elif mode == "neg-zero":
    w[1] = 0x00000000
elif mode == "short":
    w = w[:3]
data = b"".join(struct.pack("<I", x) for x in w)
if mode == "odd":
    data = data[:-1]
open(dst, "wb").write(data)
PY
}

# 37. 生成の行が全部一致: 0、count・sha256 の行、並び
gen_setup
gw --cases "${fix}/gen.json"
expect "37 generated rows all match" 0 "row: g-scalar match" "row: g-simd match" "count: g-scalar mismatches=0 n=4" "count: g-simd mismatches=0 n=4" "sha256: g-scalar " "sha256: g-simd " "sha256: gemm-a.a " "sha256: gemm-a.b " "sha256: gemm-a.native-scalar " "sha256: gemm-b.native-scalar " "summary: rows=2 match=2 mismatch=0 not-run=0"
check "37 output order is rows, counts, case hashes, summary last" "$(python3 -I -c 'import sys
L = [l for l in open(sys.argv[1]).read().splitlines() if not l.startswith(("node:", "wasm-bindgen:"))]
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
ok = first("row: g-scalar") < first("row: g-simd") < first("count: g-scalar") < first("sha256: g-scalar") < first("count: g-simd") < first("sha256: g-simd") < first("sha256: gemm-a.a") < first("sha256: gemm-a.native-scalar") < first("sha256: gemm-b.a") and L[-1].startswith("summary:")
sys.exit(0 if ok else 1)' "${OUT_F}" && echo 0 || echo 1)"
check "37 sha256 of a compared row is the hash of the wasm output bytes" "$(python3 -I -c 'import hashlib,sys
h = hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest()
sys.exit(0 if ("sha256: g-scalar " + h) in open(sys.argv[2]).read() else 1)' "${fix}/hf/gemm-a.native-scalar.bin" "${OUT_F}" && echo 0 || echo 1)"

# 38. 1 語だけ違う（NaN のペイロード・-0.0 と +0.0）は不一致、件数が出る
for mode in nan-payload neg-zero; do
  gen_setup
  mut_out "${mode}" g-scalar.bin
  export FAKE_OUTPUTS="${fix}/outs"
  gw --cases "${fix}/gen.json"
  expect "38 one word differs (${mode})" 1 "row: g-scalar mismatch (mismatches 1/4)" "count: g-scalar mismatches=1 n=4" "row: g-simd match" "match=1 mismatch=1"
  rm -rf "${fix}/outs"
done

# 39. 語の数の違い（語の数で出す。count の行は出ない）
gen_setup
mut_out short g-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/gen.json"
expect "39 length differs" 1 "row: g-scalar mismatch (length expected 4 got 3)" "!count: g-scalar" "!sha256: g-scalar" "count: g-simd mismatches=0"
rm -rf "${fix}/outs"

# 40. 呼び出しの失敗は call error（count の行は出ない）
gen_setup
printf '%s\n' '{"id":"g-scalar","error":"nonzero return"}' '{"id":"g-simd","error":"call failed"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/gen.json"
expect "40 call errors" 1 "row: g-scalar mismatch (call error)" "row: g-simd mismatch (call error)" "!count:" "!sha256: g-" "mismatch=2"

# 41. 固定の isa の行: simd128 の値違いは不一致
cat >"${fix}/isa.json" <<'JSON'
{"rows": [
  {"id": "isa-a", "export": "detect_isa_bits", "build": "scalar", "input": [], "reference_source": "fixed", "reference": ["00000000"]},
  {"id": "isa-b", "export": "detect_isa_bits", "build": "simd128", "input": [], "reference_source": "fixed", "reference": ["00000001"]}
]}
JSON
reset_gen
printf '%s\n' '{"id":"isa-a","output":["00000000"]}' '{"id":"isa-b","output":["00000001"]}' >"${FAKE_RESULTS}"
gw --cases "${fix}/isa.json"
expect "41 isa rows match" 0 "summary: rows=2 match=2"
check "41 no host tool is run without generated rows" "$(grep -q '^cargo run' "${FAKE_LOG}" && echo 1 || echo 0)"
printf '%s\n' '{"id":"isa-a","output":["00000000"]}' '{"id":"isa-b","output":["00000000"]}' >"${FAKE_RESULTS}"
gw --cases "${fix}/isa.json"
expect "41 isa-simd128 value differs" 1 "row: isa-b mismatch (first difference at index 0)" "row: isa-a match"

# 42. native-neon の生成の行は not-run (reference-unavailable)。比べず count・sha256 を出さない
gen_setup
cat >"${fix}/gen-neon.json" <<'JSON'
{"rows": [
  {"id": "g-scalar", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-neon"}
]}
JSON
printf '%s\n' '{"id":"g-scalar","output_file":"g-scalar.bin"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/gen-neon.json"
expect "42 native-neon generated row" 1 "row: g-scalar not-run (reference-unavailable)" "!count:" "!sha256:" "match=0 mismatch=0 not-run=1"

# 43. 入力の不正（終了コード 2・Node の本体を起動しない）
bad_case() {
  # bad_case <名前> <行の JSON> <期待の語>
  printf '{"rows": [%s]}\n' "$2" >"${fix}/bad-gen.json"
  gen_setup
  gw --cases "${fix}/bad-gen.json"
  expect "43 ${1}" 2 "$3" "!summary:"
  check "43 node body not started (${1})" "$(no_node_started)"
}
G='"export":"gemm_scalar","build":"scalar","case":"gemm-a","reference_source":"native-scalar"'
bad_case "generated row lacks build" '{"id":"x","export":"gemm_scalar","case":"gemm-a","reference_source":"native-scalar"}' "missing or unknown keys"
bad_case "generated row has an extra key" '{"id":"x",'"${G}"',"input":[]}' "missing or unknown keys"
bad_case "generated row lacks case" '{"id":"x","export":"gemm_scalar","build":"scalar","reference_source":"native-scalar"}' "missing or unknown keys"
bad_case "unknown build" '{"id":"x","export":"gemm_scalar","build":"avx","case":"gemm-a","reference_source":"native-scalar"}' "unknown build"
bad_case "unknown generated export" '{"id":"x","export":"gemm_other","build":"scalar","case":"gemm-a","reference_source":"native-scalar"}' "unknown export"
bad_case "case with a generated export name on a fixed row" '{"id":"x","export":"gemm_scalar","build":"scalar","input":[],"reference_source":"fixed"}' "missing or unknown keys"
bad_case "case key on a fixed export" '{"id":"x","export":"roundtrip_f32","build":"scalar","case":"gemm-a","reference_source":"native-scalar"}' "unknown export"
bad_case "fixed row with native-scalar" '{"id":"x","export":"roundtrip_f32","input":[],"reference_source":"native-scalar","reference":[]}' "reference_source not allowed"
bad_case "generated row with fixed" '{"id":"x","export":"gemm_scalar","build":"scalar","case":"gemm-a","reference_source":"fixed"}' "reference_source not allowed"
bad_case "detect_isa_bits with input" '{"id":"x","export":"detect_isa_bits","input":["00000000"],"reference_source":"fixed","reference":["00000000"]}' "takes no input"
bad_case "fixed row with unknown build" '{"id":"x","export":"roundtrip_f32","build":"avx","input":[],"reference_source":"fixed","reference":[]}' "unknown build"
bad_case "case with a bad form" '{"id":"x","export":"gemm_scalar","build":"scalar","case":"../x","reference_source":"native-scalar"}' "invalid form"
bad_case "unknown case" '{"id":"x","export":"gemm_scalar","build":"scalar","case":"nope","reference_source":"native-scalar"}' "unknown case in row: x"

# 44. manifest の不正（sha256・ファイル名・長さ・種類）
tamper() {
  # tamper <モード>: ${fix}/hf を壊す
  python3 -I - "$1" "${fix}/hf" <<'PY'
import hashlib, json, os, sys
mode, d = sys.argv[1:3]
mp = os.path.join(d, "manifest.json")
m = json.load(open(mp))
c = m["cases"][0]
if mode == "sha":
    with open(os.path.join(d, c["a_file"]), "r+b") as f:
        f.write(b"\xff")
elif mode == "slash":
    c["a_file"] = "../x"
elif mode == "dotdot":
    os.rename(os.path.join(d, c["a_file"]), os.path.join(d, "a..bin"))
    c["a_file"] = "a..bin"
elif mode == "dot-first":
    os.rename(os.path.join(d, c["a_file"]), os.path.join(d, ".a.bin"))
    c["a_file"] = ".a.bin"
elif mode == "length":
    data = open(os.path.join(d, c["a_file"]), "rb").read()[:-4]
    open(os.path.join(d, c["a_file"]), "wb").write(data)
    c["a_sha256"] = hashlib.sha256(data).hexdigest()
elif mode == "kind":
    c["kind"] = "topk"
elif mode == "dims":
    c["m"] = 0
elif mode == "extra":
    c["extra"] = 1
json.dump(m, open(mp, "w"))
PY
}
for mode in sha slash dotdot dot-first length kind dims extra; do
  gen_setup
  tamper "${mode}"
  gw --cases "${fix}/gen.json"
  expect "44 manifest tampered (${mode})" 2 "error:" "!summary:" "!${work}"
  check "44 node body not started (${mode})" "$(no_node_started)"
done
gen_setup
rm -f "${fix}/hf/manifest.json"
gw --cases "${fix}/gen.json"
expect "44 manifest missing" 2 "cannot read manifest" "!summary:"

# 45. 結果の形（生成の行は output_file、固定の行は output）
res_bad() {
  # res_bad <名前> <期待の語> <結果の行...>
  local name="$1" want="$2"
  shift 2
  gen_setup
  printf '%s\n' "$@" >"${FAKE_RESULTS}"
  gw --cases "${fix}/gen.json"
  expect "45 ${name}" 2 "${want}" "!summary:"
}
res_bad "generated row with output" "unexpected shape" '{"id":"g-scalar","output":["00000000"]}' '{"id":"g-simd","output_file":"g-simd.bin"}'
res_bad "output_file that is not <id>.bin" "unexpected shape" '{"id":"g-scalar","output_file":"other.bin"}' '{"id":"g-simd","output_file":"g-simd.bin"}'
res_bad "output_file with a path" "unexpected shape" '{"id":"g-scalar","output_file":"../g-scalar.bin"}' '{"id":"g-simd","output_file":"g-simd.bin"}'
gen_setup
rm -f "${fix}/gen-rowmap"
gw --cases "${fix}/gen.json"
expect "45 output files not written" 2 "cannot read an output file" "!summary:"
gen_setup
printf '%s\n' '{"id":"isa-a","output_file":"isa-a.bin"}' '{"id":"isa-b","output":["00000001"]}' >"${FAKE_RESULTS}"
reset_gen
gw --cases "${fix}/isa.json"
expect "45 fixed row with output_file" 2 "unexpected shape" "!summary:"
gen_setup
mut_out odd g-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/gen.json"
expect "45 native-scalar output length not a multiple of 4" 2 "not a multiple of 4" "!summary:"
rm -rf "${fix}/outs"

# 46. ビルドの口
reset_gen
printf '%s\n' '{"id":"isa-a","output":["00000000"]}' '{"id":"isa-b","output":["00000001"]}' >"${FAKE_RESULTS}"
w --wasm "${DW}" --cases "${fix}/isa.json"
expect "46 --wasm alone with a simd128 row" 2 "simd128 wasm file is required" "!summary:"
w --wasm-simd128 "${DW}" --cases "${fix}/isa.json"
expect "46 --wasm-simd128 alone with a scalar row" 2 "scalar wasm file is required" "!summary:"
gw --cases "${fix}/isa.json"
expect "46 both ports" 0 "summary: rows=2 match=2"
check "46 no cargo build with a port" "$(grep -q '^cargo build' "${FAKE_LOG}" && echo 1 || echo 0)"
printf '%s\n' '{"id":"isa-a","output":["00000000"]}' >"${FAKE_RESULTS}"
echo '{"rows": [{"id":"isa-a","export":"detect_isa_bits","build":"scalar","input":[],"reference_source":"fixed","reference":["00000000"]}]}' >"${fix}/isa-scalar-only.json"
w --wasm "${DW}" --cases "${fix}/isa-scalar-only.json"
expect "46 scalar-only rows need only --wasm" 0 "summary: rows=1 match=1"
check "46 only one wasm-bindgen call" "$([ "$(grep -c '^wasm-bindgen ' "${FAKE_LOG}")" = 1 ] && echo 0 || echo 1)"
w --wasm-simd128 "${DW}" --cases "${fix}/isa-scalar-only.json"
expect "46 --wasm-simd128 alone for scalar-only rows" 2 "scalar wasm file is required"

# 47. 既定の経路のビルド: 使うビルドだけ・env の形・target ディレクトリ
printf '%s\n' '{"id":"isa-a","output":["00000000"]}' '{"id":"isa-b","output":["00000001"]}' >"${FAKE_RESULTS}"
rm -rf "${FAKE_TARGET}"
RUSTFLAGS="-C debuginfo=2" CARGO_BUILD_RUSTFLAGS="-C debuginfo=2" w --cases "${fix}/isa.json"
expect "47 both builds" 0 "summary: rows=2 match=2"
check "47 two builds with -p wasm-glue and no --workspace" "$([ "$(grep -cE '^cargo build --manifest-path wasm/Cargo.toml -p wasm-glue --locked --target wasm32-unknown-unknown --profile wasm-size' "${FAKE_LOG}")" = 2 ] && ! grep -q -- '--workspace' "${FAKE_LOG}" && echo 0 || echo 1)"
check "47 scalar build has an empty encoded rustflags and no RUSTFLAGS" "$(grep -qxF 'env rustflags= RUSTFLAGS=unset' "${FAKE_LOG}" && echo 0 || echo 1)"
check "47 simd128 build has +simd128 and a separate target dir" "$(grep -qxF 'env rustflags=-C|target-feature=+simd128 RUSTFLAGS=unset' "${FAKE_LOG}" && grep -qE "^cargo build .*--target-dir ${FAKE_TARGET}/node-simd128\$" "${FAKE_LOG}" && echo 0 || echo 1)"
check "47 wasm-bindgen uses a separate out-dir per build" "$([ "$(grep '^wasm-bindgen ' "${FAKE_LOG}" | grep -o -- '--out-dir [^ ]*' | sort -u | wc -l | tr -d ' ')" = 2 ] && echo 0 || echo 1)"
printf '%s\n' '{"id":"isa-b","output":["00000001"]}' >"${FAKE_RESULTS}"
echo '{"rows": [{"id":"isa-b","export":"detect_isa_bits","build":"simd128","input":[],"reference_source":"fixed","reference":["00000001"]}]}' >"${fix}/isa-simd-only.json"
w --cases "${fix}/isa-simd-only.json"
expect "47 simd128-only rows" 0 "summary: rows=1 match=1"
check "47 only the simd128 build runs" "$([ "$(grep -c '^cargo build' "${FAKE_LOG}")" = 1 ] && grep -q 'node-simd128' "${FAKE_LOG}" && echo 0 || echo 1)"
printf '%s\n' '{"id":"isa-a","output":["00000000"]}' >"${FAKE_RESULTS}"
w --cases "${fix}/isa-scalar-only.json"
check "47 scalar-only rows do not build simd128" "$(grep -q 'node-simd128' "${FAKE_LOG}" && echo 1 || echo 0)"

# 48. host 側の道具: 生成の行があるときだけ呼ぶ・失敗は 2・cargo が要る
gen_setup
gw --cases "${fix}/gen.json"
check "48 host tool is run once with gen and an inputs directory" "$([ "$(grep -c '^cargo run --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked --quiet -- gen ' "${FAKE_LOG}")" = 1 ] && echo 0 || echo 1)"
gen_setup
FAKE_HOST_RC=3 gw --cases "${fix}/gen.json"
expect "48 host tool failure" 2 "host tool failed" "!summary:"
check "48 node body not started after a host tool failure" "$(no_node_started)"
mkbin "${work}/nocargo" python3 dirname basename sed head mktemp rm cat grep
cp "${bin}/node" "${bin}/wasm-bindgen" "${work}/nocargo/"
gen_setup
rc=0
PATH="${work}/nocargo" "${BASH}" "${WNC}" --wasm "${DW}" --wasm-simd128 "${DW}" --cases "${fix}/gen.json" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "48 generated rows need cargo even with ports" 2 "required command not found on PATH: cargo"

# 49. --record-dir: 生成の行も部品になる（mismatches の語は部品に入らない）
gen_setup
mut_out nan-payload g-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
newdir rec49
gw --cases "${fix}/gen.json" --record-dir "${work}/rec49" --record-combination metal-mac-mini
expect "49 generated rows with a record dir" 1 "row: g-scalar mismatch" "row: g-simd match"
check "49 two parts, ng and ok, without detail" "$([ "$(nfiles "${work}/rec49")" = 2 ] && grep -q '"label":"ng"' "${work}/rec49/g-scalar.metal-mac-mini.json" && grep -q '"label":"ok"' "${work}/rec49/g-simd.metal-mac-mini.json" && ! grep -rq 'mismatches' "${work}/rec49" && echo 0 || echo 1)"
rm -rf "${fix}/outs"

# 50. 誤用
w --wasm-simd128
expect "50 missing --wasm-simd128 value" 2 "usage:"
reset_gen
res_ok

# ---- 出力の保存と保存先からの数え直し（TASK-994 の 4/5） ----

# 2 ケース・2 行の gen.json の保存の形のファイル名。
SAVED_NAMES="g-scalar.bin g-simd.bin gemm-a.a.bin gemm-a.b.bin gemm-a.native-scalar.bin gemm-b.a.bin gemm-b.b.bin gemm-b.native-scalar.bin manifest.json outputs.json"
# dir_names_ok <dir> <空白区切りの名前>: ディレクトリの直下の名前の集合が完全一致で、全部が通常のファイル（0 なら成功）。
dir_names_ok() {
  python3 -I -c 'import os, sys
d = sys.argv[1]
got = set(os.listdir(d))
ok = got == set(sys.argv[2].split()) and all(os.path.isfile(os.path.join(d, n)) and not os.path.islink(os.path.join(d, n)) for n in got)
sys.exit(0 if ok else 1)' "$1" "$2" && echo 0 || echo 1
}

# 51. --output-dir: 名前の集合・中身・sha256 の行・出力の不変
gen_setup
gw --cases "${fix}/gen.json"
cp "${OUT_F}" "${work}/o51-without.txt"
newdir out51
gen_setup
gw --cases "${fix}/gen.json" --output-dir "${work}/out51"
expect "51 --output-dir writes the saved form" 0 "summary: rows=2 match=2" "!${work}/out51"
check "51 the file names are exactly the saved form" "$(dir_names_ok "${work}/out51" "${SAVED_NAMES}")"
check "51 manifest and inputs are copied byte for byte" "$(python3 -I -c 'import os, sys
hf, o = sys.argv[1:3]
names = os.listdir(hf)
ok = all(open(os.path.join(hf, n), "rb").read() == open(os.path.join(o, n), "rb").read() for n in names)
ok = ok and open(os.path.join(hf, "gemm-a.native-scalar.bin"), "rb").read() == open(os.path.join(o, "g-scalar.bin"), "rb").read()
sys.exit(0 if ok else 1)' "${fix}/hf" "${work}/out51" && echo 0 || echo 1)"
check "51 sha256 of each saved output is the sha256 line of the run" "$(python3 -I -c 'import hashlib, json, os, sys
o, out = sys.argv[1:3]
txt = open(out).read()
rows = json.load(open(os.path.join(o, "outputs.json")))["rows"]
ok = len(rows) == 2
for r in rows:
    h = hashlib.sha256(open(os.path.join(o, r["id"] + ".bin"), "rb").read()).hexdigest()
    ok = ok and h == r["output_sha256"] and ("sha256: " + r["id"] + " " + h + "\n") in txt
ok = ok and all(set(r) == {"id", "export", "build", "case", "output_file", "output_sha256"} for r in rows)
sys.exit(0 if ok else 1)' "${work}/out51" "${OUT_F}" && echo 0 || echo 1)"
check "51 stdout is the same with and without --output-dir" "$(cmp -s "${OUT_F}" "${work}/o51-without.txt" && echo 0 || echo 1)"

# 52. 上書きしない・リンクを辿らない・record の失敗で消える
newdir out52
echo keep >"${work}/out52/outputs.json"
gen_setup
gw --cases "${fix}/gen.json" --output-dir "${work}/out52"
expect "52 existing file is not overwritten" 2 "error:" "!summary:" "!row:" "!${work}"
check "52 existing content kept and nothing else left" "$([ "$(cat "${work}/out52/outputs.json")" = keep ] && [ "$(nfiles "${work}/out52")" = 1 ] && echo 0 || echo 1)"
newdir out52b
echo target >"${work}/out52b-target"
ln -s "${work}/out52b-target" "${work}/out52b/g-simd.bin"
gen_setup
gw --cases "${fix}/gen.json" --output-dir "${work}/out52b"
expect "52 symlink with the same name is not followed" 2 "error:" "!summary:"
check "52 symlink target untouched and nothing else left" "$([ "$(cat "${work}/out52b-target")" = target ] && [ "$(find "${work}/out52b" -type f | wc -l | tr -d ' ')" = 0 ] && echo 0 || echo 1)"
newdir out52c
newdir rec52c
echo keep >"${work}/rec52c/g-scalar.metal-mac-mini.json"
gen_setup
gw --cases "${fix}/gen.json" --output-dir "${work}/out52c" --record-dir "${work}/rec52c" --record-combination metal-mac-mini
expect "52 a failed record write removes the saved outputs" 2 "error:" "!summary:" "!row:"
check "52 nothing left in either directory but the existing record" "$([ "$(nfiles "${work}/out52c")" = 0 ] && [ "$(nfiles "${work}/rec52c")" = 1 ] && echo 0 || echo 1)"

# 53. 不正な書き出し先と値の欠け
newdir out53
ln -sfn "${work}/out53" "${work}/dirs/link53"
bad_out() {
  gen_setup
  gw --cases "${fix}/gen.json" --output-dir "$1"
  expect "53 invalid output dir ($2)" 2 "error:" "!summary:" "!$1"
  check "53 node body not started ($2)" "$(no_node_started)"
}
bad_out "${work}/dirs/missing" "missing"
bad_out "${work}/dirs/afile" "file"
bad_out "${work}/dirs/link53" "symlink"
bad_out "${work}/dirs/link53/" "symlink with slash"
bad_out "${work}/dirs/link53/." "symlink with dot"
bad_out "${work}/dirs/../out53" "parent component"
check "53 nothing written through the symlink" "$([ "$(nfiles "${work}/out53")" = 0 ] && echo 0 || echo 1)"
w --wasm "${DW}" --output-dir
expect "53 --output-dir without value" 2 "usage:"
w --wasm "${DW}" --output-dir ""
expect "53 empty --output-dir" 2 "usage:"
gen_setup
rc=0
(cd "${work}" && "${BASH}" "${WNC}" --wasm "${DW}" --wasm-simd128 "${DW}" --cases "${fix}/gen.json" --output-dir out53 >"${OUT_F}" 2>"${ERR_F}" </dev/null) || rc=$?
expect "53 relative output dir resolves from the caller cwd" 0 "summary: rows=2 match=2"
check "53 relative output dir got the saved form" "$(dir_names_ok "${work}/out53" "${SAVED_NAMES}")"

# 54. native-scalar の生成の行が無い入力で --output-dir は 2
newdir out54
for f in isa gen-neon empty; do
  gen_setup
  gw --cases "${fix}/${f}.json" --output-dir "${work}/out54"
  expect "54 --output-dir with no native-scalar generated row (${f})" 2 "needs native-scalar generated rows" "!summary:"
  check "54 node body not started (${f})" "$(no_node_started)"
done
check "54 nothing written" "$([ "$(nfiles "${work}/out54")" = 0 ] && echo 0 || echo 1)"

# 55. native-neon の生成の行は保存の形に入らない
cat >"${fix}/gen-mixed.json" <<'JSON'
{"rows": [
  {"id": "g-scalar", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "g-neon", "export": "gemm_simd", "build": "simd128", "case": "gemm-b", "reference_source": "native-neon"}
]}
JSON
gen_setup
printf '%s\n' 'g-scalar gemm-a' 'g-neon gemm-b' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"g-scalar","output_file":"g-scalar.bin"}' '{"id":"g-neon","output_file":"g-neon.bin"}' >"${FAKE_RESULTS}"
newdir out55
gw --cases "${fix}/gen-mixed.json" --output-dir "${work}/out55"
expect "55 mixed native-scalar and native-neon rows" 1 "row: g-scalar match" "row: g-neon not-run (reference-unavailable)" "recount: g-scalar mismatches=0 n=4" "recount-summary: rows=1 zero=1 nonzero=0 uncompared=0"
check "55 the neon output is not saved and not in outputs.json or the recount lines" "$([ ! -e "${work}/out55/g-neon.bin" ] && [ -e "${work}/out55/g-scalar.bin" ] && ! grep -q 'g-neon' "${work}/out55/outputs.json" && ! grep -q '^recount.*g-neon' "${OUT_F}" && echo 0 || echo 1)"

# 56. --recount: 0 と出力の形
gen_setup
gw --cases "${fix}/gen.json"
grep '^sha256: ' "${OUT_F}" >"${work}/sha-run.txt"
newdir out56
gen_setup
gw --cases "${fix}/gen.json" --output-dir "${work}/out56"
w --recount "${work}/out56"
expect "56 --recount all zero" 0 "recount: g-scalar mismatches=0 n=4" "recount: g-simd mismatches=0 n=4" "recount-summary: rows=2 zero=2 nonzero=0 uncompared=0" "sha256: g-scalar " "sha256: gemm-a.native-scalar "
check "56 the sha256 lines are the same bytes as the run" "$(grep '^sha256: ' "${OUT_F}" | cmp -s - "${work}/sha-run.txt" && echo 0 || echo 1)"
check "56 no row or count lines and no tool is started" "$(! grep -q -e '^count:' -e '^row:' -e '^summary:' "${OUT_F}" && [ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"
check "56 the last line is recount-summary" "$(tail -n 1 "${OUT_F}" | grep -q '^recount-summary: ' && echo 0 || echo 1)"

# saved_copy <新しいディレクトリ名>: out56 の写しを作る。
saved_copy() {
  rm -rf "${work:?}/$1"
  cp -R "${work}/out56" "${work}/$1"
}
# saved_mut <ディレクトリ名> <モード>: 保存の形を壊す。
saved_mut() {
  python3 -I - "${work}/$1" "$2" <<'PY'
import hashlib, json, os, sys
d, mode = sys.argv[1:3]
op = os.path.join(d, "outputs.json")
o = json.load(open(op))
def save():
    json.dump(o, open(op, "w"))
def flip(fn, at=0):
    p = os.path.join(d, fn)
    b = bytearray(open(p, "rb").read())
    b[at] ^= 1
    open(p, "wb").write(bytes(b))
    return bytes(b)
if mode == "word":
    b = flip("g-scalar.bin", 4)
    o["rows"][0]["output_sha256"] = hashlib.sha256(b).hexdigest()
    save()
elif mode == "out-sha":
    flip("g-scalar.bin")
elif mode == "input-sha":
    flip("gemm-a.a.bin")
elif mode == "ref-sha":
    flip("gemm-a.native-scalar.bin")
elif mode == "missing":
    os.unlink(os.path.join(d, "g-simd.bin"))
elif mode == "extra":
    open(os.path.join(d, "extra.txt"), "w").write("x")
elif mode == "subdir":
    os.mkdir(os.path.join(d, "sub"))
elif mode == "symlink":
    p = os.path.join(d, "gemm-a.b.bin")
    os.rename(p, p + ".real")
    os.symlink(p + ".real", p)
elif mode == "broken-json":
    open(op, "w").write("{")
elif mode == "extra-key":
    o["rows"][0]["x"] = 1
    save()
elif mode == "missing-key":
    del o["rows"][0]["build"]
    save()
elif mode == "file-name":
    o["rows"][0]["output_file"] = "other.bin"
    save()
elif mode == "file-path":
    o["rows"][0]["output_file"] = "../g-scalar.bin"
    save()
elif mode == "no-rows":
    o["rows"] = []
    save()
elif mode == "dup-id":
    o["rows"][1]["id"] = o["rows"][0]["id"]
    save()
elif mode == "half-null":
    o["rows"][0]["output_file"] = None
    save()
elif mode == "odd-length":
    p = os.path.join(d, "g-scalar.bin")
    b = open(p, "rb").read()[:-1]
    open(p, "wb").write(b)
    o["rows"][0]["output_sha256"] = hashlib.sha256(b).hexdigest()
    save()
elif mode == "unknown-case":
    o["rows"][0]["case"] = "nope"
    save()
elif mode == "unknown-export":
    o["rows"][0]["export"] = "gemm_other"
    save()
elif mode == "deep":
    open(op, "w").write("[" * 200000)
elif mode == "top-level-list":
    open(op, "w").write("[]")
else:
    raise SystemExit("unknown mode")
PY
}

# 57. 1 語書き換えて sha256 を直した保存の形は 1
saved_copy out57
saved_mut out57 word
w --recount "${work}/out57"
expect "57 one rewritten word with a fixed sha256" 1 "recount: g-scalar mismatches=1 n=4" "recount: g-simd mismatches=0 n=4" "recount-summary: rows=2 zero=1 nonzero=1 uncompared=0"

# 58. 2: 壊れた保存の形
for mode in out-sha input-sha ref-sha missing extra subdir symlink broken-json extra-key missing-key file-name file-path no-rows dup-id half-null odd-length unknown-case unknown-export deep top-level-list; do
  saved_copy out58
  saved_mut out58 "${mode}"
  w --recount "${work}/out58"
  expect "58 broken saved form (${mode})" 2 "error:" "!recount-summary:" "!${work}"
done
w --recount "${work}/dirs/missing"
expect "58 --recount missing directory" 2 "error:" "!${work}"
w --recount "${work}/dirs/afile"
expect "58 --recount on a file" 2 "error:" "!recount-summary:"
w --recount "${work}/dirs/link53"
expect "58 --recount on a symlink" 2 "error:" "!recount-summary:"
w --recount "${work}/dirs/../out56"
expect "58 --recount with a parent component" 2 "error:" "!recount-summary:"
saved_copy out58
printf '' >"${work}/out58/outputs.json"
w --recount "${work}/out58"
expect "58 empty outputs.json" 2 "error:"

# 59. --recount はほかの口と組めない
newdir out59
for opts in "--wasm ${DW}" "--wasm-simd128 ${DW}" "--cases ${fix}/gen.json" "--output-dir ${work}/out59" "--record-dir ${work}/out59 --record-combination metal-mac-mini" "--record-dir ${work}/out59"; do
  # shellcheck disable=SC2086
  w --recount "${work}/out56" ${opts}
  expect "59 --recount then ${opts%% *}" 2 "usage:" "!recount-summary:"
  # shellcheck disable=SC2086
  w ${opts} --recount "${work}/out56"
  expect "59 ${opts%% *} then --recount" 2 "usage:" "!recount-summary:"
done
w --recount
expect "59 --recount without value" 2 "usage:"
w --recount ""
expect "59 empty --recount" 2 "usage:"

# 60. --recount は node・wasm-bindgen・cargo を要らない
mkbin "${work}/recount-min" python3 dirname basename sed head mktemp rm cat grep
rc=0
PATH="${work}/recount-min" "${BASH}" "${WNC}" --recount "${work}/out56" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "60 --recount with only python3 on PATH" 0 "recount-summary: rows=2 zero=2"
w --recount "${work}/out56"
check "60 no fake command was called" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"

# 61. 実行の中の数え直し
gen_setup
gw --cases "${fix}/gen.json"
check "61 a recount line per compared row, once each" "$([ "$(grep -c '^recount: ' "${OUT_F}")" = 2 ] && [ "$(grep -c '^sha256: g-scalar ' "${OUT_F}")" = 1 ] && [ "$(grep -c '^recount-summary: ' "${OUT_F}")" = 1 ] && echo 0 || echo 1)"
check "61 recount lines come before summary and summary is last" "$(python3 -I -c 'import sys
L = [l for l in open(sys.argv[1]).read().splitlines() if not l.startswith(("node:", "wasm-bindgen:"))]
def first(p):
    return next(i for i, l in enumerate(L) if l.startswith(p))
ok = first("sha256: gemm-b.native-scalar") < first("recount: g-scalar") < first("recount: g-simd") < first("recount-summary:") < first("summary:") and L[-1].startswith("summary:")
sys.exit(0 if ok else 1)' "${OUT_F}" && echo 0 || echo 1)"
check "61 count lines equal recount lines" "$([ "$(grep '^count: ' "${OUT_F}" | sed 's/^count: //')" = "$(grep '^recount: ' "${OUT_F}" | sed 's/^recount: //')" ] && echo 0 || echo 1)"
gw --cases "${fix}/isa.json"
check "61 rows without native-scalar generated rows print no recount line" "$(grep -q 'recount' "${OUT_F}" && echo 1 || echo 0)"

# 62. 不一致のある実行
gen_setup
mut_out nan-payload g-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
newdir out62
gw --cases "${fix}/gen.json" --output-dir "${work}/out62"
expect "62 a mismatch run" 1 "count: g-scalar mismatches=1 n=4" "recount: g-scalar mismatches=1 n=4" "recount-summary: rows=2 zero=1 nonzero=1 uncompared=0" "!recount disagrees"
w --recount "${work}/out62"
expect "62 --recount of the saved mismatch run" 1 "recount: g-scalar mismatches=1 n=4" "nonzero=1"
rm -rf "${fix}/outs"

# 63. 呼び出しの失敗・語の数の違いは比べなかった行
gen_setup
printf '%s\n' '{"id":"g-scalar","error":"nonzero return"}' '{"id":"g-simd","output_file":"g-simd.bin"}' >"${FAKE_RESULTS}"
mut_out short g-simd.bin
export FAKE_OUTPUTS="${fix}/outs"
newdir out63
gw --cases "${fix}/gen.json" --output-dir "${work}/out63"
expect "63 uncompared rows" 1 "recount-uncompared: g-scalar" "recount-uncompared: g-simd" "recount-summary: rows=2 zero=0 nonzero=0 uncompared=2" "!recount: g-"
check "63 the failed row has no saved output and the short one is saved" "$([ ! -e "${work}/out63/g-scalar.bin" ] && [ "$(wc -c <"${work}/out63/g-simd.bin" | tr -d ' ')" = 12 ] && echo 0 || echo 1)"
w --recount "${work}/out63"
expect "63 --recount does not exit 0 with uncompared rows" 1 "recount-uncompared: g-scalar" "recount-summary: rows=2 zero=0 nonzero=0 uncompared=2"
rm -rf "${fix}/outs"

# 64. 判定と数え直しが食い違う場面: 数え直しの比べの式だけを変えた変異体
rm -rf "${work}/mut"
mkdir -p "${work}/mut/scripts" "${work}/mut/wasm/crates/glue"
cp "${DIR}/lib.sh" "${work}/mut/scripts/lib.sh"
cp "${GLUE_MANIFEST}" "${work}/mut/wasm/crates/glue/Cargo.toml"
python3 -I - "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" <<'PY'
import sys
s = open(sys.argv[1], encoding="utf-8").read()
old = "if data[i:i + 4] != want[i:i + 4])"
assert s.count(old) == 1
open(sys.argv[2], "w", encoding="utf-8").write(s.replace(old, "if data[i:i + 4] != want[i:i + 4] and i < 0)"))
PY
chmod +x "${work}/mut/scripts/wasm-node-check.sh"
check "64 the mutant differs from the original" "$(cmp -s "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" && echo 1 || echo 0)"
gen_setup
mut_out nan-payload g-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
newdir out64
newdir rec64
WNC_ORIG="${WNC}"
WNC="${work}/mut/scripts/wasm-node-check.sh"
gw --cases "${fix}/gen.json" --output-dir "${work}/out64" --record-dir "${work}/rec64" --record-combination metal-mac-mini
WNC="${WNC_ORIG}"
expect "64 a disagreeing recount stops the run" 2 "error: recount disagrees" "!row:" "!summary:" "!recount-summary:"
check "64 nothing written to either directory" "$([ "$(nfiles "${work}/out64")" = 0 ] && [ "$(nfiles "${work}/rec64")" = 0 ] && echo 0 || echo 1)"
rm -rf "${fix}/outs" "${work}/mut"

# 65. --output-dir を選ばない実行は一時ディレクトリ以外に何も残さない
gen_setup
newdir tmp65
newdir rec65
newdir out65
TMPDIR="${work}/tmp65" gw --cases "${fix}/gen.json"
expect "65 a run without --output-dir" 0 "summary: rows=2 match=2"
check "65 nothing is left in the temporary directory" "$([ "$(find "${work}/tmp65" -mindepth 1 | wc -l | tr -d ' ')" = 0 ] && echo 0 || echo 1)"
gen_setup
gw --cases "${fix}/gen.json" --record-dir "${work}/rec65" --record-combination metal-mac-mini --output-dir "${work}/out65"
expect "65 --record-dir with --output-dir" 0 "summary: rows=2 match=2"
check "65 record parts and saved form are separate" "$([ "$(nfiles "${work}/rec65")" = 2 ] && [ "$(dir_names_ok "${work}/out65" "${SAVED_NAMES}")" = 0 ] && echo 0 || echo 1)"
reset_gen
res_ok

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
