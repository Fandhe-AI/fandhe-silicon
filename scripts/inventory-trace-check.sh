#!/usr/bin/env bash
# 一覧と項目の対応の検査（spec TASK-16 のポインタ。親 Issue #60 の 2/4）。
# 一覧の各行の鍵が、項目ファイルのどれかに現れるかを確かめる（前向きの検査）。
# 入力のポインタ: docs/spec/03-poc/wgpu-feature-inventory/inventory.csv（一覧）と
# docs/spec/04-behavior/（項目ファイル群）。どちらも private の submodule の中にある。
# 呼び出し元: `make inventory-trace-check`（手元だけの検査。CI は呼ばない。走らせる場所は未決）。
#
# 使い方:
#   scripts/inventory-trace-check.sh [--csv PATH] [--items-dir DIR] [--key-column NAME]
#                                    [--exclude BASENAME]... [--show-keys]
#   --csv           一覧の CSV（既定: 上のポインタのパス）。先頭のレコードを見出しとして扱う
#   --items-dir     項目ファイルのディレクトリ（既定: 上のポインタのパス）。直下の *.md だけを読む
#   --key-column    鍵の列の見出しの名前（既定: 先頭の列）。値はスクリプトに書かず実行時に受ける
#   --exclude       探索から外すファイル名（繰り返し可）。README.md は常に外す
#   --show-keys     違反の行に鍵の文字列も出す。出力は spec の内容なので、PR・Issue・CI のログ・
#                   コミットに貼らない（既定は CSV の行番号だけ）
#
# 終了コード:
#   0  合格（すべての行の鍵が、少なくとも 1 つの項目ファイルに現れた）
#   1  違反（現れない鍵／鍵が空／鍵が重複／列の数の不一致／データの行が 0／探索対象が 0／
#      CSV や項目ファイルを読めない）
#   2  誤用・前提の不足（検査を終えていない。入力が無い場合を含む。成功扱いにしない）
#
# 判定の規則は「未承認の仮置き」で、前向きだけ（逆向きは実装しない）。「対応」の定義は
# spec 側の判断待ち（ポインタ: TASK-16。関連: TASK-552・TASK-1375）。仮置きの内容は
# Python 側の occurs() に閉じ込めてあり、規則が決まったらこの関数だけを差し替える。
# 列名・鍵の形・項目の行の書式は定数として持たない（引数で受けるか、入力から実行時に求める）。
# 入力は読むだけ。ファイルを作らず、ネットワークを使わない。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

require_cmd python3 || exit 2

# 引用符付きの heredoc でシェルの展開を通さない。既定のパスを先頭の位置引数で渡し、
# 残りの引数（"$@"）は Python の argparse が解釈する。
exec python3 -I - \
  "${ROOT_DIR}/docs/spec/03-poc/wgpu-feature-inventory/inventory.csv" \
  "${ROOT_DIR}/docs/spec/04-behavior" \
  "$@" <<'PY'
import argparse
import csv
import os
import sys

PROG = "inventory-trace-check.sh"


def err(msg):
    print("error: " + msg, file=sys.stderr)


def occurs(text, key, alphabet):
    """未承認の仮置きの規則: 直前と直後の文字がどちらも鍵の文字集合に入らない出現があるか。

    別の鍵の一部として現れただけのもの（接頭辞・接尾辞）を数えないための境界。
    鍵や引数を式として解釈しないよう、正規表現を使わず str.find で探す。
    """
    start = 0
    while True:
        i = text.find(key, start)
        if i < 0:
            return False
        before = text[i - 1] if i > 0 else ""
        j = i + len(key)
        after = text[j] if j < len(text) else ""
        if before not in alphabet and after not in alphabet:
            return True
        start = i + 1


def locate(keys, texts, alphabet):
    """鍵 -> 現れたファイル名の一覧。判定・出力とは分けてある（一覧を足すときの継ぎ目）。"""
    return {k: [n for n, t in texts if occurs(t, k, alphabet)] for k in keys}


class Usage(Exception):
    pass


def run(args):
    # 前提の検査はすべて先に行い、欠けていれば内容を読まずに 2 で終える。
    if not os.path.isfile(args.csv):
        raise Usage(
            "input CSV not found: %s (the private spec submodule may not be fetched; "
            "see README, section on the spec)" % args.csv
        )
    if not os.path.isdir(args.items_dir):
        raise Usage(
            "items directory not found: %s (the private spec submodule may not be fetched; "
            "see README, section on the spec)" % args.items_dir
        )
    excluded = {"README.md"} | set(args.exclude)
    for name in args.exclude:
        if os.sep in name or not os.path.isfile(os.path.join(args.items_dir, name)):
            raise Usage("--exclude names a file that does not exist directly under the items directory")

    try:
        with open(args.csv, encoding="utf-8-sig", newline="") as f:
            reader = csv.reader(f, strict=True)
            header = next(reader, None)
            if header is None:
                raise Usage("input CSV is empty")
            if args.key_column is None:
                col = 0
            else:
                hits = [i for i, h in enumerate(header) if h == args.key_column]
                if len(hits) != 1:
                    raise Usage("--key-column must match exactly one header name")
                col = hits[0]
            rows = [(reader.line_num, r) for r in reader]
    except (UnicodeDecodeError, csv.Error, OSError):
        err("cannot parse the input CSV")
        return 1

    violations = 0
    entries = []
    seen = {}
    if not rows:
        err("the input CSV has no data rows")
        violations += 1
    for line, row in rows:
        if len(row) != len(header):
            err("column count differs from the header at line %d" % line)
            violations += 1
            continue
        key = row[col]
        if key == "":
            err("empty key at line %d" % line)
            violations += 1
            continue
        if key in seen:
            err("duplicate key at line %d (first seen at line %d)" % (line, seen[key]))
            violations += 1
            continue
        seen[key] = line
        entries.append((line, key))

    names = sorted(
        n for n in os.listdir(args.items_dir)
        if n.endswith(".md") and n not in excluded
        and os.path.isfile(os.path.join(args.items_dir, n))
    )
    if not names:
        err("no item files (*.md) to search")
        violations += 1
    texts = []
    try:
        for n in names:
            with open(os.path.join(args.items_dir, n), encoding="utf-8") as f:
                texts.append((n, f.read()))
    except (UnicodeDecodeError, OSError):
        err("cannot read an item file")
        return 1

    alphabet = set()
    for _, k in entries:
        alphabet.update(k)
    found = locate([k for _, k in entries], texts, alphabet)
    for line, key in entries:
        if not found[key]:
            if args.show_keys:
                err("no item file contains the key at line %d: %s" % (line, key))
            else:
                err("no item file contains the key at line %d" % line)
            violations += 1

    if violations:
        print("[%s] FAILED: %d violation(s)" % (PROG, violations), file=sys.stderr)
        return 1
    print("[%s] ok: every row key was found in at least one item file" % PROG, file=sys.stderr)
    return 0


def main():
    parser = argparse.ArgumentParser(prog=PROG, allow_abbrev=False)
    parser.add_argument("default_csv")
    parser.add_argument("default_items_dir")
    parser.add_argument("--csv")
    parser.add_argument("--items-dir")
    parser.add_argument("--key-column")
    parser.add_argument("--exclude", action="append", default=[])
    parser.add_argument("--show-keys", action="store_true")
    args = parser.parse_args(sys.argv[1:])
    args.csv = args.csv or args.default_csv
    args.items_dir = args.items_dir or args.default_items_dir
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
