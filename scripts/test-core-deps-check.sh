#!/usr/bin/env bash
# core-deps-check.sh の回帰テスト（spec TASK-258 のポインタ。親 Issue #473 の 1/2）。
# 共通の crate（`crates/core`）に外部の crate の宣言が増えたとき、または共通の crate の feature が契約の crate
# （`crates/contract`）から転送されなくなったときに、検査が失敗することを機械で確かめ続ける。
#
# 節の構成:
#   1. 合成の cargo metadata（`--metadata-json`。cargo・git を呼ばない）: 合格・(A) 依存の違反・(B) feature の違反・誤用
#   2. 偽のコマンドと PATH を絞った環境（引数なしの既定の経路）
#   3. 実物: 実行時に一時ディレクトリへ作る最小の一時 workspace で、実物の cargo metadata を通す。依存は path だけで
#      ネットワークを使わず、リポジトリのファイルを変更しない
#   4. 実物のリポジトリ: 引数なしで合格し `record:` の行が出る
# 呼び出し元: `make test-core-deps-check`（直接実行も可）。CI のジョブからはまだ呼ばない。
# 前提: python3・cargo・git・rust-toolchain.toml の toolchain（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う。終了コードだけでは別の理由で落ちても
# 気付けず、検査が素通りする退行（fail-open）を見逃すため。期待値を緩めない。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd cargo
require_cmd git

CHECK="${DIR}/core-deps-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-core-deps-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成の metadata を ${work}/m_<ケース名>.json に作る。
python3 -I - "${work}" <<'PY'
import json
import os
import sys

out = sys.argv[1]
WROOT = os.path.join(out, "synthws")
CORE_M = WROOT + "/crates/core/Cargo.toml"
CONTRACT_M = WROOT + "/crates/contract/Cargo.toml"


def dep(name, kind=None, target=None, rename=None, path=None):
    return {"name": name, "kind": kind, "target": target, "rename": rename, "path": path}


def pkg(name, manifest, deps, features):
    return {"name": name, "manifest_path": manifest, "dependencies": deps, "features": features}


CORE_FEATS = {"test-support": []}
CONTRACT_DEPS = [dep("core-dummy", path=WROOT + "/crates/core")]
CONTRACT_FEATS = {
    "default": ["contract-core"], "contract-core": [], "backend-metal": ["contract-core"],
    "test-support": ["core-dummy/test-support"],
}


def meta(core_deps=None, core_feats=None, contract_deps=None, contract_feats=None, extra=None, wroot=WROOT):
    pk = [
        pkg("core-dummy", CORE_M, [] if core_deps is None else core_deps, CORE_FEATS if core_feats is None else core_feats),
        pkg("contract-dummy", CONTRACT_M, CONTRACT_DEPS if contract_deps is None else contract_deps,
            CONTRACT_FEATS if contract_feats is None else contract_feats),
    ]
    return {"workspace_root": wroot, "packages": pk + (extra or [])}


m = {}
m["ok"] = meta()
m["ok_path_norm"] = meta()
m["ok_path_norm"]["packages"][0]["manifest_path"] = WROOT + "//crates/./sub/../core/Cargo.toml"
# 契約の crate の backend-* など、共通の crate に対応の無い feature は対象にしない
m["ok_extra_contract_feature"] = meta(contract_feats=dict(CONTRACT_FEATS, **{"backend-new": ["contract-core"], "x": []}))
# 契約の crate の core への依存が rename されていても、キーで照合する
m["ok_rename"] = meta(
    contract_deps=[dep("core-dummy", rename="core_alias", path=WROOT + "/crates/core")],
    contract_feats=dict(CONTRACT_FEATS, **{"test-support": ["core_alias/test-support"]}),
)
# 共通の crate 以外の package の依存は検査しない範囲
m["ok_other_pkg_deps"] = meta(extra=[pkg("api", WROOT + "/crates/api/Cargo.toml", [dep("ash"), dep("cudarc")], {})])

