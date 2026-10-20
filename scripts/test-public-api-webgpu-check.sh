#!/usr/bin/env bash
# public-api-webgpu-check.sh の回帰テスト（spec TASK-241・BUILD-29 のポインタ。親 Issue #465 の 3/6。内容の (1) の検査の試験）。
# 一時ディレクトリに、ダミーの語の一覧と合成 rustdoc JSON を作って検査へ渡し、終了コード・標準出力・標準エラーの文言を確かめる。
#   前半は --json の経路（合成 JSON。cargo・git を呼ばない）。実物の語の一覧が書式どおり読めることと、
#   その各語が当たることも、設定から実行時に入力を作って確かめる（語はテストに書き写さない）。
#   後半は一時 workspace に検査・public-api-check.sh・lib.sh を写し、package 名がちょうど fandhe-silicon-core と
#   fandhe-silicon-contract の依存なしの 2 crate を置いて、実物の cargo rustdoc の JSON で既定の経路
#   （feature native-blocking-wait を外した構成・commit の記録・違反を 1 件足すと落ちること・終了コード 2 の経路）を通す。
#   ネットワーク不要・リポジトリを変更しない。
# 呼び出し元: `make test-public-api-webgpu-check`（直接実行も可。CI のジョブからは呼ばず手元で流す）。
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
python3 -I - "${work}" <<'PY'
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
with open(os.path.join(out, "broken.json"), "w", encoding="utf-8") as f:
    f.write("{not json")
PY

# 設定の合成ファイル（テスト用のダミーの語）
printf '# dummy\n[blocking-words]\nzzblock\nzzwait\n' >"${work}/ok.conf"
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
        elif sec == "blocking-words":
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
    printf '[package]\nname = "fandhe-silicon-contract"\nversion = "0.0.0"\nedition = "2024"\n\n[features]\n'
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

libs 'pub struct Own;' 'pub struct Own;'
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

    # 陽性対照: 外した feature の裏にある語の名前の関数は、その feature を外した構成では 0 件で通る。
    libs 'pub struct Own;' "pub struct Own;
#[cfg(feature = \"native-blocking-wait\")]
pub fn ${VIOL}() {}"
    real real_feature_gated 0 "blocking_wait_names_total=0" -
    not_in real_feature_gated_no_violation "${ERR_F}" 'violation'
    has_in real_feature_gated_dirty "${OUT_F}" '^record: commit=[0-9a-f]+-dirty documents=2 '

    # 同じ関数を cfg なしで置くと落ちる（違反を 1 件足すと 1 になる）。
    libs 'pub struct Own;' "pub struct Own;
pub fn ${VIOL}() {}"
    real real_contract_fn 1 "blocking_wait_names_total=1" "violation: crate=fandhe_silicon_contract features=without-native-blocking-wait item=${VIOL} kind=function"
    has_in real_contract_fn_failed "${ERR_F}" 'FAILED: 1 item\(s\)'
    not_in real_contract_fn_no_ok "${OUT_F}" '^ok:'

    # core の crate も対象。
    libs "pub fn ${VIOL}() {}" 'pub struct Own;'
    real real_core_fn 1 "blocking_wait_names_total=1" "violation: crate=fandhe_silicon_core "
    not_in real_core_fn_no_ok "${OUT_F}" '^ok:'

    # inherent のメソッド。
    libs 'pub struct Own;' "pub struct Own;
impl Own {
    pub fn ${VIOL}(&self) {}
}"
    real real_method 1 "blocking_wait_names_total=1" "item=${VIOL} kind=function"

    # 再公開（局所名が語を含む）。
    libs 'pub struct Own;' "mod m {
    pub fn ${VIOL}() {}
}
pub use m::${VIOL};"
    real real_reexport 1 - "kind=use word=${first_word}"

    libs 'this does not compile' 'pub struct Own;'
    real real_build_fail 2 - "public-api-check.sh --emit-json failed"

    # 契約の crate を外した workspace は、片方しか無い構成を成功扱いにしない。
    libs 'pub struct Own;' 'pub struct Own;'
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
