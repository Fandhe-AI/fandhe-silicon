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

## 今の値の補い（base 25d8dc2）

上の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 1/4 の確かめ方（2 本目の `9 passed`） | `9 passed; 0 failed` → 既定の組の `--doc` は 2 つの群で出る。1 つ目は `2 passed; 0 failed; 0 ignored`（`NonBlockingReadbackGate` と `CpuOpsGate (line 121)` が `ok`）、2 つ目は `9 passed; 0 failed; 0 ignored`（`compile fail ... ok` の群。`CpuOpsNonExhaustiveGate (line 131)` を含む）。1 本目（`contract-core` だけ）は `1 passed; 0 failed; 1 ignored`（`CpuOpsNonExhaustiveGate` が `ignored`）と `9 passed; 0 failed; 0 ignored`（`CpuOpsGate (line 121)` は `compile fail ... ok`）。記録の `9 passed` は、`compile fail ... ok` の群の件数と同じ値 | `cargo test -p fandhe-silicon-contract --doc`、`cargo test -p fandhe-silicon-contract --doc --no-default-features --features contract-core` |
| 1/4 の確かめ方（3 本目の `3 passed`）・完了の条件の確かめ（4/4）の (4) | `cpu_ops_gate_is_visible ... ok`（`3 passed; 0 failed`）→ 今も同じ | `cargo test -p fandhe-silicon-contract --all-features --test feature_gate_enabled` |
| 3/4 の記録時点の値 | 「コミット `bba66a5` の上」の `9 passed` → 3/4 の PR（#7920）の base は `24eaeca`（`80de3af` の親）で、値は同じ。`80de3af` の時点の `cpu_ops_public_surface.rs` の `#[test]` は 9 本、今も 9 本で `9 passed; 0 failed; 0 ignored` | `git log -1 --format=%P 80de3af`、`git show 80de3af:crates/contract/tests/cpu_ops_public_surface.rs \| grep -c '^#\[test\]'`、`cargo test -p fandhe-silicon-contract --test cpu_ops_public_surface` |
| 完了の条件の確かめ（4/4）の (5) | `crates/contract/Cargo.toml` は `80de3af` で +4 行 → `bba66a5..25d8dc2` でも `Cargo.toml` は +4 行。`cfg_gate_doctest.rs` は +17 行・-2 行（#7926） | `git diff --numstat bba66a5..HEAD -- crates/contract/src crates/contract/Cargo.toml` |
| 所在の `cfg_gate_doctest.rs` | 目印の型は `CpuOpsGate` → `CpuOpsNonExhaustiveGate` が加わった。`cfg_gate_doctest.rs` の `pub struct` は 11 件で、`CpuOpsGate` は 127 行目、`CpuOpsNonExhaustiveGate` は 142 行目 | `grep -n '^pub struct' crates/contract/src/cfg_gate_doctest.rs` |

今の値の補いとして、次の試験が加わっている（#7926）。

| 試験 | 所在 | 本数 |
| ---- | ---- | ---- |
| `CpuOpsNonExhaustiveGate` | `crates/contract/src/cfg_gate_doctest.rs`。`cpu-ops` が有効なときは `compile_fail,E0004`（外の crate からの `UnaryElementwiseOp` の網羅の `match` が `_` の腕なしで落ちる）、無効なときは `ignore`。上の表のとおり、有効な組では `compile fail ... ok`、`contract-core` だけの組では `ignored` | doctest 1 本 |
| `cpu_ops_feature_sets` | `crates/contract/tests/cpu_ops_feature_sets.rs`（`Cargo.toml` に `[[test]]` の項目は無く、feature の組に依らず流れる）。`real_manifest_contract_core_does_not_imply_cpu_ops`・`manifest_mutations_are_caught`・`real_script_has_the_contract_core_only_row`・`script_mutations_are_caught` | 4 本（`cargo test -p fandhe-silicon-contract --test cpu_ops_feature_sets` が `4 passed; 0 failed; 0 ignored`） |

`AddScalar` を 0.5 以外の定数で確かめる `elementwise_add_scalar_other_constants`（#7917）は `crates/contract/tests/cpu_ops_reference.rs` の 277 行目にあり、`cpu_ops_reference` は 8 本（`8 passed; 0 failed; 0 ignored`）。所在の頁は [cpu-ops-elementwise-check.md](cpu-ops-elementwise-check.md) と [cpu-ops.md](cpu-ops.md)。

## 今の値の補い（base 0f8b01c）

記録時点の表と既存の節は書き換えない。以下は base `0f8b01c` で観測した今の値（実行環境は GPU なしの Linux x86_64。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（4/4）の (5)（`:144`） | `bba66a5..25d8dc2` の `Cargo.toml` は +4 行 → `bba66a5..0f8b01c` で +8 行。`cfg_gate_doctest.rs` は +17 行・-2 行、`cpu_ops.rs` は +7 行・-1 行 | `git diff --numstat bba66a5..0f8b01c -- crates/contract/src crates/contract/Cargo.toml` |

## 今の値の補い（base ed82c18）

記録時点の表と既存の節は書き換えない。以下は base `ed82c18` で観測した今の値（実行環境は GPU なしの Linux x86_64）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（4/4）の (3)（`:110`） | `cfg: sites=5 label=match` → `cfg: sites=6 label=match` | `scripts/wasm-build-check.sh --cfg-only` |
| 「今の値の補い（base 25d8dc2）」の `cpu_ops_reference` の本数（`:154`） | 8 本 → Linux x86_64 で 13 本。aarch64 で `neon` が有効なビルドは 14 本（[cpu-ops](cpu-ops.md) の「完了の条件の確かめ（12/12）」の (1)・(2)） | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference` |
