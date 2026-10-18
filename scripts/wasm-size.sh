#!/usr/bin/env bash
# wasm-bindgen の glue を含めた成果物の量（gzip 後の合計）を、組ごとに測って閾値と比べる
# （spec TASK-22・BUILD-42・T-12 のポインタ。親 Issue #368 の 3/6。TASK-247・親 Issue #467 の 1/4 で内訳の出力と --require-exports を足した）。
# 呼び出し元: `make wasm-size`（手元と、CI の `wasm-macos` ジョブ。CI の `deny` ジョブは wasm-bindgen の CLI が無いので呼ばない）。
# 回帰テストは `make test-wasm-size`（`scripts/test-wasm-size.sh`。合成の入力だけで流れ、CLI も Node もネットワークも要らない。CI の `deny` ジョブが呼ぶ）。
#
# 使い方:
#   scripts/wasm-size.sh                  wasm/ をビルドし、wasm-bindgen の CLI で glue を生成して測る
#   scripts/wasm-size.sh --from-dir <dir> 生成済みの出力だけを測る（cargo も CLI も呼ばない。回帰テストが使う）
#     <dir>/<ビルド設定>/<出力先>/ に glue の出力が置かれている前提。置き方は下の「組」を参照。
#   scripts/wasm-size.sh --require-exports [--from-dir <dir>]  直接実行のみ（make ターゲットは無い）。測る成果物が期待の export を持つことも確かめる（下の「export の確かめ」）。
# 終了コード: 0 全部の組が測れて閾値以下 / 1 閾値超え（ng）または未実施（not-run）の組が 1 つ以上 /
#   2 誤用・前提の不足（CLI 無し・版違い・target 無し・依存未取得を含む）・読めない入力（成功扱いにしない）
# 標準出力: 組ごとの `size:` の行（label は ok / ng / not-run。not-run には reason）、測り終えたとき（終了コード 0 と 1）だけ
#   締めの `record:` の行、合格時の `ok:` の 1 行。標準エラー: `error:`（2）・`FAILED:`（1）。
#   測れた組（label が ok / ng）は `size:` の行に生の合計 `raw_bytes=` を足し、数えたファイルごとに
#   `file: combo=… target=… name=… raw_bytes=… gzip_bytes=…` の行を出す（ファイル名だけ。量の表の数え直し用）。
#   ホストの絶対パス・環境変数の値は出力に出さない（組名とファイル名だけ）。
#
# 組: ビルド設定 4 つ（profile release・wasm-size × 命令 scalar・simd128）× 出力先 2 つ（web・bundler）= 8。
#   欠けた組は 0 バイトや合格にせず not-run にする（reason: missing-directory / missing-file / empty-file /
#   build-failed / bindgen-failed。--require-exports では missing-export / unexpected-output も）。未実施の組がある回は終了コード 1。
# 量: 数えるファイルごとに gzip（level 9・mtime 0）した長さを取り、組ごとに合計する。.d.ts・package.json などは数えない。
# export の確かめ（--require-exports のときだけ。既定の経路の振る舞いは変えない）: 下の REQUIRED_EXPORTS の関数の export が
#   各組の `<幹>_bg.wasm` の export セクションに全部あることを確かめ、`size:` の行に `exports=<見つかった数>/<期待の数>` を足す。
#   無い組は reason=missing-export の not-run。数える名前・`*.d.ts`・`package.json` のどれでもない項目（`snippets/` など）が
#   組の中にある組は reason=unexpected-output の not-run（数え漏らしを黙らせない）。理由の優先は
#   既存の not-run の理由 → unexpected-output → missing-export。読めない wasm は終了コード 2。
# 閾値: 下の LIMIT_BYTES の 1 か所だけで宣言する。引数・環境変数で変える口は持たない（緩めない）。
#
# ビルドする経路の前提（外れたら cargo を呼ぶ前に終了コード 2）: python3 3.8 以上・cargo・rustc・
#   wasm32-unknown-unknown の target・依存が手元にある（`make wasm-verify` で取得。ビルドは --locked --offline）・
#   PATH の wasm-bindgen が `wasm-bindgen 0.2.128` と厳密に一致（crate 側の版と同じ。wasm/Cargo.lock とも突き合わせる）。
#   CLI の入手は試みない。成果物は cargo の target_directory 配下の専用ディレクトリ（glue-size）に出し、製品のビルドと共有しない。
#   RUSTFLAGS は子プロセスに渡さず、CARGO_ENCODED_RUSTFLAGS で命令の指定だけを渡す（利用者の環境で量が変わらないように）。
# 検査しないもの: 実行時の振る舞い（Node での照合は別の道具）、閾値の妥当性、ソースの変更（ソースは変更しない）。
# 値の扱いはすべて Python の中で行い、シェルは入力の値に触れない。Python 3.8 以上の標準ライブラリだけ。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2
exec python3 -I - "${ROOT_DIR}" "$@" <<'PY'
import argparse
import gzip
import os
import re
import shutil
import subprocess
import sys

