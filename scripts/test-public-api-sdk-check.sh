#!/usr/bin/env bash
# public-api-sdk-check.sh の回帰テスト（spec TASK-13 のポインタ。親 Issue #366 の 3/4）。
# 公開 API にチップの SDK の crate（`objc2-metal`・`ash`・`cudarc`）の型・経路が出たとき、または共通の crate
# （`crates/core`）が宣言する依存にそれらが入ったときに、検査が失敗することを機械で確かめ続ける。
#
# 節の構成:
#   1. 合成の rustdoc JSON（`--json`。cargo・git を呼ばない）: 合格・完全一致の陰性・違反・誤用・複数本
#   2. 合成の cargo metadata（`--metadata-json`。cargo・git を呼ばない）: 合格・違反・誤用
#   3. 部分の実行と両方の指定（偽の cargo・git が呼ばれないこと）
#   4. 偽のコマンド（引数なしの既定の経路。偽の git・cargo と、JSON を作る段の差し替え、PATH を絞った環境）
#   5. 実物: 実行時に一時ディレクトリへ作る合成 path crate の一時 workspace で、実物の rustdoc の JSON と
#      実物の cargo metadata を通す。依存は path だけでネットワークを使わず、リポジトリのファイルを変更しない
# 呼び出し元: `make test-public-api-sdk-check`（直接実行も可。CI はまだ呼ばない）。
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
require_cmd cargo
require_cmd git

CHECK="${DIR}/public-api-sdk-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-public-api-sdk-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成の入力をまとめて作る。rustdoc の JSON は ${work}/<ケース名>.json、metadata は ${work}/m_<ケース名>.json。
python3 -I - "${work}" <<'PY'
import json
import os
import sys

out = sys.argv[1]


def item(iid, name, inner, vis="public", crate_id=0):
    return {"id": iid, "crate_id": crate_id, "name": name, "visibility": vis, "inner": inner}


def base(ext1="other_lib", ext2="wgpu-core"):
    return {
        "root": 0,
        "format_version": 61,
        "includes_private": False,
        "external_crates": {"1": {"name": ext1}, "2": {"name": ext2}},
        "paths": {
            "10": {"crate_id": 1, "kind": "struct", "path": ["x", "X"]},
            "11": {"crate_id": 2, "kind": "struct", "path": ["y", "Y"]},
            "20": {"crate_id": 0, "kind": "struct", "path": ["dummy", "Z"]},
        },
        "index": {"0": item(0, "dummy", {"module": {"is_crate": True, "items": []}})},
    }


def with_fn(inner_body, extra=None, ext1="other_lib", ext2="wgpu-core"):
    """root の下に公開関数 1 件を足す。inner_body はその関数の inner（種別の鍵つき）。"""
    d = base(ext1, ext2)
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
cases["ok_ext_plain"] = with_fn(rp(10))
# 外部 crate の一覧は、解決された名前の重複なしの昇順（宣言順や出現順ではない）
cases["ok_ext_sorted"] = with_fn(rp(11), {
    "2": item(2, "g", rp(10)),
    "3": item(3, "h", rp(11)),
}, ext1="zeta", ext2="alpha_lib")
cases["ok_index_ext_plain"] = with_fn(rp(20), {"30": item(30, "q", {"struct": {}}, crate_id=1)})
cases["ok_use_null"] = with_fn({"use": {"source": "x", "name": "x", "id": None}})
cases["ok_seq_internal"] = with_fn({"struct": {"kind": {"plain": {"fields": [1]}}}})
cases["ok_tuple_null"] = with_fn({"struct": {"kind": {"tuple": [None, 1]}}})
cases["count_visibility"] = with_fn(rp(20), {
    "2": item(2, "imp", {"impl": {}}, vis="default"),
    "3": item(3, "hidden", {"struct": {}}, vis="crate"),
    "4": item(4, "scoped", {"struct": {}}, vis={"restricted": {"parent": 0, "path": "::dummy"}}),
    "5": item(5, "ext", {"struct": {}}, crate_id=1),
})
# 完全一致の陰性: チップの名前に似ているだけの crate は数えない（接頭辞・部分一致に広げない）
for tag, nm in (
    ("ash_window", "ash_window"), ("my_ash", "my_ash"), ("objc2", "objc2"),
    ("cudarc_sys", "cudarc_sys"), ("objc2_metal_ext", "objc2-metal-ext"), ("objc2_foundation", "objc2-foundation"),
):
    cases["neg_" + tag] = with_fn(rp(10), ext1=nm)

# 違反: 3 つの名前と、大小文字・`-`/`_` の揺れ
for tag, nm in (
    ("ash", "ash"), ("ASH", "ASH"), ("cudarc", "cudarc"), ("objc2_metal", "objc2_metal"),
    ("objc2-metal", "objc2-metal"), ("Objc2_Metal", "Objc2_Metal"),
):
    cases["bad_type_" + tag] = with_fn(rp(10), ext1=nm)
