#!/usr/bin/env bash
# feature `native-blocking-wait` を外した構成で、契約のコア（`crates/core` と `crates/contract`）の公開面に
# ブロッキング待ちの名前の公開アイテムが 0 件であることを、rustdoc の JSON から数えて確かめる
# （spec TASK-241・BUILD-29・BUILD-11・BUILD-12 のポインタ。親 Issue #465 の 2/6。内容の (1)）。
# あわせて、パイプライン生成の入力の形（spec BUILD-31 のポインタ。親 Issue #465 の 4/6。内容の (2)）も同じ JSON から確かめる。
# 呼び出し元: `make public-api-webgpu-check`（直接実行も可。CI にはまだ組み込まない）。回帰テストは `make test-public-api-webgpu-check`
# （`scripts/test-public-api-webgpu-check.sh`。合成 JSON と一時 workspace の実物の rustdoc の JSON を通す）。検査の枠（rustdoc の JSON を作る段）は
# `scripts/public-api-check.sh --emit-json <dir> --without-feature native-blocking-wait` を呼んで共有し、
# 抽出の実体と feature 列をここに持たない。
#
# 使い方:
#   scripts/public-api-webgpu-check.sh --config <path> [--json <path>...]
#   --config  語の一覧（必須。書式は public-api-webgpu-check.conf の先頭）。語はこのスクリプトに持たない。
#   --json    渡した rustdoc の JSON だけを検査する（cargo・git を呼ばない。組は `-` と表示）。無ければ JSON を作る。
# 終了コード: 0 一致が 0 件 / 1 一致が 1 件以上（違反） / 2 誤用・前提の不足・入力を読み切れない（成功扱いにしない）
#   2 になるもの: 引数の誤用、設定の欠落・書式違反、指定のファイルが無い・読めない、JSON の形の不一致
#   （public-api-check.sh の V1〜V3 と同じ形の検査）、JSON 1 本あたりの公開アイテムが 0 件、JSON を作る段の失敗
#   （core と contract の文書がそろわない場合を含む）、参照先が外部 crate または解決できない glob の再公開、想定外の例外。除外の印・対象を外す口は持たない。
#
# 照合の規則:
#   対象は crate_id が 0 で visibility が public / default のアイテム（public-api-check.sh の公開アイテムと同じ数え方）のうち、
#   inner が function のもの（自由関数・inherent と trait のメソッド）と再公開（use）。
#   名前を `_` で区切って小文字にした語のどれかが [blocking-words] の語に等しければ違反（部分一致では当てない）。
#
# 内容の (2) の照合の規則（対象は (1) と同じ組の JSON。cargo rustdoc は 1 回で済ませる）:
#   E1 crate_id 0・公開の `PipelineInput` という名前のアイテム（再公開は数えない）が、core の文書（root の名前が
#      `fandhe_silicon_core`）にちょうど 1 つあり、inner が enum で、attrs に non_exhaustive の印があること。
#      0 件・2 件以上・enum でない・印が無い・core 以外の文書にある定義は違反。
#   E2 その enum が `Wgsl` の variant を持つこと。
#   F1 公開の関数・メソッドのうち、名前を `_` で区切って小文字にした語に pipeline と生成の語（create / build / compile / load /
#      make / new）が 1 つずつ以上あるものは、引数の型か generics に `PipelineInput`（パスの最後の区切りで照合。参照・入れ子・
#      `impl Into<..>` の中を含む）を持つこと。持たない関数は違反（ネイティブの成果物だけを受け付ける口を作らない）。
#   形の崩れ（attrs・variants が並びでない、variant の id が解決できない、名前が当たった関数の sig が読めない）は終了コード 2。
#   `--json` の経路で core の文書が無いときは E1・E2 をかけず、`record:` の pipeline_input_checked=0 で明示する（F1 と
#   core 以外の文書にある定義の検出はかける）。既定の経路は core と contract がそろわなければ終了コード 2 なので常に 1。
#
# 標準出力: 文書ごとの `scanned:`、全部を検査し終えたとき（終了コード 0 と 1）だけ締めの `record:`、合格時の `ok:`。
#   標準エラー: `violation:`・`FAILED:`・`error:`。
#   `record: commit=<rev> documents=<n> public_items_total=<n> public_functions_total=<n> blocking_wait_names_total=<n>`
#   (2) の欄は末尾に足す: scanned に `pipeline_create_functions=<n> pipeline_input_violations=<n>`、
#   record に `pipeline_create_functions_total=<n> pipeline_input_checked=<0|1> pipeline_input_violations_total=<n>`、
#   ok の末尾に `, pipeline input ok`。(2) の違反の行は `violation: crate=<c> features=<f> check=pipeline-input reason=<..>` と
#   `violation: crate=<c> features=<f> item=<名前> kind=function check=pipeline-create reason=no-pipeline-input`。
#   FAILED の件数は (1) と (2) の合計。
#   commit は JSON を作る経路では ROOT_DIR の HEAD（変更が在れば末尾に `-dirty`）、--json の経路では `-`。
#
# 検査しないもの: 語を含まない名前のブロッキング待ち、完了の通知に非ブロッキングの形があるか、exec・upper の公開面、
#   production から 1 つ外した組以外の feature の組、doc コメント・シグネチャの中の文字列。(2) では、名前に pipeline と
#   生成の語の両方を含まない生成の口、再公開の名前、外部 crate の関数、variant の中身の形、`Wgsl` 以外の variant の有無、
#   生成の関数が実際に WGSL を受け付けるか（実行時の振る舞い）。
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

