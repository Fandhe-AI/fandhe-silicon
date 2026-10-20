#!/usr/bin/env bash
# 背後実装の feature を付けたまま wasm32 向けに契約の crate が `cargo check` を通ること、Vulkan・CUDA のモジュールが外れ
# Metal のモジュールが公開のアイテムを持たないこと、wasm32 の依存グラフにチップの SDK が載らないことを行ごとに出す
# （spec TASK-238・BUILD-22・BUILD-23 のポインタ。親 Issue #1155 の 1/2）。
# 呼び出し元: `make wasm-backend-check` と CI の `deny` ジョブ。回帰テストは `make test-wasm-backend-check`
# （`scripts/test-wasm-backend-check.sh`）。`scripts/build-profiles.sh` の wasm32 の組は check の成否だけを見る。
# この検査はそれに加えてモジュールの有無と依存グラフの件数を出す（重複ではなく、判別できる事実を足す役）。
#
# 使い方:
#   scripts/wasm-backend-check.sh                                          3 構成を実際に check / rustdoc / tree で調べる
#   scripts/wasm-backend-check.sh --rustdoc-json <feature>=<path> ... --tree-text <feature>=<path> ... --native-tree-text <feature>=<path> ...
#     渡した入力だけで module: と deps: を判定する（cargo・git を呼ばない。回帰テスト用。3 構成ぶんを 3 種すべてそろえる。
#     check: は not-run とし、終了コードは 1 になる）
# 終了コード: 0 全行 match / 1 mismatch または not-run が 1 行以上 / 2 誤用・前提の不足・cargo の rustdoc / tree / metadata の失敗・読めない入力。
# 標準出力: `check:`・`module:`・`deps:` の行（result は match / mismatch / not-run）、終了コード 0 と 1 のときだけ `record:`、合格時の `ok:`。
#   `record: commit=<rev|-> configs=3 check=<k|->/3 module=<k>/3 sdk_wasm32_total=<n> sdk_native_total=<n>`
# 標準エラー: `violation:`・`FAILED:`・`error:`。ホストの絶対パス・環境変数の値は出力しない。ソースを変更しない。
#
# 数え方・構成・期待は Python 部先頭の「定義」ブロックにだけ置く。数えるのは一意の package（名前, 版）で、名前は小文字にして `_` を `-` にそろえた完全一致。
# `native_n`（`--target all` の件数）は記録用で合否に使わない（未宣言か target で外れたかを読み分けるため）。
# RUSTFLAGS 系の環境変数は子プロセスへ渡さず、RUSTC_BOOTSTRAP は rustdoc の子プロセスにだけ渡す（public-api-check.sh と同じ扱い）。
# Python 3.8 以上の標準ライブラリだけ。値の扱いはすべて Python の中で行い、シェルは入力の値に触れない。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
exec python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import json
import os
import re
import shutil
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# ==== 定義（変えるときはここだけを変える） ====
PACKAGE = "fandhe-silicon-contract"
LIB = "fandhe_silicon_contract"
TARGET = "wasm32-unknown-unknown"
SUBDIR = "wasm-backend-check"
DEP_KINDS = "normal,build,dev"
SDK_NAMES = ("objc2-metal", "ash", "cudarc")
# feature -> (モジュール名, 期待)。present-empty は root に在り公開の子が 0 件、absent は root に無い。
CONFIGS = {
    "backend-metal": ("backend_metal", "present-empty"),
    "backend-vulkan": ("backend_vulkan", "absent"),
    "backend-cuda": ("backend_cuda", "absent"),
}
FLAG_ENVS = ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS",
            "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS", "RUSTC_BOOTSTRAP",
            "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_BUILD_RUSTDOCFLAGS",
            "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTDOCFLAGS")
# ==== 定義ここまで ====


def fail2(msg):
    print("error: " + msg, file=sys.stderr)
    sys.exit(2)


