#!/usr/bin/env bash
# プロファイルごとの feature の組で contract crate がビルドできることと、core crate が feature なし（試験の枠の
# test-support を外した形）で clippy と test を通ることを確かめる（非破壊。core の spec 対応は TASK-470）。
# native では build / clippy（-D warnings）/ test、wasm32 では check のみを回す。
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
  "test-support"                                                                   # spec TASK-24（検証用の道具。背後実装なし）
  "backend-metal,test-support"                                                     # spec TASK-24
  "backend-vulkan,test-support"                                                    # spec TASK-24
  "backend-cuda,test-support"                                                      # spec TASK-24
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

log "build-profiles: all feature sets passed"
