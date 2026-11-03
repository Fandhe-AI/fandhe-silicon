#!/usr/bin/env bash
# 下-2 の crate（crates/exec）のソースの環境変数とファイルの読み取りの字句、および既定の feature の公開の面にある
# 試験用の口の名前、操作を実行する関数の戻り値の型と一覧に無い公開の関数を数える検査（spec TASK-923 のポインタ。親 Issue #867）。
# 呼び出し元: `make exec-surface-check`、回帰テスト `scripts/test-exec-surface-check.sh`。
#
# 使い方:
#   scripts/exec-surface-check.sh                      既定の crates/exec を走査し、cargo で rustdoc の JSON を 2 組（default / all-features）作って名前を数える
#   scripts/exec-surface-check.sh --crate-dir <dir>    <dir> の crate のソースを走査する（JSON は同様に cargo で作る）
#   scripts/exec-surface-check.sh --workspace-dir <dir>
#                                                      <dir> を cargo の作業ディレクトリにして JSON を作り、<dir>/crates/exec のソースを走査する
#   scripts/exec-surface-check.sh [--crate-dir <dir>] --json-default <path> --json-all <path>
#                                                      渡した 2 つの rustdoc の JSON だけで名前を数える（cargo を呼ばない。2 つ揃えて 1 回ずつ）
# 終了コード: 0 期待どおり（違反 0 件）/ 1 違反あり /
#   2 誤用・前提の不足・読み切れない入力（python3 が無い、未知の引数、--json-default と --json-all の片方だけ・重複、
#     crate ディレクトリが無い、.rs が 0 件、UTF-8 でない、シンボリックリンク、ディレクトリやファイルが読めない、
#     `--workspace-dir` の重複・併用（--crate-dir、--json-default/--json-all）・ディレクトリが無い、
#     その workspace に `fandhe-silicon-exec` が無い、`use std::{` の対応する `}` が無い、既定の走査でリビジョンを読めない、cargo・rustdoc が使えない、
#     rustdoc の JSON を作れない、JSON が読めない・形が崩れている・root が fandhe_silicon_exec でない、
#     `Exec` が JSON に無い、戻り値の型の path を解決できない、impl の対象を解決できない、関数の sig の形が崩れている、想定外の例外）
# 標準出力: 読んだファイルごとに `scanned: file=<crate からの相対パス>`、JSON の組ごとに
#   `scanned: features=<default|all-features> items=<crate_id 0 のアイテムの数>`、数え終えたとき（終了コード 0 と 1）だけ
#   `record: commit=<rev|-> rs_files=<n> build_rs=<yes|no> env_fs=<n> items_default=<n> items_all=<n>
#   port_names_default=<n> port_names_all=<n> port_names_missing_all=<n> op_fns=<n> op_fns_with_report=<n>
#   other_fns=<n> unlisted_fns=<n>`、合格時の `ok:` の 1 行。
#   rs_files は build.rs を含む見つけた .rs の数。commit は既定の crate を走査したときだけ HEAD
#   （変更が在れば末尾に `-dirty`）、--crate-dir と --workspace-dir のときは `-`。
# 標準エラー: 違反ごとに `violation: check=env-fs file=<相対パス> line=<n> word=<字句>`、
#   `violation: check=port-name features=default name=<名前> count=<n>`、
#   `violation: check=port-name-positive features=all-features name=<名前>`、
#   `violation: check=op-fn-count fn=<名前> found=<n>`、`violation: check=op-fn-report fn=<名前>`、
#   `violation: check=unlisted-fn owner=<型|-> fn=<名前>`（関数の走査は default の組だけ）、
#   違反があれば `FAILED: <n> violation(s)`、前提の不足は `error:`。
#
# 数え方: 対象のファイル、字句、名前、期待は、Python 部の先頭の「数え方の定義」ブロックにだけ置く。
# 検査しないもの: 構文の解釈（マクロが生成するパス、別名の取り込み）、target/ と `.` 始まりのディレクトリ、.rs 以外のファイル、
#   core・upper の公開の面、wasm32、操作でない一覧の関数の戻り値、`Err` の側の型、包んだ形（`Option<…>` など）の欄、
#   all-features の組の関数。
# JSON を作る段は専用の target ディレクトリだけに書き、cargo の出力は出さない。`RUSTC_BOOTSTRAP=1` は rustdoc を流す子プロセスにだけ渡す。
# ホストの絶対パス・環境変数の値は出力に出さない。ソースを変更しない。ネットワークを使わない。
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

