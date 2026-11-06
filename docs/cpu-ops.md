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
- 4 メソッドはどれも `Result<(), ContractError>` を返す。長さが合わない・次元の積があふれる入力は `InvalidArgument` で返し、`out` には触れない。`DefaultCpuOps` の `*_simd` も同じ（simd128 でないビルドは scalar と同じ計算。下の「7/9」の節）。
- `ReferenceCpuOps` は 4 メソッドとも scalar の計算をする。`DefaultCpuOps` の `*_simd` は simd128 でないビルドで scalar と同じ計算をする（下の「7/9」の節）。

補い: `CpuOps` はいまは 6 メソッドで、部分 Top-k の `topk_partial_scalar`・`topk_partial_simd` が足されている（下の「部分 Top-k の 2 メソッドと番兵」の節）。試験のファイルは `crates/contract/tests/cpu_ops_topk_reference.rs`。

今の値の補い（base 8c52335）: 部分 Top-k の並びの規則を出力に直に当てる試験は [cpu-ops-topk-order.md](cpu-ops-topk-order.md)。

## wasm32 の simd128 版（2/5）

`DefaultCpuOps` の `gemm_simd`・`elementwise_simd` が呼ぶ版は、関数の項に付けたビルド時の `cfg` で選ぶ。実行時に CPU の機能は調べない。
「それ以外のビルド」の行は 7/9 で `UnsupportedExtensionDefault` から scalar と同じ計算に変わった（下の「7/9」の節）。

| ビルド | `*_simd` が呼ぶもの |
| ---- | ---- |
| `cfg(all(target_arch = "wasm32", target_feature = "simd128"))` | 非公開の `crates/contract/src/cpu_ops/simd128.rs` の 4 レーンの版（長さが合わなければ `InvalidArgument`、`out` には触れない） |
| `cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))` | `scalar.rs` の関数（`*_scalar` と同じ計算） |

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
| （7/9 で削除）`*_is_unsupported_without_simd128` の 2 本 | 削除前は `UnsupportedExtensionDefault` を返すことを確かめていた。現在は下の「7/9」の節のとおり scalar と同じ計算 |
| `gemm_simd_matches_cpu_reference`・`elementwise_simd_matches_scalar_bits` | 7/9 以降はどのビルドでも `*_simd` が参照（GEMM は T1）・scalar（要素ごと演算はビット。長さ 0〜9 も含む）と一致する |

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

補い: 部分 Top-k の試験は次で流す。

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
```

wasm32 の 2 構成の lint は `make build-profiles` の中で流れる。単体では次を流す。

```sh
cargo clippy -p fandhe-silicon-contract --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops -- -D warnings
CARGO_ENCODED_RUSTFLAGS="$(printf -- '-C\037target-feature=+simd128')" cargo clippy -p fandhe-silicon-contract --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops --target-dir target/wasm32-simd128 -- -D warnings
```

## 完了の条件の確かめ（5/5）

5/5（#6345）で、本リポで観測できる事実だけを下の表に記録した。ポインタは `docs/spec/05-tasks.md` の TASK-1351。実行環境は GPU なしの Linux で、コード・試験・スクリプトは変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 型と trait が feature `cpu-ops` の下で `cpu_ops` に公開されている | `crates/contract/src/lib.rs` の 44〜45 行目（`#[cfg(feature = "cpu-ops")]` と `pub mod cpu_ops;`）、`cargo check -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops` | 行は上のとおり。`cargo check` は成功 |
| (2) | scalar の GEMM が CPU 参照と一致する | `cpu_ops_reference` の `gemm_scalar_matches_cpu_reference` | `ok` |
| (3) | `simd128` が無効なビルドで `DefaultCpuOps` の `*_simd` が `UnsupportedExtensionDefault` を返す | `gemm_simd_is_unsupported_without_simd128`・`elementwise_simd_is_unsupported_without_simd128` | どちらも `ok` |
| (4) | 要素ごと演算の値と `ReferenceCpuOps` が scalar と一致する | `elementwise_scalar_relu`・`elementwise_scalar_add_scalar`・`reference_ops_follow_scalar_path` | 3 本とも `ok` |
| (5) | 長さが合わない入力で panic しない | `mismatched_lengths_do_not_panic` | `ok` |
| (6) | wasm32 の scalar・simd128 の lint とビルド | `make build-profiles`（wasm32 の契約の crate の clippy 2 構成と wasm-abi の 2 版の release ビルドを含む） | 終了コード 0 |
| (7) | 版の選択がビルド時の `cfg` だけで決まる | `cpu_ops_build_selection` の 7 本（`selection_is_by_build_cfg_only` と陽性対照 6 本） | 7 本とも `ok` |
| (8) | 依存 0・unsafe 0 | `947283a..383b2e4` の `Cargo.lock`・`deps-approved.toml` の差分、`crates/contract/Cargo.toml` の差分、`cpu_ops.rs` と `cpu_ops/*.rs` の `unsafe` ブロック・関数・impl・trait・extern と `allow(unsafe_code)` の grep | 前の 2 つは差分 0。`Cargo.toml` は `[[test]] cpu_ops_reference` の 1 項目（+4 行）だけ。grep は 0 件。`lib.rs` は `#![deny(unsafe_code)]` |
| (9) | 試験の件数と CI | `cpu_ops_reference` は 7 本、`--lib cpu_ops` は 6 本が通過。`make verify`・`make wasm-verify` を流した。CI は `main` 上の `e0e245b`・`97b40e2`・`164b13a` の run を `gh run list` で見た | `make verify`・`make wasm-verify` は終了コード 0。3 つの run の結論は `success` |

