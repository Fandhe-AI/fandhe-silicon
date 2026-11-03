# 部分 Top-k の並びの規則の確かめ（TASK-2450）

## 並びの規則を出力に直に当てる試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2450。親 Issue は #1069、この節（1/2）は #6944。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_topk_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`。既にある設定） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::topk_partial_scalar`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1352 のポインタ） |
| 入力 | 同じファイルの 20,000 件・k=64、同点ありの 200 件・k=10、非有限ありの 200 件・k=10 と `SIGNED_ZERO_INPUTS`（TASK-976 のポインタ）。生成は `SplitMix64`・`GEMM_INPUT_SEED`（TASK-1005 のポインタ） |

### 試験

期待の出力を CPU 参照から作らず、製品の出力を入力に照らして確かめる。

| 試験 | 確かめる値 |
| ---- | ---------- |
| `topk_order_rule_holds_directly` | 20,000 件・同点あり・`SIGNED_ZERO_INPUTS` の各入力で、出力が `assert_order_rule` を満たす |
| `topk_ties_cross_k_boundary_in_slot_order` | 同点ありの入力で、k 件目と同じスコアのスロットが、出力の内側ではスロット番号の昇順に並び、外側のどれよりも小さい |
| `topk_non_finite_slots_absent_directly` | 非有限ありの入力で、出力のスロットのスコアがすべて有限で、`assert_order_rule` を満たす |
| `topk_first_k_are_input_pairs` | 3 つの入力で、出力が k 件で、各件のスロット番号が番兵でなく、スコアが入力の同じ位置のスコアとビットまで一致する |

補助の関数 `assert_order_rule` は、出力が k 件であること、各件が入力の（添字, スコア）の組で有限であり添字が重複しないこと、隣り合う 2 件が `total_cmp` の降順（同点はスロット番号の昇順）であること、出力に入らなかった有限のスコアのどれもが最後の 1 件より前に並ばないことを確かめる。

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
```
