#!/usr/bin/env bash
# public-api-check.sh の回帰テスト（spec TASK-10 のポインタ。親 Issue #363 の 2/6・3/6）。
# 一時ディレクトリの合成 rustdoc JSON（ダミーの名前）を --json で渡し、終了コード・標準出力・標準エラーの文言を
# 確かめる。合成 JSON の節は cargo を呼ばない。
# 末尾の「実物」の節は、実行時に一時ディレクトリへ作る wgpu 系の名前の合成 path crate（比較基準の baseline-wgpu/
# ではない。陽性対照は TASK-12 のポインタ）の一時 workspace に検査を写し、実物の cargo rustdoc の JSON で
# JSON を作る段を通す（#3393）。依存は path だけでネットワークを使わず、リポジトリのファイルを変更しない。
# 呼び出し元: `make test-public-api-check`（直接実行も可。CI のステップは持たない）。
# 前提: python3・cargo・git・rust-toolchain.toml の toolchain（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う。終了コードだけでは別の理由で落ちても
# 気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。現行の検査と食い違ったらテストでなく検査側の挙動として直す。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

CHECK="${DIR}/public-api-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-public-api-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成 JSON をまとめて作る。ファイル名は ${work}/<ケース名>.json。
python3 -I - "${work}" <<'PY'
import copy
import json
import os
import sys

out = sys.argv[1]


def item(iid, name, inner, vis="public", crate_id=0):
    return {"id": iid, "crate_id": crate_id, "name": name, "visibility": vis, "inner": inner}


def base():
    return {
        "root": 0,
        "format_version": 61,
        "includes_private": False,
        "external_crates": {"1": {"name": "other_lib"}, "2": {"name": "wgpu-core"}},
        "paths": {
            "10": {"crate_id": 1, "kind": "struct", "path": ["other_lib", "X"]},
            "11": {"crate_id": 2, "kind": "struct", "path": ["wgpu_core", "Y"]},
            "20": {"crate_id": 0, "kind": "struct", "path": ["dummy", "Z"]},
        },
        "index": {"0": item(0, "dummy", {"module": {"is_crate": True, "items": []}})},
    }


def with_fn(inner_body, extra=None):
    """root の下に公開関数 1 件を足す。inner_body はその関数の inner（種別の鍵つき）。"""
    d = base()
    d["index"]["0"]["inner"]["module"]["items"] = [1]
    d["index"]["1"] = item(1, "f", inner_body)
    for k, v in (extra or {}).items():
        d["index"][k] = v
    return d


def rp(i):
    return {"function": {"output": {"resolved_path": {"id": i, "path": "T"}}}}


cases = {}
cases["ok_noref"] = base()
cases["ok_paths_internal"] = with_fn(rp(20))
cases["ok_index_internal"] = with_fn(rp(1))
cases["ok_ext_nonwgpu"] = with_fn(rp(10))
cases["ok_index_ext_nonwgpu"] = with_fn(rp(20), {"30": item(30, "q", {"struct": {}}, crate_id=1)})
cases["ok_use_null"] = with_fn({"use": {"source": "x", "name": "x", "id": None}})
cases["ok_seq_internal"] = with_fn({"struct": {"kind": {"plain": {"fields": [1]}}}})
cases["count_visibility"] = with_fn(rp(20), {
    "2": item(2, "imp", {"impl": {}}, vis="default"),
    "3": item(3, "hidden", {"struct": {}}, vis="crate"),
    "4": item(4, "scoped", {"struct": {}}, vis={"restricted": {"parent": 0, "path": "::dummy"}}),
    "5": item(5, "ext", {"struct": {}}, crate_id=1),
})
cases["ok_tuple_null"] = with_fn({"struct": {"kind": {"tuple": [None, 1]}}})
for tag, nm in (("my_wgpu", "my_wgpu"), ("nagano", "nagano")):
    d = with_fn(rp(10))
    d["external_crates"]["1"]["name"] = nm
    cases["ok_name_" + tag] = d

