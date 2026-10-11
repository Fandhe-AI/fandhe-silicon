#!/usr/bin/env bash
# deps-check.sh の回帰テスト: Cargo が読む祖先・CARGO_HOME の設定の [patch] の拒否と、
# 承認の記録（deps-approved.toml）の最小ケースを確かめる。
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
# 承認の記録は workspace ルート直下の deps-approved.toml（空の記録 = 外部依存 0 件の承認）。
: > "${work}/ws/deps-approved.toml"

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

# 承認の記録の最小ケース（網羅的な回帰テストは別 Issue）。依存名はダミーで、manifest だけ読むので実在不要。
# Cargo.lock は外部依存の宣言と食い違うため、これ以降は消して検査する。
rm -f "${work}/ws/Cargo.lock"
printf '[dependencies]\nexample-ext = "=1.0.0"\n' >> "${work}/ws/crates/core/Cargo.toml"
expect "pinned external dependency without approval record is rejected" 1

cat > "${work}/ws/deps-approved.toml" <<'T'
[[approval]]
name = "example-ext"
version = "=1.0.0"
used_by = ["fixture-core"]
approved_on = "2000-01-01"
approval_ref = "#0"
purpose = "fixture"
T
expect "matching approval record passes" 0

sed -i 's/"=1.0.0"/"=2.0.0"/' "${work}/ws/deps-approved.toml"
expect "approval record with a different version is rejected" 1

sed -i 's/"=2.0.0"/"=1.0.0"/; s/"fixture-core"/"other-member"/' "${work}/ws/deps-approved.toml"
expect "approval record whose used_by lacks the declaring member is rejected" 1

sed -i 's/"other-member"/"fixture-core"/' "${work}/ws/deps-approved.toml"
printf '[package]\nname = "fixture-core"\nversion = "0.0.0"\nedition = "2024"\n' > "${work}/ws/crates/core/Cargo.toml"
expect "unused approval record is rejected" 1

rm -f "${work}/ws/deps-approved.toml"
expect "missing approval record file is rejected" 1

exit "${fail}"
