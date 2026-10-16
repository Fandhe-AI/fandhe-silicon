#!/usr/bin/env bash
# wasm32 の .wasm を wasm-bindgen の glue 越しに Node で呼び、結果をネイティブの参照と照合する土台
# （spec TASK-22 のポインタ）。行ごとに match（一致）・mismatch（不一致）・not-run（未実施）を出す。
# 呼び出し元: `make wasm-node-check`（直接実行も可）。CI では `wasm-macos` ジョブが呼ぶ（回帰テストは
# `make test-wasm-node-check`＝scripts/test-wasm-node-check.sh。CI の deny ジョブが呼ぶ）。
#
# 使い方: scripts/wasm-node-check.sh [--wasm <path>] [--cases <path>]
#   --wasm  既にある .wasm を使う（cargo を呼ばない）。省略時は wasm/ を --profile wasm-size でビルドする
#   --cases 照合の入力の JSON（既定: wasm/node/cases.json）
# 終了コード: 0 全行 match / 1 mismatch か not-run がある・行が 0 件 / 2 誤用・前提の不足（node と
#   wasm-bindgen の CLI の版違いを含む）・入力や結果が不正・Node の実行が失敗
#
# 役割の分担: wasm/node/run-glue.cjs は glue を呼んで f32 のビット列を書き出すだけで、判定は持たない。
# 判定は本スクリプトの python3（標準ライブラリだけ・-I）にあり、入力の値をシェルに展開しない。
# 行の参照の出どころは必須の欄 reference_source（fixed＝固定値 / native-neon＝ネイティブの NEON）で、
# 欠け・未知の値は終了コード 2。出どころをこの環境で作れない行（今は native-neon の全行。値があっても比べない）は
# `not-run (reference-unavailable)`、作れる出どころで参照（reference）が無い行は `not-run (reference-missing)` にする。
# どちらも一致扱いにしない。出どころを先に見る。TASK-23 のラベルへの写しは crates/core/src/rerun/reference.rs（TASK-29）。
# ネットワークは使わない・npm パッケージは足さない。
# 生成物（glue・結果）は mktemp -d の一時ディレクトリにだけ出し、終了時に消す。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# 要求する Node の版（宣言点はここ 1 か所）。wasm-bindgen の CLI の版は wasm/crates/glue/Cargo.toml から読む。
NODE_EXPECTED="v24.13.0"
GLUE_MANIFEST="${ROOT_DIR}/wasm/crates/glue/Cargo.toml"
NODE_SCRIPT="${ROOT_DIR}/wasm/node/run-glue.cjs"
USAGE="usage: $(basename "$0") [--wasm <path>] [--cases <path>]"

# 相対パスは呼び出し元の cwd 基準で絶対パスにする（後でリポジトリルートへ cd しても同じファイルを指させる）。
abspath() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "${PWD}" "$1" ;;
  esac
}

# 入力の検査と判定（固定の文字列。入力のパスは引数で渡し、値はシェルに触れさせない）。
# 引数: validate <cases> / judge <cases> <results>。終了コード 0 / 1（judge の不一致・未実施・行 0 件）/ 2（入力が不正）。
read -r -d '' PY_CODE <<'PY' || true
import json
import re
import sys

HEX = re.compile(r"[0-9a-f]{8}\Z")
ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}\Z")
EXPORTS = ("roundtrip_f32",)
# 参照の出どころ（綴りは crates/core/src/rerun/reference.rs の ReferenceSource::name に揃える）。
SOURCES = ("fixed", "native-neon")
# この環境で参照を作れる出どころ（定数の 1 か所。ネイティブで作る経路は無いので fixed だけ）。引数・環境変数では変えない。
PRODUCIBLE = ("fixed",)


def die(msg):
    print("error: " + msg, file=sys.stderr)
    sys.exit(2)


def hexlist(v, what):
    if not isinstance(v, list):
        die(what + " must be a list")
    for h in v:
        if not isinstance(h, str) or not HEX.match(h):
            die(what + " has an invalid bit pattern")
    return v


def load_cases(path):
    try:
        with open(path, encoding="utf-8") as f:
            d = json.load(f)
    except (OSError, ValueError):
        die("cannot read cases as JSON")
    if not isinstance(d, dict) or set(d) != {"rows"} or not isinstance(d["rows"], list):
        die("cases must be an object with a rows list")
    seen = set()
    rows = []
    for r in d["rows"]:
        if not isinstance(r, dict) or not {"id", "export", "input", "reference_source"} <= set(r) or not set(r) <= {"id", "export", "input", "reference_source", "reference"}:
            die("a row has missing or unknown keys")
        rid = r["id"]
        if not isinstance(rid, str) or not ID.match(rid):
            die("a row id has an invalid form")
        if rid in seen:
            die("duplicate row id: " + rid)
        seen.add(rid)
        if not isinstance(r["export"], str) or r["export"] not in EXPORTS:
            die("unknown export in row: " + rid)
        inp = hexlist(r["input"], "input")
        src = r["reference_source"]
        if not isinstance(src, str) or src not in SOURCES:
            die("unknown reference_source in row: " + rid)
        ref = r.get("reference")
        if ref is not None:
            ref = hexlist(ref, "reference")
        rows.append((rid, inp, ref, src))
    return rows


