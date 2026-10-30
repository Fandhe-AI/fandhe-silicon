#!/usr/bin/env bash
# dep-graph-wgpu-check.sh の回帰テスト（spec TASK-11 のポインタ。親 Issue #364 の 5/6）。
# 一時ディレクトリの合成 cargo metadata の JSON と合成 cargo tree のテキスト（ダミーの名前）を
# --metadata-json / --tree-text で渡し、終了コード・標準出力・標準エラーの文言を確かめる。合成の節は cargo を呼ばない。
# 「偽のコマンド」の節は、PATH の先頭に置いた偽の cargo / git で引数なしの経路（cargo を流す経路）を通す。
# 末尾の「実物」の節は、実行時に一時ディレクトリへ作る合成 path crate の一時 workspace（比較基準の baseline-wgpu/
# ではない。比較基準への陽性対照は別 TASK のポインタ）に検査を写し、実物の cargo metadata / cargo tree の出力で
# 通す。狭い定義の名前・広い定義の repository をそれぞれ 1 件置くと失敗になること（検査が働いている証拠）も
# ここで確かめる。依存は path だけでネットワークを使わず、リポジトリのファイルを変更しない。
# 検査の定数 METADATA_ONLY_CRATES（metadata にだけ載る crate の一覧。TASK-82 のポインタ）の値には依存しない:
# 既定は一覧を空にした写し（PIN_ROOT）で流し、一覧の確かめは定数の行だけを差し替えた写しで行う。
# 陽性対照の節の cargo は CARGO_NET_OFFLINE=true で流すので、製品の依存があらかじめ取得済みであることが前提
# （`make deny` や CI の deny ジョブの前段で取得される）。末尾の「陽性対照」だけが実物の一覧を製品の workspace の写しに当て、記載を 1 件足すと終了コード 2 になることを確かめる。
# 呼び出し元: `make test-dep-graph-wgpu-check`（直接実行も可。CI の `deny` ジョブも実行する）。
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

REAL_CHECK="${DIR}/dep-graph-wgpu-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-dep-graph-wgpu-check.XXXXXX")"
trap 'rm -rf "${work:?}"' EXIT

# 検査の定数 METADATA_ONLY_CRATES の読み書きの補助。検査（bash）の中の Python 部（<<'PY' から PY まで）を ast で読み、
# 代入文の行の範囲（lineno〜end_lineno）を使うので、複数行の tuple や行末のコメントがあっても働く。
#   get <検査>                  定数の値を repr で出す（ast.literal_eval）
#   extend <検査>               定数の値に成り立たない記載を 1 件足した tuple の repr を出す
#   set <検査> <出力先> <値>    定数の代入文を <値> のリテラルに置き換えて書き出す
AST_HELPER="${work}/const_helper.py"
cat >"${AST_HELPER}" <<'PY'
import ast
import sys

NAME = "METADATA_ONLY_CRATES"
mode, src = sys.argv[1], sys.argv[2]
lines = open(src, encoding="utf-8").read().split("\n")
begin = next(i for i, ln in enumerate(lines) if ln.rstrip().endswith("<<'PY'")) + 1
end = next(i for i in range(begin, len(lines)) if lines[i] == "PY")
tree = ast.parse("\n".join(lines[begin:end]))
hits = [n for n in tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == NAME for t in n.targets)]
if len(hits) != 1:
    sys.exit(NAME + " assignment not found exactly once")
node = hits[0]
value = ast.literal_eval(node.value)
if mode == "get":
    print(repr(value))
elif mode == "extend":
    print(repr(tuple(value) + (("positive-control-impossible", "0.0.0"),)))
elif mode == "set":
    lo, hi = begin + node.lineno - 1, begin + node.end_lineno
    lines[lo:hi] = [NAME + " = " + sys.argv[4]]
    open(sys.argv[3], "w", encoding="utf-8").write("\n".join(lines))
else:
    sys.exit("unknown mode")
PY


# 検査の写しを作る。定数 METADATA_ONLY_CRATES の行だけを渡した値に差し替え、実物の一覧の値に依存しない形にする。
# make_variant <出力先ディレクトリ> <Python のリテラル>: <出力先>/scripts/ に写しを置く（lib.sh も写す）。
make_variant() {
  mkdir -p "$1/scripts"
  cp "${DIR}/lib.sh" "$1/scripts/"
  python3 -I "${AST_HELPER}" set "${REAL_CHECK}" "$1/scripts/dep-graph-wgpu-check.sh" "$2"
  chmod +x "$1/scripts/dep-graph-wgpu-check.sh"
}

# 既定の検査は一覧を空にした写し（PIN_ROOT）。製品の workspace の実物の一覧の値に依存せず、
# 厳密一致の既存の確かめを固定する。実物の一覧の確かめは末尾の「陽性対照」の節で行う。
# PIN_ROOT は --manifest-path の確かめに使う最小の疑似リポジトリ（git の作業ツリー）でもある。
PIN_ROOT="${work}/pin"
make_variant "${PIN_ROOT}" '()'
mkdir -p "${PIN_ROOT}/baseline-wgpu"
printf '[workspace]\n' >"${PIN_ROOT}/Cargo.toml"
printf '[workspace]\n' >"${PIN_ROOT}/baseline-wgpu/Cargo.toml"
printf '# fixture\n' >"${PIN_ROOT}/baseline-wgpu/deny.toml"
GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${PIN_ROOT}" init -q
GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${PIN_ROOT}" add -A
GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${PIN_ROOT}" -c core.hooksPath=/dev/null \
  -c user.name=test -c user.email=test@example.invalid commit -q -m fixture
CHECK="${PIN_ROOT}/scripts/dep-graph-wgpu-check.sh"
OUT_F="${work}/stdout.txt"
ERR_F="${work}/stderr.txt"
passed=0
failed=0
rc=0

# 合成入力をまとめて作る。metadata は ${work}/<ケース名>.json、tree は ${work}/<ケース名>.tree。
python3 -I - "${work}" <<'PY'
import json
import os
import sys

out = sys.argv[1]
OTHER = "https://github.com/other/x"
GFX = "https://github.com/gfx-rs/foo"


def pkg(name, ver="1.0.0", repo=OTHER, pid=None):
    return {"id": pid or "%s@%s" % (name, ver), "name": name, "version": ver, "repository": repo}


def edge(src, dst, kinds=(None,)):
    return (src, dst, [{"kind": k, "target": None} for k in kinds])


def graph(pkgs, edges, members=None):
    nodes = []
    for p in pkgs:
        deps = [{"pkg": d, "dep_kinds": ks} for (s, d, ks) in edges if s == p["id"]]
        nodes.append({"id": p["id"], "deps": deps})
    return {
        "packages": pkgs,
        "workspace_members": members if members is not None else [pkgs[0]["id"]],
        "resolve": {"nodes": nodes},
    }


APP = pkg("app", repo="https://example.invalid/app")