cases["bad_type"] = with_fn(rp(11))
cases["bad_trait"] = with_fn({"function": {"output": {"dyn_trait": {"trait": {"id": 11}}}}})
cases["bad_use"] = with_fn({"use": {"source": "x", "name": "x", "id": 11}})
for i, nm in enumerate(["wgpu-core", "wgpu_hal", "wgpu_types", "naga", "naga-types", "wgpu-naga-bridge"]):
    d = with_fn(rp(11))
    d["external_crates"]["2"]["name"] = nm
    cases["bad_name_%d" % i] = d
cases["bad_index_crate"] = with_fn(rp(40), {"40": item(40, "w", {"struct": {}}, crate_id=2)})
cases["bad_unref_index"] = with_fn(rp(20), {"41": item(41, "w", {"struct": {}}, crate_id=2)})
cases["bad_seq_wgpu"] = with_fn({"struct": {"kind": {"plain": {"fields": [41]}}}},
                                {"41": item(41, "w", {"struct": {}}, crate_id=2)})

d = with_fn({"function": {"output": {"resolved_path": {"id": None}}}})
cases["err_null_id"] = d
cases["err_str_id"] = with_fn(rp("11"))
cases["err_bool_id"] = with_fn(rp(True))
cases["err_unresolved_id"] = with_fn(rp(999))
cases["err_index_crate_unknown"] = with_fn(rp(50), {"50": item(50, "w", {"struct": {}}, crate_id=99)})
d = with_fn(rp(60))
d["paths"]["60"] = {"crate_id": 99}
cases["err_paths_crate_unknown"] = d
cases["err_seq_unresolved"] = with_fn({"struct": {"kind": {"plain": {"fields": [999]}}}})
cases["err_seq_null"] = with_fn({"struct": {"kind": {"plain": {"fields": [None]}}}})
d = with_fn(rp(20))
del d["index"]["1"]["crate_id"]
cases["err_item_no_crate_id"] = d
d = with_fn(rp(20))
del d["index"]["1"]["inner"]
cases["err_public_no_inner"] = d
d = base()
d["root"] = 7
cases["err_root_missing"] = d
d = base()
del d["index"]["0"]["inner"]
cases["err_root_no_inner"] = d
d = base()
d["index"]["0"]["visibility"] = "crate"
cases["err_no_public"] = d
for key, val in (("del_fv", None), ("str_fv", "61")):
    d = base()
    if val is None:
        del d["format_version"]
    else:
        d["format_version"] = val
    cases["err_" + key] = d
d = base()
d["includes_private"] = True
cases["err_private"] = d
d = base()
del d["index"]
cases["err_no_index"] = d

for name, doc in cases.items():
    with open(os.path.join(out, name + ".json"), "w", encoding="utf-8") as f:
        json.dump(doc, f)
with open(os.path.join(out, "notjson.json"), "w", encoding="utf-8") as f:
    f.write("{not json")
PY

# ---- 土台 ----

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

# good <ケース名> [語...]: 参照 0 件（終了コード 0・ok の行）。
good() {
  local c="$1"
  shift
  run_check --json "${work}/${c}.json"
  judge "${c}" 0 "o:ok: 1 public API document(s)" "o:scanned: " "o:wgpu_type_refs=0 wgpu_other_refs=0" "o:record: commit=- documents=1 " "o:wgpu_type_refs_total=0 wgpu_other_refs_total=0" "!violation" "$@"
}

# viol <ケース名> [語...]: 違反（終了コード 1・FAILED の行）。
viol() {
  local c="$1"
  shift
  run_check --json "${work}/${c}.json"
  judge "${c}" 1 "e:violation: " "e:FAILED: " "o:record: commit=- documents=1 " "!ok: " "$@"
}

# misuse <ケース名> [語...]: 誤用（終了コード 2・error の行・成功の行なし）。
misuse() {
  local c="$1"
  shift
  run_check --json "${work}/${c}.json"
  judge "${c}" 2 "e:error: " "!ok: " "!FAILED" "!record: " "$@"
}

# ---- 合格 ----

good ok_noref "o:public_items=1 "
# 公開アイテムは root・public・default だけを数える（crate・restricted・外部 crate のアイテムは数えない）
good count_visibility "o:public_items=3 " "o:public_items_total=3 "
good ok_paths_internal
good ok_index_internal
good ok_ext_nonwgpu
good ok_index_ext_nonwgpu
good ok_use_null
good ok_seq_internal
good ok_tuple_null
good ok_name_my_wgpu
good ok_name_nagano

