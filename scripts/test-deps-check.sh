#!/usr/bin/env bash
# deps-check.sh の回帰テスト: Cargo が読む祖先・CARGO_HOME の設定の [patch] を拒否することを確かめる。
# 呼び出し元: 手動（`scripts/test-deps-check.sh`）。CI への組み込みは別 Issue。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

mkdir -p "${work}/ws/crates/core/src" "${work}/home"
cat > "${work}/ws/Cargo.toml" <<'T'
[workspace]
members = ["crates/core"]
resolver = "3"
T
cat > "${work}/ws/crates/core/Cargo.toml" <<'T'
[package]
name = "fixture-core"
version = "0.0.0"
edition = "2024"
T
: > "${work}/ws/crates/core/src/lib.rs"
(cd "${work}/ws" && cargo generate-lockfile --offline > /dev/null 2>&1)

fail=0
# expect <説明> <期待する終了コード>: メンバー crate の cwd から deps-check を実行して終了コードを比べる。
expect() {
  local desc="$1" want="$2" rc=0
  (cd "${work}/ws/crates/core" && CARGO_HOME="${work}/home" "${DIR}/deps-check.sh" "${work}/ws/Cargo.toml") > /dev/null 2>&1 || rc=$?
  if [ "${rc}" -ne "${want}" ]; then
    echo "FAIL: ${desc} (exit ${rc}, want ${want})" >&2
    fail=1
  else
    echo "ok: ${desc}"
  fi
}

expect "clean fixture passes" 0

mkdir -p "${work}/.cargo"
printf '[patch.crates-io]\nfoo = { path = "x" }\n' > "${work}/.cargo/config.toml"
expect "ancestor .cargo/config.toml [patch] is rejected" 1
rm -rf "${work}/.cargo"

printf '[patch.crates-io]\nfoo = { path = "x" }\n' > "${work}/home/config.toml"
expect "CARGO_HOME config [patch] is rejected" 1
rm -f "${work}/home/config.toml"

exit "${fail}"
