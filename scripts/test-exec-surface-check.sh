#!/usr/bin/env bash
# exec-surface-check.sh の回帰テスト（spec TASK-923 のポインタ。親 Issue #867）。
# 実物の crates/exec の src/・tests/・Cargo.toml を一時ディレクトリへ写し、1 か所だけ字句を足した変異体を
# --crate-dir で走査させて検出を確かめる。陰性の対照・誤用の入力・実物の合格も確かめる。
# リポジトリのファイルは変更しない（実物は読むだけ）・ネットワーク不要。
# 呼び出し元: `make test-exec-surface-check`（直接実行も可）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」（`o:` 標準出力・`e:` 標準エラー・先頭が `!` は含まれないこと）。
# 期待値を緩めない。途中で止めず、最後に件数を出す。bash 3.2 互換。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
require_cmd python3
require_cmd cargo

TOOL="${DIR}/exec-surface-check.sh"
REAL_CRATE="${ROOT_DIR}/crates/exec"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-exec-surface-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# judge <名前> <期待の終了コード> <語>...: 直前の rc/出力と照らす。
judge() {
  local name="$1" want="$2" ok=1 term body
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    ok=0
  fi
  for term in "$@"; do
    case "${term}" in
      o:!*) body="${term#o:!}"; if grep -qF -- "${body}" "${OUT_F}"; then ok=0; fi ;;
      e:!*) body="${term#e:!}"; if grep -qF -- "${body}" "${ERR_F}"; then ok=0; fi ;;
      o:*) body="${term#o:}"; if ! grep -qF -- "${body}" "${OUT_F}"; then ok=0; fi ;;
      e:*) body="${term#e:}"; if ! grep -qF -- "${body}" "${ERR_F}"; then ok=0; fi ;;
      !*) body="${term#!}"; if grep -qF -- "${body}" "${OUT_F}" "${ERR_F}"; then ok=0; fi ;;
      *) if ! grep -qF -- "${term}" "${OUT_F}" "${ERR_F}"; then ok=0; fi ;;
    esac
  done
  if [ "${ok}" -eq 1 ]; then
    passed=$((passed + 1))
    echo "ok: ${name}"
  else
    failed=$((failed + 1))
    echo "NG: ${name} (exit=${rc}, want=${want})"
    sed 's/^/  out| /' "${OUT_F}" | head -n 8
    sed 's/^/  err| /' "${ERR_F}" | head -n 8
  fi
}

# run_tool <引数>...: 本体を流して rc と出力を取る。
run_tool() {
  rc=0
  "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
}

# fresh_copy <名前>: 実物の crate を写した一時 crate のパスを COPY に入れる（target は写さない）。
COPY=""
fresh_copy() {
  COPY="${work}/$1"
  mkdir "${COPY}"
  cp "${REAL_CRATE}/Cargo.toml" "${COPY}/Cargo.toml"
  local d
  for d in src tests benches examples; do
    if [ -d "${REAL_CRATE}/${d}" ]; then
      cp -R "${REAL_CRATE}/${d}" "${COPY}/${d}"
    fi
  done
  if [ -f "${REAL_CRATE}/build.rs" ]; then
    cp "${REAL_CRATE}/build.rs" "${COPY}/build.rs"
  fi
}

# insert_after <ファイル> <アンカー> <足す行>: アンカーを含む最初の行の直後に 1 行足す。見つからなければ 1 を返す。
insert_after() {
  python3 -I -c '
import sys
path, anchor, add = sys.argv[1], sys.argv[2], sys.argv[3]
lines = open(path, encoding="utf-8").read().split("\n")
for i, l in enumerate(lines):
    if anchor in l:
        lines.insert(i + 1, add)
        open(path, "w", encoding="utf-8").write("\n".join(lines))
        sys.exit(0)
sys.exit(1)
' "$1" "$2" "$3"
}


# gen_json <出力> <指定>...: 実物の形を真似た最小の rustdoc の JSON を書く。既定で合格する土台の面（Exec・操作の関数 7 つ・
# Outcome・RouteReport）を持つ。
# 指定: item:<名前>:<種類>[:<crate_id>] / use:<名前> / root:<名前> / private / fv:str / top:array / badcid /
#   noexec / opret:<fn>:<unit|outcome|report|struct_report|badid|stdresult> / variant:<plain|u32> / outcome:tuple /
#   execfn:<名前> / execfn_hidden:<名前> / freefn:<名前> / dropop:<名前> / dupop:<名前> / traitimpl
GEN="${work}/gen_json.py"
cat >"${GEN}" <<'PYEOF'
import json
import sys

