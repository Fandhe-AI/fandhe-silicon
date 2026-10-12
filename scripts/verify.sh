#!/usr/bin/env bash
# Rust の品質ゲート（非破壊）: cargo fmt --check・clippy（-D warnings）・test。
# CI の verify ジョブも本スクリプトを呼ぶ（チェック内容の定義はここにのみ置く）。
# 呼び出し元: `make verify`。ソースは変更しない（整形は `cargo fmt --all` を別途実行する）。
#
# `cargo test` の 1 回だけ、試験の枠（spec TASK-23 のポインタ）の記録の部品を新しい一時ディレクトリへ書かせ、
# 試験の後に scripts/record-summary.sh で集計の表を標準出力に出して、ディレクトリを消す（CI ではジョブのログに出る）。
# 部品のファイルは一時ディレクトリの中だけに置き、artifact にも PR の Summary にも出さない。
# 回帰テスト `scripts/test-record-summary.sh` が、ここの関数を source して偽の cargo で確かめる。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# workspace の試験を流し、枠の記録から集計を出す。
# 部品の書き出し先は環境変数 FANDHE_SILICON_TEST_RECORD_DIR（crates/core/src/test_frame/export.rs の RECORD_DIR_ENV）。
# - 呼び出し側の環境に既に在る（空文字を含む）ときは上書きせず、そのまま渡す。
#   集計も片づけもしない（手順のスクリプトなど、呼び出し側が部品を持ち帰る場合）。
# - 無いときは新しい一時ディレクトリを作って渡し、試験のあとに集計を出して消す（試験が失敗しても消す）。
# 終了コード: 試験が失敗したときはその終了コード。試験が通ったときは集計の終了コード（部品が 0 件・形の違反は非 0）。
run_cargo_test_with_record_summary() {
  if [ "${FANDHE_SILICON_TEST_RECORD_DIR+set}" = set ]; then
    cargo test --workspace --all-features
    return
  fi
  (
    dir="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-silicon-test-records.XXXXXX")"
    trap 'rm -rf "${dir:?}"' EXIT
    trap 'exit 1' INT TERM HUP
    test_rc=0
    env "FANDHE_SILICON_TEST_RECORD_DIR=${dir}" cargo test --workspace --all-features || test_rc=$?
    summary_rc=0
    log "test record summary"
    "${DIR}/record-summary.sh" "${dir}" || summary_rc=$?
    if [ "${test_rc}" -ne 0 ]; then
      exit "${test_rc}"
    fi
    exit "${summary_rc}"
  )
}

main() {
  reject_args "$(basename "$0")" "$@"
  require_cmd cargo

  cd "${ROOT_DIR}"

  log "cargo fmt --all --check"
  cargo fmt --all --check

  log "cargo clippy --workspace --all-targets --all-features -- -D warnings"
  cargo clippy --workspace --all-targets --all-features -- -D warnings

  log "cargo test --workspace --all-features"
  run_cargo_test_with_record_summary
}

# source されたとき（回帰テスト）は main を走らせない。
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  main "$@"
fi