`gemm_simd_matches_cpu_reference` と `elementwise_simd_matches_scalar_bits` は `cfg(all(target_arch = "wasm32", target_feature = "simd128"))` の下にだけあり、この環境のネイティブの試験では実行されない。上の表に入れていない。

確かめた base は `383b2e4`。完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1351）。

### 1/5〜4/5 の対応

| 段 | PR | コミット |
| -- | -- | -------- |
| 1/5 | #7786 | `e0e245b` |
| 2/5 | #7787 | `97b40e2` |
| 3/5 | #7788 | `164b13a` |
| 4/5 | #7789 | `383b2e4` |

### 追加で流したコマンド

```text
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference
cargo test -p fandhe-silicon-contract --lib cpu_ops
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection
make build-profiles
make verify
make wasm-verify
```

### 再実行の契機

`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_*.rs` を変えたとき、`scripts/build-profiles.sh` の wasm32 の lint の行を変えたとき、`UnaryElementwiseOp` に値が増えたときに、表を取り直す。

## simd128 でないビルドの版（7/9）

`DefaultCpuOps` の `*_simd` が呼ぶものは、ビルド時の `cfg` で決まる（実行時に CPU の機能を調べない）。

| ビルド | `*_simd` が呼ぶもの |
| ------ | ------------------- |
| `cfg(all(target_arch = "wasm32", target_feature = "simd128"))` | `simd128.rs` の 4 レーンの版 |
| `cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))` | `scalar.rs` の関数（`*_scalar` と同じ計算） |

4 メソッドの戻りは `Result<(), ContractError>` のまま。長さが合わない・積があふれる入力は、どのビルドでも 4 メソッド × 2 型が `InvalidArgument` を返し、`out` に書かない。

### 試験の変更

| 場所 | 変更 |
| ---- | ---- |
| lib（`cpu_ops`） | 2 本を `default_simd_values` に統合（`cfg` なし） |
| `cpu_ops_reference` | `*_is_unsupported_without_simd128` の 2 本を削除。`gemm_simd_matches_cpu_reference`・`elementwise_simd_matches_scalar_bits` の `cfg` を外した。長さの試験に `DefaultCpuOps` の `*_simd` を追加 |
| `simd128.rs` の単体試験 | 1 本減 |

この節より前の節の `UnsupportedExtensionDefault` の記述と `*_is_unsupported_without_simd128` の行は、7/9 の前の形。5/5 の確かめ表の行 (3) は 5/5 の時点の記録で、書き換えていない（現状は上の表と試験の変更のとおり）。

補い: 上の試験の変更の表の「`simd128.rs` の単体試験 | 1 本減」は誤りで、`simd128.rs` の `#[test]` は 6 本のまま（`grep -c '#\[test\]' crates/contract/src/cpu_ops/simd128.rs` が 6）。

### 確かめ方

```text
cargo test -p fandhe-silicon-contract --lib cpu_ops
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection
RUSTFLAGS="-C target-feature=+simd128" cargo clippy -p fandhe-silicon-contract --tests --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops,test-support --target-dir target/wasm32-simd128 -- -D warnings
```

simd128 の構成は型検査と lint までで、wasm32 上の実行は流していない（TASK-1351）。

## simd128 の構成の試験の型検査と lint（8/9）

`make wasm-verify` の中で、契約の crate の試験の wasm32 の型検査（既定の feature）の後、wasm32 のコンパイル試験の検査の前に、次の clippy を 1 回流す。

