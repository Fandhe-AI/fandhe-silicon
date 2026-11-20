# wasm32 の行列積の和の順の確かめ（TASK-2447）

## 語ごとのビットで比べる参照の種類（1/3）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2447。親 Issue は #1068、この節（1/3）は #6940。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 照合のスクリプト | `scripts/wasm-node-check.sh`（`KINDS` の種類 `gemm-order` と、各種類の欄 `bits`。関数 `bits_count`） |
| 回帰テスト | `scripts/test-wasm-node-check.sh` の項目 130〜139（合成のケース `order-gemm-orderdiff-a`・`order-gemm-orderdiff-cpudiff`・`order-gemm-ordersame`） |
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

欄 `bits` に (export, build) の組を持つ行は、wasm の出力をその参照と 4 バイトの語ごとのビットで比べ、`bits-count: <行> mismatches=<件数> n=<語数>` の行を `cpu-count:` の行の後に出す。件数が 0 でなければ行は `mismatch` になる。種類 `gemm-order` の行の `row:` の括弧は、通常の参照の比べに不一致があれば `mismatches <件数>/<語数>`、無くてビットの比べに不一致があれば `bits-reference mismatches <件数>/<語数>` になる。CPU 参照は下の照らしで `native-scalar` と sha256 が同じなので、通常の参照の比べに不一致が無い行は CPU 参照の比べにも不一致が無く、括弧に `cpu-reference mismatches` は出ない。比べた行では、ビットの不一致の件数を `bits-count:` の行で確かめられる。保存の形の数え直しは `recount-bits:` の行を `recount-cpu:` の行の後に出し、件数が 0 でない行を `recount-summary:` の `nonzero` に数える。ほかの 5 つの種類の欄 `bits` は空で、`bits-count:`・`recount-bits:` の行を出さない。

閾値で比べない組（scalar のビルドの `gemm_simd`・`gemm_scalar`）の欄 `bits` の参照 `cpu-reference` は、`native-scalar` と sha256（manifest の値をファイルと照らした後の値）が同じでなければならない。違えば照合の前に `error: manifest bits reference differs from the native-scalar reference` で終了コード 2 になり、`row:` の行を出さない。保存の形の数え直し（`--recount`）も同じ照らしで終了コード 2 になり、`recount-bits:` の行を出さない。そのため scalar のビルドの行の括弧は `mismatches` で、`bits-reference mismatches` は simd128 のビルドの `gemm_simd` の行に出る。

閾値で比べる組（simd128 のビルドの `gemm_simd`）の欄 `bits` の参照 `simd128-order` は、`native-scalar` と sha256 が違わなければならない。同じなら照合の前に `error: manifest bits reference equals the native-scalar reference` で終了コード 2 になり、`row:` の行を出さない。`--recount` も同じ照らしで終了コード 2 になり、`recount-bits:` の行を出さない。

1/3 の差分は照合側だけで、`wasm-parity-host gen` を変えない（1/3 の時点の `gen` は種類 `gemm-order` の manifest を出力せず、通常の `wasm-node-check.sh --cases` の経路にこの種類の行は現れない）。回帰テストは合成の manifest で流す。

### 回帰テスト

| 項目 | 入力 | 期待 |
| ---- | ---- | ---- |
| 130 | 3 行の出力が欄 `bits` の参照の写し | 3 行とも `match`、`bits-count:` が 3 行とも `mismatches=0 n=4`、`recount-summary: rows=3 zero=3 nonzero=0 uncompared=0`。行の順は `count:`・`cpu-count:`・`bits-count:`・`sha256:`・`t1:`、数え直しは `recount:`・`recount-cpu:`・`recount-bits:` |
| 131 | simd128 の `gemm_simd` の出力がネイティブの参照の写し | `count:`・`cpu-count:` は `mismatches=0` のまま、行は `mismatch (bits-reference mismatches 2/4)` |
| 132 | 和の順の参照から 1 語だけ 1 ulp 動かした出力 | `mismatch (bits-reference mismatches 1/4)`、`recount-bits:` も 1 |
| 133 | CPU 参照の 1 語だけがネイティブの参照と違うケース。保存の形の `cpu-reference` のファイルの 1 語を書き換え、manifest の `cpu_reference_sha256` も更新した数え直し | どちらも終了コード 2 で `error: manifest bits reference differs from the native-scalar reference`、`row:`・`recount-bits:` の行を出さない |
| 134 | 1 語に番兵 `7fc0dead` が残った出力 | `mismatch (mismatches 1/4)`、`bits-count:`・`cpu-count:` も 1 |
| 135 | manifest の `simd128_order_file` の鍵を消す・sha256 を変える・ファイルを 1 語縮める | 終了コード 2、`row:` の行を出さない |
| 136 | 保存の形の `simd128-order` のファイルの 1 語を書き換え、manifest の `simd128_order_sha256` も書き換え後の値に更新する（更新しないと SHA-256 の照合で終了コード 2 になり `recount-bits:` を出さない） | 数え直しの `recount-bits:` が 1、終了コード 1 |
| 137 | 種類 `gemm-special` の行 | `bits-count:`・`recount-bits:` の行を出さない |
| 138 | scalar のビルドの `gemm_scalar` の出力の 1 語を 1 ulp 動かす | `mismatch (mismatches 1/4)`、`count:`・`bits-count:`・`recount-bits:` が 1、`cpu-count:` は 0 |
| 139 | 和の順の参照がネイティブの参照と同じケース。保存の形の `simd128-order` のファイルをネイティブの参照の写しにし、manifest の `simd128_order_sha256` も更新した数え直し | どちらも終了コード 2 で `error: manifest bits reference equals the native-scalar reference`、`row:`・`recount-bits:` の行を出さない |

### 確かめ方

