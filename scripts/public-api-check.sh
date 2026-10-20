#!/usr/bin/env bash
# 製品の 4 crate（`--manifest-path` で指定した別 workspace の lib crate も可）の公開 API に wgpu 系（wgpu*・naga）の crate の型・経路が出ていないことを、
# rustdoc の JSON から機械で数えて確かめる（spec TASK-10・BUILD-47 のポインタ。親 Issue #363 の 2/6・3/6・5/6）。
# 呼び出し元: `make public-api-check`（直接実行も可）。CI の `deny` ジョブも呼ぶ（`scanned:` / `record:` の行がジョブのログに残る）。
# 回帰テストは `make test-public-api-check`（`scripts/test-public-api-check.sh`。合成 JSON と実物の rustdoc の JSON を通す）。
#
# 使い方:
#   scripts/public-api-check.sh                         workspace の lib crate の JSON を作って検査する
#   scripts/public-api-check.sh --json <path> [<path>…]  渡した JSON だけを検査する（cargo を呼ばない）
#   scripts/public-api-check.sh --emit-json <dir>        JSON を作って <dir>/{lib}.{組}.json へ写すだけで検査しない（`emitted:` の行。
#                                                        scripts/public-name-check.sh が抽出を共有するための口。--json とは併用できない）
#   scripts/public-api-check.sh --emit-json <dir> --without-feature <名前>
#                                                        契約の crate の production の feature 列から <名前> だけを除いた組（ラベル
#                                                        `without-<名前>`。{lib}.without-<名前>.json）を、lib crate ごとに 1 組だけ作る
#                                                        opt-in の口（他の crate は既定の feature）。scripts/public-api-webgpu-check.sh
#                                                        （TASK-241）が使う。--emit-json が要る。--json・--manifest-path とは併用できず、
#                                                        2 回の指定・production の列に無い名前・契約の crate の無い workspace は終了コード 2
#   scripts/public-api-check.sh --manifest-path <ROOT_DIR 配下の Cargo.toml>
#                                                        別 workspace（比較基準の baseline-wgpu/ など）の lib crate に同じ検査を当てる。
#                                                        --json・--emit-json とは併用できない。`make public-api-check` は製品の workspace だけ。
#                                                        production の feature 列は contract の package 名で当たるため、別 workspace は既定の feature
# 終了コード: 0 参照が 0 件 / 1 参照が 1 件以上（違反） / 2 誤用・前提の不足・JSON を読み切れない（成功扱いにしない）
# 標準出力: 組ごとの `scanned:` の行（走査した公開アイテムの数）、全部の文書を検査し終えたとき（終了コード 0 と 1）だけ
#   最後の `scanned:` の後に 1 回出す締めの `record:` の行、合格時の `ok:` の 1 行。標準エラー: `violation:`・`FAILED:`・`error:`。
#   `record: commit=<rev> documents=<n> public_items_total=<n> wgpu_type_refs_total=<n> wgpu_other_refs_total=<n>`
#   documents は検査した文書の数、*_total は `scanned:` の同名の値の合計。終了コード 2 では出さない（無ければ記録は不完全）。
#   commit は JSON を作る経路では ROOT_DIR の HEAD のコミット（変更が在れば末尾に `-dirty`）、--json の経路では `-`。
#   件数は出すだけで合否に使わない（公開アイテムが 1 件以上、は除く）。記録は標準出力だけに残し、ファイルは作らない。
#
# JSON を作る段（rust-toolchain.toml の toolchain のまま動かす）:
#   先に HEAD のコミットを取る（GIT_OPTIONAL_LOCKS=0。取れなければ終了コード 2。cargo を流す前に止める）。
#   cargo metadata で workspace の lib crate を列挙し、crate ごとに 2 組（production / all-features）で
#   `cargo rustdoc --lib -- -Z unstable-options --output-format json` を流す。検査の経路（引数なし・--manifest-path）だけ
#   `--document-hidden-items` を足し、#[doc(hidden)] の公開アイテム（型を消したハンドルの読む口など。TASK-136 のポインタ）
#   も走査に入れる（既定の JSON には入らず、署名に wgpu 系の型が出ても見逃すため）。`--emit-json` の経路は足さず、
#   emit を共有する他の検査の入力は変えない。includes_private は false のまま（V1 が見る）。
#   production は contract だけ本番の feature 列を明示し、ほかは既定の feature。名前は crates/contract/Cargo.toml のもの。
#   環境変数 RUSTC_BOOTSTRAP=1 は、この cargo rustdoc とその事前確認の子プロセスにだけ渡す
#   （他の cargo の呼び出し・make・CI には広げない）。依存の crate も同じ呼び出しの中で
#   その環境変数の下でコンパイルされるため、成果物は cargo metadata の target_directory 配下の
#   専用ディレクトリ（public-api-check）に分け、製品のビルドと共有しない。
#   流す前に古い JSON を消し、流した後に在ることを確かめ、組ごとの専用パス（{lib}.{組}.json）へ移す
#   （組が同じ出力パスを共有して上書きし合わないため）。target は native だけ。
#
# 数える段の規則（外れたら終了コード 2。違反は 1）:
#   V1 トップが object で index・paths・external_crates が object、root が在り、includes_private が false、
#      format_version が整数
#   V2 index[root] が crate_id 0・inner が object・name が文字列のアイテム
#   V3 index の全アイテムが object で、整数の crate_id を持つ
#   V4 crate_id が 0 以外のアイテムは external_crates で crate 名に解決し、wgpu 系なら other に 1 件数える
#   V5 crate_id が 0 で visibility が public / default のアイテムを公開アイテムとして数え、inner を再帰でたどって
#      id の鍵を持つ object を全部拾う（id は整数。use の直下の id だけ null を許す）。
#      親の鍵が resolved_path なら type、それ以外は other
#   V6 拾った id は paths、無ければ index の crate_id で解決し、0 以外なら external_crates で名前にして wgpu 系を数える
#   V6b inner の中の並びの整数も参照として V6 と同じに解決する（並びの null は親の鍵が tuple のときだけ許す）
#   V7 公開アイテムが 1 件以上
#   wgpu 系の名前: `-` を `_` にそろえ、wgpu で始まる、または naga に等しい、または naga_ で始まる
#   （wgpu は接頭辞だけで当て、naga は区切りを要る形にしている。非対称は意図どおり）。
#   wgpu_other_refs は同じ外部アイテムを V4 と V6b の両方で数えることがある（合否には影響しない）。
#   format_version の許可リストは持たない。形が読めなくなったら V1〜V7 が終了コード 2 にする。
#
# 検査しないもの: 件数の増減（数は出すだけ）、wasm32 の公開 API、依存グラフ（make deny の役割）、陽性対照、wgpu 系以外の名前。
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
import shutil
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# production の組で contract に渡す feature 列（backend-dx12-compute・テスト／診断専用は含めない）。
CONTRACT_PKG = "fandhe-silicon-contract"
CONTRACT_PRODUCTION = (
    "contract-core,cpu-isa,cpu-ops,native-blocking-wait,graphics,ext-interop,"
    "backend-metal,backend-vulkan,backend-cuda"
)
TARGET_SUBDIR = "public-api-check"
OTHER_LIB_KINDS = {"proc-macro", "dylib", "cdylib", "staticlib", "rlib"}


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def clean(text):
    """出力に出す名前から制御文字や想定外の文字を落とす。"""
    return re.sub(r"[^A-Za-z0-9_.:-]", "?", str(text))[:80]


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def is_wgpu(name):
    n = name.replace("-", "_")
    return n.startswith("wgpu") or n == "naga" or n.startswith("naga_")


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def crate_name(doc, cid):
    ext = doc["external_crates"].get(str(cid))
    need(isinstance(ext, dict) and isinstance(ext.get("name"), str), "unresolved external crate id")
    return ext["name"]