| 項目 | 値 |
| ---- | -- |
| コマンド | `cargo clippy -p fandhe-silicon-contract --tests --locked --target wasm32-unknown-unknown --no-default-features -- -D warnings` |
| feature | `contract-core,cpu-isa,cpu-ops,test-support` |
| rustflags | 利用者の環境の rustflags 系 4 つを外し、`CARGO_ENCODED_RUSTFLAGS` で `-C target-feature=+simd128` だけを足す |
| target ディレクトリ | `<target_directory>/wasm32-simd128`（`make build-profiles` の simd128 の lint と同じ置き場） |

型検査と lint までで、wasm32 上の試験の実行はしない（runner は足していない）。

### 確かめ方

```text
make wasm-verify
CARGO_ENCODED_RUSTFLAGS="$(printf -- '-C\037target-feature=+simd128')" cargo clippy -p fandhe-silicon-contract --tests --locked --target wasm32-unknown-unknown --no-default-features --features contract-core,cpu-isa,cpu-ops,test-support --target-dir target/wasm32-simd128 -- -D warnings
```

## 完了の条件の確かめ（9/9）

9/9（#7795）で、8/9 の後の `main` で観測できる事実を下の表に記録した。ポインタは `docs/spec/05-tasks.md` の TASK-1351。実行環境は GPU なしの Linux で、確かめた base は `abd6c95`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 型と trait が feature `cpu-ops` の下で `cpu_ops` に公開されている | `grep -n -B1 'pub mod cpu_ops' crates/contract/src/lib.rs`、`cargo check -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops` | 44 行目が `#[cfg(feature = "cpu-ops")]`、45 行目が `pub mod cpu_ops;`。`cargo check` は成功 |
| (2) | scalar の GEMM が CPU 参照と T-1 の範囲で一致する | `cpu_ops_reference` の `gemm_scalar_matches_cpu_reference` | `ok` |
| (3) | そのビルドの `gemm_simd` が CPU 参照と T-1 の範囲で一致する | `cpu_ops_reference` の `gemm_simd_matches_cpu_reference`（`cfg` なし）、`main` 上の `fa47f60` の run のジョブ `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`（`gh run view 41516178771 --json jobs`） | 試験は `ok`。2 つのジョブの結論は `success` |
| (4) | 要素ごと演算の値と、`elementwise_simd` が `elementwise_scalar` とビット単位で一致する（長さ 1,000,003 を含む） | `elementwise_scalar_relu`・`elementwise_scalar_add_scalar`・`reference_ops_follow_scalar_path`・`elementwise_simd_matches_scalar_bits` | 4 本とも `ok` |
| (5) | wasm32 の scalar 版・simd128 版のビルドと lint | `make build-profiles`（`contract wasm32 scalar`・`contract wasm32 simd128` の clippy と wasm-abi の 2 版の release ビルド）、`make wasm-verify`（`cargo clippy (crates/contract tests, wasm32-unknown-unknown, simd128) -D warnings` の段） | どちらも終了コード 0 |
| (6) | 版の選択がビルド時の `cfg` だけで決まる | `cpu_ops_build_selection` | 7 本とも `ok` |
| (7) | 依存 0・unsafe 0 | `git diff --stat 947283a..HEAD -- Cargo.lock deps-approved.toml`、`git diff --stat 947283a..HEAD -- crates/contract/Cargo.toml`、`grep -rc unsafe crates/contract/src/cpu_ops.rs crates/contract/src/cpu_ops/`、`unsafe {`・`unsafe fn`・`unsafe impl`・`unsafe trait`・`unsafe extern`・`allow(unsafe_code)` の grep、`grep -n 'deny(unsafe_code)' crates/contract/src/lib.rs` | 前の 2 つのうち `Cargo.lock`・`deps-approved.toml` は差分 0、`Cargo.toml` は +4 行。`unsafe` の語の grep は `simd128.rs` が 1、ほかは 0。絞った grep は 0 件。`lib.rs` の 11 行目は `#![deny(unsafe_code)]` |
| (8) | 試験の件数と実行 | `cargo test -p fandhe-silicon-contract --lib cpu_ops`、`cpu_ops_reference`、`make verify`・`make build-profiles`・`make wasm-verify` | `--lib cpu_ops` は 6 本、`cpu_ops_reference` は 7 本が通過。3 つの `make` は終了コード 0 |

5/5 の節の表は 5/5 の時点の記録で、書き換えていない。

### 追加で流したコマンド

```text
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference
cargo test -p fandhe-silicon-contract --lib cpu_ops
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection
make build-profiles
make verify
make wasm-verify
make wasm-build-check
```

`make wasm-build-check` の `cfg:` の行は `sites=5 label=match`（[wasm32-build](wasm32-build.md) の補い）。

### 再実行の契機

`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_*.rs` を変えたとき、`scripts/build-profiles.sh` の wasm32 の lint の行や `scripts/wasm-verify.sh` の simd128 の lint の段を変えたとき、`UnaryElementwiseOp` に値が増えたときに、表を取り直す。

