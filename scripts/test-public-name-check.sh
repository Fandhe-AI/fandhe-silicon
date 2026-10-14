#!/usr/bin/env bash
# public-name-check.sh の回帰テスト（spec TASK-14・TASK-113 のポインタ。親 Issue #394）。
# 一時ディレクトリに、ダミーの語の一覧と合成 rustdoc JSON・合成ソースを作って検査へ渡し、
# 終了コード・標準出力・標準エラーの文言を確かめる。語の一覧は実物の public-name-check.conf ではなく
# テスト用のダミーの語（zzname など）で、cargo を呼ばない（--json の経路）。
# 呼び出し元: `make test-public-name-check`（直接実行も可）。前提: python3。
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う（終了コードだけでは別の理由の失敗を見逃す）。
# 期待値を緩めない。食い違ったらテストでなく検査側の挙動として直す。
# 途中で止めず、最後に件数を出す。bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

require_cmd python3

CHECK="${DIR}/public-name-check.sh"
work="$(mktemp -d "${TMPDIR:-/tmp}/test-public-name-check.XXXXXX")"
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

d = base()
d["index"]["2"] = item(2, "Zz_Name_Thing", {"struct": {}})
put("name_hit", d)

d = base()
d["index"]["2"] = item(2, "myZZNAME", {"function": {}})
d["index"]["3"] = item(3, "reexport", {"use": {"name": "ZzName", "source": "x"}})
put("name_hit_multi", d)

# 非公開・他 crate のアイテムは数えない。
d = base()
d["index"]["2"] = item(2, "zzname_private", {"struct": {}}, vis="crate")
d["index"]["3"] = item(3, "zzname_ext", {"struct": {}}, crate_id=1)
put("name_nonpublic", d)

# 名前をつないだ文字列には当てない（アイテム 1 つずつ）。
d = base()
d["index"]["2"] = item(2, "zz", {"struct": {}})
d["index"]["3"] = item(3, "name", {"struct": {}})
put("name_split", d)

d = base()
d["includes_private"] = True
put("bad_private", d)
d = base()
del d["index"]
put("bad_no_index", d)
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
d["index"] = {"0": item(0, "dummy", {"module": {}}, vis="crate")}
put("zero_public", d)
with open(os.path.join(out, "broken.json"), "w", encoding="utf-8") as f:
    f.write("{not json")
PY

# 設定・ソースの合成ファイル（テスト用のダミーの語）
printf '# dummy\n[names]\nzzname\nother_Name\n[words]\nFoo\nbar_1\n[calls]\ncallit(\n' >"${work}/ok.conf"
printf '[names]\nzzname\n[words]\nFoo\n' >"${work}/missing_calls.conf"
printf '[names]\nzzname\n[words]\nFoo\n[calls]\n' >"${work}/empty_calls.conf"
printf 'zzname\n[names]\nzzname\n' >"${work}/before_section.conf"
printf '[names]\na1\n[words]\nFoo\n[calls]\nx(\n[bogus]\n' >"${work}/unknown_section.conf"
printf '[names]\na1\n[names]\nb1\n[words]\nFoo\n[calls]\nx(\n' >"${work}/dup_section.conf"
printf '[names]\na1\na1\n[words]\nFoo\n[calls]\nx(\n' >"${work}/dup_word.conf"
printf '[names]\n--\n[words]\nFoo\n[calls]\nx(\n' >"${work}/empty_norm.conf"
printf '[names]\na1\n[words]\nfoo-bar\n[calls]\nx(\n' >"${work}/bad_word.conf"
printf '[names]\na1\n[words]\nFoo\n[calls]\nx (\n' >"${work}/space_call.conf"
printf 'fn plain() {}\nlet football = 1; // foobar _foo bar_12\n' >"${work}/src_clean.rs"
printf '// FOO is a word\nlet x = bar_1;\nlet y = (foo);\n' >"${work}/src_words.rs"
printf 'let r = CallIt(1);\nlet s = recallit(2);\n' >"${work}/src_calls.rs"

