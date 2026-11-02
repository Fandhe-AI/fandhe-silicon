# web の出力の Node での読み込みの検査（TASK-248）の所在と本リポの現状

wasm-bindgen の `--target web` の出力を Node の ESM で読み込み、決まった 7 名の export の有無と
`detect_isa_bits` の戻り値を行で出す検査の所在と、本リポの現状の事実をまとめる。

## 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-248。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## web の出力の読み込みの検査

### 読み込みの枠と判定（1/4）

#### 所在

| 物 | 場所 |
| -- | ---- |
| 読み込みと観測（判定なし） | `wasm/node/load-web.mjs` |
| 検査の本体（判定） | `scripts/wasm-web-load-check.sh` |
| 回帰テスト | `scripts/test-wasm-web-load-check.sh` |

本体は `--from-dir <dir>` を必須とし、無ければ使い方の誤りで終了コード 2 になる。
node は `v24.13.0` と厳密に一致しなければ、組を読む前に終了コード 2 になる。

#### 組

組の綴りと入力の置き方は `scripts/wasm-size.sh` の `--from-dir` に揃えている（TASK-247 のポインタ）。
読むのは `<dir>/<組>/web/wasm_glue.js` と `wasm_glue_bg.wasm` の 2 名だけで、`--from-dir` 直下の 4 組以外の項目や
`web` 以外の項目は見ない。

| 組 | 入力の置き方 | `detect-isa-bits` の期待の値 |
| -- | ------------ | ---------------------------- |
| `release-scalar` | `<dir>/release-scalar/web/` | 0 |
| `release-simd128` | `<dir>/release-simd128/web/` | 1 |
| `wasm-size-scalar` | `<dir>/wasm-size-scalar/web/` | 0 |
| `wasm-size-simd128` | `<dir>/wasm-size-simd128/web/` | 1 |

#### 11 項目

| 項目 | export | いまの判定の規則 |
| ---- | ------ | ---------------- |
| `gemm-scalar-1`・`gemm-scalar-2` | `gemm_scalar` | export が無ければ `not-run`（`missing-export`）、在っても呼ばず `not-run`（`no-call-rule`） |
| `gemm-simd-1`・`gemm-simd-2` | `gemm_simd` | 同上 |
| `topk-scalar` | `topk_scalar` | 同上 |
| `topk-simd` | `topk_simd` | 同上 |
| `elementwise-scalar-relu`・`elementwise-scalar-add` | `elementwise_scalar` | 同上 |
| `elementwise-simd-relu`・`elementwise-simd-add` | `elementwise_simd` | 同上 |
| `detect-isa-bits` | `detect_isa_bits` | export が無ければ `not-run`（`missing-export`）。在れば引数なしで 1 回呼び、値が組の期待と同じなら `ok`、違えば `ng`、呼び出しが失敗すれば `ng`（`call-failed`） |

`ok` は呼んで確かめた項目にだけ付く。`detect_isa_bits` 以外の 6 名は呼ばない。

#### `load-web.mjs` の結果

結果は 1 行の JSON で、形は次の 3 つだけ。例外の文・パスは書き出さない。

| 形 | 条件 |
| -- | ---- |
| `{"init":"failed"}` | 読み込み・`initSync` の欠け・`initSync` の例外のどれか |
| `{"init":"ok","exports":[...]}` | 初期化でき、`detect_isa_bits` が無い |
| `{"init":"ok","exports":[...,"detect_isa_bits"],"detect_isa_bits":<0〜4294967295 の整数 or "call-failed">}` | 初期化でき、`detect_isa_bits` が在る（戻りが number でなければ `call-failed`） |

引数が 2 つでなければ終了コード 2、それ以外は 0。

#### 行の形

| 行 | 形 |
| -- | -- |
| 組 | `init: combo=<組> target=web result=ok\|ng\|not-run[ reason=<理由>]` |
| 項目 | `item: combo=<組> item=<名前> export=<名前> label=ok\|ng\|not-run[ reason=<理由>][ value=<値>]` |
| 締め | `summary: combos=4 items=44 ok=<n> ng=<n> not-run=<n>` |

| 理由 | 出る場面 |
| ---- | -------- |
| `missing-directory` | 組または `web` のディレクトリが無い |
| `missing-file` | `wasm_glue.js` か `wasm_glue_bg.wasm` が無い |
| `empty-file` | 2 名のどちらかが 0 バイト |
| `init-failed` | `{"init":"failed"}`（組の行は `ng`、11 項目は `not-run`） |
| `missing-export` | 初期化できた組で、項目の export が無い |
| `no-call-rule` | 計算の 10 項目で export が在る |
| `call-failed` | `detect_isa_bits` の呼び出しが失敗した |

#### 終了コード

| コード | 意味 |
| ------ | ---- |
| 0 | 4 組とも `init` が `ok` で 44 項目とも `ok` |
| 1 | `ng` か `not-run` が 1 つ以上（標準エラーに `FAILED:`） |
| 2 | 誤用・前提の不足（node の版違いを含む）・読めない入力（シンボリックリンクなど）・不正な結果（標準エラーに `error:`。このとき `init:`・`item:`・`summary:` は出さない） |

#### glue の export

glue の側の `wasm_bindgen` の付いた関数は、次の確かめのとおり 1 件（`roundtrip_f32`）で、7 名の export は無い。

```text
$ grep -n wasm_bindgen wasm/crates/glue/src/lib.rs
18:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
```

#### 確かめ方

```text
$ bash scripts/test-wasm-web-load-check.sh
...
test-wasm-web-load-check: 67 passed, 0 failed
```

記録時点のコミットは `d4ed11c`。偽の node による判定の全経路と、本物の node で `load-web.mjs` を偽の glue に直接当てる試験を含む。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | web の出力を Node で読み込む枠と回帰テスト | #3970 |
| 2/4 | web の 4 組をビルドする経路 | #3971 |
| 3/4 | make ターゲットと回帰テストの CI への組み込み | #3972 |
| 4/4 | 完了の条件の確かめ | #3973 |

親 Issue は #468。
