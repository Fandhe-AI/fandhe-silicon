#!/usr/bin/env bash
# 共通の crate（`crates/core`）が宣言する外部の crate の一覧が決められた一覧と完全に一致することと、
# 共通の crate の feature が契約の crate（`crates/contract`）の同じ名前の feature から転送されていることを、
# cargo metadata から機械で確かめる（spec TASK-258・BUILD-54・U-30 のポインタ。親 Issue #473 の 1/2）。
# 呼び出し元: `make core-deps-check`（直接実行も可）。回帰テストは `scripts/test-core-deps-check.sh`
# （`make test-core-deps-check`）。CI の `deny` ジョブも実行する。
# 隣の検査 `scripts/public-api-sdk-check.sh`（チップの SDK の crate が無いことの検査）は「入ってはいけない名前」を数える。
# こちらは「入ってよい名前の一覧」との完全一致を見る。チップの crate の名前の一覧はここに持たない
# （チップの crate は期待の一覧に入らないので、unexpected の違反として出る）。
#
# 使い方:
#   scripts/core-deps-check.sh                        cargo metadata を流して検査する（commit は HEAD。変更が在れば末尾に `-dirty`）
#   scripts/core-deps-check.sh --metadata-json <path>  渡した metadata だけを検査する（cargo・git を呼ばない。commit は `-`。1 つだけ）
#   除外の印・除外の一覧・対象を外す口は持たない。
# 終了コード: 0 違反なし / 1 違反あり / 2 誤用・前提の不足・入力を読み切れない（成功扱いにしない）
#   2 になるもの: 引数の誤用、python3・cargo・git の不足、HEAD のコミットが取れない、cargo metadata の失敗、JSON でない入力、
#   metadata の形の崩れ（`packages`・`dependencies`・`features` の型、`kind` が null・build・dev 以外）、
#   共通の crate・契約の crate が workspace_root からの manifest でちょうど 1 件に定まらない、
#   契約の crate の共通の crate への依存がちょうど 1 件に定まらない、期待の一覧の定義の自己検査の失敗、想定外の例外。
#
# 照合の規則:
#   (A) 共通の crate の依存の宣言（package 名。`package =` で改名していても実名。target 付き・optional を含む。重複は除く）を
#       kind（通常・build・dev）ごとに期待の一覧と比べ、余り（unexpected）も不足（missing）も違反にする。
#       workspace の member への依存も宣言として数える。
#   (B) 共通の crate の feature F ごとに、契約の crate に同じ名前の feature F があり、その値に `<依存のキー>/F` があること。
#       逆向きに、契約の crate の feature の値の `<キー>/G`・`<キー>?/G` で共通の crate の feature G を有効にしているものは、
#       feature の名前が G であること。共通の crate に `default` の feature があれば違反。
#       契約の crate の `backend-*` など共通の crate に対応の無い feature は対象にしない。
#   期待の一覧は Python 部先頭の「数え方の定義」の 1 か所にだけ持つ。
#
# 標準出力: 対象ごとの `scanned:`、検査し終えたとき（終了コード 0 と 1）だけ `record:`、合格時の `ok:`。
#   `scanned: check=core-deps manifest=crates/core/Cargo.toml package=<name> normal=<a,b|-> build=<…|-> dev=<…|->`
#   `scanned: check=feature-forward manifest=crates/contract/Cargo.toml package=<name> core_features=<n> forwarded=<n>`
#   `record: commit=<rev|-> expected_normal=<…|-> declared_normal=<…|-> unexpected=<n> missing=<n> declared_build=<n> declared_dev=<n> core_features=<n> forwarded=<n> forward_violations=<n>`
#   core の宣言の総数は public-api-sdk-check.sh の `declared_deps=` と突き合わせられる
#   （そちらは重複を除かない。突き合わせは別の作業）。
# 標準エラー: `violation: check=core-dep kind=<normal|build|dev> dependency=<name> reason=<unexpected|missing> target=<cfg|->`、
#   `violation: check=feature-forward feature=<name> reason=<no-contract-feature|not-forwarded|renamed|default>`、`FAILED:`・`error:`。
#
# 検査しないもの: 推移的な依存（`Cargo.lock` にだけ出るもの）、共通の crate 以外の crate の依存、契約の crate の feature の値の中身のうち
# 共通の crate を指さないもの、wasm32 の公開 API。ホストの絶対パス・環境変数の値は出力に出さない。ソースを変更しない。
# Python 3.8 以上の標準ライブラリだけ。
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

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# ---- 数え方の定義（期待の一覧の唯一の場所。値をここの外へ書き写さない）----
# 期待の一覧を変えるのは、依存を足す PR（`deps-approved.toml` の記録と同じ PR。.claude/rules/dependency-policy.md）の中だけ。
# 共通の crate の通常の依存（kind が null。全 target・optional を含む）が完全一致すべき package 名。いまは外部依存なし。
EXPECTED_NORMAL = frozenset()
# 期待の一覧が取りうる上限（共通の crate に入れてよい外部 crate の名前）。EXPECTED_NORMAL はこの部分集合でなければならない。
PERMITTED_NORMAL = frozenset({"bitflags"})
# build・dev の依存の期待の一覧。どちらも空で、宣言を全部拾う側に倒す。
EXPECTED_BUILD = frozenset()
EXPECTED_DEV = frozenset()
# 共通の crate と契約の crate の manifest（workspace_root からの相対。公開名は未決なので package 名では探さない）。
CORE_MANIFEST_REL = "crates/core/Cargo.toml"
CONTRACT_MANIFEST_REL = "crates/contract/Cargo.toml"
# 数える依存の種類（metadata の kind。null は通常の依存）。
DEP_KINDS = {None: "normal", "build": "build", "dev": "dev"}
# cargo metadata を取るコマンド（ネットワークを使わず、lock を書き換えない）。
METADATA_CMD = ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1", "--all-features"]


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def clean(text):
    """出力に出す値から制御文字や想定外の文字を落とす。"""
    return re.sub(r"[^A-Za-z0-9_.:-]", "?", str(text))[:80]