def under_app(*specs):
    """app の直下に依存を足したグラフ。spec は package か (package, kinds)。"""
    pkgs, edges = [APP], []
    for s in specs:
        p, kinds = (s if isinstance(s, tuple) else (s, (None,)))
        pkgs.append(p)
        edges.append(edge(APP["id"], p["id"], kinds))
    return graph(pkgs, edges)


def tree_text(pairs, decorate=False):
    lines = []
    for n, v in pairs:
        lines.append("%s v%s" % (n, v))
        if decorate:
            lines.append("%s v%s (*)" % (n, v))
            lines.append("")
    return "\n".join(lines) + "\n"


cases = {}
trees = {}

# ---- 合格（2 定義とも 0 件）----
cases["ok_basic"] = under_app(pkg("lib-a"))
trees["ok_basic"] = tree_text([("app", "1.0.0"), ("lib-a", "1.0.0")])
cases["ok_tree_decorated"] = cases["ok_basic"]
trees["ok_tree_decorated"] = (
    "app v1.0.0 (/some/path/app)\n\n"
    "lib-a v1.0.0\n"
    "lib-a v1.0.0 (*)\n"
    "lib-a v1.0.0 (proc-macro)\n"
    "lib-a v1.0.0 (proc-macro) (*)\n"
)
# 名前の近接（完全一致であること）
cases["ok_near_name"] = under_app(pkg("wgpu-dummy"), pkg("nagano"), pkg("wgpu-core-ext"))
# repository の近接（接頭辞であること）
cases["ok_near_repo"] = under_app(
    pkg("near-a", repo="https://github.com/gfx-rs-fork/x"),
    pkg("near-b", repo="https://gitlab.com/gfx-rs/x"),
    pkg("near-c", repo="https://github.com/other/gfx-rs/x"),
)
# docs/dep-graph-wgpu-check.md「検査しないもの」の形。文書と振る舞いを揃えるため 0 件として固定する。
cases["ok_not_checked"] = under_app(
    pkg("nc-org", repo="https://github.com/gfx-rs"),
    pkg("nc-www", repo="https://www.github.com/gfx-rs/foo"),
    pkg("nc-ssh", repo="git@github.com:gfx-rs/foo.git"),
)
# repository の欠落（null・空・鍵なし）は違反ではなく、件数と crate を出す
d = under_app(pkg("miss-a", repo=None), pkg("miss-b", repo=""), pkg("miss-c"))
del d["packages"][3]["repository"]
cases["ok_missing_repo"] = d
# 同じ (名前, 版) が 2 つの id で現れ、片方にだけ repository がある → 欠落ではない
cases["ok_dup_mixed_repo"] = under_app(
    pkg("dup-a", repo=None, pid="dup-a-1"), pkg("dup-a", repo=OTHER, pid="dup-a-2"))
# member から辺が無い package は数えない
d = graph([APP, pkg("wgpu"), pkg("gfx-foo", repo=GFX)], [])
cases["ok_unreachable"] = d

# ---- 違反: 狭い定義 ----
for i, nm in enumerate(["wgpu", "wgpu-core", "wgpu-hal", "wgpu-types", "naga"]):
    cases["bad_narrow_%d" % i] = under_app(pkg(nm))
cases["bad_narrow_fold_a"] = under_app(pkg("WGPU_Core"))
cases["bad_narrow_fold_b"] = under_app(pkg("wgpu_hal"))
for k in (None, "normal", "build", "dev"):
    cases["bad_kind_%s" % (k or "null")] = under_app((pkg("naga"), (k,)))
cases["bad_narrow_transitive"] = graph(
    [APP, pkg("mid"), pkg("naga")],
    [edge("app@1.0.0", "mid@1.0.0"), edge("mid@1.0.0", "naga@1.0.0")])

# ---- 違反: 広い定義にだけ当たる ----
for i, repo in enumerate([
    "https://github.com/gfx-rs/foo",
    "git+https://github.com/gfx-rs/Foo.git/",
    "http://GitHub.com/gfx-rs/foo",
    "  https://github.com/gfx-rs/foo  ",
]):
    cases["bad_wide_%d" % i] = under_app(pkg("gfx-foo", repo=repo))
cases["bad_wide_dup"] = under_app(
    pkg("gfx-foo", repo=OTHER, pid="gfx-foo-1"), pkg("gfx-foo", repo=GFX, pid="gfx-foo-2"))
cases["bad_both"] = under_app(pkg("wgpu", repo="https://github.com/gfx-rs/wgpu"))
cases["bad_two"] = under_app(pkg("naga"), pkg("gfx-foo", repo=GFX))
# 出力の無害化（制御文字と ; は ? に置き換わる）
cases["bad_sanitize"] = under_app(pkg("evil\x1b[31m;x", repo="https://github.com/gfx-rs/a\x1bb"))

# ---- 誤用（読めない入力・形の崩れ）----
def mutate(fn):
    d = json.loads(json.dumps(cases["ok_basic"]))
    fn(d)
    return d


cases["err_top_array"] = []
cases["err_no_packages"] = mutate(lambda d: d.pop("packages"))
cases["err_members_empty"] = mutate(lambda d: d.update(workspace_members=[]))
cases["err_members_missing"] = mutate(lambda d: d.pop("workspace_members"))
cases["err_members_nonstr"] = mutate(lambda d: d.update(workspace_members=[1]))
cases["err_no_resolve"] = mutate(lambda d: d.pop("resolve"))
cases["err_nodes_not_list"] = mutate(lambda d: d["resolve"].update(nodes={}))
cases["err_pkg_name_int"] = mutate(lambda d: d["packages"][1].update(name=1))
cases["err_pkg_no_id"] = mutate(lambda d: d["packages"][1].pop("id"))
cases["err_repo_int"] = mutate(lambda d: d["packages"][1].update(repository=1))
cases["err_deps_not_list"] = mutate(lambda d: d["resolve"]["nodes"][0].update(deps={}))
cases["err_node_no_deps"] = mutate(lambda d: d["resolve"]["nodes"][0].pop("deps"))
cases["err_dep_pkg_nonstr"] = mutate(lambda d: d["resolve"]["nodes"][0]["deps"][0].update(pkg=1))
cases["err_kinds_missing"] = mutate(lambda d: d["resolve"]["nodes"][0]["deps"][0].pop("dep_kinds"))
cases["err_kinds_empty"] = mutate(lambda d: d["resolve"]["nodes"][0]["deps"][0].update(dep_kinds=[]))
cases["err_kind_unknown"] = mutate(
    lambda d: d["resolve"]["nodes"][0]["deps"][0].update(dep_kinds=[{"kind": "weird", "target": None}]))
cases["err_kinds_elem"] = mutate(lambda d: d["resolve"]["nodes"][0]["deps"][0].update(dep_kinds=[1]))
cases["err_pkg_unknown_id"] = mutate(lambda d: d["packages"].pop(1))
cases["err_node_missing"] = mutate(lambda d: d["resolve"]["nodes"].pop(1))
cases["err_member_unknown"] = mutate(lambda d: d.update(workspace_members=["ghost@1.0.0"]))

