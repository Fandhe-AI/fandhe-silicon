# 行列積の aarch64 の neon 版の和の順と k の端数の確かめ（TASK-2446）

## 和の順と k の端数の試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2446。親 Issue は #1067、この節（1/2）は #6936。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_gemm_neon_rules.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::gemm_simd`・`CpuOps::gemm_scalar`。型は `DefaultCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 入力 | 64³（生成器の位置 0）・256³（位置 `2 * 64 * 64`）・`GemmGateRow::Odd37x41x53`（37×41×53。`a_offset()` は `2 * 64 * 64 + 2 * 256 * 256`。`GemmGateRow` の所在は TASK-1005 のポインタ） |
| 4 レーンの参照 | 試験の中の `lane_reference`。`Form::Lanes` は、k を 4 つずつ 4 レーンに `mul_add` で積み、`(l0 + l1) + (l2 + l3)` で足し、端数の k を `mul_add` で足す。`Form::SplitLaneProduct` はレーンの積と和を別の演算にし、`Form::SerialLaneSum` は 4 レーンを `((l0 + l1) + l2) + l3` で足し、`Form::SplitTailProduct` は端数の積と和を別の演算にする |
| 比べ | `compare_f32`（`T1`。TASK-24 のポインタ） |
| 番兵 | 出力を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |
| 関連の頁 | `docs/cpu-ops-gemm-scalar-rules.md`（TASK-2445 のポインタ）・`docs/cpu-ops-gemm-wasm-rules.md`（TASK-2447 のポインタ） |

### 試験

| 試験 | `cfg` | 期待 |
| ---- | ----- | ---- |
| `three_inputs_have_the_expected_shapes` | なし | (m, k, n, k を 4 で割った余り) が `(64, 64, 64, 0)`・`(256, 256, 256, 0)`・`(37, 53, 41, 1)`。`a`・`b` の長さが m×k・k×n で、値が有限 |
| `lane_reference_differs_from_scalar_and_other_forms` | なし | 3 つの入力で、`Form::Lanes` の参照が `gemm_scalar` と `compare_f32(.., T1)` で `compared()` が m×n・`is_match()` で、ビットが違う。`Form::SplitLaneProduct`・`Form::SerialLaneSum` の式とビットが違う。`Form::SplitTailProduct` の式とは、37×41×53 でビットが違い、64³・256³ で同じ |
| `gemm_simd_writes_every_element_of_three_inputs` | なし | 3 つの入力で、`gemm_simd` の出力の長さが m×n で、番兵のビットが残らず、全要素が有限 |
| `neon_gemm_simd_matches_scalar_by_t1` | `all(target_arch = "aarch64", target_feature = "neon")` | 3 つの入力で、`gemm_simd` が `gemm_scalar` と `compare_f32(.., T1)` で `compared()` が m×n・`is_match()` で、ビットが違う |
| `neon_gemm_simd_equals_lane_reference_bits` | `all(target_arch = "aarch64", target_feature = "neon")` | 3 つの入力で、`gemm_simd` が `Form::Lanes` の参照とビットまで一致 |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_gemm_neon_rules
```

x86_64 のビルドで観測した結果は `3 passed; 0 failed; 0 ignored`（`cfg` を持つ 2 本はコンパイルされない）。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 和の順と k の端数の試験 | #6936 |
