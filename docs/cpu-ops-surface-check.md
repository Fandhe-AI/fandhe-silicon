# `cpu_ops` の公開の型と feature の有無の確かめ（TASK-2457）

## feature `cpu-ops` を外した構成のコンパイル失敗の試験（1/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2457。親 Issue は #1074、この節（1/4）は #6962。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 目印の型 | `CpuOpsGate`（`crates/contract/src/cfg_gate_doctest.rs`。先例は TASK-233 のポインタ） |
| fence の切り替え | `cpu-ops` 無効で `compile_fail`、有効で通常の doctest |
| snippet | `use fandhe_silicon_contract::cpu_ops;` と `fn probe(_: &dyn cpu_ops::CpuOps) {}` の 2 行 |
| 有効側の試験 | `cpu_ops_gate_is_visible`（`crates/contract/tests/feature_gate_enabled.rs`） |
| `cpu_ops` の所在 | `docs/cpu-ops.md`（TASK-1351 のポインタ） |

### 流れる構成

- `compile_fail` で流れるのは `cpu-ops` を含まない組（`scripts/build-profiles.sh` の `NATIVE_SETS` のうち `cpu-ops` を含まない組。手元では `--no-default-features --features contract-core`）
- 通常の doctest として通るのは既定の組、`cpu-ops` を含む組、`--all-features`（`make verify`）

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --doc --no-default-features --features contract-core
cargo test -p fandhe-silicon-contract --doc
cargo test -p fandhe-silicon-contract --all-features --test feature_gate_enabled
```

記録時点の値（コミット `dc8b673` の上）の観測は次のとおり。

- 1 本目は `CpuOpsGate ... compile fail ... ok`
- 2 本目は `CpuOpsGate ... ok`（`9 passed; 0 failed`）
- 3 本目は `cpu_ops_gate_is_visible ... ok`（`3 passed; 0 failed`）

## `contract-core` だけの組（2/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2457。親 Issue は #1074、この節（2/4）は #6963。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 足した組 | `scripts/build-profiles.sh` の `NATIVE_SETS` の `"contract-core"` の行 |
| 流れる段 | `cargo build`・`cargo clippy --all-targets -D warnings`・`cargo test`（`--no-default-features --features contract-core`） |
| 流れる場所 | `make build-profiles`（CI の `verify`・`verify-macos` ジョブ） |

### 振る舞い

この組の `cargo test` の doctest で、`CpuOpsGate` が `compile fail ... ok` になる。

### 確かめ方

```sh
cargo build -p fandhe-silicon-contract --no-default-features --features contract-core
cargo clippy -p fandhe-silicon-contract --all-targets --no-default-features --features contract-core -- -D warnings
cargo test -p fandhe-silicon-contract --no-default-features --features contract-core
grep -n '"contract-core"' scripts/build-profiles.sh
```

## 公開の型と trait の形の試験（3/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2457。親 Issue は #1074、この節（3/4）は #6964。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験ファイル | `crates/contract/tests/cpu_ops_public_surface.rs`（外の crate の統合テスト） |
| `[[test]]` | `crates/contract/Cargo.toml` の `cpu_ops_public_surface`（`required-features = ["cpu-ops"]`） |
| 使う公開のパス | `fandhe_silicon_contract::cpu_ops::{CpuOps, DefaultCpuOps, ReferenceCpuOps, UnaryElementwiseOp}` と `fandhe_silicon_core::error::ContractError` |
| `cpu_ops` の所在 | `docs/cpu-ops.md`（TASK-1351 のポインタ） |

### 試験

| 試験 | 確かめること | 入力 |
| ---- | ------------ | ---- |
| `trait_signatures_are_implementable` | `CpuOps` の 6 メソッドの署名で、試験の中の型に実装できる | 本体が `Ok(())` の unit struct を `&dyn CpuOps` にして呼ぶ |
| `trait_has_six_methods` | `src/cpu_ops.rs` の `pub trait CpuOps {` から桁 0 の `}` までの `fn` が 6 つで、名前の集合が一致する | 実物のソース |
| `method_count_check_fails_when_a_method_is_added` | 既定の本体を持つメソッドを足すと 7 件になる | 実物の開きの直後に 1 件足した入力 |
| `method_count_check_fails_when_a_method_is_removed` | メソッドを 1 つ消すと 5 件になる | 実物の `topk_partial_simd` の行を外した入力 |
| `method_count_check_fails_when_open_line_is_missing` | 開きの行が無い入力は `Err` | 開きの行を別の字句に変えた入力・空の入力 |
| `implementors_are_object_safe` | 3 つの実装が `&dyn CpuOps` に入り、小さな入力で `Ok`。`DefaultCpuOps`・`ReferenceCpuOps` は積 `6.0` と `[0.0, 2.0]` | 1×1×1 の `gemm_scalar`、長さ 2 の `elementwise_scalar` |
| `product_and_reference_types_are_in_cpu_ops` | 2 つの型がそのパスで名指せて、unit struct として作れ、`Debug`・`Clone`・`Copy` を満たす | 値そのもの |
| `reference_sentinel` | `ReferenceCpuOps::SENTINEL` が `(u32::MAX, f32::NEG_INFINITY)`（スコアはビットで比べる） | 定数 |
| `elementwise_op_values` | `Relu`・`AddScalar(f32)` を外の crate で作って `match` で分けられ（`_` の腕あり）、`PartialEq`・`Copy` を満たす | `Relu`、`AddScalar(1.5)` |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --test cpu_ops_public_surface
cargo clippy -p fandhe-silicon-contract --tests -- -D warnings
cargo check -p fandhe-silicon-contract --tests --target wasm32-unknown-unknown
```

