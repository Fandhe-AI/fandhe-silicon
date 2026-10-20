# 依存グラフのプロファイルごとの検査（TASK-228・TASK-229）の所在と現状の事実

契約の crate（`fandhe-silicon-contract`）をプロファイルの feature の組にしたときの依存グラフに、判定に使う crate 名が載っていないこと、ComputeOnly（Metal）の組では加えて `objc2-metal` の per-header feature の件数を数え、Graphics（Vulkan）の組では SDK のうち `ash`・`libloading` だけを、Graphics と相互運用の組では `ash`・`cudarc`・`libloading` だけを許し、ComputeOnly（Metal）の組では SDK のうち `objc2-metal` だけを許す検査、および比較基準の単一ビルド（`--all-features`）の SDK の件数と各プロファイルとの差の記録について、本リポのスクリプトから説明できる振る舞いを記録する。
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
| `--baseline-text <path>` | 上に足す。`--tree-text`・`--profile` と組でだけ受け付け、単独は誤用（終了コード 2）。渡したときだけ比較基準の `baseline:`・`baseline-count:` とそのプロファイルの `diff:` を出す。読めない入力・crate が 0 件・検査対象の crate 自身が無い tree は終了コード 2 | 直接実行 |
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
| `baseline:` | 標準出力 | 比較基準（`cargo tree` に `--all-features` を渡した 1 回）の `commit`・`profile`・`crates_total`・`sdk_total`（tree に載る `BASELINE_SDK_NAMES` の一意の package の数）。判定はしない。`--extra-judged-crate` の影響を受けない | `scripts/profile-dep-graph-check.sh \| grep '^baseline:'` |
| `baseline-count:` | 標準出力 | `BASELINE_SDK_NAMES` の名前ごとに 1 行。0 件も出す | `scripts/profile-dep-graph-check.sh \| grep '^baseline-count:'` |
| `diff:` | 標準出力 | 判定するプロファイルごとに 1 行。`sdk_baseline`・`sdk_profile`・`diff`（比較基準との差）。`record:`・`count:` とは接頭辞が別で、陽性対照の読み方に影響しない | `scripts/profile-dep-graph-check.sh \| grep '^diff:'` |
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
| `GRAPHICS_VULKAN_ALLOWED_SDK` | Graphics（Vulkan）の組で許す SDK・ローダ | `grep -n '^GRAPHICS_VULKAN_ALLOWED_SDK' scripts/profile-dep-graph-check.sh` |
| `GRAPHICS_INTEROP_ALLOWED_SDK` | Graphics と相互運用の組で許す SDK・ローダ | `grep -n '^GRAPHICS_INTEROP_ALLOWED_SDK' scripts/profile-dep-graph-check.sh` |
| `COMPUTE_ONLY_METAL_ALLOWED_SDK` | ComputeOnly（Metal）の組で許す SDK・ローダ | `grep -n '^COMPUTE_ONLY_METAL_ALLOWED_SDK' scripts/profile-dep-graph-check.sh` |
| `BASELINE_NAME`・`BASELINE_FEATURES`・`BASELINE_SDK_NAMES` | 比較基準の名前・cargo の feature の引数・差を数える SDK | `grep -n '^BASELINE_' scripts/profile-dep-graph-check.sh` |
| `HEADER_CRATE`・`RENDER_HEADER_FEATURES`・`RECORD_ONLY_HEADER_FEATURES` | per-header feature を数える crate と、判定に使う feature / 数えるだけの feature | `grep -nE '^(HEADER_CRATE\|RENDER_HEADER\|RECORD_ONLY)' scripts/profile-dep-graph-check.sh` |
| `PROFILES` | プロファイル名 → feature の引数・判定の名前・per-header feature の設定 | `grep -n -A30 '^PROFILES' scripts/profile-dep-graph-check.sh` |

名前の一覧の決定は private spec 側の記録に従う（ポインタのみ）。crate 名は小文字にして `_` を `-` にそろえた完全一致で比べ、接頭辞には広げない。per-header feature 名は大文字と小文字を区別した完全一致で比べる。

## 検査しないもの

