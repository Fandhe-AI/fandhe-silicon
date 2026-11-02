#!/usr/bin/env bash
# wasm-web-load-check.sh の回帰テスト（spec TASK-248 のポインタ）。
# node は、一時ディレクトリに置く偽のコマンドを PATH の先頭に入れて差し替える。偽の node は合成の結果のファイルを
# 書き出すので、判定の全経路を機材なし・ネットワークなしで確かめられる。load-web.mjs 自体は本物の node で偽の glue を読ませて確かめる。
# 呼び出し元: 直接実行（`bash scripts/test-wasm-web-load-check.sh`）。
#
# 判定は「終了コード + 標準出力・標準エラーの部分一致（先頭が ! の語は含まれないこと）」で行う。
# 期待値を緩めない。途中で止めず、最後に件数を出す。リポジトリのファイルは変更しない。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
# 偽のコマンドの本文と、展開させない合成の値を単引用符で書くため。
# shellcheck disable=SC2016
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd node

WWL="${DIR}/wasm-web-load-check.sh"
NODE_SCRIPT="${ROOT_DIR}/wasm/node/load-web.mjs"
# 偽の bin を PATH に入れる前に本物を控える。
REAL_NODE="$(command -v node)"

work="$(mktemp -d "${TMPDIR:-/tmp}/test-wasm-web-load-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
bin="${work}/bin"
res="${work}/results"
mkdir -p "${bin}" "${res}"
passed=0
failed=0
rc=0

unset FAKE_NODE_VERSION FAKE_NODE_RC

COMBOS="release-scalar release-simd128 wasm-size-scalar wasm-size-simd128"

# ---- 偽の node ----
# `--version` は版を返す。それ以外は `node <load-web.mjs> <組>/web <結果>` として呼ばれる。
export FAKE_LOG="${work}/calls.log"
export FAKE_RESULTS_DIR="${res}"
printf '#!/bin/sh\n%s\n' 'if [ "$1" = "--version" ]; then echo "${FAKE_NODE_VERSION:-v24.13.0}"; exit 0; fi
combo="$(basename "$(dirname "$2")")"
echo "node $combo" >>"$FAKE_LOG"
if [ -f "$FAKE_RESULTS_DIR/$combo.json" ]; then cp "$FAKE_RESULTS_DIR/$combo.json" "$3"
elif [ -f "$FAKE_RESULTS_DIR/default.json" ]; then cp "$FAKE_RESULTS_DIR/default.json" "$3"; fi
exit "${FAKE_NODE_RC:-0}"' >"${bin}/node"
chmod +x "${bin}/node"
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
  "${BASH}" "${WWL}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
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

# 最小 PATH: 指定したコマンドだけを symlink した bin。
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

n=0
# newfix: 4 組そろった入力を新しいディレクトリに作り、パスを fx に入れる。
fx=""
newfix() {
  n=$((n + 1))
  fx="${work}/fix${n}"
  local c
  for c in ${COMBOS}; do
    mkdir -p "${fx}/${c}/web"
    echo "// fake glue" >"${fx}/${c}/web/wasm_glue.js"
    echo "wasm" >"${fx}/${c}/web/wasm_glue_bg.wasm"
  done
}

