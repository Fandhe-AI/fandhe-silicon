#!/usr/bin/env bash
# public-api-webgpu-check.sh の回帰テスト（spec TASK-241・BUILD-29・BUILD-31・BUILD-32 のポインタ。親 Issue #465 の 3/6・4/6・5/6。
# 内容の (1) の検査の試験と、内容の (2)〔パイプライン生成の入力の形〕の試験と、内容の (3)〔拡張の trait の閉じ込めと submit 系の wait の形〕の試験）。
# 一時ディレクトリに、ダミーの語の一覧と合成 rustdoc JSON を作って検査へ渡し、終了コード・標準出力・標準エラーの文言を確かめる。
#   前半は --json の経路（合成 JSON。cargo・git を呼ばない）。実物の語の一覧が書式どおり読めることと、
#   その各語が当たることも、設定から実行時に入力を作って確かめる（語はテストに書き写さない）。
#   後半は一時 workspace に検査・public-api-check.sh・lib.sh を写し、package 名がちょうど fandhe-silicon-core と
#   fandhe-silicon-contract の依存なしの 2 crate を置いて、実物の cargo rustdoc の JSON で既定の経路
#   （feature native-blocking-wait を外した構成・commit の記録・違反を 1 件足すと落ちること・終了コード 2 の経路）を通す。
#   (2) は合成 JSON〔core の文書の形・生成の関数の入力・形の崩れ〕と、一時 workspace の core に PipelineInput を置いた実物
#   〔印を外す・Wgsl を外す・2 つ目の定義・生成の関数を足すと落ちること〕で通す。生成の語は検査本体の定数から実行時に読む。
#   (3) は合成 JSON〔組 A は --json、組 B は --interop-json。規則 G1〜G3 の合格・違反・形の崩れ〕と、一時 workspace の実物
#   〔feature ext-interop の裏の拡張 trait は 0・裏から出すと落ちること、execution の型と submit 系の wait の形、
#   2 回目の JSON を作る段だけ失敗させる差し替え〕で通す。
#   ネットワーク不要・リポジトリを変更しない。
# 呼び出し元: `make test-public-api-webgpu-check`（直接実行も可。CI の `deny` ジョブも実行する）。
#   前提: python3・cargo・git（無ければ失敗。skip しない）。
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う（終了コードだけでは別の理由の失敗を見逃す）。
# 終了コード 2 のケースは、締めの `ok:`・`record:` を出していないことも必ず見る（失敗を成功扱いにしない）。
# 期待値を緩めない。食い違ったらテストでなく検査側の挙動として直す。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3
require_cmd cargo
require_cmd git

CHECK="${DIR}/public-api-webgpu-check.sh"
REAL_CONF="${DIR}/../public-api-webgpu-check.conf"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-public-api-webgpu-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成 JSON を作る。ファイル名は ${work}/<ケース名>.json。
python3 -I - "${work}" "${CHECK}" <<'PY'
import ast
import json
import os
import re
import sys

out = sys.argv[1]


def item(iid, name, inner, vis="public", crate_id=0):
    return {"id": iid, "crate_id": crate_id, "name": name, "visibility": vis, "inner": inner}


def base():
    return {
        "root": 0,
        "format_version": 61,
        "includes_private": False,
        "external_crates": {},
        "paths": {},
        "index": {
            "0": item(0, "dummy", {"module": {"is_crate": True, "items": [1]}}),
            "1": item(1, "Plain", {"struct": {}}),
        },
    }


def put(name, doc):
    with open(os.path.join(out, name + ".json"), "w", encoding="utf-8") as f:
        json.dump(doc, f)


put("clean", base())

# 公開の struct だけ。語と同じ名前でも関数でも再公開でもなければ数えない。
d = base()
d["index"]["2"] = item(2, "zzblock", {"struct": {}})
d["index"]["3"] = item(3, "Zzwait_Thing", {"enum": {}})
put("nonfunction", d)

# 非公開・他 crate の関数は数えない。
d = base()
d["index"]["2"] = item(2, "zzblock_private", {"function": {}}, vis="crate")
d["index"]["3"] = item(3, "zzblock_ext", {"function": {}}, crate_id=1)
put("nonpublic", d)

# 部分一致では当てない（`_` 区切りの語の等号だけ）。
d = base()
d["index"]["2"] = item(2, "nonzzblock", {"function": {}})
d["index"]["3"] = item(3, "zzwaiting", {"function": {}})
d["index"]["4"] = item(4, "zzblockwait_all", {"function": {}})
d["index"]["5"] = item(5, "await_it", {"function": {}})
put("partial", d)

# 同じ crate の中を指す glob の再公開は、中のアイテムを直接数えるので 0 件で通る。
d = base()
d["index"]["2"] = item(2, "inner", {"module": {"items": []}})
d["index"]["3"] = item(3, "glob", {"use": {"name": "*", "id": 2, "is_glob": True, "source": "inner"}})
put("glob_internal", d)

d = base()
d["index"]["2"] = item(2, "zzblock_fn", {"function": {}})
put("fn_hit", d)

# trait の impl のメソッド（visibility default）も当たる。
d = base()
d["index"]["2"] = item(2, "zzwait_m", {"function": {}}, vis="default")
put("method_default", d)

d = base()
d["index"]["2"] = item(2, "Zzwait_For", {"function": {}})
put("case_hit", d)

# 非 glob の再公開は name と use.name が同じ局所名。1 件を 2 回数えない。
d = base()
d["index"]["2"] = item(2, "zzblock_x", {"use": {"name": "zzblock_x", "source": "x", "id": None, "is_glob": False}})
put("use_same", d)

# name と use.name が別の語で、どちらも当たれば 2 件。
d = base()
d["index"]["2"] = item(2, "zzblock_a", {"use": {"name": "zzwait_b", "source": "x", "id": None, "is_glob": False}})
put("use_diff", d)

# 1 つの名前に 2 語が含まれても、語ごとに 1 件。
d = base()
d["index"]["2"] = item(2, "zzblock_zzwait", {"function": {}})
put("two_words", d)

# 制御文字を含む名前は出力で ? に置き換わる。
d = base()
d["index"]["2"] = item(2, "zzblock_\u0001", {"function": {}})
put("ctrl_name", d)

d = base()
d["index"]["2"] = item(2, "ext", {"module": {"items": []}}, crate_id=1)
d["index"]["3"] = item(3, "glob", {"use": {"name": "*", "id": 2, "is_glob": True, "source": "ext"}})
put("glob_external", d)

d = base()
d["index"]["3"] = item(3, "glob", {"use": {"name": "*", "id": 99, "is_glob": True, "source": "nowhere"}})
put("glob_unresolved", d)

d = base()
d["includes_private"] = True
put("bad_private", d)
for key in ("index", "paths", "external_crates"):
    d = base()
    del d[key]
    put("bad_no_" + key, d)
d = base()
del d["root"]
put("bad_no_root", d)
d = base()
d["format_version"] = "61"
put("bad_fv", d)
d = base()
d["index"]["1"]["crate_id"] = "0"
put("bad_crate_id", d)
d = base()
d["index"]["0"]["crate_id"] = 1
put("bad_root_crate", d)
d = base()
d["index"]["2"] = {"id": 2, "crate_id": 0, "name": "no_inner", "visibility": "public"}
put("bad_no_inner", d)
d = base()
d["index"] = {"0": item(0, "dummy", {"module": {}}, vis="crate")}
put("zero_public", d)
# ---- (2): パイプライン生成の入力の形 ----
PI = "PipelineInput"


def core_base(extra=None):
    d = {
        "root": 0,
        "format_version": 61,
        "includes_private": False,
        "external_crates": {},
        "paths": {},
        "index": {
            "0": item(0, "fandhe_silicon_core", {"module": {"is_crate": True, "items": [1]}}),
            "1": item(1, PI, {"enum": {"variants": [3, 4]}}),
            "3": item(3, "Wgsl", {"variant": {}}, vis="default"),
            "4": item(4, "Msl", {"variant": {}}, vis="default"),
        },
    }
    d["index"]["1"]["attrs"] = ["non_exhaustive"]
    for k, v in (extra or {}).items():
        d["index"][k] = v
    return d


def rp(path):
    return {"resolved_path": {"path": path, "id": 1, "args": None}}


def ref(t):
    return {"borrowed_ref": {"lifetime": None, "is_mutable": False, "type": t}}


def pfn(iid, name, inputs, vis="public", generics=None):
    sig = {"inputs": [["a%d" % i, t] for i, t in enumerate(inputs)], "output": None, "is_c_variadic": False}
    fn = {"sig": sig, "generics": generics or {"params": [], "where_predicates": []}}
    return item(iid, name, {"function": fn}, vis=vis)


def into_bound():
    return {"trait_bound": {"trait": {"path": "Into", "id": 9, "args": {"angle_bracketed": {"args": [{"type": rp(PI)}]}}}}}


BYTES = ref({"slice": {"primitive": "u8"}})
put("core_ok", core_base())
put("core_fn_ok", core_base({"5": pfn(5, "create_pipeline", [rp(PI)])}))
put("core_fn_ref", core_base({"5": pfn(5, "create_pipeline", [ref(ref(rp(PI)))])}))
put("core_fn_path", core_base({"5": pfn(5, "create_pipeline", [rp("fandhe_silicon_core::execution::" + PI)])}))
put("core_fn_second_arg", core_base({"5": pfn(5, "create_pipeline", [BYTES, rp(PI)])}))
put("core_fn_impl_into", core_base({"5": pfn(5, "create_pipeline", [{"impl_trait": [into_bound()]}])}))
gen = {"params": [{"name": "T", "kind": {"type": {"bounds": [into_bound()]}}}], "where_predicates": []}
put("core_fn_generic", core_base({"5": pfn(5, "create_pipeline", [{"generic": "T"}], generics=gen)}))
# 名前が当たらない関数は sig を読まない（sig が無くても通る）。
put("core_fn_unrelated", core_base({
    "5": item(5, "pipeline_count", {"function": {}}),
    "6": item(6, "create_buffer", {"function": {}}),
    "7": item(7, "Pipeline_Handle", {"function": {}}),
}))
# 非公開の生成の関数は数えない。
put("core_fn_private", core_base({"5": pfn(5, "create_pipeline", [BYTES], vis="crate")}))

