#!/usr/bin/env bash
# 比較基準の別 workspace（baseline-wgpu/）の品質ゲート（非破壊）: fmt --check・clippy（-D warnings）・test。
# 呼び出し元: `make baseline-verify`。CI の deny ジョブも同じ make ターゲットを呼ぶ。
# ソースは変更しない。ネットワーク不要（--locked で Cargo.lock の不一致を失敗させる）。
# 依存の検査（deps-check・cargo-deny）は `make deny` が呼ぶので、ここでは流さない。
# 回帰テストは持たない（measure-verify.sh と同じく、検査本体を持たず既存の道具を順に呼ぶ入口のため）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

main() {
  reject_args "$(basename "$0")" "$@"
  require_cmd cargo

  # ルートの rust-toolchain.toml を効かせるため、ルートから実行する。
  cd "${ROOT_DIR}"

  log "cargo fmt --all --check (baseline-wgpu/)"
  cargo fmt --all --check --manifest-path baseline-wgpu/Cargo.toml

  log "cargo clippy (baseline-wgpu/) -D warnings"
  cargo clippy --manifest-path baseline-wgpu/Cargo.toml --workspace --all-targets --locked -- -D warnings

  log "cargo test (baseline-wgpu/)"
  cargo test --manifest-path baseline-wgpu/Cargo.toml --workspace --locked
}

main "$@"