# (A) 外部の crate の宣言が 1 つ増えると落ちる（どの名前でも・どの種類でも・どの形でも）
m["bad_ext"] = meta(core_deps=[dep("some-lib")])
m["bad_bitflags"] = meta(core_deps=[dep("bitflags")])
m["bad_chip"] = meta(core_deps=[dep("objc2-metal")])
m["bad_build"] = meta(core_deps=[dep("some-lib", "build")])
m["bad_dev"] = meta(core_deps=[dep("some-lib", "dev")])
m["bad_target"] = meta(core_deps=[dep("some-lib", target="cfg(target_os = \"macos\")")])
m["bad_member"] = meta(core_deps=[dep("contract-dummy", path=WROOT + "/crates/contract")])
m["bad_multi"] = meta(core_deps=[dep("a-lib"), dep("b-lib", "dev"), dep("c-lib", "build")])
# 同じ名前の重複は 1 件に数える
m["bad_dup"] = meta(core_deps=[dep("some-lib"), dep("some-lib", target="cfg(unix)")])

# (B) feature の転送
FEATS2 = dict(CORE_FEATS, **{"extra": []})
m["bad_no_contract_feature"] = meta(core_feats=FEATS2)
m["bad_not_forwarded"] = meta(core_feats=FEATS2, contract_feats=dict(CONTRACT_FEATS, **{"extra": []}))
m["bad_forward_other_feature"] = meta(
    core_feats=FEATS2, contract_feats=dict(CONTRACT_FEATS, **{"extra": ["core-dummy/test-support"]}))
m["bad_renamed"] = meta(contract_feats=dict(CONTRACT_FEATS, **{"test-support": [], "ts": ["core-dummy/test-support"]}))
m["bad_renamed_weak"] = meta(contract_feats=dict(CONTRACT_FEATS, **{"test-support": ["core-dummy/test-support"], "ts": ["core-dummy?/test-support"]}))
m["bad_default"] = meta(core_feats={"test-support": [], "default": []},
    contract_feats=dict(CONTRACT_FEATS, **{"default": ["core-dummy/default"]}))

# 読めない入力
m["err_pkgs_type"] = {"workspace_root": WROOT, "packages": {}}
m["err_no_wroot"] = {"packages": []}
m["err_kind"] = meta(core_deps=[dep("x", kind="weird")])
m["err_deps_type"] = meta()
m["err_deps_type"]["packages"][0]["dependencies"] = {}
m["err_feats_type"] = meta()
m["err_feats_type"]["packages"][0]["features"] = []
m["err_feat_value"] = meta(contract_feats={"test-support": "x"})
m["err_zero_core"] = {"workspace_root": WROOT, "packages": [meta()["packages"][1]]}
m["err_zero_contract"] = {"workspace_root": WROOT, "packages": [meta()["packages"][0]]}
m["err_two_core"] = meta(extra=[meta()["packages"][0]])
m["err_contract_no_core_dep"] = meta(contract_deps=[])
m["err_contract_two_core_dep"] = meta(contract_deps=CONTRACT_DEPS + [dep("core-dummy", rename="again", path=WROOT + "/crates/core")])
m["err_contract_dev_only_core_dep"] = meta(contract_deps=[dep("core-dummy", kind="dev", path=WROOT + "/crates/core")])
m["err_dep_name"] = meta(core_deps=[{"name": 1, "kind": None}])

for k, v in m.items():
    with open(os.path.join(out, "m_%s.json" % k), "w", encoding="utf-8") as f:
        json.dump(v, f)
with open(os.path.join(out, "m_notjson.json"), "w", encoding="utf-8") as f:
    f.write("not json")
PY

report_ok() {
  passed=$((passed + 1))
  echo "ok: $1"
}

report_fail() {
  failed=$((failed + 1))
  echo "FAIL: $1: $2" >&2
  echo "--- stderr ---" >&2
  sed 's/^/    /' "${ERR_F}" >&2 || true
}

# run_check <引数...>: 検査を実行し rc・標準出力・標準エラーを控える。
run_check() {
  rc=0
  "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# judge <名前> <期待コード> [語...]: o:<語> は標準出力に、e:<語> は標準エラーに含まれること。!<語> はどちらにも含まれないこと。
judge() {
  local name="$1" want="$2" s
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    report_fail "${name}" "expected exit ${want}, got ${rc}"
    return 0
  fi
  for s in "$@"; do
    case "${s}" in
      '!'*)
        if grep -qF -e "${s#!}" "${ERR_F}" "${OUT_F}"; then
          report_fail "${name}" "output must not contain: ${s#!}"
          return 0
        fi
        ;;
      o:*)
        if ! grep -qF -e "${s#o:}" "${OUT_F}"; then
          report_fail "${name}" "stdout lacks: ${s#o:}"
          return 0
        fi
        ;;
      e:*)
        if ! grep -qF -e "${s#e:}" "${ERR_F}"; then
          report_fail "${name}" "stderr lacks: ${s#e:}"
          return 0
        fi
        ;;
    esac
  done
  report_ok "${name}"
}

