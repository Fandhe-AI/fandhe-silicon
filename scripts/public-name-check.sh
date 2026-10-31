#!/usr/bin/env bash
# 製品の crate の公開アイテムの名前と、能力の問合せの決定表のソースに、語の一覧の語が出ていないことを数えて確かめる（--exact-fields で struct の公開フィールドの集合も確かめる）
# （spec TASK-14・TASK-113 のポインタ。関連 TASK-10・TASK-142。親 Issue #394 の 2/4・4/4）。
# 呼び出し元: `make public-name-check`（直接実行も可。CI の `deny` ジョブも呼び、`scanned:` / `record:` の行がジョブのログに残る）。検査の枠（rustdoc の JSON を作る段）は
# `scripts/public-api-check.sh --emit-json` を呼んで共有し、抽出の実体をここに持たない。
#
# 使い方:
#   scripts/public-name-check.sh --config <path> [--json <path>...] [--source <path>...]
#   --config  語の一覧（必須。書式は public-name-check.conf の先頭）。語はこのスクリプトに持たない。
#   --json    渡した rustdoc の JSON だけを名前の検査に使う（cargo を呼ばない。組は `-` と表示）。無ければ JSON を作る。
#   --source  渡したソースのファイルを単語・呼び出しの形の検査に使う。指定したときだけ走る（既定の対象は持たない）。
#   --exact-fields <モジュール経路>::<struct 名>=<f1,f2,...>
#             rustdoc の JSON の中のその struct（crate 内の経路が一致する crate_id 0 のもの）の公開フィールドの名前の集合が
#             列挙どおりで過不足がないことを検査する（例: decision::DecisionInput=capability,scale,residency,scalar）。
#             複数回指定できる。渡した JSON のどれにも見つからなければ終了コード 2。非公開フィールドは数えない。
# 終了コード: 0 一致が 0 件 / 1 一致が 1 件以上（違反） / 2 誤用・前提の不足・入力を読み切れない（成功扱いにしない）
#   2 になるもの: 引数の誤用、設定の欠落・書式違反、指定のファイルが無い・読めない、JSON の形の不一致
#   （public-api-check.sh の V1〜V3 と同じ形の検査）、JSON 1 本あたりの公開アイテムが 0 件、JSON を作る段の失敗、想定外の例外。
#   除外の印・除外の一覧・対象を外す口は持たない。
#
# 照合の規則:
#   名前（match_name）: 名前と [names] の語の両方を、小文字にして [0-9a-z] 以外を除く正規化をしてから部分一致で当てる。
#     名前 1 つずつに当て、パスをつないだ文字列には当てない。対象は crate_id が 0 で visibility が public / default の
#     アイテム（public-api-check.sh の公開アイテムと同じ数え方。enum の variant・struct の field を含む）のうち
#     name が文字列のものと、再公開（use）の name。
#   単語（match_words）: ソースの各行に [words] の語を、大小文字を無視した単語単位で当てる
#     （前後が英数字と _ でない出現。コメント・文字列リテラルも数える）。
#   呼び出しの形（match_calls）: ソースの各行に [calls] の語を、大小文字を無視した部分一致で当てる。
#
#   公開フィールドの集合（match_fields）: tuple・unit の struct、struct でないものは集合が空でないので違反（形が違う）。
#
# 標準出力: JSON ごとの `scanned:`、ソースごとの `scanned-source:`、全部を検査し終えたとき（終了コード 0 と 1）だけ
#   締めの `record:`、合格時の `ok:`。標準エラー: `violation:`・`FAILED:`・`error:`。
#   `record: commit=<rev> documents=<n> public_items_total=<n> name_hits_total=<n> sources=<n> source_hits_total=<n>`
#   --exact-fields を渡したときだけ、見つかった struct ごとの `scanned-fields:` と、record の末尾に
#   `field_specs=<n> field_hits_total=<n>` が付く（違反は `violation: fields ...`）。
#   commit は JSON を作る経路では ROOT_DIR の HEAD（変更が在れば末尾に `-dirty`）、--json の経路では `-`。
#
# 検査しないもの: 語の一覧に当たらない名前、doc コメント・シグネチャの中の文字列、単語の一部として含まれるベンダー名、
#   ベンダー名を使わない分岐。`Arc` のような一般の識別子と同綴りのものにも当たる（意図した制約）。
# ホストの絶対パス・環境変数の値は出力に出さない。ソースを変更しない。Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def clean(text):
    """出力に出す値から制御文字や想定外の文字を落とす。"""
    return re.sub(r"[^A-Za-z0-9_.:/()-]", "?", str(text))[:80]


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def normalize(text):
    """名前の照合用に、小文字にして ASCII の英数字以外を除く。"""
    return re.sub(r"[^0-9a-z]", "", text.lower())