def parse_args(argv):
    kinds = {"--rustdoc-json": "json", "--tree-text": "tree", "--native-tree-text": "native"}
    a = {"json": {}, "tree": {}, "native": {}}
    i = 0
    while i < len(argv):
        t = argv[i]
        if t not in kinds or i + 1 >= len(argv):
            fail2("usage: wasm-backend-check.sh [--rustdoc-json <feature>=<path>]... "
                  "[--tree-text <feature>=<path>]... [--native-tree-text <feature>=<path>]...")
        key, sep, path = argv[i + 1].partition("=")
        if not sep or not path:
            fail2("expected <feature>=<path>")
        if key not in CONFIGS:
            fail2("unknown feature name")
        if key in a[kinds[t]]:
            fail2("duplicate feature for " + t)
        a[kinds[t]][key] = path
        i += 2
    given = [k for k in a if a[k]]
    if given:
        for k in a:
            if set(a[k]) != set(CONFIGS):
                fail2("offline mode needs all of --rustdoc-json, --tree-text and --native-tree-text for every feature")
    return a, bool(given)


def clean_env(extra=None):
    env = {k: v for k, v in os.environ.items() if k not in FLAG_ENVS}
    if extra:
        env.update(extra)
    return env


def run(cmd, cwd, env, quiet=False):
    try:
        return subprocess.run(cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE)
    except OSError:
        fail2("cannot run " + cmd[0])