if sys.version_info < (3, 8):
    print("error: python 3.8 or later is required", file=sys.stderr)
    sys.exit(2)

# 閾値（gzip 後の合計のバイト数。以下なら ok）。BUILD-42・T-12 のポインタ。ここだけで宣言し、緩めない。
LIMIT_BYTES = 60000
# --require-exports で要る関数の export の名前（TASK-247 の測る成果物の公開面。ここだけで宣言し、引数・環境変数で変えない）。
REQUIRED_EXPORTS = (
    "gemm_scalar", "gemm_simd", "topk_scalar", "topk_simd",
    "elementwise_scalar", "elementwise_simd", "detect_isa_bits",
)
# crate と同じ版。CLI がこれと厳密に一致しなければ失敗にする。
BINDGEN_VERSION = "0.2.128"
BINDGEN_PACKAGE = "wasm-bindgen"
WASM_TARGET = "wasm32-unknown-unknown"
GLUE_PACKAGE = "wasm-glue"
# wasm-bindgen の出力の名前の幹（crate 名 wasm-glue から決まる）。
STEM = "wasm_glue"
# profile 名 -> target ディレクトリ内の成果物のディレクトリ名。
PROFILES = ("release", "wasm-size")
# 命令 -> CARGO_ENCODED_RUSTFLAGS。
ISAS = (("scalar", ""), ("simd128", "-C\x1ftarget-feature=+simd128"))
# 出力先 -> 数えるファイル。
TARGETS = (
    ("web", (STEM + ".js", STEM + "_bg.wasm")),
    ("bundler", (STEM + ".js", STEM + "_bg.js", STEM + "_bg.wasm")),
)
SETTINGS = tuple("%s-%s" % (p, i) for p in PROFILES for i, _ in ISAS)
WORK_SUBDIR = "glue-size"


class Misuse(Exception):
    """誤用・前提の不足・読めない入力（終了コード 2）。"""


def need(cond, msg):
    if not cond:
        raise Misuse(msg)


def is_plain_dir(path):
    return os.path.isdir(path) and not os.path.islink(path)


def read_leb_u32(buf, pos):
    """LEB128 の u32 を最大 5 バイトで読み (値, 次の位置) を返す。外れたら ValueError。"""
    value = 0
    for i in range(5):
        if pos >= len(buf):
            raise ValueError("truncated")
        b = buf[pos]
        pos += 1
        value |= (b & 0x7F) << (7 * i)
        if not b & 0x80:
            if value > 0xFFFFFFFF:
                raise ValueError("out of range")
            return value, pos
    raise ValueError("too long")


