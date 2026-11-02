#!/usr/bin/env bash
# wasm-bindgen の `--target web` の出力を Node の ESM で読み込み、7 名の export の有無と detect_isa_bits の戻り値を
# 4 組 × 11 項目の行で出す検査の本体（spec TASK-248 のポインタ。項目 BUILD-43）。
# 呼び出し元: 今は直接実行（回帰テストは scripts/test-wasm-web-load-check.sh）。
#
# 使い方:
#   scripts/wasm-web-load-check.sh                  wasm/ の glue を 4 組でビルドし、wasm-bindgen の CLI で web の出力を作って判定する
#   scripts/wasm-web-load-check.sh --from-dir <dir> 生成済みの出力を読む（1 回だけ）。置き方は `<dir>/<組>/web/wasm_glue.js` と `wasm_glue_bg.wasm`。
#               組の綴りと置き方は scripts/wasm-size.sh の `--from-dir` に揃える（TASK-247 のポインタ）。cargo も CLI も呼ばない
#
# ビルドの経路の前提（外れたら cargo を呼ぶ前に終了コード 2。CLI の入手は試みない）: python3 3.8 以上・cargo・rustc・
#   wasm32-unknown-unknown の target・依存が手元にある（`make wasm-verify` で取得。ビルドは --locked --offline）・
#   PATH の wasm-bindgen が wasm/crates/glue/Cargo.toml の版と厳密に一致・node の版。
# ビルド: 組ごとに `cargo build --manifest-path wasm/Cargo.toml -p wasm-glue --target wasm32-unknown-unknown --profile <release|wasm-size>
#   --locked --offline`、続けて `wasm-bindgen --target web --out-dir <一時>/out/<組>/web <wasm>`。成果物は cargo の target_directory
#   配下の専用ディレクトリ（glue-web-load）に出し、製品のビルド・wasm-size.sh と共有しない。RUSTFLAGS は子に渡さず、
#   simd128 の組だけ CARGO_ENCODED_RUSTFLAGS で命令の指定を渡す。ビルドか生成に失敗した組は not-run
#   （reason: build-failed / bindgen-failed）にし、合格扱いにしない。
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
USAGE="usage: $(basename "$0") [--from-dir <dir>]"
GLUE_MANIFEST="${ROOT_DIR}/wasm/crates/glue/Cargo.toml"

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

# 検査と判定（固定の文字列）。引数: from-dir <from-dir> <work> <node-script> / build <repo-root> <work> <node-script> <glue-manifest>。
read -r -d '' PY_CODE <<'PY' || true
import json
import os
import re
import shutil
import stat
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python3 3.8 or newer is required", file=sys.stderr)
    sys.exit(2)

# 組の綴り（宣言点はここ 1 か所）。出力先は web だけ。
COMBOS = ("release-scalar", "release-simd128", "wasm-size-scalar", "wasm-size-simd128")
TARGET_DIR = "web"
WASM_TARGET = "wasm32-unknown-unknown"
GLUE_PACKAGE = "wasm-glue"
STEM = "wasm_glue"
# 専用のディレクトリの名前（wasm-size.sh の glue-size と別）。
WORK_SUBDIR = "glue-web-load"
# 組 → (profile, CARGO_ENCODED_RUSTFLAGS)。
BUILD_SETS = (
    ("release-scalar", "release", ""),
    ("release-simd128", "release", "-C\x1ftarget-feature=+simd128"),
    ("wasm-size-scalar", "wasm-size", ""),
    ("wasm-size-simd128", "wasm-size", "-C\x1ftarget-feature=+simd128"),
)
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


def judge(root, work, node_script, overrides):
    # overrides: {組: 理由}。入力の検査より先に not-run にし、node を流さない。
    st = lstat_or_none(root)
    if st is None or stat.S_ISLNK(st.st_mode) or not stat.S_ISDIR(st.st_mode):
        die("--from-dir must be an existing plain directory")
    # 入力を全部検査してから node を流す。
    inputs = [
        (combo, None, overrides[combo]) if combo in overrides else (combo,) + check_input(root, combo)
        for combo in COMBOS
    ]
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


