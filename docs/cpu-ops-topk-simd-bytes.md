# 部分 Top-k の SIMD 版と scalar 版の出力の一致の確かめ（TASK-2452）

## SIMD 版と scalar 版の出力をバイトの列で比べる試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2452。親 Issue は #1071、この節（1/2）は #6952。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_topk_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`。既にある設定） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::topk_partial_scalar`・`CpuOps::topk_partial_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1352 のポインタ） |
| 入力 | 同じファイルの 20,000 件・k=64、同点ありの 200 件・k=10、非有限ありの 200 件・k=10。生成は `SplitMix64`・`GEMM_INPUT_SEED`（TASK-1005 のポインタ）。並びの規則の試験の頁は `docs/cpu-ops-topk-order.md`（TASK-2450 のポインタ） |

### 試験

| 試験 | 確かめる値 |
| ---- | ---------- |
| `topk_simd_scalar_byte_sequences_match` | 2 型のそれぞれ、3 つの入力のそれぞれで、scalar 版と SIMD 版を `out` の前詰めの値を別々にして呼ぶ。どちらの出力も前詰めの値を残さず、バイトの列の長さが `k` 件分で、違うバイトが 0。比べたバイトの数の合計が、3 つの入力の k の合計から求めた値と等しい |
| `topk_byte_count_detects_divergence` | 3 つの入力で、`DefaultCpuOps` の scalar 版の出力のバイトの列について、自分自身との比べが 0、最後の件のスコアのビットの最下位を反転した写しとの比べが 1、先頭 2 件を入れ替えた写しとの比べが 0 より大きい値、最後の 1 件を落とした写しとの比べが `None` |

補助の関数は次のとおり。

- `out_bytes`: 出力の各件のスロット番号とスコアのビットを、この順に並べたバイトの列にする。
- `differing_bytes`: 2 つのバイトの列の長さが違えば `None`、同じなら同じ位置の値が違うバイトの数を返す。
- `assert_overwritten`: 出力のどの件も、前詰めの値のスロット番号・スコアのビットを残していないことを確かめる。

同じファイルの既存の `topk_simd_matches_scalar_bytes` も、同じ 2 つのメソッドを（スロット番号、スコアのビット）の組で比べる。

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
```