# ---- metadata にだけ載る crate（METADATA_ONLY_CRATES の確かめ用。名前はダミー）----
cases["mo_basic"] = under_app(pkg("lib-a"), pkg("mo-x"))
trees["mo_basic"] = tree_text([("app", "1.0.0"), ("lib-a", "1.0.0"), ("mo-x", "1.0.0")])
cases["mo_narrow"] = under_app(pkg("lib-a"), pkg("naga"))
trees["mo_narrow"] = tree_text([("app", "1.0.0"), ("lib-a", "1.0.0")])
cases["mo_wide"] = under_app(pkg("lib-a"), pkg("mo-gfx", repo=GFX))
trees["mo_wide"] = tree_text([("app", "1.0.0"), ("lib-a", "1.0.0")])
cases["mo_two"] = under_app(pkg("lib-a"), pkg("mo-x"), pkg("mo-y"))

for name, doc in cases.items():
    with open(os.path.join(out, name + ".json"), "w", encoding="utf-8") as f:
        json.dump(doc, f)
for name, text in trees.items():
    with open(os.path.join(out, name + ".tree"), "w", encoding="utf-8") as f:
        f.write(text)

# 読めない入力
with open(os.path.join(out, "notjson.json"), "w", encoding="utf-8") as f:
    f.write("{not json")
open(os.path.join(out, "empty.json"), "w").close()
with open(os.path.join(out, "nonutf8.json"), "wb") as f:
    f.write(b"\xff\xfe\x00")
os.mkdir(os.path.join(out, "adir.json"))
# tree 側の不一致・読めない行
with open(os.path.join(out, "tree_garbage.tree"), "w", encoding="utf-8") as f:
    f.write("app v1.0.0\nthis is not a tree line\n")
open(os.path.join(out, "tree_empty.tree"), "w").close()
with open(os.path.join(out, "tree_extra.tree"), "w", encoding="utf-8") as f:
    f.write(trees["ok_basic"] + "extra-crate v1.0.0\n")
with open(os.path.join(out, "tree_missing.tree"), "w", encoding="utf-8") as f:
    f.write("app v1.0.0\n")
with open(os.path.join(out, "tree_version.tree"), "w", encoding="utf-8") as f:
    f.write("app v1.0.0\nlib-a v2.0.0\n")
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

# count_is <名前> <ファイル> <固定文字列> <件数>: 行頭の語を含む行数が一致すること。
count_is() {
  local name="$1" file="$2" word="$3" want="$4" got
  got="$(grep -cF -e "${word}" "${file}" || true)"
  if [ "${got}" = "${want}" ]; then
    report_ok "${name}"
  else
    report_fail "${name}" "expected ${want} line(s) containing '${word}', got ${got}"
  fi
}

# good <ケース名> [語...]: 2 定義とも 0 件（終了コード 0・ok の行・record は 1 回で ok の直前）。
good() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/${c}.json"
  judge "${c}" 0 "o:ok: " "o:scanned: reading=metadata " "o:record: commit=- " \
    "o:narrow_total=0 wide_total=0" "!violation" "!FAILED" "!error:" "$@"
  count_is "${c}_one_record" "${OUT_F}" "record: " 1
  if [ "$(tail -n 1 "${OUT_F}" | cut -c1-4)" = "ok: " ]; then
    report_ok "${c}_ok_last"
  else
    report_fail "${c}_ok_last" "ok line must be the last stdout line"
  fi
}

# viol <ケース名> [語...]: 違反（終了コード 1・FAILED の行・record は出る・ok は出ない）。
viol() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/${c}.json"
  judge "${c}" 1 "e:violation: " "e:FAILED: " "o:record: commit=- " "!ok: " "!error:" "$@"
}

# misuse <ケース名> [語...]: 誤用（終了コード 2・error の行・成功の行なし）。
misuse() {
  local c="$1"
  shift
  run_check --metadata-json "${work}/${c}.json"
  judge "${c}" 2 "e:error: " "!ok: " "!FAILED" "!record: " "!violation" "$@"
}

# ---- 合格 ----

good ok_basic "o:scanned: reading=metadata crates=2 narrow=0 wide=0 no_repository=0" \
  "o:record: commit=- crates_total=2 workspace_members=1 narrow_total=0 wide_total=0 no_repository=0"
count_is "ok_basic_one_scanned" "${OUT_F}" "scanned: " 1
good ok_near_name
good ok_near_repo
good ok_not_checked
good ok_missing_repo "o:no_repository=3" "o:no-repository: crate=miss-a version=1.0.0" \
  "o:no-repository: crate=miss-b version=1.0.0" "o:no-repository: crate=miss-c version=1.0.0"
good ok_dup_mixed_repo "o:no_repository=0" "!no-repository:"
# 到達しない package は数えない（wgpu も gfx-rs の repository も crates_total に入らない）
good ok_unreachable "o:record: commit=- crates_total=1 workspace_members=1 narrow_total=0 wide_total=0"

# --tree-text が一致するとき（scanned は metadata・tree の順で 2 行）
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/ok_basic.tree"
judge "tree_match" 0 "o:scanned: reading=metadata crates=2 " "o:scanned: reading=tree crates=2" "o:ok: " "!error:"
if [ "$(grep -n '^scanned: reading=tree' "${OUT_F}" | cut -d: -f1)" = "2" ]; then
  report_ok "tree_match_second_line"
else
  report_fail "tree_match_second_line" "tree scanned line must be the second stdout line"
fi
run_check --metadata-json "${work}/ok_tree_decorated.json" --tree-text "${work}/ok_tree_decorated.tree"
judge "tree_match_decorated" 0 "o:scanned: reading=tree crates=2" "o:ok: " "!error:"

# ---- 違反: 狭い定義の名前が 1 件でも出れば失敗 ----

for i in 0 1 2 3 4; do
  case "${i}" in
    0) nm="wgpu" ;;
    1) nm="wgpu-core" ;;
    2) nm="wgpu-hal" ;;
    3) nm="wgpu-types" ;;
    *) nm="naga" ;;
  esac
  viol "bad_narrow_${i}" "e:violation: definition=narrow crate=${nm} version=1.0.0" \
    "e:FAILED: 1 crate(s)" "o:narrow=1 wide=0" "o:narrow_total=1 wide_total=0"
  count_is "bad_narrow_${i}_one_violation" "${ERR_F}" "violation: " 1
done
viol bad_narrow_fold_a "e:definition=narrow crate=WGPU_Core" "e:FAILED: 1 crate(s)" "o:narrow_total=1 wide_total=0"
viol bad_narrow_fold_b "e:definition=narrow crate=wgpu_hal" "e:FAILED: 1 crate(s)" "o:narrow_total=1 wide_total=0"
# 辺の種類ごと（DEP_KINDS を弱めると落ちる）
for k in null normal build dev; do
  viol "bad_kind_${k}" "e:definition=narrow crate=naga" "e:FAILED: 1 crate(s)" "o:narrow_total=1 wide_total=0"
