# CUDA の背後実装の device の口の到達（TASK-166）の所在と本リポの現状

TASK-166 の作業のうち、CUDA の背後実装の device から compute の口へ届く所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-166。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `impl Device for CudaDevice` の `compute()` | `grep -c 'fn compute(' crates/contract/src/backend_cuda.rs` |
| 試験 | `crates/contract/tests/backend_cuda_reach.rs`（どの試験も device に触る） | 下の「確かめ方」 |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")`（`crates/contract/src/lib.rs`） | 同上 |

## compute の口（1/3）

- `CudaDevice` の `Device::compute()` は `Some(self)` を返す
- `create_pipeline` の実装の所在は [cuda-create-pipeline.md](./cuda-create-pipeline.md)（TASK-803）
- `Device::compute()` の既定の本体の所在は [device-compute.md](./device-compute.md)（TASK-2956）

| 試験 | 確かめること |
| ---- | ------------ |
| `compute_only_device_has_compute_and_no_graphics` | `ComputeOnly`・`ProductIndependent` の device で `backend()` が `Cuda`、`compute()` が `Some`、`graphics()` が `None`（`graphics()` の行は feature `graphics` のビルドだけ） |
| `create_pipeline_through_compute_returns_a_handle` | `compute()` の `Some` から `create_pipeline` が `Ok(PipelineHandle)` を返す |

どちらも `#[ignore = "requires a CUDA device"]` を付ける。

確かめ方:

```bash
grep -c 'fn compute(' crates/contract/src/backend_cuda.rs
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_reach
cargo test -p fandhe-silicon-contract --features backend-cuda,graphics --test backend_cuda_reach
```

1 行目は `1`、2・3 行目は `0 passed; 0 failed; 4 ignored`（2/3 の試験を足した後の数。1/3 の時点では 2）。

## 確保と記録先の作成の device の要る試験（2/3）

| 試験 | 確かめること |
| ---- | ------------ |
| `allocate_through_memory_returns_a_buffer_handle` | `ComputeOnly`・`ProductIndependent` の device の `memory().allocate(16, STORAGE \| UPLOAD \| DOWNLOAD)` が `Ok(BufferHandle)` を返し、`size()` が 16、`usage()` が渡した値 |
| `create_encoder_through_queue_returns_an_encoder` | `queue().create_encoder()` が `CommandEncoder` を返す |

どちらも `#[ignore = "requires a CUDA device"]` を付ける。

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_reach
cargo test -p fandhe-silicon-contract --features backend-cuda,graphics --test backend_cuda_reach
grep -c '^#\[ignore' crates/contract/tests/backend_cuda_reach.rs
```

cargo の 2 行は `0 passed; 0 failed; 4 ignored`、grep は `4`。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-166。ポインタのみ）。

時点のコミット: `0eb5ad4`（`docs/spec` の gitlink は `050073a33856cff4559718a8e760289ec3d5a65e`）

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 上書きの所在 | `grep -n 'fn compute(' crates/contract/src/backend_cuda.rs` | 1 行（`1661:`） |
| 2 | 試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_reach` | `running 4 tests`、`0 passed; 0 failed; 4 ignored` |
| 3 | `graphics` を付けたビルド | `cargo test -p fandhe-silicon-contract --features backend-cuda,graphics --test backend_cuda_reach` | (2) と同じ |
| 4 | device の要る試験の宣言 | `grep -c '^#\[ignore' crates/contract/tests/backend_cuda_reach.rs`、(2) の `ignored` の件数 | どちらも 4 |
| 5 | 既存の lib の試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` | `57 passed; 0 failed; 15 ignored` |
| 6 | 組のビルド | `make build-profiles` | 終了コード 0 |
| 7 | 公開 API | `make public-api-sdk-check`、`make public-api-check` | `sdk_type_refs_total=0` を含む `record:` の行と `ok:` の行が出る。`make public-api-check` は終了コード 0 |
| 8 | unsafe・依存 | `cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count`、`make deps-check`、`git diff --stat 496e7d5 HEAD -- Cargo.lock deps-approved.toml crates/contract/tests/backend_cuda_unsafe_count.rs crates/core` | `1 passed; 0 failed`。`[deps-check.sh] ok: ...` の行が出て終了コード 0。diff は空 |
| 9 | CI | `gh run view 42541441831 --json status,conclusion,headSha,jobs`（`0eb5ad4` の main の push の run） | conclusion は `success`。headSha は `0eb5ad4b1a67e2056c344700668351133644e714`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` の conclusion は `success` |

(1) の出力。

```text
1661:    fn compute(&self) -> Option<&dyn ComputeOps> {
```

(2) の出力（該当の行）。

```text
running 4 tests
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(3) の出力（該当の行）。

```text
running 4 tests
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(4) の出力。

```text
4
```

(5) の出力（該当の行）。

```text
running 72 tests
test result: ok. 57 passed; 0 failed; 15 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

(7) の出力（該当の行）。

```text
record: commit=0eb5ad4b1a67e2056c344700668351133644e714 documents=10 public_items_total=13844 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
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
| 1/3 | device から compute の口を返す | #3738 |
| 2/3 | 確保と記録先の作成へ届く device の要る試験を ignore 付きで足す | #3739 |
| 3/3 | 完了の条件の確かめ | #3740 |

## 今の値の補い（base 70ab958）

記録時点の表と既存の節は書き換えない。以下は base `70ab958` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「完了の条件の確かめ」の (5) の行と出力（`backend-cuda --lib backend_cuda`） | `running 72 tests`・`57 passed; 0 failed; 15 ignored` → `running 76 tests`・`57 passed; 0 failed; 19 ignored; 0 measured; 13 filtered out` | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |
| (1) の行の `fn compute(` の所在 | `1661:` → 同じ（`1661:`）。`backend_cuda_reach` の宣言（`running 4 tests`・`0 passed; 0 failed; 4 ignored`）も同じ | `grep -n 'fn compute(' crates/contract/src/backend_cuda.rs`、`cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_reach` |
