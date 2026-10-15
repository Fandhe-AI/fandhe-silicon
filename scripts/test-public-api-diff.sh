#!/usr/bin/env bash
# public-api-diff.sh の回帰テスト（spec TASK-17 のポインタ。親 Issue #65 の 1/5）。
# 一時ディレクトリの合成 rustdoc JSON（ダミーの名前）を 2 本渡し、終了コード・標準出力・標準エラーの文言を
# 確かめる。cargo もネットワークも `docs/spec` の取得も使わず、リポジトリのファイルを変更しない。
# 呼び出し元: `make test-public-api-diff`（直接実行も可。CI の `deny` ジョブも呼ぶ）。
# 前提: python3（無ければ失敗にし、skip しない）。
#
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う。終了コードだけでは別の理由で落ちても
# 気付けず、検査が素通りする退行（fail-open）を見逃すため。
# 期待値を緩めない。現行の道具と食い違ったらテストでなく道具側の挙動として直す。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

TOOL="${DIR}/public-api-diff.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-public-api-diff.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成 JSON をまとめて作る。ファイル名は ${work}/<ケース名>.json。
python3 -I - "${work}" <<'PY'
import json
import os
import sys

out = sys.argv[1]
ESC = chr(27)

# 基本の公開 API（ダミー）: (kind, path, visibility)。
BASE = [
    ("module", ["dummy"], "public"),
    ("function", ["dummy", "f"], "public"),
    ("struct", ["dummy", "S"], "public"),
    ("enum", ["dummy", "E"], "public"),
    ("variant", ["dummy", "E", "A"], "default"),
]


def doc(items, base_id=0, order=1, extra=None, fv=61, root_name="dummy", mutate=None):
    """items から rustdoc 風の JSON を作る。id は base_id から order 方向に振る（版違いで変わる再現）。"""
    ids = list(range(base_id, base_id + len(items)))
    if order < 0:
        ids.reverse()
    index = {}
    paths = {}
    for iid, (kind, path, vis) in zip(ids, items):
        index[str(iid)] = {"id": iid, "crate_id": 0, "name": path[-1], "visibility": vis, "inner": {kind: {}}}
        paths[str(iid)] = {"crate_id": 0, "kind": kind, "path": path}
    root_id = ids[0]
    index[str(root_id)]["name"] = root_name
    d = {
        "root": root_id,
        "format_version": fv,
        "includes_private": False,
        "external_crates": {"1": {"name": "other_lib"}},
        "paths": paths,
        "index": index,
    }
    if extra:
        extra(d)
    if mutate:
        mutate(d)
    return d


def ext(d):
    """外部 crate のアイテムを index・paths の両方へ足す。"""
    d["index"]["900"] = {"id": 900, "crate_id": 1, "name": "X", "visibility": "public", "inner": {"struct": {}}}
    d["paths"]["900"] = {"crate_id": 1, "kind": "struct", "path": ["other_lib", "X"]}


cases = {}
cases["base"] = doc(BASE)
cases["renumbered"] = doc(BASE, base_id=500, order=-1)
cases["plus_g"] = doc(BASE + [("function", ["dummy", "g"], "public")], base_id=40)
cases["minus_f"] = doc([i for i in BASE if i[1] != ["dummy", "f"]])
cases["plus_g_minus_f"] = doc([i for i in BASE if i[1] != ["dummy", "f"]] + [("function", ["dummy", "g"], "public")])
cases["kind_changed"] = doc([("constant", ["dummy", "f"], "public") if i[1] == ["dummy", "f"] else i for i in BASE])
cases["private_added"] = doc(BASE + [("function", ["dummy", "hidden"], "crate")])
cases["restricted_added"] = doc(
    BASE + [("function", ["dummy", "scoped"], {"restricted": {"parent": 0, "path": "::dummy"}})]
)
cases["external_added"] = doc(BASE, extra=ext)
cases["path_unindexed_added"] = doc(
    BASE, extra=lambda d: d["paths"].update({"901": {"crate_id": 0, "kind": "function", "path": ["dummy", "ghost"]}})
)
cases["fv_62"] = doc(BASE, fv=62)
cases["other_root"] = doc(BASE, root_name="another")
cases["includes_private"] = doc(BASE, mutate=lambda d: d.update({"includes_private": True}))
cases["fv_str"] = doc(BASE, mutate=lambda d: d.update({"format_version": "61"}))
cases["not_object"] = ["x"]
cases["missing_index"] = doc(BASE, mutate=lambda d: d.pop("index"))
cases["missing_root"] = doc(BASE, mutate=lambda d: d.pop("root"))
cases["bad_paths_entry"] = doc(BASE, mutate=lambda d: d["paths"].update({"902": "oops"}))
cases["bad_paths_path"] = doc(BASE, mutate=lambda d: d["paths"].update({"902": {"crate_id": 0, "kind": "function", "path": [1]}}))
cases["bad_paths_crate_id"] = doc(BASE, mutate=lambda d: d["paths"].update({"902": {"crate_id": True, "kind": "function", "path": ["a"]}}))
cases["root_no_name"] = doc(BASE, mutate=lambda d: d["index"][str(d["root"])].pop("name"))
cases["root_foreign"] = doc(BASE, mutate=lambda d: d["index"][str(d["root"])].update({"crate_id": 1}))
cases["no_public_items"] = doc([("module", ["dummy"], "crate")])
cases["escape_path"] = doc(BASE + [("function", ["dummy", "bad" + ESC + "[31mname"], "public")])
cases["deep"] = None  # 下で文字列として書く

