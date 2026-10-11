#!/usr/bin/env bash
# deps-check.sh の回帰テスト（spec TASK-8 / AP-8 のポインタ）。
# 版指定の未固定・取得元・[patch] / [replace]・承認の記録（deps-approved.toml）の各検査が
# 実際に失敗として働くことを、一時 workspace の fixture で示す。実物の違反はコミットしない。
# 呼び出し元: `make test-deps-check`（直接実行も可。CI の `deny` ジョブも `make test-deps-check` を呼ぶ）。
#
# 判定は「終了コード + stderr のメッセージの部分一致 + FAILED の件数」で行う。終了コードだけでは
# 未固定の依存が「未固定」と「承認の記録なし」の 2 件で落ちる区別が付かず、検査が素通りする退行
# （fail-open）を見逃すため。依存名はダミーで実在不要、cargo は --offline 相当で動くのでネットワーク不要。
# 期待値を緩めない。現行の deps-check.sh と食い違ったらテストでなく検査側の挙動として報告する。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd cargo
require_cmd jq
require_cmd python3
if ! python3 -I -c 'import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)'; then
  echo "error: Python 3.11 or later is required (tomllib); run \`make doctor\` for details" >&2
  exit 2
fi

# macOS の /var -> /private/var のように cargo が出すパスと食い違わないよう実パスにする。
work="$(mktemp -d "${TMPDIR:-/tmp}/test-deps-check.XXXXXX")"
work="$(cd "${work}" && pwd -P)"
trap 'rm -rf "${work:?}"' EXIT
ws="${work}/ws"
passed=0
failed=0
rc=0
run_cwd=""

# ---- fixture ----

# new_fixture: ケースごとに初期化して順序依存を無くす。member は fixture-core / fixture-other。
new_fixture() {
  rm -rf "${work:?}/ws" "${work:?}/home" "${work:?}/.cargo" "${work:?}/outside"
  mkdir -p "${ws}/crates/core/src" "${ws}/crates/other/src" "${work}/home"
  cat > "${ws}/Cargo.toml" <<'T'
[workspace]
members = ["crates/core", "crates/other"]
resolver = "3"
T
  local m
  for m in core other; do
    printf '[package]\nname = "fixture-%s"\nversion = "0.0.0"\nedition = "2024"\n' "${m}" > "${ws}/crates/${m}/Cargo.toml"
    : > "${ws}/crates/${m}/src/lib.rs"
  done
  : > "${ws}/deps-approved.toml"
  run_cwd="${ws}/crates/core"
}

# add_dep <core|other> <行...>: member の Cargo.toml へ追記する。
add_dep() {
  local m="$1"
  shift
  printf '%s\n' "$@" >> "${ws}/crates/${m}/Cargo.toml"
}

# approve <name> <version> <used_by...>: 有効な記録を 1 件追記する（値はダミー）。
approve() {
  local name="$1" version="$2" ub="" u
  shift 2
  for u in "$@"; do ub="${ub:+${ub}, }\"${u}\""; done
  {
    printf '[[approval]]\nname = "%s"\nversion = "%s"\nused_by = [%s]\n' "${name}" "${version}" "${ub}"
    printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "fixture"\n\n'
  } >> "${ws}/deps-approved.toml"
}

# ---- 実行と判定 ----

# run_check [manifest]: 既定は member の cwd から ws/Cargo.toml の絶対パスで実行する。
run_check() {
  local manifest="${1:-${ws}/Cargo.toml}"
  rc=0
  (cd "${run_cwd}" && CARGO_HOME="${work}/home" "${DIR}/deps-check.sh" "${manifest}") > "${work}/stdout" 2> "${work}/stderr" || rc=$?
}

report_fail() {
  failed=$((failed + 1))
  {
    echo "FAIL: $1"
    echo "  reason: $2 (exit ${rc})"
    sed 's/^/  stderr| /' "${work}/stderr"
  } >&2
}
report_ok() {
  passed=$((passed + 1))
  echo "ok: $1"
}

# has <部分文字列>: stderr に固定文字列があるか（grep のエラーは不一致と同じ失敗扱い）。
has() { grep -qF -- "$1" "${work}/stderr"; }