cases["bad_trait"] = with_fn({"function": {"output": {"dyn_trait": {"trait": {"id": 10}}}}}, ext1="ash")
cases["bad_use"] = with_fn({"use": {"source": "x", "name": "x", "id": 10}}, ext1="cudarc")
cases["bad_index_crate"] = with_fn(rp(40), {"40": item(40, "w", {"struct": {}}, crate_id=1)}, ext1="objc2-metal")
cases["bad_unref_index"] = with_fn(rp(20), {"41": item(41, "w", {"struct": {}}, crate_id=1)}, ext1="ash")
cases["bad_seq"] = with_fn(
    {"struct": {"kind": {"plain": {"fields": [41]}}}},
    {"41": item(41, "w", {"struct": {}}, crate_id=1)},
    ext1="cudarc",
)

cases["err_null_id"] = with_fn({"function": {"output": {"resolved_path": {"id": None}}}})
cases["err_str_id"] = with_fn(rp("10"))
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
d = base()
del d["format_version"]
cases["err_del_fv"] = d
d = base()
d["format_version"] = "61"
cases["err_str_fv"] = d
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
with open(os.path.join(out, "notutf8.json"), "wb") as f:
    f.write(b"\xff\xfe\x00{}")

# ---- 合成の cargo metadata ----
WROOT = os.path.join(out, "synthws")
COMMON = "common-dummy"


def dep(name, kind=None, target=None):
    return {"name": name, "kind": kind, "target": target}


def pkg(name, deps, manifest=None):
    return {"name": name, "manifest_path": manifest or (WROOT + "/crates/core/Cargo.toml"), "dependencies": deps}


def meta(common_deps, others=None, **kw):
    return {"workspace_root": WROOT, "packages": [pkg(COMMON, common_deps, kw.get("manifest"))] + (others or [])}


mcases = {}
mcases["ok_nodeps"] = meta([])
mcases["ok_plain"] = meta([dep("other-lib"), dep("ash-window", "dev"), dep("objc2")])
# 共通の crate 以外の package の依存は検査しない範囲
api_pkg = pkg("api", [dep("ash"), dep("cudarc"), dep("objc2-metal")], WROOT + "/crates/api/Cargo.toml")
mcases["ok_others_chip"] = meta([dep("other-lib")], [api_pkg])
# manifest_path の表記ゆれ（重複した区切り・`.`・`..`）でも共通の crate に当たる
mcases["ok_path_norm"] = meta([dep("other-lib")], manifest=WROOT + "//crates/./sub/../core/Cargo.toml")
for tag, nm in (
    ("ash", "ash"), ("ASH", "ASH"), ("cudarc", "cudarc"), ("objc2-metal", "objc2-metal"),
    ("objc2_metal", "objc2_metal"), ("Objc2_Metal", "Objc2_Metal"),
):
    mcases["bad_" + tag] = meta([dep("other-lib"), dep(nm)])
mcases["bad_build"] = meta([dep("ash", "build")])
mcases["bad_dev"] = meta([dep("cudarc", "dev")])
mcases["bad_target"] = meta([dep("objc2-metal", None, "aarch64-apple-darwin")])
mcases["bad_target_cfg"] = meta([dep("ash", "build", "cfg(unix)")])
mcases["bad_multi"] = meta([dep("ash"), dep("cudarc", "dev"), dep("objc2-metal", "build", "x86_64-unknown-linux-gnu")])
mcases["bad_path_norm"] = meta([dep("ash")], manifest=WROOT + "//crates/./sub/../core/Cargo.toml")
# 完全一致の陰性
for tag, nm in (("ash_window", "ash-window"), ("my_ash", "my-ash"), ("objc2", "objc2"),
                ("cudarc_sys", "cudarc-sys"), ("objc2_metal_ext", "objc2-metal-ext")):
    mcases["neg_" + tag] = meta([dep(nm)])

d = meta([])
del d["workspace_root"]
mcases["err_no_root"] = d
d = meta([])
d["workspace_root"] = ""
mcases["err_empty_root"] = d
d = meta([])
d["packages"] = {}
mcases["err_pkgs_notlist"] = d
mcases["err_pkgs_missing"] = {"workspace_root": WROOT}
mcases["err_zero_common"] = {"workspace_root": WROOT, "packages": [pkg("api", [], WROOT + "/crates/api/Cargo.toml")]}
d = meta([])
d["packages"].append(pkg("twin", [], WROOT + "/crates/core/Cargo.toml"))
mcases["err_two_common"] = d
d = meta([])
d["packages"][0]["dependencies"] = {}
mcases["err_deps_notlist"] = d
d = meta([])
del d["packages"][0]["dependencies"]
mcases["err_deps_missing"] = d
mcases["err_kind_other"] = meta([dep("other-lib", "proc-macro")])
mcases["err_kind_int"] = meta([dep("other-lib", 3)])
mcases["err_name_nonstr"] = meta([dep(5)])
mcases["err_target_nonstr"] = meta([dep("other-lib", None, 7)])
d = meta([])
d["packages"][0]["name"] = 5
mcases["err_pkgname_nonstr"] = d
d = meta([])
d["packages"].append(5)
mcases["err_pkg_nonobject"] = d
mcases["err_dep_nonobject"] = meta([5])
mcases["err_top_list"] = []

