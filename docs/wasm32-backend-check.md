# wasm32 の背後実装の feature の検査

背後実装の feature を付けたまま wasm32 向けに契約の crate が `cargo check` を通ること、Vulkan・CUDA のモジュールが外れて Metal のモジュールが公開のアイテムを持たないこと、wasm32 の依存グラフにチップの SDK の crate が載らないことを、行ごとに出す検査の所在と読み方。
private spec の TASK-238・BUILD-22・BUILD-23 のポインタ（内容は転記しない。決定の記録は private spec 側の記録に従う）。親 Issue は #1155 で、この文書は 1/2（#7353）と 2/2（#7354）の範囲。

## 所在

| もの | パス |
| ---- | ---- |
| 検査の本体 | `scripts/wasm-backend-check.sh`（`make wasm-backend-check`） |
| 回帰テスト | `scripts/test-wasm-backend-check.sh`（`make test-wasm-backend-check`） |
| CI | `deny` ジョブの 2 ステップ（`make wasm-backend-check`・`make test-wasm-backend-check`。ジョブ・check 名・`ci-complete` の `needs` は不変） |

`scripts/build-profiles.sh` の wasm32 の組（`contract-core,cpu-isa,cpu-ops` に背後実装の feature を足した 3 組）は、`cargo check` が通るかどうかだけを見る。
この検査は、それに加えてモジュールの有無と依存グラフの件数を行ごとに出す（置き換えではなく、判別できる事実を足す役）。
`scripts/profile-dep-graph-check.sh` の数え方（`--target all`）は変えず、wasm32 の target の数え方はこの検査の「定義」ブロックに置く。

## 検査する構成と行

構成は契約の crate（`fandhe-silicon-contract`）の `backend-metal`・`backend-vulkan`・`backend-cuda` の 3 つ。
既定の feature は外さず `--features <名前>` だけを足し、`--target wasm32-unknown-unknown` で調べる。

| 行 | 内容 | match の条件 |
| -- | ---- | ------------ |
| `check:` | `cargo check -p fandhe-silicon-contract --target wasm32-unknown-unknown --features <feature> --locked` | 終了コード 0 |
| `module:` | wasm32 向けの rustdoc JSON（公開アイテムのモジュール）の有無 | Metal は `backend_metal` が在り公開の子が 0 件。Vulkan・CUDA は `backend_vulkan`・`backend_cuda` が無い |
| `deps:` | `cargo tree -e normal,build,dev --target wasm32-unknown-unknown` の一意の package のうち SDK（`objc2-metal`・`ash`・`cudarc`）の件数 `wasm32_n`。同じ feature の `--target all` の件数を `native_n` に記録する | `wasm32_n` が 0 |
| `record:` | 締めの 1 行（`commit`・`configs`・`check=k/3`・`module=k/3`・`sdk_wasm32_total`・`sdk_native_total`） | 終了コード 0 と 1 のときだけ出す |

名前は小文字にして `_` を `-` にそろえた完全一致で数える（接頭辞には広げない）。`native_n` は記録用で合否には使わない。
rustdoc JSON は `RUSTC_BOOTSTRAP=1` をその子プロセスにだけ渡して作り、構成ごとに別名で残す。`RUSTFLAGS` 系の環境変数は子プロセスへ渡さない。

終了コードは 0 全行 match、1 mismatch が 1 行以上（または `check:` が not-run）、2 誤用・前提の不足（python3・cargo・rustup・git・wasm32 の target）・cargo の失敗・読めない入力・検査対象の crate が tree に無い。
標準出力・標準エラーにホストの絶対パスと環境変数の値は出さない。

回帰テスト用に `--rustdoc-json <feature>=<path>`・`--tree-text <feature>=<path>`・`--native-tree-text <feature>=<path>` を 3 構成ぶんそろえて渡すと、cargo・git を呼ばずに `module:` と `deps:` だけを判定する。この経路は `check:` が not-run なので終了コードは 1 になる。

## 回帰テストとの対応

| 観点 | ケース |
| ---- | ------ |
| module（合成 JSON） | 在る・空で match／公開の子で mismatch／private の子は数えない／無くて mismatch／Vulkan・CUDA が在って mismatch／別のモジュールは数えない／形の崩れ・JSON でない・ファイル無しで終了コード 2 |
| deps（合成 tree） | 0 件で match／`ash` の表記ゆれ（大文字・`_`）／接頭辞を数えない／native にだけ在るのは match で `native_n` に出る／検査対象の crate が無い・空の tree で終了コード 2 |
| 誤用 | 未知の引数・値なし・未知の feature・重複・3 構成がそろわない・`=` なし |
| 偽の cargo | 3 構成に `--target wasm32-unknown-unknown --features <x> --locked` が渡る／`--no-default-features` を付けない／`RUSTC_BOOTSTRAP` は rustdoc だけ／rustflags 系が子に渡らない／構成ごとの JSON が別名で残る／check の失敗は 1・rustdoc / tree の失敗と target 欠落は 2 |
| 実物 | 契約の crate と core だけの一時 workspace で全行 match・終了コード 0／出力に一時ディレクトリのパスが出ない |
| 陽性対照 | 一時 workspace の写しで Vulkan のゲートから wasm32 の除外を外すと `module:` が mismatch で終了コード 1 |

## 本リポの現状での実行結果

`make wasm-backend-check` を実行すると、3 構成とも `check:`・`module:`・`deps:` が match で終了コード 0 になる。
`record:` は `check=3/3 module=3/3 sdk_wasm32_total=0 sdk_native_total=1` となる。