# check_subs <説明> <部分文字列...>: 先頭 ! は「stderr に無いこと」（grep の終了状態 1 のときだけ合格）。
check_subs() {
  local desc="$1" s g
  shift
  for s in "$@"; do
    if [ "${s#!}" != "${s}" ]; then
      g=0
      grep -qF -- "${s#!}" "${work}/stderr" || g=$?
      if [ "${g}" -ne 1 ]; then report_fail "${desc}" "stderr must not contain: ${s#!}"; return; fi
    elif ! has "${s}"; then
      report_fail "${desc}" "stderr lacks: ${s}"
      return
    fi
  done
  report_ok "${desc}"
}

expect_ok() {
  local desc="$1"
  if [ "${rc}" -ne 0 ]; then report_fail "${desc}" "want exit 0"; return; fi
  if grep -q '^error:' "${work}/stderr"; then report_fail "${desc}" "unexpected error line"; return; fi
  check_subs "${desc}" "ok: all dependency requirements are pinned"
}

# expect_violations <説明> <件数> <部分文字列...>
expect_violations() {
  local desc="$1" n="$2"
  shift 2
  if [ "${rc}" -ne 1 ]; then report_fail "${desc}" "want exit 1"; return; fi
  check_subs "${desc}" "FAILED: ${n} violation(s)" "$@"
}

# expect_usage <説明> <部分文字列>
expect_usage() {
  local desc="$1"
  if [ "${rc}" -ne 2 ]; then report_fail "${desc}" "want exit 2"; return; fi
  check_subs "${desc}" "$2"
}

# expect_refused <説明>: cargo 自身が manifest・設定を拒否し、deps-check の判定まで進まなかった。
# 通常の違反（終了コード 1 と FAILED の行）と区別する。cargo の終了コードの値には結び付けない。
expect_refused() {
  local desc="$1"
  if [ "${rc}" -eq 0 ]; then report_fail "${desc}" "want non-zero exit"; return; fi
  if has "ok: all dependency"; then report_fail "${desc}" "must not report ok"; return; fi
  if has "[deps-check.sh] FAILED:"; then report_fail "${desc}" "deps-check reached its verdict (want cargo to refuse earlier)"; return; fi
  report_ok "${desc}"
}

# ---- A. 基本・誤用 ----

new_fixture; run_check; expect_ok "A1 no external dependencies and empty record"

new_fixture; printf '# no approvals\n' > "${ws}/deps-approved.toml"; run_check; expect_ok "A2a comment-only record file"
new_fixture; printf 'approval = []\n' > "${ws}/deps-approved.toml"; run_check; expect_ok "A2b approval = []"

new_fixture
rc=0; (cd "${ws}" && CARGO_HOME="${work}/home" "${DIR}/deps-check.sh" -x) > "${work}/stdout" 2> "${work}/stderr" || rc=$?
expect_usage "A3 option-like argument" "usage:"
rc=0; (cd "${ws}" && CARGO_HOME="${work}/home" "${DIR}/deps-check.sh" a b) > "${work}/stdout" 2> "${work}/stderr" || rc=$?
expect_usage "A4 two arguments" "usage:"
run_check "${ws}/missing/Cargo.toml"
expect_usage "A5 missing manifest" "error: manifest not found:"

new_fixture; run_cwd="${ws}"; run_check "Cargo.toml"; expect_ok "A6 relative manifest from workspace root"

new_fixture; add_dep core '[dependencies]' 'example-ext = "^1.2"'
run_check "${ws}/crates/core/Cargo.toml"
expect_violations "A7 member manifest passed directly" 2 "not pinned to =x.y.z"

# ---- B. 版指定の未固定（未固定 1 件 = 「未固定」+「記録なし」の 2 件。記録で正当化もできない） ----

# b_case <ID> <宣言(TOML 値)> <cargo が正規化した req> [section] [kind] [target]
b_case() {
  local id="$1" decl="$2" req="$3" sect="${4:-[dependencies]}" kind="${5:-normal}" target="${6:-all}"
  new_fixture
  add_dep core "${sect}" "example-ext = ${decl}"
  run_check
  expect_violations "${id} unpinned ${decl}" 2 \
    "error: not pinned to =x.y.z: fixture-core -> example-ext \"${req}\" (kind=${kind}, target=${target})" \
    "error: no approval record: fixture-core -> example-ext \"${req}\""
}
b_case B1 '"^1.2"' '^1.2'
b_case B2 '"~1.2.3"' '~1.2.3'
b_case B3 '"*"' '*'
b_case B4 '"=1.2"' '=1.2'
b_case B5 '">=1, <2"' '>=1, <2'
b_case B6 '"1.2.3"' '^1.2.3'
b_case B7 '"=1"' '=1'
b_case B8 '"=1.2.3, <2"' '=1.2.3, <2'
b_case B9 '"^1"' '^1' '[dev-dependencies]' dev
b_case B10 '"^1"' '^1' '[build-dependencies]' build
b_case B11 '"^1"' '^1' "[target.'cfg(unix)'.dependencies]" normal 'cfg(unix)'
b_case B12 '{ version = "^1", optional = true }' '^1'