for name, doc in mcases.items():
    with open(os.path.join(out, "m_" + name + ".json"), "w", encoding="utf-8") as f:
        json.dump(doc, f)
with open(os.path.join(out, "m_notjson.json"), "w", encoding="utf-8") as f:
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

# --- rustdoc の JSON（--json）---
# good <ケース名> [語...]: 参照 0 件（終了コード 0・ok の行）。依存側は実行されず `-`。
good() {
  local c="$1"
  shift
  run_check --json "${work}/${c}.json"
  judge "${c}" 0 "o:ok: 1 public API document(s) checked, no chip SDK references" "o:scanned: crate=" \
    "o:sdk_type_refs=0 sdk_other_refs=0" "o:record: commit=- documents=1 " "o:sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=- common_chip_deps=-" \
    "!violation" "!common crate" "!${work}" "$@"
}

# viol <ケース名> [語...]: 違反（終了コード 1・FAILED の行）。
viol() {
  local c="$1"
  shift
  run_check --json "${work}/${c}.json"
  judge "${c}" 1 "e:violation: " "e:FAILED: " "o:record: commit=- documents=1 " "!ok: " "!${work}" "$@"
}

# misuse <ケース名> [語...]: 誤用（終了コード 2・error の行・成功の行なし）。
misuse() {
  local c="$1"
  shift
  run_check --json "${work}/${c}.json"
  judge "${c}" 2 "e:error: " "!ok: " "!FAILED" "!record: " "!${work}" "$@"
}

# --- cargo metadata（--metadata-json）---
mgood() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/m_${c}.json"
  judge "m_${c}" 0 "o:ok: common crate dependencies checked, no chip SDK references" \
    "o:scanned: check=common-deps manifest=crates/core/Cargo.toml package=common-dummy " "o:chip_deps=0" \
    "o:record: commit=- documents=- public_items_total=- sdk_type_refs_total=- sdk_other_refs_total=- " "o:common_chip_deps=0" \
    "!violation" "!public API document" "!${work}" "$@"
}

mviol() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/m_${c}.json"
  judge "m_${c}" 1 "e:violation: check=common-dep crate=common-dummy " "e:FAILED: " \
    "o:record: commit=- documents=- " "!ok: " "!${work}" "$@"
}

mmisuse() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/m_${c}.json"
  judge "m_${c}" 2 "e:error: " "!ok: " "!FAILED" "!record: " "!violation" "!${work}" "$@"
}

# ================= 1. 合成の rustdoc JSON =================

# ---- 合格 ----
good ok_noref "o:public_items=1 " "o:external_crates=-"
# 公開アイテムは root・public・default だけを数える（crate・restricted・外部 crate のアイテムは数えない）
good count_visibility "o:public_items=3 " "o:public_items_total=3 "
good ok_paths_internal "o:external_crates=-"
good ok_index_internal
good ok_ext_plain "o:external_crates=other_lib"
# チップでない外部 crate は重複なしの昇順に並ぶ
good ok_ext_sorted "o:external_crates=alpha_lib,zeta"
good ok_index_ext_plain "o:external_crates=other_lib"
good ok_use_null
good ok_seq_internal
good ok_tuple_null

# ---- 完全一致の陰性（接頭辞・部分一致に広げない）----
for t in ash_window my_ash objc2 cudarc_sys objc2_metal_ext objc2_foundation; do
  good "neg_${t}" "o:sdk_type_refs=0 "
done

# ---- 違反（名前ごと・揺れ）----
viol bad_type_ash "e:kind=type references=ash" "o:sdk_type_refs=1 sdk_other_refs=0 " "o:external_crates=ash"
viol bad_type_ASH "e:kind=type references=ASH" "o:sdk_type_refs=1 "
viol bad_type_cudarc "e:kind=type references=cudarc" "o:sdk_type_refs=1 "
viol bad_type_objc2_metal "e:kind=type references=objc2_metal" "o:sdk_type_refs=1 "
viol bad_type_objc2-metal "e:kind=type references=objc2-metal" "o:sdk_type_refs=1 "
viol bad_type_Objc2_Metal "e:kind=type references=Objc2_Metal" "o:sdk_type_refs=1 "
viol bad_trait "e:kind=other references=ash" "o:sdk_type_refs=0 sdk_other_refs=1 "
viol bad_use "e:kind=other references=cudarc" "o:sdk_other_refs=1 "
viol bad_index_crate "e:kind=other references=objc2-metal" "o:sdk_other_refs=1 "
viol bad_unref_index "e:kind=other references=ash" "o:sdk_other_refs=1 "
viol bad_seq "e:kind=other references=cudarc"
# 違反の行の形（crate・組・アイテム・種別・参照先が全部並ぶ）
run_check --json "${work}/bad_type_ash.json"
judge "violation_line_shape" 1 "e:violation: crate=dummy features=- item=f kind=type references=ash" "e:FAILED: 1 reference(s)"