## 部分 Top-k の 2 メソッドと番兵（TASK-1352 の 1/3）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1352。親 Issue は #931、この節（1/3）は #6349。

上の節の `*_simd` の記述は `gemm_simd`・`elementwise_simd` に当たる。`topk_partial_simd` はどのビルドでも `topk_partial_scalar` と同じ計算をする。

| 項目 | 内容 |
| ---- | ---- |
| `CpuOps::topk_partial_scalar` | 引数 `scores: &[f32], k: usize, out: &mut [(u32, f32)]`、戻り `Result<(), ContractError>` |
| `CpuOps::topk_partial_simd` | 同じ引数・戻り。`DefaultCpuOps`・`ReferenceCpuOps` とも `topk_partial_scalar` と同じ関数を呼ぶ |
| `ReferenceCpuOps::SENTINEL` | `(u32::MAX, f32::NEG_INFINITY)` |

- 有限のスコアだけが候補になる
- スコアの降順（`f32::total_cmp`。`-0.0` は `+0.0` より小さい）、同点は `scores` の添字の昇順で、上位を `(添字, スコア)` で `out` の先頭に書く
- 書く件数は `k`・有限のスコアの件数・`out` の長さの最小で、残りの位置は `SENTINEL` で埋める
- `k` が 0・`scores` が空・`out` が空・`out` と `k` の長さが違う場合も `Err` を返さない
- ヒープの確保をしない

確かめ方は `cargo test -p fandhe-silicon-contract --lib cpu_ops`。

## 部分 Top-k を CPU 参照と比べる試験（TASK-1352 の 2/3）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1352。親 Issue は #931、この節（2/3）は #6350。

