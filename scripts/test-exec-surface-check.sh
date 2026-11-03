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

# ---- 実物の合格 ----
run_tool
judge "real crate passes" 0 "o:ok: exec surface check passed" "o:record: commit=" "o:env_fs=0" "o:scanned: file=src/lib.rs" "o:scanned: file=tests/common/mod.rs" "e:!violation" "!error:" "!${ROOT_DIR}"

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
  run_tool --crate-dir "${COPY}"
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
run_tool --crate-dir "${COPY}"
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
run_tool --crate-dir "${COPY}"
judge "no rs files" 2 "e:error: no .rs file" "o:!ok:" "o:!record:" "!${work}"
rm -rf "${COPY}"

fresh_copy badutf
printf '\377\n' >"${COPY}/src/bad.rs"
run_tool --crate-dir "${COPY}"
judge "non-utf8 source" 2 "e:error: source is not valid UTF-8" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy symlink
ln -s lib.rs "${COPY}/src/link.rs"
run_tool --crate-dir "${COPY}"
judge "symlink to a file" 2 "e:error: symbolic link" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy symdir
ln -s src "${COPY}/linked"
run_tool --crate-dir "${COPY}"
judge "symlink to a directory" 2 "e:error: symbolic link" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy unclosed
printf 'use std::{io, fs\n' >"${COPY}/src/open.rs"
run_tool --crate-dir "${COPY}"
judge "unclosed use brace" 2 "e:error: unclosed use std brace" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

# 読めないサブディレクトリは列挙できないので合格させない（root は権限で拒否されないので飛ばす）。
if [ "$(id -u)" -ne 0 ]; then
  fresh_copy unreadable
  mkdir "${COPY}/src/locked"
  printf 'pub fn f() {}\n' >"${COPY}/src/locked/a.rs"
  chmod 000 "${COPY}/src/locked"
  run_tool --crate-dir "${COPY}"
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
PATH="${work}/nogit" "$(command -v bash)" "${TOOL}" --crate-dir "${COPY}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "no revision tool: crate dir scan" 0 "o:record: commit=-" "o:ok: exec surface check passed"
rm -rf "${COPY}"

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