d = core_base()
d["index"]["1"]["attrs"] = []
put("core_no_attr", d)
d = core_base()
d["index"]["1"]["attrs"] = [{"other": "#[derive(Debug)]"}]
put("core_other_attr", d)
d = core_base()
d["index"]["1"]["attrs"] = [{"other": "#[non_exhaustive]"}]
put("core_old_attr", d)
d = core_base()
d["index"]["1"]["inner"]["enum"]["variants"] = [4]
put("core_no_wgsl", d)
d = core_base()
d["index"]["1"]["inner"] = {"struct": {}}
put("core_not_enum", d)
d = core_base({"8": item(8, PI, {"enum": {"variants": [3]}})})
d["index"]["8"]["attrs"] = ["non_exhaustive"]
put("core_dup", d)
d = core_base()
del d["index"]["1"]
d["index"]["0"]["inner"]["module"]["items"] = []
put("core_missing", d)
# 再公開は数えない（定義は 1 つのまま）。
put("core_reexport", core_base({"8": item(8, PI, {"use": {"name": PI, "source": "x", "id": 1, "is_glob": False}})}))
d = base()
d["index"]["2"] = item(2, PI, {"enum": {"variants": [3]}})
d["index"]["3"] = item(3, "Wgsl", {"variant": {}}, vis="default")
d["index"]["2"]["attrs"] = ["non_exhaustive"]
put("dummy_with_pi", d)

put("fn_bytes", core_base({"5": pfn(5, "create_pipeline", [BYTES])}))
put("fn_upper", core_base({"5": pfn(5, "Create_Pipeline_Raw", [BYTES])}))
put("fn_method", core_base({"5": pfn(5, "build_pipeline", [BYTES], vis="default")}))
put("fn_two", core_base({"5": pfn(5, "create_pipeline", [BYTES]), "6": pfn(6, "new_pipeline", [])}))
put("fn_blocking_and_pipeline", core_base({
    "5": pfn(5, "create_pipeline", [BYTES]),
    "6": item(6, "zzblock_fn", {"function": {}}),
}))
src = open(sys.argv[2], encoding="utf-8").read()
m = re.search(r"^CREATE_WORDS = (\(.*?\))$", src, re.M)
if not m:
    sys.exit("CREATE_WORDS not found")
for i, w in enumerate(ast.literal_eval(m.group(1))):
    put("cw%d" % i, core_base({"5": pfn(5, "x_pipeline_%s" % w.upper(), [BYTES])}))

d = core_base()
d["index"]["1"]["attrs"] = "non_exhaustive"
put("bad_attrs", d)
d = core_base()
d["index"]["1"]["inner"]["enum"]["variants"] = "x"
put("bad_variants", d)
d = core_base()
d["index"]["1"]["inner"]["enum"]["variants"] = [99]
put("bad_variant_id", d)
d = core_base()
d["index"]["1"]["inner"]["enum"]["variants"] = [0]
put("bad_variant_kind", d)
put("bad_nosig", core_base({"5": item(5, "create_pipeline", {"function": {}})}))
d = core_base({"5": pfn(5, "create_pipeline", [])})
d["index"]["5"]["inner"]["function"]["sig"]["inputs"] = "x"
put("bad_inputs", d)
# ---- (3): 拡張の trait の閉じ込めと submit 系の wait の形 ----
SEM = "ImportedSemaphoreHandle"
EXEC_PATH = ["fandhe_silicon_core", "execution", SEM]


def with_handle(d, loc, iid="20", vis="public"):
    d["index"][iid] = item(int(iid), SEM, {"struct": {}}, vis=vis)
    d["paths"][iid] = {"crate_id": 0, "path": loc, "kind": "struct"}
    return d


def hrp(iid=20, path=SEM):
    return {"resolved_path": {"path": path, "id": iid, "args": None}}


def sref(elem, mut=False):
    return {"borrowed_ref": {"lifetime": None, "is_mutable": mut, "type": {"slice": elem}}}


def sfn(iid, name, params, vis="public"):
    sig = {"inputs": [[n, t] for n, t in params], "output": None, "is_c_variadic": False}
    return item(iid, name, {"function": {"sig": sig, "generics": {"params": [], "where_predicates": []}}}, vis=vis)


def wdoc(params, name="submit", vis="public", loc=None):
    d = with_handle(core_base(), loc or EXEC_PATH)
    d["index"]["30"] = sfn(30, name, params, vis=vis)
    return d


GOOD_WAIT = [("self", {"generic": "Self"}), ("wait", sref(hrp()))]
put("i_ok", wdoc(GOOD_WAIT))
put("i_ok_render", wdoc(GOOD_WAIT, name="submit_render"))
put("i_ok_method", wdoc(GOOD_WAIT, name="Submit_Ex", vis="default"))
d = wdoc(GOOD_WAIT)
d["index"]["31"] = sfn(31, "submit_render", GOOD_WAIT)
put("i_ok_two", d)
put("i_mut", wdoc([("wait", sref(hrp(), mut=True))]))
put("i_vec", wdoc([("wait", {"resolved_path": {"path": "Vec", "id": 5, "args": None}})]))
put("i_ref_handle", wdoc([("wait", {"borrowed_ref": {"lifetime": None, "is_mutable": False, "type": hrp()}})]))
put("i_array", wdoc([("wait", {"array": {"type": hrp(), "len": "2"}})]))
put("i_u32", wdoc([("wait", sref({"primitive": "u32"}))]))
put("i_other_name", wdoc([("wait", sref(hrp(20, "Other")))]))
d = wdoc([("wait", sref(hrp(21)))])
d["paths"]["21"] = {"crate_id": 0, "path": ["fandhe_silicon_core", "other", SEM], "kind": "struct"}
put("i_outside", d)
d = wdoc([("wait", sref(hrp(21)))])
d["paths"]["21"] = {"crate_id": 0, "path": ["fandhe_silicon_core", SEM], "kind": "struct"}
put("i_outside_root", d)
put("i_nowait", wdoc([("self", {"generic": "Self"}), ("waits", {"primitive": "u32"})]))
put("i_not_submit", wdoc([("wait", {"primitive": "u32"})], name="resubmit"))
put("i_private_submit", wdoc([("wait", {"primitive": "u32"})], vis="crate"))
d = with_handle(core_base(), EXEC_PATH)
d["index"]["30"] = item(30, "submit", {"function": {}})
put("i_bad_nosig", d)
d = wdoc([])
d["index"]["30"]["inner"]["function"]["sig"]["inputs"] = "x"
put("i_bad_inputs", d)
d = wdoc([])
d["index"]["30"]["inner"]["function"]["sig"]["inputs"] = [["wait"]]
put("i_bad_entry", d)
put("i_bad_waittype", wdoc([("wait", "x")]))
put("i_bad_waitid", wdoc([("wait", sref(hrp(99)))]))
d = wdoc([("wait", sref(hrp(21)))])
put("i_bad_waitid_nopaths", d)

# 定義 (G2)
put("h_ok", with_handle(core_base(), EXEC_PATH))
put("h_root", with_handle(core_base(), ["fandhe_silicon_core", SEM]))
put("h_other_mod", with_handle(core_base(), ["fandhe_silicon_core", "other", SEM]))
put("h_dummy", with_handle(base(), ["dummy", SEM]))
d = with_handle(core_base(), EXEC_PATH)
with_handle(d, EXEC_PATH, iid="21")
put("h_dup", d)
d = core_base()
d["index"]["20"] = item(20, SEM, {"struct": {}})
put("h_nopaths", d)
d = core_base()
d["index"]["20"] = item(20, SEM, {"struct": {}})
d["paths"]["20"] = {"crate_id": 0, "path": "x", "kind": "struct"}
put("h_badpath", d)
# 再公開・variant・関数は定義に数えない。
d = core_base()
d["index"]["20"] = item(20, SEM, {"use": {"name": SEM, "source": "x", "id": None, "is_glob": False}})
d["index"]["21"] = item(21, SEM, {"variant": {}}, vis="default")
d["index"]["22"] = item(22, SEM, {"function": {}})
put("h_not_defs", d)

# 拡張の trait (G1)
EI = "ExternalInterop"
d = core_base()
d["index"]["30"] = item(30, EI, {"trait": {}})
put("e_trait", d)
d = core_base()
d["index"]["30"] = item(30, EI, {"use": {"name": EI, "source": "x", "id": None, "is_glob": False}})
put("e_use", d)
d = core_base()
d["index"]["30"] = item(30, EI, {"trait": {}}, vis="crate")
put("e_private", d)
for op in ("export_memory", "import_memory", "export_semaphore", "import_semaphore"):
    d = core_base()
    d["index"]["30"] = item(30, op, {"function": {}}, vis="default")
    put("o_" + op, d)
d = core_base()
d["index"]["30"] = item(30, "export_memory", {"use": {"name": "export_memory", "source": "x", "id": None, "is_glob": False}})
put("o_use", d)
d = core_base()
d["index"]["30"] = item(30, "export_memory_x", {"function": {}})
d["index"]["31"] = item(31, "ExportMemory", {"function": {}})
d["index"]["32"] = item(32, "export_memory", {"struct": {}})
put("o_partial", d)
d = core_base()
d["index"]["30"] = item(30, "export_memory", {"function": {}}, crate_id=1)
put("o_external", d)

