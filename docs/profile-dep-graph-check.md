# 依存グラフのプロファイルごとの検査（TASK-228）の所在と現状の事実

契約の crate（`fandhe-silicon-contract`）をプロファイルの feature の組にしたときの依存グラフに、判定に使う crate 名が載っていないこと、ComputeOnly（Metal）の組では加えて `objc2-metal` の per-header feature の件数を数える検査について、本リポのスクリプトから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-228。対象は BUILD-1・BUILD-2。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/profile-dep-graph-check.sh`（bash + `python3 -I`、標準ライブラリのみ） | `scripts/profile-dep-graph-check.sh` |
| 呼び出し元 | `make profile-dep-graph-check`（単体）と `make deny`（`scripts/deny.sh`）。CI の `deny` ジョブは `make deny` の中で流す（別ステップにしない）。違反も前提の不足も失敗にし、握りつぶさない | `grep -rn profile-dep-graph-check scripts/deny.sh Makefile .github` |
| 入力 | `cargo tree --locked -p <契約の crate> <feature の引数> -e normal,build,dev --target all --prefix none`。per-header feature を数えるプロファイルは、これに加えて辺の種類に `features` を足した順方向の tree と、crate の tree に `objc2-metal` が載っているときだけ、その版ごとに `-i <名前>@<版>` を付けた逆向きの tree（順方向は契約の crate 自身の feature が有効にする分を表示しないため）。または `--tree-text`・`--features-text` で渡した出力 | 同スクリプトの先頭コメント |
| 回帰テスト | `scripts/test-profile-dep-graph-check.sh`（`make test-profile-dep-graph-check`）。合成 tree、偽の cargo / git、PATH を絞った環境、一時 workspace の実物の `cargo tree`、実物のリポジトリで、0 件の成功・1 件の失敗・読めない入力の終了コード 2 を確かめる。リポジトリのファイルを変更しない・ネットワーク不要。CI のジョブからは呼ばない（手元で流す） | `make test-profile-dep-graph-check` |
| 陽性対照 | `scripts/profile-dep-graph-positive-control.sh`（bash + `python3 -I`）。検査に `--extra-judged-crate` で実物の tree に載る名前を 1 件足して流し、全プロファイルで違反が出ることを確かめる。単体の呼び出し元は `make profile-dep-graph-positive-control`。`make deny` も検査の直後に呼ぶ（CI の `deny` ジョブの `make deny` の中で流れる） | `make profile-dep-graph-positive-control` |
| 陽性対照の回帰テスト | `scripts/test-profile-dep-graph-positive-control.sh`（`make test-profile-dep-graph-positive-control`）。偽の内側の検査で検出・0 件・基準の違反・読めない入力の各経路を、実物のリポジトリで本物の本体を確かめる。リポジトリのファイルを変更しない・ネットワーク不要。CI のジョブからは呼ばない（手元で流す） | `make test-profile-dep-graph-positive-control` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 使い方と終了コード

| 呼び方 | 動作 | 確かめ方 |
| ------ | ---- | -------- |
| 引数なし | プロファイルごとに cargo を流して数える | 直接実行 |
| `--tree-text <path> --profile <name>` | 渡した tree だけを数える（cargo・git を呼ばない）。2 つは組でだけ受け付ける | 直接実行 |
| `--features-text <path>` | 上に足す。per-header feature を数えるプロファイルでは必須、数えないプロファイルや単独では誤用（終了コード 2）。順方向と逆向きの出力をつないで渡してよい | 直接実行 |
| `--extra-judged-crate <name>` | 確かめ用の opt-in。上のどちらの経路とも併用でき、全プロファイルの判定の名前の一覧の末尾に 1 件だけ足して数える（足す向きにしか働かず、定義の一覧は書き換えない）。英数字・`_`・`-` で 1〜64 文字。すでに判定の名前にあるもの（小文字にして `_` を `-` にそろえて比べる）・2 回の指定・不正な文字は誤用（終了コード 2）。cargo の引数は変わらない | 直接実行 |

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
| `extra_judged_crate=` | 標準出力 | `--extra-judged-crate` を渡したときだけ、`record:` と `ok:` の末尾に付く。足した名前の `count:` も判定の名前の後ろに出る。渡さないときの出力は変わらない | `scripts/profile-dep-graph-check.sh --extra-judged-crate <name>` |
| `positive-control:` | 標準出力（陽性対照） | プロファイルごとに 1 行。`n` は足した名前の件数で、1 件以上なら `result=detected`。0 件は標準エラーの `positive-control-failed: … result=no-detection`（終了コード 1）。読めない入力・想定外の終了・件数の食い違いは `positive-control-failed: … result=<理由>`（終了コード 2） | `make profile-dep-graph-positive-control` |

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
| per-header feature の一覧へ足す経路 | 無い。`--extra-judged-crate` は crate 名の一覧にだけ足す。今の製品の crate には `objc2-metal` が無く、実物で「落ちる」ことを示せないため。feature の判定が働くことは回帰テストの一時 workspace の節が示す |
| 製品の workspace 全体の依存 | 契約の crate だけを `-p` で数える（全体は [dep-graph-wgpu-check.md](dep-graph-wgpu-check.md)） |

## 確かめ方

```bash
make profile-dep-graph-check
make test-profile-dep-graph-check
make profile-dep-graph-positive-control
make test-profile-dep-graph-positive-control
make deny
```

`make deny` のログに、`profile-dep-graph-check (contract profiles)` の行とプロファイルごとの `record:`、`profile-dep-graph-positive-control` の行とプロファイルごとの `positive-control:` の行が出て、終了コード 0 で終わることを見る。

## 完了の条件の確かめ（5/6）

5/6（#3926）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-228。BUILD-1・BUILD-2）。結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 検査が通り、判定の件数がプロファイルごとに 0 件 | `make profile-dep-graph-check` | 終了コード 0。`scanned:` は 2 プロファイル（各 `crates=2`）。各 `record:` は `judged_total=0`、`commit=` は base（`-dirty` 無し）。`count:` の全行が `n=0`。ComputeOnly の組の `record:` は `header_crate_n=0`・`features_judged_total=0`・`features_recorded_total=0`。feature ごとの `count:` は judged=yes が 5 行・judged=no が 3 行で、すべて `n=0` |
| A2 | ComputeOnly の組が描画系の feature を含まない定義であること | `grep -n -A22 '^PROFILES' scripts/profile-dep-graph-check.sh` | `--no-default-features` で、描画系の feature を渡す引数が無いことを確認 |
| A3 | 判定の一覧に 1 件足すと落ちること | `make profile-dep-graph-positive-control` | 終了コード 0。`positive-control:` はプロファイルごとに 1 行（2 行）で、すべて `n=1 result=detected`。内側の検査は `FAILED: 2 judged crate(s) found` |
| A4 | 回帰テストが通ること | `make test-profile-dep-graph-check`、`make test-profile-dep-graph-positive-control` | どちらも終了コード 0。合格 94 件・不合格 0 件、合格 23 件・不合格 0 件。per-header feature の判定が働くことは、前者の一時 workspace の節が示す（本書「検査しないもの」の記述どおり） |
| A5 | CI の機材の要らないジョブで通ること | main の push の `ci.yml` の run 39132697062（base と同じコミット）の `make deny (cargo-deny)` ジョブ（`ubuntu-latest`）のログ | ジョブの conclusion は success。ログに `profile-dep-graph-check (contract profiles)`・`ok: 2 profile(s) checked, no judged crates (judged=0)`・`profile-dep-graph-positive-control`・`positive-control: ... result=detected` が 2 行・`ok: positive control detected ...` が出ている |
| D | 品質ゲート | `make check`、`make verify`、`make deny`、`make deps-check` | すべて終了コード 0。`make deny` のログに検査と陽性対照の 2 つの `[deny.sh]` の行が出る |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-228）。

### 再実行の契機

次を変えたときは、上の表を流し直す。

- `scripts/profile-dep-graph-check.sh`（特に「数え方の定義」ブロック）
- `scripts/profile-dep-graph-positive-control.sh`、`scripts/deny.sh`
- `crates/contract/Cargo.toml` の feature と依存、`Cargo.lock`
- `.github/workflows/ci.yml` の `deny` ジョブ
- `docs/spec` の gitlink

確かめた base は `beb4172`、`docs/spec` の gitlink は `8664148`。

## プロファイルごとの件数の記録（6/6）

6/6（#3927）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-228。BUILD-1・BUILD-2）。結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。

| 成果物 | 本書の置き場 |
| ------ | ------------ |
| 検査と CI への組み込み | 「所在」「使い方と終了コード」「完了の条件の確かめ（5/6）」 |
| プロファイルごとの件数の記録 | 本節（`record:`・`count:` の行） |

`make profile-dep-graph-check` の標準出力（終了コード 0。`record:` の `commit=` は base と一致し `-dirty` 無し）を、プロファイルごとに並べる。

| プロファイル | `crates_total` | `judged_total` | crate の `count:` の行 | `header_crate_n` | `features_judged_total` | `features_recorded_total` | feature の `count:` の行 |
| ------------ | -------------- | -------------- | ---------------------- | ---------------- | ----------------------- | ------------------------- | ------------------------ |
| `cpu-only` | 2 | 0 | 7 行（すべて `n=0`） | — | — | — | — |
| `compute-only-metal` | 2 | 0 | 3 行（すべて `n=0`） | 0 | 0 | 0 | judged=yes が 5 行・judged=no が 3 行（すべて `n=0`） |

- 数えるだけの feature（`RECORD_ONLY_HEADER_FEATURES`）は、judged=no の 3 行として判定の対象と別に出ていて、`features_judged_total` には入らない
- crate の一覧は `SDK_LOADER_NAMES`・`SURFACE_NAMES`、judged=yes の feature は `RENDER_HEADER_FEATURES` で指す（名前は本書に写さない）
- 末尾の行は `ok: 2 profile(s) checked, no judged crates (judged=0)`

`make profile-dep-graph-positive-control` は終了コード 0。`positive-control:` は 2 行（プロファイルごとに 1 行）で、どちらも `n=1 result=detected`。

CI のログ: main の push の `ci.yml` の run 39133670015（base と同じコミット）の `make deny (cargo-deny)` ジョブ（job 117693749343）の conclusion は success。ログに上と同じ値の `record:`（プロファイルごとに 2 行）・`ok: 2 profile(s) checked, no judged crates (judged=0)`・`positive-control:`（2 行、`result=detected`）が出ている。

再実行の契機は「完了の条件の確かめ（5/6）」の「再実行の契機」と同じ。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-228）。

確かめた base は `8d838b352e33cdcdd02fd530a7c958fc4925bc82`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## 関連

- [dep-graph-wgpu-check.md](dep-graph-wgpu-check.md)（形を揃えた既存の検査）
- [.claude/rules/makefile.md](../.claude/rules/makefile.md)（ターゲットの契約）
