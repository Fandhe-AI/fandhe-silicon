#!/usr/bin/env bash
# 契約の crate（fandhe-silicon-contract）をプロファイルごとの feature の組にしたときの依存グラフに、
# 判定に使う crate 名が載っていないことを数えて確かめる（spec TASK-228・BUILD-1・BUILD-2 のポインタ）。
# ComputeOnly（Metal）の組では、`objc2-metal` の per-header feature も `cargo tree -e features` で数える。
# 呼び出し元: `make profile-dep-graph-check`。直接実行も可。`make deny`・CI からは呼ばない。
#
# 使い方:
#   scripts/profile-dep-graph-check.sh                                   cargo tree を流して数える
#   scripts/profile-dep-graph-check.sh --tree-text <path> --profile <name> [--features-text <path>]
#                                                                        渡した tree の出力だけを数える（cargo・git を呼ばない。
#                                                                        --tree-text と --profile は組でだけ受け付ける。per-header feature を
#                                                                        数えるプロファイルは --features-text が必須、それ以外は受け付けない）
# 終了コード: 0 全プロファイルで 0 件 / 1 1 件以上（違反）
#   2 誤用・前提の不足（python3・cargo・git が無い、cargo の失敗、コミットが取れない）・読めない入力・
#     数えた crate が 0 件または検査対象の crate 自身が tree に無い（成功扱いにしない）
# 標準出力: `scanned:`、プロファイルごとの `record:`、判定の名前ごとの `count:`（0 件でも出す）、合格時の `ok:`。
#   `record: commit=<rev|-> profile=<p> crates_total=<n> judged_total=<n>`
#   per-header feature を数えるプロファイルは、record の末尾に `header_crate_n` `features_judged_total` `features_recorded_total` が付き、
#   feature ごとの `count: profile=<p> feature=<名前> judged=<yes|no> n=<k>` も出る（judged=no は数えるだけで落とさない）。
#   commit は cargo を流す経路では HEAD のコミット（変更が在れば末尾に `-dirty`）、--tree-text の経路では `-`。
# 標準エラー: `violation:`・`FAILED:`（crate / feature ごと）・`error:`。
#
# 数え方: プロファイル・feature の組・依存の種類・target・名前の一覧は、Python 部の先頭の
#   「数え方の定義」ブロックにだけ置く（値をここへ書き写さない）。出力の読み方は docs/profile-dep-graph-check.md。
#   数えるのは一意の package（名前, 版）。
# ホストの絶対パス・環境変数の値は出力に出さない。ソースを変更しない。Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
# シェルは入力の値に触れない。値の扱いはすべて Python の中で行う。
exec python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import argparse
import os
import re
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# ==== 数え方の定義（数え方を変えるときはここだけを変える） ====
# 名前の一覧の決定は private spec 側の記録に従う（TASK-228・BUILD-1・BUILD-2 のポインタのみ）。
# コマンドの組み立て・走査・判定はこのブロックの値から作るので、値はこのブロックの外に書かない。

# 検査する crate（cargo の -p に渡す package 名）。
PACKAGE = "fandhe-silicon-contract"
# 数える依存の種類と target（"all" か target triple）。
DEP_KINDS = ("normal", "build", "dev")
TARGET = "all"
# 判定に使う名前（GPU の SDK・ローダ / 描画・surface）。
SDK_LOADER_NAMES = ("objc2-metal", "ash", "cudarc", "libloading")
SURFACE_NAMES = ("objc2-quartz-core", "raw-window-handle", "winit")
# 名前の比べ方: 小文字にしたあとに適用する置換。完全一致で比べる（接頭辞には広げない）。
NAME_REPLACE = (("_", "-"),)
# per-header feature を数える package（cargo の package 名）。
HEADER_CRATE = "objc2-metal"
# per-header feature のうち、1 件でもあれば違反にするもの / 数えて記録するだけのもの（大文字と小文字を区別した完全一致）。
RENDER_HEADER_FEATURES = ("MTLRenderPipeline", "MTLRenderCommandEncoder", "MTLRenderPass", "MTLTexture", "MTLDrawable")
RECORD_ONLY_HEADER_FEATURES = ("MTLDepthStencil", "MTLBlitCommandEncoder", "MTLPixelFormat")
# プロファイル名 -> 設定。処理はこの表を回すだけ。
#   features: feature の cargo 引数 / names: crate の判定の名前の一覧
#   header_crate: per-header feature を数える package 名（無ければ None）/ judged_features・recorded_features: その feature 名
PROFILES = {
    "cpu-only": {
        "features": ("--no-default-features", "--features", "contract-core,cpu-isa,cpu-ops"),
        "names": SDK_LOADER_NAMES + SURFACE_NAMES,
        "header_crate": None,
        "judged_features": (),
        "recorded_features": (),
    },
    "compute-only-metal": {
        "features": ("--no-default-features", "--features", "contract-core,cpu-isa,cpu-ops,backend-metal"),
        "names": SURFACE_NAMES,
        "header_crate": HEADER_CRATE,
        "judged_features": RENDER_HEADER_FEATURES,
        "recorded_features": RECORD_ONLY_HEADER_FEATURES,
    },
}
# ==== 数え方の定義ここまで ====