| 項目 | 事実 |
| ---- | ---- |
| ほかのプロファイル | `PROFILES` の表にあるプロファイルだけを数える |
| 件数の増減 | 0 件かどうかだけを判定する |
| 比較基準の判定 | 比較基準（`--all-features`）は記録だけで、落とさない。今の製品の crate には SDK の依存が無く、3 つとも 0 件と出るため。「3 つが載り、分けたプロファイルでは差が出る」ことは回帰テストの一時 workspace の節が示す |
| SDK の crate の有無 | ComputeOnly の組では `objc2-metal` だけが入っていてよい（ほかの SDK・ローダは判定に残る）。今の製品の crate には `objc2-metal` が無く、tree に載らないので per-header feature の件数は 0 件で、`header_crate_n=0` として出る |
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
| A2 | ComputeOnly の組が描画系の feature を含まない定義であること | `grep -n -A30 '^PROFILES' scripts/profile-dep-graph-check.sh` | `--no-default-features` で、描画系の feature を渡す引数が無いことを確認 |
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

## 現状と前提の確認（1/6）（TASK-229）

TASK-229（ポインタ: `docs/spec/05-tasks.md`。関連: BUILD-3・BUILD-4・BUILD-5）の 1/6（#7358）で、本リポのコードと公開テストから観測できる事実だけを記録した。前提・完了の条件の正は private spec 側の記録に従う（ポインタのみ）。コードは変えていない。

### 前提

| 前提 | 本リポの物 | 確かめ方 |
| ---- | ---------- | -------- |
| TASK-228 | `scripts/profile-dep-graph-check.sh`・陽性対照・本書の 5/6・6/6 の節 | `gh issue view 459 --json state`（CLOSED） |
| TASK-112 | 機材の要らないジョブの枠（[ci-device-free-jobs.md](ci-device-free-jobs.md)） | `gh issue view 393 --json state`（CLOSED） |

### 現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査のプロファイル | `PROFILES` は `cpu-only`・`compute-only-metal` の 2 件だけ | `grep -n -A30 '^PROFILES' scripts/profile-dep-graph-check.sh` |
| 判定の名前 | `compute-only-metal` の判定の名前は `SURFACE_NAMES` だけで、`SDK_LOADER_NAMES` は入っていない（`cpu-only` は両方） | 同上 |
| 判定の形 | 0 件かどうかだけ。プロファイル間の差を出す出力は無く、`record:` はプロファイルごとの件数だけ | 本書「検査しないもの」の「件数の増減」の行 |
| `--extra-judged-crate` | crate 名の一覧の末尾にだけ足す。feature 側の経路は無い | 本書「検査しないもの」 |
| feature の組のビルド | `scripts/build-profiles.sh` の `NATIVE_SETS` に `backend-vulkan,graphics` の組（BUILD-3）と `backend-vulkan,backend-cuda,graphics,ext-interop` の組（BUILD-4）があり、契約の crate の `--all-features` の build / clippy / test も同スクリプトが回す | `grep -n -E 'BUILD-[34]\|all-features' scripts/build-profiles.sh` |
| 依存グラフの実測 | 上の 2 組と `--all-features` の `cargo tree` は、どれも `fandhe-silicon-contract` と `fandhe-silicon-core` の 2 crate だけ。`SDK_LOADER_NAMES`・`SURFACE_NAMES` の名前は 0 件 | `cargo tree --locked -p fandhe-silicon-contract --all-features -e normal,build,dev --target all --prefix none`（組は `--no-default-features --features <組>`） |
| 外部依存 | 契約の crate が依存するのは `fandhe-silicon-core` だけ。`deps-approved.toml` の記録は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| feature の宣言 | `graphics`・`ext-interop`・`backend-vulkan`・`backend-cuda` は `crates/contract/Cargo.toml` に宣言がある。`cfg(feature = ...)` の参照は `ext-interop` が `src/cfg_gate_doctest.rs` と `tests/feature_gate_enabled.rs` にあり、`graphics` には無い | `git grep -n -E 'feature = "(graphics\|ext-interop)"' -- crates` |
| 同じ語彙の既存検査 | 公開 API 側は `scripts/public-api-sdk-check.sh` の `CHIP_SDK`、wasm32 の依存グラフは `scripts/wasm-backend-check.sh`（[wasm32-backend-check.md](wasm32-backend-check.md)） | 各スクリプトの先頭コメント |
| CI | 本検査は CI の `deny` ジョブの `make deny`（`scripts/deny.sh`）の中で流れる | `grep -n profile-dep-graph scripts/deny.sh` |
| 公開側の言及 | 本節の追加前は TASK-229 の言及が無かった | `git grep -n TASK-229 -- . ':!docs/spec'` |

### 段

| 段 | Issue |
| -- | ----- |
| 1/6 現状と前提の確認 | #7358 |
| 2/6 Graphics（Vulkan）の組の検査 | #7359 |
| 3/6 Graphics と相互運用の組の検査 | #7360 |
| 4/6 選んでいない SDK と比較基準の差の検査 | #7361 |
| 5/6 | #7362 |
| 6/6 | #7363 |