# ---- (4): 名前に WebGPU の語を含む公開の操作・型（ダミーの語 zzgpu。語は ok.conf の [webgpu-words]）----
def wg(extra):
    d = base()
    for i, (name, inner, kw) in enumerate(extra):
        d["index"][str(2 + i)] = item(2 + i, name, inner, **kw)
    return d


put("w_fn", wg([("as_zzgpu", {"function": {}}, {})]))
put("w_method_default", wg([("open_zzgpu_port", {"function": {}}, {"vis": "default"})]))
put("w_struct", wg([("ZzGpuDevice", {"struct": {}}, {})]))
put("w_struct_acronym", wg([("ZZGPUHandle", {"struct": {}}, {})]))
put("w_enum", wg([("Zzgpu", {"enum": {}}, {})]))
put("w_trait", wg([("HasZzGpu", {"trait": {}}, {})]))
put("w_union", wg([("ZzgpuU", {"union": {}}, {})]))
put("w_type_alias", wg([("ZzgpuAlias", {"type_alias": {}}, {})]))
put("w_variant", wg([("ZzGpu", {"variant": {}}, {})]))
put("w_split", wg([("zz_gpu_thing", {"function": {}}, {})]))
put("w_use", wg([("ZzgpuReexp", {"use": {"name": "ZzgpuReexp", "source": "x", "id": None, "is_glob": False}}, {})]))
put("w_use_alias", wg([("shown", {"use": {"name": "AsZzgpu", "source": "x", "id": None, "is_glob": False}}, {})]))
# const・static・field・module は数えない（WebGPU の口の非対応を答える値は操作ではない）。
put("w_const", wg([
    ("ZZGPU_DEVICE_PORT", {"constant": {}}, {}),
    ("ZZGPU_STATIC", {"static": {}}, {}),
    ("zzgpu_field", {"struct_field": {}}, {}),
    ("zzgpu_mod", {"module": {"items": []}}, {}),
]))
# 語を部分に含むだけの別の語は数えない。
put("w_partial", wg([
    ("nonzzgpu", {"function": {}}, {}),
    ("zzgpus", {"function": {}}, {}),
    ("Zzgpus", {"struct": {}}, {}),
    ("zz_only", {"function": {}}, {}),
    ("gpu_zz", {"function": {}}, {}),
]))
put("w_nonpublic", wg([
    ("zzgpu_private", {"function": {}}, {"vis": "crate"}),
    ("zzgpu_ext", {"function": {}}, {"crate_id": 1}),
]))
put("w_trait_alias", wg([("ZzgpuAlias2", {"trait_alias": {}}, {})]))
# 許可リストに無い種類（関連型・関連定数・マクロ）は数えない。
put("w_other_kinds", wg([
    ("ZzgpuAssoc", {"assoc_type": {}}, {}),
    ("ZZGPU_ASSOC", {"assoc_const": {}}, {}),
    ("zzgpu_macro", {"macro": ""}, {}),
]))
put("w_dup2", wg([("as_zzgpu", {"function": {}}, {}), ("as_zzgpu", {"function": {}}, {})]))
put("w_two", wg([("as_zzgpu", {"function": {}}, {}), ("ZzgpuDevice", {"struct": {}}, {})]))

with open(os.path.join(out, "broken.json"), "w", encoding="utf-8") as f:
    f.write("{not json")
PY

# 設定の合成ファイル（テスト用のダミーの語）
printf '# dummy\n[blocking-words]\nzzblock\nzzwait\n[webgpu-words]\nzzgpu\n' >"${work}/ok.conf"
printf '[webgpu-words]\nzzgpu\n[webgpu-words]\nzzgpu2\n' >"${work}/dup_wg_section.conf"
printf '[blocking-words]\nzzblock\n[webgpu-words]\nZzGpu\n' >"${work}/upper_wg_word.conf"
printf '[blocking-words]\nzzblock\n[webgpu-words]\n# none\n' >"${work}/empty_wg_section.conf"
printf '[blocking-words]\nzzblock\n' >"${work}/missing_wg_section.conf"
printf '[blocking-words]\nzzblock\n[bogus]\nx\n' >"${work}/unknown_section.conf"
printf '[Blocking-Words]\nzzblock\n' >"${work}/upper_section.conf"
printf '[blocking-words]\nzzblock\n[blocking-words]\nzzwait\n' >"${work}/dup_section.conf"
printf 'zzblock\n[blocking-words]\nzzwait\n' >"${work}/before_section.conf"
printf '[blocking-words]\nZzBlock\n' >"${work}/upper_word.conf"
printf '[blocking-words]\nzz_block\n' >"${work}/underscore_word.conf"
printf '[blocking-words]\nzzblock\nzzblock\n' >"${work}/dup_word.conf"
printf '[blocking-words]\n# only a comment\n' >"${work}/empty_section.conf"
printf '# nothing\n' >"${work}/missing_section.conf"
printf '\377\376bad\n' >"${work}/bad_utf8.conf"

# check <名前> <期待する終了コード> <標準出力に含む語|-> <標準エラーに含む語|-> <引数...>
# 終了コード 2 のときは、標準出力に `ok:`・`record:` が無いことも合否に含める。
check() {
    local name="$1" want="$2" want_out="$3" want_err="$4"
    shift 4
    rc=0
    "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    local ok=1
    [[ "${rc}" -eq "${want}" ]] || ok=0
    if [[ "${want_out}" != "-" ]] && ! grep -qF -- "${want_out}" "${OUT_F}"; then ok=0; fi
    if [[ "${want_err}" != "-" ]] && ! grep -qF -- "${want_err}" "${ERR_F}"; then ok=0; fi
    if [[ "${want}" -eq 2 ]] && grep -qE '^(ok|record):' "${OUT_F}"; then ok=0; fi
    if [[ "${ok}" -eq 1 ]]; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        echo "FAIL: ${name} (rc=${rc}, want=${want}, out~${want_out}, err~${want_err})" >&2
        sed 's/^/  out: /' "${OUT_F}" >&2
        sed 's/^/  err: /' "${ERR_F}" >&2
    fi
}

C="${work}/ok.conf"
J() { echo "${work}/$1.json"; }

# ---- 前半: --json の経路（合成 JSON）----

# 終了コード 0
check "clean は 0 件で ok" 0 "ok: 1 public API document(s)" - --config "${C}" --json "$(J clean)"
check "clean の record" 0 "record: commit=- documents=1" - --config "${C}" --json "$(J clean)"
check "clean の件数" 0 "blocking_wait_names_total=0" - --config "${C}" --json "$(J clean)"
check "非公開・他 crate の関数は数えない" 0 "blocking_wait_names=0" - --config "${C}" --json "$(J nonpublic)"
check "関数でも再公開でもないアイテムは数えない" 0 "blocking_wait_names=0" - --config "${C}" --json "$(J nonfunction)"
check "部分一致には当てない" 0 "blocking_wait_names=0" - --config "${C}" --json "$(J partial)"
check "同じ crate の中を指す glob の再公開は 0 件" 0 "blocking_wait_names=0" - --config "${C}" --json "$(J glob_internal)"

# 終了コード 1（違反）
check "自由関数の語の一致" 1 "blocking_wait_names=1" "violation: crate=dummy features=- item=zzblock_fn kind=function word=zzblock" \
    --config "${C}" --json "$(J fn_hit)"
check "FAILED の行" 1 "blocking_wait_names_total=1" "FAILED: 1 item(s)" --config "${C}" --json "$(J fn_hit)"
check "違反のとき record は出す" 1 "record: commit=-" "FAILED:" --config "${C}" --json "$(J fn_hit)"
if grep -q '^ok:' "${OUT_F}"; then
    failed=$((failed + 1))
    echo "FAIL: 違反のとき ok を出した" >&2
else
    passed=$((passed + 1))
fi
check "trait の impl のメソッド（default）も当たる" 1 - "item=zzwait_m kind=function word=zzwait" \
    --config "${C}" --json "$(J method_default)"
check "大文字小文字を区別しない" 1 - "item=Zzwait_For kind=function word=zzwait" --config "${C}" --json "$(J case_hit)"
check "name と use.name が同じ再公開は 1 回" 1 "blocking_wait_names_total=1" "kind=use word=zzblock" \
    --config "${C}" --json "$(J use_same)"
check "name と use.name が別の語なら両方数える" 1 "blocking_wait_names_total=2" "FAILED: 2 item(s)" \
    --config "${C}" --json "$(J use_diff)"
check "1 つの名前に 2 語なら語ごとに数える" 1 "blocking_wait_names_total=2" "word=zzwait" \
    --config "${C}" --json "$(J two_words)"
check "複数 JSON の合計" 1 "documents=2 public_items_total=5 public_functions_total=1 blocking_wait_names_total=1" "FAILED: 1 item(s)" \
    --config "${C}" --json "$(J clean)" "$(J fn_hit)"
check "違反の出力の値の無害化" 1 - "item=zzblock_? kind=function" --config "${C}" --json "$(J ctrl_name)"

# ---- (4) の検査（--json の経路）----
check "(4) 関数 as_ 語" 1 "webgpu_names=1" "violation: crate=dummy features=- item=as_zzgpu kind=function check=webgpu-name word=zzgpu" \
    --config "${C}" --json "$(J w_fn)"