SECTIONS = ("names", "words", "calls")


def load_config(path):
    """語の一覧を読む。書式違反・欠落は Misuse。"""
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except (OSError, UnicodeDecodeError):
        raise Misuse("cannot read the config file")
    conf = {}
    cur = None
    for raw in lines:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        m = re.fullmatch(r"\[(.*)\]", line)
        if m:
            need(m.group(1) in SECTIONS, "unknown config section")
            need(m.group(1) not in conf, "duplicate config section")
            cur = m.group(1)
            conf[cur] = []
            continue
        need(cur is not None, "word before the first config section")
        need(line not in conf[cur], "duplicate word in a config section")
        conf[cur].append(line)
    for sec in SECTIONS:
        need(sec in conf, "missing config section")
        need(conf[sec], "empty config section")
    for w in conf["names"]:
        need(normalize(w) != "", "name word is empty after normalization")
    for w in conf["words"]:
        need(re.fullmatch(r"[A-Za-z0-9_]+", w) is not None, "malformed word in [words]")
    for w in conf["calls"]:
        need(not re.search(r"\s", w), "whitespace in [calls] word")
    return conf


def match_name(name, words):
    """名前に当たる [names] の語を返す（正規化したうえでの部分一致）。"""
    n = normalize(name)
    return [w for w in words if normalize(w) in n]


def match_words(line, patterns):
    """行に単語単位で当たる [words] の語を返す。"""
    return [w for w, rx in patterns if rx.search(line)]


def match_calls(line, calls):
    """行に呼び出しの形が出ているかを、大小文字を無視した部分一致で見て、当たる語を返す。"""
    low = line.lower()
    return [w for w in calls if w.lower() in low]


def inspect(doc, words):
    """JSON 1 本を検査して (crate 名, format_version, 公開アイテム数, 名前のあるアイテム数, 違反の一覧) を返す。"""
    need(isinstance(doc, dict), "top level is not an object")
    for k in ("index", "paths", "external_crates"):
        need(isinstance(doc.get(k), dict), "missing or malformed '%s'" % k)
    need("root" in doc, "missing 'root'")
    need(doc.get("includes_private") is False, "includes_private is not false")
    fv = doc.get("format_version")
    need(is_int(fv), "format_version is not an integer")
    root = doc["index"].get(str(doc["root"]))
    need(
        isinstance(root, dict) and root.get("crate_id") == 0 and is_int(root.get("crate_id"))
        and isinstance(root.get("inner"), dict) and isinstance(root.get("name"), str),
        "malformed root item")
    for it in doc["index"].values():
        need(isinstance(it, dict) and is_int(it.get("crate_id")), "index item lacks integer crate_id")

    public = 0
    named = 0
    hits = []
    for it in doc["index"].values():
        if it["crate_id"] != 0 or it.get("visibility") not in ("public", "default"):
            continue
        public += 1
        inner = it.get("inner")
        need(isinstance(inner, dict), "public item lacks 'inner'")
        names = []
        if isinstance(it.get("name"), str):
            names.append(it["name"])
        use = inner.get("use")
        if isinstance(use, dict) and isinstance(use.get("name"), str):
            names.append(use["name"])
        # 非 glob の再公開は name と use.name が同じ局所名になる。1 件の再公開を 2 回数えない。
        names = list(dict.fromkeys(names))
        if names:
            named += 1
        for nm in names:
            for w in match_name(nm, words):
                hits.append((clean(nm), clean(w)))
    need(public >= 1, "no public items found")
    return root["name"], fv, public, named, hits