def resolve(doc, ref):
    """参照 id を crate 名に解決する。内部参照なら None。"""
    key = str(ref)
    if key in doc["paths"]:
        entry = doc["paths"][key]
        need(isinstance(entry, dict), "malformed paths entry")
        cid = entry.get("crate_id")
    elif key in doc["index"]:
        cid = doc["index"][key].get("crate_id")
    else:
        raise Misuse("unresolved reference id")
    need(is_int(cid), "crate_id is not an integer")
    if cid == 0:
        return None
    return crate_name(doc, cid)


def inspect(doc):
    """JSON 1 本を検査して (crate 名, format_version, 公開アイテム数, type 数, other 数, 違反の一覧) を返す。"""
    need(isinstance(doc, dict), "top level is not an object")
    for k in ("index", "paths", "external_crates"):
        need(isinstance(doc.get(k), dict), "missing or malformed '%s'" % k)
    need("root" in doc, "missing 'root'")
    need(doc.get("includes_private") is False, "includes_private is not false")
    fv = doc.get("format_version")
    need(is_int(fv), "format_version is not an integer")
    root = doc["index"].get(str(doc["root"]))
    root_ok = (
        isinstance(root, dict)
        and root.get("crate_id") == 0
        and not isinstance(root.get("crate_id"), bool)
        and isinstance(root.get("inner"), dict)
        and isinstance(root.get("name"), str)
    )
    need(root_ok, "malformed root item")

    items = doc["index"]
    for it in items.values():
        need(isinstance(it, dict) and is_int(it.get("crate_id")), "index item lacks integer crate_id")

    violations = []
    n_type = 0
    n_other = 0
    public = 0

    def hit(item, kind, crate):
        nonlocal n_type, n_other
        if kind == "type":
            n_type += 1
        else:
            n_other += 1
        violations.append((clean(item), kind, clean(crate)))

    def walk(node, parent, item, direct_use):
        if isinstance(node, dict):
            if "id" in node:
                rid = node["id"]
                if rid is None and direct_use:
                    pass
                else:
                    need(is_int(rid), "reference id is not an integer")
                    crate = resolve(doc, rid)
                    if crate is not None and is_wgpu(crate):
                        hit(item, "type" if parent == "resolved_path" else "other", crate)
            for k, v in node.items():
                if k != "id":
                    walk(v, k, item, False)
        elif isinstance(node, list):
            for el in node:
                if el is None:
                    need(parent == "tuple", "null in a list outside 'tuple'")
                elif is_int(el):
                    crate = resolve(doc, el)
                    if crate is not None and is_wgpu(crate):
                        hit(item, "other", crate)
                else:
                    walk(el, parent, item, False)

    for key, it in items.items():
        label = it.get("name") if isinstance(it.get("name"), str) else key
        if it["crate_id"] != 0:
            crate = crate_name(doc, it["crate_id"])
            if is_wgpu(crate):
                hit(label, "other", crate)
            continue
        if it.get("visibility") not in ("public", "default"):
            continue
        public += 1
        inner = it.get("inner")
        need(isinstance(inner, dict), "public item lacks 'inner'")
        for kind, body in inner.items():
            walk(body, kind, label, kind == "use")
    need(public >= 1, "no public items found")
    return root["name"], fv, public, n_type, n_other, violations


