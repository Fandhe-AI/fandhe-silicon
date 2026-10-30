#!/usr/bin/env bash
# プロファイルごとの feature の組で contract crate がビルドできることと、core crate が feature なし（試験の枠の
# test-support を外した形）で clippy と test を通ることを確かめる（非破壊。core の spec 対応は TASK-470）。
# native では build / clippy（-D warnings）/ test、wasm32 では contract の check と wasm-abi の 2 版（scalar・simd128）の release ビルドを回す。
# 呼び出し元: `make build-profiles`、CI の verify ジョブ。verify は --all-features しか見ないため、
# feature の組でだけ出るビルド失敗・未使用警告をここで拾う。
# 組の spec 対応は ID のポインタだけを添える（本文は転記しない）。
# wasm32 の target が無い場合は skip せず失敗する（導入は rust-toolchain.toml / rustup の役割）。
# 試験の枠の記録の書き出し先（FANDHE_SILICON_TEST_RECORD_DIR）は、呼び出し側の環境に在っても外す。ここは同じ試験を
# feature の組ごとに何度も流すので、同じディレクトリへ同じ部品を 2 回書くと失敗する。
# 記録と集計は verify.sh の `cargo test` の 1 回だけが扱う。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd rustup
unset FANDHE_SILICON_TEST_RECORD_DIR

readonly PKG="fandhe-silicon-contract"
readonly CORE_PKG="fandhe-silicon-core"
readonly WASM_TARGET="wasm32-unknown-unknown"
readonly WASM_ABI_PKG="fandhe-silicon-wasm-abi"
readonly WASM_ABI_LIB="fandhe_silicon_wasm_abi"

# native の組（--no-default-features に付ける feature 列）
NATIVE_SETS=(
  "contract-core,cpu-isa,cpu-ops"                                                  # spec BUILD-1
  "contract-core,cpu-isa,cpu-ops,backend-metal"                                    # spec BUILD-2
  "contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics"                          # spec BUILD-3
  "contract-core,cpu-isa,cpu-ops,backend-vulkan,backend-cuda,graphics,ext-interop" # spec BUILD-4
  "backend-metal"                                                                  # spec TASK-5（含意で contract-core が入る）
  "backend-vulkan"                                                                 # spec TASK-5
  "backend-cuda"                                                                   # spec TASK-5
  "artifact-inspection"                                                            # spec BUILD-65
  "fault-injection"                                                                # spec TASK-271（テスト専用の feature 単独の組）
  "test-support"                                                                   # spec TASK-24（検証用の道具。背後実装なし）
  "backend-metal,test-support"                                                     # spec TASK-24
  "backend-vulkan,test-support"                                                    # spec TASK-24
  "backend-cuda,test-support"                                                      # spec TASK-24
  "native-blocking-wait"                                                           # spec TASK-233（BUILD-11・BUILD-12。ゲートの feature 単独の組）
  "ext-interop"                                                                    # spec TASK-233（BUILD-13。ゲートの feature 単独の組）
  "backend-vulkan,ext-interop"                                                     # spec TASK-231（BUILD-14。片方の背後実装に ext-interop を足した組）
  "backend-cuda,ext-interop"                                                       # spec TASK-231（BUILD-14。片方の背後実装に ext-interop を足した組）
  "backend-metal,ext-interop"                                                      # spec TASK-231（BUILD-14。片方の背後実装に ext-interop を足した組）
)

# wasm32 の組
WASM_SETS=(
  "contract-core,cpu-isa,cpu-ops"                 # spec BUILD-19
  "contract-core,cpu-isa,cpu-ops,backend-metal"   # spec BUILD-22
  "contract-core,cpu-isa,cpu-ops,backend-vulkan"  # spec BUILD-23
  "contract-core,cpu-isa,cpu-ops,backend-cuda"    # spec BUILD-23
)

# コマンドを表示してから実行する。
run() {
  log "$*"
  "$@"
}

# リポジトリ外から呼ばれても rust-toolchain.toml の toolchain を見るよう、確認の前に移動する。
cd "${ROOT_DIR}"

if ! rustup target list --installed | grep -qx "${WASM_TARGET}"; then
  echo "error: rust target not installed: ${WASM_TARGET}" >&2
  echo "  install with: rustup target add ${WASM_TARGET} (or: rustup toolchain install)" >&2
  exit 1
fi

# 既定（spec BUILD-8）
run cargo build -p "${PKG}"
run cargo clippy -p "${PKG}" --all-targets -- -D warnings
run cargo test -p "${PKG}"

for feats in "${NATIVE_SETS[@]}"; do
  run cargo build -p "${PKG}" --no-default-features --features "${feats}"
  run cargo clippy -p "${PKG}" --all-targets --no-default-features --features "${feats}" -- -D warnings
  run cargo test -p "${PKG}" --no-default-features --features "${feats}"
done

# core を feature なしで（verify は --all-features だけなので、条件なしの型の試験が test-support 抜きでも通ることをここで拾う。spec TASK-470）
run cargo clippy -p "${CORE_PKG}" --all-targets -- -D warnings
run cargo test -p "${CORE_PKG}"

# 全 feature（spec BUILD-6。後から増える feature も自動で含む）
run cargo build -p "${PKG}" --all-features
run cargo clippy -p "${PKG}" --all-targets --all-features -- -D warnings
run cargo test -p "${PKG}" --all-features

for feats in "${WASM_SETS[@]}"; do
  run cargo check -p "${PKG}" --target "${WASM_TARGET}" --no-default-features --features "${feats}"
done

# wasm-abi の cdylib の 2 版の release ビルド（spec BUILD-19・BUILD-20 のポインタ。TASK-237 の 4/4）。
# 利用者の環境の rustflags 系を外し、simd128 版だけ CARGO_ENCODED_RUSTFLAGS で命令を足す（値は wasm-build-check.sh の SIMD_FLAGS と同じ）。
# 2 版の判別（target_features）は wasm-build-check.sh の受け持ちで、ここでは成果物の在りかと wasm の先頭 4 バイトだけを確かめる。
wasm_abi_build() {
  local variant="$1" encoded_flags="$2" target_dir="$3"
  local artifact="${target_dir}/${WASM_TARGET}/release/${WASM_ABI_LIB}.wasm"
  log "wasm-abi ${variant}: cargo build -p ${WASM_ABI_PKG} --target ${WASM_TARGET} --release --locked --target-dir ${target_dir}"
  env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS -u CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS \
    ${encoded_flags:+CARGO_ENCODED_RUSTFLAGS="${encoded_flags}"} \
    cargo build -p "${WASM_ABI_PKG}" --no-default-features --features contract-core,cpu-isa,cpu-ops \
    --target "${WASM_TARGET}" --release --locked --target-dir "${target_dir}"
  if [ ! -s "${artifact}" ]; then
    echo "error: wasm-abi ${variant}: artifact missing or empty" >&2
    exit 1
  fi
  if [ "$(od -An -tx1 -N4 "${artifact}" | tr -d ' \n')" != "0061736d" ]; then
    echo "error: wasm-abi ${variant}: artifact does not start with the wasm magic" >&2
    exit 1
  fi
}

wasm_target_dir="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
wasm_abi_build scalar "" "${wasm_target_dir}"
wasm_abi_build simd128 "$(printf -- '-C\037target-feature=+simd128')" "${wasm_target_dir}/wasm32-simd128"

log "build-profiles: all feature sets passed"
