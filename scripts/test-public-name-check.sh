#!/usr/bin/env bash
# public-name-check.sh の回帰テスト（spec TASK-14・TASK-113 のポインタ。親 Issue #394）。
# 一時ディレクトリに、ダミーの語の一覧と合成 rustdoc JSON・合成ソースを作って検査へ渡し、
# 終了コード・標準出力・標準エラーの文言を確かめる。語の一覧は実物の public-name-check.conf ではなく
# テスト用のダミーの語（zzname など）を使う。節は 2 つ:
#   前半は --json の経路（合成 JSON・合成ソース。cargo を呼ばない）。実物の設定が書式どおり読めることと、
#   設定の各語がそれぞれの照合で当たることも、設定から実行時に入力を作って確かめる（語はテストに書き写さない）。
#   後半は一時 workspace に検査・public-api-check.sh・lib.sh を写し、実物の cargo rustdoc の JSON で既定の経路
#   （JSON を作る段・commit の記録・終了コード 2 の経路）を通す。依存なし・ネットワーク不要・リポジトリを変更しない。
# 呼び出し元: `make test-public-name-check`（直接実行も可。CI の `deny` ジョブも呼ぶ）。前提: python3・cargo・git（無ければ失敗。skip しない）。
# 判定は「終了コード + 標準出力 + 標準エラーの部分一致」で行う（終了コードだけでは別の理由の失敗を見逃す）。
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

# 非 glob の再公開は name と use.name が同じ局所名。1 件を 2 回数えない。
d = base()
d["index"]["2"] = item(2, "ZzName", {"use": {"name": "ZzName", "source": "x"}})
put("name_use_same", d)

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


# --exact-fields 用: 経路 [dummy, zzmod, ZzIn] の struct と、そのフィールド。
def fields_doc(names, vis=None, plain=True, path_crate=0):
    d = base()
    ids = []
    for i, n in enumerate(names):
        fid = 10 + i
        d["index"][str(fid)] = item(fid, n, {"struct_field": {}}, vis=(vis or {}).get(n, "public"))
        ids.append(fid)
    kind = {"plain": {"fields": ids, "has_stripped_fields": False}} if plain else {"tuple": ids}
    d["index"]["5"] = item(5, "ZzIn", {"struct": {"kind": kind}})
    d["paths"]["5"] = {"crate_id": path_crate, "path": ["dummy", "zzmod", "ZzIn"], "kind": "struct"}
    return d


put("fields_ok", fields_doc(["fa", "fb"]))
put("fields_extra", fields_doc(["fa", "fb", "fc"]))
put("fields_missing", fields_doc(["fa"]))
put("fields_private_extra", fields_doc(["fa", "fb", "fp"], vis={"fp": "crate"}))
put("fields_tuple", fields_doc(["fa", "fb"], plain=False))
put("fields_other_crate", fields_doc(["fa", "fb"], path_crate=1))
PY

# 設定・ソースの合成ファイル（テスト用のダミーの語）
printf '# dummy\n[names]\nzzname\nother_Name\n[words]\nFoo\nbar_1\n[calls]\ncallit(\n' >"${work}/ok.conf"
printf '[names]\nzzname\n[words]\nFoo\n' >"${work}/missing_calls.conf"
printf '[names]\nzzname\n[words]\nFoo\n[calls]\n' >"${work}/empty_calls.conf"
printf 'zzname\n[names]\nzzname\n' >"${work}/before_section.conf"
printf '[names]\na1\n[words]\nFoo\n[calls]\nx(\n[bogus]\n' >"${work}/unknown_section.conf"
printf '[names]\na1\n[words]\nFoo\n[CALLS]\nx(\n' >"${work}/upper_section.conf"
printf '[names]\na1\n[words]\nFoo\n[calls]\nx(\n[calls_extra]\ny(\n' >"${work}/extra_section.conf"
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
REAL_CONF="${DIR}/../public-name-check.conf"
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
check "name と use.name が同じ再公開は 1 回" 1 "name_hits_total=1" "FAILED: 1 hit(s)" --config "${C}" --json "$(J name_use_same)"
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

