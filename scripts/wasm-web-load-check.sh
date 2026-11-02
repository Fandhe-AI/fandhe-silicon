#!/usr/bin/env bash
# wasm-bindgen の `--target web` の出力を Node の ESM で読み込み、7 名の export の有無と detect_isa_bits の戻り値を
# 4 組 × 11 項目の行で出す検査の本体（spec TASK-248 のポインタ。項目 BUILD-43）。
# 呼び出し元: 今は直接実行（回帰テストは scripts/test-wasm-web-load-check.sh）。
#
# 使い方: scripts/wasm-web-load-check.sh --from-dir <dir>
#   --from-dir  生成済みの出力を読む（必須・1 回だけ）。置き方は `<dir>/<組>/web/wasm_glue.js` と `wasm_glue_bg.wasm`。
#               組の綴りと置き方は scripts/wasm-size.sh の `--from-dir` に揃える（TASK-247 のポインタ）。
#               ビルドの経路は持たない（`--from-dir` が無ければ誤用）
# 標準出力: 組ごとの `init:` 行、項目ごとの `item:` 行、最後の `summary:` 行（終了コード 2 のときは何も出さない）
# 標準エラー: `error:`（2）・`FAILED:`（1）
# 終了コード: 0 4 組とも init が ok で 44 項目とも ok / 1 ng か not-run が 1 つ以上 /
#   2 誤用・前提の不足（node の版違いを含む）・読めない入力・不正な結果
#
# 役割の分担: wasm/node/load-web.mjs は読み込んで観測するだけで判定を持たない。判定は本スクリプトの python3
# （標準ライブラリだけ・-I）にあり、入力の値をシェルに展開しない。node の出力は捨てる（パスを含みうるため）。
# `ok` は呼んで確かめた項目（detect-isa-bits）にだけ付く。計算の 10 項目は呼び方の規則を持たないので not-run。
# ホストの絶対パス・環境変数の値は出力に出さない。ネットワークは使わない・npm パッケージは足さない。
# 生成物は mktemp -d の一時ディレクトリにだけ出し、終了時に消す。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# 要求する Node の版（宣言点はここ 1 か所）。
NODE_EXPECTED="v24.13.0"
NODE_SCRIPT="${ROOT_DIR}/wasm/node/load-web.mjs"
USAGE="usage: $(basename "$0") --from-dir <dir>"

abspath() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "${PWD}" "$1" ;;
  esac
}

die2() {
  echo "error: $1" >&2
  exit 2
}

need() {
  command -v "$1" >/dev/null 2>&1 || die2 "required command not found on PATH: $1"
}

# 検査と判定（固定の文字列）。引数: <from-dir> <work> <node-script>。
read -r -d '' PY_CODE <<'PY' || true
import json
import os
import stat
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python3 3.8 or newer is required", file=sys.stderr)
    sys.exit(2)

# 組の綴り（宣言点はここ 1 か所）。出力先は web だけ。
COMBOS = ("release-scalar", "release-simd128", "wasm-size-scalar", "wasm-size-simd128")
TARGET_DIR = "web"
JS_FILE = "wasm_glue.js"
WASM_FILE = "wasm_glue_bg.wasm"
# 許可表（load-web.mjs と同じ集合）。
EXPORTS = (
    "gemm_scalar",
    "gemm_simd",
    "topk_scalar",
    "topk_simd",
    "elementwise_scalar",
    "elementwise_simd",
    "detect_isa_bits",
)
# 11 項目 → export（宣言点はここ 1 か所）。
ITEMS = (
    ("gemm-scalar-1", "gemm_scalar"),
    ("gemm-scalar-2", "gemm_scalar"),
    ("gemm-simd-1", "gemm_simd"),
    ("gemm-simd-2", "gemm_simd"),
    ("topk-scalar", "topk_scalar"),
    ("topk-simd", "topk_simd"),
    ("elementwise-scalar-relu", "elementwise_scalar"),
    ("elementwise-scalar-add", "elementwise_scalar"),
    ("elementwise-simd-relu", "elementwise_simd"),
    ("elementwise-simd-add", "elementwise_simd"),
    ("detect-isa-bits", "detect_isa_bits"),
)
ISA_ITEM = "detect-isa-bits"
U32_MAX = 4294967295


def die(msg):
    print("error: " + msg, file=sys.stderr)
    sys.exit(2)


def lstat_or_none(p):
    try:
        return os.lstat(p)
    except FileNotFoundError:
        return None
    except OSError:
        die("cannot inspect an input")


def plain_dir(p):
    # None なら無い。シンボリックリンク・ディレクトリでない物は読めない入力。
    st = lstat_or_none(p)
    if st is None:
        return False
    if stat.S_ISLNK(st.st_mode) or not stat.S_ISDIR(st.st_mode):
        die("input is not a plain directory")
    return True


def expected_isa(combo):
    return 0 if combo.endswith("-scalar") else 1


def check_input(root, combo):
    # 返り値: (組の web ディレクトリ, 欠けの理由 | None)
    cdir = os.path.join(root, combo)
    if not plain_dir(cdir):
        return None, "missing-directory"
    wdir = os.path.join(cdir, TARGET_DIR)
    if not plain_dir(wdir):
        return None, "missing-directory"
    reason = None
    for name in (JS_FILE, WASM_FILE):
        st = lstat_or_none(os.path.join(wdir, name))
        if st is None:
            reason = reason or "missing-file"
            continue
        if stat.S_ISLNK(st.st_mode) or not stat.S_ISREG(st.st_mode):
            die("input is not a regular file")
        if st.st_size == 0:
            reason = reason or "empty-file"
    return wdir, reason