new_fixture
printf '\n[workspace.dependencies]\nexample-ext = "1.2"\n' >> "${ws}/Cargo.toml"
add_dep core '[dependencies]' 'example-ext.workspace = true'
run_check
expect_violations "B13 unpinned workspace.dependencies inherited" 2 \
  'error: not pinned to =x.y.z: fixture-core -> example-ext "^1.2"' \
  'error: no approval record: fixture-core -> example-ext "^1.2"'

new_fixture
add_dep core '[dependencies]' 'bad-one = "^1"' 'bad-two = "~2.0"' 'good-ext = "=3.0.0"'
approve good-ext '=3.0.0' fixture-core
run_check
expect_violations "B14 only unpinned declarations are reported" 4 \
  'not pinned to =x.y.z: fixture-core -> bad-one "^1"' \
  'not pinned to =x.y.z: fixture-core -> bad-two "~2.0"' \
  '!good-ext'

new_fixture
add_dep core '[dependencies]' 'example-ext = "^1.2"'
approve example-ext '^1.2' fixture-core
run_check
expect_violations "B15 record cannot justify an unpinned requirement" 3 \
  'error: not pinned to =x.y.z: fixture-core -> example-ext "^1.2"' \
  'invalid version'

# ---- C. 許可される形（対応する記録あり） ----

# c_case <ID> <宣言> <記録の版> [section]
c_case() {
  local id="$1" decl="$2" ver="$3" sect="${4:-[dependencies]}"
  new_fixture
  add_dep core "${sect}" "example-ext = ${decl}"
  approve example-ext "${ver}" fixture-core
  run_check
  expect_ok "${id} pinned ${decl}"
}
c_case C1 '"=1.2.3"' '=1.2.3'
c_case C2 '"=1.2.3-rc.1"' '=1.2.3-rc.1'
c_case C3 '"=0.0.0"' '=0.0.0'
c_case C4 '"=1.0.0"' '=1.0.0' '[dev-dependencies]'
c_case C5 '"=1.0.0"' '=1.0.0' '[build-dependencies]'
c_case C6 '"=1.0.0"' '=1.0.0' "[target.'cfg(unix)'.dependencies]"
c_case C7 '{ version = "=1.0.0", optional = true }' '=1.0.0' '[dependencies]'

new_fixture
printf '\n[workspace.dependencies]\nexample-ext = "=2.0.0"\n' >> "${ws}/Cargo.toml"
add_dep core '[dependencies]' 'example-ext.workspace = true'
approve example-ext '=2.0.0' fixture-core
run_check; expect_ok "C8 pinned workspace.dependencies inherited"

new_fixture
add_dep core '[dependencies]' 'example-ext = "=1.0.0"' '[dev-dependencies]' 'example-ext = "=1.0.0"'
approve example-ext '=1.0.0' fixture-core
run_check; expect_ok "C9 same dependency as normal and dev"

new_fixture
add_dep core '[dependencies]' 'fixture-other = { path = "../other" }'
run_check; expect_ok "C10 path dependency on a workspace member needs no record"

# cargo は build metadata を落として =1.2.3 に正規化するため、記録は =1.2.3 と書く（現状の挙動を固定）。
new_fixture
add_dep core '[dependencies]' 'example-ext = "=1.2.3+build.5"'
approve example-ext '=1.2.3' fixture-core
run_check; expect_ok "C11 build metadata is normalized away by cargo"

# ---- D. 取得元 ----

new_fixture
add_dep core '[dependencies]' 'example-ext = { git = "https://example.invalid/example-ext.git" }'
run_check
expect_violations "D1 git source" 2 'error: git source: fixture-core -> example-ext "*"'

new_fixture
add_dep core '[dependencies]' 'example-ext = { git = "https://example.invalid/example-ext.git", version = "=1.0.0" }'
approve example-ext '=1.0.0' fixture-core
run_check
expect_violations "D2 git source is rejected even when pinned and approved" 1 'git source'