def head_commit(root):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
    try:
        rev = subprocess.run(["git", "rev-parse", "--short=12", "HEAD"], cwd=root, env=env, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=60)
        st = subprocess.run(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, env=env,
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        fail2("cannot read the git commit")
    if rev.returncode != 0 or st.returncode != 0 or not rev.stdout.strip():
        fail2("cannot read the git commit")
    out = rev.stdout.decode("ascii", "replace").strip()
    return out + ("-dirty" if st.stdout.strip() else "")


def norm(name):
    return name.lower().replace("_", "-")


def read_text(path):
    try:
        with open(path, "r", encoding="utf-8") as f:
            return f.read()
    except (OSError, UnicodeDecodeError):
        fail2("cannot read an input file")


def module_state(doc, name):
    """(found, public_items)。形が崩れていれば終了コード 2。"""
    try:
        data = json.loads(doc)
        index = data["index"]
        root = index[str(data["root"])]
        if "module" not in root["inner"]:
            raise KeyError("root")
        found = None
        for cid in root["inner"]["module"]["items"]:
            item = index.get(str(cid))
            if item is None or item.get("name") != name:
                continue
            inner = item.get("inner", {})
            if "module" in inner:
                found = item
                break
        if found is None:
            return False, None
        pub = 0
        for cid in found["inner"]["module"]["items"]:
            item = index.get(str(cid))
            if item is not None and item.get("visibility") == "public":
                pub += 1
        return True, pub
    except (ValueError, KeyError, TypeError, AttributeError):
        fail2("cannot read the rustdoc JSON")


def count_sdk(text):
    """一意の package（名前, 版）の数を SDK ごとに。検査対象の crate が無ければ終了コード 2。"""
    seen = set()
    for line in text.splitlines():
        parts = line.strip().split()
        if len(parts) >= 2 and re.match(r"v\d", parts[1]):
            seen.add((norm(parts[0]), parts[1]))
    if not any(n == PACKAGE for n, _ in seen):
        fail2("the checked crate is missing from the dependency tree")
    return {s: len([1 for n, _ in seen if n == s]) for s in SDK_NAMES}


def main():
    args, offline = parse_args(sys.argv[2:])
    root = sys.argv[1]
    commit = "-"
    td = None
    if not offline:
        commit = head_commit(root)
        if shutil.which("cargo") is None or shutil.which("rustup") is None:
            fail2("cargo and rustup are required")
        done = run(["rustup", "target", "list", "--installed"], root, clean_env())
        if done.returncode != 0 or TARGET not in done.stdout.decode("utf-8", "replace").split():
            fail2("rust target not installed: " + TARGET)
        done = run(["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"], root, clean_env())
        if done.returncode != 0:
            fail2("cargo metadata failed")
        try:
            td = json.loads(done.stdout.decode("utf-8"))["target_directory"]
        except (ValueError, KeyError, UnicodeDecodeError):
            fail2("cannot read cargo metadata")

    rows = []
    violations = []
    check_ok = module_ok = 0
    wasm_total = native_total = 0
    for feat, (mod, expect) in CONFIGS.items():
        # check:
        if offline:
            check = "not-run"
        else:
            out = os.path.join(td, SUBDIR, "check")
            cmd = ["cargo", "check", "-p", PACKAGE, "--target", TARGET, "--features", feat, "--locked",
                  "--target-dir", out]
            check = "match" if run(cmd, root, clean_env()).returncode == 0 else "mismatch"
        rows.append("check: feature=%s target=%s result=%s" % (feat, TARGET, check))
        if check == "match":
            check_ok += 1
        else:
            violations.append("check: feature=%s result=%s" % (feat, check))

        # module:
        if offline:
            doc = read_text(args["json"][feat])
        else:
            out = os.path.join(td, SUBDIR, "rustdoc")
            built = os.path.join(out, TARGET, "doc", LIB + ".json")
            if os.path.exists(built):
                os.remove(built)
            cmd = ["cargo", "rustdoc", "-p", PACKAGE, "--lib", "--target", TARGET, "--features", feat, "--locked",
                  "--target-dir", out, "--", "-Z", "unstable-options", "--output-format", "json"]
            if run(cmd, root, clean_env({"RUSTC_BOOTSTRAP": "1"})).returncode != 0:
                fail2("cargo rustdoc failed for " + feat)
            if not os.path.isfile(built):
                fail2("rustdoc JSON was not produced")
            kept = os.path.join(out, TARGET, "doc", "%s.%s.json" % (LIB, feat))
            os.replace(built, kept)
            doc = read_text(kept)
        found, pub = module_state(doc, mod)
        if expect == "present-empty":
            ok = found and pub == 0
        else:
            ok = not found
        res = "match" if ok else "mismatch"
        rows.append("module: feature=%s module=%s expect=%s found=%s public_items=%s result=%s" % (
            feat, mod, expect, "present" if found else "absent", "-" if pub is None else pub, res))
        if ok:
            module_ok += 1
        else:
            violations.append("module: feature=%s module=%s expect=%s" % (feat, mod, expect))

        # deps:
        if offline:
            wasm_txt = read_text(args["tree"][feat])
            native_txt = read_text(args["native"][feat])
        else:
            base = ["cargo", "tree", "--locked", "-p", PACKAGE, "--features", feat, "-e", DEP_KINDS,
                    "--prefix", "none"]
            r1 = run(base + ["--target", TARGET], root, clean_env())
            r2 = run(base + ["--target", "all"], root, clean_env())
            if r1.returncode != 0 or r2.returncode != 0:
                fail2("cargo tree failed for " + feat)
            wasm_txt = r1.stdout.decode("utf-8", "replace")
            native_txt = r2.stdout.decode("utf-8", "replace")
        w = count_sdk(wasm_txt)
        n = count_sdk(native_txt)
        for sdk in SDK_NAMES:
            res = "match" if w[sdk] == 0 else "mismatch"
            rows.append("deps: feature=%s name=%s wasm32_n=%d native_n=%d result=%s" % (feat, sdk, w[sdk], n[sdk], res))
            if w[sdk] != 0:
                violations.append("deps: feature=%s name=%s wasm32_n=%d" % (feat, sdk, w[sdk]))
        wasm_total += sum(w.values())
        native_total += sum(n.values())

    for r in rows:
        print(r)
    print("record: commit=%s configs=%d check=%s/%d module=%d/%d sdk_wasm32_total=%d sdk_native_total=%d" % (
        commit, len(CONFIGS), "-" if offline else check_ok, len(CONFIGS), module_ok, len(CONFIGS),
        wasm_total, native_total))
    if offline:
        violations.append("check: not-run (offline inputs)")
    if violations:
        for v in violations:
            print("violation: " + v, file=sys.stderr)
        print("FAILED: %d violation(s)" % len(violations), file=sys.stderr)
        sys.exit(1)
    print("ok: all %d configurations match" % len(CONFIGS))


try:
    main()
except SystemExit:
    raise
except Exception:
    print("error: unexpected failure", file=sys.stderr)
    sys.exit(2)
PY