def join(names):
    return ",".join(clean(n) for n in sorted(names)) or "-"


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


def load_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, UnicodeDecodeError, ValueError):
        raise Misuse("cannot read JSON input")


def run_cargo_metadata(root):
    """cargo metadata を argv のリストで起動して JSON を返す。失敗は Misuse（ホストの出力は載せない）。"""
    env = dict(os.environ, CARGO_TERM_COLOR="never")
    try:
        done = subprocess.run(
            METADATA_CMD, cwd=root, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False, timeout=300)
    except (OSError, subprocess.TimeoutExpired):
        raise Misuse("cannot run cargo metadata")
    need(done.returncode == 0, "cargo metadata failed")
    try:
        return json.loads(done.stdout.decode("utf-8"))
    except (UnicodeDecodeError, ValueError):
        raise Misuse("cargo metadata output is not JSON")


def self_check():
    need(EXPECTED_NORMAL <= PERMITTED_NORMAL, "expected normal dependencies exceed the permitted set")


def find_package(meta, rel):
    wroot = meta.get("workspace_root")
    need(isinstance(wroot, str) and wroot != "", "missing or malformed 'workspace_root'")
    pkgs = meta.get("packages")
    need(isinstance(pkgs, list), "missing or malformed 'packages'")
    want = os.path.normpath(os.path.join(wroot, rel))
    found = []
    for pkg in pkgs:
        need(isinstance(pkg, dict), "malformed package entry")
        mp = pkg.get("manifest_path")
        if isinstance(mp, str) and os.path.normpath(mp) == want:
            found.append(pkg)
    need(len(found) == 1, "%s not found exactly once" % rel)
    pkg = found[0]
    need(isinstance(pkg.get("name"), str), "malformed package name")
    need(isinstance(pkg.get("dependencies"), list), "missing or malformed 'dependencies'")
    feats = pkg.get("features")
    need(isinstance(feats, dict), "missing or malformed 'features'")
    for v in feats.values():
        need(isinstance(v, list) and all(isinstance(x, str) for x in v), "malformed feature list")
    for dep in pkg["dependencies"]:
        need(isinstance(dep, dict), "malformed dependency entry")
        need(isinstance(dep.get("name"), str), "dependency name is not a string")
        kind = dep.get("kind")
        need(kind is None or (isinstance(kind, str) and kind in DEP_KINDS), "dependency kind outside the counted kinds")
        for key in ("target", "rename", "path"):
            need(dep.get(key) is None or isinstance(dep.get(key), str), "malformed dependency %s" % key)
    return pkg


def check_core_deps(core):
    """(A) 通常・build・dev ごとの宣言を期待の一覧と照合する。"""
    declared = {"normal": set(), "build": set(), "dev": set()}
    targets = {}
    for dep in core["dependencies"]:
        kind = DEP_KINDS[dep.get("kind")]
        declared[kind].add(dep["name"])
        targets.setdefault((kind, dep["name"]), dep.get("target") or "-")
    expected = {"normal": EXPECTED_NORMAL, "build": EXPECTED_BUILD, "dev": EXPECTED_DEV}
    violations = []
    for kind in ("normal", "build", "dev"):
        for name in sorted(declared[kind] - expected[kind]):
            violations.append((kind, clean(name), "unexpected", clean(targets[(kind, name)])))
        for name in sorted(expected[kind] - declared[kind]):
            violations.append((kind, clean(name), "missing", "-"))
    return declared, violations


