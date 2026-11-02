#!/usr/bin/env bash
# wasm32 のコンパイル試験の検査（spec TASK-239・BUILD-24 のポインタ。親 Issue #463 の 1/4）。
#
# `scripts/build-profiles.sh` の wasm32 の組（`# spec BUILD-19` の注記の行）で、行ごとに一時の crate を作り、
# `cargo check --target wasm32-unknown-unknown` の成否とエラーを期待（pass／fail とエラーコード・識別子）と照らす。
# 試験の表は `probe_table` の 1 か所で、行を足せる形にしてある（snippet は `snippet_<鍵>` に 1 つずつ置く）。
# `build-profiles.sh` は読むだけで、実行も source もしない。組の定義は注記の行の 1 か所に保つ。
# 一時の crate は `mktemp -d` の下に作って終了時に消す。cargo の出力先は `target/wasm-compile-probe` だけで、
# リポジトリのファイルは変更しない。
#
# 呼び出し元: `make wasm-compile-probe-check`（回帰テストは `make test-wasm-compile-probe-check`）。
# `scripts/wasm-verify.sh`（`make wasm-verify`。CI の `deny`・`wasm-macos` ジョブ）からも引数なしで呼ばれる。
# 要るもの: bash・cargo・grep・sed・wc・tr・cat・mktemp・wasm32-unknown-unknown の target。
#
# 使い方: scripts/wasm-compile-probe-check.sh [--build-profiles <path>]
#   --build-profiles: 読む build-profiles.sh の差し替え（回帰テスト用）。
# 終了コード:
#   0: 全行 ok
#   1: どれかの行が ng
#   2: 誤用・前提の不足（cargo が無い・wasm32 の target が無い）・読めない入力（注記の行が 0 行か 2 行以上・引用符が無い・組の文字が不正）
# 出力: 行ごとに標準出力へ
#   `record: probe=<名前> target=wasm32-unknown-unknown features=<組> expect=<pass|fail> code=<コード|-> result=<ok|ng>`、
#   全行 ok なら最後に `wasm-compile-probe-check: all probes passed`。ng の理由は標準エラー（cargo の `error` の行）。
#
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

TARGET="wasm32-unknown-unknown"

# 試験の表。1 行 1 試験で、列は `名前|足す feature（無ければ -）|期待|コード|識別子|snippet の鍵`。
probe_table() {
  cat <<'TABLE'
download_blocking|-|fail|E0599|download_blocking|memory
wait_blocking|-|fail|E0599|wait_blocking|queue
nonblocking_readback|-|pass|-|-|nonblocking
download_blocking_gate_open|native-blocking-wait|pass|-|-|memory
wait_blocking_gate_open|native-blocking-wait|pass|-|-|queue
native_handle_send_sync|-|pass|-|-|native_handle
native_handle_rejects_not_send|-|fail|E0277|Rc<u8>|native_handle_rc
encoder_closer_send_sync|-|pass|-|-|closer
encoder_closer_rejects_not_send|-|fail|E0277|Rc<()>|closer_rc
shared_closer_rejects_not_sync|-|fail|E0277|Cell<u32>|shared_closer_cell
command_encoder_send_sync|-|pass|-|-|encoder
TABLE
}

# snippet の鍵ごとの `src/lib.rs` の中身。
snippet_memory() {
  cat <<'SNIP'
use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
pub fn probe(ops: &dyn MemoryOps, buffer: &BufferHandle) {
    let _ = ops.download_blocking(buffer, 0, 4);
}
SNIP
}
snippet_queue() {
  cat <<'SNIP'
use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
pub fn probe(queue: &dyn CommandQueue, token: SubmitToken) {
    let _ = queue.wait_blocking(token, None);
}
SNIP
}
snippet_nonblocking() {
  cat <<'SNIP'
use fandhe_silicon_core::error::{CompletionState, ContractError};
use fandhe_silicon_core::execution::{CommandQueue, SubmitToken};
use fandhe_silicon_core::memory::{BufferHandle, MemoryOps};
pub fn probe(ops: &dyn MemoryOps, buffer: &BufferHandle, queue: &dyn CommandQueue, token: SubmitToken) {
    ops.download_async(buffer, 0, 4, Box::new(|_: Result<Vec<u8>, ContractError>| {}));
    queue.on_complete(token, Box::new(|_: CompletionState| {}));
}
SNIP
}