# mgood <ケース名> [語...]: 違反なし（終了コード 0）。
mgood() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/m_${c}.json"
  judge "${c}" 0 "o:ok: common crate dependencies match" "o:record: commit=- expected_normal=- declared_normal=- unexpected=0 missing=0 " \
    "o:forward_violations=0" "!violation:" "!FAILED" "!${work}" "$@"
}

# mviol <ケース名> [語...]: 違反（終了コード 1）。
mviol() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/m_${c}.json"
  judge "${c}" 1 "e:violation: check=" "e:FAILED: " "o:record: commit=- " "!ok: " "!${work}" "$@"
}

# merr <ケース名> [語...]: 誤用・読めない入力（終了コード 2）。
merr() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/m_${c}.json"
  judge "${c}" 2 "e:error: " "!ok: " "!FAILED" "!record: " "!${work}" "$@"
}

# ================= 1. 合成の cargo metadata =================
mgood ok "o:scanned: check=core-deps manifest=crates/core/Cargo.toml package=core-dummy normal=- build=- dev=-" \
  "o:scanned: check=feature-forward manifest=crates/contract/Cargo.toml package=contract-dummy core_features=1 forwarded=1" \
  "o:core_features=1 forwarded=1 forward_violations=0"
mgood ok_path_norm
mgood ok_extra_contract_feature
mgood ok_rename
mgood ok_other_pkg_deps

mviol bad_ext "e:violation: check=core-dep kind=normal dependency=some-lib reason=unexpected target=-" "e:FAILED: 1 violation(s)" \
  "o:declared_normal=some-lib unexpected=1 missing=0 "
mviol bad_bitflags "e:dependency=bitflags reason=unexpected"
mviol bad_chip "e:dependency=objc2-metal reason=unexpected"
mviol bad_build "e:kind=build dependency=some-lib reason=unexpected" "o:declared_build=1 "
mviol bad_dev "e:kind=dev dependency=some-lib reason=unexpected" "o:declared_dev=1 "
mviol bad_target "e:dependency=some-lib reason=unexpected target=cfg?target_os????macos??"
mviol bad_member "e:dependency=contract-dummy reason=unexpected"
mviol bad_multi "e:dependency=a-lib" "e:kind=dev dependency=b-lib" "e:kind=build dependency=c-lib" "e:FAILED: 3 violation(s)"
mviol bad_dup "e:FAILED: 1 violation(s)" "o:declared_normal=some-lib unexpected=1 "

mviol bad_no_contract_feature "e:violation: check=feature-forward feature=extra reason=no-contract-feature" "o:core_features=2 forwarded=1 forward_violations=1"
mviol bad_not_forwarded "e:feature=extra reason=not-forwarded"
mviol bad_forward_other_feature "e:feature=extra reason=not-forwarded"
mviol bad_renamed "e:feature=test-support reason=not-forwarded" "e:feature=test-support reason=renamed"
mviol bad_renamed_weak "e:feature=test-support reason=renamed"
mviol bad_default "e:feature=default reason=default" "!reason=not-forwarded"

merr err_pkgs_type
merr err_no_wroot
merr err_kind "e:dependency kind outside the counted kinds"
merr err_deps_type "e:missing or malformed 'dependencies'"
merr err_feats_type "e:missing or malformed 'features'"
merr err_feat_value "e:malformed feature list"
merr err_zero_core "e:crates/core/Cargo.toml not found exactly once"
merr err_zero_contract "e:crates/contract/Cargo.toml not found exactly once"
merr err_two_core "e:crates/core/Cargo.toml not found exactly once"
merr err_contract_no_core_dep "e:contract crate must depend on the common crate exactly once"
merr err_contract_two_core_dep "e:contract crate must depend on the common crate exactly once"
merr err_contract_dev_only_core_dep "e:contract crate must depend on the common crate exactly once"
merr err_dep_name "e:dependency name is not a string"