def run(cmd, cwd, env=None):
    # 子プロセスは引数の並び（shell=False）で呼び、出力は捨てる（ホストのパスを出さないため）。
    try:
        return subprocess.run(
            cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
    except OSError:
        die("cannot run a command")


def text(r):
    return r.stdout.decode("utf-8", "replace").strip()


def glue_bindgen_version(manifest):
    try:
        with open(manifest, "r", encoding="utf-8") as f:
            lines = f.read().splitlines()
    except (OSError, UnicodeDecodeError):
        die("cannot read the glue manifest")
    found = []
    for ln in lines:
        m = re.match(r'^wasm-bindgen = "=(\d+\.\d+\.\d+)"$', ln.strip())
        if m:
            found.append(m.group(1))
    if len(found) != 1:
        die("wasm-bindgen version is not exactly once in the glue manifest")
    return found[0]


def check_prerequisites(root, manifest):
    # cargo build の前に済ませる。戻り値は cargo の target_directory。
    for tool in ("cargo", "rustc", "wasm-bindgen"):
        if shutil.which(tool) is None:
            die("required command not found on PATH: " + tool)
    want = glue_bindgen_version(manifest)
    first = (text(run(["wasm-bindgen", "--version"], root)).splitlines() or [""])[0]
    if first != "wasm-bindgen " + want:
        die("wasm-bindgen version does not match the glue manifest")
    r = run(["rustc", "--print", "sysroot"], root)
    if r.returncode != 0 or not os.path.isdir(os.path.join(text(r), "lib", "rustlib", WASM_TARGET)):
        die("target %s is not installed" % WASM_TARGET)
    meta = ["cargo", "metadata", "--format-version", "1", "--locked", "--offline"]
    meta += ["--filter-platform", WASM_TARGET, "--manifest-path", "wasm/Cargo.toml"]
    r = run(meta, root)
    if r.returncode != 0:
        die("cargo metadata failed (run `make wasm-verify` first to fetch dependencies)")
    try:
        tdir = json.loads(text(r))["target_directory"]
    except (ValueError, KeyError, TypeError):
        die("cannot read target_directory from cargo metadata")
    if not isinstance(tdir, str) or not os.path.isabs(tdir):
        die("malformed target_directory")
    return tdir


def build_and_generate(root, tdir, work):
    # 戻り値は (web の出力のルート, {組: 理由})。消すのは target_directory 直下の専用ディレクトリだけ。
    wdir = os.path.join(tdir, WORK_SUBDIR)
    if os.path.dirname(os.path.abspath(wdir)) != os.path.abspath(tdir) or os.path.basename(wdir) != WORK_SUBDIR:
        die("refusing to clean an unexpected directory")
    if os.path.islink(wdir):
        die("work directory is a symlink")
    if os.path.isdir(wdir):
        shutil.rmtree(wdir)
    out_root = os.path.join(work, "out")
    os.makedirs(out_root)
    failures = {}
    for combo, profile, flags in BUILD_SETS:
        env = dict(os.environ)
        env.pop("RUSTFLAGS", None)
        env["CARGO_ENCODED_RUSTFLAGS"] = flags
        isa = "simd128" if combo.endswith("-simd128") else "scalar"
        target_dir = os.path.join(wdir, "target", isa)
        build = ["cargo", "build", "--manifest-path", "wasm/Cargo.toml", "-p", GLUE_PACKAGE]
        build += ["--target", WASM_TARGET, "--profile", profile, "--locked", "--offline"]
        build += ["--target-dir", target_dir]
        r = run(build, root, env)
        wasm = os.path.join(target_dir, WASM_TARGET, profile, STEM + ".wasm")
        if r.returncode != 0 or not os.path.isfile(wasm):
            failures[combo] = "build-failed"
            continue
        dest = os.path.join(out_root, combo, TARGET_DIR)
        os.makedirs(dest)
        r = run(["wasm-bindgen", "--target", "web", "--out-dir", dest, wasm], root)
        if r.returncode != 0:
            failures[combo] = "bindgen-failed"
    return out_root, failures


def main():
    mode = sys.argv[1]
    try:
        if mode == "from-dir":
            return judge(sys.argv[2], sys.argv[3], sys.argv[4], {})
        root, work, node_script, manifest = sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]
        tdir = check_prerequisites(root, manifest)
        out_root, failures = build_and_generate(root, tdir, work)
        return judge(out_root, work, node_script, failures)
    except OSError:
        # ホストのパスを含む例外の文言は出さず、固定の文言にする。
        die("filesystem or process operation failed")


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
  if [ "${have_from}" = 1 ]; then
    python3 -I -c "${PY_CODE}" from-dir "${from_dir}" "${work}" "${NODE_SCRIPT}" || rc=$?
  else
    python3 -I -c "${PY_CODE}" build "${ROOT_DIR}" "${work}" "${NODE_SCRIPT}" "${GLUE_MANIFEST}" || rc=$?
  fi
  exit "${rc}"
}

main "$@"