TREE_LINE = re.compile(r"^(\S+) v(\S+)( .*)?$")
FEATURE_LINE = re.compile(r'^(\S+) feature "([^"]*)"(?: \(command-line\))?(?: \(\*\))?$')
VERSION_ARG = re.compile(r"^[0-9A-Za-z.+-]+$")


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def clean(text):
    """出力に出す名前から制御文字や想定外の文字を落とす。"""
    return re.sub(r"[^A-Za-z0-9_.:/+@-]", "?", str(text))[:100]


def fold(name):
    folded = name.lower()
    for old, new in NAME_REPLACE:
        folded = folded.replace(old, new)
    return folded


def tree_cmd(feature_args):
    return ["cargo", "tree", "--locked", "-p", PACKAGE, *feature_args,
            "-e", ",".join(DEP_KINDS), "--target", TARGET, "--prefix", "none"]


def features_cmd(feature_args, inverse=None):
    """features の辺を含む cargo tree。inverse に "名前@版" を渡すと、その package の有効な feature を逆向きに引く。"""
    kinds = ",".join(DEP_KINDS + ("features",))
    cmd = ["cargo", "tree", "--locked", "-p", PACKAGE, *feature_args, "-e", kinds]
    if inverse is not None:
        cmd += ["-i", inverse]
    return cmd + ["--target", TARGET, "--prefix", "none"]


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
    """記録に載せる HEAD のコミット（変更が在れば -dirty）を返す。取れなければ Misuse。"""
    rev = _git(root, ["rev-parse", "--verify", "HEAD^{commit}"])
    sha = rev.strip() if rev is not None else ""
    need(re.fullmatch(r"[0-9a-f]{40}([0-9a-f]{24})?", sha) is not None, "cannot resolve the source commit")
    st = _git(root, ["status", "--porcelain", "--untracked-files=normal", "--ignore-submodules=all"])
    need(st is not None, "cannot resolve the source commit")
    return sha + ("-dirty" if st.strip() else "")


