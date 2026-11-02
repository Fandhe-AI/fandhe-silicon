# CPU の演算（TASK-1351）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1351。親 Issue は #930、この節（1/5）は #6341。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| ファイル | `crates/contract/src/cpu_ops.rs`、`crates/contract/src/cpu_ops/scalar.rs`（非公開） |
| 公開のパス | `fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp}` |
| feature | `cpu-ops`（既定で有効） |

## 型と scalar 版（1/5）

| 名前 | 内容 |
| ---- | ---- |
| `UnaryElementwiseOp` | `Relu`・`AddScalar(f32)`（`#[non_exhaustive]`） |
| `CpuOps` | `gemm_scalar`・`gemm_simd`（`a, b, m, k, n, out`）、`elementwise_scalar`・`elementwise_simd`（`op, a, out`） |
| `DefaultCpuOps`・`ReferenceCpuOps` | `CpuOps` を実装する unit struct |

- GEMM は行優先の f32 で、各 `(i, j)` を k の昇順の `f32::mul_add` で積む。
- 要素ごと演算は `Relu` が `x.max(0.0)`、`AddScalar(c)` が `x + c`。
- 4 メソッドはどれも `Result<(), ContractError>` を返す。長さが合わない・次元の積があふれる入力は `InvalidArgument` で返し、`out` には触れない。
- `ReferenceCpuOps` は 4 メソッドとも scalar の計算をする。`DefaultCpuOps` の `*_simd` は simd 版が無いので `UnsupportedExtensionDefault` を返し、scalar へ黙って落とさない。単体試験が確かめる。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --lib cpu_ops
```
