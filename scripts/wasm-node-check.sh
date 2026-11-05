#!/usr/bin/env bash
# wasm32 の .wasm を wasm-bindgen の glue 越しに Node で呼び、結果をネイティブの参照と照合する土台
# （spec TASK-22 のポインタ）。行ごとに match（一致）・mismatch（不一致）・not-run（未実施）を出す。
# 呼び出し元: `make wasm-node-check`（直接実行も可）。CI では `wasm-macos` ジョブが呼ぶ（回帰テストは
# `make test-wasm-node-check`＝scripts/test-wasm-node-check.sh。CI の deny ジョブが呼ぶ）。
#
# 使い方: scripts/wasm-node-check.sh [--wasm <path>] [--wasm-simd128 <path>] [--cases <path>] [--output-dir <dir>]
#   [--record-dir <dir> --record-combination <name>]
#        scripts/wasm-node-check.sh --recount <dir>
#   --wasm  既にある scalar の .wasm を使う（cargo を呼ばない）。省略時は行が使うビルドだけを
#           `-p wasm-glue` で --profile wasm-size でビルドする（simd128 は +simd128 と別の target ディレクトリ）
#   --wasm-simd128 既にある simd128 の .wasm（TASK-994）。どちらかの口を渡したら cargo でビルドしない
#   --cases 照合の入力の JSON（既定: wasm/node/cases.json）
#   --output-dir 選んだときだけ、native-scalar の生成の行の保存の形（manifest・入力・参照・wasm の出力・outputs.json）
#           を <dir> へ新しく書く（TASK-994。書き出し先の規則は --record-dir と同じ。native-scalar の生成の行が無い入力では
#           終了コード 2）
#   --recount 保存の形のディレクトリだけから件数を数え直して `recount:` の行を出す（TASK-994。ほかの口とは併用できない。
#           python3 だけを要り、node・wasm-bindgen・cargo は要らない。終了コード 0 全行の件数が 0・1 件数が 0 でない行か
#           比べなかった行がある・2 誤用・読めない・形の不正・sha256 の違い）
#   --record-dir / --record-combination  選んだときだけ、行の結果を試験の枠の記録の部品
#           （`<行の id>.<組み合わせ>.json`）として <dir> へ書く（必ず組で渡す。既定なし）。<dir> は既存のディレクトリで、
#           ファイルは新しく作る（上書きしない）。記録は本リポに置かず、パスは引数で受ける
# 終了コード: 0 全行 match / 1 mismatch か not-run がある・行が 0 件 / 2 誤用・前提の不足（node と
#   wasm-bindgen の CLI の版違いを含む）・入力や結果が不正・Node の実行が失敗・部品の書き出しの失敗
#
# 役割の分担: wasm/node/run-glue.cjs は glue を呼んで f32 のビット列を書き出すだけで、判定は持たない。
# 判定は本スクリプトの python3（標準ライブラリだけ・-I）にあり、入力の値をシェルに展開しない。
# 行は 2 つの形（TASK-994）。固定の行（export は roundtrip_f32・detect_isa_bits。値は行に書く）と、生成の行
# （export は gemm_scalar・gemm_simd。値は host 側の道具 `wasm-parity-host gen` が作る入力とネイティブの参照で、
# 4 バイトの語ごとに完全一致で比べて件数を出す）。行の build（scalar・simd128）でどちらの glue を呼ぶかを選ぶ。
# 行の参照の出どころは必須の欄 reference_source（fixed＝固定値 / native-neon＝ネイティブの NEON /
# native-scalar＝ネイティブの scalar 版）で、
# 欠け・未知の値は終了コード 2。出どころをこの環境で作れない行（今は native-neon の全行。値があっても比べない）は
# `not-run (reference-unavailable)`、作れる出どころで参照（reference）が無い行は `not-run (reference-missing)` にする。
# どちらも一致扱いにしない。出どころを先に見る。TASK-23 のラベルへの写しは crates/core/src/rerun/reference.rs（TASK-29）。
# 保存と数え直し（TASK-994）: native-scalar の生成の行が 1 行以上ある実行は、判定のあと作業の一時ディレクトリに保存の形を
# 作り、同じ関数（recount_form）で保存の形だけから数え直して判定の件数と突き合わせる（違えば終了コード 2）。
# `recount:` の行は比べた行、`recount-uncompared:` の行は比べなかった行（呼び出しの失敗・語の数の違い）。
# 部品の書き出し（TASK-29 の追加の葉）: 判定のあと、標準出力へ印字する前に python3 の中で書く。書けなければ
# row・summary を出さずに終了コード 2 にする。行の状態から枠のラベルへの写し（ROW_LABELS）と組み合わせ → 機材の表
# （COMBINATIONS）は目印の間に置き、crates/core/tests/rerun_node_record.rs が RowState・MachineCombination と
# 突き合わせる。部品の形は RecordPart::to_json と同じ。mismatch の詳細（自由な文）は部品に入れない。
# ネットワークは使わない・npm パッケージは足さない。
# 生成物（glue・結果）は mktemp -d の一時ディレクトリにだけ出し、終了時に消す。
# bash 3.2 互換（mapfile・連想配列・空配列の "${arr[@]}" を使わない）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# 要求する Node の版（宣言点はここ 1 か所）。wasm-bindgen の CLI の版は wasm/crates/glue/Cargo.toml から読む。
NODE_EXPECTED="v24.13.0"
GLUE_MANIFEST="${ROOT_DIR}/wasm/crates/glue/Cargo.toml"
NODE_SCRIPT="${ROOT_DIR}/wasm/node/run-glue.cjs"
USAGE="usage: $(basename "$0") [--wasm <path>] [--wasm-simd128 <path>] [--cases <path>] [--output-dir <dir>] [--record-dir <dir> --record-combination <name>] | --recount <dir>"