# 公開フィールドの集合（--exact-fields）
FS="zzmod::ZzIn=fa,fb"
check "フィールドが一致なら 0" 0 "scanned-fields: crate=dummy features=- item=zzmod::ZzIn public_fields=fa,fb" - \
    --config "${C}" --json "$(J fields_ok)" --exact-fields "${FS}"
check "フィールド指定の record" 0 "field_specs=1 field_hits_total=0" - \
    --config "${C}" --json "$(J fields_ok)" --exact-fields "${FS}"
check "フィールドが 1 つ多いと 1" 1 - "problem=unexpected-field:fc" \
    --config "${C}" --json "$(J fields_extra)" --exact-fields "${FS}"
check "フィールドの個数も違反に出る" 1 "field_hits_total=2" "problem=field-count:3" \
    --config "${C}" --json "$(J fields_extra)" --exact-fields "${FS}"
check "フィールドが足りないと 1" 1 - "problem=missing-field:fb" \
    --config "${C}" --json "$(J fields_missing)" --exact-fields "${FS}"
check "非公開のフィールドは数えない" 0 "public_fields=fa,fb" - \
    --config "${C}" --json "$(J fields_private_extra)" --exact-fields "${FS}"
check "タプル構造体は違反" 1 - "problem=not-a-plain-struct" \
    --config "${C}" --json "$(J fields_tuple)" --exact-fields "${FS}"
check "指定の struct が JSON に無いと 2" 2 - "struct not found in any JSON" \
    --config "${C}" --json "$(J clean)" --exact-fields "${FS}"
check "他 crate の経路は見つからず 2" 2 - "struct not found in any JSON" \
    --config "${C}" --json "$(J fields_other_crate)" --exact-fields "${FS}"
check "経路が違うと見つからず 2" 2 - "struct not found in any JSON" \
    --config "${C}" --json "$(J fields_ok)" --exact-fields "other::ZzIn=fa,fb"
check "複数 JSON は見つかったものだけ検査" 1 "field_specs=1" "problem=unexpected-field:fc" \
    --config "${C}" --json "$(J clean)" "$(J fields_extra)" --exact-fields "${FS}"
check "--exact-fields の書式違反" 2 - "malformed --exact-fields value" \
    --config "${C}" --json "$(J fields_ok)" --exact-fields "ZzIn=fa"
check "--exact-fields のフィールド重複" 2 - "duplicate field" \
    --config "${C}" --json "$(J fields_ok)" --exact-fields "zzmod::ZzIn=fa,fa"
check "指定しなければ record に field_specs が出ない" 0 "sources=0 source_hits_total=0" - \
    --config "${C}" --json "$(J fields_extra)"

# 終了コード 2（誤用・前提の不足・読み切れない入力）
check "引数なし" 2 - -
check "--config 無し" 2 - - --json "$(J clean)"
check "未知の引数" 2 - - --config "${C}" --bogus
check "設定が無い" 2 - "cannot read the config file" --config "${work}/none.conf" --json "$(J clean)"
check "節の欠落" 2 - "missing config section" --config "${work}/missing_calls.conf" --json "$(J clean)"
check "節が空" 2 - "empty config section" --config "${work}/empty_calls.conf" --json "$(J clean)"
check "最初の節の前の語" 2 - "word before the first config section" --config "${work}/before_section.conf" --json "$(J clean)"
check "未知の節" 2 - "unknown config section" --config "${work}/unknown_section.conf" --json "$(J clean)"
check "大文字の節名" 2 - "unknown config section" --config "${work}/upper_section.conf" --json "$(J clean)"
check "接尾の付いた節名" 2 - "unknown config section" --config "${work}/extra_section.conf" --json "$(J clean)"
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
check "JSON 1 本だけが 0 件でも 2" 2 - "no public items found" --config "${C}" --json "$(J clean)" "$(J zero_public)"
check "実物の設定が書式どおり読める" 0 "ok: 1 public API document(s)" - --config "${REAL_CONF}" --json "$(J clean)"
printf '\377\376bad\n' >"${work}/bad_utf8.conf"
check "UTF-8 でない設定" 2 - "cannot read the config file" --config "${work}/bad_utf8.conf" --json "$(J clean)"
printf '\377\376bad\n' >"${work}/bad_utf8.rs"
check "UTF-8 でないソース" 2 - "cannot read source input" --config "${C}" --json "$(J clean)" --source "${work}/bad_utf8.rs"
check "ソースがディレクトリ" 2 - "cannot read source input" --config "${C}" --json "$(J clean)" --source "${work}"

