#!/usr/bin/env bash
# wasm-size.sh の回帰テスト（spec TASK-22・BUILD-42・T-12 のポインタ。親 Issue #368 の 3/6）。
# TASK-247（親 Issue #467 の 1/4）で、内訳の出力（raw_bytes・file: の行）と --require-exports（export の確かめ）のケースを足した。
# 2/4 で、file: の行の sha256 と、ビルドする経路の tools: の行（道具の版・Cargo.lock のハッシュ）のケースを足した。
# 一時ディレクトリにダミーのバイト列で glue の出力の形を作って --from-dir で渡し、終了コード・標準出力・標準エラーの
# 文言を確かめる。既定の経路（ビルドする経路）は、PATH を一時ディレクトリの偽の cargo / rustc / wasm-bindgen に
# 差し替えて流す。wasm-bindgen の CLI・Node・ネットワーク・docs/spec の取得は要らず、リポジトリのファイルは変更しない。
# 呼び出し元: `make test-wasm-size`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
# 前提: python3（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う（先頭が `!` の語は含まれないことの確認）。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として直す。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

TOOL="${DIR}/wasm-size.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-wasm-size.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成の入力を作る。固定 seed のダミーのバイト列で、PoC の出力などは使わない。
python3 -I - "${work}" <<'PY'
import gzip
import os
import random
import sys

out = sys.argv[1]
SETTINGS = ["release-scalar", "release-simd128", "wasm-size-scalar", "wasm-size-simd128"]
TARGETS = {
    "web": ["wasm_glue.js", "wasm_glue_bg.wasm"],
    "bundler": ["wasm_glue.js", "wasm_glue_bg.js", "wasm_glue_bg.wasm"],
}


def gz(b):
    return len(gzip.compress(b, compresslevel=9, mtime=0))


def blob(seed, n):
    rng = random.Random(seed)
    if hasattr(rng, "randbytes"):
        return rng.randbytes(n)
    return bytes(rng.getrandbits(8) for _ in range(n))


def put(base, setting, target, name, data):
    d = os.path.join(base, setting, target)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, name), "wb") as fh:
        fh.write(data)


def build(name, wasm_n=5000, over=None, wasm_data=None):
    """全 8 組に同じ内容を置く。over は {(setting, target, file): bytes | None}。wasm_data があれば全組の _bg.wasm をそれにする。"""
    base = os.path.join(out, name)
    os.makedirs(base)
    for s in SETTINGS:
        for t, files in TARGETS.items():
            for f in files:
                if f.endswith("_bg.wasm"):
                    data = wasm_data if wasm_data is not None else blob(1, wasm_n)
                elif f.endswith("_bg.js"):
                    data = blob(2, 2000)
                else:
                    data = blob(3, 3000)
                put(base, s, t, f, data)
    for (s, t, f), data in (over or {}).items():
        p = os.path.join(base, s, t, f)
        if data is None:
            os.remove(p)
        else:
            with open(p, "wb") as fh:
                fh.write(data)
    return base


def write(name, text):
    with open(os.path.join(out, name), "w") as fh:
        fh.write(text)


build("all_ok")
write("web_total", str(gz(blob(3, 3000)) + gz(blob(1, 5000))))
write("bundler_total", str(gz(blob(3, 3000)) + gz(blob(2, 2000)) + gz(blob(1, 5000))))
write("gz_js", str(gz(blob(3, 3000))))
write("gz_bgjs", str(gz(blob(2, 2000))))
write("gz_wasm", str(gz(blob(1, 5000))))
import hashlib
write("sha_js", hashlib.sha256(blob(3, 3000)).hexdigest())
write("sha_bgjs", hashlib.sha256(blob(2, 2000)).hexdigest())
write("sha_wasm", hashlib.sha256(blob(1, 5000)).hexdigest())

# 境界: release-scalar/web の合計をちょうど 60000 / 60001 にする。js は固定、wasm の長さを探す。
js = blob(3, 3000)


