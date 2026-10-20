# wasm32 の背後実装の feature の検査

背後実装の feature を付けたまま wasm32 向けに契約の crate が `cargo check` を通ること、Vulkan・CUDA のモジュールが外れて Metal のモジュールが公開のアイテムを持たないこと、wasm32 の依存グラフにチップの SDK の crate が載らないことを、行ごとに出す検査の所在と読み方。
private spec の TASK-238・BUILD-22・BUILD-23 のポインタ（内容は転記しない。決定の記録は private spec 側の記録に従う）。親 Issue は #1155 で、この文書は 1/2（#7353）の範囲。

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
`record:` は `check=3/3 module=3/3 sdk_wasm32_total=0 sdk_native_total=0` となる。

- `objc2-metal`・`ash`・`cudarc` はどの crate にも宣言されていないので、`native_n` も 0 件。`wasm32_n` が 0 なのは、SDK の依存が wasm32 でだけ外れた結果ではなく、そもそも載っていないため
- Metal のモジュール `backend_metal` は、本リポの現状ではネイティブでも公開のアイテムを持たない
- SDK が依存に足されたときは、同じ検査がそのまま `wasm32_n` と `native_n` の差を出す

## 確かめ方

| 確かめること | コマンド |
| ------------ | -------- |
| 本体が全行 match | `make wasm-backend-check` |
| 回帰テストが通る | `make test-wasm-backend-check` |
| `deny` ジョブに足したステップ | `grep -n "wasm-backend-check" .github/workflows/ci.yml` |
| 既存の wasm32 の組が不変 | `make build-profiles` |

依存グラフの記録と TASK-238 の完了の条件の確かめは 2/2（#7354）の範囲で、この文書には書かない。