check "(4) record と FAILED" 1 "webgpu_names_total=1" "FAILED: 1 item(s)" --config "${C}" --json "$(J w_fn)"
check "(4) メソッド（default）" 1 - "item=open_zzgpu_port kind=function check=webgpu-name" --config "${C}" --json "$(J w_method_default)"
check "(4) struct（CamelCase）" 1 - "item=ZzGpuDevice kind=struct check=webgpu-name word=zzgpu" --config "${C}" --json "$(J w_struct)"
check "(4) struct（大文字の略語）" 1 - "item=ZZGPUHandle kind=struct check=webgpu-name" --config "${C}" --json "$(J w_struct_acronym)"
check "(4) enum" 1 - "item=Zzgpu kind=enum check=webgpu-name" --config "${C}" --json "$(J w_enum)"
check "(4) trait" 1 - "item=HasZzGpu kind=trait check=webgpu-name" --config "${C}" --json "$(J w_trait)"
check "(4) union" 1 - "item=ZzgpuU kind=union check=webgpu-name" --config "${C}" --json "$(J w_union)"
check "(4) type alias" 1 - "item=ZzgpuAlias kind=type_alias check=webgpu-name" --config "${C}" --json "$(J w_type_alias)"
check "(4) variant" 1 - "item=ZzGpu kind=variant check=webgpu-name" --config "${C}" --json "$(J w_variant)"
check "(4) _ で分かれた語の連続" 1 - "item=zz_gpu_thing kind=function check=webgpu-name" --config "${C}" --json "$(J w_split)"
check "(4) 再公開" 1 "webgpu_names_total=1" "item=ZzgpuReexp kind=use check=webgpu-name" --config "${C}" --json "$(J w_use)"
check "(4) 再公開の局所名だけが語を含む" 1 - "item=AsZzgpu kind=use check=webgpu-name" --config "${C}" --json "$(J w_use_alias)"
check "(4) 複数は合計" 1 "webgpu_names_total=2" "FAILED: 2 item(s)" --config "${C}" --json "$(J w_two)"
check "(4) const・static・field・module は数えない" 0 "webgpu_names_total=0" - --config "${C}" --json "$(J w_const)"
check "(4) 語を部分に含むだけの名前は数えない" 0 "webgpu_names_total=0" - --config "${C}" --json "$(J w_partial)"
check "(4) 非公開・他 crate は数えない" 0 "webgpu_names_total=0" - --config "${C}" --json "$(J w_nonpublic)"
check "(4) trait alias" 1 - "item=ZzgpuAlias2 kind=trait_alias check=webgpu-name" --config "${C}" --json "$(J w_trait_alias)"
check "(4) 関連型・関連定数・マクロは数えない" 0 "webgpu_names_total=0" - --config "${C}" --json "$(J w_other_kinds)"
check "(4) 組 B にだけある違反を数える" 1 "webgpu_names_total=1" "violation: crate=dummy features=- item=as_zzgpu kind=function check=webgpu-name" \
    --config "${C}" --json "$(J clean)" --interop-json "$(J w_fn)"
check "(4) 組 B の scanned にも件数" 1 "webgpu_names=1" - --config "${C}" --json "$(J clean)" --interop-json "$(J w_fn)"
check "(4) 両方にある同じアイテムは 1 件" 1 "webgpu_names_total=1" "FAILED: 1 item(s)" \
    --config "${C}" --json "$(J w_fn)" --interop-json "$(J w_fn)"
check "(4) 組 B にだけ増えた分を足す" 1 "webgpu_names_total=2" "FAILED: 2 item(s)" \
    --config "${C}" --json "$(J w_fn)" --interop-json "$(J w_two)"
check "(4) 同名が複数あれば個数で引く" 1 "webgpu_names_total=2" "FAILED: 2 item(s)" \
    --config "${C}" --json "$(J w_fn)" --interop-json "$(J w_dup2)"
check "(4) 組 B の外部 crate を指す glob は終了コード 2" 2 - "glob re-export target is external or unresolved" \
    --config "${C}" --json "$(J clean)" --interop-json "$(J glob_external)"
check "(4) 組 B の解決できない glob は終了コード 2" 2 - "glob re-export target is external or unresolved" \
    --config "${C}" --json "$(J clean)" --interop-json "$(J glob_unresolved)"
check "(4) 節の重複" 2 - "duplicate config section" --config "${work}/dup_wg_section.conf" --json "$(J clean)"
check "(4) 大文字の語" 2 - "malformed word in a config section" --config "${work}/upper_wg_word.conf" --json "$(J clean)"
check "(4) 節が空" 2 - "empty config section" --config "${work}/empty_wg_section.conf" --json "$(J clean)"
check "(4) 節の欠落" 2 - "missing config section" --config "${work}/missing_wg_section.conf" --json "$(J clean)"

# 終了コード 2（誤用・前提の不足・読み切れない入力）
check "引数なし" 2 - -
check "--config 無し" 2 - - --json "$(J clean)"
check "未知の引数" 2 - - --config "${C}" --bogus
check "設定が無い" 2 - "cannot read the config file" --config "${work}/none.conf" --json "$(J clean)"
check "UTF-8 でない設定" 2 - "cannot read the config file" --config "${work}/bad_utf8.conf" --json "$(J clean)"
check "未知の節" 2 - "unknown config section" --config "${work}/unknown_section.conf" --json "$(J clean)"
check "大文字の節名" 2 - "unknown config section" --config "${work}/upper_section.conf" --json "$(J clean)"
check "節の重複" 2 - "duplicate config section" --config "${work}/dup_section.conf" --json "$(J clean)"
check "最初の節の前の語" 2 - "word before the first config section" --config "${work}/before_section.conf" --json "$(J clean)"
check "大文字の語" 2 - "malformed word in a config section" --config "${work}/upper_word.conf" --json "$(J clean)"
check "_ を含む語" 2 - "malformed word in a config section" --config "${work}/underscore_word.conf" --json "$(J clean)"
check "語の重複" 2 - "duplicate word in a config section" --config "${work}/dup_word.conf" --json "$(J clean)"
check "節が空" 2 - "empty config section" --config "${work}/empty_section.conf" --json "$(J clean)"
check "節の欠落" 2 - "missing config section" --config "${work}/missing_section.conf" --json "$(J clean)"
check "JSON が無い" 2 - "cannot read JSON input" --config "${C}" --json "${work}/none.json"
check "JSON が壊れている" 2 - "cannot read JSON input" --config "${C}" --json "$(J broken)"
check "index が無い" 2 - "missing or malformed 'index'" --config "${C}" --json "$(J bad_no_index)"
check "paths が無い" 2 - "missing or malformed 'paths'" --config "${C}" --json "$(J bad_no_paths)"
check "external_crates が無い" 2 - "missing or malformed 'external_crates'" --config "${C}" --json "$(J bad_no_external_crates)"
check "root が無い" 2 - "missing 'root'" --config "${C}" --json "$(J bad_no_root)"
check "includes_private が true" 2 - "includes_private is not false" --config "${C}" --json "$(J bad_private)"
check "format_version が整数でない" 2 - "format_version is not an integer" --config "${C}" --json "$(J bad_fv)"
check "root の crate_id が 0 でない" 2 - "malformed root item" --config "${C}" --json "$(J bad_root_crate)"
check "crate_id が整数でない" 2 - "integer crate_id" --config "${C}" --json "$(J bad_crate_id)"
check "inner の無い公開アイテム" 2 - "public item lacks 'inner'" --config "${C}" --json "$(J bad_no_inner)"
check "公開アイテムが 0 件" 2 - "no public items found" --config "${C}" --json "$(J zero_public)"
check "JSON 1 本だけが 0 件でも 2" 2 - "no public items found" --config "${C}" --json "$(J clean)" "$(J zero_public)"
check "外部 crate を指す glob の再公開" 2 - "glob re-export target is external or unresolved" --config "${C}" --json "$(J glob_external)"
check "解決できない glob の再公開" 2 - "glob re-export target is external or unresolved" --config "${C}" --json "$(J glob_unresolved)"

# ---- (2) の検査（--json の経路）----
check "core の文書がなければ E1・E2 はかけない（record に明示）" 0 "pipeline_input_checked=0 pipeline_input_violations_total=0" - --config "${C}" --json "$(J clean)"
check "core の文書があれば検査済み" 0 "pipeline_input_checked=1 pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_ok)"
check "(2) 合格の ok の行" 0 "no blocking-wait names, pipeline input ok" - --config "${C}" --json "$(J core_ok)"
check "scanned に (2) の欄" 0 "pipeline_create_functions=0 pipeline_input_violations=0" - --config "${C}" --json "$(J core_ok)"
check "生成の関数が PipelineInput を取る" 0 "pipeline_create_functions_total=1 pipeline_input_checked=1 pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_fn_ok)"
check "参照で包んだ PipelineInput" 0 "pipeline_create_functions_total=1" - --config "${C}" --json "$(J core_fn_ref)"
check "完全なパスの PipelineInput" 0 "pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_fn_path)"
check "2 番目の引数の PipelineInput" 0 "pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_fn_second_arg)"
check "impl Into<PipelineInput>" 0 "pipeline_create_functions_total=1 pipeline_input_checked=1 pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_fn_impl_into)"
check "generics の bound の PipelineInput" 0 "pipeline_create_functions_total=1 pipeline_input_checked=1 pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_fn_generic)"
check "名前が当たらない関数は数えず sig も読まない" 0 "pipeline_create_functions_total=0 pipeline_input_checked=1 pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_fn_unrelated)"
check "非公開の生成の関数は数えない" 0 "pipeline_create_functions_total=0" - --config "${C}" --json "$(J core_fn_private)"
check "旧表記の non_exhaustive の印" 0 "pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_old_attr)"
check "再公開は定義に数えない" 0 "pipeline_input_violations_total=0" - --config "${C}" --json "$(J core_reexport)"

