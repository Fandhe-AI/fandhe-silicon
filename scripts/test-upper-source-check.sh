#!/usr/bin/env bash
# upper-source-check.sh の回帰テスト（spec TASK-2387 のポインタ。親 Issue #1059 の 2/4）。
# 実物の crates/upper の src/ と Cargo.toml を一時ディレクトリへ写し、1 か所だけ字句を足した変異体を
# --crate-dir で走査させて検出を確かめる。陰性の対照・誤用の入力・実物の合格も確かめる。
# リポジトリのファイルは変更しない（実物は読むだけ）・ネットワーク不要。
# 呼び出し元: `make test-upper-source-check`（直接実行も可）、CI の `deny` ジョブ。
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

# replace_once <ファイル> <旧> <新>: 旧の最初の出現を新に置き換える。見つからなければ 1 を返す。
replace_once() {
  python3 -I -c '
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(path, encoding="utf-8").read()
if old not in text:
    sys.exit(1)
open(path, "w", encoding="utf-8").write(text.replace(old, new, 1))
' "$1" "$2" "$3"
}

# need_edit <名前>: 直前の編集の戻り値が 0 でなければ場面を NG にして 1 を返す（黙って通さない）。
need_edit() {
  failed=$((failed + 1))
  echo "NG: $1 (anchor not found)"
}

ANCHOR='#![forbid(unsafe_code)]'
TRAIT_REAL="${ROOT_DIR}/crates/core/src/execution/submit.rs"

# ---- 実物の合格 ----
run_tool
judge "real crate passes" 0 "o:ok: upper source scan passed" "o:record:" "o:global_state=0 env_fs=0" "o:table_fns=4" "o:contract_call_mismatch=0" "o:dispatch_ex=0" "o:failure_constructs=0" "o:scanned: file=src/lib.rs" "e:!violation" "!error:" "!${ROOT_DIR}"
REAL_OPS="$(grep -o 'contract_ops=[0-9]*' "${OUT_F}" | head -n 1 | cut -d= -f2)"

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