- `objc2-metal` は契約の crate に macOS 向けだけ・optional で宣言され、feature `backend-metal` が有効なときだけ載る。`backend-metal` の構成の `native_n`（`--target all`）は 1 件で、`wasm32_n` は 0 件（macOS 向けの依存が wasm32 で外れた結果）。`backend-vulkan`・`backend-cuda` の構成では 0 件
- `ash`・`cudarc` はどの crate にも宣言されていないので、`native_n` も `wasm32_n` も 0 件
- Metal のモジュール `backend_metal` は、wasm32 では公開のアイテムを持たない（`public_items=0`）。ネイティブ（macOS）の構成では公開の項目があり、wasm32 では出ない
- SDK が依存に足されたときは、同じ検査がそのまま `wasm32_n` と `native_n` の差を出す

## 確かめ方

| 確かめること | コマンド |
| ------------ | -------- |
| 本体が全行 match | `make wasm-backend-check` |
| 回帰テストが通る | `make test-wasm-backend-check` |
| `deny` ジョブに足したステップ | `grep -n "wasm-backend-check" .github/workflows/ci.yml` |
| 既存の wasm32 の組が不変 | `make build-profiles` |

## 依存グラフの記録（2/2）

2/2（#7354）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-238。BUILD-22・BUILD-23）。
結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。数え方は「検査する構成と行」の節のとおり。

| 成果物 | この文書の中の置き場 |
| ------ | -------------------- |
| wasm32 の cargo check の CI のジョブ | 「所在」と「完了の条件の確かめ（2/2）」 |
| 依存グラフの記録 | 本節 |

`check:` と `module:`（3 構成）。

| feature | check（target） | check result | module | expect | found | public_items | module result |
| ------- | --------------- | ------------ | ------ | ------ | ----- | ------------ | ------------- |
| `backend-metal` | `wasm32-unknown-unknown` | match | `backend_metal` | present-empty | present | 0 | match |
| `backend-vulkan` | `wasm32-unknown-unknown` | match | `backend_vulkan` | absent | absent | - | match |
| `backend-cuda` | `wasm32-unknown-unknown` | match | `backend_cuda` | absent | absent | - | match |

`deps:`（3 構成 × 3 SDK）。全 9 行が次の値だった（この段を記録した時点の値。製品の crate に SDK の依存が無かった時点で、現状は上の「本リポの現状での実行結果」を見る）。

| feature | name | wasm32_n | native_n | result |
| ------- | ---- | -------- | -------- | ------ |
| `backend-metal`・`backend-vulkan`・`backend-cuda` の各々 | `objc2-metal` | 0 | 0 | match |
| 同上 | `ash` | 0 | 0 | match |
| 同上 | `cudarc` | 0 | 0 | match |

`record:` の行（`commit=` は末尾の base の先頭 12 桁と一致し、`-dirty` は付かない）。

```text
record: commit=6950c283b286 configs=3 check=3/3 module=3/3 sdk_wasm32_total=0 sdk_native_total=0
```

- `objc2-metal`・`ash`・`cudarc` はどの crate にも宣言されていないので、`native_n` も 0 件。`wasm32_n` の 0 は、target で外れた結果ではなく、そもそも載っていないことによる
- SDK が依存に足されたときは、同じ検査が `wasm32_n` と `native_n` の差を出す
- CI: base の push の `ci.yml` の run 39425952330、`deny` ジョブ（job 118638434970）が success。ログに同じ値の `record:` の行と `ok: all 3 configurations match` が出ており、続く `make test-wasm-backend-check` は `passed=42 failed=0`

## 完了の条件の確かめ（2/2）

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-238）。結果の欄は実測の値。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 本体が全行 match | `make wasm-backend-check` の終了コード | 0 |
| A2 | 3 構成の `check:` が match | `check:` の 3 行 | 3/3 match |
| A3 | Vulkan・CUDA のモジュールが外れ、Metal のモジュールが公開のアイテムを持たない | `module:` の 3 行 | 3/3 match |
| A4 | wasm32 の依存グラフでチップの SDK が 0 件 | `deps:` の 9 行がすべて `wasm32_n=0`、`sdk_wasm32_total=0` | 9/9 で 0 件（`native_n` も 0 件） |
| A5 | 回帰テストが通る | `make test-wasm-backend-check` | passed=42 failed=0 |
| A6 | CI の機材の要らないジョブで通る | run 39425952330 の `deny` ジョブ（job 118638434970） | success。同じ値の `record:` と `ok:` がログに出た |
| A7 | 既存の wasm32 の組が不変 | `make build-profiles` の終了コード | 0 |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` の終了コード | すべて 0 |

A4 の 0 件は、3 つの SDK がどの crate にも宣言されていないことによる。target の条件で外れる振る舞いそのものは、本リポの現状では観測していない。

再実行の契機。次を変えたときは表を流し直す。

- `scripts/wasm-backend-check.sh`（特に「定義」のブロック）と `scripts/test-wasm-backend-check.sh`
- `crates/contract/Cargo.toml` の feature と依存、`Cargo.lock`
- `crates/contract/src` の背後実装のモジュールの wasm32 の cfg ゲート
- `.github/workflows/ci.yml` の `deny` ジョブ
- `docs/spec` のサブモジュールの指す版

確かめた base は `6950c283b2867d2a1790385209dc523472a2553e`、`docs/spec` のサブモジュールの指す版は `f603cba40283866ca89338a66a85a9bde085a0d9`。