# 2 本渡して片方だけ違反（scanned が 2 行・全体は違反）
run_check --json "${work}/ok_noref.json" "${work}/bad_type_ash.json"
if [ "$(grep -c '^scanned: ' "${OUT_F}")" = "2" ]; then
  judge "two_files_one_bad" 1 "e:FAILED: 1 reference(s)" "!ok: " \
    "o:record: commit=- documents=2 public_items_total=3 sdk_type_refs_total=1 sdk_other_refs_total=0"
else
  report_fail "two_files_one_bad" "expected 2 scanned lines"
fi
# 終了コード 1 では record が最後の行で、1 回だけ出る
if [ "$(tail -n 1 "${OUT_F}" | cut -c1-8)" = "record: " ] && [ "$(grep -c '^record: ' "${OUT_F}")" = "1" ]; then
  report_ok "record_last_on_violation"
else
  report_fail "record_last_on_violation" "record line must be the last stdout line, once"
fi

# 2 本の合計（record → ok の順）
run_check --json "${work}/ok_noref.json" "${work}/count_visibility.json"
judge "two_files_totals" 0 "o:record: commit=- documents=2 public_items_total=4 sdk_type_refs_total=0 sdk_other_refs_total=0 "
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
misuse notutf8

run_check --json "${work}/no-such-file.json"
judge "missing_path" 2 "e:error: " "!ok: " "!record: " "!${work}"
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

# ================= 2. 合成の cargo metadata =================

# ---- 合格 ----
mgood ok_nodeps "o:declared_deps=0 "
mgood ok_plain "o:declared_deps=3 chip_deps=0" "o:common_declared_deps=3 "
# 共通の crate 以外の package の依存は検査しない範囲
mgood ok_others_chip "o:declared_deps=1 chip_deps=0"
mgood ok_path_norm "o:declared_deps=1 "
for t in ash_window my_ash objc2 cudarc_sys objc2_metal_ext; do
  mgood "neg_${t}" "o:declared_deps=1 chip_deps=0"
done

# ---- 違反 ----
mviol bad_ash "e:dependency=ash kind=normal target=-" "e:FAILED: 1 reference(s)" "o:declared_deps=2 chip_deps=1" "o:common_chip_deps=1"
mviol bad_ASH "e:dependency=ASH kind=normal target=-"
mviol bad_cudarc "e:dependency=cudarc kind=normal target=-"
mviol bad_objc2-metal "e:dependency=objc2-metal kind=normal target=-"
mviol bad_objc2_metal "e:dependency=objc2_metal kind=normal target=-"
mviol bad_Objc2_Metal "e:dependency=Objc2_Metal kind=normal target=-"
mviol bad_build "e:dependency=ash kind=build target=-"
mviol bad_dev "e:dependency=cudarc kind=dev target=-"
mviol bad_target "e:dependency=objc2-metal kind=normal target=aarch64-apple-darwin"
mviol bad_target_cfg "e:dependency=ash kind=build target=cfg"
mviol bad_multi "e:dependency=ash kind=normal target=-" "e:dependency=cudarc kind=dev target=-" \
  "e:dependency=objc2-metal kind=build target=x86_64-unknown-linux-gnu" "e:FAILED: 3 reference(s)" "o:chip_deps=3"
mviol bad_path_norm "e:dependency=ash kind=normal target=-"
# 違反の行の形（検査した package と依存が全部並ぶ）
run_check --metadata-json "${work}/m_bad_dev.json"
judge "m_violation_line_shape" 1 "e:violation: check=common-dep crate=common-dummy dependency=cudarc kind=dev target=-"

# ---- 誤用（終了コード 2）----
mmisuse err_no_root
mmisuse err_empty_root
mmisuse err_pkgs_notlist
mmisuse err_pkgs_missing
mmisuse err_zero_common "e:common crate not found exactly once"
mmisuse err_two_common "e:common crate not found exactly once"
mmisuse err_deps_notlist
mmisuse err_deps_missing
mmisuse err_kind_other
mmisuse err_kind_int
mmisuse err_name_nonstr
mmisuse err_target_nonstr
mmisuse err_pkgname_nonstr
mmisuse err_pkg_nonobject
mmisuse err_dep_nonobject
mmisuse err_top_list
mmisuse notjson
run_check --metadata-json "${work}/no-such-file.json"
judge "m_missing_path" 2 "e:error: " "!ok: " "!record: " "!${work}"
run_check --metadata-json
judge "m_without_path" 2 "!ok: " "!record: "
run_check --metadata-json "${work}/m_ok_nodeps.json" --metadata-json "${work}/m_ok_nodeps.json"
judge "m_given_twice" 2 "e:error: --metadata-json takes exactly one path" "!ok: " "!record: " "!scanned: "

# ================= 3. 部分の実行と両方の指定 =================
# 偽の cargo・git は呼ばれたら印のファイルを書く。--json / --metadata-json を渡した経路は cargo・git を呼ばない。
guard="${work}/guard"
mkdir -p "${guard}"
for tool in cargo git; do
  cat >"${guard}/${tool}" <<'FAKE'
#!/usr/bin/env bash
echo "$0" >>"${GUARD_MARK}"
exit 1
FAKE
  chmod +x "${guard}/${tool}"
done
mark="${work}/guard-called.txt"