| 項目 | 内容 |
| ---- | ---- |
| ファイル | `crates/contract/tests/cpu_ops_topk_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 比べの基準 | `fandhe_silicon_core::parity::topk` の `reference_topk_f32`・`compare_topk`（TASK-24 のポインタ） |
| 入力 | `SplitMix64`・`GEMM_INPUT_SEED`（TASK-1005 のポインタ）と `SIGNED_ZERO_INPUTS`（TASK-976 のポインタ） |

入力は 20,000 件・k=64、同点ありの 200 件・k=10、非有限ありの 200 件・k=10、`SIGNED_ZERO_INPUTS` の全件。`DefaultCpuOps`・`ReferenceCpuOps` の 2 型と `topk_partial_scalar`・`topk_partial_simd` の 2 メソッドの 4 つの組を公開のパスから呼ぶ。

| 試験 | 内容 |
| ---- | ---- |
| `topk_scalar_matches_cpu_reference` | `DefaultCpuOps::topk_partial_scalar` の出力が CPU 参照と位置ごとにスロット番号・スコアのビットまで一致する（符号付きの零の入力は `expected()` とも） |
| `topk_excludes_non_finite` | 4 つの組の出力に非有限のスコアも、非有限の位置のスロット番号も入らない |
| `topk_fills_first_k` | 4 つの組の出力の k 件がすべて有限のスコアで、番兵のスロット番号でない |
| `topk_simd_matches_scalar_bytes` | 2 型それぞれで `topk_partial_simd` の出力が `topk_partial_scalar` とスロット番号・スコアのビットまで一致する |
| `topk_reference_ops_follow_scalar_path` | `ReferenceCpuOps` の 2 メソッドの出力が `DefaultCpuOps::topk_partial_scalar` と一致する |

確かめ方は `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference`。

## 部分 Top-k の out が k より長い入力の試験（TASK-1352）

`crates/contract/tests/cpu_ops_topk_reference.rs` の `topk_longer_out_keeps_sentinel_tail`。

| 項目 | 内容 |
| ---- | ---- |
| 入力 | 20,000 件・k=8、同点ありの 200 件・k=5、非有限ありの 200 件・k=3。どれも有限のスコアの件数が k を超え、`out` の長さは `k + 7` |
| 呼び方 | `DefaultCpuOps`・`ReferenceCpuOps` の 2 型と `topk_partial_scalar`・`topk_partial_simd` の 2 メソッドの 4 つの組 |
| 確かめる値 | 戻りが `Ok`。先頭の `k` 件が CPU 参照と一致する。`k` 件目より後の位置がすべて `ReferenceCpuOps::SENTINEL` と（スコアはビットまで）等しい |

書く件数を `out` の長さにする変更、番兵の埋めを外す変更、書く件数を 1 件減らす変更は、この試験が落とす。

確かめ方は `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference`。

## 部分 Top-k の完了の条件の確かめ（TASK-1352 の 3/3）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1352。親 Issue は #931、この節（3/3）は #6351。

実行環境は GPU なしの Linux で、確かめた base は `f7f37dd`。コード・試験・スクリプトは変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 2 メソッドと `ReferenceCpuOps::SENTINEL` が feature `cpu-ops` の下の `cpu_ops` に公開されている | `grep -n 'fn topk_partial_' crates/contract/src/cpu_ops.rs`、`grep -n 'SENTINEL' crates/contract/src/cpu_ops.rs`、`cargo check -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops` | `fn topk_partial_` は 70・78・177・185・243・251 行目の 6 箇所、`pub const SENTINEL` は 201 行目。`cargo check` は成功 |
| (2) | scalar 版が CPU 参照と T-2 で一致する（3 ケースと符号付きの零の入力） | `cpu_ops_topk_reference` の `topk_scalar_matches_cpu_reference` | `ok` |
| (3) | 出力に非有限のスロットが 0 件 | `topk_excludes_non_finite` | `ok` |
| (4) | 先頭 k 件が埋まる | `topk_fills_first_k` | `ok` |
| (5) | `topk_partial_simd` が `topk_partial_scalar` とバイトで一致し、`ReferenceCpuOps` が scalar と一致する | `topk_simd_matches_scalar_bytes`・`topk_reference_ops_follow_scalar_path` | 2 本とも `ok` |
| (6) | wasm32 の scalar 版・simd128 版のビルドと lint | `make build-profiles`（契約の crate の wasm32 の clippy 2 構成と wasm-abi の 2 版の release ビルド）、`make wasm-verify`（契約の crate の試験の simd128 の構成の clippy の段を含む） | どちらも終了コード 0 |
| (7) | 版の選択がビルド時の `cfg` だけで決まる | `cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection` | 7 本とも `ok` |
| (8) | 依存 0・unsafe 0・確保 0 | `git diff --stat 3b6fa07..HEAD -- Cargo.lock deps-approved.toml`、`git diff --stat 3b6fa07..HEAD -- crates/contract/Cargo.toml crates/contract/src/cpu_ops/simd128.rs`、`grep -c unsafe crates/contract/src/cpu_ops.rs crates/contract/src/cpu_ops/scalar.rs`、`grep -nE 'Vec\|Box\|collect\|to_vec' crates/contract/src/cpu_ops/scalar.rs` | `Cargo.lock`・`deps-approved.toml` は差分 0。`Cargo.toml` は +4 行（`[[test]] cpu_ops_topk_reference` の 1 項目）、`simd128.rs` は差分 0。`unsafe` の語は 2 ファイルとも 0。確保の語の grep は 0 件 |
| (9) | CI で流れる場所と件数 | `cargo test -p fandhe-silicon-contract --lib cpu_ops`、`cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference`、`make verify`、`main` 上の `ab116f2`・`a60a547` の run（`gh run view <id> --json conclusion`） | `--lib cpu_ops` は 13 本、`cpu_ops_topk_reference` は 5 本が通過。`make verify` は終了コード 0。run 41532826696（`ab116f2`）・run 41535712313（`a60a547`）の結論はどちらも `success` |

今の値の補い（base 8c52335）: 上の (9) の `cpu_ops_topk_reference` の本数は、いまは 10 本（`10 passed; 0 failed; 0 ignored`）。足された試験は部分 Top-k の並びの規則を出力に直に当てるもので、[cpu-ops-topk-order.md](cpu-ops-topk-order.md) に書いてある。

### 1/3〜2/3 の対応

| 段 | PR | コミット |
| -- | -- | -------- |
| 1/3 | #7810 | `ab116f2` |
| 2/3 | #7813 | `a60a547` |

### 追加で流したコマンド

```text
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
cargo test -p fandhe-silicon-contract --lib cpu_ops
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection
make build-profiles
make wasm-verify
make verify
```

### 再実行の契機

`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_topk_reference.rs` を変えたとき、`scripts/build-profiles.sh` の wasm32 の lint の行や `scripts/wasm-verify.sh` の simd128 の lint の段を変えたときに、表を取り直す。

## 今の値の補い（base 25d8dc2）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 所在・上の `base 8c52335` の補い | 指している頁は [cpu-ops-topk-order.md](cpu-ops-topk-order.md) だけ → 次の頁が加わった。要素ごとの演算の出力の全要素の試験は [cpu-ops-elementwise-check.md](cpu-ops-elementwise-check.md)、部分 Top-k の SIMD 版と scalar 版のバイトの比べは [cpu-ops-topk-simd-bytes.md](cpu-ops-topk-simd-bytes.md)、公開の型と feature の有無の確かめは [cpu-ops-surface-check.md](cpu-ops-surface-check.md) | `ls docs/cpu-ops*.md` |
| 完了の条件の確かめ（5/5）の (9) | `--lib cpu_ops` は 6 本 → 13 本（`13 passed; 0 failed; 0 ignored`）。`cpu_ops_reference` は 7 本 → 8 本（`8 passed; 0 failed; 0 ignored`） | `cargo test -p fandhe-silicon-contract --features test-support --lib cpu_ops`、`cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference` |
| 完了の条件の確かめ（9/9）の (8) | `--lib cpu_ops` は 6 本 → 13 本。`cpu_ops_reference` は 7 本 → 8 本 | 上と同じ |
| 完了の条件の確かめ（5/5）の (8) | `947283a..383b2e4` の `Cargo.toml` は +4 行 → 同じ範囲は今も +4 行（`1 file changed, 4 insertions(+)`） | `git diff --stat 947283a..383b2e4 -- crates/contract/Cargo.toml` |
| 完了の条件の確かめ（9/9）の (7) | `947283a..HEAD` の `Cargo.toml` は +4 行 → `947283a..25d8dc2` で +16 行（`[[test]]` の 4 項目 `cpu_ops_reference`・`cpu_ops_topk_reference`・`cpu_ops_elementwise_written`・`cpu_ops_public_surface` が各 +4 行）。`Cargo.lock`・`deps-approved.toml` は差分なし | `git diff --stat 947283a..HEAD -- crates/contract/Cargo.toml Cargo.lock deps-approved.toml`、`git log --format='%h %s' 947283a..HEAD -- crates/contract/Cargo.toml` |
| 部分 Top-k の完了の条件の確かめ（3/3）の (8) | `3b6fa07..HEAD` の `Cargo.toml` は +4 行、`simd128.rs` は差分 0 → `3b6fa07..25d8dc2` で `Cargo.toml` は +12 行、`simd128.rs` は +5 行・-2 行。`simd128.rs` の差分は `f8b7a29`（#7880）の分。`Cargo.lock`・`deps-approved.toml` は差分なし | `git diff --stat 3b6fa07..HEAD -- Cargo.lock deps-approved.toml crates/contract/Cargo.toml crates/contract/src/cpu_ops/simd128.rs`、`git log --format='%h %s' 3b6fa07..HEAD -- crates/contract/Cargo.toml crates/contract/src/cpu_ops/simd128.rs` |
| 部分 Top-k の完了の条件の確かめ（3/3）の (9) と上の `base 8c52335` の補い | `cpu_ops_topk_reference` は 10 本 → 12 本（`12 passed; 0 failed; 0 ignored`）。足された 2 本の所在は [cpu-ops-topk-simd-bytes.md](cpu-ops-topk-simd-bytes.md) | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` |
| simd128 の構成の試験の型検査と lint（8/9） | 次の clippy を 1 回流す → 2 段になった。simd128 の段（`scripts/wasm-verify.sh` の 59 行目の `log`）に続けて、simd128 なしの wasm32 の scalar の段（68 行目の `log`。`--target-dir` は `wasm32-scalar`）を同じ feature の組で流す。`make wasm-verify` は終了コード 0 | `grep -n 'cargo clippy (crates/contract tests' scripts/wasm-verify.sh`、`make wasm-verify` |