後の段で足す振る舞い・試験は、その PR のコードと公開テストが示し、その PR で本書に節を足す。守る規約は [spec-confidentiality](../.claude/rules/spec-confidentiality.md) と [makefile](../.claude/rules/makefile.md)。

確かめた base は `0568b89d3c5ddc48973ea0133aba7f0447867bf3`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。

## 関連

- [dep-graph-wgpu-check.md](dep-graph-wgpu-check.md)（形を揃えた既存の検査）
- [.claude/rules/makefile.md](../.claude/rules/makefile.md)（ターゲットの契約）

## Graphics（Vulkan）の組の検査（2/6）（TASK-229）

TASK-229（ポインタ: `docs/spec/05-tasks.md`。対象 BUILD-3）の 2/6（#7359）で、`PROFILES` に Graphics（Vulkan）の組を足した。以降の段（BUILD-4・BUILD-5・完了の条件の確かめ・件数の記録）は後の段の PR が足す。

### 足したもの

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| プロファイル名 | `graphics-vulkan` | `grep -n '"graphics-vulkan"' scripts/profile-dep-graph-check.sh` |
| feature の引数 | 定義のブロックの `PROFILES` に置く（`scripts/build-profiles.sh` の `NATIVE_SETS` の BUILD-3 の行と同じ組）。値は本書に写さない | `grep -n -A8 '"graphics-vulkan"' scripts/profile-dep-graph-check.sh` |
| 判定の名前 | `SDK_LOADER_NAMES` から `GRAPHICS_VULKAN_ALLOWED_SDK` を除いたものに `SURFACE_NAMES` を足したもの。許す名前を判定から外すだけで、固定した一覧の外の SDK は判定に残る | 同上 |
| per-header feature | 数えない（`header_crate` は `None`。`--features-text` は受け付けない） | `scripts/test-profile-dep-graph-check.sh` の `gv_misuse_with_features_text` |
| 出力 | プロファイルが 3 件になり、`scanned:`・`record:`・`count:` が 1 組増える。出力の形は変えない | `make profile-dep-graph-check` |
| CI | `make deny` の中で流れる。ワークフローは変えない | `grep -n profile-dep-graph scripts/deny.sh` |

### 対応する回帰テスト

`make test-profile-dep-graph-check` と `make test-profile-dep-graph-positive-control`。

| 区分 | ケース | 確かめること |
| ---- | ------ | ------------ |
| 合成の tree | `gv_ok` | `ash`・`libloading` が載っても 0 件で合格し、その 2 件の `count:` は出ない |
| 合成の tree | `gv_metal`・`gv_cudarc`・`gv_surface_folded` | 許さない SDK と描画・surface の名前（`_` の折りたたみを含む）は違反（終了コード 1） |
| 合成の tree | `gv_prefix_only`・`gv_extra`・`gv_extra_already_judged`・`gv_misuse_with_features_text` | 接頭辞に広げない・`--extra-judged-crate` の足す向き・既に判定にある名前と `--features-text` は誤用（終了コード 2） |
| 偽のコマンド | `fake_tree_args_graphics_vulkan`・`fake_no_features_edges_graphics_vulkan`・`fake_extra`・`fake_violation` | 渡す cargo の引数、features の辺を流さないこと、`ash` が違反にならないこと |
| 一時 workspace | `real_clean` | `backend-vulkan` が引く `ash` が tree に載っても合格する |
| 一時 workspace | `real_graphics_selected`・`real_vulkan_judged_sdk` | `graphics` の裏の `winit`、`backend-vulkan` の裏の `objc2-metal` は Graphics の組（`graphics-vulkan`・`graphics-interop`）の違反になり、`cpu-only`・`compute-only-metal` では違反にならない |
| 一時 workspace | `real_extra_fails`・`real_other_target`・`real_dev_dep` | 3 プロファイル分の違反の件数、別 target、dev の依存 |
| 実物のリポジトリ | `repo_real`・`repo_real_extra_fails` | 3 プロファイルで合格し、足した名前は 3 プロファイルで違反になる |
| 陽性対照 | `repo_real`（`test-profile-dep-graph-positive-control`） | `graphics-vulkan` でも検出される |

### 実測

結果は GPU の実機なしで、末尾の base で `make profile-dep-graph-check` を流した値。

