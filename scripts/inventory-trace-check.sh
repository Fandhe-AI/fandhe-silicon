#!/usr/bin/env bash
# 一覧と項目の対応の検査（spec TASK-16 のポインタ。親 Issue #60 の 2/4 と 3/4）。
# 既定は、一覧の各行の鍵が項目ファイルのどれかに現れるかを確かめる（前向きの検査。#62 と同じ合否）。
# --item-rows を付けると、鍵が項目ファイルの表の行（項目の行）のどれかに現れるかまで絞る（opt-in）。
# --list を付けると、合格のときだけ鍵と項目の対応の一覧を標準出力へ出す（保存はしない）。
# 入力のポインタ: docs/spec/03-poc/wgpu-feature-inventory/inventory.csv（一覧）と
# docs/spec/04-behavior/（項目ファイル群）。どちらも private の submodule の中にある。
# 呼び出し元: `make inventory-trace-check`（手元だけの検査。CI は呼ばない。走らせる場所は未決）。
#
# 使い方:
#   scripts/inventory-trace-check.sh [--csv PATH] [--items-dir DIR] [--key-column NAME]
#                                    [--exclude BASENAME]... [--item-rows]
#                                    [--item-id-column NAME] [--list] [--show-keys]
#   --csv           一覧の CSV（既定: 上のポインタのパス）。先頭のレコードを見出しとして扱う
#   --items-dir     項目ファイルのディレクトリ（既定: 上のポインタのパス）。直下の *.md だけを読む
#   --key-column    鍵の列の見出しの名前（既定: 先頭の列）。値はスクリプトに書かず実行時に受ける
#   --exclude       探索から外すファイル名（繰り返し可）。README.md は常に外す
#   --item-rows     項目の行（表のデータ行）まで絞って判定する。未指定のときはファイル単位で、
#                   既定の合否は変わらない。項目の単位の規則は未承認の仮置きで、確定扱いにしない
#   --item-id-column 項目の行のラベルの列（--item-rows と併用。単独では誤用として 2）の見出しの名前（既定: 各表の先頭の列）。指定したときは、
#                   その名前の見出しがちょうど 1 つある表だけを項目の表として扱う。値は実行時に受ける
#   --list          合格のときだけ、対応の一覧を標準出力へ TSV で出す（列: line / key / file / item。
#                   item は --item-rows のときだけ値が入る（既定は空）。
#                   並びは CSV の行順 → ファイル名順 → ファイル内の行順）。ファイルは作らない。
#                   出力は spec の内容なので、PR・Issue・CI のログ・コミットに貼らない
#   --show-keys     違反の行に鍵の文字列も出す。出力は spec の内容なので、PR・Issue・CI のログ・
#                   コミットに貼らない（既定は CSV の行番号だけ）
#
# 終了コード:
#   0  合格（既定: すべての行の鍵が、少なくとも 1 つの項目ファイルに現れた。--item-rows のとき: 少なくとも 1 つの項目の行に現れた）
#   1  違反（項目ファイルに現れない鍵（--item-rows のときは項目の行に現れない鍵）／項目の行が 0（--item-rows のとき）／鍵が空／鍵が重複／列の数の不一致／データの行が 0／探索対象が 0／
#      CSV や項目ファイルを読めない）
#   2  誤用・前提の不足（検査を終えていない。入力が無い場合・--item-id-column が解決できない場合を含む。成功扱いにしない）
#
# --item-rows の規則は「未承認の仮置き」（既定では使わない）: 行頭が | の markdown の表のデータ行 1 行を項目の行とし、
# ラベルのセル以外のセルに鍵が現れれば帰属とする。列の数が見出しと合わない行・ラベルが空の行は
# 項目の行に数えない（違反にはせず件数だけを知らせる）。表の外の本文にだけ現れる鍵は違反になる。
# 1 つの鍵が複数の項目に現れる場合はすべて一覧に出す。規則は locate() に閉じ込めてある。
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
import re
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


def split_row(line):
    """表の行をセルに分ける。行頭（空白を除く）が | でなければ None。\\| は区切りにしない。"""
    t = line.strip()
    if not t.startswith("|"):
        return None
    cells, cur, i = [], [], 1
    while i < len(t):
        c = t[i]
        if c == "\\" and i + 1 < len(t) and t[i + 1] == "|":
            cur.append("|")
            i += 2
            continue
        if c == "|":
            cells.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        i += 1
    # 閉じの | で終わる行は、最後のセルを足さない（空セルを落とさないため）。
    if not (len(t) > 1 and t.endswith("|") and not t.endswith("\\|")):
        cells.append("".join(cur).strip())
    return cells


def is_separator(cells):
    """区切り行か。各セルが、両端の : を除いて 1 文字以上の - だけ。"""
    if not cells:
        return False
    for c in cells:
        core = c.strip(":")
        if core == "" or core.strip("-") != "":
            return False
    return True


def mask_fences(lines):
    """コードフェンス（``` / ~~~）の開閉行と中身を空行にして返す。行数は保つ。

    閉じは開きと同じ記号で、同じ長さ以上、記号だけの行。閉じが無ければ末尾までがフェンス内。
    """
    out, fence = [], None
    for line in lines:
        m = re.match(r" {0,3}(`{3,}|~{3,})(.*)$", line)
        if fence is None:
            if m and not (m.group(1)[0] == "`" and "`" in m.group(2)):
                fence = m.group(1)
            out.append("" if fence else line)
        else:
            if m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence) \
                    and m.group(2).strip() == "":
                fence = None
            out.append("")
    return out


