# CUDA の背後実装の `CpuOnly`・`Graphics` の要求と鍵の一覧の外の機能 ID の返し（TASK-169）の所在と本リポの現状

TASK-169 の作業のうち、CUDA の背後実装の `CpuOnly`・`Graphics` の要求の拒否と、鍵の一覧の外の機能 ID の返しの試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-169。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `check_request` と `impl Capabilities for CudaDevice`（`cuda_capability_of`） | `crates/contract/src/backend_cuda.rs` |
| 試験 | `crates/contract/tests/backend_cuda_profiles.rs`（device に触らない）と、`backend_cuda.rs` の `mod tests` の `ids_outside_the_key_list_are_unlisted` | 下の「確かめ方」 |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 関連 | device の取得は [cuda-device-request.md](./cuda-device-request.md)（TASK-160）、能力の問合せは [cuda-capability-query.md](./cuda-capability-query.md)（TASK-175） | 各文書 |

## プロファイルと所有形態の組の拒否と一覧の外の機能 ID（1/3）

`request` は、`CpuOnly`・`Graphics` を所有形態によらず `InvalidCombination` で返す。

| プロファイル | 所有形態 | 返す値 | 試験 |
| ------------ | -------- | ------ | ---- |
| `CpuOnly` | `ProductIndependent`・`Shared`・`Imported` | `InvalidCombination` | `cpu_only_is_an_invalid_combination_for_every_ownership` |
| `Graphics` | `ProductIndependent`・`Shared`・`Imported` | `InvalidCombination` | `graphics_is_an_invalid_combination_for_every_ownership` |

- `--features backend-cuda` と `--features backend-cuda,graphics` のどちらのビルドでも同じ値

鍵の一覧（`FEATURE_QUERY_KEYS`）に無い 5 つの ID（`feature:shader_f16`・`feature:SHADER_F16 `〔末尾に空白〕・`feature:SUBGRUOP`・`feature:not-a-feature`・空の文字列）を `cuda_capability_of` に当てると、compute capability `(5,3)`・`(7,4)`・`(7,5)`・`(9,0)` と属性 `None`・`Some(false)`・`Some(true)` のどの組でも core の `UNLISTED_FEATURE_QUERY`（`Unsupported`・`Some(UnknownFeature)`・`None`）と等しい値を返す。

### 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_profiles
cargo test -p fandhe-silicon-contract --features backend-cuda,graphics --test backend_cuda_profiles
cargo test -p fandhe-silicon-contract --features backend-cuda --lib ids_outside_the_key_list_are_unlisted
```

## device の要る試験（2/3）

試験は `crates/contract/tests/backend_cuda_profiles.rs` の末尾にあり、どちらも `#[ignore = "requires a CUDA device"]` 付き。このファイルは device に触らない 2 本に加えて、この 2 本を持つ。

| 試験 | 確かめること |
| ---- | ------------ |
| `rejected_profiles_are_invalid_combinations_where_compute_only_opens_a_device` | `ComputeOnly`・`ProductIndependent` の `request` が device を返して `backend()` が `Cuda`、同じ環境で `CpuOnly`・`Graphics` と 3 つの所有形態の 6 組がどれも `InvalidCombination` |
| `unlisted_ids_are_unknown_features_on_a_cuda_device` | 上の 5 つの ID が `FEATURE_QUERY_KEYS` に無く、`query_ex` が `UNLISTED_FEATURE_QUERY` と等しく、`query` が `Unsupported`。1 つの ID につき 1 行 `query: feature=… level=… reason=… evidence=…` を標準出力に出す |

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_profiles` が `2 passed; 0 failed; 2 ignored`（`--features backend-cuda,graphics` も同じ）。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-169。ポインタのみ）。

時点のコミット: `c36eecd`（`docs/spec` の gitlink は `f50c86bf8919b4ee68e62948491886fb25c9ae12`）

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 組の拒否 | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_profiles` | `running 4 tests`、`2 passed; 0 failed; 2 ignored` |
| 2 | `graphics` を付けたビルド | `cargo test -p fandhe-silicon-contract --features backend-cuda,graphics --test backend_cuda_profiles` | (1) と同じ |
| 3 | 一覧の外の ID | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib ids_outside_the_key_list_are_unlisted` | `1 passed; 0 failed; 0 ignored` |
| 4 | 既存の検査の順の試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_request`、`cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_fault` | `4 passed; 0 failed; 0 ignored`、`3 passed; 0 failed; 1 ignored` |
| 5 | device の要る試験の宣言 | (1) の `ignored` の件数、`grep -c '^#\[ignore' crates/contract/tests/backend_cuda_profiles.rs` | どちらも 2 |
| 6 | 組のビルド | `make build-profiles` | 終了コード 0 |
| 7 | 公開 API | `make public-api-sdk-check`、`make public-api-check` | `sdk_type_refs_total=0` を含む `record:` の行と `ok:` の行が出る。`make public-api-check` は終了コード 0 |
| 8 | unsafe・依存 | `cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count`、`make deps-check`、`git diff --stat 3b80ea1 HEAD -- Cargo.lock deps-approved.toml crates/contract/tests/backend_cuda_unsafe_count.rs` | `1 passed; 0 failed`。`[deps-check.sh] ok: ...` の行が出る。diff は空 |
| 9 | CI | `gh run view 42520721682 --json status,conclusion,headSha,jobs`（`c36eecd` の main の push の run） | conclusion は `success`。headSha は `c36eecd5e98e519d9cf5a5924344c02f26f5eb69`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` の conclusion は `success` |

(1) の出力（該当の行）。

```text
running 4 tests
test result: ok. 2 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力（該当の行）。

```text
running 4 tests
test result: ok. 2 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(3) の出力（該当の行）。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 73 filtered out; finished in 0.00s
```

(4) の出力（該当の行）。

```text
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 4 tests
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(7) の出力（該当の行）。

```text
record: commit=c36eecd5e98e519d9cf5a5924344c02f26f5eb69 documents=10 public_items_total=13838 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
ok: 10 public API document(s) checked, common crate dependencies checked, no chip SDK references
```

(8) の出力（該当の行）。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | プロファイルと所有形態の組の拒否と一覧の外の機能 ID の試験を足す | #3742 |
| 2/3 | device の要る試験を ignore 付きで足す | #3743 |
| 3/3 | 完了の条件の確かめ | #3744 |
