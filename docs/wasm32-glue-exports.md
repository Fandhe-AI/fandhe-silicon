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

### 部分 Top-k（3/4）

#### 所在

実装と試験は `wasm/crates/glue/src/lib.rs`。wasm32 のときだけ wasm-bindgen の export になり、host では通常の関数。

| 関数 | Rust の署名 |
| ---- | ----------- |
| `topk_scalar` | `pub fn topk_scalar(scores: &[f32], k: usize, out: &mut [u8]) -> u32` |
| `topk_simd` | `pub fn topk_simd(scores: &[f32], k: usize, out: &mut [u8]) -> u32` |

#### 出力のバイトの並び

glue は `DefaultCpuOps` の `topk_partial_scalar`・`topk_partial_simd` の結果（`k` 件の `(スロット番号, スコア)`）をバイトに並べる。

| オフセット | 中身 |
| ---------- | ---- |
| `8*i` 〜 `8*i+3` | i 番目のスロット番号（u32、リトルエンディアン） |
| `8*i+4` 〜 `8*i+7` | i 番目のスコア（f32、リトルエンディアン） |

`DefaultCpuOps` が埋めない位置は、その番兵（`u32::MAX`・`f32::NEG_INFINITY`）のバイト `FF FF FF FF 00 00 80 FF` になる。

#### 返し

| 入力 | 戻り | `out` |
| ---- | ---- | ----- |
| `out.len()` が `k`×8 | 0 | `DefaultCpuOps` の結果のバイト |
| `k` が 0 で `out` が空 | 0 | 空のまま |
| `out.len()` が `k`×8 でない | 1 | 呼ぶ前のまま |
| `k`×8 が `usize` に収まらない | 1 | 呼ぶ前のまま |

#### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `topk_matches_default_cpu_ops` | 同点・`±0.0`・NaN・無限大を含む入力と、`k` が 0・件数以下・件数より大きい組で、戻りが 0 で `out` が同じ入力の `DefaultCpuOps` の結果のバイトと一致（scalar・simd） |
| `topk_bytes_layout` | 手で書いたバイト列と一致。`k` が有限の件数を超える組で末尾が番兵のバイト（scalar・simd） |
| `topk_rejects_output_length_mismatch` | `out.len()` が `k`×8 と違うと 1 で、`out` が呼ぶ前のまま。ちょうどなら 0（scalar・simd） |
| `topk_rejects_overflowing_count` | `k`×8 があふれる `k` で 1 で、`out` が呼ぶ前のまま（scalar・simd） |

#### 確かめ方

時点のコミット: `377a723`（この変更の土台）。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ grep -n wasm_bindgen wasm/crates/glue/src/lib.rs
25:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
34:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
47:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
58:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
79:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
95:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
137:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
147:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
```

### 完了の条件の確かめ（4/4）

glue の 7 つの export について、本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-2957）。

時点のコミット: `25a2535`（この変更の土台）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | 7 つの関数の export の宣言 | `grep -n -A1 wasm_bindgen wasm/crates/glue/src/lib.rs` | 8 組（`roundtrip_f32` と 7 つの関数）。出力は下のコードブロック |
| (2) | 同じ入力の `DefaultCpuOps` と同じ出力・戻り 0 | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list` と `cargo test --manifest-path wasm/Cargo.toml --workspace --locked` | `--list` に `tests::gemm_matches_default_cpu_ops: test`・`tests::elementwise_matches_default_cpu_ops: test`・`tests::topk_matches_default_cpu_ops: test`。3 つとも `... ok`。最終行は `test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| (3) | 拒否の 3 場面で出力が変わらず戻り 1 | 同じ `cargo test` | `elementwise_rejects_unassigned_op_code`・`gemm_rejects_mismatched_lengths`・`elementwise_rejects_mismatched_lengths`・`topk_rejects_output_length_mismatch`・`gemm_rejects_overflowing_dimensions`・`topk_rejects_overflowing_count` がいずれも `... ok` |
| (4) | Top-k のバイトの並び | 同じ `cargo test` | `topk_bytes_layout ... ok` |
| (5) | `detect_isa_bits` が host で 0 | 同じ `cargo test` | `detect_isa_bits_is_zero_on_host ... ok` |
| (6) | CI で流れる場所 | `gh run view 41925431798 --json status,conclusion,headSha,jobs` と、`deny` ジョブのログ | run 41925431798（`push`）の `status` は `completed`、`conclusion` は `success`、`headSha` は `25a2535dd9a530cf9b8f2507d017dc90306dc98d`。`make deny (cargo-deny)` ジョブは `success`、`wasm32 build + size + node (macos-latest)` ジョブは `success`。`deny` ジョブのログの `test result:` の行は下のコードブロック |
| (7) | unsafe・依存の追加 0 | TASK-2957 の 3 つのコミット（`000f060`・`01d8515`・`25a2535`）に絞った `git show --stat --format= <コミット> -- crates wasm/Cargo.toml wasm/crates/glue/Cargo.toml wasm/Cargo.lock wasm/deps-approved.toml Cargo.toml Cargo.lock deps-approved.toml` と `grep -n unsafe wasm/crates/glue/src/lib.rs` と `grep -n unsafe_code wasm/Cargo.toml` | `git show` の出力は空。`grep` の出力は下のコードブロック |

```console
$ grep -n -A1 wasm_bindgen wasm/crates/glue/src/lib.rs
25:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
26-pub fn roundtrip_f32(input: &[f32]) -> Vec<f32> {
--
34:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
35-pub fn gemm_scalar(a: &[f32], b: &[f32], m: usize, k: usize, n: usize, out: &mut [f32]) -> u32 {
--
47:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
48-pub fn gemm_simd(a: &[f32], b: &[f32], m: usize, k: usize, n: usize, out: &mut [f32]) -> u32 {
--
58:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
59-pub fn detect_isa_bits() -> u32 {
--
79:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
80-pub fn elementwise_scalar(op_code: u32, c: f32, a: &[f32], out: &mut [f32]) -> u32 {
--
95:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
96-pub fn elementwise_simd(op_code: u32, c: f32, a: &[f32], out: &mut [f32]) -> u32 {
--
137:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
138-pub fn topk_scalar(scores: &[f32], k: usize, out: &mut [u8]) -> u32 {
--
147:#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
148-pub fn topk_simd(scores: &[f32], k: usize, out: &mut [u8]) -> u32 {
$ gh run view 41925431798 --job 126908464503 --log
[wasm-verify.sh] cargo test (wasm/, host)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ grep -n unsafe wasm/crates/glue/src/lib.rs
8://! 手で書く unsafe は持たない。配列の受け渡しは wasm-bindgen の型の変換に任せる。
$ grep -n unsafe_code wasm/Cargo.toml
19:unsafe_code = "forbid"
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 行列積の 2 つの export と ISA のビット | #7995 |
| 2/4 | 要素ごとの演算の 2 つの export | #7996 |
| 3/4 | 部分 Top-k の 2 つの export | #7997 |
| 4/4 | 完了の条件の確かめ | #7998 |

親 Issue は #7993。
