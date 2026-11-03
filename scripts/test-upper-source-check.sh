#!/usr/bin/env bash
# upper-source-check.sh の回帰テスト（spec TASK-2387 のポインタ。親 Issue #1059 の 1/4）。
# 実物の crates/upper の src/ と Cargo.toml を一時ディレクトリへ写し、1 か所だけ字句を足した変異体を
# --crate-dir で走査させて検出を確かめる。陰性の対照・誤用の入力・実物の合格も確かめる。
# リポジトリのファイルは変更しない（実物は読むだけ）・ネットワーク不要。
# 呼び出し元: `make test-upper-source-check`（直接実行も可）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」（`o:` 標準出力・`e:` 標準エラー・先頭が `!` は含まれないこと）。
# 期待値を緩めない。途中で止めず、最後に件数を出す。bash 3.2 互換。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
require_cmd python3

TOOL="${DIR}/upper-source-check.sh"
REAL_CRATE="${ROOT_DIR}/crates/upper"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-upper-source-check.XXXXXX")"
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

# fresh_copy <名前>: 実物の crate を写した一時 crate のパスを COPY に入れる。
COPY=""
fresh_copy() {
  COPY="${work}/$1"
  mkdir "${COPY}"
  cp -R "${REAL_CRATE}/src" "${COPY}/src"
  cp "${REAL_CRATE}/Cargo.toml" "${COPY}/Cargo.toml"
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

ANCHOR='#![forbid(unsafe_code)]'

# ---- 実物の合格 ----
run_tool
judge "real crate passes" 0 "o:ok: upper source scan passed" "o:record:" "o:global_state=0 env_fs=0" "o:scanned: file=src/lib.rs" "e:!violation" "!error:" "!${ROOT_DIR}"

# ---- 変異体（終了コード 1）----
# 行: 名前|check|語|対象ファイル|足す行（アンカーは ANCHOR。対象ファイルが実物に無ければ新しいファイル）。
# (1) グローバルな状態の群（UPPER-32）
# (2) 環境変数とファイルの読み取りの群（UPPER-40）
mutants=(
  'static item|global-state|static|src/lib.rs|static X: u8 = 0;'
  'thread_local|global-state|thread_local!|src/lib.rs|thread_local! { static Y: u8 = 0; }'
  'Mutex use|global-state|Mutex|src/lib.rs|fn m(_: std::sync::Mutex<u8>) {}'
  'static lifetime|global-state|static|src/lib.rs|fn l(_: &'"'"'static str) {}'
  'Mutex in comment|global-state|Mutex|src/lib.rs|// Mutex'
  'std::env literal|env-fs|std::env|src/lib.rs|fn a() { let _ = std::env::args(); }'
  'env::var literal|env-fs|env::var|src/lib.rs|fn a() { let _ = env::var("X"); }'
  'option_env literal|env-fs|option_env!|src/lib.rs|const A: Option<&str> = option_env!("X");'
  'env! literal|env-fs|env!|src/lib.rs|const A: &str = env!("X");'
  'std::fs literal|env-fs|std::fs|src/lib.rs|fn a() { let _ = std::fs::metadata("x"); }'
  'File::open literal|env-fs|File::open|src/lib.rs|fn a() { let _ = File::open("x"); }'
  'use std brace env|env-fs|std::{env}|src/lib.rs|use std::{env};'
  'use std brace fs|env-fs|std::{fs}|src/lib.rs|use std::{io, fs::File};'
  'use std nested env|env-fs|std::{env}|src/lib.rs|use std::{collections::{HashMap}, env::var};'
  'env in line comment|env-fs|std::env|src/lib.rs|// std::env'
  'env in doc comment|env-fs|std::fs|src/lib.rs|/// std::fs'
  'env in string|env-fs|File::open|src/lib.rs|const S: &str = "File::open";'
  'env in cfg(test) mod|env-fs|env!|src/lib.rs|#[cfg(test)] mod t { const A: &str = env!("X"); }'
  'extra file env|env-fs|std::env|src/extra.rs|fn a() { let _ = std::env::args(); }'
  'extra file static|global-state|static|src/extra.rs|static Z: u8 = 0;'
  'build.rs env|env-fs|env::var|build.rs|fn main() { let _ = std::env::var("X"); }'
)
for row in "${mutants[@]}"; do
  IFS='|' read -r name check word file add <<<"${row}"
  fresh_copy "m"
  if [ -e "${REAL_CRATE}/${file}" ]; then
    if ! insert_after "${COPY}/${file}" "${ANCHOR}" "${add}"; then
      failed=$((failed + 1))
      echo "NG: mutant ${name} (anchor not found)"
      rm -rf "${COPY}"
      continue
    fi
  else
    printf '%s\n' "${add}" >"${COPY}/${file}"
  fi
  run_tool --crate-dir "${COPY}"
  yes_build="o:build_rs=no"
  if [ "${file}" = "build.rs" ]; then yes_build="o:build_rs=yes"; fi
  judge "mutant ${name}" 1 "e:violation: check=${check}" "e:word=${word}" "e:file=${file}" "e:FAILED:" "o:record: commit=-" "${yes_build}" "o:!ok:" "!${ROOT_DIR}"
  rm -rf "${COPY}"
done

# ---- 陰性の対照（終了コード 0）----
fresh_copy neg
for d in tests benches examples; do
  mkdir "${COPY}/${d}"
  printf 'static A: u8 = 0;\nfn f() { let _ = std::env::args(); let _ = std::fs::metadata("x"); let _: Option<std::sync::Mutex<u8>> = None; }\n' >"${COPY}/${d}/x.rs"
done
mkdir "${COPY}/src/.hidden"
printf 'static H: u8 = 0;\n' >"${COPY}/src/.hidden/h.rs"
printf 'fn from_env() {}\nfn lazy_static_x() {}\nfn MutexGuard_like() {}\nstruct MutexGuard;\nfn xenv() { let _ = 1; }\n' >"${COPY}/src/names.rs"
run_tool --crate-dir "${COPY}"
judge "negative controls pass" 0 "o:ok: upper source scan passed" "o:global_state=0 env_fs=0" "o:scanned: file=src/names.rs" "o:!.hidden" "e:!violation"
rm -rf "${COPY}"

# ---- 誤用・前提の不足（終了コード 2）----
run_tool --no-such-option
judge "unknown option" 2 "o:!ok:" "o:!record:"

run_tool --crate-dir
judge "missing option value" 2 "o:!ok:" "o:!record:"

run_tool --crate-dir "${work}/does-not-exist"
judge "missing crate dir" 2 "e:error:" "o:!ok:" "o:!record:" "!${work}"

fresh_copy empty
find "${COPY}/src" -name '*.rs' -delete
run_tool --crate-dir "${COPY}"
judge "no rs files" 2 "e:error: no .rs file" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy nosrc
rm -rf "${COPY}/src"
run_tool --crate-dir "${COPY}"
judge "no src dir" 2 "e:error:" "o:!record:"
rm -rf "${COPY}"

fresh_copy badutf
printf '\377\n' >"${COPY}/src/bad.rs"
run_tool --crate-dir "${COPY}"
judge "non-utf8 source" 2 "e:error: source is not valid UTF-8" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy symlink
ln -s lib.rs "${COPY}/src/link.rs"
run_tool --crate-dir "${COPY}"
judge "symlink in src" 2 "e:error: symbolic link" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy unclosed
printf 'use std::{io, fs\n' >"${COPY}/src/open.rs"
run_tool --crate-dir "${COPY}"
judge "unclosed use brace" 2 "e:error: unclosed use std brace" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

# python3 が PATH に無い（dirname だけを置いた PATH で絶対パスの bash から起動する）。
mkdir "${work}/nopy"
ln -s "$(command -v dirname)" "${work}/nopy/dirname"
rc=0
PATH="${work}/nopy" "$(command -v bash)" "${TOOL}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "python3 missing" 2 "e:required command not found on PATH: python3" "o:!ok:"

# リビジョンの取得に使う外部コマンドが無くても既定の走査は通り、commit は `-` になる。
mkdir "${work}/nogit"
ln -s "$(command -v dirname)" "${work}/nogit/dirname"
ln -s "$(command -v python3)" "${work}/nogit/python3"
rc=0
PATH="${work}/nogit" "$(command -v bash)" "${TOOL}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "no revision tool" 0 "o:record: commit=-" "o:ok: upper source scan passed"

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