| 項目 | 値 |
| ---- | -- |
| 終了コード | 0（`ok: 3 profile(s) checked, no judged crates (judged=0)`） |
| `graphics-vulkan` の `scanned:` | `crates=2`（`fandhe-silicon-contract` と `fandhe-silicon-core`） |
| `graphics-vulkan` の `record:` | `judged_total=0`。判定の名前 5 件の `count:` はすべて `n=0` |

確かめた base は `33414f2f6c1d65ec86999406b9ac3091c0a9102f`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。

## Graphics と相互運用の組の検査（3/6）（TASK-229）

TASK-229（ポインタ: `docs/spec/05-tasks.md`。対象 BUILD-4）の 3/6（#7360）で、`PROFILES` に Graphics と相互運用の組を足した。以降の段（BUILD-5・完了の条件の確かめ・件数の記録）は後の段の PR が足す。

### 足したもの

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| プロファイル名 | `graphics-interop` | `grep -n '"graphics-interop"' scripts/profile-dep-graph-check.sh` |
| feature の引数 | 定義のブロックの `PROFILES` に置く（`scripts/build-profiles.sh` の `NATIVE_SETS` の BUILD-4 の行と同じ組）。値は本書に写さない | `grep -n -A12 '"graphics-interop"' scripts/profile-dep-graph-check.sh` |
| 判定の名前 | `SDK_LOADER_NAMES` から `GRAPHICS_INTEROP_ALLOWED_SDK` を除いたものに `SURFACE_NAMES` を足したもの。許す名前を判定から外すだけで、固定した一覧の外の SDK は判定に残る | 同上 |
| per-header feature | 数えない（`header_crate` は `None`。`--features-text` は受け付けない） | `scripts/test-profile-dep-graph-check.sh` の `gi_misuse_with_features_text` |
| 出力 | プロファイルが 4 件になり、`scanned:`・`record:`・`count:` が 1 組増える。出力の形は変えない | `make profile-dep-graph-check` |
| CI | `make deny` の中で流れる。ワークフローは変えない | `grep -n profile-dep-graph scripts/deny.sh` |

### 対応する回帰テスト

`make test-profile-dep-graph-check` と `make test-profile-dep-graph-positive-control`。

| 区分 | ケース | 確かめること |
| ---- | ------ | ------------ |
| 合成の tree | `gi_ok` | `ash`・`cudarc`・`libloading` が載っても 0 件で合格し、その 3 件の `count:` は出ない |
| 合成の tree | `gi_cudarc_allowed` | `graphics-vulkan` では違反になる `cudarc` の tree が、この組では合格する |
| 合成の tree | `gi_metal`・`gi_surface_folded` | 許さない SDK と描画・surface の名前（`_` の折りたたみを含む）は違反（終了コード 1） |
| 合成の tree | `gi_prefix_only`・`gi_extra`・`gi_extra_already_judged`・`gi_misuse_with_features_text` | 接頭辞に広げない・`--extra-judged-crate` の足す向き・既に判定にある名前と `--features-text` は誤用（終了コード 2） |
| 偽のコマンド | `fake_tree_args_graphics_interop`・`fake_no_features_edges_graphics_interop`・`fake_extra`・`fake_violation` | 渡す cargo の引数、features の辺を流さないこと、`ash` が違反にならないこと |
| 一時 workspace | `real_clean` | `backend-vulkan` が引く `ash` と `backend-cuda` が引く `cudarc` が tree に載っても合格する |
| 一時 workspace | `real_cpu_ops_cudarc` | `cpu-ops` の裏の `cudarc` は `cpu-only`・`graphics-vulkan` の違反になり、`graphics-interop` では違反にならない |
| 一時 workspace | `real_interop_only` | `ext-interop` の裏の `objc2-metal` は `graphics-interop` だけの違反になる |
| 一時 workspace | `real_extra_fails`・`real_other_target`・`real_dev_dep` | 4 プロファイル分の違反の件数、別 target、dev の依存 |
| 実物のリポジトリ | `repo_real`・`repo_real_extra_fails` | 4 プロファイルで合格し、足した名前は 4 プロファイルで違反になる |
| 陽性対照 | `repo_real`（`test-profile-dep-graph-positive-control`） | `graphics-interop` でも検出される |

### 実測

結果は GPU の実機なしで、末尾の base で `make profile-dep-graph-check` を流した値。

| 項目 | 値 |
| ---- | -- |
| 終了コード | 0（`ok: 4 profile(s) checked, no judged crates (judged=0)`） |
| `graphics-interop` の `scanned:` | `crates=2`（`fandhe-silicon-contract` と `fandhe-silicon-core`） |
| `graphics-interop` の `record:` | `judged_total=0`。判定の名前 4 件の `count:` はすべて `n=0` |