OPS = ["f16_fma", "subgroup_sum_u32", "subgroup_sum_f32", "atomic_add",
      "external_interop_shared_memory", "hardware_ray_tracing", "unified_memory_sharing"]
EXEC_OTHER = ["new", "deny_by_default", "default_opt_in", "query_route"]
out = sys.argv[1]
specs = sys.argv[2:]
root_name = "fandhe_silicon_exec"
index = {}
nid = 100
doc = {"format_version": 61, "includes_private": False, "paths": {}, "external_crates": {}}
EXEC, OUTCOME, REPORT, RESULT, CERR, STDRES = 20, 21, 22, 23, 24, 25
noexec = "noexec" in specs
retmode = {}
extra_variants = []
outcome_tuple = False
exec_extra = []
dup = []
free = []
dropped = set()
traitimpl = False
for sp in specs:
    parts = sp.split(":")
    k = parts[0]
    if k == "opret":
        retmode[parts[1]] = parts[2]
    elif k == "variant":
        extra_variants.append(parts[1])
    elif k == "outcome":
        outcome_tuple = True
    elif k in ("execfn", "execfn_hidden"):
        exec_extra.append((parts[1], k == "execfn_hidden"))
    elif k == "freefn":
        free.append(parts[1])
    elif k == "dropop":
        dropped.add(parts[1])
    elif k == "dupop":
        dup.append(parts[1])
    elif k == "traitimpl":
        traitimpl = True


def ty_rp(i, args=None):
    rp = {"path": "x", "id": i}
    if args is not None:
        rp["args"] = {"angle_bracketed": {"args": args, "constraints": []}}
    return {"resolved_path": rp}


def fn_item(i, name, output, vis="public", attrs=None):
    return {"id": i, "crate_id": 0, "name": name, "visibility": vis, "attrs": attrs or [],
            "inner": {"function": {"sig": {"inputs": [], "output": output, "is_c_variadic": False}, "generics": {}, "header": {}, "has_body": True}}}


def result_of(t, rid=RESULT):
    return ty_rp(rid, [{"type": t}, {"type": ty_rp(CERR)}])


def op_output(name):
    m = retmode.get(name, "ok")
    if m == "unit":
        return result_of(None)
    if m == "outcome":
        return ty_rp(OUTCOME)
    if m == "report":
        return result_of(ty_rp(REPORT))
    if m == "struct_report":
        return result_of(ty_rp(30))
    if m == "badid":
        return result_of(ty_rp(OUTCOME), 999)
    if m == "stdresult":
        return result_of(ty_rp(OUTCOME), STDRES)
    return result_of(ty_rp(OUTCOME))


for sp in specs:
    parts = sp.split(":")
    kind = parts[0]
    if kind == "root":
        root_name = parts[1]
    elif kind == "private":
        doc["includes_private"] = True
    elif kind == "fv":
        doc["format_version"] = "61"
    elif kind == "top":
        doc = []
    elif kind == "badcid":
        index[str(nid)] = {"id": nid, "name": "x", "inner": {"function": {}}}
        nid += 1
    elif kind == "item":
        cid = int(parts[3]) if len(parts) > 3 else 0
        index[str(nid)] = {"id": nid, "crate_id": cid, "name": parts[1], "visibility": "public", "inner": {parts[2]: {}}}
        nid += 1
    elif kind == "use":
        index[str(nid)] = {"id": nid, "crate_id": 0, "name": None, "visibility": "public",
                            "inner": {"use": {"name": parts[1], "source": "a::b", "id": None, "is_glob": False}}}
        nid += 1