done
viol bad_narrow_transitive "e:definition=narrow crate=naga" "o:crates_total=3 " "o:narrow_total=1 wide_total=0"

# ---- 違反: 広い定義にだけ当たる crate が 1 件でも出れば失敗 ----

for i in 0 1 2 3; do
  viol "bad_wide_${i}" "e:violation: definition=wide crate=gfx-foo version=1.0.0" \
    "e:FAILED: 1 crate(s)" "o:narrow=0 wide=1" "o:narrow_total=0 wide_total=1" "!definition=narrow"
done
viol bad_wide_dup "e:definition=wide crate=gfx-foo" "e:FAILED: 1 crate(s)" "o:narrow_total=0 wide_total=1"
# 狭い・広いの両方に当たる 1 crate は violation が 2 行で、件数は和集合の 1
viol bad_both "e:definition=narrow crate=wgpu" "e:definition=wide crate=wgpu" "e:FAILED: 1 crate(s)" \
  "o:narrow_total=1 wide_total=1"
count_is "bad_both_two_violations" "${ERR_F}" "violation: " 2
# 別の crate が 2 件
viol bad_two "e:definition=narrow crate=naga" "e:definition=wide crate=gfx-foo" "e:FAILED: 2 crate(s)" \
  "o:narrow_total=1 wide_total=1"
# 出力に制御文字と ; を生で出さない（? に置き換える）
viol bad_sanitize "e:definition=wide crate=evil??31m?x"
if grep -q "$(printf '\033')" "${ERR_F}" "${OUT_F}" || grep -qF ';x' "${ERR_F}" "${OUT_F}"; then
  report_fail "bad_sanitize_no_raw_chars" "control character or ';' leaked into the output"
else
  report_ok "bad_sanitize_no_raw_chars"
fi

# ---- 誤用（終了コード 2）: 読めない入力・形の崩れ ----

misuse notjson "e:not JSON"
misuse empty
misuse nonutf8 "e:cannot read input file"
misuse adir "e:cannot read input file"
misuse no-such-file "e:cannot read input file"
for c in err_top_array err_no_packages err_members_empty err_members_missing err_members_nonstr err_no_resolve \
  err_nodes_not_list err_pkg_name_int err_pkg_no_id err_repo_int err_deps_not_list err_node_no_deps \
  err_dep_pkg_nonstr err_kinds_missing err_kinds_empty err_kind_unknown err_kinds_elem err_pkg_unknown_id \
  err_node_missing err_member_unknown; do
  misuse "${c}"
done
misuse err_kind_unknown "e:unknown resolve dep kind"
misuse err_kinds_empty "e:dep kinds"
misuse err_node_missing "e:not found in resolve nodes"

# tree: 読めない行・空・集合の食い違いは成功にしない
for t in tree_garbage tree_empty tree_extra tree_missing tree_version; do
  run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/${t}.tree"
  judge "${t}" 2 "e:error: " "!ok: " "!FAILED" "!record: "
done
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/tree_missing.tree"
judge "tree_disagree_message" 2 "e:cargo metadata and cargo tree disagree"
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/no-such.tree"
judge "tree_missing_file" 2 "e:error: cannot read input file" "!ok: " "!record: "

# 引数の誤用
run_check --tree-text "${work}/ok_basic.tree"
judge "tree_without_metadata" 2 "e:--tree-text requires --metadata-json" "!ok: " "!record: "
run_check --bogus
judge "unknown_arg" 2 "!ok: " "!record: "
run_check --metadata-json
judge "metadata_without_value" 2 "!ok: " "!record: "

# --allow-metadata-superset: 一致の確認を「tree 側の crate がすべて metadata にある」へ緩める（製品向けの経路では受け付けない）
# metadata が tree より大きい入力は、オプションが無ければ不一致、あれば数え終える
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/tree_missing.tree"
judge "superset_strict_disagree" 2 "e:cargo metadata and cargo tree disagree" "!ok: " "!record: "
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/tree_missing.tree" --allow-metadata-superset
judge "superset_relaxed_ok" 0 "o:ok: " "o:record: commit=- crates_total=2 " "o:scanned: reading=tree crates=1" "!error:"
run_check --metadata-json "${work}/bad_narrow_4.json" --tree-text "${work}/tree_missing.tree" --allow-metadata-superset
judge "superset_relaxed_violation" 1 "e:definition=narrow crate=naga" "e:FAILED: " "o:record: commit=- " "!ok: "
# tree 側だけの crate がある入力は、緩めても不一致
for t in tree_extra tree_version tree_garbage tree_empty; do
  run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/${t}.tree" --allow-metadata-superset
  judge "superset_relaxed_${t}" 2 "e:error: " "!ok: " "!FAILED" "!record: "
done
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/tree_extra.tree" --allow-metadata-superset
judge "superset_relaxed_tree_only_crate" 2 "e:cargo metadata and cargo tree disagree"
# 緩めを受け付けない経路（引数なし・--tree-text なし）は誤用
run_check --allow-metadata-superset
judge "superset_without_inputs" 2 "e:--allow-metadata-superset is not accepted for the product workspace" "!ok: " "!record: "
run_check --metadata-json "${work}/ok_basic.json" --allow-metadata-superset
judge "superset_without_tree" 2 "e:--allow-metadata-superset requires --tree-text" "!ok: " "!record: "

# --manifest-path: 検査のあるリポジトリ配下の Cargo.toml だけ。誤用は終了コード 2（cargo は呼ばない）
run_check --manifest-path "${PIN_ROOT}/baseline-wgpu/Cargo.toml" --metadata-json "${work}/ok_basic.json"
judge "manifest_with_metadata" 2 "e:--manifest-path cannot be combined with --metadata-json" "!ok: " "!record: "
run_check --manifest-path "${work}/ok_basic.json" --allow-metadata-superset
judge "manifest_outside_repo" 2 "e:--manifest-path must be a Cargo.toml inside the repository" "!ok: " "!record: " "!${work}"
mkdir -p "${work}/outside"
printf '[workspace]\n' >"${work}/outside/Cargo.toml"
run_check --manifest-path "${work}/outside/Cargo.toml" --allow-metadata-superset
judge "manifest_outside_cargo_toml" 2 "e:--manifest-path must be a Cargo.toml inside the repository" "!ok: " "!record: " "!${work}"
run_check --manifest-path "${PIN_ROOT}/baseline-wgpu/no-such/Cargo.toml" --allow-metadata-superset
judge "manifest_missing_file" 2 "e:--manifest-path must be a Cargo.toml inside the repository" "!ok: " "!record: "
run_check --manifest-path "${PIN_ROOT}/baseline-wgpu/deny.toml" --allow-metadata-superset
judge "manifest_not_cargo_toml" 2 "e:--manifest-path must be a Cargo.toml inside the repository" "!ok: " "!record: "
run_check --manifest-path "${PIN_ROOT}/Cargo.toml" --allow-metadata-superset
judge "manifest_product_no_relax" 2 "e:--allow-metadata-superset is not accepted for the product workspace" "!ok: " "!record: "
run_check --manifest-path
judge "manifest_without_value" 2 "!ok: " "!record: "

