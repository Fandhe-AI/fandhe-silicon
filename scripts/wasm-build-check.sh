#!/usr/bin/env bash
# wasm32 向けの cdylib（crates/wasm-abi）の 2 版の成果物の判別・公開の型の存在・simd128 がビルド時の cfg で決まることを確かめる
# （spec TASK-237・BUILD-21・BUILD-25 のポインタ。親 Issue #462 の 3/4）。
# 呼び出し元: `make wasm-build-check`（手元で流す。CI への組み込みは後続の sub-issue）。
# 回帰テストは `make test-wasm-build-check`（`scripts/test-wasm-build-check.sh`）。
#
# 使い方:
#   scripts/wasm-build-check.sh [--require-type <path>]... [--from-dir <scalar.wasm> <simd128.wasm>]
#                               [--source-root <dir>] [--rustdoc-json <path>]
#     --require-type   公開アイテムに在るべき型の path（crate 名の有無はどちらでもよい。複数可。確かめる名前はこのスクリプトに持たせない）
#     --from-dir       ビルドせず、渡した 2 つの wasm を判別する（回帰テスト用。順は scalar、simd128。名前に反してファイルを 2 つ取る）
#     --source-root    cfg の走査の起点（既定は crates/）
#     --rustdoc-json   cargo を呼ばずにこの rustdoc JSON で（2 版とも）型を確かめる（回帰テスト用）
# 終了コード: 0 全行が match / 1 mismatch または not-run が 1 行以上 / 2 誤用・前提の不足・読めない入力。
# 標準出力: 行ごとの `artifact:`・`type:`・`cfg:`（label は match / mismatch / not-run。理由は reason=）、
#   終了コード 0 と 1 のときだけ `record:`、合格時の `ok:`。標準エラー: `error:`（2）・`FAILED:`（1）。
#   ホストの絶対パス・環境変数の値は出力しない。
# 各行:
#   artifact  2 つの wasm の先頭が `\0asm` で、custom section `target_features` に `+simd128` が simd128 版にだけ在り、2 つが別のバイト列であること。
#   type      --require-type の各 path が、2 版それぞれと同じ指定（simd128 版は +simd128）で作った wasm32 向けの rustdoc JSON の
#             公開アイテムに型（struct / enum / union / type alias / trait）として在ること。行は build=scalar|simd128 ごと。0 個なら not-run（reason=no-required-types）。
#   cfg       文字列・文字リテラルの外の `target_feature = "simd128"` の出現（コメントを除く）がすべて `cfg(all(target_arch = "wasm32", target_feature = "simd128"))`
#             （または `cfg(not(all(...)))`。scalar 側の分岐用）の中に在り、実行時の検出（`_feature_detected!("simd128")`）が無いこと。出現が 0 件なら not-run（reason=no-simd128-sites）。
# 既定の経路（ビルドする）の前提: python3 3.8 以上・cargo・wasm32-unknown-unknown の target。外れたら終了コード 2。
#   feature の組は `contract-core,cpu-isa,cpu-ops`、--locked。成果物は cargo の target_directory 配下の専用ディレクトリ
#   （wasm-build-check）に分け、製品のビルドと共有しない。RUSTFLAGS は子プロセスへ渡さず、命令の指定は
#   CARGO_ENCODED_RUSTFLAGS だけで渡す。RUSTC_BOOTSTRAP は rustdoc の子プロセスにだけ渡す（public-api-check.sh と同じ扱い）。
# 検査しないもの: 実行時の振る舞い、型の中身、CI への組み込み。
# 値の扱いはすべて Python の中で行い、シェルは入力の値に触れない。Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
exec python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

