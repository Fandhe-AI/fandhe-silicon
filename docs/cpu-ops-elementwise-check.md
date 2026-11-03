# 要素ごとの演算の出力の全要素の確かめ（TASK-2455）

## 出力の全要素が書かれることの試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2455。親 Issue は #1072、この節（1/2）は #6954。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_elementwise_written.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::elementwise_scalar`・`elementwise_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 入力 | 長さ 1,000,003。生成は `SplitMix64`・`GEMM_INPUT_SEED`（TASK-24 のポインタ）で、`0.0` と等しい値は飛ばす |
| 番兵 | `out` を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |

### 試験

| 試験 | 入力の長さ | 型とメソッド | 期待 |
| ---- | ---------- | ------------ | ---- |
| `input_has_no_special_values` | 1,000,003 | なし | 長さが `4 * 250_000 + 3`、全要素が有限、`0.0` と等しい要素が 0 件 |
| `relu_writes_every_element` | 0〜9、1,000,000〜1,000,003 の接頭辞 | 2 型 × 2 メソッドの 4 通り、`Relu` | 番兵が 0 件、各要素が `x.max(0.0)` とビットまで一致 |
| `add_scalar_writes_every_element` | 同上 | 4 通り、`AddScalar(0.5)` | 番兵が 0 件、各要素が `x + 0.5` とビットまで一致 |
| `last_elements_are_written` | 1,000,003、6、7 | 4 通り × 2 演算 | 長さ 1,000,003 の末尾 3 要素と、長さ 6・7 の接頭辞の末尾の要素が、番兵でなく式とビットまで一致 |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_written
```

観測した結果は `4 passed; 0 failed; 0 ignored`。