# 外す feature の名前（public-api-check.sh の production の列にある名前）と、検査する crate の package 名。
WITHOUT_FEATURE = "native-blocking-wait"
TARGET_PACKAGES = ("fandhe-silicon-core", "fandhe-silicon-contract")
SECTIONS = ("blocking-words",)
# 内容の (2) の照合の名前（構造の事実なので語の一覧の conf には持たない）。
CORE_CRATE = "fandhe_silicon_core"
PIPELINE_INPUT = "PipelineInput"
REQUIRED_VARIANT = "Wgsl"
PIPELINE_WORDS = ("pipeline",)
CREATE_WORDS = ("create", "build", "compile", "load", "make", "new")


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
        need(re.fullmatch(r"[a-z0-9]+", line) is not None, "malformed word in a config section")
        need(line not in conf[cur], "duplicate word in a config section")
        conf[cur].append(line)
    for sec in SECTIONS:
        need(sec in conf, "missing config section")
        need(conf[sec], "empty config section")
    return conf


def match_words(name, words):
    """名前を `_` で区切って小文字にした語のうち、[blocking-words] に等しいものを返す。"""
    parts = name.lower().split("_")
    return [w for w in words if w in parts]


def mentions_pipeline_input(node):
    """型・generics の入れ子の中に、パスの最後の区切りが PipelineInput のものがあるか。"""
    if isinstance(node, dict):
        p = node.get("path")
        if isinstance(p, str) and p.split("::")[-1] == PIPELINE_INPUT:
            return True
        return any(mentions_pipeline_input(v) for v in node.values())
    if isinstance(node, list):
        return any(mentions_pipeline_input(v) for v in node)
    return False


def is_pipeline_create(name):
    parts = name.lower().split("_")
    return any(w in parts for w in PIPELINE_WORDS) and any(w in parts for w in CREATE_WORDS)


def has_non_exhaustive(attrs):
    for a in attrs:
        vals = [a] if isinstance(a, str) else list(a.values()) if isinstance(a, dict) else []
        for v in vals:
            if isinstance(v, str) and v.strip() in ("non_exhaustive", "#[non_exhaustive]"):
                return True
    return False


