# wasm32 の glue の export の所在と本リポの現状（TASK-2957）

別 workspace `wasm/` の glue（cdylib）が持つ export の所在と、本リポの現状の事実をまとめる。

## 記録の所在

記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-2957。ポインタのみ）。本文は転記しない
（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## glue の export

### 行列積と ISA のビット（1/4）

#### 所在

実装と試験は `wasm/crates/glue/src/lib.rs`。wasm32 のときだけ wasm-bindgen の export になり、host では通常の関数。

| 関数 | Rust の署名 |
| ---- | ----------- |
| `gemm_scalar` | `pub fn gemm_scalar(a: &[f32], b: &[f32], m: usize, k: usize, n: usize, out: &mut [f32]) -> u32` |
| `gemm_simd` | `pub fn gemm_simd(a: &[f32], b: &[f32], m: usize, k: usize, n: usize, out: &mut [f32]) -> u32` |
| `detect_isa_bits` | `pub fn detect_isa_bits() -> u32` |

#### 行列積の返し

`gemm_scalar`・`gemm_simd` は契約の crate の `DefaultCpuOps` の同名のメソッドを呼び、`Ok` なら 0、`Err` なら 1 を返す。

| 入力 | 戻り | `out` |
| ---- | ---- | ----- |
| `a`・`b`・`out` の長さが `m`・`k`・`n` と合う | 0 | `DefaultCpuOps` の同名のメソッドと同じ値 |
| `m` か `n` が 0（`out` が空） | 0 | 空のまま |
| `a`・`b`・`out` のどれかの長さが合わない | 1 | 呼ぶ前のまま |
| `m`・`k`・`n` の積が `usize` に収まらない | 1 | 呼ぶ前のまま |

#### ISA のビット

| ビルド | `detect_isa_bits` の値 |
| ------ | ---------------------- |
| `all(target_arch = "wasm32", target_feature = "simd128")` が真 | 1 |
| 偽（host を含む） | 0 |

#### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `gemm_matches_default_cpu_ops` | 戻りが 0 で、`out` が同じ入力の `DefaultCpuOps` の `out` とビット列で一致（scalar・simd）。`m` か `n` が 0 でも 0 |
| `gemm_rejects_mismatched_lengths` | `a`・`b`・`out` の長さを 1 つずらすと 1 で、`out` が呼ぶ前のまま（scalar・simd） |
| `gemm_rejects_overflowing_dimensions` | `m = usize::MAX` の入力で 1 で、`out` が呼ぶ前のまま（scalar・simd） |
| `detect_isa_bits_is_zero_on_host` | host で 0 |

#### 確かめ方

時点のコミット: `5e4ccdd`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked
...
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ grep -n wasm_bindgen wasm/crates/glue/src/lib.rs
25:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
34:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
47:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
58:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
```

### 要素ごとの演算（2/4）

#### 所在

実装と試験は `wasm/crates/glue/src/lib.rs`。wasm32 のときだけ wasm-bindgen の export になり、host では通常の関数。

| 関数 | Rust の署名 |
| ---- | ----------- |
| `elementwise_scalar` | `pub fn elementwise_scalar(op_code: u32, c: f32, a: &[f32], out: &mut [f32]) -> u32` |
| `elementwise_simd` | `pub fn elementwise_simd(op_code: u32, c: f32, a: &[f32], out: &mut [f32]) -> u32` |

#### `op_code`

glue は `op_code` を契約の crate の `UnaryElementwiseOp` に読み替えてから `DefaultCpuOps` の同名のメソッドを呼び、`Ok` なら 0、`Err` なら 1 を返す。

| `op_code` | 演算 | 戻り |
| --------- | ---- | ---- |
| 0 | `Relu`（`c` は使わない） | 0 |
| 1 | `AddScalar(c)` | 0 |
| 2 以上 | なし | 1 |

#### 返し

| 入力 | 戻り | `out` |
| ---- | ---- | ----- |
| `op_code` が 0 か 1 で、`a` と `out` の長さが等しい | 0 | `DefaultCpuOps` の同名のメソッドと同じ値 |
| `op_code` が 0 か 1 で、`a` と `out` が空 | 0 | 空のまま |
| `op_code` が 2 以上（`a`・`out` が空でも） | 1 | 呼ぶ前のまま |
| `a` と `out` の長さが違う | 1 | 呼ぶ前のまま |

#### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `elementwise_matches_default_cpu_ops` | `op_code` 0・1 で戻りが 0 で、`out` が同じ入力の `DefaultCpuOps` の `out` とビット列で一致（scalar・simd）。`op_code` 0 で `c` を変えても `out` が同じ。空でも 0 |
| `elementwise_rejects_unassigned_op_code` | `op_code` が 2・3・`u32::MAX` で 1 で、`out` が呼ぶ前のまま（長さが合う・合わない・空、scalar・simd） |
| `elementwise_rejects_mismatched_lengths` | `op_code` 0・1 で `a` と `out` の長さが違うと 1 で、`out` が呼ぶ前のまま（scalar・simd） |

#### 確かめ方

時点のコミット: `000f060`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ grep -n wasm_bindgen wasm/crates/glue/src/lib.rs
25:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
34:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
47:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
58:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
79:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
95:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 行列積の 2 つの export と ISA のビット | #7995 |
| 2/4 | 要素ごとの演算の 2 つの export | #7996 |
| 3/4 | 部分 Top-k の 2 つの export | #7997 |
| 4/4 | 完了の条件の確かめ | #7998 |

親 Issue は #7993。
