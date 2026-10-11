#!/usr/bin/env bash
# 依存の版指定の検査: 全依存が `=x.y.z` の完全固定か（spec TASK-8 のポインタ。方針は
# .claude/rules/dependency-policy.md）。cargo-deny の wildcards は `*` しか拒否しないため、
# `^`・`~`・範囲・部分版（"1.2" や "=1.2"）のすり抜けを本スクリプトで機械的に検出する。
# 呼び出し元: `make deps-check`（CI への組み込みは別 Issue）。
# 使い方: scripts/deps-check.sh [path/to/Cargo.toml]（省略時はルートの Cargo.toml。fixture 検証用）
#
# 判定は許可リスト方式: `=x.y.z`（pre-release / build metadata 付き完全一致を含む）と
# workspace メンバーへの path 依存だけを許可し、未知の形は違反にする（fail-closed）。
# 承認記録の無い依存追加の検出は、ここへ検査を足して violations を共用する想定の拡張点。
# 既知の限界: どの crate からも継承されない [workspace.dependencies] の項目は
# cargo metadata に現れず検査されない（依存グラフに入らないため影響なし。使われた時点で検査される）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [[ "$1" == -* ]]; }; then
  echo "usage: $(basename "$0") [path/to/Cargo.toml]" >&2
  exit 2
fi
manifest="${1:-${ROOT_DIR}/Cargo.toml}"
if [ ! -f "${manifest}" ]; then
  echo "error: manifest not found: ${manifest}" >&2
  exit 2
fi

require_cmd cargo
require_cmd jq
require_cmd python3
# TOML 解析に標準ライブラリの tomllib（Python 3.11 以上）を使うため、起動時に版を検証する。
if ! python3 -I -c 'import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)'; then
  echo "error: Python 3.11 or later is required (tomllib); run \`make doctor\` for details" >&2
  exit 2
fi

# Cargo は cwd の祖先の .cargo/config(.toml) を読むため、呼び出し元の cwd に左右されないよう
# manifest のあるディレクトリへ移ってから実行する（下の設定ファイル検査の起点とも一致させる）。
manifest_abs="$(cd "$(dirname "${manifest}")" && pwd)/$(basename "${manifest}")"
manifest_dir="$(dirname "${manifest_abs}")"
cd "${manifest_dir}"

metadata="$(cargo metadata --format-version 1 --no-deps --offline --locked --manifest-path "${manifest_abs}")"

# 版指定の違反を 1 件 1 行で出す。crate 名・req は非信頼文字列として jq 内でのみ扱う。
# path 依存は workspace メンバーの manifest ディレクトリと一致するものだけを免除する。
# exclude 済みの crate や別 workspace への path 依存は（--no-deps で版指定も見えないため）違反にする。
version_violations="$(jq -r '
  .workspace_members as $members
  | [.packages[] | select(.id as $id | $members | index($id)) | .manifest_path | sub("/Cargo\\.toml$"; "")] as $dirs
  | .packages[] | .name as $pkg | .dependencies[]
  | . as $d
  | (if ($d.path != null) and ($dirs | index($d.path)) then empty
      elif $d.path != null then "path dependency is not a workspace member"
      elif ($d.source // "" | startswith("git+")) then "git source"
      elif ($d.req | test("^=[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?(\\+[0-9A-Za-z.-]+)?$")) then empty
      else "not pinned to =x.y.z" end) as $why
  | "error: \($why): \($pkg) -> \($d.name) \"\($d.req)\" (kind=\($d.kind // "normal"), target=\($d.target // "all"))"
' <<<"${metadata}")"

# [patch] / [replace] は cargo metadata に現れず固定版を差し替えられるため、TOML を構造的に解析し、
# トップレベルの patch / replace キーの存在自体を拒否する（見出しの書き方 `["patch"...]`・`[patch . x]`・
# `patch = {...}` の揺れを正規表現で追わない）。解析できないファイルも fail-closed で違反にする。
patch_violations=""
root_dir="$(jq -r '.workspace_root' <<<"${metadata}")"
files=()
while IFS= read -r f; do files+=("${f}"); done < <(jq -r '.packages[].manifest_path' <<<"${metadata}")
files+=("${manifest_abs}" "${root_dir}/Cargo.toml" "${root_dir}/.cargo/config.toml" "${root_dir}/.cargo/config")
# Cargo が実際に読む設定: 実行ディレクトリから祖先へ向かう .cargo/config(.toml) と CARGO_HOME 直下の config(.toml)。
dir="${manifest_dir}"
while :; do
  files+=("${dir}/.cargo/config.toml" "${dir}/.cargo/config")
  [ "${dir}" = "/" ] && break
  dir="$(dirname "${dir}")"
done
cargo_home="${CARGO_HOME:-${HOME:-}/.cargo}"
if [ -n "${cargo_home}" ]; then
  files+=("${cargo_home}/config.toml" "${cargo_home}/config")
fi
for f in "${files[@]}"; do
  [ -f "${f}" ] || continue
  if ! out="$(python3 -I -c '
import sys, tomllib
try:
    with open(sys.argv[1], "rb") as fh:
        data = tomllib.load(fh)
except Exception as e:
    print("error: cannot parse TOML (%s): %s" % (type(e).__name__, sys.argv[1]))
    sys.exit(0)
if "patch" in data or "replace" in data:
    print("error: [patch]/[replace] section is not allowed: %s" % sys.argv[1])
' "${f}")"; then
    out="error: failed to inspect ${f}"
  fi
  [ -z "${out}" ] || patch_violations+="${out}"$'\n'
done

violations="$(printf '%s\n%s\n' "${version_violations}" "${patch_violations}" | sed '/^$/d' | sort -u)"
if [ -n "${violations}" ]; then
  echo "${violations}" >&2
  log "FAILED: $(echo "${violations}" | wc -l | tr -d ' ') violation(s)"
  exit 1
fi
log "ok: all dependency requirements are pinned to =x.y.z"