def evaluate_pipeline_enum(doc, item):
    """core の文書にある PipelineInput の定義 1 件を E1・E2 で判定し、違反の reason の一覧を返す。"""
    inner = item["inner"]
    if "enum" not in inner:
        return ["not-enum"]
    reasons = []
    attrs = item.get("attrs")
    need(isinstance(attrs, list), "malformed attrs on PipelineInput")
    if not has_non_exhaustive(attrs):
        reasons.append("not-non-exhaustive")
    en = inner["enum"]
    need(isinstance(en, dict) and isinstance(en.get("variants"), list), "malformed variants on PipelineInput")
    names = []
    for vid in en["variants"]:
        v = doc["index"].get(str(vid))
        need(
            isinstance(v, dict) and isinstance(v.get("inner"), dict) and "variant" in v["inner"],
            "unresolved variant id on PipelineInput")
        names.append(v.get("name"))
    if REQUIRED_VARIANT not in names:
        reasons.append("no-wgsl")
    return reasons


def inspect(doc, words):
    """JSON 1 本を検査して (crate 名, format_version, 公開アイテム数, 公開の関数数, 違反の一覧,
    PipelineInput の定義の一覧, パイプライン生成の関数の (名前, 入力を持つか) の一覧) を返す。"""
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
    functions = 0
    hits = []
    pdefs = []
    pfuncs = []
    for it in doc["index"].values():
        if it["crate_id"] != 0 or it.get("visibility") not in ("public", "default"):
            continue
        public += 1
        inner = it.get("inner")
        need(isinstance(inner, dict), "public item lacks 'inner'")
        names = []
        if isinstance(it.get("name"), str):
            names.append(it["name"])
        kind = None
        if it.get("name") == PIPELINE_INPUT and not any(k in inner for k in ("use", "variant", "function", "struct_field")):
            pdefs.append(it)
        if "function" in inner:
            kind = "function"
            functions += 1
            nm = it.get("name")
            if isinstance(nm, str) and is_pipeline_create(nm):
                fn = inner["function"]
                sig = fn.get("sig") if isinstance(fn, dict) else None
                need(
                    isinstance(sig, dict) and isinstance(sig.get("inputs"), list),
                    "pipeline-creating function lacks a valid sig")
                ok = mentions_pipeline_input(sig["inputs"]) or mentions_pipeline_input(fn.get("generics"))
                pfuncs.append((clean(nm), ok))
        elif isinstance(inner.get("use"), dict):
            kind = "use"
            if isinstance(inner["use"].get("name"), str):
                names.append(inner["use"]["name"])
            if inner["use"].get("is_glob") is True:
                # glob の再公開は use.name が `*` で名前を照合できない。参照先が同じ crate の中なら
                # その中のアイテムを直接数えるので足りる。外部 crate や解決できない参照先は、
                # 禁止名の公開を見落とさないよう成功扱いにせず止める（fail-closed）。
                target = doc["index"].get(str(inner["use"].get("id")))
                need(
                    isinstance(target, dict) and target.get("crate_id") == 0,
                    "glob re-export target is external or unresolved")
        if kind is None:
            continue
        # 非 glob の再公開は name と use.name が同じ局所名になる。1 件の再公開を 2 回数えない。
        for nm in dict.fromkeys(names):
            for w in match_words(nm, words):
                hits.append((clean(nm), kind, clean(w)))
    need(public >= 1, "no public items found")
    return root["name"], fv, public, functions, hits, pdefs, pfuncs


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
    """public-api-check.sh --emit-json --without-feature で JSON を作り、対象 crate の (crate, 組, パス) を返す。"""
    script = os.path.join(root, "scripts", "public-api-check.sh")
    try:
        done = subprocess.run(
            ["bash", script, "--emit-json", tmp, "--without-feature", WITHOUT_FEATURE],
            cwd=root, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=None, check=False)
    except OSError:
        raise Misuse("cannot run public-api-check.sh")
    need(done.returncode == 0, "public-api-check.sh --emit-json failed")
    found = {}
    for line in done.stdout.decode("utf-8", "replace").splitlines():
        m = re.fullmatch(r"emitted: crate=(\S+) features=(\S+) file=([A-Za-z0-9_.-]+\.json)", line)
        if m and m.group(1) in TARGET_PACKAGES:
            need(m.group(1) not in found, "duplicate emitted document")
            found[m.group(1)] = (m.group(1), m.group(2), os.path.join(tmp, m.group(3)))
    # 片方でも欠けたまま合格にしない（fail-closed）。
    need(all(p in found for p in TARGET_PACKAGES), "core and contract documents were not both emitted")
    return [found[p] for p in TARGET_PACKAGES]