def load_results(path, ids):
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except (OSError, ValueError):
        die("cannot read results")
    res = {}
    for ln in lines:
        try:
            o = json.loads(ln)
        except ValueError:
            die("results line is not JSON")
        if not isinstance(o, dict) or not isinstance(o.get("id"), str) or o["id"] not in ids:
            die("results line has an unknown or missing id")
        if o["id"] in res:
            die("results repeat an id")
        if set(o) == {"id", "output"}:
            res[o["id"]] = ("output", hexlist(o["output"], "output"))
        elif set(o) == {"id", "error"} and isinstance(o["error"], str):
            res[o["id"]] = ("error", o["error"])
        else:
            die("results line has an unexpected shape")
    if set(res) != ids:
        die("results lack a row")
    return res


def main():
    mode = sys.argv[1]
    rows = load_cases(sys.argv[2])
    if mode == "validate":
        return 0
    res = load_results(sys.argv[3], {r[0] for r in rows})
    counts = {"match": 0, "mismatch": 0, "not-run": 0}
    for rid, _inp, ref, src in rows:
        kind, val = res[rid]
        detail = ""
        if src not in PRODUCIBLE:
            state = "not-run"
            detail = "reference-unavailable"
        elif ref is None:
            state = "not-run"
            detail = "reference-missing"
        elif kind == "error":
            state = "mismatch"
            detail = "call error"
        elif val == ref:
            state = "match"
        else:
            state = "mismatch"
            if len(val) != len(ref):
                detail = "length expected %d got %d" % (len(ref), len(val))
            else:
                detail = "first difference at index %d" % next(i for i in range(len(ref)) if val[i] != ref[i])
        counts[state] += 1
        print("row: %s %s%s" % (rid, state, " (" + detail + ")" if detail else ""))
    print("summary: rows=%d match=%d mismatch=%d not-run=%d" % (len(rows), counts["match"], counts["mismatch"], counts["not-run"]))
    if not rows:
        print("error: no rows", file=sys.stderr)
        return 1
    return 0 if counts["match"] == len(rows) else 1


sys.exit(main())
PY

die2() {
  echo "error: $1" >&2
  exit 2
}

# 前提のコマンドが無いときは成功扱いにせず終了コード 2 にする（導入は試みない）。
need() {
  command -v "$1" >/dev/null 2>&1 || die2 "required command not found on PATH: $1"
}

main() {
  local wasm_arg="" cases="${ROOT_DIR}/wasm/node/cases.json"
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --wasm)
        [ "$#" -ge 2 ] || { echo "${USAGE}" >&2; exit 2; }
        wasm_arg="$(abspath "$2")"
        shift 2
        ;;
      --cases)
        [ "$#" -ge 2 ] || { echo "${USAGE}" >&2; exit 2; }
        cases="$(abspath "$2")"
        shift 2
        ;;
      *)
        echo "${USAGE}" >&2
        exit 2
        ;;
    esac
  done

  need node
  need wasm-bindgen
  need python3
  if [ -z "${wasm_arg}" ]; then
    need cargo
    need jq
  fi

  # 版は完全一致で要求する（前方一致・rc 付き・別版は失敗）。.wasm のビルドより前に確かめる。
  local node_actual wb_expected wb_actual
  node_actual="$(node --version)" || die2 "node --version failed"
  [ "${node_actual}" = "${NODE_EXPECTED}" ] || die2 "node version mismatch: expected ${NODE_EXPECTED}, got ${node_actual}"
  wb_expected="$(sed -n 's/^wasm-bindgen = "=\([0-9][0-9.]*\)".*/\1/p' "${GLUE_MANIFEST}" | head -n 1)"
  [ -n "${wb_expected}" ] || die2 "cannot read the wasm-bindgen version from ${GLUE_MANIFEST}"
  wb_actual="$(wasm-bindgen --version)" || die2 "wasm-bindgen --version failed"
  [ "${wb_actual}" = "wasm-bindgen ${wb_expected}" ] || die2 "wasm-bindgen version mismatch: expected wasm-bindgen ${wb_expected}, got ${wb_actual}"
  echo "node: ${node_actual}"
  echo "wasm-bindgen: ${wb_expected}"

  python3 -I -c "${PY_CODE}" validate "${cases}" || exit "$?"

  local wasm_file
  if [ -n "${wasm_arg}" ]; then
    wasm_file="${wasm_arg}"
  else
    cd "${ROOT_DIR}"
    log "cargo build (wasm/, wasm32-unknown-unknown, profile wasm-size)"
    cargo build --manifest-path wasm/Cargo.toml --workspace --locked \
      --target wasm32-unknown-unknown --profile wasm-size >&2
    local target_dir
    target_dir="$(cargo metadata --format-version 1 --no-deps --locked --manifest-path wasm/Cargo.toml | jq -r '.target_directory')"
    wasm_file="${target_dir}/wasm32-unknown-unknown/wasm-size/wasm_glue.wasm"
  fi
  [ -s "${wasm_file}" ] || die2 "missing or empty wasm file: ${wasm_file}"

  # EXIT の trap から参照するので関数のローカルにしない。
  work=""
  work="$(mktemp -d "${TMPDIR:-/tmp}/wasm-node-check.XXXXXX")"
  trap 'rm -rf "${work:?}"' EXIT

  wasm-bindgen --target nodejs --out-dir "${work}" "${wasm_file}" >&2 || die2 "wasm-bindgen failed"
  local base glue
  base="$(basename "${wasm_file}" .wasm)"
  glue="${work}/${base}.js"
  [ -s "${glue}" ] || die2 "glue was not generated: ${base}.js"

  local results="${work}/results.jsonl"
  node "${NODE_SCRIPT}" "${glue}" "${cases}" "${results}" >&2 || die2 "node run failed"

  local rc=0
  python3 -I -c "${PY_CODE}" judge "${cases}" "${results}" || rc=$?
  exit "${rc}"
}

main "$@"
