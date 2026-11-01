#!/usr/bin/env bash
# テクスチャの形式の問合せの鍵の表の突き合わせの検査（spec TASK-1313 のポインタ。親 Issue #1015 の 2/4）。
# core の表（`crates/core/examples/texture_format_keys.rs` の出力）を、機能一覧の CSV と
# 実行時列挙の記録の JSON に突き合わせる。一覧の既定のパスは `scripts/inventory-trace-check.sh`
# （TASK-16 のポインタ）と同じ。
# 入力のポインタ: 一覧は docs/spec/03-poc/wgpu-feature-inventory/inventory.csv（private の submodule の中）。
# JSON の置き場と中の鍵の名前は private spec 側の記録に従う（ポインタのみ）。
# 呼び出し元: 直接の実行だけ（本体の make ターゲットは持たない。CI は呼ばない）。
#
# 使い方:
#   scripts/texture-format-key-check.sh --enumerate-json PATH --enumerate-list KEY --enumerate-item KEY
#                                       [--table PATH] [--csv PATH] [--key-prefix STR] [--show-keys]
#   --table           3 列（row / 鍵 / 形式の Debug）の TSV。省略時は example を cargo で実行して得る
#   --csv             機能一覧の CSV（既定: 上のポインタのパス）。先頭のレコードを見出しとし、先頭の列を鍵の列にする
#   --key-prefix      CSV の鍵がこれで始まる行だけを対象にする（既定: texformat:）
#   --enumerate-json  実行時列挙の記録の JSON（既定値なし。3 つとも必須）
#   --enumerate-list  JSON の最上位にある列挙の配列の鍵の名前
#   --enumerate-item  配列の要素の中の形式の綴りの鍵の名前
#   --show-keys       違反の診断に鍵・綴りの文字列も出す（既定は件数と行番号だけ）
#
# 判定:
#   (a) 表の形: 各行が 3 列・row と形式が空でない・鍵と形式がそれぞれ互いに異なる
#   (b) 一覧との対応: CSV の対象の行の鍵の集合と、表の row に接頭辞を付けた集合が過不足なく一致（CSV の鍵の重複も違反）
#   (c) 列挙との対応: 列挙の綴りの集合と表の形式の集合が過不足なく一致（重複も違反）
# 空の表・空の CSV・対象の行が 0 件の CSV・空の列挙・読めない入力も違反（1）。
#
# 終了コード:
#   0  合格（標準出力に件数の 1 行）
#   1  違反（読めない・空の入力を含む）
#   2  誤用・前提の不足（未知の引数・入力が無い・cargo が無い・JSON の 3 引数の欠け）
#
# 入力は読むだけ。ファイルを作らず、ネットワークを使わない。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2

# 引用符付きの heredoc でシェルの展開を通さない。既定の CSV のパスと ROOT を先頭の位置引数で渡す。
exec python3 -I - \
  "${ROOT_DIR}/docs/spec/03-poc/wgpu-feature-inventory/inventory.csv" \
  "${ROOT_DIR}" \
  "$@" <<'PY'
import argparse
import csv
import json
import os
import shutil
import subprocess
import sys

PROG = "texture-format-key-check.sh"


class Usage(Exception):
    pass


class Unreadable(Exception):
    pass


def err(msg):
    print("error: " + msg, file=sys.stderr)


def get_table_text(args):
    """表の TSV の文字列を返す。--table が無ければ example を cargo で実行して得る。"""
    if args.table is not None:
        with open(args.table, "rb") as f:
            return f.read().decode("utf-8")
    cargo = shutil.which("cargo")
    if cargo is None:
        raise Usage("cargo not found (needed when --table is not given)")
    proc = subprocess.run(
        [cargo, "run", "-q", "-p", "fandhe-silicon-core", "--example", "texture_format_keys"],
        cwd=args.root, capture_output=True, stdin=subprocess.DEVNULL,
    )
    if proc.returncode != 0:
        raise Usage("the table example failed to run")
    return proc.stdout.decode("utf-8")