def find(target_total):
    for n in range(40000, 80000):
        w = blob(9, n)
        if gz(js) + gz(w) == target_total:
            return w
    raise SystemExit("cannot construct a boundary input for %d" % target_total)


for name, total in (("boundary_eq", 60000), ("boundary_over", 60001)):
    build(name, over={("release-scalar", "web", "wasm_glue_bg.wasm"): find(total)})

build("one_ng", over={("wasm-size-simd128", "bundler", "wasm_glue_bg.wasm"): blob(9, 70000)})

REQ = ["gemm_scalar", "gemm_simd", "topk_scalar", "topk_simd", "elementwise_scalar", "elementwise_simd", "detect_isa_bits"]


def leb(n):
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def export_section(entries, raw_names=None):
    """entries は (名前, kind) の並び。raw_names があれば名前をそのバイト列で置く。"""
    payload = leb(len(entries))
    for i, (name, kind) in enumerate(entries):
        nb = raw_names[i] if raw_names else name.encode()
        payload += leb(len(nb)) + nb + bytes([kind]) + leb(0)
    return b"\x07" + leb(len(payload)) + payload


HEADER = b"\x00asm\x01\x00\x00\x00"
TYPE_SEC = b"\x01\x01\x00"


def wasm(entries, raw_names=None):
    return HEADER + TYPE_SEC + export_section(entries, raw_names)


FULL = [(n, 0) for n in REQ] + [("roundtrip_f32", 0), ("memory", 2), ("__wbindgen_start", 0)]
with open(os.path.join(out, "good.wasm"), "wb") as fh:
    fh.write(wasm(FULL))
with open(os.path.join(out, "only_rt.wasm"), "wb") as fh:
    fh.write(wasm([("roundtrip_f32", 0)]))

build("ex_ok", wasm_data=wasm(FULL))
build("ex_one_missing", wasm_data=wasm(FULL),
      over={("release-scalar", "web", "wasm_glue_bg.wasm"): wasm(FULL[1:])})
build("ex_only_rt", wasm_data=wasm([("roundtrip_f32", 0)]))
build("ex_not_func", wasm_data=wasm([(n, 3) for n in REQ]))
build("ex_no_section", wasm_data=HEADER + TYPE_SEC)
# 期待の 7 名を 2 つの export セクションへ分けた入力（重複セクションは不正）
_half = [(n, 0) for n in REQ[:3]]
_rest = [(n, 0) for n in REQ[3:]] + [("roundtrip_f32", 0)]
bad = {
    "bad_dup_export": HEADER + TYPE_SEC + export_section(_half) + export_section(_rest),
    "bad_magic": b"\x00asn\x01\x00\x00\x00" + TYPE_SEC,
    "bad_version": b"\x00asm\x02\x00\x00\x00" + TYPE_SEC,
    "bad_length": HEADER + b"\x01\x7f\x00",
    "bad_leb": HEADER + b"\x01\x80\x80\x80\x80\x80\x00",
    "bad_utf8": wasm([("x", 0)], raw_names=[b"\xff\xfe"]),
    "bad_trailing": HEADER + b"\x07\x03\x00\x00\x00",
}
for k, v in bad.items():
    build(k, over={("release-scalar", "web", "wasm_glue_bg.wasm"): v}, wasm_data=wasm(FULL))
b = build("ex_unexpected", wasm_data=wasm(FULL))
os.makedirs(os.path.join(b, "wasm-size-scalar", "bundler", "snippets"))
with open(os.path.join(b, "wasm-size-scalar", "bundler", "snippets", "x.js"), "wb") as fh:
    fh.write(blob(7, 100))
b = build("ex_dts", wasm_data=wasm(FULL))
for s_ in SETTINGS:
    for t in TARGETS:
        put(b, s_, t, "wasm_glue.d.ts", blob(5, 100))
        put(b, s_, t, "wasm_glue_bg.wasm.d.ts", blob(5, 100))
        put(b, s_, t, "package.json", blob(6, 100))
b = build("ex_symlink", wasm_data=wasm(FULL))
os.symlink(os.path.join(b, "release-scalar", "web", "wasm_glue.js"), os.path.join(b, "release-scalar", "web", "link.txt"))