# ==== 数え方の定義（数え方を変えるときはここだけを変える） ====
# 既定の対象の crate（リポジトリのルートからの相対）。
DEFAULT_CRATE_REL = "crates/exec"
# 走査するソース: crate ディレクトリの下で見つかったすべての .rs（build.rs を含む）。
BUILD_FILE = "build.rs"
SOURCE_EXT = ".rs"
# 辿らないディレクトリ名（`.` 始まりも辿らない）。
SKIP_DIRS = ("target",)
# 識別子の一部になる文字（前後がこれでないものを識別子としての一致とする）。
IDENT = "A-Za-z0-9_"
# 数えるのは各ファイルの生のテキスト（コメント・文字列を除かない）。期待は 0 件。
# 環境変数とファイルの読み取り（FB-13）: 次の字句を部分文字列としてそれぞれ独立に数える（重なりは引かない）。
ENV_FS_LITERALS = (
    "std::env", "env::var", "option_env!", "env!", "std::fs", "File::open",
    "include_str!", "include_bytes!", "include!",
)
# 加えて、`use` 文の `std::{…}`（入れ子を含む）の中に識別子として現れる語。
USE_STD_BRACE = r"\buse\s+(?:::\s*)?std\s*::\s*\{"
USE_STD_WORDS = r"(?<![%s])(env|fs)(?![%s])" % (IDENT, IDENT)
# リビジョンを読む外部コマンドの待ち時間の上限（秒）。
REV_TIMEOUT = 20
# 公開の面の名前（FB-10）: rustdoc の JSON の crate_id 0 のアイテム（種類・visibility を問わない）の名前と完全一致で数える。
PORT_NAMES = ("force_unsupported", "inject_denormal", "ForcedUnsupported", "DenormalInjection", "Injected")
# JSON を作る対象: package 名・lib の名前・target ディレクトリの下位の名前。
EXEC_PACKAGE = "fandhe-silicon-exec"
EXEC_LIB = "fandhe_silicon_exec"
TARGET_SUBDIR = "exec-surface-check"
# JSON の組: (ラベル, cargo の feature の引数)。default は期待 0 件、all-features は各名前 1 件以上。
JSON_SETS = (("default", ()), ("all-features", ("--all-features",)))
# cargo の子プロセスの待ち時間の上限（秒）。
CARGO_TIMEOUT = 580
# 操作を実行する関数（FB-22）: 持ち主の型と名前。default の組の JSON だけに当てる。
OP_OWNER = "Exec"
OP_FNS = ("f16_fma", "subgroup_sum_u32", "subgroup_sum_f32", "atomic_add",
          "external_interop_shared_memory", "hardware_ray_tracing", "unified_memory_sharing")
# 報告の型の名前。
REPORT_TYPE = "RouteReport"
# 操作でない公開の関数: (持ち主の型, 名前)。
OTHER_FNS = (("Exec", "new"), ("Exec", "deny_by_default"), ("Exec", "default_opt_in"),
            ("Exec", "query_route"), ("Outcome", "report"))
# 戻り値の外側の型として受ける paths の path。
RESULT_PATHS = (("core", "result", "Result"), ("std", "result", "Result"))
# 関数の走査を当てる組のラベル。
FN_SCAN_SET = "default"
# ==== 数え方の定義ここまで ====


class Misuse(Exception):
    """誤用・前提の不足・読み切れない入力（終了コード 2）。"""


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def collect_files(crate):
    """走査するファイルの crate からの相対パス（ソート済み、区切りは /）と、直下の build.rs の有無を返す。"""
    need(os.path.isdir(crate) and not os.path.islink(crate), "crate directory not found")
    rs = []

    def walk_error(_exc):
        raise Misuse("source directory cannot be read")

    for base, dirs, files in os.walk(crate, followlinks=False, onerror=walk_error):
        for name in dirs:
            need(not os.path.islink(os.path.join(base, name)), "symbolic link in source tree")
        dirs[:] = sorted(d for d in dirs if not d.startswith(".") and d not in SKIP_DIRS)
        for name in sorted(files):
            full = os.path.join(base, name)
            if name.endswith(SOURCE_EXT):
                need(not os.path.islink(full), "symbolic link in source tree")
                rs.append(os.path.relpath(full, crate).replace(os.sep, "/"))
    need(len(rs) > 0, "no .rs file under crate directory")
    rs.sort()
    return rs, BUILD_FILE in rs