# 相対パスは呼び出し元の cwd 基準で絶対パスにする（後でリポジトリルートへ cd しても同じファイルを指させる）。
abspath() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "${PWD}" "$1" ;;
  esac
}

# 入力の検査と判定（固定の文字列。入力のパスは引数で渡し、値はシェルに触れさせない）。
# 引数: validate <cases> <outdir|-> [dir combo] / inputs <cases> <dir> / judge <cases> <results> <inputs|-> <outputs> <saved> <outdir|-> [dir combo] / recount <dir>。終了コード 0 / 1（judge の不一致・未実施・行 0 件）/ 2（入力が不正）。
read -r -d '' PY_CODE <<'PY' || true
import hashlib
import json
import os
import re
import stat
import sys

HEX = re.compile(r"[0-9a-f]{8}\Z")
ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}\Z")
FIXED_EXPORTS = ("roundtrip_f32", "detect_isa_bits")
# 生成の行の export（TASK-994）。値は host 側の道具の入力とネイティブの参照。
GEN_EXPORTS = ("gemm_scalar", "gemm_simd")
BUILDS = ("scalar", "simd128")
# 参照の出どころ（綴りは crates/core/src/rerun/reference.rs の ReferenceSource::name に揃える）。
SOURCES = ("fixed", "native-neon", "native-scalar")
# 生成の行が取れる出どころ（fixed は値を行に書く形なので生成の行では使わない）。
GEN_SOURCES = ("native-scalar", "native-neon")
# この環境で参照を作れる出どころ（定数の 1 か所。native-scalar は host 側の道具が作る）。引数・環境変数では変えない。
PRODUCIBLE = ("fixed", "native-scalar")
FILE_NAME = re.compile(r"[a-z0-9.-]+\Z")
SHA = re.compile(r"[0-9a-f]{64}\Z")
# 行の状態 → 枠のラベル（state, 理由, label, label_reason）。RowState::from_tokens → run_result → ラベルの写しと同じにする。
ROW_LABELS = (
    # BEGIN row-labels
    ("match", None, "ok", None),
    ("mismatch", None, "ng", None),
    ("not-run", "reference-missing", "not-run", "reference-missing"),
    ("not-run", "reference-unavailable", "not-run", "reference-unavailable"),
    # END row-labels
)
# 組み合わせ → 機材（MachineCombination::ALL の順）。
COMBINATIONS = (
    # BEGIN combinations
    ("metal-mac-mini", "mac-mini"),
    ("vulkan-node-b", "node-b"),
    ("vulkan-node-a", "node-a"),
    ("vulkan-amd", "amd"),
    ("vulkan-intel", "intel"),
    ("cuda-node-b", "node-b"),
    ("cuda-node-a", "node-a"),
    # END combinations
)
# 部品の固定の欄。ビヘイビア項目の ID はこの葉の範囲のもの 1 件だけ。
BEHAVIOR_IDS = ["BUILD-41"]
BODY_KIND = "contract-test"


def die(msg):
    print("error: " + msg, file=sys.stderr)
    sys.exit(2)


def hexlist(v, what):
    if not isinstance(v, list):
        die(what + " must be a list")
    for h in v:
        if not isinstance(h, str) or not HEX.match(h):
            die(what + " has an invalid bit pattern")
    return v


def machine_of(combination):
    for name, machine in COMBINATIONS:
        if name == combination:
            return machine
    return None


def check_dir(path, what):
    # 書き出し先・読み取り元の検査（export.rs の check_dir と同じ規則）。絶対パス・`..` 無し・末尾の区切りと `.` は
    # 字句で落とす・存在する・ディレクトリ・シンボリックリンクでない。エラーに引数の値は入れない。
    if not path.startswith("/"):
        die(what + " must be absolute")
    parts = [p for p in path.split("/") if p not in ("", ".")]
    if ".." in parts:
        die(what + " must not contain a parent component")
    norm = "/" + "/".join(parts)
    try:
        st = os.lstat(norm)
    except FileNotFoundError:
        die(what + " does not exist")
    except OSError:
        die("cannot inspect " + what)
    if stat.S_ISLNK(st.st_mode):
        die(what + " must not be a symbolic link")
    if not stat.S_ISDIR(st.st_mode):
        die(what + " is not a directory")
    return norm