def check_forwarding(core, contract):
    """(B) 共通の crate の feature が契約の crate の同じ名前の feature から転送されていることを照合する。"""
    need(isinstance(core.get("manifest_path"), str), "malformed manifest_path")
    core_dir = os.path.dirname(os.path.normpath(core["manifest_path"]))
    keys = []
    for dep in contract["dependencies"]:
        path = dep.get("path")
        is_core = dep["name"] == core["name"]
        if isinstance(path, str) and os.path.normpath(path) == core_dir:
            is_core = True
        if is_core and dep.get("kind") is None:
            keys.append(dep.get("rename") or dep["name"])
    keys = sorted(set(keys))
    need(len(keys) == 1, "contract crate must depend on the common crate exactly once")
    key = keys[0]
    core_feats = core["features"]
    cfeats = contract["features"]
    violations = []
    forwarded = 0
    for f in sorted(core_feats):
        if f == "default":
            violations.append((clean(f), "default"))
            continue
        if f not in cfeats:
            violations.append((clean(f), "no-contract-feature"))
        elif (key + "/" + f) not in cfeats[f]:
            violations.append((clean(f), "not-forwarded"))
        else:
            forwarded += 1
    pat = re.compile(r"^" + re.escape(key) + r"\??/(.+)$")
    for name in sorted(cfeats):
        for v in cfeats[name]:
            m = pat.match(v)
            if m and m.group(1) in core_feats and m.group(1) != name:
                violations.append((clean(m.group(1)), "renamed"))
    return len(core_feats), forwarded, sorted(set(violations))


def main():
    root = sys.argv[1]
    parser = argparse.ArgumentParser(prog="core-deps-check.sh")
    parser.add_argument("--metadata-json", metavar="PATH", action="append")
    args = parser.parse_args(sys.argv[2:])
    need(args.metadata_json is None or len(args.metadata_json) == 1, "--metadata-json takes exactly one path")
    self_check()
    if args.metadata_json:
        commit = "-"
        meta = load_json(args.metadata_json[0])
    else:
        commit = source_commit(root)
        meta = run_cargo_metadata(root)
    need(isinstance(meta, dict), "metadata top level is not an object")
    core = find_package(meta, CORE_MANIFEST_REL)
    contract = find_package(meta, CONTRACT_MANIFEST_REL)

    declared, dep_violations = check_core_deps(core)
    print("scanned: check=core-deps manifest=%s package=%s normal=%s build=%s dev=%s"
          % (CORE_MANIFEST_REL, clean(core["name"]), join(declared["normal"]), join(declared["build"]), join(declared["dev"])))
    for kind, name, reason, target in dep_violations:
        print("violation: check=core-dep kind=%s dependency=%s reason=%s target=%s" % (kind, name, reason, target),
              file=sys.stderr)

    n_core, n_fwd, fwd_violations = check_forwarding(core, contract)
    print("scanned: check=feature-forward manifest=%s package=%s core_features=%d forwarded=%d"
          % (CONTRACT_MANIFEST_REL, clean(contract["name"]), n_core, n_fwd))
    for feat, reason in fwd_violations:
        print("violation: check=feature-forward feature=%s reason=%s" % (feat, reason), file=sys.stderr)

    unexpected = sum(1 for v in dep_violations if v[2] == "unexpected")
    missing = sum(1 for v in dep_violations if v[2] == "missing")
    fields = [
        ("commit", commit),
        ("expected_normal", join(EXPECTED_NORMAL)),
        ("declared_normal", join(declared["normal"])),
        ("unexpected", str(unexpected)),
        ("missing", str(missing)),
        ("declared_build", str(len(declared["build"]))),
        ("declared_dev", str(len(declared["dev"]))),
        ("core_features", str(n_core)),
        ("forwarded", str(n_fwd)),
        ("forward_violations", str(len(fwd_violations))),
    ]
    print("record: " + " ".join("%s=%s" % kv for kv in fields))
    total = len(dep_violations) + len(fwd_violations)
    if total:
        print("FAILED: %d violation(s)" % total, file=sys.stderr)
        return 1
    print("ok: common crate dependencies match the expected set and its features are forwarded from the contract crate")
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