# ---- 違反 ----

viol bad_type "e:kind=type" "o:wgpu_type_refs=1 "
viol bad_trait "e:kind=other" "o:wgpu_other_refs=1"
viol bad_use "e:kind=other" "o:wgpu_other_refs=1"
for i in 0 1 2 3 4 5; do
  viol "bad_name_${i}" "e:kind=type"
done
viol bad_index_crate "e:kind=other"
viol bad_unref_index "e:kind=other" "o:wgpu_other_refs=1"
viol bad_seq_wgpu "e:kind=other"

# 2 本渡して片方だけ違反（scanned が 2 行・全体は違反）
run_check --json "${work}/ok_noref.json" "${work}/bad_type.json"
if [ "$(grep -c '^scanned: ' "${OUT_F}")" = "2" ]; then
  judge "two_files_one_bad" 1 "e:FAILED: 1 reference(s)" "!ok: " \
    "o:record: commit=- documents=2 public_items_total=3 wgpu_type_refs_total=1 wgpu_other_refs_total=0"
else
  report_fail "two_files_one_bad" "expected 2 scanned lines"
fi
# 終了コード 1 では record が最後の行で、1 回だけ出る
if [ "$(tail -n 1 "${OUT_F}" | cut -c1-8)" = "record: " ] && [ "$(grep -c '^record: ' "${OUT_F}")" = "1" ]; then
  report_ok "record_last_on_violation"
else
  report_fail "record_last_on_violation" "record line must be the last stdout line, once"
fi

# 2 本の合計（record は scanned の同名の値の合計。record → ok の順）
run_check --json "${work}/ok_noref.json" "${work}/count_visibility.json"
judge "two_files_totals" 0 "o:record: commit=- documents=2 public_items_total=4 wgpu_type_refs_total=0 wgpu_other_refs_total=0"
if [ "$(tail -n 2 "${OUT_F}" | head -n 1 | cut -c1-8)" = "record: " ] \
  && [ "$(tail -n 1 "${OUT_F}" | cut -c1-4)" = "ok: " ] && [ "$(grep -c '^record: ' "${OUT_F}")" = "1" ]; then
  report_ok "record_before_ok"
else
  report_fail "record_before_ok" "last two stdout lines must be record then ok"
fi

# ---- 誤用（終了コード 2）----

misuse err_null_id
misuse err_str_id
misuse err_bool_id
misuse err_unresolved_id
misuse err_index_crate_unknown
misuse err_paths_crate_unknown
misuse err_seq_unresolved
misuse err_seq_null
misuse err_item_no_crate_id
misuse err_public_no_inner
misuse err_root_missing
misuse err_root_no_inner
misuse err_no_public
misuse err_del_fv
misuse err_str_fv
misuse err_private
misuse err_no_index
misuse notjson

run_check --json "${work}/no-such-file.json"
judge "missing_path" 2 "e:error: " "!ok: " "!record: "
run_check --bogus
judge "unknown_arg" 2 "!ok: " "!record: "
run_check --json
judge "json_without_path" 2 "!ok: " "!record: "

# 2 本のうち 1 本が読めなければ、先に合格した分があっても全体は 2（途中の scanned は出るが record は出ない）
run_check --json "${work}/ok_noref.json" "${work}/notjson.json"
if [ "$(grep -c '^scanned: ' "${OUT_F}")" = "1" ] && [ "$(grep -c '^record: ' "${OUT_F}")" = "0" ]; then
  judge "second_unreadable" 2 "e:error: " "!ok: " "!record: "
else
  report_fail "second_unreadable" "expected 1 scanned line and 0 record lines"
fi