def run_cargo(root, cmd):
    """cargo を引数の並びで起動して標準出力を返す。失敗は Misuse（標準エラーは出さない）。"""
    env = dict(os.environ, CARGO_TERM_COLOR="never")
    try:
        done = subprocess.run(
            cmd, cwd=root, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    except OSError:
        raise Misuse("cannot run cargo")
    need(done.returncode == 0, "%s failed" % " ".join(cmd[:2]))
    return done.stdout.decode("utf-8", "replace")


def read_text(path):
    try:
        with open(path, "rb") as f:
            return f.read().decode("utf-8")
    except (OSError, UnicodeDecodeError):
        raise Misuse("cannot read input file")


def parse_tree(text):
    """cargo tree --prefix none の出力から (名前, 版) の一意の集合を返す。括弧の中は読まない。"""
    found = set()
    for line in text.splitlines():
        if not line.strip():
            continue
        if line.endswith(" (*)"):
            line = line[:-4]
        m = TREE_LINE.match(line)
        need(m is not None, "unreadable cargo tree line")
        found.add((m.group(1), m.group(2)))
    return found


def parse_feature_tree(text):
    """features の辺を含む cargo tree の出力から (packages, features) を返す。features は (名前, feature) の一意の集合。"""
    packages, features = set(), set()
    for line in text.splitlines():
        if not line.strip():
            continue
        m = FEATURE_LINE.match(line)
        if m is not None:
            features.add((m.group(1), m.group(2)))
            continue
        if line.endswith(" (*)"):
            line = line[:-4]
        m = TREE_LINE.match(line)
        need(m is not None, "unreadable cargo tree line")
        packages.add((m.group(1), m.group(2)))
    need(packages, "no crates found in the dependency graph")
    need(any(fold(n) == fold(PACKAGE) for n, _ in packages), "the checked crate is missing from the dependency graph")
    return packages, features


def header_versions(crates, header_crate):
    return sorted(v for n, v in crates if fold(n) == fold(header_crate))


def cargo_features(root, profile, crates):
    """cargo を流して features の辺を含む出力を返す（順方向 + header_crate の版ごとの逆向き）。"""
    cfg = PROFILES[profile]
    out = run_cargo(root, features_cmd(cfg["features"]))
    for ver in header_versions(crates, cfg["header_crate"]):
        need(VERSION_ARG.match(ver) is not None, "unreadable cargo tree line")
        out += "\n" + run_cargo(root, features_cmd(cfg["features"], "%s@%s" % (cfg["header_crate"], ver)))
    return out


def judge(profile, crates):
    """プロファイル 1 つ分の判定に当たる (名前, 版) を並べて返す。空の tree や検査対象の欠落は Misuse。"""
    need(crates, "no crates found in the dependency graph")
    need(any(fold(n) == fold(PACKAGE) for n, _ in crates), "the checked crate is missing from the dependency graph")
    wanted = {fold(n) for n in PROFILES[profile]["names"]}
    return sorted(k for k in crates if fold(k[0]) in wanted)


def main():
    ap = argparse.ArgumentParser(prog="profile-dep-graph-check.sh", add_help=True)
    ap.add_argument("root")
    ap.add_argument("--tree-text")
    ap.add_argument("--profile")
    ap.add_argument("--features-text")
    a = ap.parse_args()
    if (a.tree_text is None) != (a.profile is None):
        raise Misuse("--tree-text and --profile must be given together")
    if a.features_text is not None and a.tree_text is None:
        raise Misuse("--features-text needs --tree-text and --profile")

    inputs = []  # (profile, crates, feature の集合 | None, commit)
    if a.tree_text is not None:
        need(a.profile in PROFILES, "unknown profile")
        has_header = PROFILES[a.profile]["header_crate"] is not None
        need(has_header == (a.features_text is not None), "--features-text is required for this profile and not accepted otherwise")
        crates = parse_tree(read_text(a.tree_text))
        feats = parse_feature_tree(read_text(a.features_text))[1] if has_header else None
        inputs.append((a.profile, crates, feats, "-"))
    else:
        commit = source_commit(a.root)
        run_cargo(a.root, ["cargo", "--version"])
        for profile, cfg in PROFILES.items():
            crates = parse_tree(run_cargo(a.root, tree_cmd(cfg["features"])))
            feats = None
            if cfg["header_crate"] is not None:
                feats = parse_feature_tree(cargo_features(a.root, profile, crates))[1]
            inputs.append((profile, crates, feats, commit))

    results = [(p, c, f, rev, judge(p, c)) for (p, c, f, rev) in inputs]
    crate_total = 0
    feature_total = 0
    for profile, crates, feats, commit, hits in results:
        cfg = PROFILES[profile]
        extra = ""
        counts = []
        if feats is not None:
            hc = cfg["header_crate"]
            for judged, names in (("yes", cfg["judged_features"]), ("no", cfg["recorded_features"])):
                for f in names:
                    k = sum(1 for (n, ff) in feats if fold(n) == fold(hc) and ff == f)
                    counts.append((judged, f, k))
            extra = " header_crate_n=%d features_judged_total=%d features_recorded_total=%d" % (
                len(header_versions(crates, hc)),
                sum(k for j, _f, k in counts if j == "yes"), sum(k for j, _f, k in counts if j == "no"))
        print("scanned: profile=%s crates=%d" % (profile, len(crates)))
        print("record: commit=%s profile=%s crates_total=%d judged_total=%d%s"
              % (commit, profile, len(crates), len(hits), extra))
        for n in cfg["names"]:
            k = sum(1 for (hn, _v) in hits if fold(hn) == fold(n))
            print("count: profile=%s crate=%s n=%d" % (profile, clean(n), k))
        for judged, f, k in counts:
            print("count: profile=%s feature=%s judged=%s n=%d" % (profile, clean(f), judged, k))
        for hn, hv in hits:
            print("violation: profile=%s crate=%s version=%s" % (profile, clean(fold(hn)), clean(hv)), file=sys.stderr)
        for judged, f, k in counts:
            if judged == "yes" and k:
                print("violation: profile=%s crate=%s feature=%s" % (profile, clean(fold(cfg["header_crate"])), clean(f)),
                      file=sys.stderr)
                feature_total += 1
        crate_total += len(hits)
    if crate_total:
        print("FAILED: %d judged crate(s) found" % crate_total, file=sys.stderr)
    if feature_total:
        print("FAILED: %d judged feature(s) found" % feature_total, file=sys.stderr)
    if crate_total or feature_total:
        return 1
    print("ok: %d profile(s) checked, no judged crates (judged=0)" % len(results))
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