def load_result(path):
    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.loads(f.read())
    except (OSError, ValueError):
        die("node result is missing or not valid JSON")
    if not isinstance(data, dict):
        die("node result is not an object")
    init = data.get("init")
    if init == "failed":
        if set(data) != {"init"}:
            die("node result has unexpected fields")
        return data
    if init != "ok":
        die("node result has an unknown init")
    exports = data.get("exports")
    if not isinstance(exports, list) or not all(isinstance(e, str) for e in exports):
        die("node result exports is not a list of strings")
    if len(set(exports)) != len(exports) or any(e not in EXPORTS for e in exports):
        die("node result exports has a duplicate or an unknown name")
    has_isa = "detect_isa_bits" in exports
    if set(data) != ({"init", "exports", "detect_isa_bits"} if has_isa else {"init", "exports"}):
        die("node result has unexpected fields")
    if has_isa:
        v = data["detect_isa_bits"]
        if v != "call-failed":
            if isinstance(v, bool) or not isinstance(v, int) or not 0 <= v <= U32_MAX:
                die("node result detect_isa_bits is invalid")
    return data


def main():
    root, work, node_script = sys.argv[1], sys.argv[2], sys.argv[3]
    st = lstat_or_none(root)
    if st is None or stat.S_ISLNK(st.st_mode) or not stat.S_ISDIR(st.st_mode):
        die("--from-dir must be an existing plain directory")
    # 入力を全部検査してから node を流す。
    inputs = [(combo,) + check_input(root, combo) for combo in COMBOS]
    judged = []
    for combo, wdir, reason in inputs:
        if reason is not None:
            judged.append((combo, "not-run", reason, None))
            continue
        out = os.path.join(work, combo + ".json")
        try:
            rc = subprocess.run(
                ["node", node_script, wdir, out],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            ).returncode
        except OSError:
            die("cannot run node")
        if rc != 0:
            die("node run failed")
        judged.append((combo, "run", None, load_result(out)))

    lines = []
    counts = {"ok": 0, "ng": 0, "not-run": 0}

    def item(combo, name, exp, label, reason=None, value=None):
        counts[label] += 1
        s = "item: combo=%s item=%s export=%s label=%s" % (combo, name, exp, label)
        if reason:
            s += " reason=" + reason
        if value is not None:
            s += " value=%s" % value
        lines.append(s)

    for combo, kind, reason, res in judged:
        if kind == "not-run":
            lines.append("init: combo=%s target=web result=not-run reason=%s" % (combo, reason))
            for name, exp in ITEMS:
                item(combo, name, exp, "not-run", reason)
            continue
        if res["init"] == "failed":
            lines.append("init: combo=%s target=web result=ng" % combo)
            for name, exp in ITEMS:
                item(combo, name, exp, "not-run", "init-failed")
            continue
        lines.append("init: combo=%s target=web result=ok" % combo)
        for name, exp in ITEMS:
            if exp not in res["exports"]:
                item(combo, name, exp, "not-run", "missing-export")
            elif name == ISA_ITEM:
                v = res["detect_isa_bits"]
                if v == "call-failed":
                    item(combo, name, exp, "ng", "call-failed")
                elif v == expected_isa(combo):
                    item(combo, name, exp, "ok", None, v)
                else:
                    item(combo, name, exp, "ng", None, v)
            else:
                item(combo, name, exp, "not-run", "no-call-rule")

    # init 行は組ごとに先頭へ、項目行がその後ろに続く並びで出ている。
    for ln in lines:
        print(ln)
    total = len(COMBOS) * len(ITEMS)
    print("summary: combos=%d items=%d ok=%d ng=%d not-run=%d" % (len(COMBOS), total, counts["ok"], counts["ng"], counts["not-run"]))
    ok_init = all(k == "run" and r["init"] == "ok" for _, k, _, r in judged)
    if counts["ok"] == total and ok_init:
        return 0
    print("FAILED: ng=%d not-run=%d" % (counts["ng"], counts["not-run"]), file=sys.stderr)
    return 1


sys.exit(main())
PY

main() {
  local from_dir="" have_from=0
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --from-dir)
        [ "${have_from}" = 0 ] || { echo "${USAGE}" >&2; exit 2; }
        [ "$#" -ge 2 ] && [ -n "$2" ] || { echo "${USAGE}" >&2; exit 2; }
        from_dir="$(abspath "$2")"
        have_from=1
        shift 2
        ;;
      *)
        echo "${USAGE}" >&2
        exit 2
        ;;
    esac
  done
  [ "${have_from}" = 1 ] || { echo "${USAGE}" >&2; exit 2; }

  need python3
  need node
  need mktemp
  local node_actual
  node_actual="$(node --version)" || die2 "node --version failed"
  [ "${node_actual}" = "${NODE_EXPECTED}" ] || die2 "node version mismatch: expected ${NODE_EXPECTED}, got ${node_actual}"

  # EXIT の trap から参照するので関数のローカルにしない。
  work=""
  work="$(mktemp -d "${TMPDIR:-/tmp}/wasm-web-load-check.XXXXXX")"
  trap 'rm -rf "${work:?}"' EXIT

  local rc=0
  python3 -I -c "${PY_CODE}" "${from_dir}" "${work}" "${NODE_SCRIPT}" || rc=$?
  exit "${rc}"
}

main "$@"