# ---- 組ごとの JSON を取り違えないこと（偽の cargo / rustdoc を PATH の先頭に置く）----
# 偽の cargo は、--all-features の有無で別の合成 JSON を {lib}.json へ書く。片方の組にだけ禁止参照が
# ある場合に、後の組の JSON で上書きされて見逃されないことを確かめる。
fakebin="${work}/fakebin"
mkdir -p "${fakebin}"
cat >"${fakebin}/cargo" <<'FAKE'
#!/usr/bin/env bash
case "$1" in
  --version) echo "cargo fake"; exit 0 ;;
  metadata)
    printf '{"workspace_members":["fake 0.0.0"],"target_directory":"%s","packages":[{"id":"fake 0.0.0","name":"fake","targets":[{"kind":["lib"],"name":"dummy"}]}]}\n' "${FAKE_TARGET}"
    exit 0 ;;
  rustdoc)
    src="${FAKE_PROD}"
    for a in "$@"; do
      if [ "$a" = "--all-features" ]; then src="${FAKE_ALL}"; fi
    done
    mkdir -p "${FAKE_TARGET}/public-api-check/doc"
    cp "${src}" "${FAKE_TARGET}/public-api-check/doc/dummy.json"
    exit 0 ;;
esac
exit 1
FAKE
printf '#!/usr/bin/env bash\nexit 0\n' >"${fakebin}/rustdoc"
# 偽のコミット解決コマンド。FAKE_GIT の値で出力と終了コードを切り替える（既定 clean）。
cat >"${fakebin}/git" <<'FAKE'
#!/usr/bin/env bash
for a in "$@"; do
  case "$a" in
    rev-parse)
      case "${FAKE_GIT}" in
        fail-rev) exit 1 ;;
        garbage) echo "/fake/host/path not-a-commit"; exit 0 ;;
      esac
      echo "0123456789abcdef0123456789abcdef01234567"
      exit 0 ;;
    status)
      case "${FAKE_GIT}" in
        fail-status) exit 1 ;;
        dirty) echo " M crates/dummy.rs" ;;
      esac
      exit 0 ;;
  esac
done
exit 1
FAKE
chmod +x "${fakebin}/cargo" "${fakebin}/rustdoc" "${fakebin}/git"

# run_fake <production の JSON> <all-features の JSON> [コミット解決の挙動]: 偽の cargo で JSON を作る経路を流す。
run_fake() {
  rc=0
  PATH="${fakebin}:${PATH}" FAKE_TARGET="${work}/faketarget" FAKE_PROD="$1" FAKE_ALL="$2" FAKE_GIT="${3:-clean}" \
    "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

fake_sha="0123456789abcdef0123456789abcdef01234567"
run_fake "${work}/bad_type.json" "${work}/ok_noref.json"
judge "build_prod_only_bad" 1 "e:features=production" "e:FAILED: 1 reference(s)" "o:features=production" "o:features=all-features" "!ok: " \
  "o:record: commit=${fake_sha} documents=2 " "o:wgpu_type_refs_total=1 "
run_fake "${work}/ok_noref.json" "${work}/bad_type.json"
judge "build_all_only_bad" 1 "e:features=all-features" "e:FAILED: 1 reference(s)" "!ok: " "o:record: commit=${fake_sha} documents=2 "
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json"
judge "build_both_ok" 0 "o:ok: 2 public API document(s)" "!violation" "o:record: commit=${fake_sha} documents=2 "
# 組ごとの件数が対応し、合計に足されること
run_fake "${work}/count_visibility.json" "${work}/ok_noref.json"
judge "build_counts_per_set" 0 \
  "o:scanned: crate=dummy features=production format_version=61 public_items=3 " \
  "o:scanned: crate=dummy features=all-features format_version=61 public_items=1 " \
  "o:public_items_total=4 "
# 変更が在れば -dirty が付く
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" dirty
judge "build_dirty" 0 "o:record: commit=${fake_sha}-dirty documents=2 "
# コミットが取れなければビルドの前に終了コード 2 で止まる
for mode in fail-rev fail-status garbage; do
  run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${mode}"
  judge "commit_${mode}" 2 "e:error: cannot resolve the source commit" "!record: " "!ok: " "!scanned: " "!/fake/host/path"
done

# ---- 実物の rustdoc の JSON で JSON を作る段（cargo metadata → 2 組の cargo rustdoc → 数える段）を通す ----
# 一時 workspace に検査と lib.sh を写して流す。wgpu 系の名前の合成 path crate はここで作る
# （比較基準の baseline-wgpu/ ではない。陽性対照は TASK-12 のポインタ）。マニフェストはリポジトリに置かない。
# 依存は path だけなのでネットワークは使わない。target は一時 workspace の中に出る。
require_cmd cargo
require_cmd git

rw="${work}/real"
mkdir -p "${rw}/ext/wgpu-dummy/src" "${rw}/ext/plain-dummy/src" "${rw}/ws/crates/api/src" "${rw}/ws/scripts"
cat >"${rw}/ext/wgpu-dummy/Cargo.toml" <<'TOML'
[package]
name = "wgpu-dummy"
version = "0.0.0"
edition = "2024"
TOML
printf 'pub struct Handle;\npub trait Tr {}\n' >"${rw}/ext/wgpu-dummy/src/lib.rs"
cat >"${rw}/ext/plain-dummy/Cargo.toml" <<'TOML'
[package]
name = "plain-dummy"
version = "0.0.0"
edition = "2024"
TOML
printf 'pub struct Thing;\n' >"${rw}/ext/plain-dummy/src/lib.rs"
cat >"${rw}/ws/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/api"]
resolver = "3"
TOML
cat >"${rw}/ws/crates/api/Cargo.toml" <<'TOML'
[package]
name = "api"
version = "0.0.0"
edition = "2024"