def item_rows(name, text, id_column):
    """項目の行を (ファイル名, 行番号, ラベル, ラベル以外のセル) で返す。

    戻り値は (行の一覧, 項目の表の数, 読み飛ばした行数)。表の単位は未承認の仮置き。
    """
    lines = mask_fences(text.splitlines())
    rows, tables, skipped = [], 0, 0
    i = 0
    while i + 1 < len(lines):
        head = split_row(lines[i])
        sep = split_row(lines[i + 1])
        if head is None or sep is None or not is_separator(sep) or len(sep) != len(head):
            i += 1
            continue
        if id_column is None:
            col = 0
        else:
            hits = [k for k, h in enumerate(head) if h == id_column]
            if len(hits) > 1:
                raise Usage("--item-id-column matches more than one header in a table")
            col = hits[0] if hits else None
        j = i + 2
        body = []
        while j < len(lines):
            cells = split_row(lines[j])
            if cells is None:
                break
            body.append((j + 1, cells))
            j += 1
        if col is not None:
            tables += 1
            for ln, cells in body:
                if len(cells) != len(head) or cells[col] == "":
                    skipped += 1
                    continue
                rows.append((name, ln, cells[col], cells[:col] + cells[col + 1:]))
        i = j
    return rows, tables, skipped


def locate_files(keys, texts, alphabet):
    """鍵 -> [(ファイル名, 0, "")]。ファイル単位（既定）。判定は occurs() に任せる。"""
    return {
        k: [(n, 0, "") for n, t in texts if occurs(t, k, alphabet)]
        for k in keys
    }


def locate(keys, rows, alphabet):
    """鍵 -> [(ファイル名, 行番号, ラベル)]。ラベル以外のセルに現れた項目の行だけを数える。

    未承認の仮置き（項目の単位）。判定は occurs() に任せ、帰属の単位はここだけで決める。
    """
    out = {}
    for k in keys:
        out[k] = [
            (n, ln, label) for n, ln, label, cells in rows
            if any(occurs(c, k, alphabet) for c in cells)
        ]
    return out


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

    # 列挙の失敗（権限なし等）も読み取りの失敗として 1 で終える（2 の誤用と区別する）。
    try:
        names = sorted(
            n for n in os.listdir(args.items_dir)
            if n.endswith(".md") and n not in excluded
            and os.path.isfile(os.path.join(args.items_dir, n))
        )
    except OSError:
        err("cannot list the items directory")
        return 1
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

    if args.item_id_column is not None and not args.item_rows:
        raise Usage("--item-id-column requires --item-rows")
    item_list, tables, skipped = [], 0, 0
    for n, t in (texts if args.item_rows else []):
        r, tb, sk = item_rows(n, t, args.item_id_column)
        item_list.extend(r)
        tables += tb
        skipped += sk
    if args.item_rows and args.item_id_column is not None and texts and tables == 0:
        raise Usage("--item-id-column matches no table header in the item files")
    if skipped:
        err("%d table row(s) were not treated as items (column count mismatch or empty label)" % skipped)
    if args.item_rows and texts and not item_list:
        err("no item rows found in the item files")
        violations += 1

    alphabet = set()
    for _, k in entries:
        alphabet.update(k)
    if args.item_rows:
        found = locate([k for _, k in entries], item_list, alphabet)
    else:
        found = locate_files([k for _, k in entries], texts, alphabet)
    for line, key in entries:
        if not found[key]:
            if not args.item_rows:
                if args.show_keys:
                    err("no item file contains the key at line %d: %s" % (line, key))
                else:
                    err("no item file contains the key at line %d" % line)
            elif args.show_keys:
                err("no item row contains the key at line %d: %s" % (line, key))
            else:
                err("no item row contains the key at line %d" % line)
            violations += 1

    if violations:
        print("[%s] FAILED: %d violation(s)" % (PROG, violations), file=sys.stderr)
        return 1
    print("[%s] ok: every row key was found in at least one item %s"
          % (PROG, "row" if args.item_rows else "file"), file=sys.stderr)
    if args.list:
        # 合格のときだけ書く。符号化・出力先の切断は呼び出し側で 2 に落ちる。
        sys.stdout.reconfigure(encoding="utf-8", newline="")
        w = csv.writer(sys.stdout, delimiter="\t", lineterminator="\n")
        w.writerow(["line", "key", "file", "item"])
        for line, key in entries:
            for n, _, label in sorted(found[key], key=lambda x: (x[0], x[1])):
                w.writerow([line, key, n, label])
        sys.stdout.flush()
    return 0


def main():
    parser = argparse.ArgumentParser(prog=PROG, allow_abbrev=False)
    parser.add_argument("default_csv")
    parser.add_argument("default_items_dir")
    parser.add_argument("--csv")
    parser.add_argument("--items-dir")
    parser.add_argument("--key-column")
    parser.add_argument("--exclude", action="append", default=[])
    parser.add_argument("--item-rows", action="store_true")
    parser.add_argument("--item-id-column")
    parser.add_argument("--list", action="store_true")
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
