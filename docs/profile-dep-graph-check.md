# 依存グラフのプロファイルごとの検査（TASK-228）の所在と現状の事実

契約の crate（`fandhe-silicon-contract`）をプロファイルの feature の組にしたときの依存グラフに、判定に使う crate 名が載っていないこと、ComputeOnly（Metal）の組では加えて `objc2-metal` の per-header feature の件数を数える検査について、本リポのスクリプトから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-228。対象は BUILD-1・BUILD-2。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/profile-dep-graph-check.sh`（bash + `python3 -I`、標準ライブラリのみ） | `scripts/profile-dep-graph-check.sh` |
| 呼び出し元 | `make profile-dep-graph-check`（単体）。`make deny` と CI のジョブからは呼ばない | `grep -rn profile-dep-graph-check scripts/deny.sh Makefile .github` |
| 入力 | `cargo tree --locked -p <契約の crate> <feature の引数> -e normal,build,dev --target all --prefix none`。per-header feature を数えるプロファイルは、これに加えて辺の種類に `features` を足した順方向の tree と、crate の tree に `objc2-metal` が載っているときだけ、その版ごとに `-i <名前>@<版>` を付けた逆向きの tree（順方向は契約の crate 自身の feature が有効にする分を表示しないため）。または `--tree-text`・`--features-text` で渡した出力 | 同スクリプトの先頭コメント |
| 回帰テスト | `scripts/test-profile-dep-graph-check.sh`（`make test-profile-dep-graph-check`）。合成 tree、偽の cargo / git、PATH を絞った環境、一時 workspace の実物の `cargo tree`、実物のリポジトリで、0 件の成功・1 件の失敗・読めない入力の終了コード 2 を確かめる。リポジトリのファイルを変更しない・ネットワーク不要 | `make test-profile-dep-graph-check` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 使い方と終了コード

| 呼び方 | 動作 | 確かめ方 |
| ------ | ---- | -------- |
| 引数なし | プロファイルごとに cargo を流して数える | 直接実行 |
| `--tree-text <path> --profile <name>` | 渡した tree だけを数える（cargo・git を呼ばない）。2 つは組でだけ受け付ける | 直接実行 |
| `--features-text <path>` | 上に足す。per-header feature を数えるプロファイルでは必須、数えないプロファイルや単独では誤用（終了コード 2）。順方向と逆向きの出力をつないで渡してよい | 直接実行 |

| コード | 意味 | 確かめ方 |
| ------ | ---- | -------- |
| 0 | 全プロファイルで判定に使う crate が 0 件 | 直接実行して `echo $?` |
| 1 | 1 件以上（判定に使う crate、または判定に使う per-header feature。数えるだけの feature は含めない） | 同スクリプトの先頭コメント |
| 2 | 誤用・前提の不足・cargo の失敗・読めない入力・数えた crate が 0 件・検査対象の crate 自身が tree に無い。成功扱いにしない | 同スクリプトの先頭コメント |

## 出力の読み方

件数の値は変わるので書かない。出る場所と見方だけを示す。

| 出力 | 場所 | 見方 | 確かめ方 |
| ---- | ---- | ---- | -------- |
| `scanned:` | 標準出力 | プロファイルごとに 1 行 | `scripts/profile-dep-graph-check.sh \| grep '^scanned:'` |
| `record:` | 標準出力 | `commit`（cargo を流す経路では HEAD のコミット、変更があれば末尾に `-dirty`。`--tree-text` の経路では `-`）、`profile`、`crates_total`、`judged_total` | `scripts/profile-dep-graph-check.sh \| grep '^record:'` |
| `count:` | 標準出力 | 判定の名前ごとに 1 行。0 件の名前も出す | `scripts/profile-dep-graph-check.sh \| grep '^count:'` |
| `count:`（feature） | 標準出力 | per-header feature ごとに 1 行（`feature=` と `judged=yes` または `judged=no`）。0 件も出す。`judged=no` は数えて記録するだけで落とさない | `scripts/profile-dep-graph-check.sh \| grep 'feature='` |
| `record:` の追加欄 | 標準出力 | per-header feature を数えるプロファイルだけ、末尾に `header_crate_n`（crate の tree に載る `objc2-metal` の版の数）・`features_judged_total`・`features_recorded_total` | 同上 |
| `violation:` / `FAILED:` / `error:` | 標準エラー | 違反の crate と版・feature / crate と feature それぞれの合計 / 終了コード 2 の理由 | 同スクリプトの先頭コメント |
| `ok:` | 標準出力 | 合格のとき 1 行 | 直接実行 |

ホストの絶対パスと環境変数の値は出力に出さない。

## 数え方の定義の所在

プロファイルの表（feature の組と判定の名前の一覧）・依存の種類・target・名前の比べ方は、スクリプトの Python 部の先頭の「数え方の定義」ブロックにだけ置く。ブロックが正で、本書には値を写さない。

| 定数 | 内容 | 確かめ方 |
| ---- | ---- | -------- |
| `PACKAGE` | 検査する crate | `grep -n '^PACKAGE' scripts/profile-dep-graph-check.sh` |
| `DEP_KINDS`・`TARGET` | 数える依存の種類と target | `grep -nE '^(DEP_KINDS\|TARGET)' scripts/profile-dep-graph-check.sh` |
| `SDK_LOADER_NAMES`・`SURFACE_NAMES` | 判定に使う名前 | `grep -nE '_NAMES =' scripts/profile-dep-graph-check.sh` |
| `HEADER_CRATE`・`RENDER_HEADER_FEATURES`・`RECORD_ONLY_HEADER_FEATURES` | per-header feature を数える crate と、判定に使う feature / 数えるだけの feature | `grep -nE '^(HEADER_CRATE\|RENDER_HEADER\|RECORD_ONLY)' scripts/profile-dep-graph-check.sh` |
| `PROFILES` | プロファイル名 → feature の引数・判定の名前・per-header feature の設定 | `grep -n -A22 '^PROFILES' scripts/profile-dep-graph-check.sh` |

名前の一覧の決定は private spec 側の記録に従う（ポインタのみ）。crate 名は小文字にして `_` を `-` にそろえた完全一致で比べ、接頭辞には広げない。per-header feature 名は大文字と小文字を区別した完全一致で比べる。

## 検査しないもの

| 項目 | 事実 |
| ---- | ---- |
| ほかのプロファイル | `PROFILES` の表にあるプロファイルだけを数える |
| 件数の増減 | 0 件かどうかだけを判定する |
| SDK の crate の有無 | ComputeOnly の組では `objc2-metal` が入っていてよい（SDK を除く判定は対象外）。今の製品の crate には `objc2-metal` が無く、tree に載らないので per-header feature の件数は 0 件で、`header_crate_n=0` として出る |
| 製品の workspace 全体の依存 | 契約の crate だけを `-p` で数える（全体は [dep-graph-wgpu-check.md](dep-graph-wgpu-check.md)） |

## 確かめ方

```bash
make profile-dep-graph-check
make test-profile-dep-graph-check
```

## 関連

- [dep-graph-wgpu-check.md](dep-graph-wgpu-check.md)（形を揃えた既存の検査）
- [.claude/rules/makefile.md](../.claude/rules/makefile.md)（ターゲットの契約）