import shutil
b = build("missing_dir")
shutil.rmtree(os.path.join(b, "release-simd128", "bundler"))
build("missing_file", over={("wasm-size-scalar", "bundler", "wasm_glue_bg.js"): None})
build("empty_file", over={("release-scalar", "web", "wasm_glue_bg.wasm"): b""})
b = build("extra_files")
for s in SETTINGS:
    for t in TARGETS:
        put(b, s, t, "wasm_glue.d.ts", blob(5, 200000))
        put(b, s, t, "package.json", blob(6, 200000))
b = build("symlink")
p = os.path.join(b, "release-scalar", "web", "wasm_glue.js")
os.remove(p)
os.symlink(os.path.join(b, "release-scalar", "web", "wasm_glue_bg.wasm"), p)
b = build("unknown_entry")
os.makedirs(os.path.join(b, "debug-scalar"))
b = build("setting_symlink")
ext = os.path.join(os.path.dirname(b), "setting_symlink_ext")
os.rename(os.path.join(b, "release-scalar"), ext)
os.symlink(ext, os.path.join(b, "release-scalar"))
b = build("dir_as_file")
p = os.path.join(b, "release-scalar", "web", "wasm_glue.js")
os.remove(p)
os.makedirs(p)
PY

# 偽のコマンドの置き場を作る。PATH はここだけにし、実物の wasm-bindgen / cargo が見えないようにする。
mk_path() {
  local dir="$1" t
  mkdir -p "${dir}"
  for t in bash env dirname basename mkdir python3 cp; do
    ln -s "$(command -v "${t}")" "${dir}/${t}"
  done
}

# 偽の cargo: metadata は JSON を返し、build は引数と CARGO_ENCODED_RUSTFLAGS を記録して .wasm を置く。
# FAKE_CARGO_FAIL_PROFILE と FAKE_CARGO_FAIL_ISA が両方一致したビルドは失敗させる。
mk_cargo() {
  local dir="$1"
  cat >"${dir}/cargo" <<'EOF'
#!/usr/bin/env bash
echo "cargo $*" >>"${FAKE_LOG}"
if [ "$1" = "metadata" ]; then
  printf '{"target_directory": "%s"}\n' "${FAKE_TARGET_DIR}"
  exit 0
fi
profile="" tdir=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --profile) profile="$2"; shift ;;
    --target-dir) tdir="$2"; shift ;;
  esac
  shift
done
echo "flags profile=${profile} encoded=${CARGO_ENCODED_RUSTFLAGS-unset} rustflags=${RUSTFLAGS-unset}" >>"${FAKE_LOG}"
isa="$(basename "${tdir}")"
if [ "${profile}" = "${FAKE_CARGO_FAIL_PROFILE:-}" ] && [ "${isa}" = "${FAKE_CARGO_FAIL_ISA:-}" ]; then
  exit 1
fi
mkdir -p "${tdir}/wasm32-unknown-unknown/${profile}"
echo dummy >"${tdir}/wasm32-unknown-unknown/${profile}/wasm_glue.wasm"
EOF
  cat >"${dir}/rustc" <<'EOF'
#!/usr/bin/env bash
echo "rustc $*" >>"${FAKE_LOG}"
case "$1" in
  -V|--version) printf '%s\n' "${FAKE_RUSTC_VERSION-rustc 1.0.0 (0000000 2000-01-01)}" ;;
  *) echo "${FAKE_SYSROOT}" ;;
esac
EOF
  chmod +x "${dir}/cargo" "${dir}/rustc"
}

mk_bindgen() {
  local dir="$1" version="$2"
  cat >"${dir}/wasm-bindgen" <<EOF
#!/usr/bin/env bash
echo "wasm-bindgen \$*" >>"\${FAKE_LOG}"
if [ "\$1" = "--version" ]; then
  echo "${version}"
  exit 0
fi
target="" out=""
while [ "\$#" -gt 1 ]; do
  case "\$1" in
    --target) target="\$2"; shift ;;
    --out-dir) out="\$2"; shift ;;
  esac
  shift
