# 要素ごとの演算の NaN・±Inf・±0 の確かめ（TASK-2456）

## NaN・±Inf・±0 の入力を当てる試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2456。親 Issue は #1073、この節（1/2）は #6958。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_elementwise_special.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::elementwise_scalar`・`elementwise_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 入力 | `CaseId::E2.input()`（TASK-977 のポインタ）。長さ 37。NaN 7・+Inf 6・-Inf 6・+0 6・-0 6 |
| 比べ | `compare_f32_with_special_values(.., T1)`（TASK-25 のポインタ） |
| 番兵 | `out` を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |
| 関連の頁 | `docs/cpu-ops-elementwise-check.md`（TASK-2455 のポインタ） |

### 試験

| 試験 | 入力 | 型とメソッド | 期待 |
| ---- | ---- | ------------ | ---- |
| `input_holds_special_values` | `CaseId::E2` | なし | 長さ 37、NaN・+Inf・-Inf・+0・-0 の件数が `[7, 6, 6, 6, 6]` |
| `relu_scalar_maps_nan_to_positive_zero` | `CaseId::E2` | 2 型の `elementwise_scalar`、`Relu` | 入力が NaN の位置の出力が `0.0_f32` のビット、出力に NaN が 0 件 |
| `relu_matches_case_reference_by_rule` | `CaseId::E2` | 2 型 × 2 メソッドの 4 通り、`Relu` | `CaseId::E2.reference()` と規則で比べ、`compared()` が 37、`exceeded()`・`non_finite_mismatch()` が 0 |
| `simd_matches_scalar_by_rule` | `CaseId::E2` の長さ 1〜37 の接頭辞 | 2 型の `elementwise_simd` と `DefaultCpuOps` の `elementwise_scalar`、`Relu`・`AddScalar(0.5)` | 規則で比べ、`compared()` が接頭辞の長さ、`exceeded()`・`non_finite_mismatch()` が 0 |
| `rule_detects_nan_passed_through` | `CaseId::E2` | `DefaultCpuOps` の `elementwise_scalar`、`Relu` | NaN の位置に入力の NaN を戻した列は `non_finite_mismatch()` が NaN の件数と等しい。零の符号だけを裏返した列は `is_match()` |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_special
```

観測した結果は `5 passed; 0 failed; 0 ignored`。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | NaN・±Inf・±0 の入力を当てる試験 | #6958 |