def wasm_function_exports(data):
    """wasm の export セクションにある関数（kind 0）の名前の集合を返す。境界付きで読み、外れたら ValueError。"""
    if data[:8] != b"\x00asm\x01\x00\x00\x00":
        raise ValueError("bad header")
    names = set()
    pos = 8
    n = len(data)
    while pos < n:
        sec_id = data[pos]
        size, pos = read_leb_u32(data, pos + 1)
        end = pos + size
        if end > n:
            raise ValueError("section exceeds input")
        if sec_id == 7:
            count, p = read_leb_u32(data, pos)
            for _ in range(count):
                ln, p = read_leb_u32(data, p)
                if p + ln > end:
                    raise ValueError("name exceeds section")
                name = data[p:p + ln].decode("utf-8")
                p += ln
                if p >= end:
                    raise ValueError("truncated export")
                kind = data[p]
                _, p = read_leb_u32(data, p + 1)
                if p > end:
                    raise ValueError("export exceeds section")
                if kind == 0:
                    names.add(name)
            if p != end:
                raise ValueError("trailing bytes in section")
        pos = end
    return names


def has_unexpected_entry(d, names, label):
    """数える名前・*.d.ts・package.json 以外の項目があるか。シンボリックリンクは Misuse。"""
    try:
        entries = os.listdir(d)
    except OSError:
        raise Misuse("cannot read %s" % label)
    found = False
    for e in entries:
        need(not os.path.islink(os.path.join(d, e)), "%s/%s is not a regular file" % (label, e))
        if e in names or e.endswith(".d.ts") or e == "package.json":
            continue
        found = True
    return found


def measure_combo(root, setting, target, names, require_exports=False):
    """1 組を測り (label, reason, gzip_bytes, files, raw_bytes, details, exports) を返す。
    details は数えたファイルごとの (名前, raw, gzip)。exports は見つかった期待名の数（--require-exports で解析できたときだけ）。
    読めない入力は Misuse。"""
    d = os.path.join(root, setting, target)
    if not os.path.lexists(d):
        return ("not-run", "missing-directory", None, 0, None, [], None)
    need(is_plain_dir(d), "%s/%s is not a directory" % (setting, target))
    total = 0
    raw_total = 0
    details = []
    exports = None
    reason = None
    for name in names:
        p = os.path.join(d, name)
        if not os.path.lexists(p):
            reason = reason or "missing-file"
            continue
        need(not os.path.islink(p) and os.path.isfile(p), "%s/%s/%s is not a regular file" % (setting, target, name))
        try:
            with open(p, "rb") as fh:
                data = fh.read()
        except OSError:
            raise Misuse("cannot read %s/%s/%s" % (setting, target, name))
        if len(data) == 0:
            reason = reason or "empty-file"
            continue
        gz = len(gzip.compress(data, compresslevel=9, mtime=0))
        total += gz
        raw_total += len(data)
        details.append((name, len(data), gz))
        if require_exports and name.endswith("_bg.wasm"):
            try:
                found = wasm_function_exports(data)
            except (ValueError, UnicodeDecodeError):
                raise Misuse("cannot parse %s/%s/%s as wasm" % (setting, target, name))
            exports = sum(1 for x in REQUIRED_EXPORTS if x in found)
    if reason:
        return ("not-run", reason, None, len(names), None, [], exports)
    if require_exports:
        if has_unexpected_entry(d, names, "%s/%s" % (setting, target)):
            return ("not-run", "unexpected-output", None, len(names), None, [], exports)
        if exports != len(REQUIRED_EXPORTS):
            return ("not-run", "missing-export", None, len(names), None, [], exports)
    return ("ok" if total <= LIMIT_BYTES else "ng", None, total, len(names), raw_total, details, exports)


def check_layout(root):
    need(os.path.lexists(root), "--from-dir does not exist")
    need(is_plain_dir(root), "--from-dir is not a directory")
    try:
        entries = os.listdir(root)
    except OSError:
        raise Misuse("cannot read --from-dir")
    for e in entries:
        need(e in SETTINGS, "unexpected entry in --from-dir")
        need(is_plain_dir(os.path.join(root, e)), "%s is not a directory" % e)


def measure_all(root, overrides, require_exports=False):
    check_layout(root)
    results = []
    for setting in SETTINGS:
        for target, names in TARGETS:
            if (setting, target) in overrides:
                results.append((setting, target, "not-run", overrides[(setting, target)], None, 0, None, [], None))
                continue
            label, reason, total, files, raw, details, exports = measure_combo(
                root, setting, target, names, require_exports)
            results.append((setting, target, label, reason, total, files, raw, details, exports))
    return results


