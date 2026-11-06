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
CASES_UNAVAILABLE_REAL="${ROOT_DIR}/wasm/node/cases-reference-unavailable.json"
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
    if name.startswith("special-gemm-") or name.startswith("special-elementwise-"):
        # 特殊値の入力の種類の合成のケース（TASK-999）: 参照の 8 語は添字の順に 1000.0・1024.0・2^-20・+0・1.0・NaN・+Inf・-Inf。
        # gemm は m=2・k=3・n=4（a 6 語・b 12 語・出力 8 語）、elementwise は n=8（入力は NaN・±Inf・±0 を含む）。
        # CPU 参照はネイティブの参照と同じ語で、名前に cpudiff を含めば語 4 だけ 2.0 にする。名前に add を含めば op_code 1・定数 0.5。
        nat = [0x447a0000, 0x44800000, 0x35800000, 0x00000000, 0x3f800000, 0x7fc00000, 0x7f800000, 0xff800000]
        cpu = list(nat)
        if "cpudiff" in name:
            cpu[4] = 0x40000000
        if name.startswith("special-gemm-"):
            cases.append({
                "name": name, "kind": "gemm-special", "m": 2, "k": 3, "n": 4,
                "a_file": name + ".a.bin", "b_file": name + ".b.bin", "native_scalar_file": name + ".native-scalar.bin",
                "cpu_reference_file": name + ".cpu-reference.bin",
                "a_sha256": wr(name + ".a.bin", [0x3f800000, 0x80000000, 0x7fc00001, 0x40000000, 0xc0000000, 0x00000000]),
                "b_sha256": wr(name + ".b.bin", [0x3f800000] * 12),
                "native_scalar_sha256": wr(name + ".native-scalar.bin", nat),
                "cpu_reference_sha256": wr(name + ".cpu-reference.bin", cpu),
            })
        else:
            add = "add" in name
            cases.append({
                "name": name, "kind": "elementwise-special", "n": 8,
                "op_code": 1 if add else 0, "c_bits": "3f000000" if add else "00000000",
                "input_file": name + ".input.bin", "native_scalar_file": name + ".native-scalar.bin",
                "cpu_reference_file": name + ".cpu-reference.bin",
                "input_sha256": wr(name + ".input.bin", [0x7fc00001, 0x7f800000, 0xff800000, 0x00000000, 0x80000000, 0x80000000, 0x40000000, 0x40400000]),
                "native_scalar_sha256": wr(name + ".native-scalar.bin", nat),
                "cpu_reference_sha256": wr(name + ".cpu-reference.bin", cpu),
            })
        continue
    if name.startswith("topk-"):
        # 部分 Top-k の合成のケース（TASK-996）: n=6・k=2。スコアは非有限が 4 語（NaN 2・+Inf・-Inf）と有限が 2 語。
        # 参照は有限の 2 件。badscore の名前は参照のスコアに非有限を持ち、スロット番号にも指数部が全部 1 の語を置く。
        sc = [0x00000000, 0x7fc00001, 0x7fc00002, 0x7f800000, 0xff800000, 0x40800000]
        nat = [0x7f800000, 0x7fc00001, 0x7f800000, 0x00000000] if "badscore" in name else [5, 0x40800000, 0, 0x00000000]
        cases.append({
            "name": name, "kind": "topk", "n": 6, "k": 2,
            "scores_file": name + ".scores.bin", "native_scalar_file": name + ".native-scalar.bin",
            "scores_sha256": wr(name + ".scores.bin", sc), "native_scalar_sha256": wr(name + ".native-scalar.bin", nat),
        })
        continue
    if name.startswith("elementwise-"):
        # 要素ごと演算の合成のケース（TASK-997）: n=5。既定の入力の語は NaN・Inf・±0 を含まない。名前に add を含めば
        # op_code 1・定数 0.5、それ以外は op_code 0・定数 0。special の名前は NaN 1 語・Inf 2 語（+Inf・-Inf）・零 3 語（+0・-0・-0）の 6 語（nan・inf・zero の 3 つの数が全部違う）、
        # nanbits の名前は NaN の 2 つのビット列（quiet の最小と signaling の最小）を含む入力にする。nanin の名前は語 2・語 4 に
        # payload つきの NaN を置く入力にする。
        inp = [0x3f800000, 0xbf800000, 0x40000000, 0xc0000000, 0x3f000000]
        if "special" in name:
            inp = [0x7fc00001, 0x7f800000, 0xff800000, 0x00000000, 0x80000000, 0x80000000]
        elif "nanin" in name:
            inp = [0x3f800000, 0xbf800000, 0x7fc00003, 0xc0000000, 0x7f800002]
        elif "nanbits" in name:
            inp = [0x7fc00000, 0x7f800001, 0x3f800000, 0x3f800000, 0x3f800000]
        nat = [0x3f800000, 0x00000000, 0x7fc00001, 0x00000000, 0x3f000000]
        if len(inp) > len(nat):
            nat = nat + [0x00000000] * (len(inp) - len(nat))
        add = "add" in name
        cases.append({
            "name": name, "kind": "elementwise", "n": len(inp),
            "op_code": 1 if add else 0, "c_bits": "3f000000" if add else "00000000",
            "input_file": name + ".input.bin", "native_scalar_file": name + ".native-scalar.bin",
            "input_sha256": wr(name + ".input.bin", inp), "native_scalar_sha256": wr(name + ".native-scalar.bin", nat),
        })
        continue
    if name.startswith("gemm-t1"):
        # 閾値の比べ用の合成のケース（TASK-995）: m=1・k=1・n=8。参照は添字の順に 1000.0・1024.0・2^-20・+0・1.0・NaN・+Inf・-Inf。
        nat = [0x447a0000, 0x44800000, 0x35800000, 0x00000000, 0x3f800000, 0x7fc00000, 0x7f800000, 0xff800000]
        cases.append({
            "name": name, "kind": "gemm", "m": 1, "k": 1, "n": 8,
            "a_file": name + ".a.bin", "b_file": name + ".b.bin", "native_scalar_file": name + ".native-scalar.bin",
            "a_sha256": wr(name + ".a.bin", [0x3f800000]), "b_sha256": wr(name + ".b.bin", [0x3f800000] * 8),
            "native_scalar_sha256": wr(name + ".native-scalar.bin", nat),
        })
        continue
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
# add_neon_ref <dir> <ケース> [添字=語...]: mk_host_fixture の出力の <ケース>.native-scalar.bin を写して
# <ケース>.native-neon.bin を書き（添字=語 で語を置き換える）、manifest のそのケースに 2 つの鍵を足す（TASK-995）。
add_neon_ref() {
  python3 -I - "$@" <<'PY'
import hashlib, json, os, sys
d, name = sys.argv[1], sys.argv[2]
with open(os.path.join(d, name + ".native-scalar.bin"), "rb") as f:
    data = bytearray(f.read())
for a in sys.argv[3:]:
    i, w = a.split("=")
    data[int(i) * 4:int(i) * 4 + 4] = int(w, 16).to_bytes(4, "little")
with open(os.path.join(d, name + ".native-neon.bin"), "wb") as f:
    f.write(bytes(data))
mp = os.path.join(d, "manifest.json")
m = json.load(open(mp))
for c in m["cases"]:
    if c["name"] == name:
        c["native_neon_file"] = name + ".native-neon.bin"
        c["native_neon_sha256"] = hashlib.sha256(bytes(data)).hexdigest()
json.dump(m, open(mp, "w"), indent=2)
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
  local nc
  for nc in $(python3 -I -c 'import json,sys
print(" ".join(sorted({r["case"] for r in json.load(open(sys.argv[1]))["rows"] if r.get("reference_source") == "native-neon"})))' "${cf}"); do
    add_neon_ref "${fix}/hf-${name}" "${nc}"
  done
  export FAKE_HOST_FIXTURE="${fix}/hf-${name}" FAKE_ROW_MAP="${fix}/rowmap-${name}"
  w --wasm "${dummy_wasm}" --wasm-simd128 "${dummy_wasm}" --cases "${cf}"
  reset_gen
  expect "22 ${name}" 0 "summary: rows=${n} match=${n} mismatch=0 not-run=0"
}
# 偽の host 側の道具の寸法は同梱の `cases` の欄の寸法と違うので、行の入力だけの写しを流す（欄と host の表の一致は
# wasm/crates/parity-host/tests/recount_expected.rs が実物で確かめる）。
python3 -I - "${CASES_REAL}" "${fix}/bundled-real.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
del d["cases"]
json.dump(d, open(sys.argv[2], "w"))
PY
bundled_check bundled "${fix}/bundled-real.json"
check "22 bundled cases table names exactly the cases the generated rows use" "$(python3 -I -c 'import json,sys
d = json.load(open(sys.argv[1]))
sys.exit(0 if {c["name"] for c in d["cases"]} == {r["case"] for r in d["rows"] if "case" in r} and len(d["cases"]) == len({c["name"] for c in d["cases"]}) else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
# 実物の生成の行（export が gemm_ で始まる行）は 17 ケース × 4 組の 68 行にちょうど等しい（ほかの種類の行は数えない）（TASK-999）
check "22 bundled generated rows are exactly 17 cases x (scalar gemm_scalar, scalar gemm_simd, simd128 gemm_scalar, simd128 gemm_simd)" "$(python3 -I -c 'import json,sys
rows = [r for r in json.load(open(sys.argv[1]))["rows"] if str(r.get("export", "")).startswith("gemm_") and r.get("reference_source") == "native-scalar"]
want = sorted((c, e, b) for c in ("gemm-64", "gemm-256", "gemm-37x41x53", "special-gemm-g-01", "special-gemm-g-02", "special-gemm-g-03", "special-gemm-g-04", "special-gemm-g-05", "special-gemm-g-06", "special-gemm-g-07", "special-gemm-g-08", "special-gemm-g-09", "special-gemm-g-10", "special-gemm-g-11", "special-gemm-g-12", "special-gemm-g-13", "special-gemm-e-1") for e, b in (("gemm_scalar", "scalar"), ("gemm_simd", "scalar"), ("gemm_scalar", "simd128"), ("gemm_simd", "simd128")))
got = sorted((r.get("case"), r.get("export"), r.get("build")) for r in rows)
sys.exit(0 if got == want else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
# 実物の部分 Top-k の行（export が topk_ で始まる行）は 42 組にちょうど等しい（TASK-996・TASK-2451・TASK-999）
check "22 bundled topk rows are exactly the forty-two (case, export, build) pairs" "$(python3 -I -c 'import json,sys
rows = [r for r in json.load(open(sys.argv[1]))["rows"] if str(r.get("export", "")).startswith("topk_")]
four = [("topk_scalar", "scalar"), ("topk_simd", "scalar"), ("topk_scalar", "simd128"), ("topk_simd", "simd128")]
want = sorted([(c, e, b) for c in ("topk-batch", "topk-ties", "topk-e-4", "topk-signed-zero-adjacent", "topk-signed-zero-separated", "topk-signed-zero-cut-at-k", "topk-signed-zero-repeated", "topk-e-4-k-13", "topk-e-3", "topk-pad-after-one-finite") for e, b in four] + [("topk-nonfinite", "topk_scalar", "scalar"), ("topk-nonfinite", "topk_simd", "simd128")])
got = sorted((r.get("case"), r.get("export"), r.get("build")) for r in rows)
sys.exit(0 if got == want else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
# 実物の要素ごと演算の行（export が elementwise_ で始まる行）は 20 組にちょうど等しい（TASK-997・TASK-999）
check "22 bundled elementwise rows are exactly the twenty (case, export, build) pairs" "$(python3 -I -c 'import json,sys
rows = [r for r in json.load(open(sys.argv[1]))["rows"] if str(r.get("export", "")).startswith("elementwise_")]
four = [("elementwise_scalar", "scalar"), ("elementwise_simd", "scalar"), ("elementwise_scalar", "simd128"), ("elementwise_simd", "simd128")]
want = sorted((c, e, b) for c in ("elementwise-relu", "elementwise-add-scalar", "elementwise-add-scalar-c-0-1", "special-elementwise-relu-e-2", "special-elementwise-add-scalar-e-2") for e, b in four)
got = sorted((r.get("case"), r.get("export"), r.get("build")) for r in rows)
sys.exit(0 if got == want else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
check "22 bundled native-neon rows are exactly the three gemm_simd simd128 rows" "$(python3 -I -c 'import json,sys
rows = [r for r in json.load(open(sys.argv[1]))["rows"] if r.get("reference_source") == "native-neon"]
want = [{"id": c + "-native-neon", "export": "gemm_simd", "build": "simd128", "case": c, "reference_source": "native-neon"} for c in ("gemm-64", "gemm-256", "gemm-37x41x53")]
sys.exit(0 if rows == want else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
check "22 every bundled row has a reference source of fixed or native-scalar, except the three native-neon rows" "$(python3 -I -c 'import json,sys
rows = json.load(open(sys.argv[1]))["rows"]
sys.exit(0 if rows and all(r.get("reference_source") in ("fixed", "native-scalar", "native-neon") for r in rows) else 1)' "${CASES_REAL}" && echo 0 || echo 1)"
check "22 every bundled fixed row has a reference, every generated row has the exact five keys" "$(python3 -I -c 'import json,sys
d = json.load(open(sys.argv[1]))
def ok(r):
    if "case" in r or str(r.get("export", "")).startswith("gemm_"):
        return set(r) == {"id", "export", "build", "case", "reference_source"} and r["reference_source"] in ("native-scalar", "native-neon")
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
    for exp, build in (("gemm_scalar", "scalar"), ("gemm_simd", "scalar"), ("gemm_scalar", "simd128")):
        rows.append({"id": "%s-%s-%s" % (c, exp.split("_")[1], build), "export": exp, "build": build, "case": c, "reference_source": "native-scalar"})
json.dump({"rows": rows}, open(sys.argv[1], "w"))
PY
bundled_check nine "${fix}/nine.json"
# 部分 Top-k の行（TASK-996）を足した形の合成の入力でも、この回帰テストを直さずに通る
python3 -I - "${fix}/topk-rows.json" <<'PY'
import json, sys
rows = [{"id": "keep-fixed", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]},
        {"id": "keep-gemm", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"}]
for c in ("topk-a", "topk-b"):
    for exp, build in (("topk_scalar", "scalar"), ("topk_simd", "scalar"), ("topk_simd", "simd128")):
        rows.append({"id": "%s-%s-%s" % (c, exp.split("_")[1], build), "export": exp, "build": build, "case": c, "reference_source": "native-scalar"})
json.dump({"rows": rows}, open(sys.argv[1], "w"))
PY
bundled_check topk-rows "${fix}/topk-rows.json"
# 要素ごと演算の行（TASK-997）を足した形の合成の入力でも、この回帰テストを直さずに通る
python3 -I - "${fix}/elementwise-rows.json" <<'PY'
import json, sys
rows = [{"id": "keep-fixed", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]},
        {"id": "keep-gemm", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"}]
for c in ("elementwise-relu", "elementwise-add-scalar"):
    for exp, build in (("elementwise_scalar", "scalar"), ("elementwise_simd", "scalar"), ("elementwise_scalar", "simd128"), ("elementwise_simd", "simd128")):
        rows.append({"id": "%s-%s-%s" % (c, exp.split("_")[1], build), "export": exp, "build": build, "case": c, "reference_source": "native-scalar"})
json.dump({"rows": rows}, open(sys.argv[1], "w"))
PY
bundled_check elementwise-rows "${fix}/elementwise-rows.json"

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
  if (m !== 2 || k !== 3 || n !== 2) return 9;
  out[0] = a[0];
  out[1] = a[1];
  return 0;
};
exports.gemm_simd = (a, b, m, k, n) => (m === 2 && k === 3 && n === 2 ? 7 : 0);
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

# 23c. topk の呼び方（TASK-996）: scores は Float32Array でビット列を保つ・out は k×8 バイトの Uint8Array で全語が番兵・
# 戻り 0 のときだけ .bin を書く・書かなかった語は番兵のまま
mk_host_fixture "${work}/glue/tin" topk-a
cat >"${work}/glue/topk-check.js" <<'JS'
// 渡された引数を照らし、食い違えば 0 以外を返す。
function check(scores, k, out) {
  if (k !== 2 || scores.length !== 6) return 9;
  if (!(scores instanceof Float32Array)) return 8;
  const u = new Uint32Array(scores.buffer, scores.byteOffset, 6);
  if (u[1] !== 0x7fc00001 || u[2] !== 0x7fc00002 || u[3] !== 0x7f800000 || u[4] !== 0xff800000) return 7;
  if (!(out instanceof Uint8Array) || out.length !== k * 8) return 6;
  const dv = new DataView(out.buffer, out.byteOffset, out.length);
  for (let i = 0; i < k * 2; i += 1) {
    if (dv.getUint32(i * 4, true) !== 0x7fc0dead) return 5;
  }
  return 0;
}
exports.check = check;
JS
cat >"${work}/glue/topk.js" <<'JS'
const { check } = require("./topk-check.js");
exports.topk_scalar = (scores, k, out) => {
  const bad = check(scores, k, out);
  if (bad !== 0) return bad;
  for (let i = 0; i < 8; i += 1) out[i] = i + 1;
  return 0;
};
exports.topk_simd = (scores, k, out) => (check(scores, k, out) === 0 ? 3 : 0);
JS
cat >"${work}/glue/topkthrow.js" <<'JS'
exports.topk_simd = () => { throw new Error("boom"); };
JS
cat >"${work}/glue/topk-cases.json" <<'JSON'
{"rows": [
  {"id": "t1", "export": "topk_scalar", "build": "scalar", "case": "topk-a", "reference_source": "native-scalar"},
  {"id": "t2", "export": "topk_simd", "build": "scalar", "case": "topk-a", "reference_source": "native-scalar"},
  {"id": "t3", "export": "topk_simd", "build": "simd128", "case": "topk-a", "reference_source": "native-scalar"},
  {"id": "t4", "export": "topk_scalar", "build": "scalar", "case": "no-such-case", "reference_source": "native-scalar"}
]}
JSON
rm -rf "${work}/glue/tout"
mkdir -p "${work}/glue/tout"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/topk.js" "${work}/glue/topk-cases.json" "${work}/glue/rt.jsonl" "${work}/glue/topkthrow.js" "${work}/glue/tin" "${work}/glue/tout" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue topk rows" 0
check "23 topk: a zero return writes the out bytes as they are, the unwritten words keep the fill word" "$(python3 -I -c 'import sys
data = open(sys.argv[1], "rb").read()
sys.exit(0 if data == bytes(range(1, 9)) + bytes([0xad, 0xde, 0xc0, 0x7f]) * 2 else 1)' "${work}/glue/tout/t1.bin" && echo 0 || echo 1)"
check "23 topk: nonzero return, throw and unknown case are error rows and write nothing" "$(python3 -I -c 'import json,os,sys
rows = {json.loads(l)["id"]: json.loads(l) for l in open(sys.argv[1])}
ok = rows["t1"] == {"id": "t1", "output_file": "t1.bin"} and rows["t2"] == {"id": "t2", "error": "nonzero return"} and rows["t3"] == {"id": "t3", "error": "call failed"} and rows["t4"] == {"id": "t4", "error": "call failed"}
ok = ok and sorted(os.listdir(sys.argv[2])) == ["t1.bin"]
sys.exit(0 if ok else 1)' "${work}/glue/rt.jsonl" "${work}/glue/tout" && echo 0 || echo 1)"

# 23d. elementwise の呼び方（TASK-997）: op_code と定数（0.5 ちょうど）・a は Float32Array で payload つきの NaN（語 2・語 4）も含めビット列を保つ・
# out は長さ n の Float32Array で全語が番兵・戻り 0 のときだけ .bin を書く・書かなかった語は番兵のまま
mk_host_fixture "${work}/glue/ein" elementwise-add-nanin-a
cat >"${work}/glue/ew-check.js" <<'JS'
// 渡された引数を照らし、食い違えば 0 以外を返す。
function check(opCode, c, a, out) {
  if (opCode !== 1 || c !== 0.5) return 9;
  if (!(a instanceof Float32Array) || a.length !== 5) return 8;
  const u = new Uint32Array(a.buffer, a.byteOffset, 5);
  if (u[0] !== 0x3f800000 || u[1] !== 0xbf800000 || u[3] !== 0xc0000000) return 7;
  if (u[2] !== 0x7fc00003 || u[4] !== 0x7f800002) return 4;
  if (!(out instanceof Float32Array) || out.length !== 5) return 6;
  const w = new Uint32Array(out.buffer, out.byteOffset, 5);
  for (let i = 0; i < 5; i += 1) {
    if (w[i] !== 0x7fc0dead) return 5;
  }
  return 0;
}
exports.check = check;
JS
cat >"${work}/glue/ew.js" <<'JS'
const { check } = require("./ew-check.js");
exports.elementwise_scalar = (opCode, c, a, out) => {
  const bad = check(opCode, c, a, out);
  if (bad !== 0) return bad;
  // 受け取った NaN の語（語 2・語 4）をそのまま out の語 0・語 1 へ書き出し、語 2 は固定値にする。
  const u = new Uint32Array(a.buffer, a.byteOffset, 5);
  const w = new Uint32Array(out.buffer, out.byteOffset, 5);
  w[0] = u[2];
  w[1] = u[4];
  w[2] = 3;
  return 0;
};
exports.elementwise_simd = (opCode, c, a, out) => (check(opCode, c, a, out) === 0 ? 3 : 0);
JS
cat >"${work}/glue/ewthrow.js" <<'JS'
exports.elementwise_simd = () => { throw new Error("boom"); };
JS
cat >"${work}/glue/ew-cases.json" <<'JSON'
{"rows": [
  {"id": "e1", "export": "elementwise_scalar", "build": "scalar", "case": "elementwise-add-nanin-a", "reference_source": "native-scalar"},
  {"id": "e2", "export": "elementwise_simd", "build": "scalar", "case": "elementwise-add-nanin-a", "reference_source": "native-scalar"},
  {"id": "e3", "export": "elementwise_simd", "build": "simd128", "case": "elementwise-add-nanin-a", "reference_source": "native-scalar"},
  {"id": "e4", "export": "elementwise_scalar", "build": "scalar", "case": "no-such-case", "reference_source": "native-scalar"}
]}
JSON
rm -rf "${work}/glue/eout"
mkdir -p "${work}/glue/eout"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/ew.js" "${work}/glue/ew-cases.json" "${work}/glue/re.jsonl" "${work}/glue/ewthrow.js" "${work}/glue/ein" "${work}/glue/eout" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "23 run-glue elementwise rows" 0
check "23 elementwise: a zero return writes the out bytes as they are, the unwritten words keep the fill word" "$(python3 -I -c 'import struct,sys
data = open(sys.argv[1], "rb").read()
sys.exit(0 if struct.unpack("<5I", data) == (0x7fc00003, 0x7f800002, 3, 0x7fc0dead, 0x7fc0dead) else 1)' "${work}/glue/eout/e1.bin" && echo 0 || echo 1)"
check "23 elementwise: nonzero return, throw and unknown case are error rows and write nothing" "$(python3 -I -c 'import json,os,sys
rows = {json.loads(l)["id"]: json.loads(l) for l in open(sys.argv[1])}
ok = rows["e1"] == {"id": "e1", "output_file": "e1.bin"} and rows["e2"] == {"id": "e2", "error": "nonzero return"} and rows["e3"] == {"id": "e3", "error": "call failed"} and rows["e4"] == {"id": "e4", "error": "call failed"}
ok = ok and sorted(os.listdir(sys.argv[2])) == ["e1.bin"]
sys.exit(0 if ok else 1)' "${work}/glue/re.jsonl" "${work}/glue/eout" && echo 0 || echo 1)"

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
elif mode == "first-word":
    w[0] ^= 1
elif mode == "last-word":
    w[-1] ^= 1
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
for mode in nan-payload neg-zero first-word last-word; do
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
elif mode == "kind-unknown":
    c["kind"] = "mystery"
elif mode == "kind-list":
    c["kind"] = ["gemm"]
elif mode == "kind-missing":
    del c["kind"]
elif mode == "dims":
    c["m"] = 0
elif mode == "extra":
    c["extra"] = 1
json.dump(m, open(mp, "w"))
PY
}
for mode in sha slash dotdot dot-first length kind kind-unknown kind-list kind-missing dims extra; do
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

# 52d. 空でない保存先と、保存先と記録先が同じディレクトリは保存の前に 2
newdir out52d
echo other >"${work}/out52d/unrelated.txt"
gen_setup
gw --cases "${fix}/gen.json" --output-dir "${work}/out52d"
expect "52 non-empty output dir is refused" 2 "output dir must be empty" "!summary:" "!row:"
check "52 non-empty output dir left as it was" "$([ "$(nfiles "${work}/out52d")" = 1 ] && echo 0 || echo 1)"
newdir out52e
gen_setup
gw --cases "${fix}/gen.json" --output-dir "${work}/out52e" --record-dir "${work}/out52e" --record-combination metal-mac-mini
expect "52 same output and record dir is refused" 2 "must differ" "!summary:" "!row:"
check "52 same dir left empty" "$([ "$(nfiles "${work}/out52e")" = 0 ] && echo 0 || echo 1)"
check "52 node body not started (non-empty / same dir)" "$(no_node_started)"

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
elif mode == "first-word" or mode == "last-word":
    b = flip("g-scalar.bin", 0 if mode == "first-word" else 15)
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

for mode in first-word last-word; do
  saved_copy out57b
  saved_mut out57b "${mode}"
  w --recount "${work}/out57b"
  expect "57 one rewritten ${mode} with a fixed sha256" 1 "recount: g-scalar mismatches=1 n=4" "recount: g-simd mismatches=0 n=4" "recount-summary: rows=2 zero=1 nonzero=1 uncompared=0"
done

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
# 58b. 保存先の JSON が FIFO・シンボリックリンクなら待たずに 2（見張りの kill で待ちを失敗にする）
for jf in outputs.json manifest.json; do
  for kind in fifo symlink; do
    saved_copy out58
    rm -f "${work}/out58/${jf}"
    if [ "${kind}" = fifo ]; then mkfifo "${work}/out58/${jf}"; else ln -s "${work}/out56/${jf}" "${work}/out58/${jf}"; fi
    rc=0
    # GNU timeout は macOS に無いので、見張りの子プロセスと kill で待ちを 20 秒に絞る
    "${BASH}" "${WNC}" --recount "${work}/out58" >"${OUT_F}" 2>"${ERR_F}" </dev/null &
    cpid=$!
    (sleep 20; kill "${cpid}" 2>/dev/null) >/dev/null 2>&1 &
    wpid=$!
    wait "${cpid}" || rc=$?
    kill "${wpid}" 2>/dev/null || true
    wait "${wpid}" 2>/dev/null || true
    expect "58 ${jf} is a ${kind}" 2 "error:" "!recount-summary:"
  done
done

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
old = "if data[i:i + unit] != want[i:i + unit])"
assert s.count(old) == 1
open(sys.argv[2], "w", encoding="utf-8").write(s.replace(old, "if data[i:i + unit] != want[i:i + unit] and i < 0)"))
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

# ---- 部分 Top-k の種類（TASK-996 の 1/4） ----

cat >"${fix}/tk.json" <<'JSON'
{"rows": [
  {"id": "t-scalar", "export": "topk_scalar", "build": "scalar", "case": "topk-a", "reference_source": "native-scalar"},
  {"id": "t-simd", "export": "topk_simd", "build": "simd128", "case": "topk-b", "reference_source": "native-scalar"}
]}
JSON
TK_SAVED_NAMES="t-scalar.bin t-simd.bin topk-a.scores.bin topk-a.native-scalar.bin topk-b.scores.bin topk-b.native-scalar.bin manifest.json outputs.json"
# tk_setup: 2 ケースの偽の host 側の道具の出力（topk-a・topk-b）と、全行一致の結果を用意する。
tk_setup() {
  rm -rf "${fix}/hf" "${fix}/outs"
  mk_host_fixture "${fix}/hf" topk-a topk-b
  printf '%s\n' 't-scalar topk-a' 't-simd topk-b' >"${fix}/gen-rowmap"
  printf '%s\n' '{"id":"t-scalar","output_file":"t-scalar.bin"}' '{"id":"t-simd","output_file":"t-simd.bin"}' >"${FAKE_RESULTS}"
  export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
}
# tk_mut <モード> <出力のファイル>: topk-a の参照の写しを 1 件ごとに崩した出力を ${fix}/outs に書く（8 バイトが 1 件）。
tk_mut() {
  mkdir -p "${fix}/outs"
  python3 -I - "$1" "${fix}/hf/topk-a.native-scalar.bin" "${fix}/outs/$2" <<'PY'
import struct, sys
mode, src, dst = sys.argv[1:4]
w = list(struct.unpack("<4I", open(src, "rb").read()))
if mode == "slot":
    w[0] ^= 1
elif mode == "score":
    w[1] ^= 1
elif mode == "both":
    w[0] ^= 1
    w[1] ^= 1
elif mode == "last":
    w[2] ^= 1
elif mode == "neg-zero":
    w[3] = 0x80000000
elif mode == "short":
    w = w[:2]
elif mode == "odd":
    w = w[:3]
open(dst, "wb").write(b"".join(struct.pack("<I", x) for x in w))
PY
}

# 66. topk の行が全部一致: 0、count・sha256・nonfinite の行、並び
tk_setup
gw --cases "${fix}/tk.json"
expect "66 topk rows all match" 0 "row: t-scalar match" "row: t-simd match" "count: t-scalar mismatches=0 n=2" "count: t-simd mismatches=0 n=2" "sha256: t-scalar " "sha256: topk-a.scores " "sha256: topk-a.native-scalar " "sha256: topk-b.scores " "nonfinite: topk-a input=4 output=0" "nonfinite: topk-b input=4 output=0" "summary: rows=2 match=2 mismatch=0 not-run=0"
check "66 output order is rows, counts, case hashes, nonfinite, summary last" "$(python3 -I -c 'import sys
L = [l for l in open(sys.argv[1]).read().splitlines() if not l.startswith(("node:", "wasm-bindgen:"))]
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
ok = first("row: t-simd") < first("count: t-scalar") < first("sha256: t-simd") < first("sha256: topk-a.scores") < first("sha256: topk-b.native-scalar") < first("nonfinite: topk-a") < first("nonfinite: topk-b") and L[-1].startswith("summary:")
sys.exit(0 if ok else 1)' "${OUT_F}" && echo 0 || echo 1)"
check "66 the unit is 8 bytes: the sha256 line of a row is the hash of the output bytes" "$(python3 -I -c 'import hashlib,sys
h = hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest()
sys.exit(0 if ("sha256: t-scalar " + h) in open(sys.argv[2]).read() else 1)' "${fix}/hf/topk-a.native-scalar.bin" "${OUT_F}" && echo 0 || echo 1)"

# 67. 1 件ごとの比べ: スロット番号だけ・スコアだけ・両方・-0.0 と +0.0・最後の件だけは、どれも 1 件の不一致
for mode in slot score both neg-zero last; do
  tk_setup
  tk_mut "${mode}" t-scalar.bin
  export FAKE_OUTPUTS="${fix}/outs"
  gw --cases "${fix}/tk.json"
  expect "67 one entry differs (${mode})" 1 "row: t-scalar mismatch (mismatches 1/2)" "count: t-scalar mismatches=1 n=2" "row: t-simd match" "match=1 mismatch=1"
  rm -rf "${fix}/outs"
done

# 68. 件数の違い・単位の倍数でない出力・呼び出しの失敗
tk_setup
tk_mut short t-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/tk.json"
expect "68 length differs" 1 "row: t-scalar mismatch (length expected 2 got 1)" "!count: t-scalar" "!sha256: t-scalar" "count: t-simd mismatches=0 n=2"
rm -rf "${fix}/outs"
tk_setup
tk_mut odd t-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/tk.json"
expect "68 a 12-byte output is not a multiple of 8" 2 "not a multiple of 8" "!summary:"
rm -rf "${fix}/outs"
tk_setup
printf '%s\n' '{"id":"t-scalar","error":"nonzero return"}' '{"id":"t-simd","error":"call failed"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/tk.json"
expect "68 call errors" 1 "row: t-scalar mismatch (call error)" "row: t-simd mismatch (call error)" "!count:" "mismatch=2"

# 69. native-neon の topk の行は not-run (reference-unavailable)。比べず count・nonfinite を出さない
tk_setup
cat >"${fix}/tk-neon.json" <<'JSON'
{"rows": [
  {"id": "t-scalar", "export": "topk_scalar", "build": "scalar", "case": "topk-a", "reference_source": "native-neon"}
]}
JSON
printf '%s\n' '{"id":"t-scalar","output_file":"t-scalar.bin"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/tk-neon.json"
expect "69 native-neon topk row" 1 "row: t-scalar not-run (reference-unavailable)" "!count:" "!sha256:" "!nonfinite:" "match=0 mismatch=0 not-run=1"

# 70. 参照のスコアに非有限があるケース: output の件数が出て、全行 match なら終了コードは変わらない
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" topk-badscore-a
printf '%s\n' 't-bad topk-badscore-a' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"t-bad","output_file":"t-bad.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
cat >"${fix}/tk-bad.json" <<'JSON'
{"rows": [
  {"id": "t-bad", "export": "topk_scalar", "build": "scalar", "case": "topk-badscore-a", "reference_source": "native-scalar"}
]}
JSON
gw --cases "${fix}/tk-bad.json"
expect "70 non-finite scores in the reference are counted at the score word" 0 "row: t-bad match" "nonfinite: topk-badscore-a input=4 output=1" "summary: rows=1 match=1"

# 71. export の種類とケースの種類の照らし（Node の本体を起動しない）
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" gemm-a topk-a
printf '%s\n' 'x gemm-a' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"x","output_file":"x.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
printf '{"rows": [%s]}\n' '{"id":"x","export":"topk_scalar","build":"scalar","case":"gemm-a","reference_source":"native-scalar"}' >"${fix}/kind-bad.json"
gw --cases "${fix}/kind-bad.json"
expect "71 a topk export on a gemm case" 2 "does not match the case kind" "!summary:"
check "71 node body not started (topk export on gemm case)" "$(no_node_started)"
printf '{"rows": [%s]}\n' '{"id":"x","export":"gemm_simd","build":"scalar","case":"topk-a","reference_source":"native-scalar"}' >"${fix}/kind-bad.json"
gw --cases "${fix}/kind-bad.json"
expect "71 a gemm export on a topk case" 2 "does not match the case kind" "!summary:"
check "71 node body not started (gemm export on topk case)" "$(no_node_started)"

# 72. topk の manifest の不正（どれも 2。Node の本体を起動しない）
tk_tamper() {
  python3 -I - "$1" "${fix}/hf" <<'PY'
import hashlib, json, os, sys
mode, d = sys.argv[1:3]
mp = os.path.join(d, "manifest.json")
m = json.load(open(mp))
c = m["cases"][0]
def rewrite(fk, hk, data):
    open(os.path.join(d, c[fk]), "wb").write(data)
    c[hk] = hashlib.sha256(data).hexdigest()
if mode == "extra":
    c["m"] = 1
elif mode == "no-k":
    del c["k"]
elif mode == "n-zero":
    c["n"] = 0
elif mode == "k-zero":
    c["k"] = 0
elif mode == "k-bool":
    c["k"] = True
elif mode == "n-str":
    c["n"] = "6"
elif mode == "scores-length":
    rewrite("scores_file", "scores_sha256", open(os.path.join(d, c["scores_file"]), "rb").read()[:-4])
elif mode == "native-length":
    rewrite("native_scalar_file", "native_scalar_sha256", open(os.path.join(d, c["native_scalar_file"]), "rb").read()[:-4])
elif mode == "scores-sha":
    with open(os.path.join(d, c["scores_file"]), "r+b") as f:
        f.write(b"\xff")
elif mode == "native-sha":
    with open(os.path.join(d, c["native_scalar_file"]), "r+b") as f:
        f.write(b"\xff")
elif mode == "slash":
    c["scores_file"] = "../x"
elif mode == "dotdot":
    os.rename(os.path.join(d, c["native_scalar_file"]), os.path.join(d, "n..bin"))
    c["native_scalar_file"] = "n..bin"
elif mode == "gemm-keys":
    c["kind"] = "gemm"
json.dump(m, open(mp, "w"))
PY
}
for mode in extra no-k n-zero k-zero k-bool n-str scores-length native-length scores-sha native-sha slash dotdot gemm-keys; do
  tk_setup
  tk_tamper "${mode}"
  gw --cases "${fix}/tk.json"
  expect "72 topk manifest tampered (${mode})" 2 "error:" "!summary:" "!${work}"
  check "72 node body not started (${mode})" "$(no_node_started)"
done

# 73. gemm と topk のケースが混ざった入力: --output-dir の名前の集合と中身
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" gemm-a topk-a
printf '%s\n' 'g-scalar gemm-a' 't-scalar topk-a' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"g-scalar","output_file":"g-scalar.bin"}' '{"id":"t-scalar","output_file":"t-scalar.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
cat >"${fix}/mixed.json" <<'JSON'
{"rows": [
  {"id": "g-scalar", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "t-scalar", "export": "topk_scalar", "build": "scalar", "case": "topk-a", "reference_source": "native-scalar"}
]}
JSON
newdir out73
gw --cases "${fix}/mixed.json" --output-dir "${work}/out73"
expect "73 gemm and topk rows together" 0 "count: g-scalar mismatches=0 n=4" "count: t-scalar mismatches=0 n=2" "nonfinite: topk-a input=4 output=0" "!nonfinite: gemm-a" "summary: rows=2 match=2"
check "73 the saved form holds the files of both kinds" "$(dir_names_ok "${work}/out73" "g-scalar.bin t-scalar.bin gemm-a.a.bin gemm-a.b.bin gemm-a.native-scalar.bin topk-a.scores.bin topk-a.native-scalar.bin manifest.json outputs.json")"
check "73 the saved topk inputs and output are copied byte for byte" "$(python3 -I -c 'import os, sys
hf, o = sys.argv[1:3]
ok = all(open(os.path.join(hf, n), "rb").read() == open(os.path.join(o, n), "rb").read() for n in ("manifest.json", "topk-a.scores.bin", "topk-a.native-scalar.bin"))
ok = ok and open(os.path.join(hf, "topk-a.native-scalar.bin"), "rb").read() == open(os.path.join(o, "t-scalar.bin"), "rb").read()
sys.exit(0 if ok else 1)' "${fix}/hf" "${work}/out73" && echo 0 || echo 1)"
w --recount "${work}/out73"
expect "73 --recount of the mixed saved form" 0 "recount: g-scalar mismatches=0 n=4" "recount: t-scalar mismatches=0 n=2" "nonfinite: topk-a input=4 output=0" "recount-summary: rows=2 zero=2 nonzero=0 uncompared=0"

# 74. manifest に topk のケースがあり、行は gemm だけ: nonfinite の行は無く、保存の形に topk の入力が入り、--recount が 0
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" gemm-a gemm-b topk-a
printf '%s\n' 'g-scalar gemm-a' 'g-simd gemm-b' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"g-scalar","output_file":"g-scalar.bin"}' '{"id":"g-simd","output_file":"g-simd.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
newdir out74
gw --cases "${fix}/gen.json" --output-dir "${work}/out74"
expect "74 gemm rows with a topk case in the manifest" 0 "summary: rows=2 match=2" "!nonfinite:"
check "74 the saved form has the topk inputs" "$(dir_names_ok "${work}/out74" "${SAVED_NAMES} topk-a.scores.bin topk-a.native-scalar.bin")"
w --recount "${work}/out74"
expect "74 --recount" 0 "recount-summary: rows=2 zero=2 nonzero=0 uncompared=0" "!nonfinite:"

# 75. topk の保存の形の数え直し: 0・1・2
tk_setup
newdir out75
gw --cases "${fix}/tk.json" --output-dir "${work}/out75"
expect "75 saved topk rows" 0 "summary: rows=2 match=2"
check "75 the file names are exactly the saved form" "$(dir_names_ok "${work}/out75" "${TK_SAVED_NAMES}")"
w --recount "${work}/out75"
expect "75 --recount of the saved form" 0 "recount: t-scalar mismatches=0 n=2" "recount: t-simd mismatches=0 n=2" "sha256: topk-a.scores " "nonfinite: topk-a input=4 output=0" "nonfinite: topk-b input=4 output=0" "recount-summary: rows=2 zero=2 nonzero=0 uncompared=0"
check "75 --recount order is recount, sha256, nonfinite, recount-summary" "$(python3 -I -c 'import sys
L = open(sys.argv[1]).read().splitlines()
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
ok = first("recount: t-simd") < first("sha256: t-scalar") < first("sha256: topk-b.native-scalar") < first("nonfinite: topk-a") < first("nonfinite: topk-b") < first("recount-summary:") and L[-1].startswith("recount-summary:")
sys.exit(0 if ok else 1)' "${OUT_F}" && echo 0 || echo 1)"
# tk_saved_mut <ディレクトリ名> <モード>: topk の保存の形を壊す。
tk_saved_mut() {
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
if mode == "first-entry" or mode == "last-entry":
    b = flip("t-scalar.bin", 0 if mode == "first-entry" else 15)
    o["rows"][0]["output_sha256"] = hashlib.sha256(b).hexdigest()
    save()
elif mode == "odd":
    b = open(os.path.join(d, "t-scalar.bin"), "rb").read()[:12]
    open(os.path.join(d, "t-scalar.bin"), "wb").write(b)
    o["rows"][0]["output_sha256"] = hashlib.sha256(b).hexdigest()
    save()
elif mode == "out-sha":
    flip("t-scalar.bin")
elif mode == "scores-sha":
    flip("topk-a.scores.bin")
elif mode == "missing":
    os.unlink(os.path.join(d, "t-simd.bin"))
elif mode == "kind-row":
    o["rows"][0]["export"] = "gemm_scalar"
    save()
PY
}
for mode in first-entry last-entry; do
  rm -rf "${work:?}/out75m"
  cp -R "${work}/out75" "${work}/out75m"
  tk_saved_mut out75m "${mode}"
  w --recount "${work}/out75m"
  expect "75 --recount of a saved form with a changed entry (${mode})" 1 "recount: t-scalar mismatches=1 n=2" "recount: t-simd mismatches=0 n=2"
done
for mode in odd out-sha scores-sha missing kind-row; do
  rm -rf "${work:?}/out75m"
  cp -R "${work}/out75" "${work}/out75m"
  tk_saved_mut out75m "${mode}"
  w --recount "${work}/out75m"
  expect "75 --recount of a broken saved form (${mode})" 2 "error:" "!recount-summary:"
done

# 76. 実行の中の数え直しが判定と食い違う場面: 数え直しの側の nonfinite の呼び出しだけを変えた変異体
rm -rf "${work}/mut"
mkdir -p "${work}/mut/scripts" "${work}/mut/wasm/crates/glue"
cp "${DIR}/lib.sh" "${work}/mut/scripts/lib.sh"
cp "${GLUE_MANIFEST}" "${work}/mut/wasm/crates/glue/Cargo.toml"
python3 -I - "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" <<'PY'
import sys
s = open(sys.argv[1], encoding="utf-8").read()
old = "sha_lines += nonfinite_lines(manifest_cases, cases, used)"
assert s.count(old) == 1
open(sys.argv[2], "w", encoding="utf-8").write(s.replace(old, "sha_lines += nonfinite_lines(manifest_cases, cases, set())"))
PY
chmod +x "${work}/mut/scripts/wasm-node-check.sh"
check "76 the mutant differs from the original" "$(cmp -s "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" && echo 1 || echo 0)"
tk_setup
newdir out76
newdir rec76
WNC_ORIG="${WNC}"
WNC="${work}/mut/scripts/wasm-node-check.sh"
gw --cases "${fix}/tk.json" --output-dir "${work}/out76" --record-dir "${work}/rec76" --record-combination metal-mac-mini
WNC="${WNC_ORIG}"
expect "76 a disagreeing recount of the nonfinite lines stops the run" 2 "error: recount disagrees" "!row:" "!summary:" "!recount-summary:"
check "76 nothing written to either directory" "$([ "$(nfiles "${work}/out76")" = 0 ] && [ "$(nfiles "${work}/rec76")" = 0 ] && echo 0 || echo 1)"
rm -rf "${work}/mut"
reset_gen
res_ok

# 77. 実物の cases-reference-unavailable.json は 5 行とも not-run (reference-unavailable)（TASK-996・TASK-997・TASK-995）
python3 -I - "${CASES_UNAVAILABLE_REAL}" "${FAKE_RESULTS}" "${fix}/rowmap-77" <<'PY'
import json, sys
rows = json.load(open(sys.argv[1]))["rows"]
open(sys.argv[2], "w").write("".join(json.dumps({"id": r["id"], "output_file": r["id"] + ".bin"}) + "\n" for r in rows))
open(sys.argv[3], "w").write("".join(r["id"] + " " + r["case"] + "\n" for r in rows))
PY
mk_host_fixture "${fix}/hf-77" topk-batch topk-ties topk-nonfinite elementwise-relu elementwise-add-scalar
export FAKE_HOST_FIXTURE="${fix}/hf-77" FAKE_ROW_MAP="${fix}/rowmap-77"
w --wasm "${dummy_wasm}" --wasm-simd128 "${dummy_wasm}" --cases "${CASES_UNAVAILABLE_REAL}"
reset_gen
expect "77 reference-unavailable rows" 1 "row: topk-batch-native-neon not-run (reference-unavailable)" "row: topk-ties-native-neon not-run (reference-unavailable)" "row: topk-nonfinite-native-neon not-run (reference-unavailable)" "row: elementwise-relu-native-neon not-run (reference-unavailable)" "row: elementwise-add-scalar-native-neon not-run (reference-unavailable)" "summary: rows=5 match=0 mismatch=0 not-run=5" "!count:" "!t1:" "!recount:" "!recount-summary:" "!nonfinite:" "!input-special:" "!sha256:"
check "77 the file has five rows, all native-neon" "$(python3 -I -c 'import json,sys
rows = json.load(open(sys.argv[1]))["rows"]
sys.exit(0 if len(rows) == 5 and all(r.get("reference_source") == "native-neon" for r in rows) else 1)' "${CASES_UNAVAILABLE_REAL}" && echo 0 || echo 1)"
res_ok

# ---- 要素ごと演算の種類（TASK-997 の 1/4） ----

cat >"${fix}/ew.json" <<'JSON'
{"rows": [
  {"id": "e-scalar", "export": "elementwise_scalar", "build": "scalar", "case": "elementwise-a", "reference_source": "native-scalar"},
  {"id": "e-simd", "export": "elementwise_simd", "build": "simd128", "case": "elementwise-add-b", "reference_source": "native-scalar"}
]}
JSON
EW_SAVED_NAMES="e-scalar.bin e-simd.bin elementwise-a.input.bin elementwise-a.native-scalar.bin elementwise-add-b.input.bin elementwise-add-b.native-scalar.bin manifest.json outputs.json"
# ew_setup: 2 ケースの偽の host 側の道具の出力と、全行一致の結果を用意する。
ew_setup() {
  rm -rf "${fix}/hf" "${fix}/outs"
  mk_host_fixture "${fix}/hf" elementwise-a elementwise-add-b
  printf '%s\n' 'e-scalar elementwise-a' 'e-simd elementwise-add-b' >"${fix}/gen-rowmap"
  printf '%s\n' '{"id":"e-scalar","output_file":"e-scalar.bin"}' '{"id":"e-simd","output_file":"e-simd.bin"}' >"${FAKE_RESULTS}"
  export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
}
# ew_mut <モード> <出力のファイル>: elementwise-a の参照の写しを 1 語ごとに崩した出力を ${fix}/outs に書く（4 バイトが 1 語）。
ew_mut() {
  mkdir -p "${fix}/outs"
  python3 -I - "$1" "${fix}/hf/elementwise-a.native-scalar.bin" "${fix}/outs/$2" <<'PY'
import struct, sys
mode, src, dst = sys.argv[1:4]
w = list(struct.unpack("<5I", open(src, "rb").read()))
extra = b""
if mode == "first":
    w[0] ^= 1
elif mode == "last":
    w[4] ^= 1
elif mode == "neg-zero":
    w[1] = 0x80000000
elif mode == "payload":
    w[2] ^= 1
elif mode == "short":
    w = w[:3]
elif mode == "odd":
    extra = b"\x00"
open(dst, "wb").write(b"".join(struct.pack("<I", x) for x in w) + extra)
PY
}

# 78. elementwise の行が全部一致: 0、count・sha256・input-special の行、並び
ew_setup
gw --cases "${fix}/ew.json"
expect "78 elementwise rows all match" 0 "row: e-scalar match" "row: e-simd match" "count: e-scalar mismatches=0 n=5" "count: e-simd mismatches=0 n=5" "sha256: e-scalar " "sha256: elementwise-a.input " "sha256: elementwise-a.native-scalar " "sha256: elementwise-add-b.input " "input-special: elementwise-a nan=0 inf=0 zero=0" "input-special: elementwise-add-b nan=0 inf=0 zero=0" "!nonfinite:" "summary: rows=2 match=2 mismatch=0 not-run=0"
check "78 output order is rows, counts, case hashes, input-special, summary last" "$(python3 -I -c 'import sys
L = [l for l in open(sys.argv[1]).read().splitlines() if not l.startswith(("node:", "wasm-bindgen:"))]
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
ok = first("row: e-simd") < first("count: e-scalar") < first("sha256: e-simd") < first("sha256: elementwise-a.input") < first("sha256: elementwise-add-b.native-scalar") < first("input-special: elementwise-a") < first("input-special: elementwise-add-b") and L[-1].startswith("summary:")
sys.exit(0 if ok else 1)' "${OUT_F}" && echo 0 || echo 1)"
check "78 the unit is 4 bytes: the sha256 line of a row is the hash of the output bytes" "$(python3 -I -c 'import hashlib,sys
h = hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest()
sys.exit(0 if ("sha256: e-scalar " + h) in open(sys.argv[2]).read() else 1)' "${fix}/hf/elementwise-a.native-scalar.bin" "${OUT_F}" && echo 0 || echo 1)"

# 79. 1 語ごとの比べ: 最初の語・最後の語・-0.0 と +0.0・NaN の payload だけの違いは、どれも 1 語の不一致
for mode in first last neg-zero payload; do
  ew_setup
  ew_mut "${mode}" e-scalar.bin
  export FAKE_OUTPUTS="${fix}/outs"
  gw --cases "${fix}/ew.json"
  expect "79 one word differs (${mode})" 1 "row: e-scalar mismatch (mismatches 1/5)" "count: e-scalar mismatches=1 n=5" "row: e-simd match" "match=1 mismatch=1"
  rm -rf "${fix}/outs"
done

# 80. 語数の違い・4 の倍数でない出力・呼び出しの失敗
ew_setup
ew_mut short e-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/ew.json"
expect "80 length differs" 1 "row: e-scalar mismatch (length expected 5 got 3)" "!count: e-scalar" "!sha256: e-scalar" "count: e-simd mismatches=0 n=5"
rm -rf "${fix}/outs"
ew_setup
ew_mut odd e-scalar.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/ew.json"
expect "80 a 21-byte output is not a multiple of 4" 2 "not a multiple of 4" "!summary:"
rm -rf "${fix}/outs"
ew_setup
printf '%s\n' '{"id":"e-scalar","error":"nonzero return"}' '{"id":"e-simd","error":"call failed"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/ew.json"
expect "80 call errors" 1 "row: e-scalar mismatch (call error)" "row: e-simd mismatch (call error)" "!count:" "mismatch=2"

# 81. native-neon の elementwise の行は not-run (reference-unavailable)。比べず count・input-special を出さない
ew_setup
cat >"${fix}/ew-neon.json" <<'JSON'
{"rows": [
  {"id": "e-scalar", "export": "elementwise_simd", "build": "scalar", "case": "elementwise-a", "reference_source": "native-neon"}
]}
JSON
printf '%s\n' '{"id":"e-scalar","output_file":"e-scalar.bin"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/ew-neon.json"
expect "81 native-neon elementwise row" 1 "row: e-scalar not-run (reference-unavailable)" "!count:" "!sha256:" "!input-special:" "match=0 mismatch=0 not-run=1"

# 82. 入力に特殊値がある合成のケース: 件数が出て、全行 match なら終了コードは変わらない
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" elementwise-special-a
printf '%s\n' 'e-sp elementwise-special-a' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"e-sp","output_file":"e-sp.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
cat >"${fix}/ew-sp.json" <<'JSON'
{"rows": [
  {"id": "e-sp", "export": "elementwise_scalar", "build": "scalar", "case": "elementwise-special-a", "reference_source": "native-scalar"}
]}
JSON
gw --cases "${fix}/ew-sp.json"
expect "82 special values in the input are counted by word class" 0 "row: e-sp match" "input-special: elementwise-special-a nan=1 inf=2 zero=3" "summary: rows=1 match=1"
rm -rf "${fix}/hf"
mk_host_fixture "${fix}/hf" elementwise-nanbits-a
printf '%s\n' 'e-sp elementwise-nanbits-a' >"${fix}/gen-rowmap"
printf '{"rows": [%s]}\n' '{"id":"e-sp","export":"elementwise_scalar","build":"scalar","case":"elementwise-nanbits-a","reference_source":"native-scalar"}' >"${fix}/ew-nb.json"
gw --cases "${fix}/ew-nb.json"
expect "82 two NaN bit patterns count as two NaN words" 0 "input-special: elementwise-nanbits-a nan=2 inf=0 zero=0"

# 83. export の種類とケースの種類の照らし（Node の本体を起動しない）
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" gemm-a topk-a elementwise-a
printf '%s\n' 'x gemm-a' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"x","output_file":"x.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
n83=0
for pair in "elementwise_scalar gemm-a" "elementwise_simd topk-a" "gemm_simd elementwise-a" "topk_scalar elementwise-a"; do
  n83=$((n83 + 1))
  printf '{"rows": [{"id":"x","export":"%s","build":"scalar","case":"%s","reference_source":"native-scalar"}]}\n' "${pair%% *}" "${pair##* }" >"${fix}/kind-bad.json"
  gw --cases "${fix}/kind-bad.json"
  expect "83 export and case kind differ (${n83})" 2 "does not match the case kind" "!summary:"
  check "83 node body not started (${n83})" "$(no_node_started)"
done

# 84. elementwise の manifest の不正（どれも 2。Node の本体を起動しない・一時のパスを出さない）
ew_tamper() {
  python3 -I - "$1" "${fix}/hf" <<'PY'
import hashlib, json, os, sys
mode, d = sys.argv[1:3]
mp = os.path.join(d, "manifest.json")
m = json.load(open(mp))
c = m["cases"][0]
def rewrite(fk, hk, data):
    open(os.path.join(d, c[fk]), "wb").write(data)
    c[hk] = hashlib.sha256(data).hexdigest()
if mode == "extra":
    c["k"] = 1
elif mode == "no-op":
    del c["op_code"]
elif mode == "no-c":
    del c["c_bits"]
elif mode == "n-zero":
    c["n"] = 0
elif mode == "n-bool":
    c["n"] = True
elif mode == "op-2":
    c["op_code"] = 2
elif mode == "op-neg":
    c["op_code"] = -1
elif mode == "op-bool":
    c["op_code"] = True
elif mode == "op-str":
    c["op_code"] = "1"
elif mode == "c-short":
    c["c_bits"] = "3f00000"
elif mode == "c-upper":
    c["c_bits"] = "3F000000"
elif mode == "c-num":
    c["c_bits"] = 1056964608
elif mode == "input-length":
    rewrite("input_file", "input_sha256", open(os.path.join(d, c["input_file"]), "rb").read()[:-4])
elif mode == "native-length":
    rewrite("native_scalar_file", "native_scalar_sha256", open(os.path.join(d, c["native_scalar_file"]), "rb").read()[:-4])
elif mode == "input-sha":
    with open(os.path.join(d, c["input_file"]), "r+b") as f:
        f.write(b"\xff")
elif mode == "native-sha":
    with open(os.path.join(d, c["native_scalar_file"]), "r+b") as f:
        f.write(b"\xff")
elif mode == "slash":
    c["input_file"] = "../x"
elif mode == "dotdot":
    os.rename(os.path.join(d, c["native_scalar_file"]), os.path.join(d, "n..bin"))
    c["native_scalar_file"] = "n..bin"
elif mode == "gemm-keys":
    c["kind"] = "gemm"
json.dump(m, open(mp, "w"))
PY
}
cat >"${fix}/ew1.json" <<'JSON'
{"rows": [
  {"id": "e-scalar", "export": "elementwise_scalar", "build": "scalar", "case": "elementwise-a", "reference_source": "native-scalar"}
]}
JSON
for mode in extra no-op no-c n-zero n-bool op-2 op-neg op-bool op-str c-short c-upper c-num input-length native-length input-sha native-sha slash dotdot gemm-keys; do
  rm -rf "${fix}/hf" "${fix}/outs"
  mk_host_fixture "${fix}/hf" elementwise-a
  printf '%s\n' 'e-scalar elementwise-a' >"${fix}/gen-rowmap"
  printf '%s\n' '{"id":"e-scalar","output_file":"e-scalar.bin"}' >"${FAKE_RESULTS}"
  export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
  ew_tamper "${mode}"
  gw --cases "${fix}/ew1.json"
  expect "84 elementwise manifest tampered (${mode})" 2 "error:" "!summary:" "!${work}"
  check "84 node body not started (${mode})" "$(no_node_started)"
done

# 85. gemm・topk・elementwise のケースが混ざった入力: --output-dir の名前の集合と中身、その --recount
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" gemm-a topk-a elementwise-a
printf '%s\n' 'g-scalar gemm-a' 't-scalar topk-a' 'e-scalar elementwise-a' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"g-scalar","output_file":"g-scalar.bin"}' '{"id":"t-scalar","output_file":"t-scalar.bin"}' '{"id":"e-scalar","output_file":"e-scalar.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
cat >"${fix}/mixed3.json" <<'JSON'
{"rows": [
  {"id": "g-scalar", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "t-scalar", "export": "topk_scalar", "build": "scalar", "case": "topk-a", "reference_source": "native-scalar"},
  {"id": "e-scalar", "export": "elementwise_scalar", "build": "scalar", "case": "elementwise-a", "reference_source": "native-scalar"}
]}
JSON
newdir out85
gw --cases "${fix}/mixed3.json" --output-dir "${work}/out85"
expect "85 gemm, topk and elementwise rows together" 0 "count: g-scalar mismatches=0 n=4" "count: t-scalar mismatches=0 n=2" "count: e-scalar mismatches=0 n=5" "nonfinite: topk-a input=4 output=0" "!nonfinite: elementwise-a" "input-special: elementwise-a nan=0 inf=0 zero=0" "!input-special: gemm-a" "!input-special: topk-a" "summary: rows=3 match=3"
check "85 the saved form holds the files of the three kinds" "$(dir_names_ok "${work}/out85" "g-scalar.bin t-scalar.bin e-scalar.bin gemm-a.a.bin gemm-a.b.bin gemm-a.native-scalar.bin topk-a.scores.bin topk-a.native-scalar.bin elementwise-a.input.bin elementwise-a.native-scalar.bin manifest.json outputs.json")"
check "85 the saved elementwise input and output are copied byte for byte" "$(python3 -I -c 'import os, sys
hf, o = sys.argv[1:3]
ok = all(open(os.path.join(hf, n), "rb").read() == open(os.path.join(o, n), "rb").read() for n in ("manifest.json", "elementwise-a.input.bin", "elementwise-a.native-scalar.bin"))
ok = ok and open(os.path.join(hf, "elementwise-a.native-scalar.bin"), "rb").read() == open(os.path.join(o, "e-scalar.bin"), "rb").read()
sys.exit(0 if ok else 1)' "${fix}/hf" "${work}/out85" && echo 0 || echo 1)"
w --recount "${work}/out85"
expect "85 --recount of the mixed saved form" 0 "recount: e-scalar mismatches=0 n=5" "input-special: elementwise-a nan=0 inf=0 zero=0" "recount-summary: rows=3 zero=3 nonzero=0 uncompared=0"

# 86. manifest に elementwise のケースがあり、行は gemm だけ: input-special の行は無く、保存の形に入力が入り、--recount が 0
rm -rf "${fix}/hf" "${fix}/outs"
mk_host_fixture "${fix}/hf" gemm-a gemm-b elementwise-a
printf '%s\n' 'g-scalar gemm-a' 'g-simd gemm-b' >"${fix}/gen-rowmap"
printf '%s\n' '{"id":"g-scalar","output_file":"g-scalar.bin"}' '{"id":"g-simd","output_file":"g-simd.bin"}' >"${FAKE_RESULTS}"
export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
newdir out86
gw --cases "${fix}/gen.json" --output-dir "${work}/out86"
expect "86 gemm rows with an elementwise case in the manifest" 0 "summary: rows=2 match=2" "!input-special:"
check "86 the saved form has the elementwise inputs" "$(dir_names_ok "${work}/out86" "${SAVED_NAMES} elementwise-a.input.bin elementwise-a.native-scalar.bin")"
w --recount "${work}/out86"
expect "86 --recount" 0 "recount-summary: rows=2 zero=2 nonzero=0 uncompared=0" "!input-special:"

# 87. elementwise の保存の形の数え直し: 0・1・2
ew_setup
newdir out87
gw --cases "${fix}/ew.json" --output-dir "${work}/out87"
expect "87 saved elementwise rows" 0 "summary: rows=2 match=2"
check "87 the file names are exactly the saved form" "$(dir_names_ok "${work}/out87" "${EW_SAVED_NAMES}")"
w --recount "${work}/out87"
expect "87 --recount of the saved form" 0 "recount: e-scalar mismatches=0 n=5" "recount: e-simd mismatches=0 n=5" "sha256: elementwise-a.input " "input-special: elementwise-a nan=0 inf=0 zero=0" "input-special: elementwise-add-b nan=0 inf=0 zero=0" "recount-summary: rows=2 zero=2 nonzero=0 uncompared=0"
check "87 --recount order is recount, sha256, input-special, recount-summary" "$(python3 -I -c 'import sys
L = open(sys.argv[1]).read().splitlines()
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
ok = first("recount: e-simd") < first("sha256: e-scalar") < first("sha256: elementwise-add-b.native-scalar") < first("input-special: elementwise-a") < first("input-special: elementwise-add-b") < first("recount-summary:") and L[-1].startswith("recount-summary:")
sys.exit(0 if ok else 1)' "${OUT_F}" && echo 0 || echo 1)"
# ew_saved_mut <ディレクトリ名> <モード>: elementwise の保存の形を壊す。
ew_saved_mut() {
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
if mode == "first-word" or mode == "last-word":
    b = flip("e-scalar.bin", 0 if mode == "first-word" else 16)
    o["rows"][0]["output_sha256"] = hashlib.sha256(b).hexdigest()
    save()
elif mode == "odd":
    b = open(os.path.join(d, "e-scalar.bin"), "rb").read()[:18]
    open(os.path.join(d, "e-scalar.bin"), "wb").write(b)
    o["rows"][0]["output_sha256"] = hashlib.sha256(b).hexdigest()
    save()
elif mode == "out-sha":
    flip("e-scalar.bin")
elif mode == "input-sha":
    flip("elementwise-a.input.bin")
elif mode == "missing":
    os.unlink(os.path.join(d, "e-simd.bin"))
elif mode == "kind-row":
    o["rows"][0]["export"] = "gemm_scalar"
    save()
PY
}
for mode in first-word last-word; do
  rm -rf "${work:?}/out87m"
  cp -R "${work}/out87" "${work}/out87m"
  ew_saved_mut out87m "${mode}"
  w --recount "${work}/out87m"
  expect "87 --recount of a saved form with a changed word (${mode})" 1 "recount: e-scalar mismatches=1 n=5" "recount: e-simd mismatches=0 n=5"
done
for mode in odd out-sha input-sha missing kind-row; do
  rm -rf "${work:?}/out87m"
  cp -R "${work}/out87" "${work}/out87m"
  ew_saved_mut out87m "${mode}"
  w --recount "${work}/out87m"
  expect "87 --recount of a broken saved form (${mode})" 2 "error:" "!recount-summary:"
done

# 88. 実行の中の数え直しが判定と食い違う場面: 数え直しの側の input-special の呼び出しだけを変えた変異体
rm -rf "${work}/mut"
mkdir -p "${work}/mut/scripts" "${work}/mut/wasm/crates/glue"
cp "${DIR}/lib.sh" "${work}/mut/scripts/lib.sh"
cp "${GLUE_MANIFEST}" "${work}/mut/wasm/crates/glue/Cargo.toml"
python3 -I - "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" <<'PY'
import sys
s = open(sys.argv[1], encoding="utf-8").read()
old = "sha_lines += input_special_lines(manifest_cases, cases, used)"
assert s.count(old) == 1
open(sys.argv[2], "w", encoding="utf-8").write(s.replace(old, "sha_lines += input_special_lines(manifest_cases, cases, set())"))
PY
chmod +x "${work}/mut/scripts/wasm-node-check.sh"
check "88 the mutant differs from the original" "$(cmp -s "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" && echo 1 || echo 0)"
ew_setup
newdir out88
newdir rec88
WNC_ORIG="${WNC}"
WNC="${work}/mut/scripts/wasm-node-check.sh"
gw --cases "${fix}/ew.json" --output-dir "${work}/out88" --record-dir "${work}/rec88" --record-combination metal-mac-mini
WNC="${WNC_ORIG}"
expect "88 a disagreeing recount of the input-special lines stops the run" 2 "error: recount disagrees" "!row:" "!summary:" "!recount-summary:"
check "88 nothing written to either directory" "$([ "$(nfiles "${work}/out88")" = 0 ] && [ "$(nfiles "${work}/rec88")" = 0 ] && echo 0 || echo 1)"
rm -rf "${work}/mut"
reset_gen
res_ok

# ---- 閾値の比べ（TASK-995 の 1/4） ----

cat >"${fix}/t1.json" <<'JSON'
{"rows": [
  {"id": "t-simd", "export": "gemm_simd", "build": "simd128", "case": "gemm-t1", "reference_source": "native-scalar"}
]}
JSON
cat >"${fix}/t3.json" <<'JSON'
{"rows": [
  {"id": "t-simd", "export": "gemm_simd", "build": "simd128", "case": "gemm-t1", "reference_source": "native-scalar"},
  {"id": "t-simd-scalar", "export": "gemm_simd", "build": "scalar", "case": "gemm-t1", "reference_source": "native-scalar"},
  {"id": "t-scalar-simd128", "export": "gemm_scalar", "build": "simd128", "case": "gemm-t1", "reference_source": "native-scalar"}
]}
JSON
T1_SAVED_NAMES="t-simd.bin gemm-t1.a.bin gemm-t1.b.bin gemm-t1.native-scalar.bin manifest.json outputs.json"
# t1_setup <行の入力>: gemm-t1 の偽の host 側の道具の出力と、全行一致の結果を用意する。
t1_setup() {
  rm -rf "${fix}/hf" "${fix}/outs"
  mk_host_fixture "${fix}/hf" gemm-t1
  python3 -I - "$1" "${fix}/gen-rowmap" "${FAKE_RESULTS}" <<'PY'
import json, sys
rows = json.load(open(sys.argv[1]))["rows"]
open(sys.argv[2], "w").write("".join(r["id"] + " " + r["case"] + "\n" for r in rows))
open(sys.argv[3], "w").write("".join(json.dumps({"id": r["id"], "output_file": r["id"] + ".bin"}) + "\n" for r in rows))
PY
  export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
}
# t1_out <出力のファイル> <添字=語（16 進）|short|odd>...: gemm-t1 の参照の写しの語を置き換えた出力を ${fix}/outs に書く。
t1_out() {
  mkdir -p "${fix}/outs"
  python3 -I - "${fix}/hf/gemm-t1.native-scalar.bin" "${fix}/outs/$1" "$@" <<'PY'
import struct, sys
w = list(struct.unpack("<8I", open(sys.argv[1], "rb").read()))
extra = b""
for a in sys.argv[4:]:
    if a == "short":
        w = w[:7]
    elif a == "odd":
        extra = b"\x00"
    else:
        i, v = a.split("=")
        w[int(i)] = int(v, 16)
open(sys.argv[2], "wb").write(b"".join(struct.pack("<I", x) for x in w) + extra)
PY
}
# t1_case <名前> <期待コード> <添字=語 を空白で区切った文字列> <期待の語>...: 1 行の入力で t-simd の語を置き換えて流す。
t1_case() {
  local name="$1" rcw="$2" subs="$3"
  shift 3
  t1_setup "${fix}/t1.json"
  # 語は空白区切りで渡す（意図した分割）。
  # shellcheck disable=SC2086
  t1_out t-simd.bin ${subs}
  export FAKE_OUTPUTS="${fix}/outs"
  gw --cases "${fix}/t1.json"
  expect "${name}" "${rcw}" "$@"
  rm -rf "${fix}/outs"
}

# 89. 全語が同じ: 0、count・sha256・t1 の行、並び
t1_setup "${fix}/t1.json"
gw --cases "${fix}/t1.json"
expect "89 identical words are within the threshold" 0 "row: t-simd match" "count: t-simd mismatches=0 n=8" "sha256: t-simd " "t1: t-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0" "summary: rows=1 match=1 mismatch=0 not-run=0"
check "89 output order is count, sha256, t1" "$(python3 -I -c 'import sys
L = open(sys.argv[1]).read().splitlines()
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
sys.exit(0 if first("count: t-simd") < first("sha256: t-simd ") < first("t1: t-simd") < first("summary:") else 1)' "${OUT_F}" && echo 0 || echo 1)"

# 90. 比べの範囲: 閾値で比べるのは gemm_simd・simd128 の行だけ（ほかの行はビットの完全一致のまま）
t1_setup "${fix}/t3.json"
for f in t-simd.bin t-simd-scalar.bin t-scalar-simd128.bin; do
  t1_out "${f}" 4=3f800001
done
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/t3.json"
expect "90 only the gemm_simd simd128 row is compared by the threshold" 1 "row: t-simd match" "row: t-simd-scalar mismatch (mismatches 1/8)" "row: t-scalar-simd128 mismatch (mismatches 1/8)" "count: t-simd mismatches=0 n=8" "count: t-simd-scalar mismatches=1 n=8" "t1: t-simd " "!t1: t-simd-scalar" "!t1: t-scalar-simd128" "match=1 mismatch=2 not-run=0"
rm -rf "${fix}/outs"

# 91. 2 つの最大値が別の語（絶対の最大と相対の最大）。不一致は 0 で終了コード 0
t1_case "91 the absolute and relative maxima are at different words" 0 "1=44800800 2=35c00000" "count: t-simd mismatches=0 n=8" "t1: t-simd max-abs=0.25 max-abs-at=1 max-rel=0.5 max-rel-at=2 abs-over=1 rel-over=1" "row: t-simd match"

# 91b. 最大が同値の語が 2 つあるときは最初の語の添字を出す（絶対: 語 0 と語 1 が誤差 0.5。相対: 語 1 と語 4 が 2^-11）
t1_case "91 equal absolute maxima report the first word" 0 "0=447a2000 1=44801000" "t1: t-simd max-abs=0.5 max-abs-at=0 " "abs-over=2 rel-over=0"
t1_case "91 equal relative maxima report the first word" 0 "1=44801000 4=3f801000" "t1: t-simd max-abs=0.5 max-abs-at=1 max-rel=0.00048828125 max-rel-at=1 "

# 92. 相対の境界（語 0・参照 1000.0）: 比がちょうど上限は不一致、下回れば一致
t1_case "92 relative error exactly at the limit (above)" 1 "0=447a4000" "row: t-simd mismatch (mismatches 1/8)" "mismatches=1 n=8" "abs-over=1 rel-over=1"
t1_case "92 relative error exactly at the limit (below)" 1 "0=4479c000" "row: t-simd mismatch (mismatches 1/8)" "abs-over=1 rel-over=1"
t1_case "92 relative error under the limit" 0 "0=447a2000" "mismatches=0 n=8" "abs-over=1 rel-over=0" "row: t-simd match"

# 93. 参照が零（語 3）: 絶対誤差だけで判定し、相対は作らない
t1_case "93 reference zero, the other sign of zero" 0 "3=80000000" "mismatches=0 n=8" "max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=-"
t1_case "93 reference zero, just under the absolute limit" 0 "3=3727c5ac" "mismatches=0 n=8" "abs-over=0 rel-over=0" "max-rel=0.0 max-rel-at=-"
t1_case "93 reference zero, just over the absolute limit" 1 "3=3727c5ad" "mismatches=1 n=8" "abs-over=1 rel-over=0" "max-rel-at=-"
t1_case "93 reference zero, negative just over the absolute limit" 1 "3=b727c5ad" "mismatches=1 n=8" "abs-over=1 rel-over=0"

# 94. 有限でない語: NaN は NaN と一致（payload・符号は見ない）、Inf は同じ符号の Inf とだけ一致。集計には入らない
for w in 7f800001 ffc00001 7fc00000; do
  t1_case "94 reference NaN matches a NaN (${w})" 0 "5=${w}" "mismatches=0 n=8" "max-abs=0.0 max-abs-at=-"
done
t1_case "94 reference NaN against a finite word" 1 "5=3f800000" "mismatches=1 n=8" "max-abs=0.0 max-abs-at=-"
for w in 7fc00000 7f800001 7f800000 ff800000; do
  t1_case "94 finite reference against a non-finite word (${w})" 1 "4=${w}" "mismatches=1 n=8" "max-abs=0.0 max-abs-at=-"
done
t1_case "94 reference +Inf against +Inf" 0 "6=7f800000" "mismatches=0 n=8"
for w in ff800000 7f7fffff 7fc00000; do
  t1_case "94 reference +Inf against another word (${w})" 1 "6=${w}" "mismatches=1 n=8" "max-abs=0.0 max-abs-at=-"
done
t1_case "94 reference -Inf against +Inf" 1 "7=7f800000" "mismatches=1 n=8" "max-abs=0.0 max-abs-at=-"

# 95. 語数の違い・4 の倍数でない出力・呼び出しの失敗
t1_case "95 length differs" 1 "short" "row: t-simd mismatch (length expected 8 got 7)" "!count: t-simd" "!t1:"
t1_case "95 a 33-byte output is not a multiple of 4" 2 "odd" "not a multiple of 4" "!summary:" "!t1:"
t1_setup "${fix}/t1.json"
printf '%s\n' '{"id":"t-simd","error":"nonzero return"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/t1.json"
expect "95 a call error" 1 "row: t-simd mismatch (call error)" "!count:" "!t1:"

# 96. native-neon の gemm_simd・simd128 の行は not-run (reference-unavailable)。比べず t1 を出さない
t1_setup "${fix}/t1.json"
cat >"${fix}/t1-neon.json" <<'JSON'
{"rows": [
  {"id": "t-simd", "export": "gemm_simd", "build": "simd128", "case": "gemm-t1", "reference_source": "native-neon"}
]}
JSON
gw --cases "${fix}/t1-neon.json"
expect "96 native-neon gemm_simd simd128 row" 1 "row: t-simd not-run (reference-unavailable)" "!count:" "!t1:" "match=0 mismatch=0 not-run=1"

# 97. --output-dir と --recount: 判定と同じ t1 の行が出る。保存の形の語を変えた場合と、行の build を変えた場合
t1_setup "${fix}/t1.json"
t1_out t-simd.bin 1=44800800 2=35c00000
export FAKE_OUTPUTS="${fix}/outs"
newdir out97
gw --cases "${fix}/t1.json" --output-dir "${work}/out97"
expect "97 saved form of a threshold row" 0 "t1: t-simd max-abs=0.25 max-abs-at=1 max-rel=0.5 max-rel-at=2 abs-over=1 rel-over=1" "summary: rows=1 match=1"
check "97 the file names are exactly the saved form" "$(dir_names_ok "${work}/out97" "${T1_SAVED_NAMES}")"
w --recount "${work}/out97"
expect "97 --recount prints the same t1 line" 0 "recount: t-simd mismatches=0 n=8" "t1: t-simd max-abs=0.25 max-abs-at=1 max-rel=0.5 max-rel-at=2 abs-over=1 rel-over=1" "recount-summary: rows=1 zero=1 nonzero=0 uncompared=0"
check "97 --recount order is recount, sha256, t1, recount-summary" "$(python3 -I -c 'import sys
L = open(sys.argv[1]).read().splitlines()
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
sys.exit(0 if first("recount: t-simd") < first("sha256: t-simd ") < first("t1: t-simd") < first("sha256: gemm-t1") and L[-1].startswith("recount-summary:") else 1)' "${OUT_F}" && echo 0 || echo 1)"
# t1_saved_mut <ディレクトリ名> <モード>: 閾値の行の保存の形を変える。
t1_saved_mut() {
  python3 -I - "${work}/$1" "$2" <<'PY'
import hashlib, json, os, sys
d, mode = sys.argv[1:3]
op = os.path.join(d, "outputs.json")
o = json.load(open(op))
if mode == "over":
    p = os.path.join(d, "t-simd.bin")
    b = bytearray(open(p, "rb").read())
    b[0:4] = (0x447a4000).to_bytes(4, "little")
    open(p, "wb").write(bytes(b))
    o["rows"][0]["output_sha256"] = hashlib.sha256(bytes(b)).hexdigest()
elif mode == "scalar":
    o["rows"][0]["build"] = "scalar"
json.dump(o, open(op, "w"))
PY
}
rm -rf "${work:?}/out97m"
cp -R "${work}/out97" "${work}/out97m"
t1_saved_mut out97m over
w --recount "${work}/out97m"
expect "97 --recount of a saved form over the threshold" 1 "recount: t-simd mismatches=1 n=8" "abs-over=2 rel-over=2"
rm -rf "${work:?}/out97m"
cp -R "${work}/out97" "${work}/out97m"
t1_saved_mut out97m scalar
w --recount "${work}/out97m"
expect "97 --recount of a row whose build is scalar compares the bits" 1 "recount: t-simd mismatches=2 n=8" "!t1:" "recount-summary: rows=1 zero=0 nonzero=1"

# 98. 実行の中の数え直しが判定と食い違う場面: 数え直しの側の t1_count の引数だけを入れ替えた変異体
rm -rf "${work}/mut"
mkdir -p "${work}/mut/scripts" "${work}/mut/wasm/crates/glue"
cp "${DIR}/lib.sh" "${work}/mut/scripts/lib.sh"
cp "${GLUE_MANIFEST}" "${work}/mut/wasm/crates/glue/Cargo.toml"
python3 -I - "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" <<'PY'
import sys
s = open(sys.argv[1], encoding="utf-8").read()
old = "diff, t1, max_abs = t1_count(want, data)"
assert s.count(old) == 1
open(sys.argv[2], "w", encoding="utf-8").write(s.replace(old, "diff, t1, max_abs = t1_count(data, want)"))
PY
chmod +x "${work}/mut/scripts/wasm-node-check.sh"
check "98 the mutant differs from the original" "$(cmp -s "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" && echo 1 || echo 0)"
t1_setup "${fix}/t1.json"
t1_out t-simd.bin 2=35c00000
export FAKE_OUTPUTS="${fix}/outs"
newdir out98
newdir rec98
WNC_ORIG="${WNC}"
WNC="${work}/mut/scripts/wasm-node-check.sh"
gw --cases "${fix}/t1.json" --output-dir "${work}/out98" --record-dir "${work}/rec98" --record-combination metal-mac-mini
WNC="${WNC_ORIG}"
expect "98 a disagreeing recount of the t1 lines stops the run" 2 "error: recount disagrees" "!row:" "!summary:" "!recount-summary:"
check "98 nothing written to either directory" "$([ "$(nfiles "${work}/out98")" = 0 ] && [ "$(nfiles "${work}/rec98")" = 0 ] && echo 0 || echo 1)"
rm -rf "${work}/mut" "${fix}/outs"
reset_gen
res_ok

# ---- 特殊値の入力の種類（TASK-999 の 1/6） ----

# sp_rows <出力のファイル> <行の接頭辞> <export の接頭辞> <ケース> [出どころ]: 4 組（scalar・simd128 × scalar・simd）の行の入力を書く。
sp_rows() {
  python3 -I - "$@" <<'PY'
import json, sys
dst, pre, base, case = sys.argv[1:5]
src = sys.argv[5] if len(sys.argv) > 5 else "native-scalar"
rows = []
for exp, build in (("scalar", "scalar"), ("simd", "scalar"), ("scalar", "simd128"), ("simd", "simd128")):
    rows.append({"id": "%s-%s-%s" % (pre, exp, build), "export": "%s_%s" % (base, exp), "build": build, "case": case, "reference_source": src})
json.dump({"rows": rows}, open(dst, "w"))
PY
}
# sp_prep <行の入力> <ケース名...>: 偽の host 側の道具の出力（ケース名の合成のケース）と、全行 output_file の結果を用意する。
sp_prep() {
  local rows="$1"
  shift
  rm -rf "${fix}/hf" "${fix}/outs"
  mk_host_fixture "${fix}/hf" "$@"
  python3 -I - "${rows}" "${fix}/gen-rowmap" "${FAKE_RESULTS}" <<'PY'
import json, sys
rows = json.load(open(sys.argv[1]))["rows"]
open(sys.argv[2], "w").write("".join(r["id"] + " " + r["case"] + "\n" for r in rows))
open(sys.argv[3], "w").write("".join(json.dumps({"id": r["id"], "output_file": r["id"] + ".bin"}) + "\n" for r in rows))
PY
  export FAKE_HOST_FIXTURE="${fix}/hf" FAKE_ROW_MAP="${fix}/gen-rowmap"
}
# sp_out <ケース> <出力のファイル> <添字=語（16 進）|short|odd>...: ケースの参照の写しの語を置き換えた出力を ${fix}/outs に書く。
sp_out() {
  mkdir -p "${fix}/outs"
  python3 -I - "${fix}/hf/$1.native-scalar.bin" "${fix}/outs/$2" "$@" <<'PY'
import struct, sys
raw = open(sys.argv[1], "rb").read()
w = list(struct.unpack("<%dI" % (len(raw) // 4), raw))
extra = b""
for a in sys.argv[5:]:
    if a == "short":
        w = w[:-1]
    elif a == "odd":
        extra = b"\x00"
    else:
        i, v = a.split("=")
        w[int(i)] = int(v, 16)
open(sys.argv[2], "wb").write(b"".join(struct.pack("<I", x) for x in w) + extra)
PY
}
# sp_out_all <ケース> <行の接頭辞> <添字=語...>: 4 行すべての出力に同じ置き換えを書く。
sp_out_all() {
  local c="$1" pre="$2" id
  shift 2
  rm -rf "${fix}/outs"
  for id in "${pre}-scalar-scalar" "${pre}-simd-scalar" "${pre}-scalar-simd128" "${pre}-simd-simd128"; do
    sp_out "${c}" "${id}.bin" "$@"
  done
  export FAKE_OUTPUTS="${fix}/outs"
}
# sp_tamper <モード> <ケース>: 偽の host 側の道具の manifest（${fix}/hf）を壊す。
sp_tamper() {
  python3 -I - "${fix}/hf" "$1" "$2" <<'PY'
import hashlib, json, os, sys
d, mode, case = sys.argv[1:4]
mp = os.path.join(d, "manifest.json")
m = json.load(open(mp))
c = next(x for x in m["cases"] if x["name"] == case)
cp = os.path.join(d, c["cpu_reference_file"])
if mode == "dropkey":
    del c["cpu_reference_file"]
elif mode == "dropsha":
    del c["cpu_reference_sha256"]
elif mode == "extrakey":
    c["extra"] = 1
elif mode == "badsha":
    c["cpu_reference_sha256"] = "0" * 64
elif mode == "shortfile":
    b = open(cp, "rb").read()[:-4]
    open(cp, "wb").write(b)
    c["cpu_reference_sha256"] = hashlib.sha256(b).hexdigest()
elif mode == "rmfile":
    os.remove(cp)
elif mode == "kind":
    c["kind"] = "gemm" if c["kind"] == "gemm-special" else "elementwise"
json.dump(m, open(mp, "w"))
PY
}
# sp_saved_mut <ディレクトリ名> <モード> <ケース>: 保存の形を変える。
sp_saved_mut() {
  python3 -I - "${work}/$1" "$2" "$3" <<'PY'
import hashlib, json, os, sys
d, mode, case = sys.argv[1:4]
op = os.path.join(d, "outputs.json")
o = json.load(open(op))
mp = os.path.join(d, "manifest.json")
m = json.load(open(mp))
c = next(x for x in m["cases"] if x["name"] == case)
cp = os.path.join(d, c["cpu_reference_file"])
if mode == "word":
    p = os.path.join(d, o["rows"][0]["output_file"])
    b = bytearray(open(p, "rb").read())
    b[16:20] = (0x40400000).to_bytes(4, "little")
    open(p, "wb").write(bytes(b))
    o["rows"][0]["output_sha256"] = hashlib.sha256(bytes(b)).hexdigest()
elif mode == "cpuonly":
    b = bytearray(open(cp, "rb").read())
    b[16:20] = (0x40400000).to_bytes(4, "little")
    open(cp, "wb").write(bytes(b))
    c["cpu_reference_sha256"] = hashlib.sha256(bytes(b)).hexdigest()
elif mode == "badcpusha":
    c["cpu_reference_sha256"] = "0" * 64
elif mode == "rmcpu":
    os.remove(cp)
elif mode == "export":
    e = o["rows"][0]["export"]
    o["rows"][0]["export"] = ("elementwise_" if e.startswith("gemm_") else "gemm_") + e.split("_")[1]
json.dump(o, open(op, "w"))
json.dump(m, open(mp, "w"))
PY
}

# sp_kind <ラベル> <行の接頭辞> <export の接頭辞> <ケース> <cpudiff のケース> <既存の種類のケース> <既存の種類の語の置き換え> <保存の形の名前の並び>:
# 1 つの種類について、4 組すべての行で項目 99〜108 を流す。
sp_kind() {
  local L="$1" P="$2" B="$3" C="$4" CD="$5" OC="$6" OS="$7" SAVED="$8" subs mode
  local RJ="${fix}/sp-${P}.json"
  sp_rows "${RJ}" "${P}" "${B}" "${C}"

  # 99. 全行一致
  sp_prep "${RJ}" "${C}"
  gw --cases "${RJ}"
  expect "99 ${L} all rows match" 0 "row: ${P}-scalar-scalar match" "row: ${P}-simd-simd128 match" "count: ${P}-simd-scalar mismatches=0 n=8" "cpu-count: ${P}-scalar-scalar mismatches=0 n=8" "cpu-count: ${P}-simd-simd128 mismatches=0 n=8" "t1: ${P}-scalar-scalar max-abs=0.0" "t1: ${P}-simd-scalar max-abs=0.0" "t1: ${P}-scalar-simd128 max-abs=0.0" "t1: ${P}-simd-simd128 max-abs=0.0" "sha256: ${C}.cpu-reference " "recount: ${P}-simd-simd128 mismatches=0 n=8" "recount-cpu: ${P}-scalar-scalar mismatches=0 n=8" "recount-summary: rows=4 zero=4 nonzero=0 uncompared=0" "summary: rows=4 match=4 mismatch=0 not-run=0"
  check "99 ${L} output order" "$(python3 -I -c 'import sys
L = open(sys.argv[1]).read().splitlines()
p = sys.argv[2]
def first(prefix):
    return next(i for i, l in enumerate(L) if l.startswith(prefix))
a = p + "-scalar-scalar"
ok = first("count: " + a) < first("cpu-count: " + a) < first("sha256: " + a + " ") < first("t1: " + a) < first("summary:")
ok = ok and first("recount: " + a) < first("recount-cpu: " + a) < first("recount-summary:")
sys.exit(0 if ok else 1)' "${OUT_F}" "${P}" && echo 0 || echo 1)"
  # 99b. input-special の行は elementwise-special のケースにだけ出る
  case "${P}" in
    e) expect "99 ${L} input-special line" 0 "input-special: ${C} nan=1 inf=2 zero=3" ;;
    *) expect "99 ${L} no input-special line" 0 "!input-special:" ;;
  esac

  # 100. payload だけの NaN の違いと符号違いの零は一致。同じ種類の変更は既存の種類の行では不一致のまま
  for subs in "5=7fc00001" "5=ffc00001" "5=7f800001" "3=80000000"; do
    sp_prep "${RJ}" "${C}"
    # shellcheck disable=SC2086
    sp_out_all "${C}" "${P}" ${subs}
    gw --cases "${RJ}"
    expect "100 ${L} ${subs} is within the rule" 0 "row: ${P}-scalar-scalar match" "row: ${P}-simd-simd128 match" "count: ${P}-scalar-scalar mismatches=0 n=8" "cpu-count: ${P}-simd-scalar mismatches=0 n=8" "summary: rows=4 match=4"
  done
  sp_rows "${fix}/sp-old.json" old "${B}" "${OC}"
  sp_prep "${fix}/sp-old.json" "${OC}"
  # shellcheck disable=SC2086
  sp_out "${OC}" old-scalar-scalar.bin ${OS}
  export FAKE_OUTPUTS="${fix}/outs"
  gw --cases "${fix}/sp-old.json"
  expect "100 ${L} the same kind of change is a mismatch for the existing kind" 1 "row: old-scalar-scalar mismatch (mismatches 1/" "!cpu-count:"

  # 101. 語の違い（片方だけ NaN・Inf の符号違い・有限と NaN）
  for subs in "5=3f800000" "6=ff800000" "4=7fc00000" "7=7f800000"; do
    sp_prep "${RJ}" "${C}"
    # shellcheck disable=SC2086
    sp_out_all "${C}" "${P}" ${subs}
    gw --cases "${RJ}"
    expect "101 ${L} ${subs} is a mismatch" 1 "row: ${P}-scalar-scalar mismatch (mismatches 1/8)" "row: ${P}-simd-scalar mismatch (mismatches 1/8)" "row: ${P}-scalar-simd128 mismatch (mismatches 1/8)" "row: ${P}-simd-simd128 mismatch (mismatches 1/8)" "cpu-count: ${P}-scalar-scalar mismatches=1 n=8" "match=0 mismatch=4 not-run=0"
  done

  # 102. 閾値の内と外
  sp_prep "${RJ}" "${C}"
  sp_out_all "${C}" "${P}" 0=447a2000
  gw --cases "${RJ}"
  expect "102 ${L} inside the threshold" 0 "row: ${P}-simd-simd128 match" "summary: rows=4 match=4"
  sp_prep "${RJ}" "${C}"
  sp_out_all "${C}" "${P}" 0=447a4000
  gw --cases "${RJ}"
  expect "102 ${L} outside the threshold" 1 "row: ${P}-simd-simd128 mismatch (mismatches 1/8)" "row: ${P}-scalar-scalar mismatch (mismatches 1/8)"

  # 103. CPU 参照だけと違う: 終了コード 1、数え直しも 1、部品は ng で cpu-reference の語を含まない
  sp_rows "${fix}/sp-cd.json" "${P}" "${B}" "${CD}"
  sp_prep "${fix}/sp-cd.json" "${CD}"
  newdir out103
  newdir rec103
  gw --cases "${fix}/sp-cd.json" --output-dir "${work}/out103" --record-dir "${work}/rec103" --record-combination metal-mac-mini
  expect "103 ${L} output equal to native but not to the CPU reference" 1 "row: ${P}-scalar-scalar mismatch (cpu-reference mismatches 1/8)" "row: ${P}-simd-simd128 mismatch (cpu-reference mismatches 1/8)" "count: ${P}-scalar-scalar mismatches=0 n=8" "cpu-count: ${P}-scalar-scalar mismatches=1 n=8" "recount-cpu: ${P}-scalar-scalar mismatches=1 n=8" "recount-summary: rows=4 zero=0 nonzero=4 uncompared=0" "match=0 mismatch=4 not-run=0"
  w --recount "${work}/out103"
  expect "103 ${L} --recount of the saved form is 1" 1 "recount: ${P}-scalar-scalar mismatches=0 n=8" "recount-cpu: ${P}-scalar-scalar mismatches=1 n=8" "recount-summary: rows=4 zero=0 nonzero=4 uncompared=0"
  check "103 ${L} the record parts are ng and do not carry the detail" "$(python3 -I -c 'import glob, sys
fs = glob.glob(sys.argv[1] + "/*.json")
txt = [open(f).read() for f in fs]
sys.exit(0 if len(fs) == 4 and all("\"ng\"" in t and "cpu-reference" not in t for t in txt) else 1)' "${work}/rec103" && echo 0 || echo 1)"
  # 両方の参照と違う出力は mismatches の詳細
  sp_prep "${fix}/sp-cd.json" "${CD}"
  sp_out_all "${CD}" "${P}" 4=40400000
  gw --cases "${fix}/sp-cd.json"
  expect "103 ${L} output different from both references" 1 "row: ${P}-scalar-scalar mismatch (mismatches 1/8)" "cpu-count: ${P}-scalar-scalar mismatches=1 n=8" "!cpu-reference mismatches"

  # 104. manifest の不正（どれも 2。Node の本体を起動しない・一時のパスを出さない）
  for mode in dropkey dropsha extrakey badsha shortfile rmfile kind; do
    sp_prep "${RJ}" "${C}"
    sp_tamper "${mode}" "${C}"
    gw --cases "${RJ}"
    expect "104 ${L} manifest ${mode}" 2 "error:" "!summary:" "!${work}"
    check "104 ${L} node body not started (${mode})" "$(no_node_started)"
  done

  # 105. 保存の形の名前の集合と数え直し: 0・1・2
  sp_prep "${RJ}" "${C}"
  newdir out105
  gw --cases "${RJ}" --output-dir "${work}/out105"
  expect "105 ${L} --output-dir" 0 "summary: rows=4 match=4"
  check "105 ${L} the saved names include the CPU reference file" "$(dir_names_ok "${work}/out105" "${SAVED}")"
  w --recount "${work}/out105"
  expect "105 ${L} --recount is 0" 0 "recount-cpu: ${P}-simd-simd128 mismatches=0 n=8" "recount-summary: rows=4 zero=4 nonzero=0 uncompared=0"
  rm -rf "${work:?}/out105m"
  cp -R "${work}/out105" "${work}/out105m"
  sp_saved_mut out105m word "${C}"
  w --recount "${work}/out105m"
  expect "105 ${L} --recount of a changed output is 1" 1 "recount: ${P}-scalar-scalar mismatches=1 n=8" "recount-summary: rows=4 zero=3 nonzero=1 uncompared=0"
  rm -rf "${work:?}/out105m"
  cp -R "${work}/out105" "${work}/out105m"
  sp_saved_mut out105m cpuonly "${C}"
  w --recount "${work}/out105m"
  expect "105 ${L} --recount of a changed CPU reference file only is 1" 1 "recount: ${P}-scalar-scalar mismatches=0 n=8" "recount-cpu: ${P}-scalar-scalar mismatches=1 n=8" "recount-summary: rows=4 zero=0 nonzero=4 uncompared=0"
  for mode in badcpusha rmcpu export; do
    rm -rf "${work:?}/out105m"
    cp -R "${work}/out105" "${work}/out105m"
    sp_saved_mut out105m "${mode}" "${C}"
    w --recount "${work}/out105m"
    expect "105 ${L} --recount of a broken saved form (${mode}) is 2" 2 "error:" "!recount-summary:"
  done
  rm -rf "${work:?}/out105m"

  # 108. native-neon の行は not-run (reference-unavailable)。比べず count・cpu-count を出さない
  sp_rows "${fix}/sp-neon.json" "${P}" "${B}" "${C}" native-neon
  sp_prep "${fix}/sp-neon.json" "${C}"
  gw --cases "${fix}/sp-neon.json"
  expect "108 ${L} native-neon rows" 1 "row: ${P}-scalar-scalar not-run (reference-unavailable)" "!count:" "!cpu-count:" "match=0 mismatch=0 not-run=4"
  reset_gen
}

sp_kind "gemm-special" g gemm special-gemm-a special-gemm-cpudiff-a gemm-a "2=7fc00002" "g-scalar-scalar.bin g-simd-scalar.bin g-scalar-simd128.bin g-simd-simd128.bin special-gemm-a.a.bin special-gemm-a.b.bin special-gemm-a.native-scalar.bin special-gemm-a.cpu-reference.bin manifest.json outputs.json"
sp_kind "elementwise-special" e elementwise special-elementwise-add-a special-elementwise-add-cpudiff-a elementwise-a "2=7fc00002" "e-scalar-scalar.bin e-simd-scalar.bin e-scalar-simd128.bin e-simd-simd128.bin special-elementwise-add-a.input.bin special-elementwise-add-a.native-scalar.bin special-elementwise-add-a.cpu-reference.bin manifest.json outputs.json"

# 106. export と種類の照らし: 特殊値の種類の export は gemm・elementwise の 2 つだけ。topk の export は 2。両方の種類を指す行は通る
cat >"${fix}/sp-kinds.json" <<'JSON'
{"rows": [
  {"id": "k-ga", "export": "gemm_scalar", "build": "scalar", "case": "gemm-a", "reference_source": "native-scalar"},
  {"id": "k-gs", "export": "gemm_simd", "build": "simd128", "case": "special-gemm-a", "reference_source": "native-scalar"},
  {"id": "k-ea", "export": "elementwise_scalar", "build": "scalar", "case": "elementwise-a", "reference_source": "native-scalar"},
  {"id": "k-es", "export": "elementwise_simd", "build": "simd128", "case": "special-elementwise-add-a", "reference_source": "native-scalar"}
]}
JSON
sp_prep "${fix}/sp-kinds.json" gemm-a special-gemm-a elementwise-a special-elementwise-add-a
gw --cases "${fix}/sp-kinds.json"
expect "106 both kinds of an export are accepted together" 0 "summary: rows=4 match=4 mismatch=0 not-run=0" "cpu-count: k-gs mismatches=0 n=8" "cpu-count: k-es mismatches=0 n=8" "!cpu-count: k-ga" "!cpu-count: k-ea"
for pair in "gemm_scalar special-elementwise-add-a" "elementwise_scalar special-gemm-a" "topk_scalar special-gemm-a" "topk_simd special-elementwise-add-a"; do
  # shellcheck disable=SC2086
  set -- ${pair}
  printf '{"rows": [{"id":"x","export":"%s","build":"scalar","case":"%s","reference_source":"native-scalar"}]}\n' "$1" "$2" >"${fix}/sp-bad.json"
  sp_prep "${fix}/sp-bad.json" special-gemm-a special-elementwise-add-a
  gw --cases "${fix}/sp-bad.json"
  expect "106 ${1} on ${2}" 2 "does not match the case kind" "!summary:"
  check "106 node body not started (${1} on ${2})" "$(no_node_started)"
done
reset_gen

# 107. 実物の形の名前の特殊値の行を足した合成の入力でも、同梱の入力の項目 22 を直さずに通る
python3 -I - "${fix}/special-rows.json" <<'PY'
import json, sys
rows = [{"id": "keep-fixed", "export": "roundtrip_f32", "input": ["3f800000"], "reference_source": "fixed", "reference": ["3f800000"]}]
cs = (("special-gemm-g-01", "gemm"), ("special-elementwise-relu-e-2", "elementwise"), ("special-elementwise-add-scalar-e-2", "elementwise"))
for c, base in cs:
    for exp, build in (("scalar", "scalar"), ("simd", "scalar"), ("scalar", "simd128"), ("simd", "simd128")):
        rows.append({"id": "%s-%s-%s" % (c, exp, build), "export": "%s_%s" % (base, exp), "build": build, "case": c, "reference_source": "native-scalar"})
json.dump({"rows": rows}, open(sys.argv[1], "w"))
PY
bundled_check special-rows "${fix}/special-rows.json"

# 109. run-glue.cjs（変えていない）に特殊値の種類の manifest を当てる: 呼び方・引数・ビット列・番兵
mk_host_fixture "${work}/glue/spin" special-gemm-a special-elementwise-add-a
cat >"${work}/glue/sp.js" <<'JS'
exports.gemm_scalar = (a, b, m, k, n, out) => {
  if (m !== 2 || k !== 3 || n !== 4 || a.length !== 6 || b.length !== 12 || out.length !== 8) return 9;
  const w = new Uint32Array(out.buffer, out.byteOffset, 8);
  if (w[7] !== 0x7fc0dead) return 5;
  out[0] = a[2];
  return 0;
};
exports.gemm_simd = () => 7;
exports.elementwise_scalar = (opCode, c, a, out) => {
  if (opCode !== 1 || c !== 0.5 || !(a instanceof Float32Array) || a.length !== 8 || out.length !== 8) return 9;
  const u = new Uint32Array(a.buffer, a.byteOffset, 8);
  if (u[0] !== 0x7fc00001 || u[1] !== 0x7f800000 || u[2] !== 0xff800000 || u[4] !== 0x80000000) return 7;
  const w = new Uint32Array(out.buffer, out.byteOffset, 8);
  if (w[7] !== 0x7fc0dead) return 5;
  w[0] = u[0];
  return 0;
};
exports.elementwise_simd = () => 7;
JS
echo 'exports.gemm_simd = () => 0;' >"${work}/glue/spsimd.js"
cat >"${work}/glue/sp-cases.json" <<'JSON'
{"rows": [
  {"id": "s1", "export": "gemm_scalar", "build": "scalar", "case": "special-gemm-a", "reference_source": "native-scalar"},
  {"id": "s2", "export": "gemm_simd", "build": "scalar", "case": "special-gemm-a", "reference_source": "native-scalar"},
  {"id": "s3", "export": "elementwise_scalar", "build": "scalar", "case": "special-elementwise-add-a", "reference_source": "native-scalar"},
  {"id": "s4", "export": "elementwise_simd", "build": "scalar", "case": "special-elementwise-add-a", "reference_source": "native-scalar"}
]}
JSON
rm -rf "${work}/glue/spout"
mkdir -p "${work}/glue/spout"
rc=0
"${REAL_NODE}" "${NODE_SCRIPT}" "${work}/glue/sp.js" "${work}/glue/sp-cases.json" "${work}/glue/rsp.jsonl" "${work}/glue/spsimd.js" "${work}/glue/spin" "${work}/glue/spout" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
expect "109 run-glue special rows" 0
check "109 gemm and elementwise special rows write the bit patterns with the fill word kept" "$(python3 -I -c 'import struct,sys
g = struct.unpack("<8I", open(sys.argv[1], "rb").read())
e = struct.unpack("<8I", open(sys.argv[2], "rb").read())
sys.exit(0 if g == (0x7fc00001,) + (0x7fc0dead,) * 7 and e == (0x7fc00001,) + (0x7fc0dead,) * 7 else 1)' "${work}/glue/spout/s1.bin" "${work}/glue/spout/s3.bin" && echo 0 || echo 1)"
check "109 nonzero returns are error rows and write nothing" "$(python3 -I -c 'import json,os,sys
rows = {json.loads(l)["id"]: json.loads(l) for l in open(sys.argv[1])}
ok = rows["s1"] == {"id": "s1", "output_file": "s1.bin"} and rows["s3"] == {"id": "s3", "output_file": "s3.bin"}
ok = ok and rows["s2"] == {"id": "s2", "error": "nonzero return"} and rows["s4"] == {"id": "s4", "error": "nonzero return"}
ok = ok and sorted(os.listdir(sys.argv[2])) == ["s1.bin", "s3.bin"]
sys.exit(0 if ok else 1)' "${work}/glue/rsp.jsonl" "${work}/glue/spout" && echo 0 || echo 1)"

# 110. 実行の中の数え直しが判定と食い違う場面: 数え直しの側の CPU 参照の比べだけを外した変異体
rm -rf "${work}/mut"
mkdir -p "${work}/mut/scripts" "${work}/mut/wasm/crates/glue"
cp "${DIR}/lib.sh" "${work}/mut/scripts/lib.sh"
cp "${GLUE_MANIFEST}" "${work}/mut/wasm/crates/glue/Cargo.toml"
python3 -I - "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" <<'PY'
import sys
s = open(sys.argv[1], encoding="utf-8").read()
old = 'cpu = cpu_count(cases[r["case"]], data)'
assert s.count(old) == 1
open(sys.argv[2], "w", encoding="utf-8").write(s.replace(old, "cpu = None"))
PY
chmod +x "${work}/mut/scripts/wasm-node-check.sh"
check "110 the mutant differs from the original" "$(cmp -s "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" && echo 1 || echo 0)"
sp_rows "${fix}/sp-g.json" g gemm special-gemm-cpudiff-a
sp_prep "${fix}/sp-g.json" special-gemm-cpudiff-a
newdir out110
newdir rec110
WNC_ORIG="${WNC}"
WNC="${work}/mut/scripts/wasm-node-check.sh"
gw --cases "${fix}/sp-g.json" --output-dir "${work}/out110" --record-dir "${work}/rec110" --record-combination metal-mac-mini
WNC="${WNC_ORIG}"
expect "110 a disagreeing recount of the CPU reference comparison stops the run" 2 "error: recount disagrees" "!row:" "!summary:" "!recount-summary:"
check "110 nothing written to either directory" "$([ "$(nfiles "${work}/out110")" = 0 ] && [ "$(nfiles "${work}/rec110")" = 0 ] && echo 0 || echo 1)"
rm -rf "${work}/mut" "${fix}/outs"
reset_gen
res_ok

# 111. --recount --expect-cases: 保存の形の行の集合・build・種類・寸法を期待の入力から決める。別の和の順の印
# exp_json <出力> [simd 印をつけるケース]: gen.json の行に、gemm-a・gemm-b の期待（m=2・k=3・n=2）を足した期待の入力を書く。
exp_json() {
  python3 -I - "${fix}/gen.json" "$1" "${2:-}" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d["cases"] = []
for name in ("gemm-a", "gemm-b"):
    e = {"name": name, "kind": "gemm", "dims": {"m": 2, "k": 3, "n": 2}}
    if name == sys.argv[3]:
        e["simd_sum_order_differs"] = True
    d["cases"].append(e)
json.dump(d, open(sys.argv[2], "w"))
PY
}
# exp_mut <ディレクトリ名> <モード>: out56 の写しの保存の形を、期待の入力と食い違うように書き換える（sha256 は書き換えに合わせる）。
exp_mut() {
  python3 -I - "${work}/$1" "$2" <<'PY'
import hashlib, json, os, sys
d, mode = sys.argv[1:3]
op, mp = os.path.join(d, "outputs.json"), os.path.join(d, "manifest.json")
o, m = json.load(open(op)), json.load(open(mp))
sha = lambda b: hashlib.sha256(b).hexdigest()
def wr(fn, b):
    open(os.path.join(d, fn), "wb").write(b)
    return sha(b)
def rd(fn):
    return open(os.path.join(d, fn), "rb").read()
def case(name):
    return next(c for c in m["cases"] if c["name"] == name)
if mode == "drop-row":
    os.unlink(os.path.join(d, "g-simd.bin"))
    o["rows"] = o["rows"][:1]
elif mode == "build":
    o["rows"][1]["build"] = "scalar"
elif mode == "shrink":
    c = case("gemm-b")
    c.update(m=1, k=1, n=1)
    c["a_sha256"] = wr(c["a_file"], rd(c["a_file"])[:4])
    c["b_sha256"] = wr(c["b_file"], rd(c["b_file"])[:4])
    nat = rd(c["native_scalar_file"])[:4]
    c["native_scalar_sha256"] = wr(c["native_scalar_file"], nat)
    o["rows"][1]["output_sha256"] = wr("g-simd.bin", nat)
elif mode == "kind":
    # gemm-a を gemm-special の項に替え、出力の語を 1 ビットだけ動かす（厳密な比べなら不一致・閾値の比べなら一致）。
    c = case("gemm-a")
    c["kind"] = "gemm-special"
    c["cpu_reference_file"] = "gemm-a.cpu-reference.bin"
    c["cpu_reference_sha256"] = wr(c["cpu_reference_file"], rd(c["native_scalar_file"]))
    keys = ["name", "kind", "m", "k", "n", "a_file", "b_file", "native_scalar_file", "cpu_reference_file", "a_sha256", "b_sha256", "native_scalar_sha256", "cpu_reference_sha256"]
    m["cases"][m["cases"].index(c)] = {k: c[k] for k in keys}
    b = bytearray(rd("g-scalar.bin"))
    b[0] ^= 1
    o["rows"][0]["output_sha256"] = wr("g-scalar.bin", bytes(b))
elif mode == "near":
    b = bytearray(rd("g-simd.bin"))
    b[0] ^= 1
    o["rows"][1]["output_sha256"] = wr("g-simd.bin", bytes(b))
json.dump(o, open(op, "w"))
json.dump(m, open(mp, "w"))
PY
}
exp_json "${fix}/exp.json"
exp_json "${fix}/exp-flat.json" gemm-b
w --recount "${work}/out56" --expect-cases "${fix}/exp.json"
expect "111 --recount with the expected cases" 0 "recount: g-scalar mismatches=0 n=4" "recount-summary: rows=2 zero=2 nonzero=0 uncompared=0" "!recount-flat:"
w --expect-cases "${fix}/exp.json"
expect "111 --expect-cases needs --recount" 2 "usage:"
w --wasm "${DW}" --expect-cases "${fix}/exp.json"
expect "111 --expect-cases does not combine with the run options" 2 "usage:"
w --recount "${work}/out56" --expect-cases
expect "111 --expect-cases without value" 2 "usage:"
w --recount "${work}/out56" --expect-cases "${work}/dirs/missing.json"
expect "111 --expect-cases with an unreadable file" 2 "cannot read cases" "!recount-summary:"
for mode in drop-row build; do
  saved_copy out111
  exp_mut out111 "${mode}"
  w --recount "${work}/out111" --expect-cases "${fix}/exp.json"
  expect "111 saved form with ${mode} against the expected rows" 2 "saved rows differ from the expected rows" "!recount-summary:"
done
saved_copy out111
exp_mut out111 shrink
w --recount "${work}/out111" --expect-cases "${fix}/exp.json"
expect "111 gemm-b shrunk in the saved form" 2 "manifest case differs from the expected case" "!recount-summary:"
saved_copy out111
exp_mut out111 kind
w --recount "${work}/out111" --expect-cases "${fix}/exp.json"
expect "111 gemm-a rewritten as gemm-special in the saved form" 2 "manifest case differs from the expected case" "!recount-summary:"
w --recount "${work}/out111"
expect "111 the same rewritten form is counted by the threshold without --expect-cases" 0 "recount: g-scalar mismatches=0 n=4"
# 別の和の順の印: 参照と全語が同じなら不一致、1 ビット違えば一致
w --recount "${work}/out56" --expect-cases "${fix}/exp-flat.json"
expect "111 simd128 gemm_simd identical to the reference with the flag" 1 "recount: g-simd mismatches=0 n=4" "recount-flat: g-simd max-abs=0.0" "recount-summary: rows=2 zero=1 nonzero=1 uncompared=0"
saved_copy out111
exp_mut out111 near
w --recount "${work}/out111" --expect-cases "${fix}/exp-flat.json"
expect "111 simd128 gemm_simd one bit away from the reference with the flag" 0 "recount-summary: rows=2 zero=2 nonzero=0" "!recount-flat:"
# 実行の中の判定でも同じ期待を使う
gen_setup
gw --cases "${fix}/exp.json"
expect "111 a run with matching expected cases" 0 "row: g-simd match" "summary: rows=2 match=2"
gen_setup
gw --cases "${fix}/exp-flat.json"
expect "111 a run whose flagged row never left the reference" 1 "row: g-simd mismatch (max-abs is zero (simd128 sum order not exercised))" "recount-flat: g-simd max-abs=0.0" "summary: rows=2 match=1 mismatch=1" "!recount disagrees"
gen_setup
python3 -I - "${fix}/exp.json" "${fix}/exp-dims.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d["cases"][1]["dims"]["n"] = 3
json.dump(d, open(sys.argv[2], "w"))
PY
gw --cases "${fix}/exp-dims.json"
expect "111 a run whose expected dimensions differ from the manifest" 2 "manifest case differs from the expected case" "!row:" "!summary:"
python3 -I - "${fix}/exp.json" "${fix}/exp-extra.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d["cases"].append({"name": "gemm-c", "kind": "gemm", "dims": {"m": 2, "k": 3, "n": 2}})
json.dump(d, open(sys.argv[2], "w"))
PY
gen_setup
gw --cases "${fix}/exp-extra.json"
expect "111 a run whose expected cases name a case the manifest lacks" 2 "manifest cases differ from the expected cases" "!summary:"
for bad in '{"name": "x", "kind": "nope", "dims": {"n": 1}}' '{"name": "x", "kind": "gemm", "dims": {"m": 1, "k": 1}}' '{"name": "x", "kind": "gemm", "dims": {"m": 1, "k": 1, "n": 0}}' '{"name": "x", "kind": "topk", "dims": {"n": 1, "k": 1}, "simd_sum_order_differs": true}' '{"name": "x", "kind": "gemm", "dims": {"m": 1, "k": 1, "n": 1}, "other": 1}'; do
  python3 -I - "${fix}/exp.json" "${fix}/exp-bad.json" "${bad}" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d["cases"] = [json.loads(sys.argv[3])]
json.dump(d, open(sys.argv[2], "w"))
PY
  w --recount "${work}/out56" --expect-cases "${fix}/exp-bad.json"
  expect "111 an invalid expected case ${bad}" 2 "error:" "!recount-summary:"
done
reset_gen

# ---- ネイティブの参照の行（TASK-995 の 5/5） ----
cat >"${fix}/n1.json" <<'JSON'
{"rows": [
  {"id": "n-neon", "export": "gemm_simd", "build": "simd128", "case": "gemm-t1", "reference_source": "native-neon"}
]}
JSON
cat >"${fix}/n2.json" <<'JSON'
{"rows": [
  {"id": "n-simd", "export": "gemm_simd", "build": "simd128", "case": "gemm-t1", "reference_source": "native-scalar"},
  {"id": "n-neon", "export": "gemm_simd", "build": "simd128", "case": "gemm-t1", "reference_source": "native-neon"}
]}
JSON
cat >"${fix}/n1-expect.json" <<'JSON'
{"rows": [
  {"id": "n-neon", "export": "gemm_simd", "build": "simd128", "case": "gemm-t1", "reference_source": "native-neon"}
], "cases": [
  {"name": "gemm-t1", "kind": "gemm", "dims": {"m": 1, "k": 1, "n": 8}, "simd_sum_order_differs": true}
]}
JSON
# n_setup <行の入力> [add_neon_ref の引数（添字=語）...]: gemm-t1 の host 側の道具の出力に native-neon の参照を足し、全行の結果を用意する。
n_setup() {
  local rows="$1"
  shift
  t1_setup "${rows}"
  if [ "${1:-}" != "none" ]; then
    add_neon_ref "${fix}/hf" gemm-t1 "$@"
  fi
  rm -rf "${fix}/outs"
}
N_SAVED_NAMES="n-simd.bin gemm-t1.a.bin gemm-t1.b.bin gemm-t1.native-scalar.bin gemm-t1.native-neon.bin manifest.json outputs.json"

# 112. 参照のファイルと比べる。scalar の参照とは違い、native-neon の参照とは同じ出力は一致
n_setup "${fix}/n1.json" 4=40000000
t1_out n-neon.bin 4=40000000
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/n1.json"
expect "112 an output equal to the native-neon reference matches" 0 "row: n-neon match" "count: n-neon mismatches=0 n=8" "sha256: n-neon " "t1: n-neon max-abs=0.0" "sha256: gemm-t1.native-neon " "summary: rows=1 match=1 mismatch=0 not-run=0" "!recount-summary:"
reset_gen
rm -rf "${fix}/outs"

# 113. 参照から閾値を超える 1 語。出力は scalar の参照と同じ（scalar と比べる変異を捕まえる）
n_setup "${fix}/n1.json" 4=40000000
t1_out n-neon.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/n1.json"
expect "113 one word beyond the threshold from the native-neon reference is a mismatch" 1 "row: n-neon mismatch (mismatches 1/8)" "count: n-neon mismatches=1 n=8" "summary: rows=1 match=0 mismatch=1 not-run=0"
reset_gen
rm -rf "${fix}/outs"

# 114. 参照の鍵が無い manifest は not-run。実物の 3 行も鍵の無い host の出力では not-run
n_setup "${fix}/n1.json" none
gw --cases "${fix}/n1.json"
expect "114 a manifest without the native-neon keys leaves the row not-run" 1 "row: n-neon not-run (reference-unavailable)" "summary: rows=1 match=0 mismatch=0 not-run=1" "!count:" "!t1:" "!sha256:"
reset_gen
python3 -I - "${CASES_REAL}" "${fix}/n3rows.json" "${FAKE_RESULTS}" "${fix}/n3-rowmap" <<'PY'
import json, sys
rows = [r for r in json.load(open(sys.argv[1]))["rows"] if r.get("reference_source") == "native-neon"]
json.dump({"rows": rows}, open(sys.argv[2], "w"))
open(sys.argv[3], "w").write("".join(json.dumps({"id": r["id"], "output_file": r["id"] + ".bin"}) + "\n" for r in rows))
open(sys.argv[4], "w").write("".join(r["id"] + " " + r["case"] + "\n" for r in rows))
PY
rm -rf "${fix}/hf114"
mk_host_fixture "${fix}/hf114" gemm-64 gemm-256 gemm-37x41x53
export FAKE_HOST_FIXTURE="${fix}/hf114" FAKE_ROW_MAP="${fix}/n3-rowmap"
gw --cases "${fix}/n3rows.json"
expect "114 the three bundled native-neon rows are not-run against a host output without the keys" 1 "row: gemm-64-native-neon not-run (reference-unavailable)" "row: gemm-256-native-neon not-run (reference-unavailable)" "row: gemm-37x41x53-native-neon not-run (reference-unavailable)" "summary: rows=3 match=0 mismatch=0 not-run=3"
reset_gen
res_ok

# 115. 参照が scalar の参照と同じバイト列: 期待の入力で別の和の順の印があるときだけ mismatch
n_setup "${fix}/n1-expect.json"
t1_out n-neon.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/n1-expect.json"
expect "115 a native-neon reference equal to the native-scalar one is a mismatch when the sum order is expected to differ" 1 "row: n-neon mismatch (native-neon equals native-scalar)" "summary: rows=1 match=0 mismatch=1 not-run=0"
reset_gen
rm -rf "${fix}/outs"
n_setup "${fix}/n1.json"
t1_out n-neon.bin
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/n1.json"
expect "115 the same references without the expected cases are not checked" 0 "row: n-neon match"
reset_gen
rm -rf "${fix}/outs"

# 116. manifest の不正は終了コード 2
n_setup "${fix}/n1.json" 4=40000000
python3 -I - "${fix}/hf/manifest.json" "${fix}/hf/gemm-t1.native-neon.bin" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
m["cases"][0]["native_neon_sha256"] = "0" * 64
json.dump(m, open(sys.argv[1], "w"))
PY
gw --cases "${fix}/n1.json"
expect "116 a native-neon sha256 that differs from the file stops the run" 2 "error: manifest sha256 mismatch" "!row:"
reset_gen
n_setup "${fix}/n1.json" 4=40000000
python3 -I - "${fix}/hf" <<'PY'
import hashlib, json, os, sys
d = sys.argv[1]
fn = os.path.join(d, "gemm-t1.native-neon.bin")
b = open(fn, "rb").read()[:28]
open(fn, "wb").write(b)
m = json.load(open(os.path.join(d, "manifest.json")))
m["cases"][0]["native_neon_sha256"] = hashlib.sha256(b).hexdigest()
json.dump(m, open(os.path.join(d, "manifest.json"), "w"))
PY
gw --cases "${fix}/n1.json"
expect "116 a native-neon file of the wrong length stops the run" 2 "error: manifest file length mismatch" "!row:"
reset_gen
for drop in native_neon_file native_neon_sha256; do
  n_setup "${fix}/n1.json" 4=40000000
  python3 -I - "${fix}/hf/manifest.json" "${drop}" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
del m["cases"][0][sys.argv[2]]
json.dump(m, open(sys.argv[1], "w"))
PY
  gw --cases "${fix}/n1.json"
  expect "116 only ${drop} present stops the run" 2 "error: manifest case has missing or unknown keys" "!row:"
  reset_gen
done
cat >"${fix}/n-topk.json" <<'JSON'
{"rows": [
  {"id": "n-topk", "export": "topk_scalar", "build": "scalar", "case": "topk-a", "reference_source": "native-scalar"}
]}
JSON
rm -rf "${fix}/hf-nt"
mk_host_fixture "${fix}/hf-nt" topk-a
python3 -I - "${fix}/hf-nt/manifest.json" "${fix}/n-topk.json" "${FAKE_RESULTS}" "${fix}/nt-rowmap" <<'PY'
import hashlib, json, sys
m = json.load(open(sys.argv[1]))
c = m["cases"][0]
c["native_neon_file"] = c["native_scalar_file"]
c["native_neon_sha256"] = c["native_scalar_sha256"]
json.dump(m, open(sys.argv[1], "w"))
rows = json.load(open(sys.argv[2]))["rows"]
open(sys.argv[3], "w").write("".join(json.dumps({"id": r["id"], "output_file": r["id"] + ".bin"}) + "\n" for r in rows))
open(sys.argv[4], "w").write("".join(r["id"] + " " + r["case"] + "\n" for r in rows))
PY
export FAKE_HOST_FIXTURE="${fix}/hf-nt" FAKE_ROW_MAP="${fix}/nt-rowmap"
gw --cases "${fix}/n-topk.json"
expect "116 the native-neon keys on a topk case stop the run" 2 "error: manifest case has missing or unknown keys" "!row:"
reset_gen
res_ok

# 117. native-scalar の行と native-neon の行の混在: 保存の形・数え直し
n_setup "${fix}/n2.json" 4=40000000
t1_out n-simd.bin 4=3f800001
t1_out n-neon.bin 4=40000000
export FAKE_OUTPUTS="${fix}/outs"
newdir out117
gw --cases "${fix}/n2.json" --output-dir "${work}/out117"
expect "117 a mixed run matches and saves" 0 "row: n-simd match" "row: n-neon match" "summary: rows=2 match=2 mismatch=0 not-run=0" "recount-summary: rows=1 zero=1 nonzero=0 uncompared=0" "sha256: gemm-t1.native-neon " "t1: n-neon " "t1: n-simd "
check "117 the saved form holds the reference file and no output of the native-neon row" "$(dir_names_ok "${work}/out117" "${N_SAVED_NAMES}")"
w --recount "${work}/out117"
expect "117 the saved form is recounted" 0 "recount-summary: rows=1 zero=1 nonzero=0 uncompared=0" "recount: n-simd mismatches=0 n=8" "sha256: gemm-t1.native-neon " "!n-neon"
# 117b. 数え直しの側の t1_count の引数だけを入れ替えた変異体は食い違いで終了コード 2
rm -rf "${work}/mut"
mkdir -p "${work}/mut/scripts" "${work}/mut/wasm/crates/glue"
cp "${DIR}/lib.sh" "${work}/mut/scripts/lib.sh"
cp "${GLUE_MANIFEST}" "${work}/mut/wasm/crates/glue/Cargo.toml"
python3 -I - "${WNC}" "${work}/mut/scripts/wasm-node-check.sh" <<'PY'
import sys
s = open(sys.argv[1], encoding="utf-8").read()
old = "diff, t1, max_abs = t1_count(want, data)"
assert s.count(old) == 1
open(sys.argv[2], "w", encoding="utf-8").write(s.replace(old, "diff, t1, max_abs = t1_count(data, want)"))
PY
chmod +x "${work}/mut/scripts/wasm-node-check.sh"
newdir out117b
WNC_ORIG="${WNC}"
WNC="${work}/mut/scripts/wasm-node-check.sh"
gw --cases "${fix}/n2.json" --output-dir "${work}/out117b"
WNC="${WNC_ORIG}"
expect "117 a recount that swaps the threshold arguments disagrees" 2 "error: recount disagrees"
rm -rf "${work}/mut"
reset_gen
rm -rf "${fix}/outs"

# 118. 期待の入力があり、参照が scalar と違い、出力が参照とビットで同じ: 別の和の順を通らなかった行
n_setup "${fix}/n1-expect.json" 4=3f800001
t1_out n-neon.bin 4=3f800001
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/n1-expect.json"
expect "118 an output identical to a differing reference is flagged as flat" 1 "row: n-neon mismatch (max-abs is zero" "summary: rows=1 match=0 mismatch=1 not-run=0"
reset_gen
rm -rf "${fix}/outs"

# 119. 語数の違い・呼び出しの失敗
n_setup "${fix}/n1.json" 4=40000000
t1_out n-neon.bin short
export FAKE_OUTPUTS="${fix}/outs"
gw --cases "${fix}/n1.json"
expect "119 a short output is a length mismatch" 1 "row: n-neon mismatch (length expected 8 got 7)" "!t1:"
reset_gen
rm -rf "${fix}/outs"
n_setup "${fix}/n1.json" 4=40000000
printf '%s\n' '{"id":"n-neon","error":"call failed"}' >"${FAKE_RESULTS}"
gw --cases "${fix}/n1.json"
expect "119 a call error is a mismatch" 1 "row: n-neon mismatch (call error)"
reset_gen
res_ok

echo "passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