# ---- 関数の表の走査: 契約の呼び出し・失敗の構築（終了コード 1）----
# 行: 名前|対象ファイル|アンカー（関数の宣言の行）|足す行|標準エラーに出る語 1|語 2。足す行は宣言の直後（本体の先頭）に入る。
# (3) 契約の呼び出しの群（UPPER-1）
# (4) 失敗の構築の群（UPPER-2）
CALL_MUTANTS=(
  'extra create_encoder|src/device.rs|pub fn create_command_encoder(|let _ = self.device.queue().create_encoder();|check=contract-call fn=create_command_encoder op=create_encoder expected=1 found=2|o:contract_call_mismatch=1'
  'dispatch_ex in submit|src/device.rs|pub fn submit(|let _ = self.device.queue().dispatch_ex(p, b, g);|check=contract-call fn=submit op=dispatch_ex expected=0 found=1|o:dispatch_ex=1'
  'gpu_timing in finish|src/encoder.rs|pub fn finish(|let _ = q.gpu_timing(t);|check=contract-call fn=finish op=gpu_timing expected=0 found=1|o:contract_call_mismatch=1'
  'path call in create|src/device.rs|pub fn create_command_encoder(|let _ = CommandQueue::poll(q, t);|check=contract-call fn=create_command_encoder op=poll expected=0 found=1|o:contract_call_mismatch=1'
  'spaced call|src/encoder.rs|pub fn begin_compute_pass(|let _ = q . poll (t);|check=contract-call fn=begin_compute_pass op=poll expected=0 found=1|o:contract_call_mismatch=1'
)
FAIL_MUTANTS=(
  'Err in submit|src/device.rs|pub fn submit(|return Err(e);|check=failure-construct fn=submit|word=Err('
  'panic in finish|src/encoder.rs|pub fn finish(|panic!("x");|check=failure-construct fn=finish|word=panic!'
  'assert in begin|src/encoder.rs|pub fn begin_compute_pass(|assert!(true);|check=failure-construct fn=begin_compute_pass|word=assert!'
  'unreachable in create|src/device.rs|pub fn create_command_encoder(|unreachable!();|check=failure-construct fn=create_command_encoder|word=unreachable!'
  'todo in submit|src/device.rs|pub fn submit(|todo!();|check=failure-construct fn=submit|word=todo!'
  'unimplemented in finish|src/encoder.rs|pub fn finish(|unimplemented!();|check=failure-construct fn=finish|word=unimplemented!'
  'assert_eq in begin|src/encoder.rs|pub fn begin_compute_pass(|assert_eq!(1, 1);|check=failure-construct fn=begin_compute_pass|word=assert_eq!'
  'debug_assert in create|src/device.rs|pub fn create_command_encoder(|debug_assert!(true);|check=failure-construct fn=create_command_encoder|word=debug_assert!'
  'unwrap in submit|src/device.rs|pub fn submit(|x.unwrap();|check=failure-construct fn=submit|word=.unwrap('
  'expect in finish|src/encoder.rs|pub fn finish(|x.expect("x");|check=failure-construct fn=finish|word=.expect('
)
for row in "${CALL_MUTANTS[@]}" "${FAIL_MUTANTS[@]}"; do
  IFS='|' read -r name file anchor add t1 t2 <<<"${row}"
  fresh_copy "fm"
  if ! insert_after "${COPY}/${file}" "${anchor}" "${add}"; then
    need_edit "fn mutant ${name}"
    rm -rf "${COPY}"
    continue
  fi
  run_tool --crate-dir "${COPY}"
  judge "fn mutant ${name}" 1 "e:violation: ${t1}" "${t2}" "e:FAILED:" "o:record: commit=-" "o:!ok:" "!${ROOT_DIR}"
  rm -rf "${COPY}"
done

# 本体の呼び出しの名前を trait に無い名前へ変える（found=0）。
fresh_copy "fm"
if replace_once "${COPY}/src/device.rs" ".submit(buffer.into_contract()" ".send(buffer.into_contract()"; then
  run_tool --crate-dir "${COPY}"
  judge "fn mutant submit renamed" 1 "e:violation: check=contract-call fn=submit op=submit expected=1 found=0" "e:FAILED:" "o:!ok:"
else
  need_edit "fn mutant submit renamed"
fi
rm -rf "${COPY}"

# ---- 関数の表の走査: 陰性の対照（終了コード 0）----
# 表の外の関数の本体
fresh_copy "fneg1"
if insert_after "${COPY}/src/device.rs" "pub fn gpu_timing(" "let _ = Err(1);" &&
  insert_after "${COPY}/src/pass.rs" "pub fn end(" 'panic!("x");'; then
  run_tool --crate-dir "${COPY}"
  judge "fn negative: outside table" 0 "o:ok: upper source scan passed" "o:failure_constructs=0" "o:contract_call_mismatch=0" "e:!violation"
else
  need_edit "fn negative: outside table"
fi
rm -rf "${COPY}"

# 表の関数の本体のコメント・文字列（数えない）
fresh_copy "fneg2"
if insert_after "${COPY}/src/device.rs" "pub fn submit(" '// Err( .dispatch_ex( panic!' &&
  insert_after "${COPY}/src/device.rs" "pub fn submit(" 'let _s = "Err( .dispatch_ex(";' &&
  insert_after "${COPY}/src/device.rs" "pub fn submit(" '/* .create_encoder( */' &&
  insert_after "${COPY}/src/device.rs" "pub fn submit(" 'let _r = r#".submit("#;'; then
  run_tool --crate-dir "${COPY}"
  judge "fn negative: comments and strings" 0 "o:ok: upper source scan passed" "o:failure_constructs=0" "o:contract_call_mismatch=0" "o:dispatch_ex=0" "e:!violation"
