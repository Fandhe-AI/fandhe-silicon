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

## CPU 参照と比べる試験（3/5）

| 項目 | 内容 |
| ---- | ---- |
| ファイル | `crates/contract/tests/cpu_ops_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 比べの基準 | `fandhe_silicon_core::parity` の `reference_gemm_f32`・`compare_f32`・`T1`（TASK-24 のポインタ） |
| 入力 | `SplitMix64`・`GEMM_INPUT_SEED`・`GemmGateRow::Odd37x41x53`（TASK-1005 のポインタ） |

入力は公開の生成器だけで作る。GEMM は 64³・256³・37×41×53（M×N×K）の 3 ケース、要素ごと演算は長さ 1,000,003 で 0 と非有限を含まない列を使う。

| 試験 | 内容 |
| ---- | ---- |
| `gemm_scalar_matches_cpu_reference` | `DefaultCpuOps::gemm_scalar` が参照と T1 で一致し、ビットも一致する |
| `reference_ops_follow_scalar_path` | `ReferenceCpuOps` の 4 メソッドが `DefaultCpuOps` の scalar とビットで一致する（GEMM の 3 ケース、`Relu`・`AddScalar(0.5)`） |
| `elementwise_scalar_relu` | 各要素が `max(x, 0.0)` とビットで一致する |
| `elementwise_scalar_add_scalar` | 各要素が `x + 0.5` とビットで一致する |
| `mismatched_lengths_do_not_panic` | 長さが合わない・積があふれる入力でも 4 メソッドが panic せず `Err` を返す |
| `gemm_simd_is_unsupported_without_simd128`・`elementwise_simd_is_unsupported_without_simd128` | `cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))` のビルドで `DefaultCpuOps` の `*_simd` が `UnsupportedExtensionDefault` を返す |
| `gemm_simd_matches_cpu_reference`・`elementwise_simd_matches_scalar_bits` | `cfg(all(target_arch = "wasm32", target_feature = "simd128"))` のビルドで `*_simd` が参照（GEMM は T1）・scalar（要素ごと演算はビット。長さ 0〜9 も含む）と一致する |

## 版の選択の試験（4/5）

| 項目 | 内容 |
| ---- | ---- |
| ファイル | `crates/contract/tests/cpu_ops_build_selection.rs`（feature の条件なし） |
| 読む対象 | `src/cpu_ops.rs` と `src/cpu_ops/` 直下の `.rs` 全部（読めない・0 件は失敗） |

判定は `(ファイル名, 中身)` の列から問題の列を返す純粋関数で、外部 crate もコマンドの起動も使わない。

| 項目 | 見るもの |
| ---- | ---- |
| (a) | 実行時の検出の字句（`is_x86_feature_detected`・`is_aarch64_feature_detected`・`is_arm_feature_detected`・`is_wasm_feature_detected`・`std_detect`・`std::detect`）がどのファイルにも無い（コメントも数える） |
| (b) | `cpu_ops.rs` に `simd128` が有効な形と無効な形の `#[cfg(...)]` の行がそれぞれ 1 件以上ある |
| (c) | `core::arch::wasm32`・`std::arch::wasm32` の字句は `cpu_ops/simd128.rs` の中だけにある |
| (d) | `mod simd128;` の行がちょうど 1 行で、直前の属性が `simd128` が有効な形の `#[cfg(...)]` である |

| 試験 | 入力の改変 | 出る問題 |
| ---- | ---------- | -------- |
| `selection_is_by_build_cfg_only` | なし（実物） | 0 件 |
| `positive_control_runtime_detection` | `cpu_ops.rs` に検出の呼び出しの行を足す | (a) |
| `positive_control_runtime_detection_in_comment` | `scalar.rs` にコメントだけで検出の字句を足す | (a) |
| `positive_control_missing_not_cfg` | `cpu_ops.rs` から無効な形の `#[cfg(...)]` の行をすべて消す | (b) |
| `positive_control_wasm32_arch_outside_simd128` | `scalar.rs` に `core::arch::wasm32` の `use` を足す | (c) |
| `positive_control_simd128_file_missing` | `simd128.rs` を列から外す | (c) |
| `positive_control_simd128_mod_without_cfg` | `mod simd128;` の直前の 1 行を消す | (d) |

feature の条件が無いので、`make verify` の `cargo test` と `make build-profiles` の各組で流れ、`make wasm-verify` の wasm32 の型検査にも入る。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --lib cpu_ops
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection
```

wasm32 の 2 構成の lint は `make build-profiles` の中で流れる。単体では次を流す。

```sh
cargo clippy -p fandhe-silicon-contract --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops -- -D warnings
CARGO_ENCODED_RUSTFLAGS="$(printf -- '-C\037target-feature=+simd128')" cargo clippy -p fandhe-silicon-contract --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops --target-dir target/wasm32-simd128 -- -D warnings
```
