#!/usr/bin/env bash
# 開発環境の診断のみを行う。導入・更新・修復は一切しない。
# 必須ツールが欠けていれば非 0 で終了し、次の行動は呼び出し元（人・`make doctor`）が決める。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

status=0

# check_tool <required|optional> <name> <version-command...>
check_tool() {
  local level="$1" name="$2"
  shift 2
  if command -v "${name}" >/dev/null 2>&1; then
    echo "ok   ${name}: $("$@" 2>&1 | head -n1)"
  elif [ "${level}" = "required" ]; then
    echo "miss ${name}: not found on PATH (required)"
    status=1
  else
    echo "skip ${name}: not found on PATH (optional)"
  fi
}

echo "environment diagnosis for ${ROOT_DIR} (read-only, no changes made)"
check_tool required git git --version
check_tool required make make --version
check_tool required lefthook lefthook version
check_tool required editorconfig-checker editorconfig-checker --version
check_tool required shellcheck sh -c 'shellcheck --version | grep "^version:"'
# rustup のプロキシ（cargo・rustc）や rustup 自身は、rust-toolchain.toml が効くディレクトリで
# 実行すると未導入の toolchain を自動導入する（rustup 1.29 時点。RUSTUP_AUTO_INSTALL=0 でも
# 止まらないことを確認済み）。診断で導入を起こさないよう、toolchain の override が効かない
# `/` で問い合わせる。
check_tool required rustup sh -c 'cd / && rustup --version 2>/dev/null'
check_tool required cargo sh -c 'cd / && cargo --version'
check_tool optional cargo-deny cargo-deny --version
check_tool optional jq jq --version
# python3 は `make verify` の試験の記録の集計（scripts/record-summary.sh。3.8 以上）が要るので必須。
# deps-check は tomllib（Python 3.11 以上）を使う。3.11 に足りなければ note で知らせる（deps-check 側は任意扱い）。
if command -v python3 >/dev/null 2>&1; then
  if ! python3 -I -c 'import sys; sys.exit(0 if sys.version_info >= (3, 8) else 1)'; then
    echo "miss python3: $(python3 --version 2>&1 | head -n1) is older than 3.8 (required by \`make verify\` for the test record summary)"
    status=1
  elif python3 -I -c 'import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)'; then
    echo "ok   python3: $(python3 --version 2>&1 | head -n1)"
  else
    echo "note python3: $(python3 --version 2>&1 | head -n1) is older than 3.11; \`make deps-check\` needs 3.11 or later"
  fi
else
  echo "miss python3: not found on PATH (required: 3.8 or later for \`make verify\` test record summary; 3.11 or later for \`make deps-check\`; also used by \`make inventory-trace-check\` / \`make test-inventory-trace-check\`, \`scripts/session-record-check.sh\`, \`scripts/metal-session.sh\`, \`scripts/ci-frame-check.sh\`, \`scripts/public-api-check.sh\`, \`scripts/public-name-check.sh\`, \`scripts/dep-graph-wgpu-check.sh\` (called by \`make deny\`), \`scripts/upper-layer-check.sh\`, \`scripts/public-api-diff.sh\`, \`scripts/rerun-compare.sh\` and their tests)"
  status=1
fi

# rust-toolchain.toml の channel が導入済みかを、導入済み一覧との突き合わせだけで判定する
toolchain_file="${ROOT_DIR}/rust-toolchain.toml"
if [ -f "${toolchain_file}" ] && command -v rustup >/dev/null 2>&1; then
  channel="$(sed -n 's/^channel *= *"\([^"]*\)".*/\1/p' "${toolchain_file}" | head -n1)"
  if [ -z "${channel}" ]; then
    echo "miss rust-toolchain: channel not found in rust-toolchain.toml"
    status=1
  elif (cd / && rustup toolchain list) | grep -q "^${channel}-"; then
    echo "ok   rust-toolchain: ${channel} is installed"
  else
    echo "miss rust-toolchain: ${channel} is not installed; run \`rustup toolchain install ${channel}\`"
    status=1
  fi
fi
check_tool optional direnv direnv version

if [ -f "${ROOT_DIR}/.env" ]; then
  echo "ok   .env: present (values are not displayed)"
else
  echo "note .env: not found; \`make setup\` copies .env.example to .env"
fi

if [ "${status}" -ne 0 ]; then
  echo "one or more required tools are missing; see README.md \"必要なツール\" for install instructions" >&2
fi

exit "${status}"
