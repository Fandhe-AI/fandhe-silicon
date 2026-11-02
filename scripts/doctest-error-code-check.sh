#!/usr/bin/env bash
# doctest の `compile_fail,E0xxx` が、期待したエラーコードで落ちていることを確かめる。
# stable の rustdoc は `compile_fail` のエラーコードを照合しない（unstable の扱い）ため、
# 通常の `cargo test` では別のエラーで落ちても doctest が合格する。
# `RUSTC_BOOTSTRAP=1` を付けた `cargo test --doc` では照合が効くので、その試験だけをここで流す。
# `RUSTC_BOOTSTRAP=1` は doctest を流す cargo の呼び出しにだけ付け、このスクリプトの外・ほかのビルドには漏らさない。
#
# 流す組は scripts/build-profiles.sh が core を試験する組（feature なし・test-support・graphics・両方）と、
# scripts/verify.sh の `--workspace --all-features`（contract の `memory_free_doctest` は test-support の裏にあるのでここで流れる）。
# 呼び出し元: `make doctest-error-code-check`（直接実行も可）。CI の `deny` ジョブも実行する。
# 回帰テスト: `scripts/test-doctest-error-code-check.sh`（`make test-doctest-error-code-check`）。
#
# 使い方:
#   scripts/doctest-error-code-check.sh                       本リポの組を順に流す
#   scripts/doctest-error-code-check.sh --manifest-path <p>   指定した Cargo.toml の crate だけを `--all-features` で 1 回流す（回帰テスト用）
# 終了コード: 0 全部通った / 2 誤用・前提の不足（cargo 無し）/ それ以外の非 0 は cargo test の終了コード（doctest が落ちた。コードの食い違いを含む）
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

readonly CORE_PKG="fandhe-silicon-core"

usage() {
  echo "usage: $(basename "$0") [--manifest-path <Cargo.toml>]" >&2
}

# doctest だけを、エラーコードの照合を効かせて流す。引数は `cargo test --doc` にそのまま渡す。
run_doc() {
  log "RUSTC_BOOTSTRAP=1 cargo test --doc $*"
  env RUSTC_BOOTSTRAP=1 cargo test --doc "$@"
}

main() {
  local manifest=""
  case "$#" in
    0) ;;
    2)
      if [ "$1" != "--manifest-path" ]; then
        usage
        exit 2
      fi
      manifest="$2"
      ;;
    *)
      usage
      exit 2
      ;;
  esac
  require_cmd cargo || exit 2

  if [ -n "${manifest}" ]; then
    run_doc --manifest-path "${manifest}" --all-features
    return
  fi

  cd "${ROOT_DIR}"
  run_doc -p "${CORE_PKG}"
  run_doc -p "${CORE_PKG}" --features test-support
  run_doc -p "${CORE_PKG}" --features graphics
  run_doc -p "${CORE_PKG}" --features test-support,graphics
  run_doc --workspace --all-features
  log "doctest error codes ok"
}

main "$@"
