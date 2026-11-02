#!/usr/bin/env bash
# 機能の問合せの鍵の一覧の突き合わせの検査（spec TASK-1114 のポインタ。親 Issue #948 の 5/6）。
# core の鍵の一覧（`crates/core/examples/feature_keys.rs` の出力）を、機能一覧の CSV に突き合わせる。
# 一覧の既定のパスは `scripts/inventory-trace-check.sh`（TASK-16 のポインタ）と同じ。
# 入力のポインタ: 一覧は docs/spec/03-poc/wgpu-feature-inventory/inventory.csv（private の submodule の中）。
# 呼び出し元: `make feature-key-check`（直接の実行も可。手元だけの検査で、CI は呼ばない）。
#
# 使い方:
#   scripts/feature-key-check.sh [--keys PATH] [--csv PATH] [--key-prefix STR] [--show-keys] [--example NAME]
#   --keys        鍵の一覧（1 行 1 鍵の 1 列のテキスト）。省略時は example を cargo で実行して得る（下の注記）
#   --csv         機能一覧の CSV（既定: 上のポインタのパス）。先頭のレコードを見出しとし、先頭の列を鍵の列にする
#   --key-prefix  鍵がこれで始まる行だけを CSV の対象にする（既定: feature:）
#   --show-keys   違反の診断に鍵の文字列も出す（既定は件数と行番号だけ）
#   --example     --keys を省いたときに cargo で実行する example の名前（既定: feature_keys）。
#                 英小文字・数字・_ だけ。--keys との同時の指定は誤用。接頭辞は呼び手が --key-prefix で渡す
#
# 判定:
#   (a) 一覧の形: 各行が 1 列（タブなし）で空でない・互いに異なる・どれも接頭辞で始まる
#   (b) 一覧との対応: CSV の対象の行の鍵の集合と一覧の鍵の集合が過不足なく一致（CSV の鍵の重複・列数の不一致も違反）
# 空の一覧・空の CSV・対象の行が 0 件の CSV・読めない入力も違反（1）。
#
# 終了コード:
#   0  合格（標準出力に件数の 1 行）
#   1  違反（読めない・空の入力を含む）
#   2  誤用・前提の不足（未知の引数・--example の誤用・入力が無い・cargo が無い・example の失敗）
#
# 入力は読むだけ。`--keys` を渡したときは、ファイルを作らず、ネットワークを使わない。
# `--keys` を省略したときは cargo 経由で example を実行するので、cargo の通常の動作として
# target 配下へビルド成果物を書き、キャッシュの状態によっては依存の解決でネットワークも使う。
# 読み取り専用の環境やネットワークを使えない環境では、`--keys` に事前に得た一覧を渡す。
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
import os
import re
import shutil
import subprocess
import sys

PROG = "feature-key-check.sh"


class Usage(Exception):
    pass


def err(msg):
    print("error: " + msg, file=sys.stderr)


def get_keys_text(args):
    """鍵の一覧の文字列を返す。--keys が無ければ example を cargo で実行して得る。"""
    if args.keys is not None:
        with open(args.keys, "rb") as f:
            return f.read().decode("utf-8")
    cargo = shutil.which("cargo")
    if cargo is None:
        raise Usage("cargo not found (needed when --keys is not given)")
    proc = subprocess.run(
        [cargo, "run", "-q", "-p", "fandhe-silicon-core", "--example", args.example or "feature_keys"],
        cwd=args.root, capture_output=True, stdin=subprocess.DEVNULL,
    )
    if proc.returncode != 0:
        raise Usage("the key list example failed to run")
    return proc.stdout.decode("utf-8")


def run(args):
    if args.keys is not None and not os.path.isfile(args.keys):
        raise Usage("key list file not found: %s" % args.keys)
    if not os.path.isfile(args.csv):
        raise Usage("input CSV not found (the private submodule may need to be fetched): %s" % args.csv)
    sk = args.show_keys

    try:
        text = get_keys_text(args)
    except (UnicodeDecodeError, OSError):
        err("cannot read the key list")
        return 1
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    if not lines:
        err("the key list is empty")
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

    violations = 0

    # (a) 一覧の形
    keys = {}  # 鍵 -> 行番号
    for n, line in enumerate(lines, 1):
        if "\t" in line:
            err("key list line %d has more than 1 column" % n)
            violations += 1
            continue
        if line == "":
            err("empty key at key list line %d" % n)
            violations += 1
            continue
        if not line.startswith(args.key_prefix):
            err("key at key list line %d does not start with the prefix%s"
                % (n, ": " + line if sk else ""))
            violations += 1
        if line in keys:
            err("duplicate key at key list line %d (first at line %d)%s"
                % (n, keys[line], ": " + line if sk else ""))
            violations += 1
            continue
        keys[line] = n

    # (b) 機能一覧との対応
    seen_csv = {}
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
    if not seen_csv:
        err("the input CSV has no rows with the key prefix")
        return 1
    for k, line in seen_csv.items():
        if k not in keys:
            err("CSV key at line %d is not in the key list%s" % (line, ": " + k if sk else ""))
            violations += 1
    for k, n in keys.items():
        if k not in seen_csv:
            err("key at key list line %d has no CSV key%s" % (n, ": " + k if sk else ""))
            violations += 1

    if violations:
        print("[%s] FAILED: %d violation(s)" % (PROG, violations), file=sys.stderr)
        return 1
    sys.stdout.write("[%s] ok: keys=%d csv=%d\n" % (PROG, len(keys), len(seen_csv)))
    sys.stdout.flush()
    return 0


def main():
    parser = argparse.ArgumentParser(prog=PROG, allow_abbrev=False)
    parser.add_argument("default_csv")
    parser.add_argument("root")
    parser.add_argument("--keys")
    parser.add_argument("--csv")
    parser.add_argument("--key-prefix", default="feature:")
    parser.add_argument("--show-keys", action="store_true")
    parser.add_argument("--example")
    args = parser.parse_args(sys.argv[1:])
    args.csv = args.csv or args.default_csv
    try:
        # cargo の検索・実行より前に検証する（入力の値はメッセージに埋め込まない）。
        if args.example is not None and re.fullmatch(r"[a-z0-9_]+", args.example) is None:
            raise Usage("invalid --example name")
        if args.keys is not None and args.example is not None:
            raise Usage("--keys and --example cannot be used together")
        return run(args)
    except Usage as e:
        err(str(e))
        return 2
    except Exception as e:  # 予期しない例外でも 0 にせず、入力の中身を出さない
        err("internal error (%s)" % type(e).__name__)
        return 2


sys.exit(main())
PY