run_check --metadata-json "${work}/m_notjson.json"
judge "err_not_json" 2 "e:error: cannot read JSON input" "!ok: " "!record: "
run_check --metadata-json "${work}/does-not-exist.json"
judge "err_missing_file" 2 "e:error: cannot read JSON input" "!ok: " "!record: " "!${work}"
run_check --metadata-json "${work}/m_ok.json" --metadata-json "${work}/m_ok.json"
judge "err_two_metadata" 2 "e:error: --metadata-json takes exactly one path" "!ok: " "!record: "
run_check --unknown-flag
judge "err_unknown_arg" 2 "!ok: " "!record: "
run_check --metadata-json
judge "err_missing_value" 2 "!ok: " "!record: "

# ================= 2. 偽のコマンドと PATH を絞った環境 =================
fakebin="${work}/fakebin"
mkdir -p "${fakebin}"
args_log="${work}/cargo-args.log"
fake_sha="0123456789abcdef0123456789abcdef01234567"
cat >"${fakebin}/git" <<'GIT'
#!/usr/bin/env bash
case "$1" in
  rev-parse) echo "0123456789abcdef0123456789abcdef01234567" ;;
  status) [ "${FAKE_DIRTY:-}" = "1" ] && echo " M x" ; exit 0 ;;
esac
GIT
cat >"${fakebin}/cargo" <<'CARGO'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"${FAKE_ARGS_LOG}"
case "${FAKE_CARGO:-ok}" in
  ok) cat "${FAKE_METADATA}" ;;
  fail) exit 1 ;;
  badjson) echo "not json" ;;
esac
CARGO
chmod +x "${fakebin}/git" "${fakebin}/cargo"