if isinstance(doc, dict):
    index["0"] = {"id": 0, "crate_id": 0, "name": root_name, "visibility": "public", "inner": {"module": {"items": []}}}
    index["1"] = {"id": 1, "crate_id": 1, "name": "external_thing", "visibility": "public", "inner": {"function": {}}}
    doc["root"] = 0
    # 土台の面。
    index[str(REPORT)] = {"id": REPORT, "crate_id": 0, "name": "RouteReport", "visibility": "public",
                          "inner": {"struct": {"kind": {"plain": {"fields": [], "has_stripped_fields": False}}}}}
    index["30"] = {"id": 30, "crate_id": 0, "name": "Wrapped", "visibility": "public",
                    "inner": {"struct": {"kind": {"plain": {"fields": [31], "has_stripped_fields": False}}}}}
    index["31"] = {"id": 31, "crate_id": 0, "name": "report", "visibility": "public",
                    "inner": {"struct_field": ty_rp(REPORT)}}
    vids = []
    for n, vname in enumerate(["Ran", "Unavailable"]):
        fid = 40 + n
        vid = 42 + n
        index[str(fid)] = {"id": fid, "crate_id": 0, "name": "report", "visibility": "public",
                            "inner": {"struct_field": ty_rp(REPORT)}}
        kind = {"tuple": [fid]} if outcome_tuple else {"struct": {"fields": [fid], "has_stripped_fields": False}}
        index[str(vid)] = {"id": vid, "crate_id": 0, "name": vname, "visibility": "default",
                            "inner": {"variant": {"kind": kind}}}
        vids.append(vid)
    for n, vk in enumerate(extra_variants):
        vid = 50 + n
        if vk == "plain":
            kind = "plain"
        else:
            index[str(60 + n)] = {"id": 60 + n, "crate_id": 0, "name": "0", "visibility": "default",
                                  "inner": {"struct_field": ty_rp(70)}}
            kind = {"tuple": [60 + n]}
        index[str(vid)] = {"id": vid, "crate_id": 0, "name": "Extra%d" % n, "visibility": "default",
                            "inner": {"variant": {"kind": kind}}}
        vids.append(vid)
    index["70"] = {"id": 70, "crate_id": 0, "name": "Other", "visibility": "public", "inner": {"struct": {"kind": "unit"}}}
    index[str(OUTCOME)] = {"id": OUTCOME, "crate_id": 0, "name": "Outcome", "visibility": "public",
                            "inner": {"enum": {"variants": vids}}}
    doc["paths"][str(RESULT)] = {"crate_id": 2, "path": ["core", "result", "Result"], "kind": "enum"}
    doc["paths"][str(STDRES)] = {"crate_id": 2, "path": ["std", "result", "Result"], "kind": "enum"}
    doc["paths"][str(CERR)] = {"crate_id": 3, "path": ["fandhe_silicon_contract", "ContractError"], "kind": "enum"}
    items = []
    fid = 200
    if not noexec:
        index[str(EXEC)] = {"id": EXEC, "crate_id": 0, "name": "Exec", "visibility": "public",
                            "inner": {"struct": {"kind": "unit"}}}
        for n in OPS:
            if n in dropped:
                continue
            index[str(fid)] = fn_item(fid, n, op_output(n))
            items.append(fid)
            fid += 1
        for n in EXEC_OTHER:
            index[str(fid)] = fn_item(fid, n, None)
            items.append(fid)
            fid += 1
        for n, hidden in exec_extra:
            index[str(fid)] = fn_item(fid, n, result_of(None), attrs=(["#[doc(hidden)]"] if hidden else []))
            items.append(fid)
            fid += 1
        dup_items = []
        for n in dup:
            index[str(fid)] = fn_item(fid, n, op_output(n))
            dup_items.append(fid)
            fid += 1
        index["80"] = {"id": 80, "crate_id": 0, "name": None, "visibility": "default",
                        "inner": {"impl": {"trait": None, "for": ty_rp(EXEC), "items": items}}}
        if dup_items:
            index["81"] = {"id": 81, "crate_id": 0, "name": None, "visibility": "default",
                            "inner": {"impl": {"trait": None, "for": ty_rp(EXEC), "items": dup_items}}}
        if traitimpl:
            tis = []
            for n in ("fmt", "clone"):
                index[str(fid)] = fn_item(fid, n, None, vis="default")
                tis.append(fid)
                fid += 1
            index["82"] = {"id": 82, "crate_id": 0, "name": None, "visibility": "default",
                            "inner": {"impl": {"trait": {"id": 9, "path": "Clone"}, "for": ty_rp(EXEC), "items": tis}}}
    index[str(fid)] = fn_item(fid, "report", None)
    index["83"] = {"id": 83, "crate_id": 0, "name": None, "visibility": "default",
                    "inner": {"impl": {"trait": None, "for": ty_rp(OUTCOME), "items": [fid]}}}
    fid += 1
    for n in free:
        index[str(fid)] = fn_item(fid, n, None)
        fid += 1
    doc["index"] = index