new_fixture
mkdir -p "${work}/outside/src"
printf '[package]\nname = "outside-crate"\nversion = "0.0.0"\nedition = "2024"\n' > "${work}/outside/Cargo.toml"
: > "${work}/outside/src/lib.rs"
add_dep core '[dependencies]' 'outside-crate = { path = "../../../outside" }'
run_check
expect_violations "D3 path dependency outside the workspace" 2 \
  'path dependency is not a workspace member: fixture-core -> outside-crate "*"'

new_fixture
mkdir -p "${ws}/crates/excluded/src"
printf '[package]\nname = "excluded-crate"\nversion = "0.0.0"\nedition = "2024"\n' > "${ws}/crates/excluded/Cargo.toml"
: > "${ws}/crates/excluded/src/lib.rs"
printf '[workspace]\nmembers = ["crates/core", "crates/other"]\nexclude = ["crates/excluded"]\nresolver = "3"\n' > "${ws}/Cargo.toml"
add_dep core '[dependencies]' 'excluded-crate = { path = "../excluded" }'
run_check
expect_violations "D4 path dependency on an excluded crate" 2 \
  'path dependency is not a workspace member: fixture-core -> excluded-crate "*"'

new_fixture
add_dep core '[dependencies]' 'ghost = { path = "../ghost" }'
run_check; expect_refused "D5 path dependency to a missing directory"

new_fixture
add_dep core '[dependencies]' 'example-ext = "==1.2.3"'
run_check; expect_refused "D6 uninterpretable version requirement"

# ---- E. [patch] / [replace]（設定ファイルの解析も含む） ----

PATCH_BODY='[patch.crates-io]
example-ext = { path = "crates/other" }'
E_MSG='error: [patch]/[replace] section is not allowed:'

new_fixture; printf '\n%s\n' "${PATCH_BODY}" >> "${ws}/Cargo.toml"
run_check; expect_violations "E1 root [patch.crates-io]" 1 "${E_MSG}"

new_fixture; printf '\n[replace]\n"example-ext:1.0.0" = { path = "crates/other" }\n' >> "${ws}/Cargo.toml"
run_check; expect_violations "E2 root [replace]" 1 "${E_MSG}"

new_fixture
{
  printf 'patch = { crates-io = { example-ext = { path = "crates/other" } } }\n\n'
  cat "${ws}/Cargo.toml"
} > "${work}/root.toml"
mv "${work}/root.toml" "${ws}/Cargo.toml"
run_check; expect_violations "E3 root inline patch = {...}" 1 "${E_MSG}"

new_fixture; add_dep core "${PATCH_BODY}"
run_check; expect_violations "E4 member [patch.crates-io]" 1 "${E_MSG}"

new_fixture; mkdir -p "${ws}/.cargo"; printf '%s\n' "${PATCH_BODY}" > "${ws}/.cargo/config.toml"
run_check; expect_violations "E5 workspace .cargo/config.toml [patch]" 1 "${E_MSG}"

# E6 / E8 / E13 は cargo が拡張子なしの .cargo/config をまだ受け付けること（現状は非推奨の warning）に依存する。
# cargo がこれをエラーにすると refused 側に変わり、この 3 件が落ちる（原因は cargo の挙動変更）。
new_fixture; mkdir -p "${ws}/.cargo"; printf '%s\n' "${PATCH_BODY}" > "${ws}/.cargo/config"
run_check; expect_violations "E6 workspace .cargo/config (no extension) [patch]" 1 "${E_MSG}"

new_fixture; mkdir -p "${work}/.cargo"; printf '%s\n' "${PATCH_BODY}" > "${work}/.cargo/config.toml"
run_check; expect_violations "E7 ancestor .cargo/config.toml [patch]" 1 "${E_MSG}"

new_fixture; mkdir -p "${work}/.cargo"; printf '%s\n' "${PATCH_BODY}" > "${work}/.cargo/config"
run_check; expect_violations "E8 ancestor .cargo/config (no extension) [patch]" 1 "${E_MSG}"

new_fixture; printf '%s\n' "${PATCH_BODY}" > "${work}/home/config.toml"
run_check; expect_violations "E9 CARGO_HOME config.toml [patch]" 1 "${E_MSG}"

new_fixture; printf '[replace]\n"example-ext:1.0.0" = { path = "x" }\n' > "${work}/home/config.toml"
run_check; expect_violations "E10 CARGO_HOME config.toml [replace]" 1 "${E_MSG}"

new_fixture; mkdir -p "${ws}/.cargo"; printf '[build]\njobs = 1\n' > "${ws}/.cargo/config.toml"
run_check; expect_ok "E11 harmless .cargo/config.toml"