def record_dir(path, combination):
    if machine_of(combination) is None:
        die("unknown record combination")
    return check_dir(path, "record dir")


def part_json(rid, combination, label, reason):
    rec = {
        "behavior_ids": BEHAVIOR_IDS,
        "machine_combination": combination,
        "machine": machine_of(combination),
        "label": label,
    }
    if reason is not None:
        rec["label_reason"] = reason
    rec["body_kind"] = BODY_KIND
    rec["body"] = {"test_id": rid}
    return json.dumps(rec, separators=(",", ":"), ensure_ascii=True) + "\n"


def write_new(norm, items, what):
    # 新しいファイルだけを作る（O_EXCL。リンクは辿らない）。items は (名前, バイト列) の列。
    # 失敗したらこの実行で作ったものを全部消し、終了コード 2。作った名前の列を返す。
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
    created = []
    failed = False
    dfd = -1
    try:
        dfd = os.open(norm, os.O_RDONLY | os.O_DIRECTORY | getattr(os, "O_NOFOLLOW", 0))
        for name, data in items:
            fd = os.open(name, flags, 0o644, dir_fd=dfd)
            created.append(name)
            with os.fdopen(fd, "wb") as f:
                f.write(data)
    except OSError:
        failed = True
    if failed:
        left = False
        for name in created:
            try:
                os.unlink(name, dir_fd=dfd)
            except OSError:
                left = True
        if dfd >= 0:
            os.close(dfd)
        die("cannot write %s, nothing was kept" % what if not left else "cannot write %s and cleanup was incomplete" % what)
    os.close(dfd)
    return created


def remove_written(norm, names):
    # 先に書き終えた口の書き出しを、後の口の失敗で取り消す（この実行で作った名前だけ）。
    try:
        dfd = os.open(norm, os.O_RDONLY | os.O_DIRECTORY | getattr(os, "O_NOFOLLOW", 0))
    except OSError:
        return
    for name in names:
        try:
            os.unlink(name, dir_fd=dfd)
        except OSError:
            pass
    os.close(dfd)


def write_parts(norm, combination, outs):
    write_new(norm, [("%s.%s.json" % (rid, combination), part_json(rid, combination, label, reason).encode("ascii")) for rid, label, reason in outs], "record parts")


def load_cases(path):
    try:
        with open(path, encoding="utf-8") as f:
            d = json.load(f)
    except (OSError, ValueError):
        die("cannot read cases as JSON")
    if not isinstance(d, dict) or set(d) != {"rows"} or not isinstance(d["rows"], list):
        die("cases must be an object with a rows list")
    seen = set()
    rows = []
    for r in d["rows"]:
        if not isinstance(r, dict):
            die("a row has missing or unknown keys")
        # 行の形の判別は 1 か所。case の鍵があるか export が生成の export なら生成の行。
        gen = "case" in r or r.get("export") in GEN_EXPORTS
        if gen:
            if set(r) != {"id", "export", "build", "case", "reference_source"}:
                die("a row has missing or unknown keys")
        else:
            need = {"id", "export", "input", "reference_source"}
            if not need <= set(r) or not set(r) <= need | {"reference", "build"}:
                die("a row has missing or unknown keys")
        rid = r["id"]
        if not isinstance(rid, str) or not ID.match(rid):
            die("a row id has an invalid form")
        if rid in seen:
            die("duplicate row id: " + rid)
        seen.add(rid)
        exports = GEN_EXPORTS if gen else FIXED_EXPORTS
        if not isinstance(r["export"], str) or r["export"] not in exports:
            die("unknown export in row: " + rid)
        build = r.get("build", "scalar")
        if not isinstance(build, str) or build not in BUILDS:
            die("unknown build in row: " + rid)
        row = {"id": rid, "export": r["export"], "build": build, "gen": gen}
        if gen:
            case = r["case"]
            if not isinstance(case, str) or not ID.match(case):
                die("a row case has an invalid form")
            src = r["reference_source"]
            if not isinstance(src, str) or src not in SOURCES:
                die("unknown reference_source in row: " + rid)
            if src not in GEN_SOURCES:
                die("reference_source not allowed in row: " + rid)
            row.update(case=case, src=src, inp=None, ref=None)
        else:
            inp = hexlist(r["input"], "input")
            if r["export"] == "detect_isa_bits" and inp:
                die("detect_isa_bits takes no input in row: " + rid)
            src = r["reference_source"]
            if not isinstance(src, str) or src not in SOURCES:
                die("unknown reference_source in row: " + rid)
            if src == "native-scalar":
                die("reference_source not allowed in row: " + rid)
            ref = r.get("reference")
            if ref is not None:
                ref = hexlist(ref, "reference")
            row.update(case=None, src=src, inp=inp, ref=ref)
        rows.append(row)
    return rows


