# 行列積の scalar 版の積和の順と特殊値の確かめ（TASK-2445）

## 積和の順と特殊値の入力の試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2445。親 Issue は #1066、この節（1/2）は #6932。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_gemm_scalar_rules.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::gemm_scalar`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 参照 | `reference_gemm_f32`・`reference_gemm_f32_descending_k`（TASK-24 のポインタ） |
| 有限の入力 | 64³（生成器の位置 0）・256³（位置 `2 * 64 * 64`）・`GemmGateRow::Odd37x41x53`（37×41×53。`a_offset()` は `2 * 64 * 64 + 2 * 256 * 256`。`GemmGateRow` の所在は TASK-1005 のポインタ） |
| 特殊値の入力 | `CaseId::G01`〜`G13`（TASK-977 のポインタ）。NaN・+Inf・-Inf・+0・-0 の件数の和は `[3, 6, 3, 59, 60]` |
| 比べ | `compare_f32`・`compare_f32_with_special_values`（`T1`。TASK-25 のポインタ） |
| 番兵 | 出力を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |
| 関連の頁 | `docs/cpu-ops-elementwise-special.md`（TASK-2456 のポインタ） |

### 試験

| 試験 | 入力 | 型 | 期待 |
| ---- | ---- | -- | ---- |
| `gemm_scalar_follows_ascending_k` | 有限の 3 つの入力 | 2 型の `gemm_scalar` | `reference_gemm_f32` と `compare_f32(.., T1)` で `compared()` が m×n・`is_match()`、ビットまで一致。`reference_gemm_f32_descending_k` とはビットが違う |
| `special_inputs_hold_special_values` | `G01`〜`G13` の `a`・`b` | なし | NaN・+Inf・-Inf・+0・-0 の件数の和が `[3, 6, 3, 59, 60]` |
| `gemm_scalar_output_classes_match_design` | `G01`〜`G13` | 2 型の `gemm_scalar` | 出力の長さが `design()` の長さ、番兵のビットが残らない、各要素の `classify` が `design()` と同じ |
| `gemm_scalar_single_element_classes` | `G01`〜`G07`・`G13` | 2 型の `gemm_scalar` | 出力 1 要素の区分が試験の中の表（NaN・+Inf・-Inf・-Inf・NaN・NaN・NaN・NaN）と同じ |
| `gemm_scalar_matches_case_reference_by_rule` | `G01`〜`G13` | 2 型の `gemm_scalar` | `compare_f32_with_special_values(id.reference(), 出力, T1)` で `compared()` が m×n、`exceeded()`・`non_finite_mismatch()` が 0、`is_match()` |
| `rule_detects_replaced_non_finite` | `G05`・`G02` | `DefaultCpuOps` の `gemm_scalar` | 先頭の出力を `0.0`（`G05`）・-Inf（`G02`）に置き換えた列は `non_finite_mismatch()` が 1、`is_match()` が偽、`classify` が `design()` と違う |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_gemm_scalar_rules
```

観測した結果は `6 passed; 0 failed; 0 ignored`。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 積和の順と特殊値の入力の試験 | #6932 |