# run_guarded <引数...>: 偽の cargo・git を PATH の先頭に置いて検査を流す。
run_guarded() {
  rc=0
  rm -f "${mark}"
  PATH="${guard}:${PATH}" GUARD_MARK="${mark}" "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}
# not_called <名前>: 偽の cargo・git が呼ばれていないこと。
not_called() {
  if [ -e "${mark}" ]; then
    report_fail "$1" "cargo or git was called"
  else
    report_ok "$1"
  fi
}

run_guarded --json "${work}/ok_noref.json"
judge "json_only_runs_one_side" 0 "o:record: commit=- documents=1 " "o:common_declared_deps=- common_chip_deps=-" "!common crate dependencies" "!check=common-deps"
not_called "json_only_no_cargo_git"
run_guarded --metadata-json "${work}/m_ok_plain.json"
judge "metadata_only_runs_one_side" 0 "o:record: commit=- documents=- public_items_total=- sdk_type_refs_total=- sdk_other_refs_total=- common_declared_deps=3 common_chip_deps=0" \
  "!public API document" "!scanned: crate="
not_called "metadata_only_no_cargo_git"
run_guarded --json "${work}/ok_noref.json" --metadata-json "${work}/m_ok_plain.json"
judge "both_ok" 0 "o:ok: 1 public API document(s) checked, common crate dependencies checked, no chip SDK references" \
  "o:record: commit=- documents=1 public_items_total=1 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=3 common_chip_deps=0"
not_called "both_no_cargo_git"
# 片側だけの違反でも全体は 1。両側の違反は合計する
run_guarded --json "${work}/ok_noref.json" --metadata-json "${work}/m_bad_ash.json"
judge "both_deps_bad" 1 "e:FAILED: 1 reference(s)" "e:check=common-dep" "!ok: " "o:sdk_type_refs_total=0 "
not_called "both_deps_bad_no_cargo_git"
run_guarded --json "${work}/bad_type_ash.json" --metadata-json "${work}/m_ok_plain.json"
judge "both_rustdoc_bad" 1 "e:FAILED: 1 reference(s)" "e:kind=type references=ash" "!ok: " "o:common_chip_deps=0"
run_guarded --json "${work}/bad_type_ash.json" --metadata-json "${work}/m_bad_dev.json"
judge "both_sum" 1 "e:FAILED: 2 reference(s)" "e:kind=type references=ash" "e:check=common-dep" "!ok: " \
  "o:sdk_type_refs_total=1 sdk_other_refs_total=0 common_declared_deps=1 common_chip_deps=1"
not_called "both_sum_no_cargo_git"
# 片側が読めなければ全体は 2
run_guarded --json "${work}/ok_noref.json" --metadata-json "${work}/m_err_zero_common.json"
judge "both_metadata_misuse" 2 "e:common crate not found exactly once" "!ok: " "!record: "
run_guarded --json "${work}/notjson.json" --metadata-json "${work}/m_ok_plain.json"
judge "both_json_misuse" 2 "e:error: " "!ok: " "!record: "

# ================= 4. 偽のコマンド（引数なしの既定の経路）=================
# 検査・lib.sh・差し替えの public-api-check.sh を一時の「リポジトリ」へ写し、偽の git・cargo を PATH の先頭に置く。
# ROOT_DIR は lib.sh の場所から決まるので、写し先が git の作業ツリーでなくても解決できる。
fr="${work}/fakeroot"
mkdir -p "${fr}/scripts"
cp "${CHECK}" "${DIR}/lib.sh" "${fr}/scripts/"
chmod +x "${fr}/scripts/public-api-sdk-check.sh"
# 差し替えの JSON を作る段。bash の組み込みだけで JSON を書き写す（PATH を絞った環境でも動かすため）。
cat >"${fr}/scripts/public-api-check.sh" <<'FAKE'
#!/usr/bin/env bash
case "${FAKE_EMIT:-ok}" in
  fail) exit 1 ;;
  none) exit 0 ;;
esac
[ "$1" = "--emit-json" ] || exit 1
dir="$2"
printf '%s' "$(<"${FAKE_PROD}")" >"${dir}/api.production.json"
printf '%s' "$(<"${FAKE_ALL}")" >"${dir}/api.all-features.json"
echo "emitted: crate=api features=production file=api.production.json"
echo "emitted: crate=api features=all-features file=api.all-features.json"
FAKE
chmod +x "${fr}/scripts/public-api-check.sh"
FR_CHECK="${fr}/scripts/public-api-sdk-check.sh"

fakebin="${work}/fakebin"
mkdir -p "${fakebin}"
cat >"${fakebin}/cargo" <<'FAKE'
#!/usr/bin/env bash
if [ -n "${FAKE_ARGS_LOG:-}" ]; then
  echo "$*" >>"${FAKE_ARGS_LOG}"
fi
case "${FAKE_CARGO:-ok}" in
  fail) exit 1 ;;
  badjson) echo "{not json"; exit 0 ;;
esac
case "$1" in
  metadata) cat "${FAKE_MD}" ;;
  *) exit 1 ;;