def load_manifest(dirpath, rows):
    # host 側の道具の出力（manifest.json と .bin）を読んで照らす。ファイル名は文字種と `..`・先頭の `.` で拒否し、
    # 通常のファイルであること・sha256・長さを確かめる。エラーに引数のパスの値は入れない。
    def name_ok(n):
        return isinstance(n, str) and FILE_NAME.match(n) is not None and ".." not in n and not n.startswith(".")

    try:
        with open(os.path.join(dirpath, "manifest.json"), encoding="utf-8") as f:
            m = json.load(f)
    except (OSError, ValueError):
        die("cannot read manifest")
    if not isinstance(m, dict) or set(m) != {"cases"} or not isinstance(m["cases"], list):
        die("manifest must be an object with a cases list")
    keys = {"name", "kind", "m", "k", "n", "a_file", "b_file", "native_scalar_file", "a_sha256", "b_sha256", "native_scalar_sha256"}
    cases = {}
    for c in m["cases"]:
        if not isinstance(c, dict) or set(c) != keys:
            die("manifest case has missing or unknown keys")
        name = c["name"]
        if not isinstance(name, str) or not ID.match(name) or name in cases:
            die("manifest case name is invalid or repeated")
        if c["kind"] != "gemm":
            die("manifest case kind is unknown")
        dims = []
        for key in ("m", "k", "n"):
            v = c[key]
            if not isinstance(v, int) or isinstance(v, bool) or v <= 0:
                die("manifest case dimension is invalid")
            dims.append(v)
        mm, kk, nn = dims
        files = {}
        for fk, hk, words in (("a_file", "a_sha256", mm * kk), ("b_file", "b_sha256", kk * nn), ("native_scalar_file", "native_scalar_sha256", mm * nn)):
            fname, want = c[fk], c[hk]
            if not name_ok(fname) or not isinstance(want, str) or not SHA.match(want):
                die("manifest file entry is invalid")
            full = os.path.join(dirpath, fname)
            try:
                st = os.lstat(full)
                if not stat.S_ISREG(st.st_mode):
                    die("manifest file is not a regular file")
                with open(full, "rb") as f:
                    data = f.read()
            except OSError:
                die("cannot read a manifest file")
            if hashlib.sha256(data).hexdigest() != want:
                die("manifest sha256 mismatch")
            if len(data) != words * 4:
                die("manifest file length mismatch")
            files[fk] = (full, want)
        cases[name] = {"m": mm, "k": kk, "n": nn, "files": files}
    for r in rows:
        if r["gen"] and r["case"] not in cases:
            die("unknown case in row: " + r["id"])
    return m["cases"], cases


def load_results(path, rows, outputs):
    ids = {r["id"] for r in rows}
    gen = {r["id"]: r["gen"] for r in rows}
    srcs = {r["id"]: r["src"] for r in rows}
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except (OSError, ValueError):
        die("cannot read results")
    res = {}
    for ln in lines:
        try:
            o = json.loads(ln)
        except ValueError:
            die("results line is not JSON")
        if not isinstance(o, dict) or not isinstance(o.get("id"), str) or o["id"] not in ids:
            die("results line has an unknown or missing id")
        rid = o["id"]
        if rid in res:
            die("results repeat an id")
        if set(o) == {"id", "output"} and not gen[rid]:
            res[rid] = ("output", hexlist(o["output"], "output"))
        elif set(o) == {"id", "output_file"} and gen[rid] and o["output_file"] == rid + ".bin":
            # 出力のパスは結果の値を使わず id から組み立てる。
            full = os.path.join(outputs, rid + ".bin")
            try:
                if not stat.S_ISREG(os.lstat(full).st_mode):
                    die("an output file is not a regular file")
                with open(full, "rb") as f:
                    data = f.read()
            except OSError:
                die("cannot read an output file")
            if srcs[rid] == "native-scalar" and len(data) % 4 != 0:
                die("an output file length is not a multiple of 4")
            res[rid] = ("file", data)
        elif set(o) == {"id", "error"} and isinstance(o["error"], str):
            res[rid] = ("error", o["error"])
        else:
            die("results line has an unexpected shape")
    if set(res) != ids:
        die("results lack a row")
    return res


def case_sha_lines(manifest_cases, used_cases):
    # ケースごとの入力・参照の sha256 の行（manifest のケースの順）。
    out = []
    for c in manifest_cases:
        if c["name"] in used_cases:
            out.append("sha256: %s.a %s" % (c["name"], c["a_sha256"]))
            out.append("sha256: %s.b %s" % (c["name"], c["b_sha256"]))
            out.append("sha256: %s.native-scalar %s" % (c["name"], c["native_scalar_sha256"]))
    return out


def recount_form(d):
    # 保存の形のディレクトリだけから件数を数え直す（TASK-994）。ディレクトリの中身を信用せず、形・名前・sha256 を
    # 照らす。読めない・形の不正・sha256 の違いは終了コード 2。想定外の例外も 2 に寄せる（1 と区別するため）。
    try:
        return recount_form_inner(d)
    except Exception:
        die("cannot read saved directory")


