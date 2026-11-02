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
- 4 メソッドはどれも `Result<(), ContractError>` を返す。長さが合わない・次元の積があふれる入力は `InvalidArgument` で返し、`out` には触れない。ただし `DefaultCpuOps` の `*_simd` は入力を検証せず、未対応の `UnsupportedExtensionDefault` を優先して返す。
- `ReferenceCpuOps` は 4 メソッドとも scalar の計算をする。`DefaultCpuOps` の `*_simd` は simd 版が無いので `UnsupportedExtensionDefault` を返し、scalar へ黙って落とさない。単体試験が確かめる。

## wasm32 の simd128 版（2/5）

`DefaultCpuOps` の `gemm_simd`・`elementwise_simd` が呼ぶ版は、関数の項に付けたビルド時の `cfg` で選ぶ。実行時に CPU の機能は調べない。
上の節の `*_simd` の記述は、下の表の「それ以外のビルド」の行の振る舞いに当たる。

| ビルド | `*_simd` が呼ぶもの |
| ---- | ---- |
| `cfg(all(target_arch = "wasm32", target_feature = "simd128"))` | 非公開の `crates/contract/src/cpu_ops/simd128.rs` の 4 レーンの版（長さが合わなければ `InvalidArgument`、`out` には触れない） |
| `cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))` | `UnsupportedExtensionDefault` |

- `simd128.rs` は unsafe を使わず、`core::arch::wasm32` の値渡しの関数だけを使う。
- `make build-profiles` は契約の crate の lib を wasm32 の scalar 構成と simd128 構成で `clippy -D warnings` にかける（lib のみ）。続けて wasm-abi の 2 版の release ビルドを流す。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --lib cpu_ops
```

wasm32 の 2 構成の lint は `make build-profiles` の中で流れる。単体では次を流す。

```sh
cargo clippy -p fandhe-silicon-contract --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops -- -D warnings
CARGO_ENCODED_RUSTFLAGS="$(printf -- '-C\037target-feature=+simd128')" cargo clippy -p fandhe-silicon-contract --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops --target-dir target/wasm32-simd128 -- -D warnings
```