# run_fake <metadata> [dirty] [cargo モード]: 偽の git / cargo を PATH の先頭に入れて既定の経路を流す。
run_fake() {
  rc=0
  rm -f "${args_log}"
  PATH="${fakebin}:${PATH}" FAKE_METADATA="$1" FAKE_DIRTY="${2:-0}" FAKE_CARGO="${3:-ok}" FAKE_ARGS_LOG="${args_log}" \
    "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

run_fake "${work}/m_ok.json"
judge "fake_ok" 0 "o:record: commit=${fake_sha} expected_normal=- " "o:ok: " "!-dirty" "!violation:"
if [ "$(grep -cxF -e "metadata --locked --offline --no-deps --format-version 1 --all-features" "${args_log}")" = "1" ] \
  && [ "$(wc -l <"${args_log}" | tr -d ' ')" = "1" ]; then
  report_ok "fake_metadata_args"
else
  report_fail "fake_metadata_args" "cargo must be called once with the fixed metadata arguments"
fi
run_fake "${work}/m_ok.json" 1
judge "fake_dirty" 0 "o:record: commit=${fake_sha}-dirty "
run_fake "${work}/m_bad_ext.json"
judge "fake_violation" 1 "e:dependency=some-lib reason=unexpected" "o:record: commit=${fake_sha} " "!ok: "
run_fake "${work}/m_ok.json" 0 fail
judge "fake_cargo_fails" 2 "e:error: cargo metadata failed" "!ok: " "!record: "
run_fake "${work}/m_ok.json" 0 badjson
judge "fake_cargo_not_json" 2 "e:error: cargo metadata output is not JSON" "!ok: " "!record: "

bash_bin="$(command -v bash)"
dirname_bin="$(command -v dirname)"
py_real="$(python3 -I -c 'import sys; print(sys.executable)')"

nopy="${work}/nopy"
mkdir -p "${nopy}"
ln -s "${dirname_bin}" "${nopy}/dirname"
rc=0
PATH="${nopy}" "${bash_bin}" "${CHECK}" --metadata-json "${work}/m_ok.json" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_python3" 2 "e:required command not found on PATH: python3" "!ok: " "!record: " "!scanned:"

nocargo="${work}/nocargo"
mkdir -p "${nocargo}"
ln -s "${dirname_bin}" "${nocargo}/dirname"
ln -s "${py_real}" "${nocargo}/python3"
ln -s "${bash_bin}" "${nocargo}/bash"
cp "${fakebin}/git" "${nocargo}/git"
rc=0
PATH="${nocargo}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_cargo" 2 "e:error: cannot run cargo metadata" "!ok: " "!record: "

nogit="${work}/nogit"
mkdir -p "${nogit}"
ln -s "${dirname_bin}" "${nogit}/dirname"
ln -s "${py_real}" "${nogit}/python3"
rc=0
PATH="${nogit}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_git" 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"

# ================= 3. 実物（一時 workspace の実物の cargo metadata）=================
# 一時 workspace に検査と lib.sh を写して流す。マニフェストはリポジトリに置かず、依存は path だけでネットワークを使わない。
ws="${work}/realws"
real_ok=1
mkdir -p "${ws}/scripts" "${ws}/crates/core/src" "${ws}/crates/contract/src" "${work}/outside/src" || real_ok=0
if [ "${real_ok}" = "1" ]; then
  cp "${CHECK}" "${DIR}/lib.sh" "${ws}/scripts/" || real_ok=0
  cat >"${ws}/Cargo.toml" <<'TOML'
[workspace]
resolver = "2"
members = ["crates/core", "crates/contract"]
TOML
  cat >"${ws}/crates/core/Cargo.toml" <<'TOML'
[package]
name = "real-core"
version = "0.0.0"
edition = "2021"
publish = false

[features]
test-support = []
TOML
  echo "" >"${ws}/crates/core/src/lib.rs"
  cat >"${ws}/crates/contract/Cargo.toml" <<'TOML'
[package]
name = "real-contract"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
real-core = { path = "../core" }

[features]
test-support = ["real-core/test-support"]
backend-x = []
TOML
  echo "" >"${ws}/crates/contract/src/lib.rs"
  cat >"${work}/outside/Cargo.toml" <<'TOML'
[package]
name = "outside-lib"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]
TOML
  echo "" >"${work}/outside/src/lib.rs"
  # 検査が git で commit を取るので、一時 workspace を git の作業ツリーにする。
  ( cd "${ws}" \
    && git init -q . \
    && git add -A \
    && git -c user.name=t -c user.email=t@example.invalid -c commit.gpgsign=false commit -q -m init \
    && cargo generate-lockfile --offline >/dev/null 2>&1 \
    && git add -A \
    && git -c user.name=t -c user.email=t@example.invalid -c commit.gpgsign=false commit -q -m lock ) >/dev/null 2>&1 || real_ok=0
fi
if [ "${real_ok}" = "1" ]; then
  rc=0
  "${ws}/scripts/core-deps-check.sh" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
  judge "real_ok" 0 "o:ok: " "o:scanned: check=core-deps manifest=crates/core/Cargo.toml package=real-core normal=- build=- dev=-" \
    "o:core_features=1 forwarded=1 forward_violations=0" "!-dirty" "!violation:" "!${work}"

  # 外部の crate を 1 つ足すと落ちる（workspace の外の path crate。registry は使わない）
  printf '\n[dependencies]\noutside-lib = { path = "../../../outside" }\n' >>"${ws}/crates/core/Cargo.toml"
  ( cd "${ws}" && cargo generate-lockfile --offline ) >/dev/null 2>&1 || report_fail "real_relock" "cannot refresh the lock file"
  rc=0
  "${ws}/scripts/core-deps-check.sh" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
  judge "real_added_dependency" 1 "e:violation: check=core-dep kind=normal dependency=outside-lib reason=unexpected" "e:FAILED: 1 violation(s)" \
    "o:declared_normal=outside-lib unexpected=1 " "!ok: " "!${work}"
else
  report_fail "real_fixture_setup" "cannot prepare the temporary workspace"
fi

# ================= 4. 実物のリポジトリ =================
rc=0
"${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "repo_ok" 0 "o:scanned: check=core-deps manifest=crates/core/Cargo.toml package=fandhe-silicon-core " \
  "o:scanned: check=feature-forward manifest=crates/contract/Cargo.toml package=fandhe-silicon-contract " "o:record: commit=" "o:unexpected=0 missing=0 " \
  "o:forward_violations=0" "o:ok: " "!violation:" "!FAILED"

echo "passed=${passed} failed=${failed}"
[ "${failed}" -eq 0 ]