esac
FAKE
# 偽の git。FAKE_GIT の値で出力と終了コードを切り替える（既定 clean）。
cat >"${fakebin}/git" <<'FAKE'
#!/usr/bin/env bash
for a in "$@"; do
  case "$a" in
    rev-parse)
      case "${FAKE_GIT:-clean}" in
        fail-rev) exit 1 ;;
        garbage) echo "/fake/host/path not-a-commit"; exit 0 ;;
      esac
      echo "0123456789abcdef0123456789abcdef01234567"
      exit 0 ;;
    status)
      case "${FAKE_GIT:-clean}" in
        fail-status) exit 1 ;;
        dirty) echo " M crates/dummy.rs" ;;
      esac
      exit 0 ;;
  esac
done
exit 1
FAKE
chmod +x "${fakebin}/cargo" "${fakebin}/git"
fake_sha="0123456789abcdef0123456789abcdef01234567"
args_log="${work}/fake-args.log"

# run_fake <production の JSON> <all-features の JSON> <metadata> [git の挙動] [cargo の挙動] [emit の挙動]
run_fake() {
  rc=0
  rm -f "${args_log}"
  PATH="${fakebin}:${PATH}" FAKE_PROD="$1" FAKE_ALL="$2" FAKE_MD="$3" FAKE_GIT="${4:-clean}" FAKE_CARGO="${5:-ok}" \
    FAKE_EMIT="${6:-ok}" FAKE_ARGS_LOG="${args_log}" \
    "${FR_CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json"
judge "fake_ok" 0 "o:ok: 2 public API document(s) checked, common crate dependencies checked, no chip SDK references" \
  "o:record: commit=${fake_sha} documents=2 public_items_total=2 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=3 common_chip_deps=0" \
  "o:scanned: crate=dummy features=production " "o:scanned: crate=dummy features=all-features " "!-dirty" "!violation"
# cargo metadata は固定の引数（ロックを書き換えず、ネットワークを使わず、全 feature）で呼ばれる
if [ "$(grep -cxF -e "metadata --locked --offline --no-deps --format-version 1 --all-features" "${args_log}")" = "1" ] \
  && [ "$(wc -l <"${args_log}" | tr -d ' ')" = "1" ]; then
  report_ok "fake_metadata_args"
else
  report_fail "fake_metadata_args" "cargo must be called once with the fixed metadata arguments"
fi
# 変更が在れば -dirty が付く
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json" dirty
judge "fake_dirty" 0 "o:record: commit=${fake_sha}-dirty documents=2 "
# 組ごとの JSON を取り違えない（片方の組だけの違反を見逃さない）
run_fake "${work}/bad_type_ash.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json"
judge "fake_prod_only_bad" 1 "e:crate=dummy features=production item=f kind=type references=ash" "e:FAILED: 1 reference(s)" "!ok: " \
  "o:record: commit=${fake_sha} documents=2 " "o:sdk_type_refs_total=1 "
run_fake "${work}/ok_noref.json" "${work}/bad_type_cudarc.json" "${work}/m_ok_plain.json"
judge "fake_all_only_bad" 1 "e:references=cudarc" "e:FAILED: 1 reference(s)" "!ok: "
# 依存側だけの違反
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_bad_ash.json"
judge "fake_deps_bad" 1 "e:violation: check=common-dep crate=common-dummy dependency=ash kind=normal target=-" "e:FAILED: 1 reference(s)" "!ok: " \
  "o:record: commit=${fake_sha} documents=2 "
# 両側の違反は合計する
run_fake "${work}/bad_type_ash.json" "${work}/ok_noref.json" "${work}/m_bad_multi.json"
judge "fake_sum" 1 "e:FAILED: 4 reference(s)" "!ok: "

# コミットが取れなければビルドの前に終了コード 2 で止まる
for mode in fail-rev fail-status garbage; do
  run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json" "${mode}"
  judge "commit_${mode}" 2 "e:error: cannot resolve the source commit" "!record: " "!ok: " "!scanned: " "!/fake/host/path"
  if [ -e "${args_log}" ]; then
    report_fail "commit_${mode}_before_cargo" "cargo was called before the commit was resolved"
  else
    report_ok "commit_${mode}_before_cargo"
  fi
done

# cargo metadata の失敗・JSON でない出力は 2（rustdoc 側の分は scanned に出るが record と ok は出ない）
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json" clean fail
judge "fake_metadata_fails" 2 "e:error: cargo metadata failed" "!ok: " "!record: "
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json" clean badjson
judge "fake_metadata_not_json" 2 "e:error: cargo metadata output is not JSON" "!ok: " "!record: "
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_err_zero_common.json"
judge "fake_metadata_misuse" 2 "e:common crate not found exactly once" "!ok: " "!record: "

# JSON を作る段の失敗は 2
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json" clean ok fail
judge "emit_fails" 2 "e:error: public-api-check.sh --emit-json failed" "!ok: " "!record: " "!scanned: "
run_fake "${work}/ok_noref.json" "${work}/ok_noref.json" "${work}/m_ok_plain.json" clean ok none
judge "emit_no_lines" 2 "e:error: no JSON was emitted" "!ok: " "!record: " "!scanned: "

# ---- ツール未導入（成功にせず終了コード 2）----
# PATH を絞った環境で、検査を絶対パスの bash から起動する。絞った PATH に入れるのは必要なコマンドだけ。
bash_bin="$(command -v bash)"
dirname_bin="$(command -v dirname)"
py_real="$(python3 -I -c 'import sys; print(sys.executable)')"

nopy="${work}/nopy"
mkdir -p "${nopy}"
ln -s "${dirname_bin}" "${nopy}/dirname"
rc=0
PATH="${nopy}" "${bash_bin}" "${CHECK}" --metadata-json "${work}/m_ok_nodeps.json" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_python3" 2 "e:required command not found on PATH: python3" "!ok: " "!record: " "!scanned:"

# cargo だけ無い（git・bash・python3 はある）: JSON を作る段の後、metadata の起動で止まる
nocargo="${work}/nocargo"
mkdir -p "${nocargo}"
ln -s "${dirname_bin}" "${nocargo}/dirname"
ln -s "${py_real}" "${nocargo}/python3"
ln -s "${bash_bin}" "${nocargo}/bash"
cp "${fakebin}/git" "${nocargo}/git"
rc=0
PATH="${nocargo}" FAKE_PROD="${work}/ok_noref.json" FAKE_ALL="${work}/ok_noref.json" "${bash_bin}" "${FR_CHECK}" \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_cargo" 2 "e:error: cannot run cargo metadata" "!ok: " "!record: "

# git が無い: ビルドの前に止まる
nogit="${work}/nogit"
mkdir -p "${nogit}"
ln -s "${dirname_bin}" "${nogit}/dirname"
ln -s "${py_real}" "${nogit}/python3"
rc=0
PATH="${nogit}" "${bash_bin}" "${FR_CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_git" 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"

# ================= 5. 実物（一時 workspace の実物の rustdoc の JSON と cargo metadata）=================
# 一時 workspace に検査と lib.sh と public-api-check.sh を写して流す。チップの SDK の名前の合成 path crate は
# ここで作る（中身のないダミー。crates.io からは取らない）。マニフェストはリポジトリに置かない。
# 依存は path だけなのでネットワークは使わない。target は一時 workspace の中に出る。
rw="${work}/real"
mkdir -p "${rw}/ws/crates/api/src" "${rw}/ws/crates/core/src" "${rw}/ws/scripts"
for dummy in ash cudarc objc2-metal plain-dummy; do
  mkdir -p "${rw}/ext/${dummy}/src"
  cat >"${rw}/ext/${dummy}/Cargo.toml" <<TOML
[package]
name = "${dummy}"
version = "0.0.0"
edition = "2024"
TOML
  printf 'pub struct Handle;\npub trait Tr {}\npub struct Thing;\n' >"${rw}/ext/${dummy}/src/lib.rs"
done
cat >"${rw}/ws/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/api", "crates/core"]
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
ash = { path = "../../../ext/ash" }
cudarc = { path = "../../../ext/cudarc" }
objc2-metal = { path = "../../../ext/objc2-metal" }
plain-dummy = { path = "../../../ext/plain-dummy" }
TOML
printf 'pub struct Own;\n' >"${rw}/ws/crates/core/src/lib.rs"
cp "${DIR}/public-api-sdk-check.sh" "${DIR}/public-api-check.sh" "${DIR}/lib.sh" "${rw}/ws/scripts/"
chmod +x "${rw}/ws/scripts/public-api-sdk-check.sh" "${rw}/ws/scripts/public-api-check.sh"
if [ -f "${DIR}/../rust-toolchain.toml" ]; then
  cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi

# real_lib <lib.rs の中身>: api の公開面を差し替える。
real_lib() {
  printf '%s\n' "$1" >"${rw}/ws/crates/api/src/lib.rs"
}
# real_core <追記する Cargo.toml の節>: core の manifest を作り直し、lock を更新する（--locked の検査のため）。
real_core() {
  {
    printf '[package]\nname = "common-dummy"\nversion = "0.0.0"\nedition = "2024"\n'
    printf '%s\n' "$1"
  } >"${rw}/ws/crates/core/Cargo.toml"
  (cd "${rw}/ws" && cargo generate-lockfile --offline) >/dev/null 2>"${ERR_F}"
}
# real_git: 開発者の全体設定とフックを効かせない git。
real_git() {
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
    -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# run_real: 写した検査を引数なしで流す。共有の target ディレクトリへ書かせない。
run_real() {
  rc=0
  (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR ./scripts/public-api-sdk-check.sh) \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}
# real_line_has <接頭辞> <語>: 標準出力の指定の接頭辞の行に語が含まれること。
real_line_has() {
  grep "^$1" "${OUT_F}" | grep -qF -e "$2"
}

printf 'target/\n' >"${rw}/ws/.gitignore"
real_lib 'pub struct Own;'
if real_core '' \
  && real_git init -q >/dev/null 2>>"${ERR_F}" \
  && real_git add -A >/dev/null 2>>"${ERR_F}" \
  && real_git commit -q -m fixture >/dev/null 2>>"${ERR_F}"; then
  # 合格: api は ash などに依存しているが公開 API には出さない（依存側の検査は共通の crate だけ）
  real_lib 'pub fn g() -> plain_dummy::Thing { plain_dummy::Thing }'
  # 記録の commit が -dirty でないことも確かめるため、公開面の変更をコミットしてから流す
  real_git add -A >/dev/null 2>&1
  real_git commit -q -m fixture-api >/dev/null 2>&1
  run_real
  judge "real_clean" 0 "o:ok: 4 public API document(s) checked, common crate dependencies checked, no chip SDK references" \
    "o:scanned: check=common-deps manifest=crates/core/Cargo.toml package=common-dummy declared_deps=0 chip_deps=0" \
    "o:scanned: crate=api features=production " "o:scanned: crate=api features=all-features " \
    "o:sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0" "!violation"
  if grep -qE '^record: commit=[0-9a-f]{40} documents=4 ' "${OUT_F}"; then
    report_ok "real_clean_record_commit"
  else
    report_fail "real_clean_record_commit" "record must carry the clean HEAD commit"
  fi
  if real_line_has "scanned: crate=api features=production " "external_crates=plain_dummy"; then
    report_ok "real_clean_external_crates"
  else
    report_fail "real_clean_external_crates" "api external crates must list plain_dummy"
  fi

  # 型として出す（3 つの名前。production と all-features の両方で数える）
  real_lib 'pub fn a() -> ash::Handle { ash::Handle }
pub fn c() -> cudarc::Handle { cudarc::Handle }
pub fn m() -> objc2_metal::Handle { objc2_metal::Handle }'
  run_real
  judge "real_type_leak" 1 "e:features=production item=a kind=type references=ash" \
    "e:features=all-features item=a kind=type references=ash" \
    "e:features=production item=c kind=type references=cudarc" "e:features=all-features item=c kind=type references=cudarc" \
    "e:features=production item=m kind=type references=objc2_metal" "e:features=all-features item=m kind=type references=objc2_metal" \
    "e:FAILED: 6 reference(s)" "o:sdk_type_refs_total=6 " "!ok: "

  # feature の裏の漏れは all-features の組だけに出る
  real_lib '#[cfg(feature = "leak")]
pub fn leak() -> objc2_metal::Handle { objc2_metal::Handle }'
  run_real
  judge "real_type_leak_feature" 1 "e:features=all-features item=leak kind=type references=objc2_metal" \
    "e:FAILED: 1 reference(s)" "o:scanned: crate=api features=production" "!ok: "
  # production の組の行に参照 0 件が出ていること（all-features の行と取り違えて合格しない）
  if real_line_has "scanned: crate=api features=production " "sdk_type_refs=0 sdk_other_refs=0"; then
    report_ok "real_type_leak_feature_production_clean"
  else
    report_fail "real_type_leak_feature_production_clean" "production line must have zero refs"
  fi
  if grep -q 'features=production item=leak' "${ERR_F}"; then
    report_fail "real_type_leak_feature_not_in_production" "production must not report the feature-gated leak"
  else
    report_ok "real_type_leak_feature_not_in_production"
  fi

  # 再公開
  real_lib 'pub use cudarc::Tr;'
  run_real
  judge "real_reexport" 1 "e:kind=other references=cudarc" "!ok: "

  # 共通の crate の依存側: 通常の依存（公開 API には出さない）
  real_lib 'pub fn g() -> plain_dummy::Thing { plain_dummy::Thing }'
  real_core '[dependencies]
ash = { path = "../../../ext/ash" }
plain-dummy = { path = "../../../ext/plain-dummy" }'
  run_real
  judge "real_core_dep" 1 "e:violation: check=common-dep crate=common-dummy dependency=ash kind=normal target=-" \
    "e:FAILED: 1 reference(s)" "o:declared_deps=2 chip_deps=1" "o:sdk_type_refs_total=0 sdk_other_refs_total=0 " "o:common_chip_deps=1" "!ok: " "!violation: crate="

  # dev の依存と、target 付きの build の依存
  real_core '[dev-dependencies]
objc2-metal = { path = "../../../ext/objc2-metal" }

[target.'"'"'cfg(unix)'"'"'.build-dependencies]
cudarc = { path = "../../../ext/cudarc" }'
  run_real
  judge "real_core_dev_and_target_build" 1 "e:dependency=objc2-metal kind=dev target=-" "e:dependency=cudarc kind=build target=cfg" \
    "e:FAILED: 2 reference(s)" "o:chip_deps=2" "!ok: "

  # optional の依存も数える
  real_core '[dependencies]
cudarc = { path = "../../../ext/cudarc", optional = true }'
  run_real
  judge "real_core_optional_dep" 1 "e:dependency=cudarc kind=normal target=-" "e:FAILED: 1 reference(s)" "!ok: "

  # 共通の crate がチップでない依存だけなら合格に戻る
  real_core '[dependencies]
plain-dummy = { path = "../../../ext/plain-dummy" }'
  run_real
  judge "real_core_plain_dep" 0 "o:declared_deps=1 chip_deps=0" "o:ok: " "!violation"
else
  report_fail "real_fixture_setup" "cannot prepare the temporary workspace"
fi

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
