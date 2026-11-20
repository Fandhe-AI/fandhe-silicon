# wasm32 の行列積の和の順の確かめ（TASK-2447）

## 語ごとのビットで比べる参照の種類（1/3）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2447。親 Issue は #1068、この節（1/3）は #6940。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 照合のスクリプト | `scripts/wasm-node-check.sh`（`KINDS` の種類 `gemm-order` と、各種類の欄 `bits`。関数 `bits_count`） |
| 回帰テスト | `scripts/test-wasm-node-check.sh` の項目 130〜137（合成のケース `order-gemm-orderdiff-a`・`order-gemm-orderdiff-cpudiff`） |
| 関連の頁 | `docs/wasm32-native-parity.md`（TASK-994・TASK-995 のポインタ） |

### 種類 `gemm-order`

| 項目 | 値 |
| ---- | -- |
| manifest の鍵 | `name`・`kind`・`m`・`k`・`n`・`a_file`・`b_file`・`native_scalar_file`・`cpu_reference_file`・`simd128_order_file`・`a_sha256`・`b_sha256`・`native_scalar_sha256`・`cpu_reference_sha256`・`simd128_order_sha256` |
| ファイル | `a`（m×k 語）・`b`（k×n 語）・`native-scalar`・`cpu-reference`・`simd128-order`（どれも m×n 語。1 語 4 バイト） |
| export | `gemm_scalar`・`gemm_simd` |
| 閾値の比べ（`t1:` の行） | simd128 のビルドの `gemm_simd` |
| CPU 参照の比べ（`cpu-count:` の行） | 全部の行 |
| 欄 `bits` | simd128 のビルドの `gemm_simd` は `simd128_order_file`、scalar のビルドの `gemm_simd`・`gemm_scalar` は `cpu_reference_file` |

欄 `bits` に (export, build) の組を持つ行は、wasm の出力をその参照と 4 バイトの語ごとのビットで比べ、`bits-count: <行> mismatches=<件数> n=<語数>` の行を `cpu-count:` の行の後に出す。件数が 0 でなければ行は `mismatch` で、`row:` の行の括弧の中は `bits-reference mismatches <件数>/<語数>`。保存の形の数え直しは `recount-bits:` の行を `recount-cpu:` の行の後に出し、件数が 0 でない行を `recount-summary:` の `nonzero` に数える。ほかの 5 つの種類の欄 `bits` は空で、`bits-count:`・`recount-bits:` の行を出さない。

### 回帰テスト

| 項目 | 入力 | 期待 |
| ---- | ---- | ---- |
| 130 | 3 行の出力が欄 `bits` の参照の写し | 3 行とも `match`、`bits-count:` が 3 行とも `mismatches=0 n=4`、`recount-summary: rows=3 zero=3 nonzero=0 uncompared=0`。行の順は `count:`・`cpu-count:`・`bits-count:`・`sha256:`・`t1:`、数え直しは `recount:`・`recount-cpu:`・`recount-bits:` |
| 131 | simd128 の `gemm_simd` の出力がネイティブの参照の写し | `count:`・`cpu-count:` は `mismatches=0` のまま、行は `mismatch (bits-reference mismatches 2/4)` |
| 132 | 和の順の参照から 1 語だけ 1 ulp 動かした出力 | `mismatch (bits-reference mismatches 1/4)`、`recount-bits:` も 1 |
| 133 | CPU 参照の 1 語だけがネイティブの参照と違うケース | scalar のビルドの 2 行が `mismatch (bits-reference mismatches 1/4)`、`cpu-count:` は 0 |
| 134 | 1 語に番兵 `7fc0dead` が残った出力 | `mismatch (mismatches 1/4)`、`bits-count:`・`cpu-count:` も 1 |
| 135 | manifest の `simd128_order_file` の鍵を消す・sha256 を変える・ファイルを 1 語縮める | 終了コード 2、`row:` の行を出さない |
| 136 | 保存の形の `simd128-order` のファイルの 1 語を書き換える | 数え直しの `recount-bits:` が 1、終了コード 1 |
| 137 | 種類 `gemm-special` の行 | `bits-count:`・`recount-bits:` の行を出さない |

### 確かめ方

```sh
make test-wasm-node-check
```

観測した結果は `passed: 714, failed: 0`。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | Node での照合に語ごとのビットで直に比べる参照の種類を足す | #6940 |