今の値の補いとして、次の試験が加わっている。

| 試験 | 所在 | 本数 |
| ---- | ---- | ---- |
| `AddScalar` を 0.5 以外の定数（`-2.5`・`1.0`・`3.75`・`0.3`・`-0.125`・`1.0e6`）で確かめる `elementwise_add_scalar_other_constants`（#7917） | `crates/contract/tests/cpu_ops_reference.rs` の 277 行目。`DefaultCpuOps`・`ReferenceCpuOps` の scalar 版・SIMD 版の出力を、長さ 11 通りで `x + c` とビットまで比べる | `cpu_ops_reference` の 8 本のうちの 1 本 |
| `cpu_ops_feature_sets`（#7926） | `crates/contract/tests/cpu_ops_feature_sets.rs`（`Cargo.toml` に `[[test]]` の項目は無い）。`contract-core` から含意を辿った先に `cpu-ops` が無いことと、`scripts/build-profiles.sh` の `NATIVE_SETS` に `contract-core` だけの組の行があることを、文字列から確かめる | 4 本（`cargo test -p fandhe-silicon-contract --test cpu_ops_feature_sets` が `4 passed; 0 failed; 0 ignored`） |
| `CpuOpsNonExhaustiveGate`（#7926） | `crates/contract/src/cfg_gate_doctest.rs` の 142 行目の目印の型（rustdoc の表示は `line 131`）。`cpu-ops` が有効なときは `compile_fail,E0004`、無効なときは `ignore` | doctest 1 本（詳細は [cpu-ops-surface-check.md](cpu-ops-surface-check.md)） |

