#!/usr/bin/env bash
# wasm32 向けの別 workspace（wasm/）の品質ゲート（非破壊）: fmt --check・clippy（ホストと wasm32、-D warnings）・
# test（ホスト）・wasm32 の build（.wasm の存在確認）・依存の検査（ポインタ: TASK-22）。
# 呼び出し元: `make wasm-verify`。CI の deny ジョブも同じ make ターゲットを呼ぶ。
# ソースは変更しない。外部依存（wasm-bindgen）があるので、初回は取得にネットワークが要る
# （--locked で Cargo.lock の不一致を失敗させる）。wasm32 の target が無いときは skip せず失敗させる。
# 回帰テストは持たない（検査本体を持たず既存の道具を順に呼ぶ入口のため。measure-verify.sh と同じ）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

main() {
  reject_args "$(basename "$0")" "$@"
  require_cmd cargo
  require_cmd jq

  # ルートの rust-toolchain.toml を効かせるため、ルートから実行する。
  cd "${ROOT_DIR}"

  log "cargo fmt --all --check (wasm/)"
  cargo fmt --all --check --manifest-path wasm/Cargo.toml

  log "cargo clippy (wasm/, host) -D warnings"
  cargo clippy --manifest-path wasm/Cargo.toml --workspace --all-targets --all-features --locked -- -D warnings

  # wasm-bindgen の export は wasm32 のときだけ有効になるので、lint が届くのはこの実行だけ。
  log "cargo clippy (wasm/, wasm32-unknown-unknown) -D warnings"
  cargo clippy --manifest-path wasm/Cargo.toml --workspace --all-targets --all-features --locked \
    --target wasm32-unknown-unknown -- -D warnings

  log "cargo test (wasm/, host)"
  cargo test --manifest-path wasm/Cargo.toml --workspace --locked

  log "cargo build (wasm/, wasm32-unknown-unknown, profile wasm-size)"
  cargo build --manifest-path wasm/Cargo.toml --workspace --locked \
    --target wasm32-unknown-unknown --profile wasm-size

  # ビルドが終わっただけで成功扱いにせず、成果物の .wasm が実在して空でないことを確かめる。
  target_dir="$(cargo metadata --format-version 1 --no-deps --locked --manifest-path wasm/Cargo.toml | jq -r '.target_directory')"
  wasm_file="${target_dir}/wasm32-unknown-unknown/wasm-size/wasm_glue.wasm"
  if [ ! -s "${wasm_file}" ]; then
    echo "error: missing or empty build artifact: ${wasm_file}" >&2
    exit 1
  fi
  log "artifact: wasm_glue.wasm exists and is not empty"

  log "deps-check (wasm/)"
  "${DIR}/deps-check.sh" --allow-path-root "${ROOT_DIR}/crates" "${ROOT_DIR}/wasm/Cargo.toml"
}

main "$@"