for name, d in cases.items():
    if d is None:
        continue
    with open(os.path.join(out, name + ".json"), "w", encoding="utf-8") as f:
        json.dump(d, f)

with open(os.path.join(out, "notjson.json"), "w", encoding="utf-8") as f:
    f.write("{not json")
with open(os.path.join(out, "deep.json"), "w", encoding="utf-8") as f:
    f.write("[" * 200000)
with open(os.path.join(out, "badutf8.json"), "wb") as f:
    f.write(b"\xff\xfe\x00")
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

# run_tool <引数...>: 道具を実行し rc・標準出力・標準エラーを控える。
run_tool() {
  rc=0
  "${TOOL}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# judge <名前> <期待コード> [語...]: o:<語> は標準出力に、e:<語> は標準エラーに含まれること。!<語> はどちらにも含まれないこと。
# どのケースでも一時ディレクトリの絶対パス（引数のパス）が出力に出ないことも確かめる。
judge() {
  local name="$1" want="$2" s
  shift 2
  if [ "${rc}" -ne "${want}" ]; then
    report_fail "${name}" "expected exit ${want}, got ${rc}"
    return 0
  fi
  if grep -qF -e "${work}" "${ERR_F}" "${OUT_F}"; then
    report_fail "${name}" "output must not contain the argument path"
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

# same <名前> <a> <b> [語...]: 差分なし（終了コード 0）。
same() {
  local name="$1" a="$2" b="$3"
  shift 3
  run_tool "${work}/${a}.json" "${work}/${b}.json"
  judge "${name}" 0 "o:ok: no public API differences" "o:summary: added=0 removed=0 unchanged=" "!added:" "!removed:" "!DIFFERENT" "$@"
}

# differ <名前> <a> <b> [語...]: 差分あり（終了コード 1）。
differ() {
  local name="$1" a="$2" b="$3"
  shift 3
  run_tool "${work}/${a}.json" "${work}/${b}.json"
  judge "${name}" 1 "e:DIFFERENT: " "o:summary: " "!ok: " "$@"
}

# misuse <名前> <a> <b> [語...]: 前提の不足（終了コード 2・error の行・成功や締めの行なし）。
misuse() {
  local name="$1" a="$2" b="$3"
  shift 3
  run_tool "${work}/${a}.json" "${work}/${b}.json"
  judge "${name}" 2 "e:error: " "!ok: " "!summary: " "!DIFFERENT" "$@"
}

# ---- 差分なし ----

same same base base "o:summary: added=0 removed=0 unchanged=5 format_version=61"
# id を全部付け替えて並びも入れ替えても差分なし（id で比べないことの確かめ）
same renumbered base renumbered "o:unchanged=5"
same renumbered_rev renumbered base
# 非公開・restricted・外部 crate・index に無い id は数えない
same private_added base private_added
same restricted_added base restricted_added
same external_added base external_added
same path_unindexed_added base path_unindexed_added

# ---- 差分あり ----

differ added base plus_g "o:added: kind=function path=dummy::g" "o:summary: added=1 removed=0 unchanged=5" "e:DIFFERENT: 1 difference" "!removed:"
differ removed base minus_f "o:removed: kind=function path=dummy::f" "o:summary: added=0 removed=1 unchanged=4" "!added:"
differ added_and_removed base plus_g_minus_f "o:removed: kind=function path=dummy::f" "o:added: kind=function path=dummy::g" "o:summary: added=1 removed=1 unchanged=4" "e:DIFFERENT: 2 difference"
differ kind_changed base kind_changed "o:removed: kind=function path=dummy::f" "o:added: kind=constant path=dummy::f" "o:summary: added=1 removed=1 unchanged=4"
# 向きを逆にすると added と removed が入れ替わる
differ added_reversed plus_g base "o:removed: kind=function path=dummy::g" "!added:"

# 出力は removed → added → summary の順で決定的
run_tool "${work}/base.json" "${work}/plus_g_minus_f.json"
if [ "$(grep -n -e '^removed: ' "${OUT_F}" | cut -d: -f1)" = "1" ] && [ "$(grep -n -e '^added: ' "${OUT_F}" | cut -d: -f1)" = "2" ] \
  && [ "$(grep -n -e '^summary: ' "${OUT_F}" | cut -d: -f1)" = "3" ]; then
  report_ok "output_order"
else
  report_fail "output_order" "expected removed, added, summary in this order"
fi

# 制御文字は生のまま出さず ? に置き換える
run_tool "${work}/base.json" "${work}/escape_path.json"
esc="$(printf '\033')"
if [ "${rc}" -eq 1 ] && ! grep -qF -e "${esc}" "${OUT_F}" "${ERR_F}" && grep -qF -e "path=dummy::bad??31mname" "${OUT_F}"; then
  report_ok "sanitized_path"
else
  report_fail "sanitized_path" "expected exit 1 with the escape and bracket replaced by ?"
fi

# ---- 前提の不足 ----

misuse format_version_mismatch base fv_62
misuse format_version_mismatch_rev fv_62 base
misuse root_name_mismatch base other_root
misuse includes_private_true base includes_private
misuse includes_private_true_first includes_private base
misuse format_version_string base fv_str
misuse not_object base not_object
misuse missing_index base missing_index
misuse missing_root base missing_root
misuse malformed_paths_entry base bad_paths_entry
misuse malformed_paths_path base bad_paths_path
misuse malformed_paths_crate_id base bad_paths_crate_id
misuse root_without_name base root_no_name
misuse root_foreign_crate base root_foreign
misuse no_public_items base no_public_items
misuse no_public_items_first no_public_items base
misuse notjson base notjson
misuse notjson_first notjson base
misuse badutf8 base badutf8 "!utf"
# 深すぎる入れ子は違反の 1 と読ませず 2 で止める
misuse deep_nesting base deep

run_tool "${work}/base.json" "${work}/missing_file.json"
judge "missing_file" 2 "e:error: " "!ok: " "!summary: " "!missing_file"
run_tool "${work}/base.json" "${work}"
judge "directory_as_input" 2 "e:error: " "!ok: " "!summary: "

# ---- 誤用 ----

run_tool
judge "args_none" 2 "e:usage: " "!ok: " "!summary: "
run_tool "${work}/base.json"
judge "args_one" 2 "e:usage: " "!ok: " "!summary: "
run_tool "${work}/base.json" "${work}/base.json" "${work}/base.json"
judge "args_three" 2 "e:usage: " "!ok: " "!summary: "
# 未知のオプションの語は出力に出さない
run_tool --bogusflagword "${work}/base.json" "${work}/base.json"
judge "unknown_option" 2 "e:usage: " "!bogusflagword" "!ok: " "!summary: "

# ---- ファイルを作らない ----

empty="${work}/empty_cwd"
mkdir "${empty}"
rc=0
(cd "${empty}" && "${TOOL}" "${work}/base.json" "${work}/plus_g.json" >"${OUT_F}" 2>"${ERR_F}" </dev/null) || rc=$?
if [ "${rc}" -eq 1 ] && [ -z "$(ls -A "${empty}")" ]; then
  report_ok "creates_no_files"
else
  report_fail "creates_no_files" "expected exit 1 and an empty working directory"
fi

echo "test-public-api-diff: passed=${passed} failed=${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
