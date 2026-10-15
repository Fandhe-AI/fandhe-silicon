#!/usr/bin/env bash
# 測定専用の別 workspace（measure/）の品質ゲート（非破壊）: fmt --check・clippy（-D warnings）・test・依存の検査。
# 呼び出し元: `make measure-verify`。CI の deny ジョブも同じ make ターゲットを呼ぶ。
# ソースは変更しない。ネットワーク不要（--locked で Cargo.lock の不一致を失敗させる）。
# 回帰テストは持たない（verify.sh と同じく、検査本体を持たず既存の道具を順に呼ぶ入口のため）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

main() {
  reject_args "$(basename "$0")" "$@"
  require_cmd cargo

  # ルートの rust-toolchain.toml を効かせるため、ルートから実行する。
  cd "${ROOT_DIR}"

  log "cargo fmt --all --check (measure/)"
  cargo fmt --all --check --manifest-path measure/Cargo.toml

  log "cargo clippy (measure/) -D warnings"
  cargo clippy --manifest-path measure/Cargo.toml --workspace --all-targets --all-features --locked -- -D warnings

  log "cargo test (measure/)"
  cargo test --manifest-path measure/Cargo.toml --workspace --all-features --locked

  log "deps-check (measure/)"
  "${DIR}/deps-check.sh" "${ROOT_DIR}/measure/Cargo.toml"
}

main "$@"
