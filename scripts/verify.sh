#!/usr/bin/env bash
# Rust の品質ゲート（非破壊）: cargo fmt --check・clippy（-D warnings）・test。
# CI の verify ジョブも本スクリプトを呼ぶ（チェック内容の定義はここにのみ置く）。
# 呼び出し元: `make verify`。ソースは変更しない（整形は `cargo fmt --all` を別途実行する）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo

cd "${ROOT_DIR}"

log "cargo fmt --all --check"
cargo fmt --all --check

log "cargo clippy --workspace --all-targets --all-features -- -D warnings"
cargo clippy --workspace --all-targets --all-features -- -D warnings

log "cargo test --workspace --all-features"
cargo test --workspace --all-features