check "non_exhaustive の印が無い" 1 "pipeline_input_violations_total=1" "violation: crate=fandhe_silicon_core features=- check=pipeline-input reason=not-non-exhaustive" --config "${C}" --json "$(J core_no_attr)"
check "別の属性だけでは印にならない" 1 - "reason=not-non-exhaustive" --config "${C}" --json "$(J core_other_attr)"
check "Wgsl の variant が無い" 1 - "check=pipeline-input reason=no-wgsl" --config "${C}" --json "$(J core_no_wgsl)"
check "enum でない PipelineInput" 1 - "check=pipeline-input reason=not-enum" --config "${C}" --json "$(J core_not_enum)"
check "PipelineInput が 2 つ" 1 - "check=pipeline-input reason=duplicate" --config "${C}" --json "$(J core_dup)"
check "PipelineInput が無い" 1 "pipeline_input_checked=1" "check=pipeline-input reason=missing" --config "${C}" --json "$(J core_missing)"
check "core の外の定義" 1 - "violation: crate=dummy features=- check=pipeline-input reason=outside-core" --config "${C}" --json "$(J core_ok)" "$(J dummy_with_pi)"
check "core が無くても外の定義は違反" 1 "pipeline_input_checked=0" "reason=outside-core" --config "${C}" --json "$(J dummy_with_pi)"
check "PipelineInput を取らない生成の関数" 1 "pipeline_input_violations_total=1" "violation: crate=fandhe_silicon_core features=- item=create_pipeline kind=function check=pipeline-create reason=no-pipeline-input" --config "${C}" --json "$(J fn_bytes)"
check "FAILED は (2) の件数" 1 - "FAILED: 1 item(s)" --config "${C}" --json "$(J fn_bytes)"
check "大文字の生成の関数も当たる" 1 - "item=Create_Pipeline_Raw kind=function check=pipeline-create" --config "${C}" --json "$(J fn_upper)"
check "trait のメソッド（default）も当たる" 1 - "item=build_pipeline kind=function check=pipeline-create" --config "${C}" --json "$(J fn_method)"
check "引数が無い生成の関数も違反" 1 "pipeline_create_functions_total=2 pipeline_input_checked=1 pipeline_input_violations_total=2" "item=new_pipeline kind=function check=pipeline-create" --config "${C}" --json "$(J fn_two)"
check "(1) と (2) の違反の合計" 1 - "FAILED: 2 item(s)" --config "${C}" --json "$(J fn_blocking_and_pipeline)"
check "(1) の違反の行の形は変わらない" 1 - "violation: crate=fandhe_silicon_core features=- item=zzblock_fn kind=function word=zzblock" --config "${C}" --json "$(J fn_blocking_and_pipeline)"
for f in "${work}"/cw*.json; do
    check "生成の語が当たる: $(basename "${f}")" 1 - "check=pipeline-create reason=no-pipeline-input" --config "${C}" --json "${f}"
done

check "attrs が並びでない" 2 - "malformed attrs on PipelineInput" --config "${C}" --json "$(J bad_attrs)"
check "variants が並びでない" 2 - "malformed variants on PipelineInput" --config "${C}" --json "$(J bad_variants)"
check "解決できない variant の id" 2 - "unresolved variant id on PipelineInput" --config "${C}" --json "$(J bad_variant_id)"
check "variant でないアイテムを指す id" 2 - "unresolved variant id on PipelineInput" --config "${C}" --json "$(J bad_variant_kind)"
check "名前が当たった関数に sig が無い" 2 - "pipeline-creating function lacks a valid sig" --config "${C}" --json "$(J bad_nosig)"
check "sig の inputs が並びでない" 2 - "pipeline-creating function lacks a valid sig" --config "${C}" --json "$(J bad_inputs)"

# ---- (3) の検査（--json / --interop-json の経路）----
check "--interop-json が無ければ G1 をかけない（record に明示）" 0 "interop_documents=0 ext_interop_checked=0" - --config "${C}" --json "$(J core_ok)"
check "--interop-json があれば検査済み" 0 "interop_documents=1 ext_interop_checked=1 external_interop_items_total=0 semaphore_handle_defined=0 submit_wait_params_total=0 interop_violations_total=0" - \
    --config "${C}" --json "$(J core_ok)" --interop-json "$(J core_ok)"
check "(3) 合格の ok の行" 0 "no blocking-wait names, pipeline input ok, ext-interop ok" - --config "${C}" --json "$(J core_ok)" --interop-json "$(J core_ok)"
check "(3) interop の JSON が無ければ ok の行は skipped" 0 "pipeline input ok, ext-interop skipped" - --config "${C}" --json "$(J core_ok)"
check "組 A の scanned に (3) の欄" 0 "external_interop_items=0 semaphore_handle_defs=0 submit_functions=0 submit_wait_params=0 interop_violations=0" - \
    --config "${C}" --json "$(J core_ok)"
check "組 B の scanned は (1)(2) の欄を持たない" 0 "scanned: crate=fandhe_silicon_core features=- format_version=61 public_items=4 external_interop_items=0 interop_op_names=0 semaphore_handle_defs=0 submit_functions=0 submit_wait_params=0 interop_violations=0" - \
    --config "${C}" --json "$(J core_ok)" --interop-json "$(J core_ok)"
check "組 A の外した feature の裏でない拡張 trait は数えるだけ" 0 "external_interop_items_total=1 " - --config "${C}" --json "$(J e_trait)"
check "組 A の拡張 trait は違反にならない" 0 "external_interop_items=1 semaphore_handle_defs=0 submit_functions=0 submit_wait_params=0 interop_violations=0" - --config "${C}" --json "$(J e_trait)"
check "組 B の非公開の拡張 trait は当てない" 0 "external_interop_items=0" - --config "${C}" --json "$(J core_ok)" --interop-json "$(J e_private)"
check "組 B の他 crate の操作名は当てない" 0 "interop_op_names=0" - --config "${C}" --json "$(J core_ok)" --interop-json "$(J o_external)"
check "操作名は完全一致だけ（部分一致・大文字・関数でない名前は当てない）" 0 "interop_op_names=0" - --config "${C}" --json "$(J core_ok)" --interop-json "$(J o_partial)"
check "再公開・variant・関数は定義に数えない" 0 "semaphore_handle_defined=0" - --config "${C}" --json "$(J h_not_defs)"
check "組 B の core の定義が execution にある" 0 "semaphore_handle_defs=1 submit_functions=0" - --config "${C}" --json "$(J core_ok)" --interop-json "$(J h_ok)"
check "定義が core の execution にある" 0 "semaphore_handle_defined=1 " - --config "${C}" --json "$(J h_ok)"
check "wait が &[ImportedSemaphoreHandle]" 0 "submit_functions=1 submit_wait_params=1 interop_violations=0" - --config "${C}" --json "$(J i_ok)"
check "submit_render も対象" 0 "submit_wait_params_total=1 " - --config "${C}" --json "$(J i_ok_render)"
check "メソッド（default）・大文字の名前も対象" 0 "submit_wait_params_total=1 " - --config "${C}" --json "$(J i_ok_method)"
check "submit 系が 2 つ" 0 "submit_functions=2 submit_wait_params=2" - --config "${C}" --json "$(J i_ok_two)"
check "wait を持たない submit は数えるだけ" 0 "submit_functions=1 submit_wait_params=0 interop_violations=0" - --config "${C}" --json "$(J i_nowait)"
check "submit の語を含まない名前は数えない" 0 "submit_functions=0 submit_wait_params=0" - --config "${C}" --json "$(J i_not_submit)"
check "非公開の submit は数えない" 0 "submit_functions=0" - --config "${C}" --json "$(J i_private_submit)"
check "組 B でも wait の形をかける" 0 "submit_wait_params=1 interop_violations=0" - --config "${C}" --json "$(J core_ok)" --interop-json "$(J i_ok)"

check "組 B の拡張 trait は違反" 1 "interop_violations_total=1" \
    "violation: crate=fandhe_silicon_core features=- item=ExternalInterop kind=trait check=ext-interop reason=visible-without-feature" \
    --config "${C}" --json "$(J core_ok)" --interop-json "$(J e_trait)"
check "組 B の拡張 trait の再公開は違反" 1 - "item=ExternalInterop kind=use check=ext-interop reason=visible-without-feature" \
    --config "${C}" --json "$(J core_ok)" --interop-json "$(J e_use)"
for op in export_memory import_memory export_semaphore import_semaphore; do
    check "組 B の操作名は違反: ${op}" 1 - "item=${op} kind=function check=ext-interop reason=interop-op-without-feature" \
        --config "${C}" --json "$(J core_ok)" --interop-json "$(J "o_${op}")"
done
check "組 B の操作名の再公開は違反" 1 - "kind=use check=ext-interop reason=interop-op-without-feature" \
    --config "${C}" --json "$(J core_ok)" --interop-json "$(J o_use)"