def parse_field_spec(text):
    """`a::b::Name=f1,f2` を (モジュール経路のリスト, struct 名, フィールド名の集合) にする。書式違反は Misuse。"""
    m = re.fullmatch(r"((?:[A-Za-z0-9_]+::)+)([A-Za-z0-9_]+)=([A-Za-z0-9_]+(?:,[A-Za-z0-9_]+)*)", text)
    need(m is not None, "malformed --exact-fields value")
    mods = [x for x in m.group(1).split("::") if x]
    fields = m.group(3).split(",")
    need(len(set(fields)) == len(fields), "duplicate field in --exact-fields")
    return mods, m.group(2), fields


def match_fields(doc, spec):
    """spec に当たる struct を doc から探し、(見つかった数, 公開フィールド名の並び, 違反の一覧) を返す。"""
    mods, name, expected = spec
    want = mods + [name]
    found = 0
    seen = []
    problems = []
    for iid, ent in doc["paths"].items():
        if not (isinstance(ent, dict) and ent.get("crate_id") == 0 and ent.get("kind") == "struct"):
            continue
        path = ent.get("path")
        if not (isinstance(path, list) and path[1:] == want):
            continue
        found += 1
        it = doc["index"].get(str(iid))
        need(isinstance(it, dict), "struct listed in paths is missing from index")
        st = it.get("inner", {}).get("struct") if isinstance(it.get("inner"), dict) else None
        need(isinstance(st, dict), "malformed struct item")
        plain = st.get("kind", {}).get("plain") if isinstance(st.get("kind"), dict) else None
        if not isinstance(plain, dict):
            problems.append("not-a-plain-struct")
            continue
        ids = plain.get("fields")
        need(isinstance(ids, list), "malformed struct fields")
        names = []
        for fid in ids:
            f = doc["index"].get(str(fid))
            need(isinstance(f, dict) and isinstance(f.get("name"), str), "malformed field item")
            if f.get("visibility") in ("public", "default"):
                names.append(f["name"])
        seen = names
        for n in names:
            if n not in expected:
                problems.append("unexpected-field:" + clean(n))
        for n in expected:
            if n not in names:
                problems.append("missing-field:" + clean(n))
        if len(names) != len(expected):
            problems.append("field-count:%d" % len(names))
    return found, seen, problems


def _git(root, args):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
    try:
        done = subprocess.run(
            ["git"] + args, cwd=root, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if done.returncode != 0:
        return None
    return done.stdout.decode("utf-8", "replace")


def source_commit(root):
    """記録に載せる HEAD のコミット（変更が在れば -dirty）。取れなければ Misuse。"""
    rev = _git(root, ["rev-parse", "--verify", "HEAD^{commit}"])
    sha = rev.strip() if rev is not None else ""
    need(re.fullmatch(r"[0-9a-f]{40}([0-9a-f]{24})?", sha) is not None, "cannot resolve the source commit")
    st = _git(root, ["status", "--porcelain", "--untracked-files=normal", "--ignore-submodules=all"])
    need(st is not None, "cannot resolve the source commit")
    return sha + ("-dirty" if st.strip() else "")


def emit_docs(root, tmp):
    """public-api-check.sh --emit-json で JSON を作り、(crate, 組, パス) の一覧を返す。"""
    script = os.path.join(root, "scripts", "public-api-check.sh")
    try:
        done = subprocess.run(
            ["bash", script, "--emit-json", tmp], cwd=root, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=None, check=False)
    except OSError:
        raise Misuse("cannot run public-api-check.sh")
    need(done.returncode == 0, "public-api-check.sh --emit-json failed")
    out = []
    for line in done.stdout.decode("utf-8", "replace").splitlines():
        m = re.fullmatch(r"emitted: crate=(\S+) features=(\S+) file=([A-Za-z0-9_.-]+\.json)", line)
        if m:
            out.append((m.group(1), m.group(2), os.path.join(tmp, m.group(3))))
    need(out, "no JSON was emitted")
    return out


def load_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, UnicodeDecodeError, ValueError):
        raise Misuse("cannot read JSON input")


def display_path(root, path):
    real = os.path.realpath(path)
    base = os.path.realpath(root)
    if real.startswith(base + os.sep):
        return clean(os.path.relpath(real, base))
    return clean(os.path.basename(path))