new_fixture; mkdir -p "${ws}/.cargo"; printf 'not = valid = toml\n' > "${ws}/.cargo/config.toml"
run_check; expect_refused "E12 unparsable .cargo/config.toml is refused by cargo"

# cargo は拡張子なしの config を優先して読むため、壊れた config.toml は検査側の fail-closed の経路に届く。
new_fixture; mkdir -p "${ws}/.cargo"
printf '[build]\njobs = 1\n' > "${ws}/.cargo/config"
printf 'not = valid = toml\n' > "${ws}/.cargo/config.toml"
run_check; expect_violations "E13 unparsable config.toml next to a valid config" 1 'error: cannot parse TOML ('

# ---- F. 承認の記録 ----

# 固定済みの依存 1 件（記録なし）を宣言した fixture を作る。
pinned_fixture() { new_fixture; add_dep core '[dependencies]' 'example-ext = "=1.0.0"'; }

pinned_fixture; run_check
expect_violations "F1 pinned dependency without a record" 1 'no approval record: fixture-core -> example-ext "=1.0.0"'

pinned_fixture; approve example-ext '=1.0.0' fixture-core; run_check
expect_ok "F2 matching record"

pinned_fixture; approve example-ext '=2.0.0' fixture-core; run_check
expect_violations "F3 record version differs" 1 \
  'approved version differs: fixture-core -> example-ext declares "=1.0.0", approved "=2.0.0"'

new_fixture; add_dep core '[dependencies]' 'example-ext = "=1.0.0"'; add_dep other '[dependencies]' 'example-ext = "=1.0.0"'
approve example-ext '=1.0.0' fixture-core fixture-other; run_check
expect_ok "F4 two members listed in used_by"

new_fixture; add_dep core '[dependencies]' 'example-ext = "=1.0.0"'; add_dep other '[dependencies]' 'example-ext = "=1.0.0"'
approve example-ext '=1.0.0' fixture-core; run_check
expect_violations "F5 member missing from used_by" 1 'member not listed in used_by: fixture-other -> example-ext'

pinned_fixture; approve example-ext '=1.0.0' fixture-core fixture-other; run_check
expect_violations "F6 used_by lists a member that does not declare it" 1 'unused approval record: example-ext (used_by fixture-other)'

new_fixture; approve example-ext '=1.0.0' fixture-core; run_check
expect_violations "F7 record without a declaration" 1 'unused approval record: example-ext (used_by fixture-core)'

new_fixture; add_dep core '[dependencies]' 'example-ext = "=1.0.0"'; add_dep other '[dependencies]' 'example-ext = "=2.0.0"'
approve example-ext '=1.0.0' fixture-core fixture-other; run_check
expect_violations "F8 version differs per member" 1 \
  'approved version differs: fixture-other -> example-ext declares "=2.0.0", approved "=1.0.0"'

new_fixture; add_dep core '[dependencies]' 'example-ext = "=1.0.0"' 'second-ext = "=3.0.0"'
approve example-ext '=1.0.0' fixture-core; run_check
expect_violations "F9 only one of two dependencies is recorded" 1 'no approval record: fixture-core -> second-ext "=3.0.0"'

new_fixture; add_dep core '[dependencies]' 'alias-ext = { package = "example-ext", version = "=1.0.0" }'
approve alias-ext '=1.0.0' fixture-core; run_check
expect_violations "F10 record under the alias instead of the real name" 2 \
  'no approval record: fixture-core -> example-ext "=1.0.0"' 'unused approval record: alias-ext'

new_fixture; add_dep core '[dependencies]' 'alias-ext = { package = "example-ext", version = "=1.0.0" }'
approve example-ext '=1.0.0' fixture-core; run_check
expect_ok "F11 renamed dependency recorded under the real name"

new_fixture; add_dep core '[dev-dependencies]' 'example-ext = "=1.0.0"'; run_check
expect_violations "F12 dev-dependency also needs a record" 1 'no approval record'

new_fixture; rm -f "${ws}/deps-approved.toml"; run_check
expect_violations "F13 record file missing (even with no external dependency)" 1 'approval record file not found:'

new_fixture; printf 'not = valid = toml\n' > "${ws}/deps-approved.toml"; run_check
expect_violations "F14 unparsable record file" 1 'cannot parse approval record file ('

new_fixture; printf '\377\376\000\n' > "${ws}/deps-approved.toml"; run_check
expect_violations "F15a non-UTF-8 record file" 1 'cannot parse approval record file ('

