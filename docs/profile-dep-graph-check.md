# 依存グラフのプロファイルごとの検査（TASK-228）の所在と現状の事実

契約の crate（`fandhe-silicon-contract`）をプロファイルの feature の組にしたときの依存グラフに、判定に使う crate 名が載っていないことを数える検査について、本リポのスクリプトから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-228。対象は BUILD-1。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/profile-dep-graph-check.sh`（bash + `python3 -I`、標準ライブラリのみ） | `scripts/profile-dep-graph-check.sh` |
| 呼び出し元 | `make profile-dep-graph-check`（単体）。`make deny` と CI のジョブからは呼ばない | `grep -rn profile-dep-graph-check scripts/deny.sh Makefile .github` |
| 入力 | `cargo tree --locked -p <契約の crate> <feature の引数> -e normal,build,dev --target all --prefix none`。または `--tree-text` で渡した出力 | 同スクリプトの先頭コメント |
| 回帰テスト | `scripts/test-profile-dep-graph-check.sh`（`make test-profile-dep-graph-check`）。合成 tree、偽の cargo / git、PATH を絞った環境、一時 workspace の実物の `cargo tree`、実物のリポジトリで、0 件の成功・1 件の失敗・読めない入力の終了コード 2 を確かめる。リポジトリのファイルを変更しない・ネットワーク不要 | `make test-profile-dep-graph-check` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 使い方と終了コード

| 呼び方 | 動作 | 確かめ方 |
| ------ | ---- | -------- |
| 引数なし | プロファイルごとに cargo を流して数える | 直接実行 |
| `--tree-text <path> --profile <name>` | 渡した tree だけを数える（cargo・git を呼ばない）。2 つは組でだけ受け付ける | 直接実行 |

| コード | 意味 | 確かめ方 |
| ------ | ---- | -------- |
| 0 | 全プロファイルで判定に使う crate が 0 件 | 直接実行して `echo $?` |
| 1 | 1 件以上（違反） | 同スクリプトの先頭コメント |
| 2 | 誤用・前提の不足・cargo の失敗・読めない入力・数えた crate が 0 件・検査対象の crate 自身が tree に無い。成功扱いにしない | 同スクリプトの先頭コメント |

## 出力の読み方

件数の値は変わるので書かない。出る場所と見方だけを示す。

| 出力 | 場所 | 見方 | 確かめ方 |
| ---- | ---- | ---- | -------- |
| `scanned:` | 標準出力 | プロファイルごとに 1 行 | `scripts/profile-dep-graph-check.sh \| grep '^scanned:'` |
| `record:` | 標準出力 | `commit`（cargo を流す経路では HEAD のコミット、変更があれば末尾に `-dirty`。`--tree-text` の経路では `-`）、`profile`、`crates_total`、`judged_total` | `scripts/profile-dep-graph-check.sh \| grep '^record:'` |
| `count:` | 標準出力 | 判定の名前ごとに 1 行。0 件の名前も出す | `scripts/profile-dep-graph-check.sh \| grep '^count:'` |
| `violation:` / `FAILED:` / `error:` | 標準エラー | 違反の crate と版 / 違反の合計 / 終了コード 2 の理由 | 同スクリプトの先頭コメント |
| `ok:` | 標準出力 | 合格のとき 1 行 | 直接実行 |

ホストの絶対パスと環境変数の値は出力に出さない。

## 数え方の定義の所在

プロファイルの表（feature の組と判定の名前の一覧）・依存の種類・target・名前の比べ方は、スクリプトの Python 部の先頭の「数え方の定義」ブロックにだけ置く。ブロックが正で、本書には値を写さない。

| 定数 | 内容 | 確かめ方 |
| ---- | ---- | -------- |
| `PACKAGE` | 検査する crate | `grep -n '^PACKAGE' scripts/profile-dep-graph-check.sh` |
| `DEP_KINDS`・`TARGET` | 数える依存の種類と target | `grep -nE '^(DEP_KINDS\|TARGET)' scripts/profile-dep-graph-check.sh` |
| `SDK_LOADER_NAMES`・`SURFACE_NAMES` | 判定に使う名前 | `grep -nE '_NAMES =' scripts/profile-dep-graph-check.sh` |
| `PROFILES` | プロファイル名 → feature の引数と判定の名前 | `grep -n -A6 '^PROFILES' scripts/profile-dep-graph-check.sh` |

名前の一覧の決定は private spec 側の記録に従う（ポインタのみ）。名前は小文字にして `_` を `-` にそろえた完全一致で比べ、接頭辞には広げない。

## 検査しないもの

| 項目 | 事実 |
| ---- | ---- |
| ほかのプロファイル | `PROFILES` の表にあるプロファイルだけを数える |
| 件数の増減 | 0 件かどうかだけを判定する |
| 製品の workspace 全体の依存 | 契約の crate だけを `-p` で数える（全体は [dep-graph-wgpu-check.md](dep-graph-wgpu-check.md)） |

## 確かめ方

```bash
make profile-dep-graph-check
make test-profile-dep-graph-check
```

## 関連

- [dep-graph-wgpu-check.md](dep-graph-wgpu-check.md)（形を揃えた既存の検査）
- [.claude/rules/makefile.md](../.claude/rules/makefile.md)（ターゲットの契約）