# ---- ツール未導入（成功にせず終了コード 2）----
# PATH を絞った環境で、検査を絶対パスの bash から起動する。絞った PATH に入れるのは必要なコマンドだけ。
bash_bin="$(command -v bash)"
dirname_bin="$(command -v dirname)"
git_bin="$(command -v git)"
py_real="$(python3 -I -c 'import sys; print(sys.executable)')"

nopy="${work}/nopy"
mkdir -p "${nopy}"
ln -s "${dirname_bin}" "${nopy}/dirname"
rc=0
PATH="${nopy}" "${bash_bin}" "${CHECK}" --metadata-json "${work}/ok_basic.json" \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_python3" 2 "e:required command not found on PATH: python3" "!ok: " "!record: " "!scanned:"

nocargo="${work}/nocargo"
mkdir -p "${nocargo}"
ln -s "${dirname_bin}" "${nocargo}/dirname"
ln -s "${py_real}" "${nocargo}/python3"
ln -s "${git_bin}" "${nocargo}/git"
rc=0
PATH="${nocargo}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_cargo" 2 "e:error: cannot run cargo" "!ok: " "!record: " "!scanned:"

nogit="${work}/nogit"
mkdir -p "${nogit}"
ln -s "${dirname_bin}" "${nogit}/dirname"
ln -s "${py_real}" "${nogit}/python3"
rc=0
PATH="${nogit}" "${bash_bin}" "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "no_git" 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"

# ---- 偽の cargo / git（引数なしの経路。PATH の先頭に置く）----
# 偽の cargo は FAKE_CARGO_MODE で、偽の git は FAKE_GIT_MODE で振る舞いを変える。
fakebin="${work}/fakebin"
mkdir -p "${fakebin}"
cat >"${fakebin}/cargo" <<'FAKE'
#!/usr/bin/env bash
if [ "${FAKE_CARGO_MODE:-ok}" = "fail" ]; then
  exit 1
fi
if [ -n "${FAKE_ARGS_LOG:-}" ]; then
  echo "$*" >>"${FAKE_ARGS_LOG}"
fi
case "$1" in
  --version) echo "cargo 0.0.0 (fake)" ;;
  metadata)
    if [ "${FAKE_CARGO_MODE:-ok}" = "badjson" ]; then
      echo "{not json"
    else
      cat "${FAKE_MD}"
    fi
    ;;
  tree) cat "${FAKE_TREE}" ;;
  *) exit 1 ;;
esac
FAKE
cat >"${fakebin}/git" <<'FAKE'
#!/usr/bin/env bash
case "${FAKE_GIT_MODE:-ok}" in
  fail) exit 1 ;;
  garbage)
    echo "/secret/host/path/not-a-sha"
    exit 0
    ;;
esac
case "$1" in
  rev-parse) echo "0123456789abcdef0123456789abcdef01234567" ;;
  status)
    if [ "${FAKE_GIT_MODE:-ok}" = "dirty" ]; then
      echo " M tracked.rs"
    fi
    ;;
  *) exit 1 ;;
esac
FAKE
chmod +x "${fakebin}/cargo" "${fakebin}/git"