PACKAGE = "fandhe-silicon-wasm-abi"
LIB = "fandhe_silicon_wasm_abi"
FEATURES = "contract-core,cpu-isa,cpu-ops"
TARGET = "wasm32-unknown-unknown"
SUBDIR = "wasm-build-check"
SIMD_FLAGS = "-C\x1ftarget-feature=+simd128"
# 子プロセスへ引き継がない rustflags 系の環境変数（命令の指定を利用者の環境から切り離す）。
FLAG_ENVS = ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS",
            "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS", "RUSTC_BOOTSTRAP",
            "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_BUILD_RUSTDOCFLAGS",
            "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTDOCFLAGS")
# 型として数える rustdoc JSON の kind（関数・モジュール・定数などは型ではない）。
TYPE_KINDS = ("struct", "enum", "union", "type_alias", "trait")


class Misuse(Exception):
    pass


def fail2(msg):
    print("error: " + msg, file=sys.stderr)
    sys.exit(2)


def parse_args(argv):
    a = {"types": [], "from": None, "src": None, "json": None}
    i = 0
    n = len(argv)
    while i < n:
        t = argv[i]
        if t == "--require-type" and i + 1 < n:
            a["types"].append(argv[i + 1])
            i += 2
        elif t == "--from-dir" and i + 2 < n:
            a["from"] = (argv[i + 1], argv[i + 2])
            i += 3
        elif t == "--source-root" and i + 1 < n:
            a["src"] = argv[i + 1]
            i += 2
        elif t == "--rustdoc-json" and i + 1 < n:
            a["json"] = argv[i + 1]
            i += 2
        else:
            fail2("usage: wasm-build-check.sh [--require-type <path>]... [--from-dir <scalar.wasm> <simd128.wasm>] "
                  "[--source-root <dir>] [--rustdoc-json <path>]")
    return a


def clean_env(extra=None):
    env = {k: v for k, v in os.environ.items() if k not in FLAG_ENVS}
    if extra:
        env.update(extra)
    return env