else
  need_edit "fn negative: comments and strings"
fi
rm -rf "${COPY}"

# 文字のリテラルの `{`・文字列の `}` が本体の範囲をずらさない（別々の写し）
fresh_copy "fneg3"
if insert_after "${COPY}/src/device.rs" "pub fn create_command_encoder(" "let _c = '{';"; then
  run_tool --crate-dir "${COPY}"
  judge "fn negative: brace char literal" 0 "o:ok: upper source scan passed" "o:contract_call_mismatch=0" "e:!violation"
else
  need_edit "fn negative: brace char literal"
fi
rm -rf "${COPY}"
fresh_copy "fneg4"
if insert_after "${COPY}/src/device.rs" "pub fn create_command_encoder(" 'let _d = "}";'; then
  run_tool --crate-dir "${COPY}"
  judge "fn negative: brace in string" 0 "o:ok: upper source scan passed" "o:contract_call_mismatch=0" "e:!violation"
else
  need_edit "fn negative: brace in string"
fi
rm -rf "${COPY}"

# trait の写しにダミーの操作を足す: 期待 0 の名前として数えられ、合格のまま
TRAIT_COPY="${work}/trait-probe.rs"
cp "${TRAIT_REAL}" "${TRAIT_COPY}"
if insert_after "${TRAIT_COPY}" "pub trait CommandQueue {" "    fn probe_op(&self);"; then
  run_tool --queue-trait-file "${TRAIT_COPY}"
  judge "trait copy with extra operation" 0 "o:ok: upper source scan passed" "o:contract_ops=$((REAL_OPS + 1))" "o:record: commit=-" "e:!violation"
  fresh_copy "tp"
  if insert_after "${COPY}/src/device.rs" "pub fn submit(" "let _ = self.device.queue().probe_op();"; then
    run_tool --crate-dir "${COPY}" --queue-trait-file "${TRAIT_COPY}"
    judge "trait copy: call of the extra operation" 1 "e:violation: check=contract-call fn=submit op=probe_op expected=0 found=1" "e:FAILED:" "o:!ok:"
  else
    need_edit "trait copy: call of the extra operation"
  fi
  rm -rf "${COPY}"
else
  need_edit "trait copy with extra operation"
fi

# 深さ 2 の `fn` は操作の名前に入らない
TRAIT_NEST="${work}/trait-nest.rs"
cp "${TRAIT_REAL}" "${TRAIT_NEST}"
if insert_after "${TRAIT_NEST}" "fn poll_ex(&self, token: SubmitToken) -> CompletionStateEx {" "        fn nested_probe() {}"; then
  fresh_copy "tn"
  if insert_after "${COPY}/src/device.rs" "pub fn submit(" "let _ = self.nested_probe();"; then
    run_tool --crate-dir "${COPY}" --queue-trait-file "${TRAIT_NEST}"
    judge "trait copy: nested fn is not an operation" 0 "o:ok: upper source scan passed" "o:contract_ops=${REAL_OPS}" "e:!violation"
  else
    need_edit "trait copy: nested fn is not an operation"
  fi
  rm -rf "${COPY}"
else
  need_edit "trait copy: nested fn is not an operation"
fi

# ---- 関数の表の走査: 誤用・読み切れない入力（終了コード 2）----
fresh_copy "fmis1"
if replace_once "${COPY}/src/encoder.rs" "pub fn finish(self)" "pub fn finish_renamed(self)"; then
  run_tool --crate-dir "${COPY}"
  judge "fn missing from source" 2 "e:error: function finish must be found exactly once (found 0)" "o:!ok:" "o:!record:" "!${work}" "!${ROOT_DIR}"
  printf 'trait T { fn finish(self); }\n' >"${COPY}/src/decl.rs"
  run_tool --crate-dir "${COPY}"
  judge "fn without body" 2 "e:error: function finish has no body" "o:!ok:" "o:!record:"