`--lib cpu_ops` の本数は `--features test-support` を付けなくても 13 本（`cargo test -p fandhe-silicon-contract --lib cpu_ops`）。`cpu_ops_build_selection` は今も 7 本（`cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection`）。

## 今の値の補い（base 42731d8）

記録時点の表と既存の節は書き換えない。以下は base `42731d8` で観測した今の値（実行環境は GPU なしの Linux。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「simd128 の構成は型検査と lint までで、wasm32 上の実行は流していない」の文（`:190`） | wasm32 上の実行は流していない → `wasm/crates/glue` の export（`topk_scalar`・`topk_simd`・`elementwise_scalar`・`elementwise_simd` を含む）を通して、`make wasm-node-check` が Node 上で `cpu_ops` の演算を流す（CI の `wasm-macos` ジョブ）。`crates/wasm-abi` の export を通した実行は無く、`crates/contract` の試験そのものを wasm32 で流してはいない | `grep -n 'pub fn' wasm/crates/glue/src/lib.rs`、`jq -r '.rows[].export' wasm/node/cases.json \| sort -u` |
| 回帰テスト `make test-wasm-node-check` の最終行 | `passed: 505, failed: 0` | `make test-wasm-node-check` |

## 今の値の補い（base e416527）

記録時点の表と既存の節は書き換えない。以下は base `e416527` で観測した今の値（実行環境は GPU なしの Linux。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「今の値の補い（base 25d8dc2）」の `cpu_ops_topk_reference` の本数（`:357`） | 12 本 → 18 本（`18 passed; 0 failed; 0 ignored`）。足された 6 本は -0.0 を含む入力を扱う試験で、所在は [cpu-ops-topk-signed-zero.md](cpu-ops-topk-signed-zero.md) | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` |
| 「今の値の補い（base 42731d8）」の `make test-wasm-node-check` の最終行（`:377`） | `passed: 505, failed: 0` → `passed: 544, failed: 0` | `make test-wasm-node-check` |
| 「今の値の補い（base 42731d8）」の `wasm/crates/glue` の export を通した実行（`:190` の文の今の値） | `make wasm-node-check` が Node 上で流す行は、ビットの完全一致で比べるものに加えて、`gemm_simd`・`simd128` の行を閾値で語ごとに比べるもの（`t1:` の行を出す）がある。所在は [wasm32-native-parity.md](wasm32-native-parity.md) の「Node での照合の閾値の比べ（1/4）」 | `grep -n 't1\|T1' scripts/wasm-node-check.sh` |

## 今の値の補い（base 0f8b01c）

記録時点の表と既存の節は書き換えない。以下は base `0f8b01c` で観測した今の値（実行環境は GPU なしの Linux x86_64。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 所在・頁の一覧（`:351`） | `cpu-ops-topk-order.md` などの一覧 → 要素ごとの演算の NaN・±Inf・±0 の確かめの頁 [cpu-ops-elementwise-special.md](cpu-ops-elementwise-special.md) が加わった | `ls docs/cpu-ops*.md` |
| `Cargo.toml` の差分（`:355`） | `947283a..25d8dc2` で +16 行 → `947283a..0f8b01c` で +20 行（`[[test]]` の 5 項目。`cpu_ops_elementwise_special` が加わった）。`Cargo.lock`・`deps-approved.toml` は差分なし | `git diff --numstat 947283a..0f8b01c -- crates/contract/Cargo.toml Cargo.lock deps-approved.toml` |
| `Cargo.toml` の差分（`:356`） | `3b6fa07..25d8dc2` で +12 行 → `3b6fa07..0f8b01c` で +16 行。`simd128.rs` は +5 行・-2 行 → +3 行・-2 行。`Cargo.lock`・`deps-approved.toml` は差分なし | `git diff --numstat 3b6fa07..0f8b01c -- crates/contract/Cargo.toml crates/contract/src/cpu_ops/simd128.rs Cargo.lock deps-approved.toml` |
| 試験の表（`:360`） | `cpu_ops_elementwise_special`（`Cargo.toml` に `[[test]]`、`required-features = ["cpu-ops", "test-support"]`）が加わった。5 本: `input_holds_special_values`・`relu_scalar_maps_nan_to_positive_zero`・`relu_matches_case_reference_by_rule`・`rule_detects_nan_passed_through`・`simd_matches_scalar_by_rule`（`5 passed; 0 failed; 0 ignored`）。所在は [cpu-ops-elementwise-special.md](cpu-ops-elementwise-special.md) | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_special` |
| `make test-wasm-node-check` の最終行（`:386`） | `passed: 544, failed: 0` → `passed: 642, failed: 0` | `make test-wasm-node-check` |
| 閾値の比べの行（`:387`） | `gemm_simd`・`simd128` の行を閾値で語ごとに比べる → 特殊値の入力の 2 種類（`gemm-special`・`elementwise-special`）の 4 組も閾値で比べる。所在は [wasm32-native-parity.md](wasm32-native-parity.md) | `sed -n 28,40p scripts/wasm-node-check.sh` |