# 実物の設定の各語が、それぞれの照合で当たること。入力は設定から実行時に作る（語はここに書かない）。
mkdir -p "${work}/gen"
python3 -I - "${REAL_CONF}" "${work}/gen" <<'PY'
import json
import os
import sys

conf, out = sys.argv[1], sys.argv[2]
sec = None
words = {"names": [], "words": [], "calls": []}
with open(conf, encoding="utf-8") as f:
    for raw in f:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[") and line.endswith("]"):
            sec = line[1:-1]
        elif sec in words:
            words[sec].append(line)
for i, w in enumerate(words["names"]):
    mod = {"id": 0, "crate_id": 0, "name": "dummy", "visibility": "public"}
    mod["inner"] = {"module": {"is_crate": True, "items": [1]}}
    st = {"id": 1, "crate_id": 0, "name": "Q_" + w.upper() + "_q", "visibility": "public"}
    st["inner"] = {"struct": {}}
    doc = {"root": 0, "format_version": 61, "includes_private": False, "external_crates": {}}
    doc["paths"] = {}
    doc["index"] = {"0": mod, "1": st}
    with open(os.path.join(out, "n%d.json" % i), "w", encoding="utf-8") as g:
        json.dump(doc, g)
for i, w in enumerate(words["words"]):
    with open(os.path.join(out, "w%d.rs" % i), "w", encoding="utf-8") as g:
        g.write("let x = 1; // %s here\n" % w)
for i, w in enumerate(words["calls"]):
    with open(os.path.join(out, "c%d.rs" % i), "w", encoding="utf-8") as g:
        g.write("let x = %s);\n" % w)
PY
for f in "${work}"/gen/n*.json; do
    check "実物の [names] の語が当たる: $(basename "${f}")" 1 - "FAILED:" --config "${REAL_CONF}" --json "$(J clean)" "${f}"
done
for f in "${work}"/gen/w*.rs "${work}"/gen/c*.rs; do
    check "実物の [words]・[calls] の語が当たる: $(basename "${f}")" 1 - "FAILED:" --config "${REAL_CONF}" --json "$(J clean)" --source "${f}"
done
check "ソースが無い" 2 - "cannot read source input" --config "${C}" --json "$(J clean)" --source "${work}/none.rs"
# 2 のときは締めの行を出さない（成功扱いにしない）。直前の実行の標準出力を見る。
if grep -q '^ok:' "${OUT_F}" || grep -q '^record:' "${OUT_F}"; then
    failed=$((failed + 1))
    echo "FAIL: 終了コード 2 で ok/record を出した" >&2
else
    passed=$((passed + 1))
fi