with open(out, "w", encoding="utf-8") as fh:
    json.dump(doc, fh)
PYEOF
gen_json() {
  local out="$1"
  shift
  python3 -I "${GEN}" "${out}" "$@"
}
J_DEF="${work}/j_default.json"
J_ALL="${work}/j_all.json"
gen_json "${J_DEF}" item:other_item:struct
gen_json "${J_ALL}" item:other_item:struct item:force_unsupported:function item:inject_denormal:function \
  item:ForcedUnsupported:variant item:DenormalInjection:enum item:Injected:variant
SYN=(--json-default "${J_DEF}" --json-all "${J_ALL}")

# ---- 実物の合格 ----
run_tool
judge "real crate passes" 0 "o:ok: exec surface check passed" "o:record: commit=" "o:env_fs=0" "o:scanned: features=default" "o:scanned: features=all-features" "o:port_names_default=0" "o:port_names_missing_all=0" "o:unlisted_fns=0" "o:scanned: file=src/lib.rs" "o:scanned: file=tests/common/mod.rs" "e:!violation" "!error:" "!${ROOT_DIR}"

# ---- 変異体（終了コード 1）----
# 行: 名前|語|対象ファイル|アンカー|足す行。対象ファイルが実物に無ければ新しいファイル（アンカーは -）。
# 環境変数とファイルの読み取りの群（FB-13）
mutants=(
  'std::env literal|std::env|src/lib.rs|#![forbid(unsafe_code)]|fn a() { let _ = std::env::args(); }'
  'env::var literal|env::var|src/lib.rs|#![forbid(unsafe_code)]|fn a() { let _ = env::var("X"); }'
  'option_env literal|option_env!|src/lib.rs|#![forbid(unsafe_code)]|const A: Option<&str> = option_env!("X");'
  'env! literal|env!|src/lib.rs|#![forbid(unsafe_code)]|const A: &str = env!("X");'
  'std::fs literal|std::fs|src/lib.rs|#![forbid(unsafe_code)]|fn a() { let _ = std::fs::metadata("x"); }'
  'File::open literal|File::open|src/lib.rs|#![forbid(unsafe_code)]|fn a() { let _ = File::open("x"); }'
  'include_str literal|include_str!|src/lib.rs|#![forbid(unsafe_code)]|const S: &str = include_str!("x.txt");'
  'include_bytes literal|include_bytes!|src/lib.rs|#![forbid(unsafe_code)]|const B: &[u8] = include_bytes!("x.bin");'
  'include literal|include!|src/lib.rs|#![forbid(unsafe_code)]|include!("x.rs");'
  'use std brace env|std::{env}|src/lib.rs|#![forbid(unsafe_code)]|use std::{env};'
  'use std brace fs|std::{fs}|src/lib.rs|#![forbid(unsafe_code)]|use std::{io, fs::File};'
  'use std nested env|std::{env}|src/lib.rs|#![forbid(unsafe_code)]|use std::{collections::{HashMap}, env::var};'
  'pub use std brace|std::{fs}|src/lib.rs|#![forbid(unsafe_code)]|pub use ::std::{ fs };'
  'literal in line comment|std::env|src/lib.rs|#![forbid(unsafe_code)]|// std::env'
  'literal in doc comment|std::fs|src/lib.rs|#![forbid(unsafe_code)]|/// std::fs'
  'literal in string|File::open|src/lib.rs|#![forbid(unsafe_code)]|const S: &str = "File::open";'
  'literal in cfg(test) mod|env!|src/lib.rs|#![forbid(unsafe_code)]|#[cfg(test)] mod t { const A: &str = env!("X"); }'
  'existing tests file|std::env|tests/route_query.rs|mod common;|fn a() { let _ = std::env::args(); }'
  'extra file without mod|std::env|src/extra.rs|-|fn a() { let _ = std::env::args(); }'
  'build.rs|env::var|build.rs|-|fn main() { let _ = std::env::var("X"); }'
)
for row in "${mutants[@]}"; do
  IFS='|' read -r name word file anchor add <<<"${row}"
  fresh_copy "m"
  if [ "${anchor}" = "-" ]; then
    printf '%s\n' "${add}" >"${COPY}/${file}"
  elif ! insert_after "${COPY}/${file}" "${anchor}" "${add}"; then
    failed=$((failed + 1))
    echo "NG: mutant ${name} (anchor not found)"
    rm -rf "${COPY}"
    continue
  fi
  run_tool --crate-dir "${COPY}" "${SYN[@]}"
  yes_build="o:build_rs=no"
  if [ "${file}" = "build.rs" ]; then yes_build="o:build_rs=yes"; fi
  judge "mutant ${name}" 1 "e:violation: check=env-fs" "e:word=${word}" "e:file=${file}" "e:FAILED:" "o:record: commit=-" "${yes_build}" "o:!ok:" "!${ROOT_DIR}" "!${work}"
  rm -rf "${COPY}"