def report(results):
    counts = {"ok": 0, "ng": 0, "not-run": 0}
    for setting, target, label, reason, total, files, raw, details, exports in results:
        counts[label] += 1
        line = "size: combo=%s target=%s label=%s" % (setting, target, label)
        if reason:
            line += " reason=%s" % reason
        if total is not None:
            line += " gzip_bytes=%d files=%d" % (total, files)
        line += " limit=%d" % LIMIT_BYTES
        # 既存の欄の綴りと順を保つため、足した欄は limit の後ろに置く。
        if raw is not None:
            line += " raw_bytes=%d" % raw
        if exports is not None:
            line += " exports=%d/%d" % (exports, len(REQUIRED_EXPORTS))
        print(line)
        for name, fraw, fgz in details if total is not None else ():
            print("file: combo=%s target=%s name=%s raw_bytes=%d gzip_bytes=%d" % (setting, target, name, fraw, fgz))
    print("record: combos=%d ok=%d ng=%d not_run=%d limit=%d"
          % (len(results), counts["ok"], counts["ng"], counts["not-run"], LIMIT_BYTES))
    if counts["ng"] or counts["not-run"]:
        print("FAILED: %d combo(s) over the limit, %d not measured" % (counts["ng"], counts["not-run"]), file=sys.stderr)
        return 1
    print("ok: all %d combos are within %d bytes (gzip)" % (len(results), LIMIT_BYTES))
    return 0