# ---- 実物の rustdoc の JSON（既定の経路）----
# 一時 workspace に検査・public-api-check.sh・lib.sh を写して流す（先例: test-public-api-check.sh の実物の節）。
# 依存なし。target は一時 workspace の中に出る（.gitignore で dirty にしない）。語の一覧は ws の外のダミーの語。
rw="${work}/real"
mkdir -p "${rw}/ws/crates/api/src" "${rw}/ws/scripts"
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
TOML
printf 'target/\n' >"${rw}/ws/.gitignore"
printf '[names]\nzzname\n[words]\nFoo\n[calls]\ncallit(\n' >"${rw}/name.conf"
cp "${DIR}/public-name-check.sh" "${DIR}/public-api-check.sh" "${DIR}/lib.sh" "${rw}/ws/scripts/"
chmod +x "${rw}/ws/scripts/public-name-check.sh" "${rw}/ws/scripts/public-api-check.sh"
if [[ -f "${DIR}/../rust-toolchain.toml" ]]; then
    cp "${DIR}/../rust-toolchain.toml" "${rw}/ws/rust-toolchain.toml"
fi

real_lib() { printf '%s\n' "$1" >"${rw}/ws/crates/api/src/lib.rs"; }
# 開発者の全体設定とフックを効かせない git。
real_git() {
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${rw}/ws" \
        -c core.hooksPath=/dev/null -c user.name=test -c user.email=test@example.invalid "$@"
}
# judge_real <名前> <期待する終了コード> <標準出力に含む語|-> <標準エラーに含む語|->:
# 直近の実行を終了コード・出力の部分一致で判定する。標準出力にホストの絶対パスが出ていないことも見る（標準エラーには cargo 自身の出力が混ざる）。
judge_real() {
    local name="$1" want="$2" want_out="$3" want_err="$4" ok=1
    [[ "${rc}" -eq "${want}" ]] || ok=0
    if [[ "${want_out}" != "-" ]] && ! grep -qF -- "${want_out}" "${OUT_F}"; then ok=0; fi
    if [[ "${want_err}" != "-" ]] && ! grep -qF -- "${want_err}" "${ERR_F}"; then ok=0; fi
    if grep -qF -- "${work}" "${OUT_F}"; then ok=0; fi
    if [[ "${ok}" -eq 1 ]]; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        echo "FAIL: ${name} (rc=${rc}, want=${want}, out~${want_out}, err~${want_err})" >&2
        sed 's/^/  out: /' "${OUT_F}" >&2
        sed 's/^/  err: /' "${ERR_F}" >&2
    fi
}
# real <名前> <期待する終了コード> <標準出力に含む語|-> <標準エラーに含む語|-> [追加の引数...]:
# 写した検査を流して判定する。共有の target ディレクトリへ書かせない。
real() {
    local name="$1" want="$2" want_out="$3" want_err="$4"
    shift 4
    rc=0
    (cd "${rw}/ws" && env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR \
        ./scripts/public-name-check.sh --config "${rw}/name.conf" "$@") >"${OUT_F}" 2>"${ERR_F}" </dev/null || rc=$?
    judge_real "${name}" "${want}" "${want_out}" "${want_err}"
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
# not_in <名前> <ファイル> <正規表現>: 直近の出力に正規表現が無いこと（失敗を成功扱いにしない）。
not_in() {
    if grep -qE -- "$3" "$2"; then
        failed=$((failed + 1))
        echo "FAIL: $1 (unexpected: $3)" >&2
    else
        passed=$((passed + 1))
    fi
}

real_lib 'pub struct Own;'
if (cd "${rw}/ws" && cargo generate-lockfile --offline) >/dev/null 2>"${ERR_F}" \
    && real_git init -q >/dev/null 2>>"${ERR_F}" \
    && real_git add -A >/dev/null 2>>"${ERR_F}" \
    && real_git commit -q -m fixture >/dev/null 2>>"${ERR_F}"; then
    real real_clean 0 "documents=2" -
    has_in real_clean_commit "${OUT_F}" '^record: commit=[0-9a-f]{40}([0-9a-f]{24})? documents=2 '
    has_in real_clean_ok "${OUT_F}" '^ok: 2 public API document\(s\)'
    not_in real_clean_no_violation "${ERR_F}" 'violation'

    real_lib 'pub struct Own;
pub fn f() {}'
    real real_dirty 0 "documents=2" -
    has_in real_dirty_mark "${OUT_F}" '^record: commit=[0-9a-f]+-dirty documents=2 '

    real_lib 'pub struct ZzNameThing;'
    real real_name_hit 1 "name_hits_total=2" "features=production item=ZzNameThing word=zzname"
    has_in real_name_hit_all "${ERR_F}" 'features=all-features item=ZzNameThing word=zzname'
    has_in real_name_hit_failed "${ERR_F}" 'FAILED: 2 hit\(s\)'
    not_in real_name_hit_no_ok "${OUT_F}" '^ok:'

    real_lib '#[cfg(feature = "leak")]
pub fn zz_name_only() {}'
    real real_name_hit_feature 1 "scanned: crate=api features=production" "features=all-features item=zz_name_only word=zzname"
    # production の組の行は 0 件のまま（組の取り違えで合格しない）
    has_in real_name_hit_feature_production "${OUT_F}" '^scanned: crate=api features=production .*name_hits=0$'

    real_lib 'mod m { pub struct ZzName; }
pub use m::ZzName;'
    real real_reexport 1 - "item=ZzName"

    printf 'let x = 1; // Foo callit(2)\n' >"${rw}/src.rs"
    real_lib 'pub struct Own;'
    real real_source_with_emit 1 "sources=1" "FAILED:" --source "${rw}/src.rs"
    has_in real_source_with_emit_docs "${OUT_F}" 'documents=2 '

    real_lib 'this does not compile'
    real real_build_fail 2 - "public-api-check.sh --emit-json failed"
    not_in real_build_fail_no_closing "${OUT_F}" '^(ok|record):'

    # 以降は public-api-check.sh を差し替える。元のスクリプトは各ケースの後に写し直す。
    printf '#!/usr/bin/env bash\nexit 0\n' >"${rw}/ws/scripts/public-api-check.sh"
    real real_no_emit 2 - "no JSON was emitted"
    not_in real_no_emit_no_closing "${OUT_F}" '^(ok|record):'

    printf '#!/usr/bin/env bash\nexit 1\n' >"${rw}/ws/scripts/public-api-check.sh"
    real real_emit_exit1 2 - "public-api-check.sh --emit-json failed"
    not_in real_emit_exit1_no_closing "${OUT_F}" '^(ok|record):'
    cp "${DIR}/public-api-check.sh" "${rw}/ws/scripts/"

    # コミットが取れなければビルドの前に止まる（lib.rs はコンパイルできない中身のまま）
    mv "${rw}/ws/.git" "${rw}/ws-git-away"
    real_lib 'this does not compile'
    real real_no_commit 2 - "cannot resolve the source commit"
    not_in real_no_commit_no_closing "${OUT_F}" '^(ok|record):'
    not_in real_no_commit_before_build "${ERR_F}" 'emit-json failed'
    mv "${rw}/ws-git-away" "${rw}/ws/.git"
    # "cannot run public-api-check.sh" は bash 自体を起動できないときだけ出る。ここでは再現しない。
else
    failed=$((failed + 1))
    echo "FAIL: real_fixture_setup (cannot prepare the temporary workspace)" >&2
fi

# ---- 決定表の語彙の検査（Makefile の public-name-check の引数）を実物のリポジトリに当てる陽性対照 ----
# 実物の core の rustdoc の JSON とソースの一時コピーに、禁止の語・呼び出し・フィールドを 1 つ足すと落ちること。
# 実物の引数は Makefile から読む（テストに書き写さない）。
DEC_ARGS="$(sed -n 's/^\t@scripts\/public-name-check.sh --config public-name-check.conf //p' "${DIR}/../Makefile" | head -n 1)"
DEC_SRC="${DIR}/../$(printf '%s\n' "${DEC_ARGS}" | sed -n 's/.*--source \([^ ]*\).*/\1/p')"
[[ -f "${DEC_SRC}" ]] || DEC_SRC="${work}/missing-decision-source"
# decision/ 配下に tests.rs 以外のファイルが増えたら、検査の対象（Makefile の --source）に足し忘れないよう落とす。
extra_dec="$(find "${DIR}/../crates/core/src/decision" -type f ! -name tests.rs 2>/dev/null | head -n 1)"
if [[ -z "${extra_dec}" ]]; then
    passed=$((passed + 1))
else
    failed=$((failed + 1))
    echo "FAIL: decision/ に tests.rs 以外のファイルがある（--source に足す）" >&2
fi
real_dir="${work}/realjson"
mkdir -p "${real_dir}"
if [[ -n "${DEC_ARGS}" ]] && bash "${DIR}/public-api-check.sh" --emit-json "${real_dir}" >"${work}/emit.txt" 2>/dev/null \
    && [[ -f "${real_dir}/fandhe_silicon_core.production.json" ]]; then
    RJ="${real_dir}/fandhe_silicon_core.production.json"
    # Makefile の引数のうち、実物のソースの経路を一時コピーへ差し替えて流す。
    dec() { # dec <名前> <期待する終了コード> <標準エラーに含む語|-> <ソース> <JSON> <追加のフィールド指定|->
        local src="$4" js="$5"
        local -a args
        args=(--config "${REAL_CONF}" --json "${js}" --source "${src}")
        # Makefile の --exact-fields の値だけを取り出す
        local spec
        spec="$(printf '%s\n' "${DEC_ARGS}" | sed -n 's/.*--exact-fields \([^ ]*\).*/\1/p')"
        args=("${args[@]}" --exact-fields "${spec}")
        check "$1" "$2" - "$3" "${args[@]}"
    }
    dec "決定表（実物）は合格" 0 - "${DEC_SRC}" "${RJ}" -
    mutated="${work}/mut"
    mkdir -p "${mutated}"
    for w in Metal Vulkan Cuda Apple NVIDIA BackendKind adapter_name M4 Arc; do
        { cat "${DEC_SRC}"; printf '// %s\n' "${w}"; } >"${mutated}/decision.rs"
        dec "決定表のコピーに ${w} を足すと落ちる" 1 "word=${w}" "${mutated}/decision.rs" "${RJ}" -
    done
    { cat "${DEC_SRC}"; printf 'fn x() { diagnostics(1); }\n'; } >"${mutated}/decision.rs"
    dec "決定表のコピーに diagnostics( を足すと落ちる" 1 "word=diagnostics(" "${mutated}/decision.rs" "${RJ}" -
    python3 -I - "${RJ}" "${mutated}/field.json" "${mutated}/tensor.json" <<'PY'
import json
import sys

src, out_field, out_name = sys.argv[1:4]
doc = json.load(open(src, encoding="utf-8"))
sid = next(k for k, v in doc["paths"].items() if v["path"][1:] == ["decision", "DecisionInput"])
fields = doc["index"][sid]["inner"]["struct"]["kind"]["plain"]["fields"]
proto = doc["index"][str(fields[0])]
new = dict(proto, id=999999, name="dtype")
doc["index"]["999999"] = new
fields.append(999999)
json.dump(doc, open(out_field, "w", encoding="utf-8"))
# decision モジュールの公開アイテムの名前に製品固有の語を足す（名前の検査）
doc = json.load(open(src, encoding="utf-8"))
doc["index"]["999998"] = dict(proto, id=999998, name="tensor_shape")
json.dump(doc, open(out_name, "w", encoding="utf-8"))
PY
    dec "DecisionInput にフィールドを 1 つ足すと落ちる" 1 "problem=unexpected-field:dtype" "${DEC_SRC}" "${mutated}/field.json" -
    dec "公開アイテムの名前に tensor を足すと落ちる" 1 "word=tensor" "${DEC_SRC}" "${mutated}/tensor.json" -
else
    failed=$((failed + 1))
    echo "FAIL: decision_real_setup (cannot read Makefile args or emit the real JSON)" >&2
fi

echo "test-public-name-check: passed=${passed} failed=${failed}"
[[ "${failed}" -eq 0 ]]
