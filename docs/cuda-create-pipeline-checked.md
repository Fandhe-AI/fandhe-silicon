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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | ComputeOps に宣言を照らしてから作る口を足す | #4851 |
| 2/4 | 次元数の照らしと作成の失敗の受け渡しの試験を足す | #4852 |
| 3/4 | device の要る宣言の照らしの試験を ignore 付きで足す | #4853 |