def recount_form_inner(d):
    try:
        with open(os.path.join(d, "outputs.json"), encoding="utf-8") as f:
            o = json.load(f)
    except (OSError, ValueError):
        die("cannot read outputs.json")
    if not isinstance(o, dict) or set(o) != {"rows"} or not isinstance(o["rows"], list) or not o["rows"]:
        die("outputs.json must be an object with a non-empty rows list")
    keys = {"id", "export", "build", "case", "output_file", "output_sha256"}
    rows = []
    seen = set()
    for e in o["rows"]:
        if not isinstance(e, dict) or set(e) != keys:
            die("outputs.json row has missing or unknown keys")
        rid = e["id"]
        if not isinstance(rid, str) or not ID.match(rid) or rid in seen:
            die("outputs.json row id is invalid or repeated")
        seen.add(rid)
        if e["export"] not in GEN_EXPORTS or e["build"] not in BUILDS:
            die("outputs.json row has an unknown export or build")
        if not isinstance(e["case"], str) or not ID.match(e["case"]):
            die("outputs.json row case is invalid")
        of, osha = e["output_file"], e["output_sha256"]
        if of is None:
            if osha is not None:
                die("outputs.json row output entry is inconsistent")
        elif of != rid + ".bin" or not isinstance(osha, str) or not SHA.match(osha):
            die("outputs.json row output entry is invalid")
        rows.append({"id": rid, "case": e["case"], "gen": True, "has_out": of is not None, "sha": osha})
    manifest_cases, cases = load_manifest(d, rows)
    expected = ["manifest.json", "outputs.json"]
    for c in manifest_cases:
        expected += [c["a_file"], c["b_file"], c["native_scalar_file"]]
    expected += [r["id"] + ".bin" for r in rows if r["has_out"]]
    if len(set(expected)) != len(expected) or set(os.listdir(d)) != set(expected):
        die("saved directory has missing, unexpected or repeated files")
    for name in expected:
        if not stat.S_ISREG(os.lstat(os.path.join(d, name)).st_mode):
            die("saved directory entry is not a regular file")
    compared = {}
    uncompared = set()
    rec_lines = []
    sha_lines = []
    for r in rows:
        rid = r["id"]
        if not r["has_out"]:
            uncompared.add(rid)
            rec_lines.append("recount-uncompared: %s" % rid)
            continue
        with open(os.path.join(d, rid + ".bin"), "rb") as f:
            data = f.read()
        if hashlib.sha256(data).hexdigest() != r["sha"]:
            die("saved output sha256 mismatch")
        if len(data) % 4 != 0:
            die("saved output length is not a multiple of 4")
        with open(cases[r["case"]]["files"]["native_scalar_file"][0], "rb") as f:
            want = f.read()
        if len(data) != len(want):
            uncompared.add(rid)
            rec_lines.append("recount-uncompared: %s" % rid)
            continue
        diff = sum(1 for i in range(0, len(want), 4) if data[i:i + 4] != want[i:i + 4])
        compared[rid] = (diff, len(want) // 4)
        rec_lines.append("recount: %s mismatches=%d n=%d" % (rid, diff, len(want) // 4))
        sha_lines.append("sha256: %s %s" % (rid, r["sha"]))
    sha_lines += case_sha_lines(manifest_cases, {r["case"] for r in rows})
    zero = sum(1 for v in compared.values() if v[0] == 0)
    summary = "recount-summary: rows=%d zero=%d nonzero=%d uncompared=%d" % (len(rows), zero, len(compared) - zero, len(uncompared))
    return {"compared": compared, "uncompared": uncompared, "rec_lines": rec_lines, "sha_lines": sha_lines, "summary": summary}


def form_items(inputs, manifest_cases, form_rows):
    # 保存の形のファイル（名前 → バイト列）。manifest と入力・参照はそのまま写し、wasm の出力は native-scalar の
    # 生成の行のものだけを入れる。
    items = {}

    def put(name, data):
        if name in items:
            die("saved form name collision")
        items[name] = data

    with open(os.path.join(inputs, "manifest.json"), "rb") as f:
        put("manifest.json", f.read())
    for c in manifest_cases:
        for fk in ("a_file", "b_file", "native_scalar_file"):
            with open(os.path.join(inputs, c[fk]), "rb") as f:
                put(c[fk], f.read())
    out_rows = []
    for rid, r, kind, val in form_rows:
        has = kind == "file"
        if has:
            put(rid + ".bin", val)
        out_rows.append({
            "id": rid,
            "export": r["export"],
            "build": r["build"],
            "case": r["case"],
            "output_file": rid + ".bin" if has else None,
            "output_sha256": hashlib.sha256(val).hexdigest() if has else None,
        })
    put("outputs.json", (json.dumps({"rows": out_rows}, indent=2, ensure_ascii=True) + "\n").encode("ascii"))
    return list(items.items())


def main():
    mode = sys.argv[1]
    if mode == "recount":
        res = recount_form(check_dir(sys.argv[2], "saved dir"))
        for ln in res["rec_lines"] + res["sha_lines"] + [res["summary"]]:
            print(ln)
        return 0 if not any(v[0] for v in res["compared"].values()) and not res["uncompared"] else 1
    rows = load_cases(sys.argv[2])
    if mode == "validate":
        # 書き出しを選んだときだけ組み合わせと書き出し先を検査する。要るビルドと生成の入力の有無を語で返す。
        if sys.argv[3] != "-":
            check_dir(sys.argv[3], "output dir")
            if not any(r["gen"] and r["src"] == "native-scalar" for r in rows):
                die("output dir needs native-scalar generated rows")
        if sys.argv[4:]:
            record_dir(sys.argv[4], sys.argv[5])
        needs = [b for b in BUILDS if any(r["build"] == b for r in rows)]
        if any(r["gen"] for r in rows):
            needs.append("generated")
        for n in needs:
            print(n)
        return 0
    if mode == "inputs":
        load_manifest(sys.argv[3], rows)
        return 0
    results_path, inputs, outputs, saved, out_arg = sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6], sys.argv[7]
    extra = sys.argv[8:]
    norm_out = None
    if out_arg != "-":
        norm_out = check_dir(out_arg, "output dir")
    norm = None
    if extra:
        norm = record_dir(extra[0], extra[1])
    manifest_cases = []
    cases = {}
    if any(r["gen"] for r in rows):
        if inputs == "-":
            die("generated rows need an inputs directory")
        manifest_cases, cases = load_manifest(inputs, rows)
    res = load_results(results_path, rows, outputs)
    counts = {"match": 0, "mismatch": 0, "not-run": 0}
    lines = []
    outs = []
    cmp_lines = []
    used_cases = set()
    # native-scalar の生成の行の保存と数え直しの突き合わせ用。
    form_rows = []
    j_compared = {}
    j_uncompared = set()
    for r in rows:
        rid, ref, src = r["id"], r["ref"], r["src"]
        kind, val = res[rid]
        detail = ""
        if r["gen"] and src == "native-scalar":
            used_cases.add(r["case"])
            form_rows.append((rid, r, kind, val))
            j_uncompared.add(rid)
        if src not in PRODUCIBLE:
            state = "not-run"
            detail = "reference-unavailable"
        elif not r["gen"] and ref is None:
            state = "not-run"
            detail = "reference-missing"
        elif kind == "error":
            state = "mismatch"
            detail = "call error"
        elif r["gen"]:
            with open(cases[r["case"]]["files"]["native_scalar_file"][0], "rb") as f:
                want = f.read()
            if len(val) != len(want):
                state = "mismatch"
                detail = "length expected %d got %d" % (len(want) // 4, len(val) // 4)
            else:
                nwords = len(want) // 4
                diff = sum(1 for i in range(0, len(want), 4) if want[i:i + 4] != val[i:i + 4])
                state = "match" if diff == 0 else "mismatch"
                if diff:
                    detail = "mismatches %d/%d" % (diff, nwords)
                cmp_lines.append("count: %s mismatches=%d n=%d" % (rid, diff, nwords))
                cmp_lines.append("sha256: %s %s" % (rid, hashlib.sha256(val).hexdigest()))
                if src == "native-scalar":
                    j_compared[rid] = (diff, nwords)
                    j_uncompared.discard(rid)
        elif val == ref:
            state = "match"
        else:
            state = "mismatch"
            if len(val) != len(ref):
                detail = "length expected %d got %d" % (len(ref), len(val))
            else:
                detail = "first difference at index %d" % next(i for i in range(len(ref)) if val[i] != ref[i])
        counts[state] += 1
        lines.append("row: %s %s%s" % (rid, state, " (" + detail + ")" if detail else ""))
        reason = detail if state == "not-run" else None
        for st, rs, label, label_reason in ROW_LABELS:
            if st == state and rs == reason:
                outs.append((rid, label, label_reason))
        if len(outs) != len(lines):
            die("no label for a row state")
    rec_extra = []
    written = []
    if form_rows:
        # 保存の形を作業の一時ディレクトリへ書き、保存の形だけから数え直して判定の件数と突き合わせる。
        items = form_items(inputs, manifest_cases, form_rows)
        saved_abs = os.path.abspath(saved)
        write_new(saved_abs, items, "saved form")
        rc_res = recount_form(saved_abs)
        if rc_res["compared"] != j_compared or rc_res["uncompared"] != j_uncompared:
            die("recount disagrees")
        rec_extra = rc_res["rec_lines"] + [rc_res["summary"]]
        if norm_out is not None:
            written = write_new(norm_out, items, "saved outputs")
    if norm is not None and rows:
        try:
            write_parts(norm, extra[1], outs)
        except SystemExit:
            if norm_out is not None:
                remove_written(norm_out, written)
            raise
    for ln in lines:
        print(ln)
    for ln in cmp_lines:
        print(ln)
    for ln in case_sha_lines(manifest_cases, used_cases):
        print(ln)
    for ln in rec_extra:
        print(ln)
    print("summary: rows=%d match=%d mismatch=%d not-run=%d" % (len(rows), counts["match"], counts["mismatch"], counts["not-run"]))
    if not rows:
        print("error: no rows", file=sys.stderr)
        return 1
    return 0 if counts["match"] == len(rows) else 1


sys.exit(main())
PY

die2() {
  echo "error: $1" >&2
  exit 2
}

# 前提のコマンドが無いときは成功扱いにせず終了コード 2 にする（導入は試みない）。
need() {
  command -v "$1" >/dev/null 2>&1 || die2 "required command not found on PATH: $1"
}

main() {
  local wasm_arg="" wasm_simd_arg="" cases="${ROOT_DIR}/wasm/node/cases.json"
  local rec_dir="" rec_combo="" have_dir=0 have_combo=0
  local out_dir="" recount_dir="" others=0
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --wasm)
        [ "$#" -ge 2 ] || { echo "${USAGE}" >&2; exit 2; }
        wasm_arg="$(abspath "$2")"
        others=1
        shift 2
        ;;
      --wasm-simd128)
        [ "$#" -ge 2 ] || { echo "${USAGE}" >&2; exit 2; }
        wasm_simd_arg="$(abspath "$2")"
        others=1
        shift 2
        ;;
      --cases)
        [ "$#" -ge 2 ] || { echo "${USAGE}" >&2; exit 2; }
        cases="$(abspath "$2")"
        others=1
        shift 2
        ;;
      --record-dir)
        [ "$#" -ge 2 ] && [ -n "$2" ] || { echo "${USAGE}" >&2; exit 2; }
        rec_dir="$(abspath "$2")"
        have_dir=1
        others=1
        shift 2
        ;;
      --record-combination)
        [ "$#" -ge 2 ] || { echo "${USAGE}" >&2; exit 2; }
        rec_combo="$2"
        have_combo=1
        others=1
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] && [ -n "$2" ] || { echo "${USAGE}" >&2; exit 2; }
        out_dir="$(abspath "$2")"
        others=1
        shift 2
        ;;
      --recount)
        [ "$#" -ge 2 ] && [ -n "$2" ] || { echo "${USAGE}" >&2; exit 2; }
        recount_dir="$(abspath "$2")"
        shift 2
        ;;
      *)
        echo "${USAGE}" >&2
        exit 2
        ;;
    esac
  done
  # 書き出しは 2 つを組で選ぶ（片方だけは誤用）。
  [ "${have_dir}" = "${have_combo}" ] || { echo "${USAGE}" >&2; exit 2; }

  # 数え直しは保存の形だけを読む。ほかの口とは併用できず、node・wasm-bindgen・cargo を要らない。
  if [ -n "${recount_dir}" ]; then
    [ "${others}" = 0 ] || { echo "${USAGE}" >&2; exit 2; }
    need python3
    local rrc=0
    python3 -I -c "${PY_CODE}" recount "${recount_dir}" || rrc=$?
    exit "${rrc}"
  fi

  need node
  need wasm-bindgen
  need python3
  if [ -z "${wasm_arg}" ] && [ -z "${wasm_simd_arg}" ]; then
    need cargo
    need jq
  fi

  # 版は完全一致で要求する（前方一致・rc 付き・別版は失敗）。.wasm のビルドより前に確かめる。
  local node_actual wb_expected wb_actual
  node_actual="$(node --version)" || die2 "node --version failed"
  [ "${node_actual}" = "${NODE_EXPECTED}" ] || die2 "node version mismatch: expected ${NODE_EXPECTED}, got ${node_actual}"
  wb_expected="$(sed -n 's/^wasm-bindgen = "=\([0-9][0-9.]*\)".*/\1/p' "${GLUE_MANIFEST}" | head -n 1)"
  [ -n "${wb_expected}" ] || die2 "cannot read the wasm-bindgen version from ${GLUE_MANIFEST}"
  wb_actual="$(wasm-bindgen --version)" || die2 "wasm-bindgen --version failed"
  [ "${wb_actual}" = "wasm-bindgen ${wb_expected}" ] || die2 "wasm-bindgen version mismatch: expected wasm-bindgen ${wb_expected}, got ${wb_actual}"
  echo "node: ${node_actual}"
  echo "wasm-bindgen: ${wb_expected}"

  # 入力を検査し、要るビルドと生成の入力の有無を語で受ける（書き出しを選んだときは組み合わせと書き出し先も、
  # .wasm のビルドより前に検査する。${rec_dir:+...} は選ばないと 0 語）。語は標準出力に出さない。
  local needs
  needs="$(python3 -I -c "${PY_CODE}" validate "${cases}" "${out_dir:--}" ${rec_dir:+"${rec_dir}" "${rec_combo}"})" || exit "$?"
  local need_scalar=0 need_simd=0 need_gen=0 w
  for w in ${needs}; do
    case "${w}" in
      scalar) need_scalar=1 ;;
      simd128) need_simd=1 ;;
      generated) need_gen=1 ;;
    esac
  done
  if [ "${need_gen}" = 1 ]; then
    need cargo
  fi

  # 一時ディレクトリは cargo の後始末も含めて、ここから使う（EXIT の trap から参照するので関数のローカルにしない）。
  work=""
  work="$(mktemp -d "${TMPDIR:-/tmp}/wasm-node-check.XXXXXX")"
  trap 'rm -rf "${work:?}"' EXIT

  # 生成の行があるときだけ、host 側の道具で入力とネイティブの参照を作って照らす（TASK-994）。
  local inputs="-"
  if [ "${need_gen}" = 1 ]; then
    inputs="${work}/inputs"
    mkdir "${inputs}"
    (cd "${ROOT_DIR}" && cargo run --manifest-path wasm/Cargo.toml -p wasm-parity-host --locked --quiet -- gen "${inputs}" >&2) || die2 "host tool failed"
    python3 -I -c "${PY_CODE}" inputs "${cases}" "${inputs}" || exit "$?"
  fi

  # 行が使うビルドの .wasm。口を渡さなければ、使うビルドだけをここで作る。
  local wasm_scalar="" wasm_simd="" target_dir=""
  if [ -z "${wasm_arg}" ] && [ -z "${wasm_simd_arg}" ] && { [ "${need_scalar}" = 1 ] || [ "${need_simd}" = 1 ]; }; then
    cd "${ROOT_DIR}"
    target_dir="$(cargo metadata --format-version 1 --no-deps --locked --manifest-path wasm/Cargo.toml | jq -r '.target_directory')"
    if [ "${need_scalar}" = 1 ]; then
      log "cargo build (wasm-glue, wasm32-unknown-unknown, profile wasm-size, scalar)"
      env -u RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS -u CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS CARGO_ENCODED_RUSTFLAGS= \
        cargo build --manifest-path wasm/Cargo.toml -p wasm-glue --locked \
        --target wasm32-unknown-unknown --profile wasm-size >&2
      wasm_scalar="${target_dir}/wasm32-unknown-unknown/wasm-size/wasm_glue.wasm"
    fi
    if [ "${need_simd}" = 1 ]; then
      log "cargo build (wasm-glue, wasm32-unknown-unknown, profile wasm-size, simd128)"
      env -u RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS -u CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS \
        CARGO_ENCODED_RUSTFLAGS="$(printf -- '-C\037target-feature=+simd128')" \
        cargo build --manifest-path wasm/Cargo.toml -p wasm-glue --locked \
        --target wasm32-unknown-unknown --profile wasm-size --target-dir "${target_dir}/node-simd128" >&2
      wasm_simd="${target_dir}/node-simd128/wasm32-unknown-unknown/wasm-size/wasm_glue.wasm"
    fi
  else
    wasm_scalar="${wasm_arg}"
    wasm_simd="${wasm_simd_arg}"
    [ "${need_scalar}" = 0 ] || [ -n "${wasm_scalar}" ] || die2 "a scalar wasm file is required (--wasm)"
    [ "${need_simd}" = 0 ] || [ -n "${wasm_simd}" ] || die2 "a simd128 wasm file is required (--wasm-simd128)"
  fi
  local glue_scalar="-" glue_simd="-" wf base
  if [ "${need_scalar}" = 1 ]; then
    wf="${wasm_scalar}"
    [ -s "${wf}" ] || die2 "missing or empty wasm file: ${wf}"
    mkdir "${work}/glue-scalar"
    wasm-bindgen --target nodejs --out-dir "${work}/glue-scalar" "${wf}" >&2 || die2 "wasm-bindgen failed"
    base="$(basename "${wf}" .wasm)"
    glue_scalar="${work}/glue-scalar/${base}.js"
    [ -s "${glue_scalar}" ] || die2 "glue was not generated: ${base}.js"
  fi
  if [ "${need_simd}" = 1 ]; then
    wf="${wasm_simd}"
    [ -s "${wf}" ] || die2 "missing or empty wasm file: ${wf}"
    mkdir "${work}/glue-simd128"
    wasm-bindgen --target nodejs --out-dir "${work}/glue-simd128" "${wf}" >&2 || die2 "wasm-bindgen failed"
    base="$(basename "${wf}" .wasm)"
    glue_simd="${work}/glue-simd128/${base}.js"
    [ -s "${glue_simd}" ] || die2 "glue was not generated: ${base}.js"
  fi

  local results="${work}/results.jsonl" outputs="${work}/outputs" saved="${work}/saved"
  mkdir "${outputs}" "${saved}"
  node "${NODE_SCRIPT}" "${glue_scalar}" "${cases}" "${results}" "${glue_simd}" "${inputs}" "${outputs}" >&2 || die2 "node run failed"

  local rc=0
  python3 -I -c "${PY_CODE}" judge "${cases}" "${results}" "${inputs}" "${outputs}" "${saved}" "${out_dir:--}" ${rec_dir:+"${rec_dir}" "${rec_combo}"} || rc=$?
  exit "${rc}"
}

main "$@"