確かめた base は `e4bbdb41ab64eb7bf88512be89d56a4e13b19797`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。

## 選んでいない SDK と比較基準の差（4/6）（TASK-229）

TASK-229（ポインタ: `docs/spec/05-tasks.md`。対象 BUILD-5）の 4/6（#7361）で、ComputeOnly（Metal）の組の判定に選んでいない SDK を足し、比較基準の単一ビルドの記録を足した。試験の広い網羅・完了の条件の確かめ・件数の記録は後の段の PR が足す。

### 足したもの

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ComputeOnly（Metal）の判定の名前 | `SDK_LOADER_NAMES` から `COMPUTE_ONLY_METAL_ALLOWED_SDK` を除いたものに `SURFACE_NAMES` を足したもの。これで 4 プロファイルとも、選んでいない SDK・ローダの crate が判定に入る。`objc2-metal` の per-header feature の設定は変えない | `grep -n -A8 '"compute-only-metal"' scripts/profile-dep-graph-check.sh` |
| 比較基準の走査 | 4 プロファイルの後に `cargo tree` へ `BASELINE_FEATURES` を渡して 1 回流す（features の辺は取らない）。判定はせず、`baseline:`・`baseline-count:`・`diff:` の行に記録する。終了コード 0/1 には影響しない | `make profile-dep-graph-check` |
| `--tree-text` の経路 | `--baseline-text <path>` を渡したときだけ比較基準を読む。渡さないときの出力は変わらない | 「使い方と終了コード」 |
| CI | `make deny` の中で流れる。ワークフローは変えない | `grep -n profile-dep-graph scripts/deny.sh` |

### 対応する回帰テスト

`make test-profile-dep-graph-check` と `make test-profile-dep-graph-positive-control`。

| 区分 | ケース | 確かめること |
| ---- | ------ | ------------ |
| 合成の tree | `cm_ash`・`cm_cudarc`・`cm_libloading`・`cm_metal_ok` | ComputeOnly の組で `objc2-metal` 以外の SDK・ローダは違反（終了コード 1）、`objc2-metal` は違反にならない |
| 合成の tree | `bl_three`・`bl_diff_compute_only`・`bl_none` | 比較基準に 3 つの SDK が載るときの `baseline:`・`baseline-count:`・`diff:` の値 |
| 合成の tree | `bl_extra_not_in_baseline`・`bl_absent_without_flag` | `--extra-judged-crate` の名前は比較基準に出ない・`--baseline-text` を渡さなければ比較基準の行は出ない |
| 合成の tree | `bl_misuse_alone`・`bl_empty`・`bl_no_package`・`bl_unreadable` | `--baseline-text` の単独・0 件・検査対象の欠落・読めない入力は終了コード 2 |
| 偽のコマンド | `fake_tree_args_baseline`・`fake_no_features_edges_baseline`・`fake_baseline_lines` | 比較基準で渡す cargo の引数、features の辺を流さないこと、出力の行 |
| 一時 workspace | `real_baseline` | `backend-metal`・`backend-vulkan`・`backend-cuda` の裏に SDK があるとき、比較基準に 3 つが載り、4 プロファイルは合格し、`diff:` はプロファイルごとに期待どおり |
| 一時 workspace | `real_feature_selected`・`real_dev_dep`・`real_cpu_ops_cudarc` | 違反になるプロファイルに `compute-only-metal` が加わる |
| 実物のリポジトリ | `repo_real` | 4 プロファイルで合格し、`baseline:` 1 行・`baseline-count:` 3 行・`diff:` 4 行が出る |
| 陽性対照 | `repo_real`（`test-profile-dep-graph-positive-control`） | 接頭辞を分けたので、基準と足した組の件数の比べ方は変わらず通る |

### 実測

結果は GPU の実機なしで、`make profile-dep-graph-check` を流した値。

| 項目 | 値 |
| ---- | -- |
| 終了コード | 0（`ok: 4 profile(s) checked, no judged crates (judged=0)`） |
| `compute-only-metal` の crate の `count:` | 6 行、すべて `n=0` |
| `baseline:` | `crates_total=2`・`sdk_total=0`（今の製品の crate には SDK の依存が無い） |
| `baseline-count:` / `diff:` | 3 行すべて `n=0` / 4 行すべて `diff=0` |

確かめた base は `1249d2eab8a69df0c1b2b4182a27e27fc8736f67`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。