[features]
leak = []

[dependencies]
wgpu-dummy = { path = "../../../ext/wgpu-dummy" }
plain-dummy = { path = "../../../ext/plain-dummy" }
TOML
cp "${DIR}/public-api-check.sh" "${DIR}/lib.sh" "${rw}/ws/scripts/"
chmod +x "${rw}/ws/scripts/public-api-check.sh"
if [ -f "${DIR}/../rust-toolchain.toml" ]; then
  cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi

# real_lib <lib.rs の中身>: api の公開面を差し替える。
real_lib() {
  printf '%s\n' "$1" >"${rw}/ws/crates/api/src/lib.rs"
}
# real_git: 開発者の全体設定とフックを効かせない git。
real_git() {
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
    -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# run_real: 写した検査を引数なしで流す。共有の target ディレクトリへ書かせない。
run_real() {
  rc=0
  (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR ./scripts/public-api-check.sh) \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

real_lib 'pub struct Own;'
if (cd "${rw}/ws" && cargo generate-lockfile --offline) >/dev/null 2>"${ERR_F}" \
  && real_git init -q >/dev/null 2>>"${ERR_F}" \
  && real_git add -A >/dev/null 2>>"${ERR_F}" \
  && real_git commit -q -m fixture >/dev/null 2>>"${ERR_F}"; then
  real_lib 'pub struct Own;
pub fn f() -> Vec<Own> { Vec::new() }'
  run_real
  judge "real_clean" 0 "o:ok: 2 public API document(s)" "o:wgpu_type_refs_total=0 wgpu_other_refs_total=0" "!violation"

  real_lib 'pub fn g() -> plain_dummy::Thing { plain_dummy::Thing }'
  run_real
  judge "real_plain_dep" 0 "o:ok: 2 public API document(s)" "o:wgpu_type_refs_total=0 wgpu_other_refs_total=0" "!violation"

  real_lib '#[cfg(feature = "leak")]
pub fn leak() -> wgpu_dummy::Handle { wgpu_dummy::Handle }'
  run_real
  judge "real_type_leak_feature" 1 "e:features=all-features item=leak kind=type references=wgpu_dummy" \
    "e:FAILED: 1 reference(s)" "o:scanned: crate=api features=production" "!ok: "
  # production の組の行に参照 0 件が出ていること（all-features の行と取り違えて合格しない）
  if grep '^scanned: crate=api features=production ' "${OUT_F}" | grep -qF "wgpu_type_refs=0 wgpu_other_refs=0"; then
    report_ok "real_type_leak_feature_production_clean"
  else
    report_fail "real_type_leak_feature_production_clean" "production line must have zero refs"
  fi

  real_lib 'pub fn leak() -> wgpu_dummy::Handle { wgpu_dummy::Handle }'
  run_real
  judge "real_type_leak" 1 "e:features=production item=leak kind=type references=wgpu_dummy" \
    "e:features=all-features item=leak kind=type references=wgpu_dummy" "e:FAILED: 2 reference(s)" "!ok: "

  real_lib 'pub use wgpu_dummy::Tr;'
  run_real
  judge "real_reexport" 1 "e:kind=other references=wgpu_dummy" "!ok: "
else
  report_fail "real_fixture_setup" "cannot prepare the temporary workspace"
fi

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