def run(cmd, root, env=None):
    """子プロセスを引数の並びで呼ぶ（shell=False）。出力は捨てる（ホストのパスを出さないため）。"""
    return subprocess.run(cmd, cwd=root, env=env, stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def lock_bindgen_version(root):
    path = os.path.join(root, "wasm", "Cargo.lock")
    try:
        with open(path, encoding="utf-8") as fh:
            lines = fh.read().splitlines()
    except OSError:
        raise Misuse("cannot read wasm/Cargo.lock")
    found = []
    for i, line in enumerate(lines):
        if line.strip() == 'name = "%s"' % BINDGEN_PACKAGE:
            m = re.match(r'^version = "([^"]+)"$', lines[i + 1].strip()) if i + 1 < len(lines) else None
            need(m is not None, "malformed wasm/Cargo.lock")
            found.append(m.group(1))
    need(len(found) == 1, "wasm-bindgen is not exactly once in wasm/Cargo.lock")
    need(found[0] == BINDGEN_VERSION, "wasm/Cargo.lock wasm-bindgen version differs from the expected one")


def check_prerequisites(root):
    """cargo を呼ぶ前に済ませる確認。外れたら Misuse。戻り値は cargo の target_directory。"""
    for tool in ("cargo", "rustc", "wasm-bindgen"):
        need(shutil.which(tool) is not None, "required command not found on PATH: %s" % tool)
    try:
        r = run(["wasm-bindgen", "--version"], root)
    except OSError:
        raise Misuse("cannot run wasm-bindgen")
    out = r.stdout.decode("utf-8", "replace").strip()
    m = re.match(r"^wasm-bindgen (\d+\.\d+\.\d+)$", out.splitlines()[0] if out else "")
    need(r.returncode == 0 and m is not None, "cannot read the wasm-bindgen version")
    need(m.group(1) == BINDGEN_VERSION, "wasm-bindgen version is not the expected %s" % BINDGEN_VERSION)
    lock_bindgen_version(root)
    r = run(["rustc", "--print", "sysroot"], root)
    need(r.returncode == 0, "cannot find the rust sysroot")
    sysroot = r.stdout.decode("utf-8", "replace").strip()
    need(os.path.isdir(os.path.join(sysroot, "lib", "rustlib", WASM_TARGET)), "target %s is not installed" % WASM_TARGET)
    # 依存が手元に無い（make wasm-verify で未取得）ときは、ビルドの失敗にせず前提の不足にする。
    meta = ["cargo", "metadata", "--format-version", "1", "--locked", "--offline"]
    meta += ["--filter-platform", WASM_TARGET, "--manifest-path", "wasm/Cargo.toml"]
    r = run(meta, root)
    need(r.returncode == 0, "cargo metadata failed (run `make wasm-verify` first to fetch dependencies)")
    import json
    try:
        tdir = json.loads(r.stdout.decode("utf-8"))["target_directory"]
    except (ValueError, KeyError, TypeError):
        raise Misuse("cannot read target_directory from cargo metadata")
    need(isinstance(tdir, str) and os.path.isabs(tdir), "malformed target_directory")
    return tdir


def build_and_generate(root, tdir):
    """ビルドと glue の生成。戻り値は (出力のルート, 失敗した組の理由)。"""
    work = os.path.join(tdir, WORK_SUBDIR)
    # 消す対象は target_directory 直下の専用ディレクトリだけ。
    inside = os.path.dirname(os.path.abspath(work)) == os.path.abspath(tdir)
    need(inside and os.path.basename(work) == WORK_SUBDIR, "refusing to clean an unexpected directory")
    need(not os.path.islink(work), "work directory is a symlink")
    if os.path.isdir(work):
        shutil.rmtree(work)
    out_root = os.path.join(work, "out")
    os.makedirs(out_root)
    failures = {}
    for profile in PROFILES:
        for isa, flags in ISAS:
            setting = "%s-%s" % (profile, isa)
            env = dict(os.environ)
            env.pop("RUSTFLAGS", None)
            env["CARGO_ENCODED_RUSTFLAGS"] = flags
            target_dir = os.path.join(work, "target", isa)
            build = ["cargo", "build", "--manifest-path", "wasm/Cargo.toml", "-p", GLUE_PACKAGE]
            build += ["--target", WASM_TARGET, "--locked", "--offline", "--profile", profile]
            build += ["--target-dir", target_dir]
            r = run(build, root, env)
            wasm = os.path.join(target_dir, WASM_TARGET, profile, STEM + ".wasm")
            if r.returncode != 0 or not os.path.isfile(wasm):
                for target, _ in TARGETS:
                    failures[(setting, target)] = "build-failed"
                continue
            for target, _ in TARGETS:
                dest = os.path.join(out_root, setting, target)
                os.makedirs(dest)
                r = run(["wasm-bindgen", "--target", target, "--out-dir", dest, wasm], root)
                if r.returncode != 0:
                    failures[(setting, target)] = "bindgen-failed"
    return out_root, failures


class Parser(argparse.ArgumentParser):
    def error(self, message):
        # argparse は入力の値をそのまま出すので、固定の文言に差し替える。
        print("usage: wasm-size.sh [--require-exports] [--from-dir <dir>]", file=sys.stderr)
        print("error: invalid arguments", file=sys.stderr)
        sys.exit(2)


def main():
    ap = Parser(prog="wasm-size.sh", allow_abbrev=False, add_help=False)
    ap.add_argument("root")
    ap.add_argument("--from-dir", dest="from_dir")
    ap.add_argument("--require-exports", dest="require_exports", action="count", default=0)
    args = ap.parse_args()
    if args.require_exports > 1:
        ap.error("duplicate")
    try:
        if args.from_dir is not None:
            need(args.from_dir != "", "--from-dir is empty")
            return report(measure_all(args.from_dir, {}, args.require_exports == 1))
        tdir = check_prerequisites(args.root)
        out_root, failures = build_and_generate(args.root, tdir)
        return report(measure_all(out_root, failures, args.require_exports == 1))
    except Misuse as e:
        print("error: %s" % e, file=sys.stderr)
        return 2
    except OSError:
        # ホストのパスを含む例外の文言は出さず、固定の文言にする。
        print("error: filesystem or process operation failed", file=sys.stderr)
        return 2


sys.exit(main())
PY