done

# ---- 陰性の対照（終了コード 0）----
fresh_copy neg
mkdir -p "${COPY}/target" "${COPY}/.hidden" "${COPY}/src/.hidden"
for d in target .hidden src/.hidden; do
  printf 'fn f() { let _ = std::env::args(); let _ = std::fs::metadata("x"); }\n' >"${COPY}/${d}/x.rs"
done
printf 'fn from_env() {}\nfn xenv() { let _ = 1; }\nfn fsx() {}\nuse std::{io};\n' >"${COPY}/src/names.rs"
run_tool --crate-dir "${COPY}" "${SYN[@]}"
judge "negative controls pass" 0 "o:ok: exec surface check passed" "o:env_fs=0" "o:scanned: file=src/names.rs" "o:!.hidden" "o:!target/" "e:!violation"
rm -rf "${COPY}"

# ---- 誤用・前提の不足（終了コード 2）----
run_tool --no-such-option
judge "unknown option" 2 "o:!ok:" "o:!record:"

run_tool --crate-dir
judge "missing option value" 2 "o:!ok:" "o:!record:"

run_tool --crate-dir "${work}/does-not-exist"
judge "missing crate dir" 2 "e:error: crate directory not found" "o:!ok:" "o:!record:" "!${work}"

fresh_copy empty
find "${COPY}" -name '*.rs' -delete
run_tool --crate-dir "${COPY}" "${SYN[@]}"
judge "no rs files" 2 "e:error: no .rs file" "o:!ok:" "o:!record:" "!${work}"
rm -rf "${COPY}"

fresh_copy badutf
printf '\377\n' >"${COPY}/src/bad.rs"
run_tool --crate-dir "${COPY}" "${SYN[@]}"
judge "non-utf8 source" 2 "e:error: source is not valid UTF-8" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy symlink
ln -s lib.rs "${COPY}/src/link.rs"
run_tool --crate-dir "${COPY}" "${SYN[@]}"
judge "symlink to a file" 2 "e:error: symbolic link" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy symdir
ln -s src "${COPY}/linked"
run_tool --crate-dir "${COPY}" "${SYN[@]}"
judge "symlink to a directory" 2 "e:error: symbolic link" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy unclosed
printf 'use std::{io, fs\n' >"${COPY}/src/open.rs"
run_tool --crate-dir "${COPY}" "${SYN[@]}"
judge "unclosed use brace" 2 "e:error: unclosed use std brace" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

# 読めないサブディレクトリは列挙できないので合格させない（root は権限で拒否されないので飛ばす）。
if [ "$(id -u)" -ne 0 ]; then
  fresh_copy unreadable
  mkdir "${COPY}/src/locked"
  printf 'pub fn f() {}\n' >"${COPY}/src/locked/a.rs"
  chmod 000 "${COPY}/src/locked"
  run_tool --crate-dir "${COPY}" "${SYN[@]}"
  judge "unreadable subdirectory" 2 "e:error: source directory cannot be read" "o:!ok:" "o:!record:"
  chmod 755 "${COPY}/src/locked"
  rm -rf "${COPY}"
fi

# python3 が PATH に無い（dirname だけを置いた PATH で絶対パスの bash から起動する）。
mkdir "${work}/nopy"
ln -s "$(command -v dirname)" "${work}/nopy/dirname"
rc=0
PATH="${work}/nopy" "$(command -v bash)" "${TOOL}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "python3 missing" 2 "e:required command not found on PATH: python3" "o:!ok:"