def run(args):
    if args.enumerate_json is None or args.enumerate_list is None or args.enumerate_item is None:
        raise Usage("--enumerate-json, --enumerate-list and --enumerate-item are all required")
    if args.table is not None and not os.path.isfile(args.table):
        raise Usage("table file not found: %s" % args.table)
    if not os.path.isfile(args.csv):
        raise Usage("input CSV not found: %s" % args.csv)
    if not os.path.isfile(args.enumerate_json):
        raise Usage("enumeration JSON not found: %s" % args.enumerate_json)
    sk = args.show_keys

    try:
        text = get_table_text(args)
    except (UnicodeDecodeError, OSError):
        err("cannot read the table")
        return 1
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    if not lines:
        err("the table is empty")
        return 1

    try:
        with open(args.csv, encoding="utf-8-sig", newline="") as f:
            reader = csv.reader(f, strict=True)
            header = next(reader, None)
            csv_rows = [(reader.line_num, r) for r in reader]
    except (UnicodeDecodeError, csv.Error, OSError):
        err("cannot parse the input CSV")
        return 1
    if header is None:
        err("the input CSV is empty")
        return 1

    try:
        with open(args.enumerate_json, encoding="utf-8") as f:
            doc = json.load(f)
    except (UnicodeDecodeError, ValueError, OSError):
        err("cannot parse the enumeration JSON")
        return 1
    if not isinstance(doc, dict) or not isinstance(doc.get(args.enumerate_list), list):
        err("the enumeration JSON has no list under the given list key")
        return 1
    spellings = []
    for idx, el in enumerate(doc[args.enumerate_list]):
        if not isinstance(el, dict) or not isinstance(el.get(args.enumerate_item), str):
            err("enumeration item %d has no string under the given item key" % idx)
            return 1
        spellings.append((idx, el[args.enumerate_item]))
    if not spellings:
        err("the enumeration is empty")
        return 1

    violations = 0

    # (a) 表の形
    table = []  # (行番号, row, key, format)
    seen_keys, seen_formats = {}, {}
    for n, line in enumerate(lines, 1):
        cols = line.split("\t")
        if len(cols) != 3:
            err("table line %d does not have 3 columns" % n)
            violations += 1
            continue
        row, key, fmt = cols
        ok = True
        for what, v in (("row", row), ("key", key), ("format", fmt)):
            if v == "":
                err("empty %s at table line %d" % (what, n))
                violations += 1
                ok = False
        if key and key in seen_keys:
            err("duplicate key at table line %d (first at line %d)%s"
                % (n, seen_keys[key], ": " + key if sk else ""))
            violations += 1
            ok = False
        elif key:
            seen_keys[key] = n
        if fmt and fmt in seen_formats:
            err("duplicate format at table line %d (first at line %d)%s"
                % (n, seen_formats[fmt], ": " + fmt if sk else ""))
            violations += 1
            ok = False
        elif fmt:
            seen_formats[fmt] = n
        if ok:
            table.append((n, row, key, fmt))
    if not table and violations == 0:
        err("the table has no usable rows")
        return 1

    # 表の row（初出順）と row ごとの鍵の数。
    row_order, row_count, row_line = [], {}, {}
    for n, row, _key, _fmt in table:
        if row not in row_count:
            row_order.append(row)
            row_count[row] = 0
            row_line[row] = n
        row_count[row] += 1

    # (b) 機能一覧との対応
    targeted, seen_csv = [], {}
    for line, rec in csv_rows:
        if not rec:
            continue
        if len(rec) != len(header):
            err("column count differs from the header at CSV line %d" % line)
            violations += 1
            continue
        if not rec[0].startswith(args.key_prefix):
            continue
        k = rec[0]
        if k in seen_csv:
            err("duplicate CSV key at line %d (first at line %d)%s"
                % (line, seen_csv[k], ": " + k if sk else ""))
            violations += 1
            continue
        seen_csv[k] = line
        targeted.append(k)
    if not targeted:
        err("the input CSV has no rows with the key prefix")
        return 1
    want = {args.key_prefix + r: r for r in row_order}
    for k, line in seen_csv.items():
        if k not in want:
            err("CSV key at line %d has no table row%s" % (line, ": " + k if sk else ""))
            violations += 1
    for k, r in want.items():
        if k not in seen_csv:
            err("table row at line %d has no CSV key%s" % (row_line[r], ": " + k if sk else ""))
            violations += 1

    # (c) 実行時列挙との対応
    seen_enum = {}
    for idx, s in spellings:
        if s in seen_enum:
            err("duplicate enumeration spelling at item %d (first at item %d)%s"
                % (idx, seen_enum[s], ": " + s if sk else ""))
            violations += 1
            continue
        seen_enum[s] = idx
    for s, idx in seen_enum.items():
        if s not in seen_formats:
            err("enumeration item %d has no table format%s" % (idx, ": " + s if sk else ""))
            violations += 1
    for fmt, n in seen_formats.items():
        if fmt not in seen_enum:
            err("table format at line %d is not enumerated%s" % (n, ": " + fmt if sk else ""))
            violations += 1

    if violations:
        print("[%s] FAILED: %d violation(s)" % (PROG, violations), file=sys.stderr)
        return 1
    multi = ",".join("%s:%d" % (r, row_count[r]) for r in row_order if row_count[r] >= 2) or "-"
    sys.stdout.write(
        "[%s] ok: table=%d rows=%d multi=%s csv=%d enumerate=%d\n"
        % (PROG, len(table), len(row_order), multi, len(targeted), len(seen_enum))
    )
    sys.stdout.flush()
    return 0


def main():
    parser = argparse.ArgumentParser(prog=PROG, allow_abbrev=False)
    parser.add_argument("default_csv")
    parser.add_argument("root")
    parser.add_argument("--table")
    parser.add_argument("--csv")
    parser.add_argument("--key-prefix", default="texformat:")
    parser.add_argument("--enumerate-json")
    parser.add_argument("--enumerate-list")
    parser.add_argument("--enumerate-item")
    parser.add_argument("--show-keys", action="store_true")
    args = parser.parse_args(sys.argv[1:])
    args.csv = args.csv or args.default_csv
    try:
        return run(args)
    except Usage as e:
        err(str(e))
        return 2
    except Exception as e:  # 予期しない例外でも 0 にせず、入力の中身を出さない
        err("internal error (%s)" % type(e).__name__)
        return 2


sys.exit(main())
PY