# check <名前> <期待する終了コード> <標準出力に含む語|-> <標準エラーに含む語|-> <引数...>
check() {
    local name="$1" want="$2" want_out="$3" want_err="$4"
    shift 4
    rc=0
    "${CHECK}" "$@" >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    local ok=1
    [[ "${rc}" -eq "${want}" ]] || ok=0
    if [[ "${want_out}" != "-" ]] && ! grep -qF -- "${want_out}" "${OUT_F}"; then ok=0; fi
    if [[ "${want_err}" != "-" ]] && ! grep -qF -- "${want_err}" "${ERR_F}"; then ok=0; fi
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

# 終了コード 0
check "clean は 0 件で ok" 0 "ok: 1 public API document(s)" - --config "${C}" --json "$(J clean)"
check "clean の record" 0 "name_hits_total=0 sources=0 source_hits_total=0" - --config "${C}" --json "$(J clean)"
check "record の commit は -" 0 "record: commit=- documents=1" - --config "${C}" --json "$(J clean)"
check "非公開・他 crate は数えない" 0 "name_hits=0" - --config "${C}" --json "$(J name_nonpublic)"
check "名前をつないだ文字列には当てない" 0 "name_hits=0" - --config "${C}" --json "$(J name_split)"

# 名前の正規化と部分一致（終了コード 1）
check "名前の部分一致（記号・大小文字を無視）" 1 "name_hits=1" "item=Zz_Name_Thing word=zzname" --config "${C}" --json "$(J name_hit)"
check "FAILED の行" 1 "name_hits_total=1" "FAILED: 1 hit(s)" --config "${C}" --json "$(J name_hit)"
check "関数名と再公開の名前の両方" 1 "name_hits=2" "item=ZzName" --config "${C}" --json "$(J name_hit_multi)"
check "複数 JSON の合計" 1 "documents=2 public_items_total=5 name_hits_total=1" - --config "${C}" --json "$(J clean)" "$(J name_hit)"

# 単語・呼び出しの形（--source）
check "単語単位の一致（大小文字を無視）" 1 "word_hits=3 call_hits=0" "source=src_words.rs line=1 word=Foo" \
    --config "${C}" --json "$(J clean)" --source "${work}/src_words.rs"
check "単語の一部・前後の _ には当てない" 0 "word_hits=0 call_hits=0" - \
    --config "${C}" --json "$(J clean)" --source "${work}/src_clean.rs"
check "呼び出しの形は部分一致（大小文字を無視）" 1 "word_hits=0 call_hits=2" "word=callit(" \
    --config "${C}" --json "$(J clean)" --source "${work}/src_calls.rs"
check "ソースを渡さなければ走らない" 0 "sources=0" - --config "${C}" --json "$(J clean)"
check "ソースの合計" 1 "sources=2 source_hits_total=5" - \
    --config "${C}" --json "$(J clean)" --source "${work}/src_words.rs" "${work}/src_calls.rs"

# 終了コード 2（誤用・前提の不足・読み切れない入力）
check "引数なし" 2 - -
check "--config 無し" 2 - - --json "$(J clean)"
check "未知の引数" 2 - - --config "${C}" --bogus
check "設定が無い" 2 - "cannot read the config file" --config "${work}/none.conf" --json "$(J clean)"
check "節の欠落" 2 - "missing config section" --config "${work}/missing_calls.conf" --json "$(J clean)"
check "節が空" 2 - "empty config section" --config "${work}/empty_calls.conf" --json "$(J clean)"
check "最初の節の前の語" 2 - "word before the first config section" --config "${work}/before_section.conf" --json "$(J clean)"
check "未知の節" 2 - "unknown config section" --config "${work}/unknown_section.conf" --json "$(J clean)"
check "節の重複" 2 - "duplicate config section" --config "${work}/dup_section.conf" --json "$(J clean)"
check "語の重複" 2 - "duplicate word" --config "${work}/dup_word.conf" --json "$(J clean)"
check "正規化すると空の語" 2 - "empty after normalization" --config "${work}/empty_norm.conf" --json "$(J clean)"
check "[words] の書式違反" 2 - "malformed word in [words]" --config "${work}/bad_word.conf" --json "$(J clean)"
check "[calls] の空白" 2 - "whitespace in [calls] word" --config "${work}/space_call.conf" --json "$(J clean)"
check "JSON が無い" 2 - "cannot read JSON input" --config "${C}" --json "${work}/none.json"
check "JSON が壊れている" 2 - "cannot read JSON input" --config "${C}" --json "$(J broken)"
check "index が無い" 2 - "missing or malformed 'index'" --config "${C}" --json "$(J bad_no_index)"
check "includes_private が true" 2 - "includes_private is not false" --config "${C}" --json "$(J bad_private)"
check "format_version が整数でない" 2 - "format_version is not an integer" --config "${C}" --json "$(J bad_fv)"
check "crate_id が整数でない" 2 - "integer crate_id" --config "${C}" --json "$(J bad_crate_id)"
check "root の crate_id が 0 でない" 2 - "malformed root item" --config "${C}" --json "$(J bad_root_crate)"
check "公開アイテムが 0 件" 2 - "no public items found" --config "${C}" --json "$(J zero_public)"
check "ソースが無い" 2 - "cannot read source input" --config "${C}" --json "$(J clean)" --source "${work}/none.rs"
# 2 のときは締めの行を出さない（成功扱いにしない）。直前の実行の標準出力を見る。
if grep -q '^ok:' "${OUT_F}" || grep -q '^record:' "${OUT_F}"; then
    failed=$((failed + 1))
    echo "FAIL: 終了コード 2 で ok/record を出した" >&2
else
    passed=$((passed + 1))
fi

echo "test-public-name-check: passed=${passed} failed=${failed}"
[[ "${failed}" -eq 0 ]]