# 以下の 6 つの snippet の行は spec TASK-553・BUILD-30 のポインタ（親 Issue #1166 の 1/2）。
snippet_native_handle() {
  cat <<'SNIP'
use fandhe_silicon_core::device::{DeviceOwnership, DeviceRequest, NativeDeviceHandle};
use std::sync::Arc;
fn require_send_sync<T: Send + Sync>() {}
pub fn probe() -> DeviceOwnership {
    require_send_sync::<NativeDeviceHandle>();
    require_send_sync::<DeviceOwnership>();
    require_send_sync::<DeviceRequest>();
    DeviceOwnership::Imported { native: NativeDeviceHandle::new(Arc::new(0u8)) }
}
SNIP
}
snippet_native_handle_rc() {
  cat <<'SNIP'
use fandhe_silicon_core::device::NativeDeviceHandle;
use std::rc::Rc;
pub fn probe() -> NativeDeviceHandle {
    NativeDeviceHandle::new(Rc::new(0u8))
}
SNIP
}
snippet_closer() {
  cat <<'SNIP'
use fandhe_silicon_core::execution::{CommandEncoder, __BackendEncoderCloser, __BackendSharedEncoderCloser};
use std::sync::Arc;
struct Closer(u32);
impl __BackendEncoderCloser for Closer {
    fn close(&mut self) { self.0 += 1; }
}
struct Shared;
impl __BackendSharedEncoderCloser for Shared {
    fn close(&self, _id: u64) {}
}
pub fn probe() -> (CommandEncoder, CommandEncoder) {
    let boxed = CommandEncoder::__backend_new(1, Some(Box::new(Closer(0))));
    let shared = CommandEncoder::__backend_new_shared(2, Arc::new(Shared));
    (boxed, shared)
}
SNIP
}
snippet_closer_rc() {
  cat <<'SNIP'
use fandhe_silicon_core::execution::__BackendEncoderCloser;
use std::rc::Rc;
struct Closer(Rc<()>);
impl __BackendEncoderCloser for Closer {
    fn close(&mut self) {}
}
SNIP
}
snippet_shared_closer_cell() {
  cat <<'SNIP'
use fandhe_silicon_core::execution::__BackendSharedEncoderCloser;
use std::cell::Cell;
struct Shared(Cell<u32>);
impl __BackendSharedEncoderCloser for Shared {
    fn close(&self, _id: u64) { self.0.set(self.0.get() + 1); }
}
SNIP
}
snippet_encoder() {
  cat <<'SNIP'
use fandhe_silicon_core::execution::CommandEncoder;
fn require_send_sync<T: Send + Sync>() {}
pub fn probe() {
    require_send_sync::<CommandEncoder>();
}
SNIP
}

usage() {
  echo "usage: $(basename "$0") [--build-profiles <path>]" >&2
  exit 2
}

SCRIPT_PATH="${ROOT_DIR}/scripts/build-profiles.sh"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --build-profiles)
      [ "$#" -ge 2 ] || usage
      SCRIPT_PATH="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done
if [ ! -f "${SCRIPT_PATH}" ] || [ ! -r "${SCRIPT_PATH}" ]; then
  echo "error: cannot read the profile script" >&2
  exit 2