# リビジョンを読むコマンドが無い PATH: 既定の走査は通らず、--crate-dir の走査は通る。
mkdir "${work}/nogit"
ln -s "$(command -v dirname)" "${work}/nogit/dirname"
ln -s "$(command -v python3)" "${work}/nogit/python3"
rc=0
PATH="${work}/nogit" "$(command -v bash)" "${TOOL}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "no revision tool: default scan" 2 "e:error: cannot resolve the source commit" "o:!ok:"
fresh_copy norev
rc=0
PATH="${work}/nogit" "$(command -v bash)" "${TOOL}" --crate-dir "${COPY}" "${SYN[@]}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "no revision tool: crate dir scan" 0 "o:record: commit=-" "o:ok: exec surface check passed"
rm -rf "${COPY}"


# ---- 公開の面の名前（FB-10）: 合成の JSON ----
run_tool "${SYN[@]}"
judge "synthetic json passes" 0 "o:ok: exec surface check passed" "o:port_names_default=0" "o:port_names_missing_all=0" "o:scanned: features=default items=" "o:scanned: features=all-features items=" "e:!violation"

# default に名前を 1 つ足す（名前ごと・種類を変えて）。
pn_rows=(
  'force_unsupported|item:force_unsupported:function'
  'inject_denormal|item:inject_denormal:function'
  'ForcedUnsupported|item:ForcedUnsupported:variant'
  'DenormalInjection|item:DenormalInjection:enum'
  'Injected|item:Injected:struct_field'
  'Injected|use:Injected'
  'force_unsupported|use:force_unsupported'
)
n=0
for row in "${pn_rows[@]}"; do
  IFS='|' read -r pname pspec <<<"${row}"
  n=$((n + 1))
  gen_json "${work}/pn_def.json" item:other_item:struct "${pspec}"
  run_tool --json-default "${work}/pn_def.json" --json-all "${J_ALL}"
  judge "default has ${pspec}" 1 "e:violation: check=port-name features=default name=${pname} count=1" "e:FAILED:" "o:port_names_default=1" "o:!ok:"
done

# all-features から 1 つの名前を消す。
gen_json "${work}/pos_all.json" item:force_unsupported:function item:inject_denormal:function item:ForcedUnsupported:variant item:DenormalInjection:enum
run_tool --json-default "${J_DEF}" --json-all "${work}/pos_all.json"
judge "all-features lacks a name" 1 "e:violation: check=port-name-positive features=all-features name=Injected" "e:FAILED:" "o:port_names_missing_all=1" "o:!ok:"

# 陰性: crate_id 1 の同名、部分的に含む別の名前。
gen_json "${work}/neg_def.json" item:force_unsupported:function:1 item:ForcedUnsupported:variant:2 item:force_unsupported_count:constant item:MyInjected:variant
run_tool --json-default "${work}/neg_def.json" --json-all "${J_ALL}"
judge "other crate and partial names pass" 0 "o:ok:" "o:port_names_default=0" "e:!violation"

# 誤用: 口の指定。
run_tool --json-default "${J_DEF}"
judge "only --json-default" 2 "e:error:" "o:!record:" "o:!ok:"
run_tool --json-all "${J_ALL}"
judge "only --json-all" 2 "e:error:" "o:!record:" "o:!ok:"
run_tool --json-default "${J_DEF}" --json-default "${J_DEF}" --json-all "${J_ALL}"
judge "duplicate --json-default" 2 "e:error: duplicate --json-default" "o:!record:" "o:!ok:"

# 誤用: JSON の中身。
printf 'not json\n' >"${work}/notjson.json"
gen_json "${work}/top_array.json" top:array
gen_json "${work}/private.json" private
gen_json "${work}/fv_str.json" fv:str
gen_json "${work}/badcid.json" badcid
gen_json "${work}/other_root.json" root:some_other_crate
bad_rows=(
  "missing file|${work}/no-such.json"
  "not json|${work}/notjson.json"
  "top level array|${work}/top_array.json"
  "includes_private true|${work}/private.json"
  "format_version string|${work}/fv_str.json"
  "item without crate_id|${work}/badcid.json"
  "root of another crate|${work}/other_root.json"
)
for row in "${bad_rows[@]}"; do
  IFS='|' read -r bname bpath <<<"${row}"
  run_tool --json-default "${bpath}" --json-all "${J_ALL}"
  judge "bad json: ${bname}" 2 "e:error:" "o:!record:" "o:!ok:" "!${work}"
