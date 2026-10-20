#!/usr/bin/env bash
# feature `native-blocking-wait` を外した構成で、契約のコア（`crates/core` と `crates/contract`）の公開面に
# ブロッキング待ちの名前の公開アイテムが 0 件であることを、rustdoc の JSON から数えて確かめる
# （spec TASK-241・BUILD-29・BUILD-11・BUILD-12 のポインタ。親 Issue #465 の 2/6。内容の (1)）。
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
# 標準出力: 文書ごとの `scanned:`、全部を検査し終えたとき（終了コード 0 と 1）だけ締めの `record:`、合格時の `ok:`。
#   標準エラー: `violation:`・`FAILED:`・`error:`。
#   `record: commit=<rev> documents=<n> public_items_total=<n> public_functions_total=<n> blocking_wait_names_total=<n>`
#   commit は JSON を作る経路では ROOT_DIR の HEAD（変更が在れば末尾に `-dirty`）、--json の経路では `-`。
#
# 検査しないもの: 語を含まない名前のブロッキング待ち、完了の通知に非ブロッキングの形があるか、exec・upper の公開面、
#   production から 1 つ外した組以外の feature の組、doc コメント・シグネチャの中の文字列。
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


def inspect(doc, words):
    """JSON 1 本を検査して (crate 名, format_version, 公開アイテム数, 公開の関数数, 違反の一覧) を返す。"""
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
        if "function" in inner:
            kind = "function"
            functions += 1
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
    return root["name"], fv, public, functions, hits


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
    total = {"public": 0, "functions": 0, "hits": 0}
    for _crate, label, path in inputs:
        name, fv, public, functions, hits = inspect(load_json(path), words)
        print("scanned: crate=%s features=%s format_version=%d public_items=%d public_functions=%d blocking_wait_names=%d"
              % (clean(name), clean(label), fv, public, functions, len(hits)))
        for item, kind, word in hits:
            print("violation: crate=%s features=%s item=%s kind=%s word=%s"
                  % (clean(name), clean(label), item, kind, word), file=sys.stderr)
        total["public"] += public
        total["functions"] += functions
        total["hits"] += len(hits)
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
          % (commit, len(inputs), total["public"], total["functions"], total["hits"]))
    if total["hits"]:
        print("FAILED: %d item(s)" % total["hits"], file=sys.stderr)
        return 1
    print("ok: %d public API document(s) checked, no blocking-wait names" % len(inputs))
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