done
echo js >"\${out}/wasm_glue.js"
echo wasm >"\${out}/wasm_glue_bg.wasm"
if [ -n "\${FAKE_BG_WASM:-}" ]; then cp "\${FAKE_BG_WASM}" "\${out}/wasm_glue_bg.wasm"; fi
if [ "\${target}" = "bundler" ]; then echo bgjs >"\${out}/wasm_glue_bg.js"; fi
EOF
  chmod +x "${dir}/wasm-bindgen"
}

# run_case <name> <args...>: ツールを流し、終了コードを rc に、出力を OUT_F / ERR_F に残す。
run_case() {
  rc=0
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

count_lines() {
  grep -c -F -e "$1" "${OUT_F}" || true
}

expect_count() {
  local name="$1" word="$2" want="$3" got
  got="$(count_lines "${word}")"
  if [ "${got}" = "${want}" ]; then
    passed=$((passed + 1))
  else
    echo "FAIL ${name}: '${word}' appears ${got} time(s), expected ${want}" >&2
    failed=$((failed + 1))
  fi
}

web_total="$(cat "${work}/web_total")"
bundler_total="$(cat "${work}/bundler_total")"

# 1. 8 組とも小さなダミー -> 0
run_case --from-dir "${work}/all_ok"
expect "all-ok" 0 "record: combos=8 ok=8 ng=0 not_run=0 limit=60000" "ok:" "!FAILED" "!error:" \
  "combo=release-scalar target=web label=ok gzip_bytes=${web_total} files=2 limit=60000" \
  "combo=wasm-size-simd128 target=bundler label=ok gzip_bytes=${bundler_total} files=3 limit=60000"
expect_count "all-ok-lines" "label=ok" 8

# 1b. 内訳: raw_bytes と file: の行（TASK-247）。数え方は gzip level 9・mtime 0 のファイルごと
gz_js="$(cat "${work}/gz_js")"
gz_bgjs="$(cat "${work}/gz_bgjs")"
gz_wasm="$(cat "${work}/gz_wasm")"
run_case --from-dir "${work}/all_ok"
expect "raw-and-file-lines" 0 \
  "combo=release-scalar target=web label=ok gzip_bytes=${web_total} files=2 limit=60000 raw_bytes=8000" \
  "combo=wasm-size-simd128 target=bundler label=ok gzip_bytes=${bundler_total} files=3 limit=60000 raw_bytes=10000" \
  "file: combo=release-scalar target=web name=wasm_glue.js raw_bytes=3000 gzip_bytes=${gz_js}" \
  "file: combo=release-scalar target=web name=wasm_glue_bg.wasm raw_bytes=5000 gzip_bytes=${gz_wasm}" \
  "file: combo=release-scalar target=bundler name=wasm_glue_bg.js raw_bytes=2000 gzip_bytes=${gz_bgjs}" \
  "!exports="
expect_count "file-lines" "file: combo=" 20

# 1c. file: の行の sha256（TASK-247 の 2/4）。数えたバイト列そのもののハッシュ
sha_js="$(cat "${work}/sha_js")"
sha_bgjs="$(cat "${work}/sha_bgjs")"
sha_wasm="$(cat "${work}/sha_wasm")"
run_case --from-dir "${work}/all_ok"
expect "file-sha256" 0 \
  "name=wasm_glue.js raw_bytes=3000 gzip_bytes=${gz_js} sha256=${sha_js}" \
  "name=wasm_glue_bg.js raw_bytes=2000 gzip_bytes=${gz_bgjs} sha256=${sha_bgjs}" \
  "name=wasm_glue_bg.wasm raw_bytes=5000 gzip_bytes=${gz_wasm} sha256=${sha_wasm}" \
  "!tools:"
expect_count "file-sha256-count" " sha256=" 20
run_case --from-dir "${work}/missing_file"
expect "not-run-no-sha256" 1 "label=not-run reason=missing-file" "!combo=wasm-size-scalar target=bundler name="
expect_count "not-run-file-lines" "file: combo=wasm-size-scalar target=bundler" 0

# 2. 境界: ちょうど 60000 は ok、60001 は ng
run_case --from-dir "${work}/boundary_eq"
expect "boundary-eq" 0 "combo=release-scalar target=web label=ok gzip_bytes=60000 files=2" "ng=0"
run_case --from-dir "${work}/boundary_over"
expect "boundary-over" 1 "combo=release-scalar target=web label=ng gzip_bytes=60001 files=2" "ng=1" "FAILED:"

# 3. 1 組だけ閾値超え -> 1
run_case --from-dir "${work}/one_ng"
expect "one-ng" 1 "combo=wasm-size-simd128 target=bundler label=ng" "record: combos=8 ok=7 ng=1 not_run=0" "FAILED:" "!ok:"
expect_count "one-ng-lines" "label=ng" 1

# 4. 組のディレクトリが欠ける -> 1、残りは測られる
run_case --from-dir "${work}/missing_dir"
expect "missing-dir" 1 "combo=release-simd128 target=bundler label=not-run reason=missing-directory" \
  "record: combos=8 ok=7 ng=0 not_run=1" "FAILED:" "!gzip_bytes=0" "!ok:"
expect_count "missing-dir-measured" "label=ok" 7

# 5. 数えるファイルの欠落・空
run_case --from-dir "${work}/missing_file"
expect "missing-file" 1 "combo=wasm-size-scalar target=bundler label=not-run reason=missing-file" "not_run=1"
run_case --from-dir "${work}/empty_file"
expect "empty-file" 1 "combo=release-scalar target=web label=not-run reason=empty-file" "not_run=1" "!gzip_bytes=0"

# 6. .d.ts・package.json は数えない
run_case --from-dir "${work}/extra_files"
expect "extra-files" 0 "combo=release-scalar target=web label=ok gzip_bytes=${web_total} files=2" "ok=8"

# 7. 読めない入力 -> 2、record は出ない
run_case --from-dir "${work}/symlink"
expect "symlink" 2 "error: release-scalar/web/wasm_glue.js is not a regular file" "!record:" "!ok:"
run_case --from-dir "${work}/setting_symlink"
expect "setting-symlink" 2 "error: release-scalar is not a directory" "!record:" "!ok:"
run_case --from-dir "${work}/dir_as_file"
expect "dir-as-file" 2 "error:" "!record:"
run_case --from-dir "${work}/unknown_entry"
expect "unknown-entry" 2 "error: unexpected entry in --from-dir" "!record:"

# 7b. --require-exports（TASK-247）
run_case --require-exports --from-dir "${work}/ex_ok"
expect "exports-all-present" 0 "record: combos=8 ok=8 ng=0 not_run=0" "ok:" "!FAILED" "!error:" \
  "combo=release-scalar target=web label=ok gzip_bytes=" "exports=7/7"
expect_count "exports-all-present-lines" "exports=7/7" 8
run_case --from-dir "${work}/ex_ok" --require-exports
expect "exports-flag-after" 0 "ok=8" "exports=7/7"
run_case --require-exports --from-dir "${work}/ex_one_missing"
expect "exports-one-missing" 1 "combo=release-scalar target=web label=not-run reason=missing-export" "exports=6/7" \
  "record: combos=8 ok=7 ng=0 not_run=1" "FAILED:" "!ok:"
run_case --require-exports --from-dir "${work}/ex_only_rt"
expect "exports-only-roundtrip" 1 "record: combos=8 ok=0 ng=0 not_run=8" "reason=missing-export" "exports=0/7" "FAILED:" "!gzip_bytes="
expect_count "exports-only-roundtrip-lines" "reason=missing-export" 8
run_case --require-exports --from-dir "${work}/ex_not_func"
expect "exports-not-function" 1 "not_run=8" "reason=missing-export" "exports=0/7"
run_case --require-exports --from-dir "${work}/ex_no_section"
expect "exports-no-section" 1 "not_run=8" "reason=missing-export" "exports=0/7"
# 既定の経路は export を見ない
run_case --from-dir "${work}/ex_only_rt"
expect "exports-ignored-by-default" 0 "ok=8" "!exports="

for k in bad_magic bad_version bad_length bad_leb bad_utf8 bad_trailing bad_dup_export; do
  run_case --require-exports --from-dir "${work}/${k}"
  expect "wasm-unparseable(${k})" 2 "error: cannot parse release-scalar/web/wasm_glue_bg.wasm as wasm" "!record:" "!ok:"
  run_case --from-dir "${work}/${k}"
  expect "wasm-unparseable-default-ignored(${k})" 0 "ok=8"
done

run_case --require-exports --from-dir "${work}/ex_unexpected"
expect "unexpected-output" 1 "combo=wasm-size-scalar target=bundler label=not-run reason=unexpected-output" \
  "record: combos=8 ok=7 ng=0 not_run=1" "FAILED:"
run_case --from-dir "${work}/ex_unexpected"
expect "unexpected-output-default-ignored" 0 "ok=8" "!unexpected-output"
run_case --require-exports --from-dir "${work}/ex_dts"
expect "dts-still-ignored" 0 "ok=8" "exports=7/7" "!unexpected-output"
run_case --require-exports --from-dir "${work}/ex_symlink"
expect "exports-symlink-entry" 2 "error: release-scalar/web/link.txt is not a regular file" "!record:"

run_case --require-exports=x --from-dir "${work}/ex_ok"
expect "require-exports-with-value" 2 "!record:"
run_case --require-exports --require-exports --from-dir "${work}/ex_ok"
expect "require-exports-twice" 2 "error: invalid arguments" "!record:"

# 8. 誤用 -> 2
run_case --bogus
expect "unknown-arg" 2 "!record:"
run_case --from-dir
expect "from-dir-missing-value" 2 "!record:"
run_case --from-dir "${work}/no-such-dir"
expect "no-such-dir" 2 "error: --from-dir does not exist" "!record:"
run_case --from-dir "${work}/web_total"
expect "from-dir-is-file" 2 "error: --from-dir is not a directory" "!record:"
run_case --from-dir ""
expect "from-dir-empty" 2 "error: --from-dir is empty" "!record:"
run_case "${work}/all_ok"
expect "positional" 2 "!record:"

# 9. 既定の経路（偽のコマンド）
fake="${work}/fake"
mk_path "${fake}/bin"
mk_cargo "${fake}/bin"
mkdir -p "${fake}/target" "${fake}/sysroot/lib/rustlib/wasm32-unknown-unknown"
FAKE_LOG="${fake}/log.txt"
export FAKE_LOG
FAKE_TARGET_DIR="${fake}/target"
export FAKE_TARGET_DIR
FAKE_SYSROOT="${fake}/sysroot"
export FAKE_SYSROOT

run_fake() {
  : >"${FAKE_LOG}"
  rc=0
  PATH="${fake}/bin" "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
}

# 9a. wasm-bindgen が無い
if PATH="${fake}/bin" command -v wasm-bindgen >/dev/null 2>&1; then
  echo "FAIL fake-path: wasm-bindgen is visible" >&2
  failed=$((failed + 1))
fi
run_fake
expect "no-bindgen" 2 "error: required command not found on PATH: wasm-bindgen" "!record:"
if grep -q "^cargo " "${FAKE_LOG}"; then
  echo "FAIL no-bindgen: cargo was called" >&2
  failed=$((failed + 1))
else
  passed=$((passed + 1))
fi

# 9b. 版違い・読めない出力 -> 2、cargo は呼ばれない
for v in "wasm-bindgen 0.2.127" "wasm-bindgen 0.2.1280" "garbage" ""; do
  mk_bindgen "${fake}/bin" "${v}"
  run_fake
  expect "bad-version(${v})" 2 "error:" "!record:"
  if grep -q "^cargo " "${FAKE_LOG}"; then
    echo "FAIL bad-version(${v}): cargo was called" >&2
    failed=$((failed + 1))
  else
    passed=$((passed + 1))
  fi
done

# 9c. target 未導入 -> 2
mk_bindgen "${fake}/bin" "wasm-bindgen 0.2.128"
FAKE_SYSROOT="${fake}/empty-sysroot"
mkdir -p "${FAKE_SYSROOT}"
run_fake
expect "no-target" 2 "error: target wasm32-unknown-unknown is not installed" "!record:"
FAKE_SYSROOT="${fake}/sysroot"

# 9d. 8 組とも組み立てられる -> 0。flags が組ごとに正しく渡る。利用者の RUSTFLAGS は子に渡らない
RUSTFLAGS="-C debuginfo=2" run_fake
expect "fake-all" 0 "record: combos=8 ok=8 ng=0 not_run=0" "ok:"
if grep -q "profile=release encoded= rustflags=unset" "${FAKE_LOG}" \
  && grep -q "profile=wasm-size encoded= rustflags=unset" "${FAKE_LOG}" \
  && [ "$(grep -c "encoded=-C.*target-feature=+simd128 rustflags=unset" "${FAKE_LOG}")" = "2" ] \
  && [ "$(grep -c -e '--locked --offline' "${FAKE_LOG}")" -ge 4 ]; then
  passed=$((passed + 1))
else
  echo "FAIL fake-flags: flags log unexpected" >&2
  sed 's/^/  log: /' "${FAKE_LOG}" >&2
  failed=$((failed + 1))
fi
if [ "$(grep -c '^wasm-bindgen --target' "${FAKE_LOG}")" = "8" ]; then
  passed=$((passed + 1))
else
  echo "FAIL fake-bindgen-calls: expected 8 wasm-bindgen runs" >&2
  failed=$((failed + 1))
fi

# 9d2. tools: の行（道具の版と wasm/Cargo.lock のハッシュ）
lock_sha="$(python3 -I -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "${DIR}/../wasm/Cargo.lock")"
run_fake
expect "fake-tools-line" 0 \
  "tools: wasm_bindgen_version=0.2.128 rustc_version=1.0.0 cargo_lock_sha256=${lock_sha}"
expect_count "fake-tools-line-count" "tools: " 1

# 9d3. rustc の版が読めない -> 2、cargo build は呼ばれない
for v in "" "garbage" "rustc x.y"; do
  FAKE_RUSTC_VERSION="${v}" run_fake
  expect "rustc-bad-version(${v})" 2 "error: cannot read the rustc version" "!record:" "!tools:"
  if grep -q "^cargo " "${FAKE_LOG}"; then
    echo "FAIL rustc-bad-version(${v}): cargo was called" >&2
    failed=$((failed + 1))
  else
    passed=$((passed + 1))
  fi
done

# 9e. 1 設定の cargo が失敗 -> 1、該当の 2 組が not-run build-failed
FAKE_CARGO_FAIL_PROFILE=wasm-size FAKE_CARGO_FAIL_ISA=simd128 run_fake
expect "fake-build-failed" 1 \
  "combo=wasm-size-simd128 target=web label=not-run reason=build-failed" \
  "combo=wasm-size-simd128 target=bundler label=not-run reason=build-failed" \
  "record: combos=8 ok=6 ng=0 not_run=2" "FAILED:"

# 9f. --require-exports の既定の経路（偽の wasm-bindgen が合成の wasm を _bg.wasm に写す）
FAKE_BG_WASM="${work}/good.wasm" run_fake --require-exports
expect "fake-require-exports-ok" 0 "record: combos=8 ok=8 ng=0 not_run=0" "exports=7/7"
FAKE_BG_WASM="${work}/only_rt.wasm" run_fake --require-exports
expect "fake-require-exports-missing" 1 "record: combos=8 ok=0 ng=0 not_run=8" "reason=missing-export"
FAKE_BG_WASM="${work}/only_rt.wasm" run_fake
expect "fake-default-no-export-check" 0 "ok=8" "!exports="

echo "test-wasm-size: ${passed} passed, ${failed} failed" >&2
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