## aarch64 の NEON 版（11/12）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1351。親 Issue は #930、この節（11/12）は #8055。

この節より前の節の `x.max(0.0)` の字句と `cfg` の 2 枝の説明は 11/12 の前の形。現状は下の「所在」と「動き」のとおり（`grep -n 'max(0.0)' crates/contract/src/cpu_ops/*.rs` が 0 件）。

### 所在

| 対象 | 所在 |
| ---- | ---- |
| NEON 版の計算 | `crates/contract/src/cpu_ops/neon.rs`（`pub(super)` の `gemm`・`elementwise`） |
| 版の選び分け | `crates/contract/src/cpu_ops.rs` の `gemm_simd_impl`・`elementwise_simd_impl`。`simd128`・`neon`・scalar の 3 枝。scalar の枝は `not(all(wasm32, simd128))` と `not(all(aarch64, neon))` の 2 行の `cfg` を積む |
| `Relu` の 1 要素の式 | `crates/contract/src/cpu_ops/scalar.rs` の `relu`。scalar・simd128 の端数・neon の端数が呼ぶ |
| `unsafe` の個数テスト | `crates/contract/tests/cpu_ops_neon_unsafe_count.rs`。登録は `tests/common/mod.rs` の `UNSAFE_MODULES` |
| 方針 | [unsafe-policy.md](unsafe-policy.md) |

### 動き

- `cfg(all(target_arch = "aarch64", target_feature = "neon"))` のビルドで、`DefaultCpuOps` の `gemm_simd`・`elementwise_simd` が `neon.rs` の 4 レーンの版を呼ぶ。`topk_partial_simd` は変わらない。
- GEMM: k を 4 つずつ `vfmaq_f32` で 4 レーンに蓄え、`vaddvq_f32` で水平和を取る。k の端数は昇順の `f32::mul_add`。
- `Relu`: `vcgtq_f32` の比較のマスクと `vandq_u32`。`AddScalar(c)`: `vaddq_f32`。4 要素の分割は `as_chunks::<4>`、端数は `relu(x)` と `x + c`。
- 長さが合わない入力は、`out` に触れず simd128 版と同じ文字列の `InvalidArgument` を返す。
- 読み書きは長さ 4 の配列と lane の設定・取り出しで、生のポインタを作らない。`unsafe` は `gemm`・`elementwise` が内側の関数を呼ぶ 2 か所のブロックだけ。

### 試験

| 試験 | 流れるビルド |
| ---- | ------------ |
| `cpu_ops_reference`: `gemm_simd_matches_scalar_by_t1`・`lane_order_differs_from_scalar`・`tail_order_changes_bits`・`gemm_simd_exact_on_small_integers_for_each_k` | すべて |
| `cpu_ops_reference`: `gemm_simd_differs_from_scalar_on_neon`・`gemm_simd_follows_lane_order_on_neon` | aarch64 で `neon` が有効なビルド |
| `cpu_ops_reference`: `gemm_simd_equals_scalar_without_lanes` | wasm32 の `simd128` も aarch64 の `neon` も無効なビルド |
| `cpu_ops_build_selection`: (e)〜(h) と陽性対照 6 本 | すべて（ソースの文字列を読む） |
| `cpu_ops_neon_unsafe_count`・`common_unsafe_count_samples` の `check_backend_covers_only_the_registered_module` | すべて |

### 確かめ方

```text
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection
cargo test -p fandhe-silicon-contract --test cpu_ops_neon_unsafe_count
cargo test -p fandhe-silicon-contract --test common_unsafe_count_samples
cargo clippy -p fandhe-silicon-contract --all-targets --no-default-features --features contract-core,cpu-isa,cpu-ops --target aarch64-unknown-linux-gnu -- -D warnings
```

観測した件数（Linux x86_64）: `cpu_ops_reference` 13 passed、`cpu_ops_build_selection` 13 passed、`cpu_ops_neon_unsafe_count` 1 passed、`common_unsafe_count_samples` 16 passed。aarch64 の型検査と lint は `--target aarch64-unknown-linux-gnu` の clippy までで、aarch64 上の実行は CI の `verify-macos` ジョブが流す。