check "違反のとき ok を出さない" 1 "record: " "FAILED: 1 item(s)" --config "${C}" --json "$(J core_ok)" --interop-json "$(J e_trait)"
check "core の外の定義" 1 - "violation: crate=dummy features=- check=semaphore-handle reason=outside-core" --config "${C}" --json "$(J h_dummy)"
check "定義が core の直下" 1 - "violation: crate=fandhe_silicon_core features=- check=semaphore-handle reason=outside-execution" --config "${C}" --json "$(J h_root)"
check "定義が execution 以外のモジュール" 1 - "check=semaphore-handle reason=outside-execution" --config "${C}" --json "$(J h_other_mod)"
check "定義が 2 件" 1 - "check=semaphore-handle reason=duplicate" --config "${C}" --json "$(J h_dup)"
check "組 B の定義の違反" 1 - "features=- check=semaphore-handle reason=outside-execution" --config "${C}" --json "$(J core_ok)" --interop-json "$(J h_root)"
check "wait が &mut [..]" 1 - "violation: crate=fandhe_silicon_core features=- item=submit kind=function check=submit-wait reason=not-shared-slice" --config "${C}" --json "$(J i_mut)"
check "wait が Vec" 1 - "item=submit kind=function check=submit-wait reason=not-shared-slice" --config "${C}" --json "$(J i_vec)"
check "wait が &H" 1 - "check=submit-wait reason=not-shared-slice" --config "${C}" --json "$(J i_ref_handle)"
check "wait が配列" 1 - "check=submit-wait reason=not-shared-slice" --config "${C}" --json "$(J i_array)"
check "wait が &[u32]" 1 - "check=submit-wait reason=not-semaphore-handle" --config "${C}" --json "$(J i_u32)"
check "wait の要素が別の名前の型" 1 - "check=submit-wait reason=not-semaphore-handle" --config "${C}" --json "$(J i_other_name)"
check "wait の要素が execution の外" 1 - "check=submit-wait reason=outside-execution" --config "${C}" --json "$(J i_outside)"
check "wait の要素が core の直下" 1 - "check=submit-wait reason=outside-execution" --config "${C}" --json "$(J i_outside_root)"
check "組 B の wait の違反" 1 - "features=- item=submit kind=function check=submit-wait reason=not-shared-slice" --config "${C}" --json "$(J core_ok)" --interop-json "$(J i_mut)"
check "(1)(2)(3) の違反が FAILED に合計される" 1 - "FAILED: 3 item(s)" --config "${C}" --json "$(J fn_blocking_and_pipeline)" --interop-json "$(J e_trait)"
check "(1) の違反の行の形は変わらない" 1 - "violation: crate=fandhe_silicon_core features=- item=zzblock_fn kind=function word=zzblock" \
    --config "${C}" --json "$(J fn_blocking_and_pipeline)" --interop-json "$(J e_trait)"

check "--interop-json の単独指定" 2 - "--interop-json requires --json" --config "${C}" --interop-json "$(J core_ok)"
check "--interop-json の JSON が無い" 2 - "cannot read JSON input" --config "${C}" --json "$(J core_ok)" --interop-json "${work}/none.json"
check "--interop-json の形の崩れ" 2 - "missing or malformed 'index'" --config "${C}" --json "$(J core_ok)" --interop-json "$(J bad_no_index)"
check "定義の paths が無い" 2 - "unresolved path for ImportedSemaphoreHandle" --config "${C}" --json "$(J h_nopaths)"
check "定義の path が並びでない" 2 - "unresolved path for ImportedSemaphoreHandle" --config "${C}" --json "$(J h_badpath)"
check "組 B の定義の paths が無い" 2 - "unresolved path for ImportedSemaphoreHandle" --config "${C}" --json "$(J core_ok)" --interop-json "$(J h_nopaths)"
check "名前が当たった submit に sig が無い" 2 - "submit function lacks a valid sig" --config "${C}" --json "$(J i_bad_nosig)"
check "submit の inputs が並びでない" 2 - "submit function lacks a valid sig" --config "${C}" --json "$(J i_bad_inputs)"
check "submit の引数の要素が組でない" 2 - "submit function lacks a valid sig" --config "${C}" --json "$(J i_bad_entry)"
check "wait の型が object でない" 2 - "submit function lacks a valid sig" --config "${C}" --json "$(J i_bad_waittype)"
check "wait の要素の id が引けない" 2 - "unresolved wait parameter type" --config "${C}" --json "$(J i_bad_waitid)"
check "wait の要素の id が paths に無い" 2 - "unresolved wait parameter type" --config "${C}" --json "$(J i_bad_waitid_nopaths)"
check "組 B の wait の要素の id が引けない" 2 - "unresolved wait parameter type" --config "${C}" --json "$(J core_ok)" --interop-json "$(J i_bad_waitid)"

# 実物の語の一覧が書式どおり読める。
check "実物の設定が書式どおり読める" 0 "ok: 1 public API document(s)" - --config "${REAL_CONF}" --json "$(J clean)"

# 実物の設定の各語が当たること。入力は設定から実行時に作る（語はここに書かない）。
mkdir -p "${work}/gen"
python3 -I - "${REAL_CONF}" "${work}/gen" <<'PY'
import json
import os
import sys

conf, out = sys.argv[1], sys.argv[2]
words = []
sec = None
with open(conf, encoding="utf-8") as f:
    for raw in f:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[") and line.endswith("]"):
            sec = line[1:-1]
        elif sec in ("blocking-words", "webgpu-words"):
            words.append(line)
if not words:
    sys.exit("no words in the real config")
for i, w in enumerate(words):
    mod = {"id": 0, "crate_id": 0, "name": "dummy", "visibility": "public"}
    mod["inner"] = {"module": {"is_crate": True, "items": [1]}}
    fn = {"id": 1, "crate_id": 0, "name": "x_" + w.upper() + "_y", "visibility": "public"}
    fn["inner"] = {"function": {}}
    doc = {"root": 0, "format_version": 61, "includes_private": False, "external_crates": {}}
    doc["paths"] = {}
    doc["index"] = {"0": mod, "1": fn}
    with open(os.path.join(out, "w%d.json" % i), "w", encoding="utf-8") as g:
        json.dump(doc, g)
PY
for f in "${work}"/gen/w*.json; do
    check "実物の語が当たる: $(basename "${f}")" 1 - "FAILED: 1 item(s)" --config "${REAL_CONF}" --json "${f}"
done

# ---- 後半: 実物の rustdoc の JSON（既定の経路）----
# 一時 workspace に検査・public-api-check.sh・lib.sh を写して流す。依存なし。target は一時 workspace の中に出る
# （.gitignore で dirty にしない）。語の一覧は実物の設定を渡す。
rw="${work}/real"
ws="${rw}/ws"
mkdir -p "${ws}/crates/core/src" "${ws}/crates/contract/src" "${ws}/scripts"

# 契約の crate が宣言すべき feature は、写した public-api-check.sh の production の列から実行時に取り出す
# （cargo は未知の feature 名を拒むため。列をここに書き写すと本体の変更でずれる）。
feature_names="$(python3 -I - "${DIR}/public-api-check.sh" <<'PY'
import ast
import re
import sys

src = open(sys.argv[1], encoding="utf-8").read()
m = re.search(r"^CONTRACT_PRODUCTION = (\(.*?\))$", src, re.S | re.M)
if not m:
    sys.exit("CONTRACT_PRODUCTION not found")
names = ast.literal_eval(m.group(1)).split(",")
if not names or any(re.fullmatch(r"[a-z0-9-]+", n) is None for n in names):
    sys.exit("malformed feature list")
print("\n".join(names))
PY
)" || feature_names=""

# write_ws <members の TOML 配列>
write_ws() {
    printf '[workspace]\nmembers = %s\nresolver = "3"\n' "$1" >"${ws}/Cargo.toml"
}
write_ws '["crates/core", "crates/contract"]'
cat >"${ws}/crates/core/Cargo.toml" <<'TOML'
[package]
name = "fandhe-silicon-core"
version = "0.0.0"
edition = "2024"
TOML
{
    printf '[package]\nname = "fandhe-silicon-contract"\nversion = "0.0.0"\nedition = "2024"\n\n'
    printf '[dependencies]\nfandhe-silicon-core = { path = "../core" }\n\n[features]\n'
    printf '%s\n' "${feature_names}" | while IFS= read -r feat; do
        if [[ -n "${feat}" ]]; then printf '%s = []\n' "${feat}"; fi
    done
} >"${ws}/crates/contract/Cargo.toml"
printf 'target/\n' >"${ws}/.gitignore"
cp "${CHECK}" "${DIR}/public-api-check.sh" "${DIR}/lib.sh" "${ws}/scripts/"
chmod +x "${ws}/scripts/public-api-webgpu-check.sh" "${ws}/scripts/public-api-check.sh"
if [[ -f "${DIR}/../rust-toolchain.toml" ]]; then
    cp "${DIR}/../rust-toolchain.toml" "${ws}/rust-toolchain.toml"
fi