wasm32 の型検査は `make wasm-verify` の contract の試験の段で流れる。

記録時点の値（コミット `bba66a5` の上）の観測は `9 passed; 0 failed`。

## 完了の条件の確かめ（4/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2457。親 Issue は #1074、この節（4/4）は #6965。

実行環境は GPU なしの Linux。確かめた base のコミットは `961ff6f`。コード・試験・スクリプトは変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | `cpu-ops` を有効にした組で `cpu_ops` の 4 つの名前が外の crate から使え、`CpuOps` の 6 メソッドの署名と数が固定されている | `cargo test -p fandhe-silicon-contract --test cpu_ops_public_surface`、`grep -n -B1 'pub mod cpu_ops' crates/contract/src/lib.rs` | `9 passed; 0 failed`（`trait_signatures_are_implementable`・`trait_has_six_methods`・`method_count_check_fails_when_a_method_is_added`・`method_count_check_fails_when_a_method_is_removed`・`method_count_check_fails_when_open_line_is_missing`・`implementors_are_object_safe`・`product_and_reference_types_are_in_cpu_ops`・`reference_sentinel`・`elementwise_op_values` がすべて `ok`）。grep は `44-#[cfg(feature = "cpu-ops")]` と `45:pub mod cpu_ops;` |
| (2) | wasm32 の型検査でも同じ試験が通る | `cargo check -p fandhe-silicon-contract --test cpu_ops_public_surface --locked --target wasm32-unknown-unknown`、`make wasm-verify` | 終了コード 0、終了コード 0 |
| (3) | 版の選択がビルド時の `cfg` だけで決まる（既存の検査） | `cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection`、`scripts/wasm-build-check.sh --cfg-only`、1/4〜3/4 のコミットで `scripts/wasm-build-check.sh` と `crates/contract/tests/cpu_ops_build_selection.rs` を対象にした `git show --stat` | `7 passed; 0 failed`。`cfg: sites=5 label=match`・`record: commit=961ff6f mode=cfg-only cfg_sites=5 not_run=0`・`ok: all rows match`、終了コード 0。`git show --stat` の出力は空 |
| (4) | `cpu-ops` を外した構成で `cpu_ops` を参照するとコンパイルに失敗し、有効にした構成で通る | `cargo test -p fandhe-silicon-contract --doc --no-default-features --features contract-core`、`cargo test -p fandhe-silicon-contract --doc`、`cargo test -p fandhe-silicon-contract --all-features --test feature_gate_enabled`、`make build-profiles` のログの `CpuOpsGate` の行を数える | 前者は `CpuOpsGate (line 121) - compile fail ... ok`、既定の組は `CpuOpsGate (line 121) ... ok`、`cpu_ops_gate_is_visible ... ok`。`make build-profiles` は終了コード 0 で、`compile fail ... ok` の行が 15、`compile fail` を含まない `... ok` の行が 6 |
| (5) | 依存 0・unsafe 0 | 1/4〜3/4 のコミットで `Cargo.lock`・`deps-approved.toml`・`crates/contract/Cargo.toml`・`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops`・`crates/contract/src/lib.rs`・`crates/wasm-abi` を対象にした `git show --stat`、`grep -c unsafe crates/contract/tests/cpu_ops_public_surface.rs crates/contract/src/cfg_gate_doctest.rs`、`grep -n 'deny(unsafe_code)' crates/contract/src/lib.rs` | `80de3af` の `crates/contract/Cargo.toml` の `4 insertions(+)`（`[[test]]` の 1 項目）のみで、ほかの対象の差分は 0。`unsafe` は 2 ファイルとも 0 件。`11:#![deny(unsafe_code)]` |
| (6) | CI で流れる場所のコマンド | `make verify`、`make build-profiles`、`make wasm-verify` | 終了コードはそれぞれ 0、0、0。`make verify` の `test result: ok` の passed の合計は 2201 |

### 1/4〜3/4 の対応

| 段 | PR | コミット |
| -- | -- | -------- |
| 1/4 | #7914 | `795341f` |
| 2/4 | #7919 | `bba66a5` |
| 3/4 | #7920 | `80de3af` |

### 追加で流したコマンド

```text
cargo check -p fandhe-silicon-contract --test cpu_ops_public_surface --locked --target wasm32-unknown-unknown   # 終了コード 0
scripts/wasm-build-check.sh --cfg-only                                                                         # ok: all rows match
make build-profiles                                                                                            # 終了コード 0
```

### 再実行の契機

`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/src/cfg_gate_doctest.rs`・`crates/contract/tests/cpu_ops_public_surface.rs`・`crates/contract/tests/feature_gate_enabled.rs`・`scripts/build-profiles.sh` の `NATIVE_SETS` を変えたときは、表を取り直す。