# run_fake <cargo モード> <git モード> <metadata> <tree>
run_fake() {
  rc=0
  PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE="$1" FAKE_GIT_MODE="$2" FAKE_MD="$3" FAKE_TREE="$4" \
    "${CHECK}" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

# --manifest-path は metadata と tree の両方の cargo の引数に渡る。--allow-metadata-superset で metadata ⊋ tree でも数え終える
args_log="${work}/fake-args.log"
rc=0
PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE=ok FAKE_GIT_MODE=ok FAKE_ARGS_LOG="${args_log}" \
  FAKE_MD="${work}/ok_basic.json" FAKE_TREE="${work}/tree_missing.tree" \
  "${CHECK}" --manifest-path "${PIN_ROOT}/baseline-wgpu/Cargo.toml" --allow-metadata-superset \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "fake_manifest_superset" 0 "o:ok: " "o:record: commit=0123456789abcdef0123456789abcdef01234567 crates_total=2 " "!error:"
count_is "fake_manifest_metadata_arg" "${args_log}" "metadata --locked --format-version 1 --all-features --manifest-path " 1
count_is "fake_manifest_tree_arg" "${args_log}" "tree --locked --workspace --all-features --target all -e normal,build,dev --prefix none --no-dedupe --manifest-path " 1
rc=0
PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE=ok FAKE_GIT_MODE=ok \
  FAKE_MD="${work}/ok_basic.json" FAKE_TREE="${work}/tree_missing.tree" \
  "${CHECK}" --manifest-path "${PIN_ROOT}/baseline-wgpu/Cargo.toml" \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "fake_manifest_strict_disagree" 2 "e:cargo metadata and cargo tree disagree" "!ok: " "!record: "

run_fake ok ok "${work}/ok_basic.json" "${work}/ok_basic.tree"
judge "fake_ok" 0 "o:record: commit=0123456789abcdef0123456789abcdef01234567 crates_total=2 " \
  "o:scanned: reading=tree crates=2" "o:ok: " "!commit=0123456789abcdef0123456789abcdef01234567-dirty"
run_fake ok dirty "${work}/ok_basic.json" "${work}/ok_basic.tree"
judge "fake_dirty" 0 "o:commit=0123456789abcdef0123456789abcdef01234567-dirty " "o:ok: "
run_fake ok ok "${work}/bad_narrow_4.json" "${work}/ok_basic.tree"
judge "fake_tree_disagree" 2 "e:cargo metadata and cargo tree disagree" "!ok: " "!record: "
run_fake fail ok "${work}/ok_basic.json" "${work}/ok_basic.tree"
judge "fake_cargo_fails" 2 "e:error: cargo --version failed" "!ok: " "!record: "
run_fake badjson ok "${work}/ok_basic.json" "${work}/ok_basic.tree"
judge "fake_metadata_not_json" 2 "e:error: cargo metadata output is not JSON" "!ok: " "!record: "
run_fake ok fail "${work}/ok_basic.json" "${work}/ok_basic.tree"
judge "fake_git_fails" 2 "e:error: cannot resolve the source commit" "!ok: " "!record: " "!scanned:"
run_fake ok garbage "${work}/ok_basic.json" "${work}/ok_basic.tree"
judge "fake_git_garbage" 2 "e:error: cannot resolve the source commit" "!/secret/host/path" "!ok: " "!record: "

# ---- metadata にだけ載る crate の一覧（METADATA_ONLY_CRATES。TASK-82 のポインタ）----
# 一覧の値は検査の写しに差し替えて確かめ、実物の一覧の値には依存しない。
use_variant() {
  VARIANT_ROOT="${work}/variant-$1"
  make_variant "${VARIANT_ROOT}" "$2"
  CHECK="${VARIANT_ROOT}/scripts/dep-graph-wgpu-check.sh"
}
use_pin() {
  CHECK="${PIN_ROOT}/scripts/dep-graph-wgpu-check.sh"
}
MO_X='(("mo-x", "1.0.0"),)'

# 一覧が空: 一覧に無い metadata だけの crate は不一致
run_check --metadata-json "${work}/mo_basic.json" --tree-text "${work}/ok_basic.tree"
judge "mo_unlisted" 2 "e:error: cargo metadata and cargo tree disagree on the crate set" "!ok: " "!record: " "!metadata-only:"

use_variant listed "${MO_X}"
# 一覧にあり、metadata だけにある: 合格。総数に入り、1 件ごとに出る
run_check --metadata-json "${work}/mo_basic.json" --tree-text "${work}/ok_basic.tree"
judge "mo_listed_ok" 0 "o:ok: " "o:record: commit=- crates_total=3 workspace_members=1 narrow_total=0 wide_total=0" \
  "o:metadata-only: crate=mo-x version=1.0.0" "o:scanned: reading=tree crates=2" "!error:"
count_is "mo_listed_ok_one_line" "${OUT_F}" "metadata-only: " 1
if [ "$(grep -n '^metadata-only:' "${OUT_F}" | cut -d: -f1)" -gt "$(grep -n '^record:' "${OUT_F}" | cut -d: -f1)" ]; then
  report_ok "mo_listed_ok_after_record"
else
  report_fail "mo_listed_ok_after_record" "metadata-only line must come after the record line"
fi
# 一覧にあるが metadata にも無い（古い記載）
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/ok_basic.tree"
judge "mo_listed_absent" 2 "e:error: listed metadata-only crate is not metadata-only: mo-x 1.0.0" "!ok: " "!record: "
# 一覧にあるが tree にもある
run_check --metadata-json "${work}/mo_basic.json" --tree-text "${work}/mo_basic.tree"
judge "mo_listed_in_tree" 2 "e:error: listed metadata-only crate is not metadata-only: mo-x 1.0.0" "!ok: " "!record: "
# 一覧にない metadata だけの crate がもう 1 件ある
run_check --metadata-json "${work}/mo_two.json" --tree-text "${work}/ok_basic.tree"
judge "mo_listed_plus_unlisted" 2 "e:cargo metadata and cargo tree disagree on the crate set" "!ok: " "!record: "
# 版が違えば別の crate
use_variant listed_ver '(("mo-x", "2.0.0"),)'
run_check --metadata-json "${work}/mo_basic.json" --tree-text "${work}/ok_basic.tree"
judge "mo_listed_other_version" 2 "e:cargo metadata and cargo tree disagree on the crate set" "!ok: "
# tree 側だけの crate は一覧があっても不一致
use_variant listed "${MO_X}"
run_check --metadata-json "${work}/mo_basic.json" --tree-text "${work}/tree_extra.tree"
judge "mo_listed_tree_only" 2 "e:cargo metadata and cargo tree disagree on the crate set" "!ok: " "!record: "
# 一覧の crate も狭い定義の対象のまま（総数にも入る）
use_variant narrow '(("naga", "1.0.0"),)'
run_check --metadata-json "${work}/mo_narrow.json" --tree-text "${work}/mo_narrow.tree"
judge "mo_listed_narrow" 1 "e:violation: definition=narrow crate=naga version=1.0.0" "e:FAILED: 1 crate(s)" \
  "o:record: commit=- crates_total=3 " "o:narrow_total=1 wide_total=0" "o:metadata-only: crate=naga version=1.0.0" "!ok: "
# 一覧の crate は広い定義の対象のままでもある（repository が gfx-rs 配下）
use_variant wide '(("mo-gfx", "1.0.0"),)'
run_check --metadata-json "${work}/mo_wide.json" --tree-text "${work}/mo_wide.tree"
judge "mo_listed_wide" 1 "e:violation: definition=wide crate=mo-gfx version=1.0.0" "e:FAILED: 1 crate(s)" \
  "o:record: commit=- crates_total=3 " "o:narrow_total=0 wide_total=1" "o:metadata-only: crate=mo-gfx version=1.0.0" \
  "!definition=narrow" "!ok: "
# 緩めの経路は一覧を使わない（一覧にある crate が metadata だけでなくても、無くても影響しない）
use_variant listed "${MO_X}"
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/tree_missing.tree" --allow-metadata-superset
judge "mo_superset_ignores_list" 0 "o:ok: " "o:record: commit=- crates_total=2 " "!metadata-only:" "!error:"
# 引数なしの経路（偽の cargo）でも一覧が働く。別 workspace の厳密一致では一覧を使わない
printf '[workspace]\n' >"${VARIANT_ROOT}/Cargo.toml"
mkdir -p "${VARIANT_ROOT}/baseline-wgpu"
printf '[workspace]\n' >"${VARIANT_ROOT}/baseline-wgpu/Cargo.toml"
run_fake ok ok "${work}/mo_basic.json" "${work}/ok_basic.tree"
judge "mo_fake_product_listed" 0 "o:ok: " "o:metadata-only: crate=mo-x version=1.0.0" "o:record: commit=0123456789abcdef0123456789abcdef01234567 crates_total=3 "
rc=0
PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE=ok FAKE_GIT_MODE=ok \
  FAKE_MD="${work}/mo_basic.json" FAKE_TREE="${work}/ok_basic.tree" \
  "${CHECK}" --manifest-path "${VARIANT_ROOT}/baseline-wgpu/Cargo.toml" \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "mo_fake_other_workspace_strict" 2 "e:cargo metadata and cargo tree disagree on the crate set" "!ok: " "!metadata-only:"
rc=0
PATH="${fakebin}:${PATH}" FAKE_CARGO_MODE=ok FAKE_GIT_MODE=ok \
  FAKE_MD="${work}/mo_basic.json" FAKE_TREE="${work}/ok_basic.tree" \
  "${CHECK}" --manifest-path "${VARIANT_ROOT}/Cargo.toml" \
  >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
judge "mo_fake_product_manifest_listed" 0 "o:ok: " "o:metadata-only: crate=mo-x version=1.0.0"

# 一覧の形が崩れていれば終了コード 2（入力を読む前に止まるので、どの経路でも同じ）
for spec in 'list:[("mo-x", "1.0.0")]' 'short:(("mo-x",),)' 'empty_name:(("", "1.0.0"),)' \
  'int_ver:(("mo-x", 1),)' 'str_entry:("mo-x",)' 'triple:(("a", "b", "c"),)'; do
  use_variant "bad_${spec%%:*}" "${spec#*:}"
  run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/ok_basic.tree"
  judge "mo_malformed_${spec%%:*}" 2 "e:error: malformed metadata-only crate list" "!ok: " "!record: "
done
use_variant dup '(("mo-x", "1.0.0"), ("mo-x", "1.0.0"))'
run_check --metadata-json "${work}/mo_basic.json" --tree-text "${work}/ok_basic.tree"
judge "mo_malformed_duplicate" 2 "e:error: duplicate entry in metadata-only crate list" "!ok: " "!record: "
use_variant ctl '(("evil\x1b[31m;x", "1.0.0"),)'
run_check --metadata-json "${work}/ok_basic.json" --tree-text "${work}/ok_basic.tree"
judge "mo_listed_sanitized" 2 "e:listed metadata-only crate is not metadata-only: evil??31m?x 1.0.0" "!ok: "
if grep -q "$(printf '\033')" "${ERR_F}" "${OUT_F}"; then
  report_fail "mo_listed_sanitized_no_raw_chars" "control character leaked into the output"
else
  report_ok "mo_listed_sanitized_no_raw_chars"
fi
use_pin

# ---- 実物（一時 workspace の実物の cargo metadata / cargo tree）----
# 合成 path crate だけの一時 workspace を作り、検査を写して引数なしで流す。ネットワークは使わない（--offline）。
rw="${work}/real"
mkdir -p "${rw}/ws/app/src" "${rw}/ws/scripts" "${rw}/ext"
cat >"${rw}/ws/Cargo.toml" <<'TOML'
[workspace]
members = ["app"]
resolver = "2"
TOML
printf '%s\n' '// fixture' >"${rw}/ws/app/src/lib.rs"
cp "${PIN_ROOT}/scripts/dep-graph-wgpu-check.sh" "${PIN_ROOT}/scripts/lib.sh" "${rw}/ws/scripts/"
chmod +x "${rw}/ws/scripts/dep-graph-wgpu-check.sh"
if [ -f "${DIR}/../rust-toolchain.toml" ]; then
  cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi

# mk_ext <名前> [repository]: ${rw}/ext/<名前> に空の合成 crate を作る。
mk_ext() {
  mkdir -p "${rw}/ext/$1/src"
  {
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2021"\n' "$1"
    if [ "$#" -ge 2 ]; then
      printf 'repository = "%s"\n' "$2"
    fi
  } >"${rw}/ext/$1/Cargo.toml"
  printf '%s\n' '// fixture' >"${rw}/ext/$1/src/lib.rs"
}
mk_ext plain-dummy "https://example.invalid/plain"
mk_ext wgpu "https://example.invalid/wgpu-dummy"
mk_ext naga "https://example.invalid/naga-dummy"
# 名前は無害で、repository だけが広い定義に当たる crate
mk_ext gfx-lookalike "https://github.com/gfx-rs/lookalike"

# real_git: 開発者の全体設定とフックを効かせない git。
real_git() {
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
    -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# real_app <app の依存の節>: app/Cargo.toml を書き換え、ロックを作り直してコミットする。
real_app() {
  {
    printf '[package]\nname = "app"\nversion = "0.1.0"\nedition = "2021"\nrepository = "https://example.invalid/app"\n\n'
    printf '%s\n' "$1"
  } >"${rw}/ws/app/Cargo.toml"
  (cd "${rw}/ws" && cargo generate-lockfile --offline) >/dev/null 2>"${ERR_F}" \
    && real_git add -A >/dev/null 2>>"${ERR_F}" \
    && real_git commit -q -m fixture >/dev/null 2>>"${ERR_F}"
}
# run_real: 写した検査を引数なしで流す。共有の target ディレクトリへ書かせない。
run_real() {
  rc=0
  (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR ./scripts/dep-graph-wgpu-check.sh) \
    >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
}

PLAIN='[dependencies]
plain-dummy = { path = "../../ext/plain-dummy" }'
if real_git init -q >/dev/null 2>"${ERR_F}" && real_app "${PLAIN}"; then
  # (a) 一般の依存だけ
  run_real
  judge "real_clean" 0 "o:scanned: reading=metadata crates=2 narrow=0 wide=0 no_repository=0" \
    "o:scanned: reading=tree crates=2" "o:workspace_members=1 narrow_total=0 wide_total=0 no_repository=0" \
    "o:ok: " "!violation" "!error:" "!${rw}"
  if grep -Eq '^record: commit=[0-9a-f]{40,64} ' "${OUT_F}"; then
    report_ok "real_clean_commit_is_clean_sha"
  else
    report_fail "real_clean_commit_is_clean_sha" "commit must be a full hex sha without -dirty"
  fi
  count_is "real_clean_two_scanned" "${OUT_F}" "scanned: " 2

  # (g) 追跡済みのファイルを触ると -dirty が付く
  printf '%s\n' '// touched' >>"${rw}/ws/app/src/lib.rs"
  run_real
  judge "real_dirty" 0 "o:ok: " "!${rw}"
  if grep -Eq '^record: commit=[0-9a-f]{40,64}-dirty ' "${OUT_F}"; then
    report_ok "real_dirty_suffix"
  else
    report_fail "real_dirty_suffix" "commit must end with -dirty"
  fi
  real_git checkout -- . >/dev/null 2>&1 || true

  # (b) 通常の依存に狭い名前
  real_app '[dependencies]
wgpu = { path = "../../ext/wgpu" }' || report_fail "real_narrow_normal_setup" "cannot prepare"
  run_real
  judge "real_narrow_normal" 1 "e:violation: definition=narrow crate=wgpu version=0.1.0" "e:FAILED: 1 crate(s)" \
    "o:narrow_total=1 wide_total=0" "!ok: " "!${rw}"

  # (c) 広い定義にだけ当たる crate を dev-dependencies に
  real_app '[dev-dependencies]
gfx-lookalike = { path = "../../ext/gfx-lookalike" }' || report_fail "real_wide_dev_setup" "cannot prepare"
  run_real
  judge "real_wide_dev" 1 "e:violation: definition=wide crate=gfx-lookalike version=0.1.0" "e:FAILED: 1 crate(s)" \
    "o:narrow_total=0 wide_total=1" "!definition=narrow" "!ok: " "!${rw}"

  # (d) build-dependencies に狭い名前
  real_app '[build-dependencies]
naga = { path = "../../ext/naga" }' || report_fail "real_narrow_build_setup" "cannot prepare"
  run_real
  judge "real_narrow_build" 1 "e:definition=narrow crate=naga" "e:FAILED: 1 crate(s)" "!ok: " "!${rw}"

  # (e) optional（feature の裏）の狭い名前（--all-features を外すと見逃す）
  real_app '[dependencies]
wgpu = { path = "../../ext/wgpu", optional = true }

[features]
gpu = ["dep:wgpu"]' || report_fail "real_narrow_optional_setup" "cannot prepare"
  run_real
  judge "real_narrow_optional" 1 "e:definition=narrow crate=wgpu" "e:FAILED: 1 crate(s)" "!ok: " "!${rw}"

  # (f) 別 target の依存の狭い名前（--target all を外すと見逃す）
  real_app "[target.'cfg(windows)'.dependencies]
naga = { path = \"../../ext/naga\" }" || report_fail "real_narrow_target_setup" "cannot prepare"
  run_real
  judge "real_narrow_target" 1 "e:definition=narrow crate=naga" "e:FAILED: 1 crate(s)" "!ok: " "!${rw}"

  # (h) 実物の出力をファイルに落として --metadata-json / --tree-text で渡す経路でも同じ判定になる
  # （フラグは検査の「数え方の定義」と同じ。定義を変えたらここも合わせる）
  real_app "${PLAIN}" || report_fail "real_files_setup" "cannot prepare"
  if (cd "${rw}/ws" \
      && cargo metadata --locked --format-version 1 --all-features >"${rw}/md.json" 2>/dev/null \
      && cargo tree --locked --workspace --all-features --target all -e normal,build,dev --prefix none --no-dedupe \
        >"${rw}/tree.txt" 2>/dev/null); then
    run_check --metadata-json "${rw}/md.json" --tree-text "${rw}/tree.txt"
    judge "real_files_clean" 0 "o:scanned: reading=tree crates=2" "o:record: commit=- crates_total=2 " "o:ok: " "!${rw}"
  else
    report_fail "real_files_clean" "cannot produce real cargo output"
  fi
  real_app '[dependencies]
naga = { path = "../../ext/naga" }' || report_fail "real_files_bad_setup" "cannot prepare"
  if (cd "${rw}/ws" \
      && cargo metadata --locked --format-version 1 --all-features >"${rw}/md2.json" 2>/dev/null \
      && cargo tree --locked --workspace --all-features --target all -e normal,build,dev --prefix none --no-dedupe \
        >"${rw}/tree2.txt" 2>/dev/null); then
    run_check --metadata-json "${rw}/md2.json" --tree-text "${rw}/tree2.txt"
    judge "real_files_narrow" 1 "e:definition=narrow crate=naga" "o:commit=- " "!ok: " "!${rw}"
  else
    report_fail "real_files_narrow" "cannot produce real cargo output"
  fi

  # (i) 弱い依存の feature（dep?/feat）で metadata にだけ載る crate。一覧が空なら不一致、一覧にあれば合格
  # （feature の裏の依存が cargo tree に出ない構成。オフラインの path crate だけで再現する）
  mkdir -p "${rw}/ext/weak-host/src" "${rw}/ext/weak-dep/src"
  printf '[package]\nname = "weak-host"\nversion = "0.1.0"\nedition = "2021"\nrepository = "https://example.invalid/host"\n\n[dependencies]\nweak-dep = { path = "../weak-dep", optional = true }\n\n[features]\nx = ["weak-dep?/f"]\n' \
    >"${rw}/ext/weak-host/Cargo.toml"
  printf '[package]\nname = "weak-dep"\nversion = "0.1.0"\nedition = "2021"\nrepository = "https://example.invalid/dep"\n\n[features]\nf = []\n' \
    >"${rw}/ext/weak-dep/Cargo.toml"
  printf '%s\n' '// fixture' >"${rw}/ext/weak-host/src/lib.rs"
  printf '%s\n' '// fixture' >"${rw}/ext/weak-dep/src/lib.rs"
  real_app '[dependencies]
weak-host = { path = "../../ext/weak-host", features = ["x"] }' || report_fail "real_weak_setup" "cannot prepare"
  run_real
  judge "real_weak_unlisted" 2 "e:error: cargo metadata and cargo tree disagree on the crate set" "!ok: " "!record: " "!${rw}"
  make_variant "${work}/variant-weak" '(("weak-dep", "0.1.0"),)'
  cp "${work}/variant-weak/scripts/dep-graph-wgpu-check.sh" "${rw}/ws/scripts/dep-graph-wgpu-check.sh"
  run_real
  judge "real_weak_listed" 0 "o:ok: " "o:metadata-only: crate=weak-dep version=0.1.0" "o:scanned: reading=metadata crates=3 " \
    "o:scanned: reading=tree crates=2" "!error:" "!${rw}"
  # 通常の依存に切り替えると tree にも載るので、一覧の記載は古くなり不一致
  real_app '[dependencies]
weak-dep = { path = "../../ext/weak-dep" }' || report_fail "real_weak_stale_setup" "cannot prepare"
  run_real
  judge "real_weak_listed_now_in_tree" 2 "e:error: listed metadata-only crate is not metadata-only: weak-dep 0.1.0" "!ok: " "!record: " "!${rw}"
  cp "${PIN_ROOT}/scripts/dep-graph-wgpu-check.sh" "${rw}/ws/scripts/dep-graph-wgpu-check.sh"
else
  report_fail "real_fixture_setup" "cannot prepare the temporary workspace"
fi

# ---- 陽性対照（実物の検査を製品の workspace に当てる）----
# 製品の workspace の最小の写し（Cargo.toml・Cargo.lock・crates/・toolchain）を一時ディレクトリに作り、
# 実物の検査はそのまま合格すること、実物の一覧に成り立たない記載を 1 件足した写しは終了コード 2 になることを確かめる。
pc="${work}/product"
mkdir -p "${pc}"
cp "${ROOT_DIR}/Cargo.toml" "${ROOT_DIR}/Cargo.lock" "${pc}/"
[ -f "${ROOT_DIR}/rust-toolchain.toml" ] && cp "${ROOT_DIR}/rust-toolchain.toml" "${pc}/"
cp -R "${ROOT_DIR}/crates" "${pc}/crates"
if real_literal="$(python3 -I "${AST_HELPER}" extend "${REAL_CHECK}")" \
  && mkdir -p "${pc}/scripts" && cp "${DIR}/lib.sh" "${DIR}/dep-graph-wgpu-check.sh" "${pc}/scripts/" \
  && GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${pc}" init -q \
  && GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${pc}" add -A \
  && GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${pc}" -c core.hooksPath=/dev/null \
    -c user.name=test -c user.email=test@example.invalid commit -q -m fixture; then
  pc_run() {
    rc=0
    (cd "${pc}" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR CARGO_NET_OFFLINE=true "$1") >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
  }
  # 実物の検査（一覧は実物のまま）: 合格
  pc_run ./scripts/dep-graph-wgpu-check.sh
  judge "positive_control_real_ok" 0 "o:ok: " "o:record: commit=" "!error:" "!violation"
  # 実物の一覧に成り立たない記載を 1 件足した写し: 終了コード 2
  make_variant "${work}/variant-pc" "${real_literal}"
  cp "${work}/variant-pc/scripts/dep-graph-wgpu-check.sh" "${pc}/scripts/dep-graph-wgpu-check.sh"
  pc_run ./scripts/dep-graph-wgpu-check.sh
  judge "positive_control_extra_entry_fails" 2 \
    "e:error: listed metadata-only crate is not metadata-only: positive-control-impossible 0.0.0" "!ok: " "!record: "
else
  report_fail "positive_control_setup" "cannot prepare the product workspace copy"
fi

echo "passed: ${passed}, failed: ${failed}"
[ "${failed}" -eq 0 ]