done

# cargo の無い PATH: --json-* を渡さない流しは cargo を呼べず 2。
fresh_copy nocargo
mkdir "${work}/nocargo-bin"
ln -s "$(command -v dirname)" "${work}/nocargo-bin/dirname"
ln -s "$(command -v python3)" "${work}/nocargo-bin/python3"
ln -s "$(command -v git)" "${work}/nocargo-bin/git"
rc=0
PATH="${work}/nocargo-bin" "$(command -v bash)" "${TOOL}" --crate-dir "${COPY}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "cargo missing" 2 "e:error: cannot run cargo" "o:!record:" "o:!ok:"
rm -rf "${COPY}"

# ---- 操作を実行する関数の戻り値の型と一覧に無い公開の関数（FB-22）: 合成の JSON ----
run_tool "${SYN[@]}"
judge "fn scan: baseline" 0 "o:ok:" "o:op_fns=7 op_fns_with_report=7 other_fns=5 unlisted_fns=0" "e:!violation"

# 行: 名前|期待の終了コード|指定(スペース区切り)|見る語(スペース区切り。語の中の ~ は空白)
fn_rows=(
  'unit return|1|opret:hardware_ray_tracing:unit|e:check=op-fn-report~fn=hardware_ray_tracing o:op_fns_with_report=6'
  'bare outcome|1|opret:atomic_add:outcome|e:check=op-fn-report~fn=atomic_add o:op_fns_with_report=6'
  'outcome plain variant|1|variant:plain|e:check=op-fn-report~fn=f16_fma e:check=op-fn-report~fn=unified_memory_sharing o:op_fns_with_report=0'
  'outcome u32 tuple variant|1|variant:u32|e:check=op-fn-report~fn=f16_fma o:op_fns_with_report=0'
  'unlisted Exec fn|1|execfn:extra_op|e:check=unlisted-fn~owner=Exec~fn=extra_op o:unlisted_fns=1'
  'unlisted hidden Exec fn|1|execfn_hidden:hidden_op|e:check=unlisted-fn~owner=Exec~fn=hidden_op o:unlisted_fns=1'
  'unlisted free fn|1|freefn:free_op|e:check=unlisted-fn~owner=-~fn=free_op o:unlisted_fns=1'
  'op fn missing|1|dropop:atomic_add|e:check=op-fn-count~fn=atomic_add~found=0 o:op_fns=6'
  'op fn duplicated|1|dupop:atomic_add|e:check=op-fn-count~fn=atomic_add~found=2 o:op_fns=8'
  'neg trait impl fns|0|traitimpl|o:unlisted_fns=0 o:op_fns_with_report=7'
  'neg Result of report|0|opret:atomic_add:report|o:op_fns_with_report=7'
  'neg Result of struct with report|0|opret:atomic_add:struct_report|o:op_fns_with_report=7'
  'neg std Result|0|opret:atomic_add:stdresult|o:op_fns_with_report=7'
  'neg tuple variant report|0|outcome:tuple|o:op_fns_with_report=7'
  'misuse no Exec|2|noexec|e:error:'
  'misuse unresolved Result id|2|opret:atomic_add:badid|e:error:~return~type~path~cannot~be~resolved'
)
for row in "${fn_rows[@]}"; do
  IFS='|' read -r fname fwant fspecs fterms <<<"${row}"
  # shellcheck disable=SC2086
  gen_json "${work}/fn_def.json" item:other_item:struct ${fspecs}
  run_tool --json-default "${work}/fn_def.json" --json-all "${J_ALL}"
  args=()
  for t in ${fterms}; do args+=("${t//\~/ }"); done
  case "${fwant}" in
    1) args+=("e:FAILED:" "o:!ok:") ;;
    0) args+=("o:ok:" "e:!violation") ;;
    2) args+=("o:!record:" "o:!ok:") ;;
  esac
  judge "fn scan: ${fname}" "${fwant}" "${args[@]}"
done

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
