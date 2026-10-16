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

unset FAKE_NODE_VERSION FAKE_NODE_RC FAKE_WB_VERSION FAKE_WB_NO_GLUE FAKE_CARGO_RC FAKE_CARGO_NO_ARTIFACT

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
  build)
    [ -z "${FAKE_CARGO_NO_ARTIFACT:-}" ] || exit "${FAKE_CARGO_RC:-0}"
    d="$FAKE_TARGET/wasm32-unknown-unknown/wasm-size"
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

# 22. 同梱の入力（偽 node が参照どおりの結果を返すと 0）
cat >"${FAKE_RESULTS}" <<'EOF'
{"id":"roundtrip-empty","output":[]}
{"id":"roundtrip-finite","output":["3f800000","80000000","7f7fffff"]}
{"id":"roundtrip-non-finite","output":["7f800000","ff800000","7fc00000","80000000"]}
EOF
w --wasm "${dummy_wasm}"
expect "22 bundled cases" 0 "summary: rows=3 match=3 mismatch=0 not-run=0"
check "22 every bundled row is fixed and has a reference" "$(python3 -I -c 'import json,sys
d = json.load(open(sys.argv[1]))
sys.exit(0 if d["rows"] and all(r.get("reference_source") == "fixed" and isinstance(r.get("reference"), list) for r in d["rows"]) else 1)' "${CASES_REAL}" && echo 0 || echo 1)"

# 23. run-glue.cjs 自体（本物の node で偽の glue を読ませる）
mkdir -p "${work}/glue"
echo 'exports.roundtrip_f32 = (a) => a;' >"${work}/glue/id.js"
echo 'exports.roundtrip_f32 = (a) => a.map((x) => x + 1);' >"${work}/glue/inc.js"
echo 'exports.roundtrip_f32 = () => { throw new Error("boom"); };' >"${work}/glue/throw.js"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/id.js" "${CASES_REAL}" "${work}/glue/r1.jsonl" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue identity glue" 0
if python3 -I -c 'import json,sys
rows = {r["id"]: r for r in json.load(open(sys.argv[1]))["rows"]}
out = [json.loads(l) for l in open(sys.argv[2])]
sys.exit(0 if len(out) == len(rows) and all(o["output"] == rows[o["id"]]["reference"] for o in out) else 1)' "${CASES_REAL}" "${work}/glue/r1.jsonl"; then
  pass "23 run-glue keeps bit patterns (NaN, -0.0, Inf)"
else
  fail "23 run-glue keeps bit patterns (NaN, -0.0, Inf)" "output differs from reference"
fi
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/inc.js" "${CASES_REAL}" "${work}/glue/r2.jsonl" >"${OUT_F}" 2>"${ERR_F}" || true
check "23 run-glue reports a changed value" "$(grep -q '"id":"roundtrip-finite","output":\["3f800000"' "${work}/glue/r2.jsonl" && echo 1 || echo 0)"
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/throw.js" "${CASES_REAL}" "${work}/glue/r3.jsonl" >"${OUT_F}" 2>"${ERR_F}" || true
check "23 run-glue turns a throw into an error row" "$(grep -qc 'call failed' "${work}/glue/r3.jsonl" && echo 0 || echo 1)"
echo '{"rows":[{"id":"a","export":"constructor","input":[]}]}' >"${fix}/ctor.json"
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/id.js" "${fix}/ctor.json" "${work}/glue/r4.jsonl" >"${OUT_F}" 2>"${ERR_F}" || true
check "23 run-glue rejects a non-listed export" "$(grep -q 'unknown export' "${work}/glue/r4.jsonl" && echo 0 || echo 1)"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue usage" 2 "usage:"

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
for t in fixed native-neon; do
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
leak="$(cat "${work}"/rec36/*.json | grep -cE 'call error|first difference|length expected' || true)"
check "36 mismatch detail is not in the parts" "$([ "${leak}" = 0 ] && echo 0 || echo 1)"
check "36 ng part has no label_reason" "$(grep -q 'label_reason' "${work}/rec36/rec-mismatch.metal-mac-mini.json" && echo 1 || echo 0)"
printf '%s\n' '{"id":"rec-match","output":["3f800000"]}' '{"id":"rec-mismatch","error":"call failed"}' '{"id":"rec-missing","output":["3f800000"]}' '{"id":"rec-unavail","output":["3f800000"]}' >"${FAKE_RESULTS}"
newdir rec36b
w --wasm "${dummy_wasm}" --cases "${fix}/rec.json" --record-dir "${work}/rec36b" --record-combination metal-mac-mini
leak="$(cat "${work}"/rec36b/*.json | grep -cE 'call error|call failed' || true)"
check "36 call error detail is not in the parts" "$([ "${leak}" = 0 ] && echo 0 || echo 1)"
res_ok

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