fi
# 相対パスは cd の前に絶対にしておく。
case "${SCRIPT_PATH}" in
  /*) ;;
  *) SCRIPT_PATH="$(pwd)/${SCRIPT_PATH}" ;;
esac
require_cmd cargo || exit 2

# 生成する Cargo.toml の path の文字列に埋めるので、壊す文字は先に断る。
case "${ROOT_DIR}" in
  *'"'* | *\\* | *$'\n'*)
    echo "error: the repository path contains a character that cannot be embedded in Cargo.toml" >&2
    exit 2
    ;;
esac

work="$(mktemp -d "${TMPDIR:-/tmp}/wasm-compile-probe-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT

# wasm32 の組を取り出して検証する（読めない入力で途中まで流さない）。
grep -E '# spec BUILD-19$' "${SCRIPT_PATH}" >"${work}/line" || :
lines="$(wc -l <"${work}/line" | tr -d ' ')"
if [ "${lines}" -ne 1 ]; then
  echo "error: expected exactly 1 note line for the wasm32 feature set, found ${lines}" >&2
  exit 2
fi
base_set="$(sed -n 's/^[^"]*"\([^"]*\)".*$/\1/p' "${work}/line")"
if [ -z "${base_set}" ]; then
  echo "error: no quoted feature set on the wasm32 note line" >&2
  exit 2
fi
if ! printf '%s\n' "${base_set}" | grep -Eq '^[a-z0-9][a-z0-9,-]*$'; then
  echo "error: invalid characters in the wasm32 feature set" >&2
  exit 2
fi

# feature の列（カンマ区切り）を `"a", "b"` の形にする。
quote_features() {
  printf '%s\n' "$1" | sed 's/,/", "/g; s/^/"/; s/$/"/'
}

cd "${ROOT_DIR}"
mkdir -p "${ROOT_DIR}/target/wasm-compile-probe"
probe_table >"${work}/table"

failures=0
while IFS='|' read -r name extra expect code ident snip; do
  features="${base_set}"
  if [ "${extra}" != "-" ]; then
    features="${features},${extra}"
  fi
  crate="${work}/${name}"
  mkdir -p "${crate}/src"
  {
    echo "[package]"
    echo "name = \"wasm-compile-probe-$(printf '%s' "${name}" | tr '_' '-')\""
    echo 'version = "0.0.0"'
    echo 'edition = "2024"'
    echo 'publish = false'
    echo
    echo "[dependencies]"
    echo "fandhe-silicon-contract = { path = \"${ROOT_DIR}/crates/contract\", default-features = false, features = [$(quote_features "${features}")] }"
    echo "fandhe-silicon-core = { path = \"${ROOT_DIR}/crates/core\" }"
    echo
    echo "[workspace]"
  } >"${crate}/Cargo.toml"
  "snippet_${snip}" >"${crate}/src/lib.rs"

  out="${work}/out.${name}"
  err="${work}/err.${name}"
  log "cargo check --target ${TARGET} (probe=${name} features=${features})"
  rc=0
  # 利用者の環境の RUSTFLAGS 系・出力先・target は引き継がない。
  env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS \
    -u CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS -u CARGO_TARGET_DIR \
    -u CARGO_BUILD_TARGET_DIR -u CARGO_BUILD_TARGET \
    cargo check --offline --manifest-path "${crate}/Cargo.toml" --target "${TARGET}" \
    --target-dir "${ROOT_DIR}/target/wasm-compile-probe" >"${out}" 2>"${err}" || rc=$?

  # target が無いときは行を判定せず止める（fail の行を合格にしない）。
  if grep -q -F 'may not be installed' "${out}" "${err}"; then
    echo "error: the ${TARGET} target is not installed (rustup target add ${TARGET})" >&2
    exit 2
  fi

  ok=0
  if [ "${expect}" = "pass" ]; then
    if [ "${rc}" -eq 0 ]; then ok=1; fi
  else
    if [ "${rc}" -ne 0 ]; then
      cat "${out}" "${err}" >"${work}/both"
      if grep -E "^error\\[${code}\\]:" "${work}/both" | grep -q -F "\`${ident}\`"; then
        ok=1
      fi
    fi
  fi

  result="ok"
  if [ "${ok}" -ne 1 ]; then
    result="ng"
    failures=$((failures + 1))
    # 一時パスを含む `-->` の行は出さず、`error` で始まる行だけを出す。
    cat "${out}" "${err}" | grep -E '^error' >&2 || :
  fi
  echo "record: probe=${name} target=${TARGET} features=${features} expect=${expect} code=${code} result=${result}"
done <"${work}/table"

if [ "${failures}" -ne 0 ]; then
  echo "wasm-compile-probe-check: ${failures} probe(s) failed" >&2
  exit 1
fi
echo "wasm-compile-probe-check: all probes passed"