pinned_fixture; rm -f "${ws}/deps-approved.toml"; mkdir "${ws}/deps-approved.toml"; run_check
expect_violations "F15b record path is a directory" 2 'cannot parse approval record file ('

# f16 <理由>: 標準入力の記録（固定済みの依存 1 件を宣言）が書式違反として落ちる。書式違反 + 記録なしの 2 件。
# 呼び出しは < <(...) で行う（パイプだとサブシェルになり passed / failed がメインシェルへ戻らない）。
f16() {
  local reason="$1"
  pinned_fixture
  cat > "${ws}/deps-approved.toml"
  run_check
  expect_violations "F16 malformed record: ${reason}" 2 \
    "malformed approval record (approval #1: ${reason}" 'no approval record: fixture-core -> example-ext "=1.0.0"'
}
BASE_HEAD='[[approval]]
name = "example-ext"
version = "=1.0.0"
used_by = ["fixture-core"]'
f16 'missing key purpose' < <({ printf '%s\n' "${BASE_HEAD}"; printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\n'; })
f16 'unknown key note' < <({ printf '%s\n' "${BASE_HEAD}"; printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "fixture"\nnote = "x"\n'; })
f16 'invalid approval_ref' < <({ printf '%s\n' "${BASE_HEAD}"; printf 'approved_on = "2000-01-01"\napproval_ref = "PR 12"\npurpose = "fixture"\n'; })
f16 'invalid approved_on' < <({ printf '%s\n' "${BASE_HEAD}"; printf 'approved_on = "2000/01/01"\napproval_ref = "#0"\npurpose = "fixture"\n'; })
f16 'invalid approved_on' < <({ printf '%s\n' "${BASE_HEAD}"; printf 'approved_on = 2000-01-01\napproval_ref = "#0"\npurpose = "fixture"\n'; })
f16 'invalid used_by' < <({ printf '[[approval]]\nname = "example-ext"\nversion = "=1.0.0"\nused_by = []\n'; printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "fixture"\n'; })
f16 'invalid used_by' < <({ printf '[[approval]]\nname = "example-ext"\nversion = "=1.0.0"\nused_by = "fixture-core"\n'; printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "fixture"\n'; })
f16 'invalid purpose' < <({ printf '%s\n' "${BASE_HEAD}"; printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "   "\n'; })
f16 'invalid purpose' < <({ printf '%s\n' "${BASE_HEAD}"; printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "a\\nb"\n'; })
f16 'invalid version' < <({ printf '[[approval]]\nname = "example-ext"\nversion = "1.0.0"\nused_by = ["fixture-core"]\n'; printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "fixture"\n'; })

pinned_fixture; approve example-ext '=1.0.0' fixture-core; approve example-ext '=1.0.0' fixture-core; run_check
expect_violations "F17 duplicate record name" 1 'malformed approval record (approval #2: duplicate name'

pinned_fixture
printf '[approval]\nname = "example-ext"\n' > "${ws}/deps-approved.toml"; run_check
expect_violations "F18a [approval] is not an array of tables" 2 'approval must be an array of tables ([[approval]])'
pinned_fixture
printf 'approval = "yes"\n' > "${ws}/deps-approved.toml"; run_check
expect_violations "F18b approval = string" 2 'approval must be an array of tables ([[approval]])'

pinned_fixture; approve example-ext '=1.0.0' fixture-core
printf 'allow_all = true\n' | cat - "${ws}/deps-approved.toml" > "${work}/rec.toml"
mv "${work}/rec.toml" "${ws}/deps-approved.toml"; run_check
expect_violations "F19 unknown top-level key" 1 'unknown top-level key in approval record file: allow_all'

# 記録の値をシェルに通していないことの回帰: コマンド置換が目印ファイルを作らない。
new_fixture
marker="${work}/injected"
# shellcheck disable=SC2016 # コマンド置換を展開させず、リテラルのまま記録へ書くのが目的
{
  printf '[[approval]]\nname = "$(touch %s)"\nversion = "=1.0.0"\n' "${marker}"
  printf 'used_by = ["`touch %s`"]\n' "${marker}"
  printf 'approved_on = "2000-01-01"\napproval_ref = "#0"\npurpose = "fixture"\n'
} > "${ws}/deps-approved.toml"
run_check
expect_violations "F20 command substitution in record values" 1 'unused approval record:'
if [ -e "${marker}" ]; then report_fail "F20 record value was executed by the shell" "marker file exists"; else report_ok "F20 no marker file created"; fi

echo "# passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