# setres <json> [組...]: 組が無ければ default.json、あればその組の結果を書く。
setres() {
  local json="$1" c
  shift
  rm -f "${res}"/*.json
  if [ "$#" -eq 0 ]; then
    printf '%s\n' "${json}" >"${res}/default.json"
  else
    for c in "$@"; do printf '%s\n' "${json}" >"${res}/${c}.json"; done
  fi
}

ALL7='"gemm_scalar","gemm_simd","topk_scalar","topk_simd","elementwise_scalar","elementwise_simd","detect_isa_bits"'
setres_isa() {
  # setres_isa <scalar の値> <simd128 の値>: detect_isa_bits だけが在る結果。
  rm -f "${res}"/*.json
  printf '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":%s}\n' "$1" >"${res}/release-scalar.json"
  printf '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":%s}\n' "$1" >"${res}/wasm-size-scalar.json"
  printf '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":%s}\n' "$2" >"${res}/release-simd128.json"
  printf '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":%s}\n' "$2" >"${res}/wasm-size-simd128.json"
}

# ---- 偽の node での判定 ----

# 1. 7 名とも無い
newfix
setres '{"init":"ok","exports":[]}'
w --from-dir "${fx}"
expect "1 no exports" 1 "summary: combos=4 items=44 ok=0 ng=0 not-run=44" "reason=missing-export" "FAILED:"
check "1 44 missing-export rows" "$([ "$(grep -c 'reason=missing-export' "${OUT_F}")" = 44 ] && echo 0 || echo 1)"
check "1 node once per combo" "$([ "$(grep -c '^node ' "${FAKE_LOG}")" = 4 ] && echo 0 || echo 1)"

# 2. detect_isa_bits だけが在り値が合う
setres_isa 0 1
w --from-dir "${fx}"
expect "2 isa values match" 1 "summary: combos=4 items=44 ok=4 ng=0 not-run=40" "item: combo=release-scalar item=detect-isa-bits export=detect_isa_bits label=ok value=0" "item: combo=wasm-size-simd128 item=detect-isa-bits export=detect_isa_bits label=ok value=1"
check "2 four ok rows" "$([ "$(grep -c 'label=ok' "${OUT_F}")" = 4 ] && echo 0 || echo 1)"

# 3. 値が合わない
setres_isa 1 1
w --from-dir "${fx}"
expect "3 isa mismatch" 1 "item: combo=release-scalar item=detect-isa-bits export=detect_isa_bits label=ng value=1" "ok=2 ng=2"

# 4. call-failed
setres_isa '"call-failed"' '"call-failed"'
w --from-dir "${fx}"
expect "4 call-failed" 1 "label=ng reason=call-failed" "ng=4"

# 5. 7 名とも在り値も合う: 計算の 40 項目は not-run
setres '{"init":"ok","exports":['"${ALL7}"'],"detect_isa_bits":0}' release-scalar wasm-size-scalar
printf '{"init":"ok","exports":[%s],"detect_isa_bits":1}\n' "${ALL7}" >"${res}/release-simd128.json"
cp "${res}/release-simd128.json" "${res}/wasm-size-simd128.json"
w --from-dir "${fx}"
expect "5 all exports present" 1 "summary: combos=4 items=44 ok=4 ng=0 not-run=40" "reason=no-call-rule" "FAILED:"
check "5 forty no-call-rule rows" "$([ "$(grep -c 'reason=no-call-rule' "${OUT_F}")" = 40 ] && echo 0 || echo 1)"

# 6. 初期化の失敗
setres '{"init":"failed"}'
w --from-dir "${fx}"
expect "6 init failed" 1 "init: combo=release-scalar target=web result=ng" "reason=init-failed" "summary: combos=4 items=44 ok=0 ng=0 not-run=44"

# 7. 欠け
setres '{"init":"ok","exports":[]}'
newfix
rm -rf "${fx}/release-simd128"
w --from-dir "${fx}"
expect "7a missing combo dir" 1 "init: combo=release-simd128 target=web result=not-run reason=missing-directory" "item: combo=release-simd128 item=topk-simd export=topk_simd label=not-run reason=missing-directory"
check "7a node not called for the missing combo" "$(grep -q 'release-simd128' "${FAKE_LOG}" && echo 1 || echo 0)"
newfix
rm -rf "${fx}/release-scalar/web"
w --from-dir "${fx}"
expect "7b missing web dir" 1 "init: combo=release-scalar target=web result=not-run reason=missing-directory"
check "7b node not called for the missing web" "$(grep -q 'release-scalar' "${FAKE_LOG}" && echo 1 || echo 0)"

# 8. ファイルの欠け・空
newfix
rm "${fx}/wasm-size-scalar/web/wasm_glue.js"
w --from-dir "${fx}"
expect "8a missing file" 1 "init: combo=wasm-size-scalar target=web result=not-run reason=missing-file"
newfix
: >"${fx}/wasm-size-scalar/web/wasm_glue_bg.wasm"
w --from-dir "${fx}"
expect "8b empty file" 1 "init: combo=wasm-size-scalar target=web result=not-run reason=empty-file"

# 9. シンボリックリンク
newfix
mv "${fx}/release-scalar" "${work}/real-combo"
ln -s "${work}/real-combo" "${fx}/release-scalar"
w --from-dir "${fx}"
expect "9a symlinked combo dir" 2 "error:" "!summary:"
newfix
mv "${fx}/release-scalar/web" "${work}/real-web"
ln -s "${work}/real-web" "${fx}/release-scalar/web"
w --from-dir "${fx}"
expect "9b symlinked web dir" 2 "error:" "!summary:"
newfix
ln -sf "${fx}/release-simd128/web/wasm_glue_bg.wasm" "${fx}/release-scalar/web/wasm_glue.js"
w --from-dir "${fx}"
expect "9c symlinked file" 2 "error:" "!summary:"

# 10. --from-dir が無いパス・ファイル
w --from-dir "${work}/nonexistent"
expect "10a missing from-dir" 2 "error:" "!summary:"
echo x >"${work}/afile"
w --from-dir "${work}/afile"
expect "10b from-dir is a file" 2 "error:" "!summary:"

# 11. node の版違い
newfix
setres '{"init":"ok","exports":[]}'
FAKE_NODE_VERSION=v24.21.0 w --from-dir "${fx}"
expect "11 node version mismatch" 2 "node version mismatch" "expected v24.13.0" "got v24.21.0" "!summary:"
check "11 node script not called" "$([ ! -s "${FAKE_LOG}" ] && echo 0 || echo 1)"

# 12. 壊れた結果
newfix
for case_json in \
  'not json' \
  '[]' \
  '{"init":"weird"}' \
  '{"init":"ok","exports":[1]}' \
  '{"init":"ok","exports":["detect_isa_bits"]}' \
  '{"init":"ok","exports":[],"detect_isa_bits":0}' \
  '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":-1}' \
  '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":4294967296}' \
  '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":0.5}' \
  '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":true}' \
  '{"init":"ok","exports":[],"extra":1}' \
  '{"init":"failed","exports":[]}' \
  '{"init":"ok","exports":["gemm_scalar","gemm_scalar"]}'; do
  setres "${case_json}"
  w --from-dir "${fx}"
  expect "12 broken result ${case_json}" 2 "error:" "!summary:"
done
rm -f "${res}"/*.json
w --from-dir "${fx}"
expect "12 missing result file" 2 "error:" "!summary:"

# 13. 許可表の外の名前
setres '{"init":"ok","exports":["roundtrip_f32"]}'
w --from-dir "${fx}"
expect "13 unknown export name" 2 "error:" "!summary:"

# 14. node の失敗
setres '{"init":"ok","exports":[]}'
FAKE_NODE_RC=3 w --from-dir "${fx}"
expect "14 node run failed" 2 "node run failed" "!summary:"

# 15. 引数の誤用
w
expect "15a no args" 2 "usage:"
w --from-dir
expect "15b no value" 2 "usage:"
w --from-dir ""
expect "15c empty value" 2 "usage:"
w --from-dir "${fx}" --from-dir "${fx}"
expect "15d twice" 2 "usage:"
w --from-dir "${fx}" --bogus
expect "15e unknown" 2 "usage:"
w --bogus
expect "15f unknown only" 2 "usage:"

# 16. 余計な項目があっても同じ結果
newfix
setres '{"init":"ok","exports":[]}'
mkdir -p "${fx}/bundler" "${fx}/other" "${fx}/release-scalar/bundler"
echo x >"${fx}/release-scalar/web/package.json"
w --from-dir "${fx}"
expect "16 extra entries" 1 "summary: combos=4 items=44 ok=0 ng=0 not-run=44"

# 17. 一時ディレクトリのパスを出さない
w --from-dir "${fx}"
expect "17 no path leak" 1 "!${work}"

# 18. node・python3 が無い
mkbin "${work}/nonode" python3 dirname basename sed head mktemp rm cat grep
w_min() {
  rc=0
  PATH="$1" "${BASH}" "${WWL}" --from-dir "${fx}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}
w_min "${work}/nonode"
expect "18a node missing" 2 "node"
mkbin "${work}/nopy" dirname basename sed head mktemp rm cat grep
cp "${bin}/node" "${work}/nopy/node"
w_min "${work}/nopy"
expect "18b python3 missing" 2 "python3"

# ---- 本物の node で load-web.mjs を直接流す ----
# 偽の glue は ESM（package.json の type で明示）。計算の 6 名は呼ばれたら 97 で終わり、マーカーを書く。

real_dir="${work}/real"
mkdir -p "${real_dir}"
marker="${real_dir}/called"

# mkglue <名前> <glue の本文>: 組のディレクトリを新しく作りパスを gd に入れる。
gd=""
mkglue() {
  gd="${real_dir}/$1"
  mkdir -p "${gd}"
  echo '{"type":"module"}' >"${gd}/package.json"
  printf '%s\n' "$2" >"${gd}/wasm_glue.js"
  printf 'abc' >"${gd}/wasm_glue_bg.wasm"
}

INIT_OK='export function initSync(arg) { if (typeof arg !== "object" || !(arg.module instanceof Uint8Array) || arg.module.length !== 3) throw new Error("bad init"); }'
COMPUTE="$(for f in gemm_scalar gemm_simd topk_scalar topk_simd elementwise_scalar elementwise_simd; do
  printf 'export function %s() { import("node:fs").then((m) => m.writeFileSync("%s", "x")); process.exit(97); }\n' "${f}" "${marker}"
done)"

# runreal <glue のディレクトリ>: load-web.mjs を流し、結果を rjson に入れる。
rjson=""
runreal() {
  rc=0
  rm -f "${real_dir}/out.json"
  "${REAL_NODE}" "${NODE_SCRIPT}" "$1" "${real_dir}/out.json" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
  rjson="$(cat "${real_dir}/out.json" 2>/dev/null || true)"
}

# realeq <名前> <期待の JSON>: 出力を JSON として等値で比べる。
realeq() {
  local ok
  ok="$(python3 -I -c 'import json,sys
a=json.loads(sys.argv[1]); b=json.loads(sys.argv[2])
print(0 if a==b else 1)' "${rjson:-null}" "$2" 2>/dev/null || echo 1)"
  if [ "${rc}" -ne 0 ] || [ "${ok}" != 0 ]; then
    fail "$1" "rc=${rc} json=${rjson}"
  else
    pass "$1"
  fi
}

rm -f "${marker}"
mkglue all7 "${INIT_OK}
${COMPUTE}
export function detect_isa_bits() { return 1; }"
runreal "${gd}"
realeq "r1 all seven exports" '{"init":"ok","exports":["gemm_scalar","gemm_simd","topk_scalar","topk_simd","elementwise_scalar","elementwise_simd","detect_isa_bits"],"detect_isa_bits":1}'
check "r1 compute exports not called" "$([ ! -e "${marker}" ] && echo 0 || echo 1)"

mkglue neg "${INIT_OK}
export function detect_isa_bits() { return -1; }"
runreal "${gd}"
realeq "r2 negative wraps to u32" '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":4294967295}'

mkglue throws "${INIT_OK}
export function detect_isa_bits() { throw new Error('boom'); }"
runreal "${gd}"
realeq "r3 detect_isa_bits throws" '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":"call-failed"}'

mkglue undef "${INIT_OK}
export function detect_isa_bits() { }"
runreal "${gd}"
realeq "r4 detect_isa_bits returns undefined" '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":"call-failed"}'

mkglue partial "${INIT_OK}
export function roundtrip_f32() { return 0; }
export function gemm_scalar() { process.exit(97); }"
runreal "${gd}"
realeq "r5 partial exports" '{"init":"ok","exports":["gemm_scalar"]}'

mkglue initthrows 'export function initSync() { throw new Error("secret-detail"); }'
runreal "${gd}"
realeq "r6a initSync throws" '{"init":"failed"}'
check "r6a no detail on stderr" "$(grep -q 'secret-detail' "${ERR_F}" "${OUT_F}" && echo 1 || echo 0)"
check "r6a no path on stderr" "$(grep -qF "${real_dir}" "${ERR_F}" "${OUT_F}" && echo 1 || echo 0)"

mkglue noinit 'export function detect_isa_bits() { return 0; }'
runreal "${gd}"
realeq "r6b no initSync" '{"init":"failed"}'

mkglue nojs "${INIT_OK}"
rm "${gd}/wasm_glue.js"
runreal "${gd}"
realeq "r6c no wasm_glue.js" '{"init":"failed"}'

mkglue syntaxerr 'export function ('
runreal "${gd}"
realeq "r6d syntax error" '{"init":"failed"}'
check "r6d no path on stderr" "$(grep -qF "${real_dir}" "${ERR_F}" "${OUT_F}" && echo 1 || echo 0)"

mkglue nowasm "${INIT_OK}"
rm "${gd}/wasm_glue_bg.wasm"
runreal "${gd}"
realeq "r6e no wasm file" '{"init":"failed"}'

mkglue withdts "${INIT_OK}
export function detect_isa_bits() { return 0; }"
echo 'export function detect_isa_bits(): number;' >"${gd}/wasm_glue.d.ts"
runreal "${gd}"
realeq "r7 extra d.ts is ignored" '{"init":"ok","exports":["detect_isa_bits"],"detect_isa_bits":0}'

# 引数の数
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "r8a zero args" 2 "usage:"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${gd}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "r8b one arg" 2 "usage:"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${gd}" "${real_dir}/o.json" extra >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
expect "r8c three args" 2 "usage:"

echo "test-wasm-web-load-check: ${passed} passed, ${failed} failed"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