else
  need_edit "fn missing from source"
fi
rm -rf "${COPY}"

fresh_copy "fmis2"
printf 'fn finish() {}\n' >"${COPY}/src/dup.rs"
run_tool --crate-dir "${COPY}"
judge "fn defined twice" 2 "e:error: function finish must be found exactly once (found 2)" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy "fmis3"
printf 'fn x() {}\n/* never closed\n' >"${COPY}/src/open2.rs"
run_tool --crate-dir "${COPY}"
judge "unclosed block comment" 2 "e:error: unclosed comment or literal" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy "fmis4"
printf 'const S: &str = "never closed;\n' >"${COPY}/src/open2.rs"
run_tool --crate-dir "${COPY}"
judge "unclosed string" 2 "e:error: unclosed comment or literal" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

fresh_copy "fmis5"
printf 'const C: char = '"'"'\\n;\n' >"${COPY}/src/open2.rs"
run_tool --crate-dir "${COPY}"
judge "unclosed char literal" 2 "e:error: unclosed comment or literal" "o:!ok:" "o:!record:"
rm -rf "${COPY}"

# --queue-trait-file の誤用
run_tool --queue-trait-file
judge "queue trait option without value" 2 "o:!ok:" "o:!record:"

run_tool --queue-trait-file "${work}/does-not-exist.rs"
judge "queue trait file missing" 2 "e:error: queue trait file not found" "o:!ok:" "o:!record:" "!${work}"

printf 'pub trait Other { fn a(&self); }\n' >"${work}/no-trait.rs"
run_tool --queue-trait-file "${work}/no-trait.rs"
judge "queue trait not in file" 2 "e:error: queue trait must be found exactly once" "o:!ok:" "o:!record:" "!${work}"

printf 'pub trait CommandQueue {}\n' >"${work}/empty-trait.rs"
run_tool --queue-trait-file "${work}/empty-trait.rs"
judge "queue trait without operations" 2 "e:error: no operation found in queue trait" "o:!ok:" "o:!record:"

printf 'pub trait CommandQueue { fn a(&self);\n' >"${work}/open-trait.rs"
run_tool --queue-trait-file "${work}/open-trait.rs"
judge "queue trait body not closed" 2 "e:error: queue trait body is not closed" "o:!ok:" "o:!record:"

cp "${TRAIT_REAL}" "${work}/trait-renamed.rs"
if replace_once "${work}/trait-renamed.rs" "fn create_encoder(&self)" "fn create_encoder_renamed(&self)"; then
  run_tool --queue-trait-file "${work}/trait-renamed.rs"
  judge "table operation missing from trait" 2 "e:error: table operation create_encoder is not in the queue trait" "o:!ok:" "o:!record:"
else
  need_edit "table operation missing from trait"
fi

printf '\377\n' >"${work}/bad-utf8.rs"
run_tool --queue-trait-file "${work}/bad-utf8.rs"
judge "queue trait file not utf-8" 2 "e:error: queue trait file is not valid UTF-8" "o:!ok:" "o:!record:"

ln -s "${TRAIT_REAL}" "${work}/trait-link.rs"
run_tool --queue-trait-file "${work}/trait-link.rs"
judge "queue trait file is a symlink" 2 "e:error: queue trait file is a symbolic link" "o:!ok:" "o:!record:"

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

# リビジョンの取得に使う外部コマンドが無くても既定の走査は通り、commit は `-` になる。
mkdir "${work}/nogit"
ln -s "$(command -v dirname)" "${work}/nogit/dirname"
ln -s "$(command -v python3)" "${work}/nogit/python3"
rc=0
PATH="${work}/nogit" "$(command -v bash)" "${TOOL}" >"${OUT_F}" 2>"${ERR_F}" || rc=$?
judge "no revision tool" 0 "o:record: commit=-" "o:ok: upper source scan passed"

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