def scan_source(root, path, conf, patterns):
    """ソース 1 本を走査して (表示名, 単語の一致数, 呼び出しの形の一致数, 違反の一覧) を返す。"""
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except (OSError, UnicodeDecodeError):
        raise Misuse("cannot read source input")
    disp = display_path(root, path)
    hits = []
    nw = nc = 0
    for i, line in enumerate(lines, 1):
        for w in match_words(line, patterns):
            nw += 1
            hits.append((disp, i, clean(w)))
        for w in match_calls(line, conf["calls"]):
            nc += 1
            hits.append((disp, i, clean(w)))
    return disp, nw, nc, hits


def run_docs(inputs, conf, total, specs):
    found_by_spec = [0] * len(specs)
    for _crate, label, path in inputs:
        doc = load_json(path)
        name, fv, public, named, hits = inspect(doc, conf["names"])
        print("scanned: crate=%s features=%s format_version=%d public_items=%d named_items=%d name_hits=%d"
              % (clean(name), label, fv, public, named, len(hits)))
        for item, word in hits:
            print("violation: crate=%s features=%s item=%s word=%s" % (clean(name), label, item, word),
                  file=sys.stderr)
        total["public"] += public
        total["names"] += len(hits)
        for i, spec in enumerate(specs):
            found, seen, problems = match_fields(doc, spec)
            if not found:
                continue
            found_by_spec[i] += found
            item = "::".join(spec[0] + [spec[1]])
            print("scanned-fields: crate=%s features=%s item=%s public_fields=%s"
                  % (clean(name), label, clean(item), ",".join(clean(n) for n in seen)))
            for pr in problems:
                print("violation: fields crate=%s features=%s item=%s problem=%s"
                      % (clean(name), label, clean(item), clean(pr)), file=sys.stderr)
            total["field_hits"] += len(problems)
    need(all(found_by_spec), "--exact-fields struct not found in any JSON")


def main():
    root = sys.argv[1]
    parser = argparse.ArgumentParser(prog="public-name-check.sh")
    parser.add_argument("--config", required=True, metavar="PATH")
    parser.add_argument("--json", nargs="+", metavar="PATH")
    parser.add_argument("--source", nargs="+", metavar="PATH")
    parser.add_argument("--exact-fields", action="append", default=[], metavar="SPEC")
    args = parser.parse_args(sys.argv[2:])

    conf = load_config(args.config)
    patterns = [(w, re.compile(r"(?<![A-Za-z0-9_])" + re.escape(w) + r"(?![A-Za-z0-9_])", re.IGNORECASE))
                for w in conf["words"]]
    specs = [parse_field_spec(t) for t in args.exact_fields]
    total = {"public": 0, "names": 0, "sources": 0, "src_hits": 0, "field_hits": 0}

    if args.json is not None:
        commit = "-"
        inputs = [("-", "-", p) for p in args.json]
        run_docs(inputs, conf, total, specs)
        ndocs = len(inputs)
    else:
        commit = source_commit(root)
        with tempfile.TemporaryDirectory() as tmp:
            inputs = emit_docs(root, tmp)
            run_docs(inputs, conf, total, specs)
        ndocs = len(inputs)

    for path in args.source or []:
        disp, nw, nc, hits = scan_source(root, path, conf, patterns)
        print("scanned-source: path=%s word_hits=%d call_hits=%d" % (disp, nw, nc))
        for d, line, word in hits:
            print("violation: source=%s line=%d word=%s" % (d, line, word), file=sys.stderr)
        total["sources"] += 1
        total["src_hits"] += nw + nc

    extra = ""
    if specs:
        extra = " field_specs=%d field_hits_total=%d" % (len(specs), total["field_hits"])
    print("record: commit=%s documents=%d public_items_total=%d name_hits_total=%d sources=%d source_hits_total=%d%s"
          % (commit, ndocs, total["public"], total["names"], total["sources"], total["src_hits"], extra))
    hits_all = total["names"] + total["src_hits"] + total["field_hits"]
    if hits_all:
        print("FAILED: %d hit(s)" % hits_all, file=sys.stderr)
        return 1
    print("ok: %d public API document(s) and %d source file(s) checked, no listed words"
          % (ndocs, total["sources"]))
    return 0


try:
    sys.exit(main())
except Misuse as e:
    print("error: %s" % e, file=sys.stderr)
    sys.exit(2)
except SystemExit:
    raise
except Exception as e:  # 想定外は違反(1)と読ませない
    print("error: unexpected %s" % type(e).__name__, file=sys.stderr)
    sys.exit(2)
PY