def load_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, UnicodeDecodeError, ValueError):
        raise Misuse("cannot read JSON input")


def run_docs(inputs, words):
    total = {"public": 0, "functions": 0, "hits": 0, "pcreate": 0, "pchecked": 0, "pviol": 0}
    results = []
    for _crate, label, path in inputs:
        doc = load_json(path)
        name, fv, public, functions, hits, pdefs, pfuncs = inspect(doc, words)
        # (2) の違反を、この文書に属するものとして集める（(行に出す属性, 理由) の並び）。
        pv = []
        for fname, ok in pfuncs:
            if not ok:
                pv.append(("pipeline-create", fname, "no-pipeline-input"))
        if name == CORE_CRATE:
            total["pchecked"] = 1
            if not pdefs:
                pv.append(("pipeline-input", None, "missing"))
            elif len(pdefs) > 1:
                pv.append(("pipeline-input", None, "duplicate"))
            else:
                for reason in evaluate_pipeline_enum(doc, pdefs[0]):
                    pv.append(("pipeline-input", None, reason))
        elif pdefs:
            pv.append(("pipeline-input", None, "outside-core"))
        results.append((name, label, fv, public, functions, hits, len(pfuncs), pv))
    for name, label, fv, public, functions, hits, pcreate, pv in results:
        print("scanned: crate=%s features=%s format_version=%d public_items=%d public_functions=%d blocking_wait_names=%d"
              " pipeline_create_functions=%d pipeline_input_violations=%d"
              % (clean(name), clean(label), fv, public, functions, len(hits), pcreate, len(pv)))
        for item, kind, word in hits:
            print("violation: crate=%s features=%s item=%s kind=%s word=%s"
                  % (clean(name), clean(label), item, kind, word), file=sys.stderr)
        for check, item, reason in pv:
            if item is None:
                print("violation: crate=%s features=%s check=%s reason=%s"
                      % (clean(name), clean(label), check, reason), file=sys.stderr)
            else:
                print("violation: crate=%s features=%s item=%s kind=function check=%s reason=%s"
                      % (clean(name), clean(label), item, check, reason), file=sys.stderr)
        total["public"] += public
        total["functions"] += functions
        total["hits"] += len(hits)
        total["pcreate"] += pcreate
        total["pviol"] += len(pv)
    return total


def main():
    root = sys.argv[1]
    parser = argparse.ArgumentParser(prog="public-api-webgpu-check.sh")
    parser.add_argument("--config", required=True, metavar="PATH")
    parser.add_argument("--json", nargs="+", metavar="PATH")
    args = parser.parse_args(sys.argv[2:])

    words = load_config(args.config)["blocking-words"]
    if args.json is not None:
        commit = "-"
        inputs = [("-", "-", p) for p in args.json]
        total = run_docs(inputs, words)
    else:
        commit = source_commit(root)
        with tempfile.TemporaryDirectory() as tmp:
            inputs = emit_docs(root, tmp)
            total = run_docs(inputs, words)

    print("record: commit=%s documents=%d public_items_total=%d public_functions_total=%d blocking_wait_names_total=%d"
          " pipeline_create_functions_total=%d pipeline_input_checked=%d pipeline_input_violations_total=%d"
          % (commit, len(inputs), total["public"], total["functions"], total["hits"],
              total["pcreate"], total["pchecked"], total["pviol"]))
    if total["hits"] or total["pviol"]:
        print("FAILED: %d item(s)" % (total["hits"] + total["pviol"]), file=sys.stderr)
        return 1
    print("ok: %d public API document(s) checked, no blocking-wait names, pipeline input ok" % len(inputs))
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