def run(cmd, cwd, env):
    try:
        return subprocess.run(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except OSError:
        fail2("cannot run " + cmd[0])


def target_dir(root):
    done = run(["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"], root, clean_env())
    if done.returncode != 0:
        fail2("cargo metadata failed")
    try:
        return json.loads(done.stdout.decode("utf-8"))["target_directory"]
    except (ValueError, KeyError, UnicodeDecodeError):
        fail2("cannot read cargo metadata")


def check_wasm_target(root):
    if shutil.which("rustup") is None:
        return
    done = run(["rustup", "target", "list", "--installed"], root, clean_env())
    if done.returncode == 0 and TARGET not in done.stdout.decode("utf-8", "replace").split():
        fail2("rust target not installed: " + TARGET)


def build_wasm(root, td, kind, flags):
    out = os.path.join(td, SUBDIR, kind)
    env = clean_env({"CARGO_ENCODED_RUSTFLAGS": flags} if flags else None)
    cmd = ["cargo", "build", "-p", PACKAGE, "--no-default-features", "--features", FEATURES,
          "--target", TARGET, "--release", "--locked", "--target-dir", out]
    if run(cmd, root, env).returncode != 0:
        fail2("cargo build failed for the %s artifact" % kind)
    return os.path.join(out, TARGET, "release", LIB + ".wasm")


def build_rustdoc_json(root, td, kind, flags):
    out = os.path.join(td, SUBDIR, "rustdoc-" + kind)
    path = os.path.join(out, TARGET, "doc", LIB + ".json")
    if os.path.exists(path):
        os.remove(path)
    cmd = ["cargo", "rustdoc", "--locked", "--target-dir", out, "-p", PACKAGE, "--no-default-features",
          "--features", FEATURES, "--target", TARGET, "--lib", "--", "-Z", "unstable-options",
          "--output-format", "json"]
    extra = {"RUSTC_BOOTSTRAP": "1"}
    if flags:
        # 依存の rustc には RUSTFLAGS、rustdoc 本体の cfg には RUSTDOCFLAGS が効く。2 版のビルドと同じ指定を両方へ渡す。
        extra["CARGO_ENCODED_RUSTFLAGS"] = flags
        extra["CARGO_ENCODED_RUSTDOCFLAGS"] = flags
    if run(cmd, root, clean_env(extra)).returncode != 0:
        fail2("cargo rustdoc failed")
    if not os.path.isfile(path):
        fail2("rustdoc JSON was not produced")
    return path


# ---- wasm の読み取り ----
def leb(data, pos, end):
    val = 0
    shift = 0
    while True:
        if pos >= end or shift > 35:
            raise Misuse("malformed LEB128")
        b = data[pos]
        pos += 1
        val |= (b & 0x7F) << shift
        if not b & 0x80:
            return val, pos
        shift += 7


def target_features(data):
    """custom section `target_features` の項目（`+simd128` 形式）の集合。無ければ None。"""
    if len(data) < 8 or data[:8] != b"\x00asm\x01\x00\x00\x00":
        raise Misuse("not a wasm module")
    pos = 8
    found = None
    while pos < len(data):
        sid = data[pos]
        pos += 1
        size, pos = leb(data, pos, len(data))
        end = pos + size
        if end > len(data):
            raise Misuse("section exceeds the file")
        if sid == 0:
            nlen, p = leb(data, pos, end)
            if p + nlen > end:
                raise Misuse("section name exceeds the section")
            name = data[p:p + nlen]
            p += nlen
            if name == b"target_features":
                if found is not None:
                    raise Misuse("duplicate target_features section")
                count, p = leb(data, p, end)
                found = set()
                for _ in range(count):
                    if p >= end:
                        raise Misuse("truncated target_features")
                    prefix = chr(data[p])
                    p += 1
                    flen, p = leb(data, p, end)
                    if p + flen > end:
                        raise Misuse("truncated target_features")
                    found.add(prefix + data[p:p + flen].decode("utf-8", "replace"))
                    p += flen
        pos = end
    return found


def read_wasm(path):
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError:
        raise Misuse("cannot read a wasm input")
    if not data:
        raise Misuse("empty wasm input")
    return data


def artifact_rows(paths):
    if os.path.abspath(paths[0]) == os.path.abspath(paths[1]):
        raise Misuse("the two wasm inputs are the same path")
    blobs = [read_wasm(p) for p in paths]
    feats = [target_features(b) for b in blobs]
    same = blobs[0] == blobs[1]
    rows = []
    for kind, blob, f, want in (("scalar", blobs[0], feats[0], False), ("simd128", blobs[1], feats[1], True)):
        has = f is not None and "+simd128" in f
        reason = ""
        if same:
            ok, reason = False, " reason=identical-artifacts"
        elif f is None:
            ok, reason = False, " reason=missing-target-features"
        else:
            ok = has == want
        lab = "match" if ok else "mismatch"
        rows.append(("artifact: kind=%s label=%s simd128=%s%s bytes=%d sha256=%s" % (
            kind, lab, "present" if has else "absent", reason, len(blob), hashlib.sha256(blob).hexdigest()), lab))
    return rows


# ---- 型 ----
def public_types(json_path):
    try:
        with open(json_path, encoding="utf-8") as f:
            doc = json.load(f)
        paths, index = doc["paths"], doc["index"]
        if not isinstance(paths, dict) or not isinstance(index, dict):
            raise ValueError
    except (OSError, UnicodeDecodeError, ValueError, KeyError, TypeError):
        raise Misuse("cannot read rustdoc JSON")
    public = set()
    for key, ent in paths.items():
        try:
            if ent["crate_id"] != 0 or ent["kind"] not in TYPE_KINDS:
                continue
            item = index.get(key)
            if not isinstance(item, dict) or item.get("visibility") not in ("public", "default"):
                continue
            public.add("::".join(ent["path"]))
        except (KeyError, TypeError, AttributeError):
            raise Misuse("unexpected rustdoc JSON shape")
    return public


def type_rows(types, json_paths):
    """json_paths は (build 名, rustdoc JSON のパス) の列。build ごとに要求された型を照合する。"""
    if not types:
        return [("type: label=not-run reason=no-required-types", "not-run")], 0
    sets = [(kind, public_types(path)) for kind, path in json_paths]
    rows = []
    both = 0
    for t in types:
        all_ok = True
        for kind, public in sets:
            ok = t in public or (LIB + "::" + t) in public
            all_ok = all_ok and ok
            lab = "match" if ok else "mismatch"
            rows.append(("type: name=%s label=%s build=%s" % (re.sub(r"[^A-Za-z0-9_:]", "?", t), lab, kind), lab))
        if all_ok:
            both += 1
    return rows, both


# ---- cfg ----
RAW_OPEN = re.compile(r'(?:b|c)?r(#*)"')


def is_ident(ch):
    return ch.isalnum() or ch == "_"


def strip_comments(text):
    """コメントを空白に置き換える（改行を残し、位置と行番号を保つ）。
    戻り値は (本文, 文字列・文字リテラルの内側かの印)。通常文字列・raw string・文字リテラルは字句として読み、
    印の付いた位置から始まる一致は属性やマクロ呼び出しではないので数えない。"""
    out = []
    mark = []
    i, n = 0, len(text)

    def emit(ch, flag):
        out.append(ch)
        mark.append(flag)

    while i < n:
        c = text[i]
        two = text[i:i + 2]
        if two == "//":
            while i < n and text[i] != "\n":
                emit(" ", False)
                i += 1
            continue
        if two == "/*":
            depth = 0
            while i < n:
                if text[i:i + 2] == "/*":
                    depth += 1
                    emit(" ", False)
                    emit(" ", False)
                    i += 2
                elif text[i:i + 2] == "*/":
                    depth -= 1
                    emit(" ", False)
                    emit(" ", False)
                    i += 2
                    if depth == 0:
                        break
                else:
                    emit("\n" if text[i] == "\n" else " ", False)
                    i += 1
            continue
        prev_ident = i > 0 and is_ident(text[i - 1])
        raw = RAW_OPEN.match(text, i) if c in "rbc" and not prev_ident else None
        if raw:
            close = '"' + raw.group(1)
            end = text.find(close, raw.end())
            end = n if end < 0 else end + len(close)
            while i < end:
                emit(text[i], True)
                i += 1
            continue
        if c == '"':
            emit(c, True)
            i += 1
            while i < n and text[i] != '"':
                if text[i] == "\\" and i + 1 < n:
                    emit(text[i], True)
                    i += 1
                emit(text[i], True)
                i += 1
            if i < n:
                emit('"', True)
                i += 1
            continue
        if c == "'":
            # 文字リテラル（'x' '\n' '"'）か、ライフタイム（'a）か。
            if i + 1 < n and text[i + 1] == "\\":
                j = i + 2
                while j < n and text[j] != "'" and text[j] != "\n":
                    j += 1
                end = j + 1 if j < n and text[j] == "'" else i + 1
            elif i + 2 < n and text[i + 2] == "'" and text[i + 1] != "'":
                end = i + 3
            else:
                end = i + 1
            while i < end:
                emit(text[i], end > i + 1)
                i += 1
            continue
        emit(c, False)
        i += 1
    return "".join(out), mark


SITE = re.compile(r'target_feature\s*=\s*"simd128"')
ALL = r'all\s*\(\s*target_arch\s*=\s*"wasm32"\s*,\s*target_feature\s*=\s*"simd128"\s*,?\s*\)'
GOOD = re.compile(r'cfg!?\s*\(\s*(?:' + ALL + r'|not\s*\(\s*' + ALL + r'\s*\))\s*\)')
RUNTIME = re.compile(r'_feature_detected!\s*\(\s*"simd128"')


def cfg_rows(root, src_root):
    sites = 0
    bad = []
    if not os.path.isdir(src_root):
        raise Misuse("source root is not a directory")
    aroot = os.path.abspath(root) + os.sep
    for base, dirs, files in os.walk(src_root):
        dirs[:] = sorted(d for d in dirs if d not in ("target", ".git"))
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            full = os.path.join(base, name)
            try:
                with open(full, encoding="utf-8") as f:
                    text, mark = strip_comments(f.read())
            except (OSError, UnicodeDecodeError):
                raise Misuse("cannot read a source file")
            rel = os.path.relpath(full, root if os.path.abspath(full).startswith(aroot) else src_root)
            good = [m.span() for m in GOOD.finditer(text) if not mark[m.start()]]
            for m in SITE.finditer(text):
                if mark[m.start()]:
                    continue
                sites += 1
                if not any(s <= m.start() and m.end() <= e for s, e in good):
                    bad.append((rel, text.count("\n", 0, m.start()) + 1, "unsupported-cfg-form"))
            for m in RUNTIME.finditer(text):
                if mark[m.start()]:
                    continue
                bad.append((rel, text.count("\n", 0, m.start()) + 1, "runtime-detection"))
    lines = []
    if bad:
        lines.append(("cfg: sites=%d label=mismatch reason=%s" % (sites, bad[0][2]), "mismatch"))
        for rel, ln, why in bad:
            lines.append(("violation: file=%s line=%d reason=%s" % (rel, ln, why), None))
    elif sites == 0:
        lines.append(("cfg: sites=0 label=not-run reason=no-simd128-sites", "not-run"))
    else:
        lines.append(("cfg: sites=%d label=match" % sites, "match"))
    return lines, sites


def commit(root):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
    try:
        r = subprocess.run(["git", "-C", root, "rev-parse", "--short", "HEAD"], env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if r.returncode != 0:
            return "-"
        rev = r.stdout.decode().strip()
        s = subprocess.run(["git", "-C", root, "status", "--porcelain"], env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        return rev + ("-dirty" if s.returncode == 0 and s.stdout.strip() else "")
    except OSError:
        return "-"


def main():
    root = sys.argv[1]
    a = parse_args(sys.argv[2:])
    try:
        td = None
        if a["from"] is None or (a["types"] and a["json"] is None):
            if shutil.which("cargo") is None:
                fail2("required command not found on PATH: cargo")
            check_wasm_target(root)
            td = target_dir(root)
        if a["from"] is None:
            paths = (build_wasm(root, td, "scalar", ""), build_wasm(root, td, "simd128", SIMD_FLAGS))
        else:
            paths = a["from"]
        if a["types"] and a["json"] is None:
            scalar_doc = build_rustdoc_json(root, td, "scalar", "")
            simd_doc = build_rustdoc_json(root, td, "simd128", SIMD_FLAGS)
            jsons = (("scalar", scalar_doc), ("simd128", simd_doc))
        else:
            jsons = (("scalar", a["json"]), ("simd128", a["json"]))
        art = artifact_rows(paths)
        typ, tm = type_rows(a["types"], jsons)
        src = a["src"] or os.path.join(root, "crates")
        cfg, sites = cfg_rows(root, src)
    except Misuse as e:
        fail2(str(e))
    rows = art + typ + cfg
    for text, _ in rows:
        print(text)
    labels = [lab for _, lab in rows if lab]
    matched = sum(1 for _, lab in art if lab == "match")
    not_run = sum(1 for lab in labels if lab == "not-run")
    print("record: commit=%s artifact=%d/2 types=%d/%d cfg_sites=%d not_run=%d" % (
        commit(root), matched, tm, len(a["types"]), sites, not_run))
    if all(lab == "match" for lab in labels):
        print("ok: all rows match")
        return 0
    print("FAILED: at least one row is mismatch or not-run", file=sys.stderr)
    return 1


sys.exit(main())
PY
