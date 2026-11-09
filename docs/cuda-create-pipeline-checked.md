# CUDA の背後実装の宣言を照らしてからのパイプラインの生成（TASK-457）の所在と本リポの現状

TASK-457 の作業のうち、`ComputeOps::create_pipeline_checked` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-457。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `ComputeOps::create_pipeline_checked`（既定の本体。`CudaDevice` は上書きしない） | `grep -n 'create_pipeline_checked' crates/core/src/execution/compute.rs` |
| 実装のファイル | `crates/core/src/execution/compute.rs` | 同上 |
| cfg | feature なし | `crates/core/src/execution/compute.rs` |
| 試験 | `crates/core/tests/compute_create_pipeline_checked.rs`（device に触らない） | `cargo test -p fandhe-silicon-core --test compute_create_pipeline_checked` |

## 宣言を照らしてから作る口（1/4）

引数は `input`・`layout`・`declared`。既定の本体の順は `validate_declared_operands(declared, layout)` → `create_pipeline(input, layout)`。

| 入力 | 返す値 |
| ---- | ------ |
| 宣言の件数が `layout.slots` と違う | 先頭が `C1` の `InvalidArgument`。`create_pipeline` を呼ばない |
| 照らしを通る | `create_pipeline` を 1 回呼び、結果を値のまま返す |

試験:

- `declared_count_mismatch_does_not_call_create_pipeline`
- `matching_declaration_calls_create_pipeline_once_and_returns_as_is`
- `reachable_through_dyn_compute_ops`
- `create_pipeline_checked_signature_is_pinned`

確かめ方: `cargo test -p fandhe-silicon-core --test compute_create_pipeline_checked`

## 次元数の照らしと作成の失敗の受け渡し（2/4）

| 試験 | 入力 | 返す値 |
| ---- | ---- | ------ |
| `dimension_mismatch_does_not_call_create_pipeline` | `shape` 2 次元・`stride` 1 次元の宣言 1 件と `slots` 1 件 | 先頭が `C3` の `InvalidArgument`。`create_pipeline` を呼ばない。`stride` を 2 次元にすると `Ok`、`create_pipeline` を 1 回 |
| `create_pipeline_error_is_returned_as_is` | 失敗する `create_pipeline`・照らしを通る宣言 | `create_pipeline` の `InvalidArgument("inner")` をそのまま返す。呼び出しは 1 回 |
| `rejection_comes_before_a_failing_create_pipeline` | 失敗する `create_pipeline`・件数が違う宣言／次元数が違う宣言 | 先頭が `C1`／`C3`（`inner` でない）。`create_pipeline` を呼ばない |

確かめ方: `cargo test -p fandhe-silicon-core --test compute_create_pipeline_checked`

## device の要る試験（3/4）

試験は `crates/contract/src/backend_cuda.rs` の `checked_pipeline_device_tests` module にある。どれも `#[ignore = "requires a CUDA device"]` で既定では流れない。`CudaDevice` で `create_pipeline_checked` を呼ぶ。

| 試験 | 入力 | 確かめること |
| ---- | ---- | ------------ |
| `declared_count_mismatch_is_rejected_and_adds_no_pipeline` | `slots` 3 件・宣言 1 件 | 先頭が `C1` の `InvalidArgument`。表の件数が増えない |
| `dimension_mismatch_is_rejected_and_adds_no_pipeline` | `slots` 3 件・宣言 3 件で 1 件目の `shape` が 2 次元・`stride` が 1 次元 | 先頭が `C3` の `InvalidArgument`。表の件数が増えない |
| `rejection_comes_before_the_input_is_classified` | `Ptx` の入力・`slots` 2 件・宣言 1 件 | 先頭が `C1` の `InvalidArgument`（`Unsupported` でない）。表の件数が増えない |
| `matching_declaration_creates_a_pipeline_of_this_device` | CUDA C の入力・`slots` 3 件・宣言 3 件 | `Ok`。handle の device の識別子がこの device と同じ。表の件数が 1 増える |

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::checked_pipeline_device_tests`（通常の実行では `0 passed; 0 failed; 4 ignored`）

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-457。ポインタのみ）。

時点のコミット: `f99d6d8`（`docs/spec` の gitlink は `050073a33856cff4559718a8e760289ec3d5a65e`）

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 共通の crate の口 | `cargo test -p fandhe-silicon-core --test compute_create_pipeline_checked`・`cargo test -p fandhe-silicon-core --doc execution::compute` | 前者は 7 passed。後者は `test result:` の行が 2 つで、8 passed（67 filtered out）と 6 passed（61 filtered out）。終了コード 0 |
| 2 | 公開の名前と型の検査 | `make public-api-webgpu-check`・`make public-name-check`・`make public-api-sdk-check` | `pipeline_create_functions_total=2`・`pipeline_input_violations_total=0`・`name_hits_total=0`・`sdk_type_refs_total=0`。いずれも終了コード 0 |
| 3 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::checked_pipeline_device_tests` | 0 passed; 0 failed; 4 ignored、終了コード 0 |
| 4 | 依存・unsafe | `git diff --stat ed0c415 HEAD -- Cargo.lock deps-approved.toml crates/core/Cargo.toml crates/contract/Cargo.toml crates/contract/src/lib.rs crates/contract/tests/backend_cuda_unsafe_count.rs`・`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count`・`make deps-check` | diff は空。個数テストは 1 passed。`make deps-check` は終了コード 0 |
| 5 | wasm32 の型検査 | `cargo check -p fandhe-silicon-core --tests --target wasm32-unknown-unknown` | 終了コード 0 |
| 6 | CI | `gh run view 42553299287 --json conclusion,headSha,jobs`（3/4 のマージの main の push の run） | conclusion `success`、headSha `f99d6d81315b05307fafa779229861e5335a2b5d`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success` |

(1) の出力。

```text
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 67 filtered out; finished in 0.00s
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 61 filtered out; finished in 0.02s
```

(2) の出力。

```text
record: commit=f99d6d81315b05307fafa779229861e5335a2b5d documents=2 public_items_total=3907 public_functions_total=537 pipeline_create_functions_total=2 pipeline_input_checked=1 pipeline_input_violations_total=0
record: commit=f99d6d81315b05307fafa779229861e5335a2b5d documents=10 public_items_total=13920 name_hits_total=0 sources=1 source_hits_total=0 field_specs=1 field_hits_total=0
record: commit=f99d6d81315b05307fafa779229861e5335a2b5d documents=10 public_items_total=13920 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
```

(3) の出力。

```text
running 4 tests
test result: ok. 0 passed; 0 failed; 4 ignored; 0 measured; 85 filtered out; finished in 0.00s
```

(4) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | ComputeOps に宣言を照らしてから作る口を足す | #4851 |
| 2/4 | 次元数の照らしと作成の失敗の受け渡しの試験を足す | #4852 |
| 3/4 | device の要る宣言の照らしの試験を ignore 付きで足す | #4853 |
| 4/4 | 完了の条件の確かめ | #4854 |