def read_text(crate, rel):
    try:
        with open(os.path.join(crate, rel), "rb") as fh:
            return fh.read().decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        raise Misuse("source is not valid UTF-8")
    except OSError:
        raise Misuse("source cannot be read")


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def scan_env_fs(text):
    """(語, 行) の一覧。"""
    hits = []
    for lit in ENV_FS_LITERALS:
        pos = text.find(lit)
        while pos != -1:
            hits.append((lit, line_of(text, pos)))
            pos = text.find(lit, pos + 1)
    for m in re.finditer(USE_STD_BRACE, text):
        depth = 1
        i = m.end()
        while i < len(text) and depth > 0:
            ch = text[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            i += 1
        need(depth == 0, "unclosed use std brace")
        inner_start = m.end()
        inner = text[inner_start : i - 1]
        for w in re.finditer(USE_STD_WORDS, inner):
            hits.append(("std::{%s}" % w.group(1), line_of(text, inner_start + w.start())))
    return hits


def vcs_out(args):
    env = dict(os.environ)
    env["GIT_OPTIONAL_LOCKS"] = "0"
    try:
        res = subprocess.run(
            ["git"] + args,
            cwd=ROOT,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=REV_TIMEOUT,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if res.returncode != 0:
        return None
    return res.stdout.decode("utf-8", errors="replace").strip()


def tool_env():
    env = dict(os.environ)
    env["RUSTC_BOOTSTRAP"] = "1"
    return env


def run_quiet(args, env=None, timeout=CARGO_TIMEOUT, cwd=None):
    """出力を捨てて終了コードを返す。起動できなければ None。"""
    try:
        res = subprocess.run(args, cwd=cwd or ROOT, env=env, stdin=subprocess.DEVNULL,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=timeout)
    except (OSError, subprocess.SubprocessError):
        return None
    return res.returncode


def build_json_sets(cwd):
    """cargo rustdoc で JSON を組ごとに作り、ラベル -> パスの辞書を返す。"""
    need(run_quiet(["cargo", "--version"], timeout=REV_TIMEOUT, cwd=cwd) == 0, "cannot run cargo")
    try:
        meta = subprocess.run(
            ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
            cwd=cwd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            timeout=CARGO_TIMEOUT)
    except (OSError, subprocess.SubprocessError):
        raise Misuse("cargo metadata failed")
    need(meta.returncode == 0, "cargo metadata failed")
    try:
        doc = json.loads(meta.stdout.decode("utf-8"))
        target_dir = doc["target_directory"]
        pkgs = [p for p in doc["packages"] if p["name"] == EXEC_PACKAGE]
        has_lib = any("lib" in t["kind"] for p in pkgs for t in p["targets"])
    except (ValueError, KeyError, TypeError, AttributeError):
        raise Misuse("cargo metadata output is not usable")
    need(isinstance(target_dir, str) and has_lib, "exec package not found in workspace")
    need(run_quiet(["rustdoc", "-Z", "unstable-options", "--version"], env=tool_env(), timeout=REV_TIMEOUT, cwd=cwd) == 0,
          "rustdoc cannot emit JSON output")
    out_dir = os.path.join(target_dir, TARGET_SUBDIR)
    produced = os.path.join(out_dir, "doc", EXEC_LIB + ".json")
    paths = {}
    for label, feat in JSON_SETS:
        try:
            if os.path.exists(produced):
                os.remove(produced)
        except OSError:
            raise Misuse("cannot clear previous JSON output")
        cmd = (["cargo", "rustdoc", "--locked", "--offline", "--target-dir", out_dir, "-p", EXEC_PACKAGE]
                + list(feat)
                + ["--lib", "--", "-Z", "unstable-options", "--output-format", "json", "--document-hidden-items"])
        rc = run_quiet(cmd, env=tool_env(), cwd=cwd)
        need(rc == 0 and os.path.isfile(produced), "cargo rustdoc failed (%s)" % label)
        dest = os.path.join(out_dir, "doc", "%s.%s.json" % (EXEC_LIB, label))
        try:
            os.replace(produced, dest)
        except OSError:
            raise Misuse("cannot store JSON output (%s)" % label)
        paths[label] = dest
    return paths


def load_doc(path):
    """rustdoc の JSON を読み、形を検査して (crate_id 0 のアイテムの一覧, 文書全体) を返す。"""
    try:
        with open(path, "rb") as fh:
            doc = json.loads(fh.read().decode("utf-8", errors="strict"))
    except (OSError, ValueError):
        raise Misuse("cannot read JSON input")
    need(isinstance(doc, dict), "JSON top level is not an object")
    for key in ("index", "paths", "external_crates"):
        need(isinstance(doc.get(key), dict), "JSON field %s is not an object" % key)
    need("root" in doc, "JSON has no root")
    need(doc.get("includes_private") is False, "JSON includes private items")
    fv = doc.get("format_version")
    need(isinstance(fv, int) and not isinstance(fv, bool), "JSON format_version is not an integer")
    items = []
    for item in doc["index"].values():
        need(isinstance(item, dict), "JSON index entry is not an object")
        cid = item.get("crate_id")
        need(isinstance(cid, int) and not isinstance(cid, bool), "JSON item has no integer crate_id")
        if cid == 0:
            items.append(item)
    root = doc["index"].get(str(doc["root"]))
    need(isinstance(root, dict) and root.get("crate_id") == 0 and root.get("name") == EXEC_LIB,
          "root item is not %s" % EXEC_LIB)
    return items, doc


def count_port_names(items):
    """名前 -> 件数。use アイテムは inner.use.name も見る（1 アイテムは 1 回だけ数える）。"""
    counts = dict((n, 0) for n in PORT_NAMES)
    for item in items:
        names = [item.get("name")]
        inner = item.get("inner")
        if isinstance(inner, dict) and isinstance(inner.get("use"), dict):
            names.append(inner["use"].get("name"))
        for n in PORT_NAMES:
            if n in names:
                counts[n] += 1
    return counts


def resolve_id(doc, rid):
    """id を index（あれば）で引く。(index のアイテム or None, paths の path or None)。"""
    item = doc["index"].get(str(rid))
    pinfo = doc["paths"].get(str(rid))
    path = None
    if isinstance(pinfo, dict) and isinstance(pinfo.get("path"), list):
        path = tuple(pinfo["path"])
    return item, path


def is_report_item(doc, rid):
    item = doc["index"].get(str(rid))
    return (isinstance(item, dict) and item.get("crate_id") == 0 and item.get("name") == REPORT_TYPE)


def field_is_report(doc, fid):
    """欄の id が報告の型そのものを指す欄か（包んだ形は数えない）。"""
    if fid is None:
        return False
    fld = doc["index"].get(str(fid))
    if not isinstance(fld, dict):
        return False
    inner = fld.get("inner")
    ty = inner.get("struct_field") if isinstance(inner, dict) else None
    rp = ty.get("resolved_path") if isinstance(ty, dict) else None
    return isinstance(rp, dict) and is_report_item(doc, rp.get("id"))


def field_ids(kind):
    """struct の kind または enum variant の kind から欄の id の並びを返す。"""
    if isinstance(kind, dict):
        if isinstance(kind.get("tuple"), list):
            return list(kind["tuple"])
        for key in ("struct", "plain"):
            if isinstance(kind.get(key), dict) and isinstance(kind[key].get("fields"), list):
                return list(kind[key]["fields"])
    return []


def has_report_type(doc, rid):
    """型が報告を持つ型か（報告の型そのもの、または全 variant / 欄が報告の欄を持つ enum / struct）。"""
    item = doc["index"].get(str(rid))
    if not isinstance(item, dict) or item.get("crate_id") != 0:
        return False
    if item.get("name") == REPORT_TYPE:
        return True
    inner = item.get("inner")
    if not isinstance(inner, dict):
        return False
    if isinstance(inner.get("enum"), dict):
        vids = inner["enum"].get("variants")
        if not isinstance(vids, list) or not vids:
            return False
        for vid in vids:
            v = doc["index"].get(str(vid))
            vin = v.get("inner") if isinstance(v, dict) else None
            vk = vin.get("variant") if isinstance(vin, dict) else None
            kind = vk.get("kind") if isinstance(vk, dict) else None
            if not any(field_is_report(doc, f) for f in field_ids(kind)):
                return False
        return True
    if isinstance(inner.get("struct"), dict):
        return any(field_is_report(doc, f) for f in field_ids(inner["struct"].get("kind")))
    return False


def returns_report(doc, fn_item):
    """関数の戻り値が Result<報告を持つ型, _> か。"""
    fn = fn_item["inner"]["function"]
    need(isinstance(fn, dict) and isinstance(fn.get("sig"), dict), "JSON function signature is malformed")
    sig = fn["sig"]
    need("output" in sig, "JSON function signature is malformed")
    out = sig["output"]
    if out is None or not isinstance(out, dict):
        return False
    rp = out.get("resolved_path")
    if not isinstance(rp, dict):
        return False
    item, path = resolve_id(doc, rp.get("id"))
    if item is None and path is None:
        raise Misuse("return type path cannot be resolved")
    if item is not None and item.get("crate_id") == 0:
        return False
    if path is None or path not in RESULT_PATHS:
        return False
    ab = rp.get("args")
    args = ab.get("angle_bracketed", {}).get("args") if isinstance(ab, dict) else None
    if not isinstance(args, list) or len(args) != 2 or not isinstance(args[0], dict):
        return False
    ty = args[0].get("type")
    inner_rp = ty.get("resolved_path") if isinstance(ty, dict) else None
    return isinstance(inner_rp, dict) and has_report_type(doc, inner_rp.get("id"))


def scan_functions(doc):
    """(違反の一覧, op_fns, op_fns_with_report, other_fns, unlisted_fns)。"""
    index = doc["index"]
    owners = {}
    has_exec = False
    for item in index.values():
        if item.get("crate_id") != 0:
            continue
        inner = item.get("inner")
        if not isinstance(inner, dict):
            continue
        if item.get("name") == OP_OWNER and "struct" in inner:
            has_exec = True
        imp = inner.get("impl")
        if isinstance(imp, dict) and imp.get("trait") is None:
            tgt = imp.get("for")
            rp = tgt.get("resolved_path") if isinstance(tgt, dict) else None
            need(isinstance(rp, dict), "impl target cannot be resolved")
            ti, tp = resolve_id(doc, rp.get("id"))
            if isinstance(ti, dict) and isinstance(ti.get("name"), str):
                oname = ti["name"]
            elif tp:
                oname = tp[-1]
            else:
                raise Misuse("impl target cannot be resolved")
            for mid in imp.get("items") or []:
                owners[str(mid)] = oname
    need(has_exec, "%s type not found in JSON" % OP_OWNER)
    fns = []
    for key, item in index.items():
        if item.get("crate_id") != 0:
            continue
        inner = item.get("inner")
        if not isinstance(inner, dict) or "function" not in inner:
            continue
        if item.get("visibility") != "public":
            continue
        fns.append((owners.get(key, "-"), item.get("name"), item))
    viol = []
    op_total = op_rep = 0
    for name in OP_FNS:
        found = [f for f in fns if f[0] == OP_OWNER and f[1] == name]
        op_total += len(found)
        if len(found) != 1:
            viol.append("violation: check=op-fn-count fn=%s found=%d" % (name, len(found)))
        for f in found:
            if returns_report(doc, f[2]):
                op_rep += 1
            else:
                viol.append("violation: check=op-fn-report fn=%s" % name)
    listed = set((OP_OWNER, n) for n in OP_FNS) | set(OTHER_FNS)
    other = sum(1 for f in fns if (f[0], f[1]) in set(OTHER_FNS))
    unlisted = sorted((f[0], str(f[1])) for f in fns if (f[0], f[1]) not in listed)
    for o, n in unlisted:
        viol.append("violation: check=unlisted-fn owner=%s fn=%s" % (o, n))
    return viol, op_total, op_rep, other, len(unlisted)


def commit_label():
    """HEAD のコミット（変更が在れば -dirty）。取れなければ Misuse。"""
    rev = vcs_out(["rev-parse", "--verify", "HEAD^{commit}"])
    need(rev is not None and re.fullmatch(r"[0-9a-f]{40}([0-9a-f]{24})?", rev) is not None,
          "cannot resolve the source commit")
    status = vcs_out(["status", "--porcelain", "--untracked-files=normal", "--ignore-submodules=all"])
    need(status is not None, "cannot resolve the source commit")
    return rev + ("-dirty" if status else "")


def run(argv):
    parser = argparse.ArgumentParser(prog="exec-surface-check.sh", allow_abbrev=False)
    parser.add_argument("--crate-dir", default=None)
    parser.add_argument("--workspace-dir", action="append", default=[])
    parser.add_argument("--json-default", action="append", default=[])
    parser.add_argument("--json-all", action="append", default=[])
    args = parser.parse_args(argv)
    need(len(args.json_default) <= 1, "duplicate --json-default")
    need(len(args.json_all) <= 1, "duplicate --json-all")
    need(bool(args.json_default) == bool(args.json_all), "--json-default and --json-all must be given together")
    need(len(args.workspace_dir) <= 1, "duplicate --workspace-dir")
    ws = args.workspace_dir[0] if args.workspace_dir else None
    if ws is not None:
        need(args.crate_dir is None and not args.json_default and not args.json_all,
              "--workspace-dir cannot be combined with --crate-dir or --json-default/--json-all")
        need(os.path.isdir(ws) and not os.path.islink(ws), "workspace directory not found")
    default_run = args.crate_dir is None and ws is None
    if ws is not None:
        crate = os.path.join(ws, DEFAULT_CRATE_REL)
    else:
        crate = os.path.join(ROOT, DEFAULT_CRATE_REL) if default_run else args.crate_dir

    rs, has_build = collect_files(crate)
    violations = []
    env_fs_total = 0
    for rel in rs:
        text = read_text(crate, rel)
        print("scanned: file=%s" % rel)
        for word, line in scan_env_fs(text):
            env_fs_total += 1
            violations.append(("env-fs", rel, line, word))

    commit = commit_label() if default_run else "-"
    if args.json_default:
        paths = {"default": args.json_default[0], "all-features": args.json_all[0]}
    else:
        paths = build_json_sets(ws if ws is not None else ROOT)
    docs = {}
    raw = {}
    for label, _feat in JSON_SETS:
        docs[label], raw[label] = load_doc(paths[label])
        print("scanned: features=%s items=%d" % (label, len(docs[label])))
    fn_viol, op_n, op_rep, other_n, unlisted_n = scan_functions(raw[FN_SCAN_SET])
    cnt_def = count_port_names(docs["default"])
    cnt_all = count_port_names(docs["all-features"])
    default_total = sum(cnt_def.values())
    all_total = sum(cnt_all.values())
    missing = [n for n in PORT_NAMES if cnt_all[n] == 0]
    print("record: commit=%s rs_files=%d build_rs=%s env_fs=%d items_default=%d items_all=%d "
          "port_names_default=%d port_names_all=%d port_names_missing_all=%d "
          "op_fns=%d op_fns_with_report=%d other_fns=%d unlisted_fns=%d"
          % (commit, len(rs), "yes" if has_build else "no", env_fs_total,
              len(docs["default"]), len(docs["all-features"]), default_total, all_total, len(missing),
              op_n, op_rep, other_n, unlisted_n))
    for check, rel, line, word in violations:
        print("violation: check=%s file=%s line=%d word=%s" % (check, rel, line, word), file=sys.stderr)
    total = len(violations)
    for n in PORT_NAMES:
        if cnt_def[n] > 0:
            total += 1
            print("violation: check=port-name features=default name=%s count=%d" % (n, cnt_def[n]), file=sys.stderr)
    for n in missing:
        total += 1
        print("violation: check=port-name-positive features=all-features name=%s" % n, file=sys.stderr)
    for line in fn_viol:
        total += 1
        print(line, file=sys.stderr)
    if total:
        print("FAILED: %d violation(s)" % total, file=sys.stderr)
        return 1
    print("ok: exec surface check passed")
    return 0


ROOT = sys.argv[1]
try:
    code = run(sys.argv[2:])
except Misuse as exc:
    print("error: %s" % exc, file=sys.stderr)
    code = 2
except SystemExit:
    raise
except Exception:
    print("error: unexpected failure", file=sys.stderr)
    code = 2
sys.exit(code)
PY