# core の基準の中身。PipelineInput（non_exhaustive・Wgsl を持つ enum）を 1 つ置く。
CORE_OK='#[non_exhaustive]
pub enum PipelineInput<'"'"'a> {
    Wgsl { source: &'"'"'a str },
    Msl { source: &'"'"'a str },
}
pub struct Own;'
# (a) の陽性対照用に、先頭の属性の行を外した版。
CORE_NO_ATTR="${CORE_OK#*$'\n'}"

# libs <core の lib.rs> <contract の lib.rs>
libs() {
    printf '%s\n' "$1" >"${ws}/crates/core/src/lib.rs"
    printf '%s\n' "$2" >"${ws}/crates/contract/src/lib.rs"
}
# 開発者の全体設定とフックを効かせない git。
real_git() {
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${ws}" \
        -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# real <名前> <期待する終了コード> <標準出力に含む語|-> <標準エラーに含む語|->:
# 写した検査を実物の設定で流して判定する。共有の target ディレクトリへ書かせない。
# 標準出力にホストの絶対パスが出ていないことも見る（標準エラーには cargo 自身の出力が混ざる）。
# 終了コード 2 のときは `ok:`・`record:` が無いことも見る。
real() {
    local name="$1" want="$2" want_out="$3" want_err="$4" ok=1
    rc=0
    (cd "${ws}" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR \
        ./scripts/public-api-webgpu-check.sh --config "${REAL_CONF}") >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    [[ "${rc}" -eq "${want}" ]] || ok=0
    if [[ "${want_out}" != "-" ]] && ! grep -qF -- "${want_out}" "${OUT_F}"; then ok=0; fi
    if [[ "${want_err}" != "-" ]] && ! grep -qF -- "${want_err}" "${ERR_F}"; then ok=0; fi
    if grep -qF -- "${work}" "${OUT_F}"; then ok=0; fi
    if [[ "${want}" -eq 2 ]] && grep -qE '^(ok|record):' "${OUT_F}"; then ok=0; fi
    if [[ "${ok}" -eq 1 ]]; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        echo "FAIL: ${name} (rc=${rc}, want=${want}, out~${want_out}, err~${want_err})" >&2
        sed 's/^/  out: /' "${OUT_F}" >&2
        sed 's/^/  err: /' "${ERR_F}" >&2
    fi
}
# has_in <名前> <ファイル> <正規表現>: 直近の出力に正規表現が在ること。
has_in() {
    if grep -qE -- "$3" "$2"; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        echo "FAIL: $1 (missing: $3)" >&2
    fi
}
# not_in <名前> <ファイル> <正規表現>: 直近の出力に正規表現が無いこと。
not_in() {
    if grep -qE -- "$3" "$2"; then
        failed=$((failed + 1))
        echo "FAIL: $1 (unexpected: $3)" >&2
    else
        passed=$((passed + 1))
    fi
}

# 違反に使う関数名。語の値は書かない（実物の設定の語のうち最初のものを実行時に取る）。
first_word="$(python3 -I - "${REAL_CONF}" <<'PY'
import re
import sys

sec = None
for raw in open(sys.argv[1], encoding="utf-8"):
    line = raw.strip()
    if not line or line.startswith("#"):
        continue
    if line.startswith("["):
        sec = line
    elif sec == "[blocking-words]":
        if re.fullmatch(r"[a-z0-9]+", line):
            print(line)
        break
PY
)" || first_word=""
VIOL="${first_word}_idle"
# WebGPU の語（[webgpu-words] の最初の語）。
wg_word="$(python3 -I - "${REAL_CONF}" <<'PY'
import re
import sys

sec = None
for raw in open(sys.argv[1], encoding="utf-8"):
    line = raw.strip()
    if not line or line.startswith("#"):
        continue
    if line.startswith("["):
        sec = line
    elif sec == "[webgpu-words]":
        if re.fullmatch(r"[a-z0-9]+", line):
            print(line)
        break
PY
)" || wg_word=""
wg_upper="$(printf '%s' "${wg_word}" | tr '[:lower:]' '[:upper:]')"

libs "${CORE_OK}" 'pub struct Own;'
if [[ -n "${feature_names}" && -n "${first_word}" ]] \
    && (cd "${ws}" && cargo generate-lockfile --offline) >/dev/null 2>"${ERR_F}" \
    && real_git init -q >/dev/null 2>>"${ERR_F}" \
    && real_git add -A >/dev/null 2>>"${ERR_F}" \
    && real_git commit -q -m fixture >/dev/null 2>>"${ERR_F}"; then
    real real_clean 0 "documents=2" -
    has_in real_clean_commit "${OUT_F}" '^record: commit=[0-9a-f]{40}([0-9a-f]{24})? documents=2 '
    not_in real_clean_not_dirty "${OUT_F}" '^record: commit=[0-9a-f]+-dirty'
    has_in real_clean_ok "${OUT_F}" '^ok: 2 public API document\(s\)'
    has_in real_clean_core "${OUT_F}" '^scanned: crate=fandhe_silicon_core features=without-native-blocking-wait '
    has_in real_clean_contract "${OUT_F}" '^scanned: crate=fandhe_silicon_contract features=without-native-blocking-wait '
    not_in real_clean_no_violation "${ERR_F}" 'violation'
    has_in real_clean_pipeline_input "${OUT_F}" 'pipeline_input_checked=1 pipeline_input_violations_total=0'
    has_in real_clean_ok_pipeline "${OUT_F}" 'pipeline input ok, ext-interop ok$'
    has_in real_clean_interop "${OUT_F}" 'interop_documents=2 ext_interop_checked=1 external_interop_items_total=0 semaphore_handle_defined=0 '
    has_in real_clean_b_core "${OUT_F}" '^scanned: crate=fandhe_silicon_core features=without-ext-interop '
    has_in real_clean_b_contract "${OUT_F}" '^scanned: crate=fandhe_silicon_contract features=without-ext-interop '

    # 陽性対照: 外した feature の裏にある語の名前の関数は、その feature を外した構成では 0 件で通る。
    libs "${CORE_OK}" "pub struct Own;
#[cfg(feature = \"native-blocking-wait\")]
pub fn ${VIOL}() {}"
    real real_feature_gated 0 "blocking_wait_names_total=0" -
    not_in real_feature_gated_no_violation "${ERR_F}" 'violation'
    has_in real_feature_gated_dirty "${OUT_F}" '^record: commit=[0-9a-f]+-dirty documents=2 '

    # 同じ関数を cfg なしで置くと落ちる（違反を 1 件足すと 1 になる）。
    libs "${CORE_OK}" "pub struct Own;
pub fn ${VIOL}() {}"
    real real_contract_fn 1 "blocking_wait_names_total=1" "violation: crate=fandhe_silicon_contract features=without-native-blocking-wait item=${VIOL} kind=function"
    has_in real_contract_fn_failed "${ERR_F}" 'FAILED: 1 item\(s\)'
    not_in real_contract_fn_no_ok "${OUT_F}" '^ok:'

    # core の crate も対象。
    libs "${CORE_OK}
pub fn ${VIOL}() {}" 'pub struct Own;'
    real real_core_fn 1 "blocking_wait_names_total=1" "violation: crate=fandhe_silicon_core "
    not_in real_core_fn_no_ok "${OUT_F}" '^ok:'

    # inherent のメソッド。
    libs "${CORE_OK}" "pub struct Own;
impl Own {
    pub fn ${VIOL}(&self) {}
}"
    real real_method 1 "blocking_wait_names_total=1" "item=${VIOL} kind=function"

    # 再公開（局所名が語を含む）。
    libs "${CORE_OK}" "mod m {
    pub fn ${VIOL}() {}
}
pub use m::${VIOL};"
    real real_reexport 1 - "kind=use word=${first_word}"

    # ---- (4) の陽性対照（実物の rustdoc の JSON）----
    # 公開の自由関数 as_<語> を足すと落ちる。
    libs "${CORE_OK}" "pub struct Own;
pub fn as_${wg_word}() {}"
    real real_webgpu_fn 1 "webgpu_names_total=1" "item=as_${wg_word} kind=function check=webgpu-name word=${wg_word}"
    has_in real_webgpu_fn_failed "${ERR_F}" 'FAILED: 1 item\(s\)'
    not_in real_webgpu_fn_no_ok "${OUT_F}" '^ok:'
    # CamelCase の型も落ちる（語の綴りは設定の語を先頭だけ大文字にして作る）。
    wg_camel="$(printf '%s' "${wg_word}" | cut -c1 | tr '[:lower:]' '[:upper:]')$(printf '%s' "${wg_word}" | cut -c2-)"
    libs "${CORE_OK}" "pub struct ${wg_camel}Device;" 'pub struct Own;'
    real real_webgpu_struct 1 "webgpu_names_total=1" "kind=struct check=webgpu-name"
    # inherent のメソッド・公開 trait のメソッドも落ちる。
    libs "${CORE_OK}" "pub struct Own;
impl Own {
    pub fn as_${wg_word}(&self) {}
}"
    real real_webgpu_method 1 "webgpu_names_total=1" "item=as_${wg_word} kind=function check=webgpu-name"
    libs "${CORE_OK}" "pub struct Own;
pub trait Port {
    fn as_${wg_word}(&self);
}"
    real real_webgpu_trait_method 1 "webgpu_names_total=1" "item=as_${wg_word} kind=function check=webgpu-name"
    # native-blocking-wait の裏だけにある名前は組 B で拾う（組 A には出ない）。
    libs "${CORE_OK}" "pub struct Own;
#[cfg(feature = \"native-blocking-wait\")]
pub fn as_${wg_word}() {}"
    real real_webgpu_gated 1 "webgpu_names_total=1" "violation: crate=fandhe_silicon_contract features=without-ext-interop item=as_${wg_word}"
    # 定数は WebGPU の口の非対応を答える値であり数えない。
    libs "${CORE_OK}
pub const ${wg_upper}_PORT: u32 = 1;
pub static ${wg_upper}_STATIC: u32 = 1;" 'pub struct Own;'
    real real_webgpu_const 0 "webgpu_names_total=0" -
    not_in real_webgpu_const_no_violation "${ERR_F}" 'violation'

    # ---- (2) の陽性対照（実物の rustdoc の JSON）----
    # (a) non_exhaustive を外すと落ちる。
    libs "${CORE_NO_ATTR}" 'pub struct Own;'
    real real_pi_no_attr 1 "pipeline_input_violations_total=1" "check=pipeline-input reason=not-non-exhaustive"
    not_in real_pi_no_attr_no_ok "${OUT_F}" '^ok:'
    # (b) Wgsl の variant を外すと落ちる。
    libs '#[non_exhaustive]
pub enum PipelineInput<'"'"'a> { Msl { source: &'"'"'a str } }
pub struct Own;' 'pub struct Own;'
    real real_pi_no_wgsl 1 - "check=pipeline-input reason=no-wgsl"
    # (c) 2 つ目の公開の PipelineInput（別のモジュール）で落ちる。
    libs "${CORE_OK}
pub mod other {
    #[non_exhaustive]
    pub enum PipelineInput { Wgsl }
}" 'pub struct Own;'
    real real_pi_dup 1 - "check=pipeline-input reason=duplicate"
    # PipelineInput が無い core でも落ちる。
    libs 'pub struct Own;' 'pub struct Own;'
    real real_pi_missing 1 - "check=pipeline-input reason=missing"
    # (d) PipelineInput を取らない生成の関数を contract に足すと落ちる。
    libs "${CORE_OK}" "pub struct Own;