```sh
make test-wasm-node-check
```

観測した結果は `passed: 720, failed: 0`。

## 和の順の参照と 9 行（2/3）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2447。親 Issue は #1068、この節（2/3）は #6941。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| host 側の道具 | `wasm/crates/parity-host/src/lib.rs` の `GEMM_ORDER_CASES`・`build_order_cases`・`simd128_order`・`gemm_cpu_reference`、`GemmCaseSource::SquareFrom`。`wasm-parity-host gen <dir>` が 3 ケースの `.a.bin`・`.b.bin`・`.native-scalar.bin`・`.cpu-reference.bin`・`.simd128-order.bin` と manifest の 3 要素（種類 `gemm-order`）を書く |
| 試験 | `wasm/crates/parity-host/tests/gemm_order.rs`（10 本）、`wasm/crates/parity-host/tests/recount_expected.rs` の `order_simd_rows_equal_to_native_reference_fail` |
| 照合の入力 | `wasm/node/cases.json` の行 9 行と `cases` の欄の 3 要素 |

### 3 ケース

| ケース | m・k・n | 入力 |
| ------ | ------- | ---- |
| `order-gemm-64` | 64・64・64 | 生成器の位置 0 から `a`、続けて `b`（`gemm-64` と同じ入力） |
| `order-gemm-256` | 256・256・256 | 生成器の位置 `2 * 64 * 64` から `a`、続けて `b` |
| `order-gemm-37x41x53` | 37・53・41 | `GemmGateRow::Odd37x41x53`（`gemm-37x41x53` と同じ入力） |

`simd128_order` は各出力を、k を 4 つずつ 4 レーンの積と和（別の演算）で積み、4 レーンを先頭から順に足し、端数の k は昇順の `f32::mul_add` で足した値にする。`gemm_cpu_reference` は `reference_gemm_f32` の出力。

### 9 行

| 行 | export | build | 語ごとのビットで比べる参照 |
| -- | ------ | ----- | -------------------------- |
| `order-gemm-<n>-simd128-gemm-simd` | `gemm_simd` | `simd128` | `simd128-order` |
| `order-gemm-<n>-scalar-gemm-simd` | `gemm_simd` | `scalar` | `cpu-reference` |
| `order-gemm-<n>-scalar-gemm-scalar` | `gemm_scalar` | `scalar` | `cpu-reference` |

（`<n>` は `64`・`256`・`37x41x53`。9 行とも `reference_source` は `native-scalar`。）

### 試験

| 試験 | 期待 |
| ---- | ---- |
| `order_cases_follow_generator_positions` | 3 ケースの名前・寸法と、入力が生成器の位置 0・`2 * 64 * 64` と `GemmGateRow::Odd37x41x53` の値 |
| `order_cases_share_inputs_with_existing_cases_except_256` | 64³ と 37×41×53 の入力が `build_cases` の同じケースと同じ、256³ は違う |
| `simd128_order_hand_case` | m=1・k=6・n=1、`a` が `1.0e8`・`3.0` × 5、`b` が `1.0` × 6 で `simd128_order` が `1.0e8`。4 レーンの和を 3 番から足す式は `100000008.0`。`a` が `1.0e8`・`0.0` × 3・`4.0`・`5.0` で `100000008.0`。m=1・k=5・n=1、`a` が `-1.0`・`0.0` × 3・1 + 2^-12、`b` が `1.0` × 4・1 + 2^-12 で 2^-11 + 2^-24 |
| `simd128_order_rejects_bad_lengths` | `a` の長さが m×k でなければ `Err` |
| `simd128_order_rejects_unallocatable_output` | m=`usize::MAX`・k=0・n=1（`a`・`b` は空）で `simd128_order` が `Err(GenError::InputFailed)` |
| `square_from_rejects_unallocatable_inputs` | 1 辺の 2 乗は `usize` に収まり f32 のバイト数が `isize::MAX` を超える `GemmCaseSource::SquareFrom` で `build_case` が `Err(GenError::InputFailed)` |
| `cpu_reference_matches_core_and_default_scalar` | `gemm_cpu_reference` が `reference_gemm_f32` と `DefaultCpuOps::gemm_scalar` とビットまで同じ |
| `simd128_order_differs_from_cpu_reference_within_t1` | 3 ケースとも `simd128_order` は `gemm_cpu_reference` とビットの違う語を持ち、`compare_f32(.., T1)` は `is_match()` |
| `variants_differ_from_simd128_order_by_bits` | レーンの積和を `mul_add` にした式・4 レーンを 3 番から足す式・(0 番 + 1 番) + (2 番 + 3 番) の式は 3 ケースとも `simd128_order` とビットの違う語を持つ。端数を足さない式は k が 4 の倍数のケースで同じ、37×41×53 で全語違う |
| `order_manifest_entries_match_files` | manifest の 3 要素の鍵の並び・値・ファイル名と、5 つのファイルの中身と sha256 |
| `order_simd_rows_equal_to_native_reference_fail` | 保存の形の simd128 の 3 行の出力をネイティブの参照の写しにすると、`--recount --expect-cases` が終了コード 1 で、3 行の `recount-bits:` が 0 でない |

### 確かめ方

```sh
cargo test --manifest-path wasm/Cargo.toml -p wasm-parity-host --test gemm_order
cargo test --manifest-path wasm/Cargo.toml -p wasm-parity-host --test recount_expected
make test-wasm-node-check
```

観測した結果は `10 passed; 0 failed; 0 ignored`・`5 passed; 0 failed; 0 ignored`・`passed: 720, failed: 0`。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | Node での照合に語ごとのビットで直に比べる参照の種類を足す | #6940 |
| 2/3 | 行列積の simd128 版の和の順の参照と 9 行を照合の入力に足す | #6941 |