def _git(root, args):
    """記録用に外部コマンドを引数の並びで起動し、成功時の標準出力（str）を返す。失敗は None。"""
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
    """記録に載せる、作業ツリーの HEAD のコミット（変更が在れば -dirty）を返す。取れなければ Misuse。"""
    rev = _git(root, ["rev-parse", "--verify", "HEAD^{commit}"])
    sha = rev.strip() if rev is not None else ""
    need(re.fullmatch(r"[0-9a-f]{40}([0-9a-f]{24})?", sha) is not None, "cannot resolve the source commit")
    st = _git(root, ["status", "--porcelain", "--untracked-files=normal", "--ignore-submodules=all"])
    need(st is not None, "cannot resolve the source commit")
    return sha + ("-dirty" if st.strip() else "")


def resolve_manifest(root, value):
    """--manifest-path の値を実パスにして返す。root 配下の Cargo.toml（通常ファイル）以外は Misuse。"""
    real_root = os.path.realpath(root)
    path = os.path.realpath(value)
    ok = (
        os.path.basename(path) == "Cargo.toml"
        and os.path.isfile(path)
        and os.path.commonpath([real_root, path]) == real_root
    )
    need(ok, "--manifest-path must be a Cargo.toml inside the repository")
    return path


def build_docs(root, manifest=None, without=None, hidden=False):
    """workspace の lib crate ごとに 2 組の rustdoc JSON を作って (crate, 組, パス) を返す。

    manifest が在れば、その Cargo.toml の workspace を対象にする（cwd は root のまま）。
    without が在れば、既定の 2 組の代わりに「契約の production の feature 列から without を除いた」1 組だけを作る
    （他の crate は既定の feature。組のラベルは `without-<名前>`）。
    hidden が真なら rustdoc に --document-hidden-items を渡し、#[doc(hidden)] の公開アイテム（型を消したハンドルの
    読む口など）も JSON に入れる。検査の経路だけが真で、--emit-json の経路（他の検査と共有）は偽のまま（TASK-136 のポインタ）。
    """
    mpath = ["--manifest-path", manifest] if manifest else []
    env = dict(os.environ, RUSTC_BOOTSTRAP="1")

    def run(cmd, with_env=None, to_stderr=False):
        try:
            return subprocess.run(
                cmd, cwd=root, env=with_env, stdin=subprocess.DEVNULL,
                stdout=(2 if to_stderr else subprocess.PIPE),
                stderr=(None if to_stderr else subprocess.DEVNULL), check=False)
        except OSError:
            raise Misuse("cannot run %s" % cmd[0])

    need(run(["cargo", "--version"]).returncode == 0, "cargo is not usable")
    meta = run(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"] + mpath)
    need(meta.returncode == 0, "cargo metadata failed")
    try:
        md = json.loads(meta.stdout)
    except ValueError:
        raise Misuse("cargo metadata output is not JSON")
    members = set(md["workspace_members"])
    targets = []
    for pkg in sorted(md["packages"], key=lambda p: p["name"]):
        if pkg["id"] not in members:
            continue
        libs = [t for t in pkg["targets"] if "lib" in t["kind"]]
        others = [t for t in pkg["targets"] if "lib" not in t["kind"] and OTHER_LIB_KINDS & set(t["kind"])]
        need(not others, "unsupported library target kind in a workspace member")
        if libs:
            targets.append((pkg["name"], libs[0]["name"].replace("-", "_")))
    need(targets, "no library crates found in the workspace")
    if without is not None:
        # 引いた結果が実際に効くことを保証するため、契約の crate が無い workspace は通さない。
        need(any(n == CONTRACT_PKG for n, _l in targets), "contract package is not in the workspace")

    probe = run(["rustdoc", "-Z", "unstable-options", "--version"], with_env=env)
    need(probe.returncode == 0, "rustdoc cannot emit JSON output")
    tdir = os.path.join(md["target_directory"], TARGET_SUBDIR)
    out = []
    for name, lib in targets:
        production = []
        if name == CONTRACT_PKG:
            production = ["--no-default-features", "--features", CONTRACT_PRODUCTION]
        sets = [("production", production), ("all-features", ["--all-features"])]
        if without is not None:
            rest = ",".join(f for f in CONTRACT_PRODUCTION.split(",") if f != without)
            feats = ["--no-default-features", "--features", rest] if name == CONTRACT_PKG else []
            sets = [("without-" + without, feats)]
        for label, feats in sets:
            path = os.path.join(tdir, "doc", lib + ".json")
            if os.path.exists(path):
                os.remove(path)
            cmd = ["cargo", "rustdoc", "--locked", "--target-dir", tdir, "-p", name]
            cmd += feats
            cmd += mpath
            cmd += ["--lib", "--", "-Z", "unstable-options", "--output-format", "json"]
            if hidden:
                cmd.append("--document-hidden-items")
            done = run(cmd, with_env=env, to_stderr=True)
            need(done.returncode == 0, "cargo rustdoc failed for %s (%s)" % (clean(name), label))
            need(os.path.isfile(path), "rustdoc JSON was not produced for %s" % clean(name))
            # 次の組が同じパスへ出力して上書きするため、組ごとの専用パスへ移して保持する。
            kept = os.path.join(tdir, "doc", "%s.%s.json" % (lib, label))
            os.replace(path, kept)
            out.append((name, label, kept))
    return out


def load(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, UnicodeDecodeError, ValueError):
        raise Misuse("cannot read JSON input")


def main():
    root = sys.argv[1]
    parser = argparse.ArgumentParser(prog="public-api-check.sh")
    parser.add_argument("--json", nargs="+", metavar="PATH")
    parser.add_argument("--emit-json", metavar="DIR")
    parser.add_argument("--manifest-path", metavar="PATH")
    parser.add_argument("--without-feature", action="append", metavar="NAME")
    args = parser.parse_args(sys.argv[2:])
    without = None
    if args.without_feature is not None:
        need(len(args.without_feature) == 1, "--without-feature can be given only once")
        without = args.without_feature[0]
        need(args.emit_json is not None, "--without-feature requires --emit-json")
        need(args.json is None, "--without-feature cannot be combined with --json")
        need(args.manifest_path is None, "--without-feature cannot be combined with --manifest-path")
        need(without in CONTRACT_PRODUCTION.split(","), "--without-feature name is not in the production feature list")
    manifest = None
    if args.manifest_path is not None:
        need(args.json is None, "--manifest-path cannot be combined with --json")
        need(args.emit_json is None, "--manifest-path cannot be combined with --emit-json")
        manifest = resolve_manifest(root, args.manifest_path)
    if args.emit_json is not None:
        need(args.json is None, "--emit-json cannot be combined with --json")
        need(os.path.isdir(args.emit_json), "--emit-json directory does not exist")
        for crate, label, path in build_docs(root, None, without):
            fname = os.path.basename(path)
            # 別の FS でも動くよう、移動ではなく複写にする。
            shutil.copyfile(path, os.path.join(args.emit_json, fname))
            print("emitted: crate=%s features=%s file=%s" % (clean(crate), label, clean(fname)))
        return 0
    if args.json is not None:
        commit = "-"
        inputs = [("-", "-", p) for p in args.json]
    else:
        commit = source_commit(root)
        inputs = build_docs(root, manifest, hidden=True)

    total = 0
    sum_public = sum_type = sum_other = 0
    for _crate, label, path in inputs:
        name, fv, public, n_type, n_other, violations = inspect(load(path))
        print("scanned: crate=%s features=%s format_version=%d public_items=%d wgpu_type_refs=%d wgpu_other_refs=%d"
              % (clean(name), label, fv, public, n_type, n_other))
        for item, kind, crate in violations:
            print("violation: crate=%s features=%s item=%s kind=%s references=%s"
                  % (clean(name), label, item, kind, crate), file=sys.stderr)
        total += n_type + n_other
        sum_public += public
        sum_type += n_type
        sum_other += n_other
    print("record: commit=%s documents=%d public_items_total=%d wgpu_type_refs_total=%d wgpu_other_refs_total=%d"
          % (commit, len(inputs), sum_public, sum_type, sum_other))
    if total:
        print("FAILED: %d reference(s)" % total, file=sys.stderr)
        return 1
    print("ok: %d public API document(s) checked, no wgpu-family references" % len(inputs))
    return 0


sys.setrecursionlimit(20000)
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