pub fn create_pipeline(bytes: &[u8]) -> u32 { bytes.len() as u32 }"
    real real_pipeline_bytes 1 - "item=create_pipeline kind=function check=pipeline-create reason=no-pipeline-input"
    has_in real_pipeline_bytes_failed "${ERR_F}" 'FAILED: 1 item\(s\)'
    # (e) PipelineInput を取る生成の関数は通る。
    libs "${CORE_OK}" "pub struct Own;
pub fn create_pipeline(i: &fandhe_silicon_core::PipelineInput<'_>) -> u32 { let _ = i; 0 }"
    real real_pipeline_input 0 "pipeline_create_functions_total=1 pipeline_input_checked=1 pipeline_input_violations_total=0" -
    # (f) 外した feature の裏に置いた (d) は、外した組では見えない。
    libs "${CORE_OK}" "pub struct Own;
#[cfg(feature = \"native-blocking-wait\")]
pub fn create_pipeline(bytes: &[u8]) -> u32 { bytes.len() as u32 }"
    real real_pipeline_gated 0 "pipeline_create_functions_total=0" -
    # (g) inherent のメソッドも対象。
    libs "${CORE_OK}" "pub struct Own;
impl Own {
    pub fn create_pipeline(&self, bytes: &[u8]) -> u32 { bytes.len() as u32 }
}"
    real real_pipeline_method 1 - "item=create_pipeline kind=function check=pipeline-create"

    # ---- (3) の陽性対照（実物の rustdoc の JSON。外す feature ごとに 2 回 JSON を作る）----
    EXEC_MOD='pub mod execution {
    pub struct ImportedSemaphoreHandle;
}'
    CORE_EXEC="${CORE_OK}
${EXEC_MOD}"
    EXT_TRAIT='pub trait ExternalInterop {
    fn export_memory(&self) {}
}'
    # (a) ext-interop の裏の拡張 trait は、その feature を外した組には出ないので 0 件で通り、組 A では数えられる。
    libs "${CORE_OK}" "pub struct Own;
#[cfg(feature = \"ext-interop\")]
${EXT_TRAIT}"
    real real_interop_gated 0 "ext_interop_checked=1" -
    has_in real_interop_gated_counted "${OUT_F}" 'external_interop_items_total=[1-9]'
    not_in real_interop_gated_no_violation "${ERR_F}" 'violation'
    # (b) 裏から出す（cfg を外す）と、ext-interop を外した組で見えて落ちる。
    libs "${CORE_OK}" "pub struct Own;
${EXT_TRAIT}"
    real real_interop_open 1 - "features=without-ext-interop item=ExternalInterop kind=trait check=ext-interop reason=visible-without-feature"
    has_in real_interop_open_op "${ERR_F}" 'item=export_memory kind=function check=ext-interop reason=interop-op-without-feature'
    not_in real_interop_open_no_ok "${OUT_F}" '^ok:'
    # (c) 操作名の自由関数を core に置くと落ちる。
    libs "${CORE_OK}
pub fn import_semaphore() {}" 'pub struct Own;'
    real real_interop_op_fn 1 - "violation: crate=fandhe_silicon_core features=without-ext-interop item=import_semaphore kind=function check=ext-interop"
    # (d) execution の型と、それを &[..] で取る submit 系は通る（定義 1 件・wait 2 件）。
    libs "${CORE_EXEC}" "pub struct Queue;
impl Queue {
    pub fn submit(&self, wait: &[fandhe_silicon_core::execution::ImportedSemaphoreHandle]) { let _ = wait; }
    pub fn submit_render(&self, wait: &[fandhe_silicon_core::execution::ImportedSemaphoreHandle]) { let _ = wait; }
}"
    real real_submit_ok 0 "semaphore_handle_defined=1 submit_wait_params_total=2 interop_violations_total=0" -
    not_in real_submit_ok_no_violation "${ERR_F}" 'violation'
    # (e) 型が core の直下なら落ちる。contract に置いても落ちる。
    libs "${CORE_OK}
pub struct ImportedSemaphoreHandle;" 'pub struct Own;'
    real real_handle_root 1 - "check=semaphore-handle reason=outside-execution"
    libs "${CORE_OK}" 'pub struct ImportedSemaphoreHandle;'
    real real_handle_contract 1 - "violation: crate=fandhe_silicon_contract features=without-native-blocking-wait check=semaphore-handle reason=outside-core"
    # (f) wait の形が違えば落ちる。
    libs "${CORE_EXEC}" "pub struct Queue;
impl Queue {
    pub fn submit(&self, wait: Vec<fandhe_silicon_core::execution::ImportedSemaphoreHandle>) { let _ = wait; }
}"
    real real_wait_vec 1 - "item=submit kind=function check=submit-wait reason=not-shared-slice"
    libs "${CORE_EXEC}" "pub struct Queue;
impl Queue {
    pub fn submit(&self, wait: &mut [fandhe_silicon_core::execution::ImportedSemaphoreHandle]) { let _ = wait; }
}"
    real real_wait_mut 1 - "item=submit kind=function check=submit-wait reason=not-shared-slice"
    libs "${CORE_EXEC}" "pub struct Queue;
impl Queue {
    pub fn submit(&self, wait: &[u32]) { let _ = wait; }
}"
    real real_wait_u32 1 - "item=submit kind=function check=submit-wait reason=not-semaphore-handle"
    # wait を持たない submit は数えるだけで通る。
    libs "${CORE_EXEC}" "pub struct Queue;
impl Queue {
    pub fn submit(&self) {}
}"
    real real_submit_nowait 0 "submit_wait_params_total=0" -

    # (g) 2 回目（ext-interop を外す）の JSON を作る段だけ失敗させる。1 回目は写した本物へ渡す。
    cp "${DIR}/public-api-check.sh" "${ws}/scripts/public-api-check.real.sh"
    cat >"${ws}/scripts/public-api-check.sh" <<'SH'
#!/usr/bin/env bash
for a in "$@"; do
    if [[ "$a" == "ext-interop" ]]; then exit 1; fi
done
exec bash "$(dirname "${BASH_SOURCE[0]}")/public-api-check.real.sh" "$@"
SH
    libs "${CORE_OK}" 'pub struct Own;'
    real real_second_emit_fail 2 - "public-api-check.sh --emit-json failed"
    # 2 回目だけ contract の文書が出ない構成も成功扱いにしない。
    cat >"${ws}/scripts/public-api-check.sh" <<'SH'
#!/usr/bin/env bash
real="$(dirname "${BASH_SOURCE[0]}")/public-api-check.real.sh"
for a in "$@"; do
    if [[ "$a" == "ext-interop" ]]; then
        bash "${real}" "$@" | grep -v fandhe-silicon-contract || true
        exit 0
    fi
done
exec bash "${real}" "$@"
SH
    real real_second_core_only 2 - "core and contract documents were not both emitted"
    rm -f "${ws}/scripts/public-api-check.real.sh"
    cp "${DIR}/public-api-check.sh" "${ws}/scripts/"

    libs 'this does not compile' 'pub struct Own;'
    real real_build_fail 2 - "public-api-check.sh --emit-json failed"

    # 契約の crate を外した workspace は、片方しか無い構成を成功扱いにしない。
    libs "${CORE_OK}" 'pub struct Own;'
    write_ws '["crates/core"]'
    (cd "${ws}" && cargo generate-lockfile --offline) >/dev/null 2>&1 || true
    real real_no_contract 2 - "public-api-check.sh --emit-json failed"
    write_ws '["crates/core", "crates/contract"]'
    (cd "${ws}" && cargo generate-lockfile --offline) >/dev/null 2>&1 || true

    # 以降は public-api-check.sh を差し替える。元のスクリプトは各ケースの後に写し直す。
    printf '#!/usr/bin/env bash\nexit 0\n' >"${ws}/scripts/public-api-check.sh"
    real real_no_emit 2 - "core and contract documents were not both emitted"

    printf '#!/usr/bin/env bash\necho "emitted: crate=fandhe-silicon-core features=x file=core.json"\nexit 0\n' >"${ws}/scripts/public-api-check.sh"
    real real_core_only 2 - "core and contract documents were not both emitted"

    printf '#!/usr/bin/env bash\necho "emitted: crate=fandhe-silicon-core features=x file=a.json"\necho "emitted: crate=fandhe-silicon-core features=x file=b.json"\nexit 0\n' >"${ws}/scripts/public-api-check.sh"
    real real_duplicate_emit 2 - "duplicate emitted document"

    printf '#!/usr/bin/env bash\nexit 1\n' >"${ws}/scripts/public-api-check.sh"
    real real_emit_exit1 2 - "public-api-check.sh --emit-json failed"
    cp "${DIR}/public-api-check.sh" "${ws}/scripts/"

    # コミットが取れなければビルドの前に止まる（lib.rs はコンパイルできない中身のまま）。
    mv "${ws}/.git" "${rw}/ws-git-away"
    libs 'this does not compile' 'pub struct Own;'
    real real_no_commit 2 - "cannot resolve the source commit"
    not_in real_no_commit_before_build "${ERR_F}" 'emit-json failed'
    mv "${rw}/ws-git-away" "${ws}/.git"
else
    failed=$((failed + 1))
    echo "FAIL: real_fixture_setup (cannot prepare the temporary workspace)" >&2
    sed 's/^/  err: /' "${ERR_F}" >&2
fi

echo "test-public-api-webgpu-check: passed=${passed} failed=${failed}"
[[ "${failed}" -eq 0 ]]
