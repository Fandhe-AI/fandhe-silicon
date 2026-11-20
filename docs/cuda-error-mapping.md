# CUDA の背後実装のエラーの写し方（TASK-291）の所在と本リポの現状

TASK-291 の作業のうち、CUDA の背後実装の `ComputeOps::create_pipeline` が受け付けない入力に返す値と、ドライバ・NVRTC の呼び出しの失敗を写す関数の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-291。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入力の分類 | `crates/contract/src/backend_cuda.rs` の `classify_pipeline_input` | `git grep -n "fn classify_pipeline_input" -- crates/contract/src/backend_cuda.rs` |
| 失敗の写し | 同じファイルの `backend_failure`・`allocation_failure`・`artifact_failure`・`nvrtc_failure` | `git grep -n "fn backend_failure\|fn allocation_failure\|fn artifact_failure\|fn nvrtc_failure" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `pipeline_error_tests` module。device に触らない | 下の「確かめ方」 |

## 受け付けない入力の理由と失敗の写し（1/3）

`classify_pipeline_input` が受け付けない 6 つの入力に返す値は `ArtifactRejected { kind: "unsupported-input", reason }` で、`reason` は次のとおり。試験は `unsupported_inputs_name_the_variant_and_the_accepted_inputs` で、入力のソースの字句が `reason` に載らないことも照らす。

| 入力 | `reason` |
| ---- | -------- |
| `Msl` | `pipeline input Msl is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `MslSpecialized` | `pipeline input MslSpecialized is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `MslWithOptions` | `pipeline input MslWithOptions is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `MslSpecializedWithOptions` | `pipeline input MslSpecializedWithOptions is Metal-only and is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `SpirV` | `pipeline input SpirV is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |
| `Dxil` | `pipeline input Dxil is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx` |

失敗を写す関数が返す値と試験:

| 関数と入力 | 返す値 | 試験 |
| ---------- | ------ | ---- |
| `backend_failure("memcpy_htod", "detail")` | `BackendFailure { op: "memcpy_htod", backend: Cuda, detail: "\"detail\"" }` | `driver_failures_carry_the_call_name_and_the_debug_text` |
| `allocation_failure("alloc_zeros", 64, …)` の `CUDA_ERROR_OUT_OF_MEMORY` | `AllocationFailed { requested_bytes: 64 }` | 同上 |
| `artifact_failure(kind, call, true, "detail")`（`module` と `cuModuleLoadData`、`function` と `cuModuleGetFunction`） | `ArtifactRejected { kind, reason: "<call>: \"detail\"" }` | `artifact_failures_carry_the_call_name_and_the_debug_text` |
| `artifact_failure(kind, call, false, "detail")`（同じ 2 組） | `BackendFailure { op: <call>, backend: Cuda, detail: "\"detail\"" }` | 同上 |
| `nvrtc_failure` の `nvrtcCompileProgram` の `NVRTC_ERROR_COMPILATION` | `ArtifactRejected { kind: "cuda-c", reason: "nvrtcCompileProgram: <失敗の Debug の文字列>" }` | 同上 |
| `nvrtc_failure` の `nvrtcCreateProgram` の `NVRTC_ERROR_OUT_OF_MEMORY` | `BackendFailure { op: "nvrtcCreateProgram", backend: Cuda, detail: <失敗の Debug の文字列> }` | 同上 |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::pipeline_error_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::tests::cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified
```

1 つ目は 3 件、2 つ目は 1 件が通る。

## device の要る試験（2/3）

device に触る試験は `crates/contract/src/backend_cuda.rs` の `pipeline_error_device_tests` module にあり、どれも `#[ignore = "requires a CUDA device"]` を付けている。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。試験のビルドの device は、`compile_with_nvrtc` に入った回数と、`create_pipeline` が `bind_to_thread`・モジュールの読み込み・関数の取り出しを呼ぶ前に 1 ずつ足す回数（欄 `pipeline_driver_calls`）を持ち、試験はその 2 つの回数を読む。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `unsupported_inputs_are_rejected_without_entering_nvrtc_or_the_driver` | `Msl`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions`・`SpirV`・`Dxil` の入力で `create_pipeline` を 1 回ずつ、続けて `CudaC` の入力で 1 回 | 6 つとも `ArtifactRejected { kind: "unsupported-input" }` で、理由は受けた variant の名前と `CudaC`・`Ptx` を含み、`Metal` の語は `MslSpecializedWithOptions` の理由だけが含む。6 つの後も表の件数は増えず、2 つの回数は 0。`CudaC` の 1 回の後は `compile_with_nvrtc` に入った回数が 1、もう 1 つの回数が 3 で、表の件数が 1 増える |
| `artifact_failures_name_the_failed_call_and_add_no_pipeline` | コンパイルできない `CudaC`・無い入口名の `CudaC`・読めないテキストの `Ptx` の入力で `create_pipeline` を 1 回ずつ | 順に `ArtifactRejected` の `cuda-c`・`function`・`module` で、理由は `nvrtcCompileProgram: `・`cuModuleGetFunction: `・`cuModuleLoadData: ` で始まる。表の件数は増えない。`compile_with_nvrtc` に入った回数は順に 1・2・2、もう 1 つの回数は順に 0・3・5 |

`Msl` 系の入力のソースは Metal の背後実装の試験の `KERNEL`・`SPECIALIZED_KERNEL` と同じ文字列で、`SpirV` の入力は何もしない compute の入口 `k` の語の列、`Dxil` の入力は DXBC のコンテナの見出しだけの 32 バイト。

| 項目 | 値 |
| ---- | -- |
| module | `crates/contract/src/backend_cuda.rs` の `pipeline_error_device_tests` |
| feature | `backend-cuda` |
| `#[ignore]` | `requires a CUDA device` |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::pipeline_error_device_tests
```

2 件が `ignored` と数えられる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-291。ポインタのみ）。

時点のコミット: `298023ca`。`docs/spec` の gitlink は `96476a8254c0c1fba240acf062644b4433123e99`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `298023ca` の `crates/contract/src/backend_cuda.rs` の 2 つの module に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n` で 5 本の名前を 1 つずつ引く | 出力 (2) |
| (3) | 口の所在 | `git grep -n` で `classify_pipeline_input`・`backend_failure`・`allocation_failure`・`artifact_failure`・`nvrtc_failure`・`pipeline_driver_calls` を 1 つずつ引く | 出力 (3) |
| (4) | 理由の文 | `grep -cF` と、1/3・2/3 の squash の `git diff --numstat` | 出力 (4) |
| (5) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 2 つの実行 | 出力 (5) |
| (6) | 依存の差 | `3dd568de^ 298023ca` の差と `make deps-check` | 出力 (6) |
| (7) | unsafe の数と差 | `3dd568de^ 298023ca` の差と `unsafe` の行の数 | 出力 (7) |
| (8) | CI | 2/3 のマージの後の main の push の run | 出力 (8) |

`3dd568de^` は 1/3 の squash `3dd568de` の親。(5) の `cargo test` と (6) の `make deps-check` は作業木で流した。

出力 (1):

```text
$ git show 298023ca:crates/contract/src/backend_cuda.rs | awk '/^mod pipeline_error_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
3
$ git show 298023ca:crates/contract/src/backend_cuda.rs | awk '/^mod pipeline_error_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
2
$ git show 298023ca:crates/contract/src/backend_cuda.rs | awk '/^mod pipeline_error_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
2
```

出力 (2):

```text
$ git grep -n "fn unsupported_inputs_name_the_variant_and_the_accepted_inputs(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:8604:    fn unsupported_inputs_name_the_variant_and_the_accepted_inputs() {
$ git grep -n "fn driver_failures_carry_the_call_name_and_the_debug_text(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:8676:    fn driver_failures_carry_the_call_name_and_the_debug_text() {
$ git grep -n "fn artifact_failures_carry_the_call_name_and_the_debug_text(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:8701:    fn artifact_failures_carry_the_call_name_and_the_debug_text() {
$ git grep -n "fn unsupported_inputs_are_rejected_without_entering_nvrtc_or_the_driver(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:8886:    fn unsupported_inputs_are_rejected_without_entering_nvrtc_or_the_driver() {
$ git grep -n "fn artifact_failures_name_the_failed_call_and_add_no_pipeline(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:8992:    fn artifact_failures_name_the_failed_call_and_add_no_pipeline() {
```

出力 (3):

```text
$ git grep -n "fn classify_pipeline_input" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:833:fn classify_pipeline_input<'a>(
$ git grep -n "fn backend_failure(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:791:fn backend_failure(op: &'static str, e: impl std::fmt::Debug) -> ContractError {
$ git grep -n "fn allocation_failure(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:782:fn allocation_failure(op: &'static str, requested_bytes: u64, e: DriverError) -> ContractError {
$ git grep -n "fn artifact_failure(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:962:fn artifact_failure(
$ git grep -n "fn nvrtc_failure(" 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:1004:fn nvrtc_failure(e: CompileError) -> ContractError {
$ git grep -n "pipeline_driver_calls: AtomicU64," 298023ca -- crates/contract/src/backend_cuda.rs
298023ca:crates/contract/src/backend_cuda.rs:2101:    pipeline_driver_calls: AtomicU64,
```

出力 (4):

```text
$ git show 298023ca:crates/contract/src/backend_cuda.rs | grep -cF 'is Metal-only and'
3
$ git diff --numstat 3dd568de^ 3dd568de -- crates/contract/src/backend_cuda.rs
175	9	crates/contract/src/backend_cuda.rs
$ git diff --numstat 298023ca^ 298023ca -- crates/contract/src/backend_cuda.rs
306	0	crates/contract/src/backend_cuda.rs
```

出力 (5):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::pipeline_error 2>&1 | grep -E '^(running|test result:)'
running 5 tests
test result: ok. 3 passed; 0 failed; 2 ignored; 0 measured; 132 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (6):

```text
$ git diff --stat 3dd568de^ 298023ca -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml
$ make deps-check; echo exit=$?
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
exit=0
```

出力 (7):

```text
$ git diff --stat 3dd568de^ 298023ca -- crates/contract/tests/backend_cuda_unsafe_count.rs crates/core
$ git show 3dd568de^:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show 298023ca:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
```

出力 (8):

```text
$ gh run list --branch main --event push --commit 298023cab4e686ba9487b1ad9264083c5df99782 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44700838580
$ gh run view 44700838580 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
298023cab4e686ba9487b1ad9264083c5df99782
make deny (cargo-deny): success
make verify + build-profiles (macos-latest): success
make verify (fmt + clippy + test): success
$ gh run view 44700838580 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136168551924
$ gh run view 44700838580 --job 136168551924 --log | grep -oE 'test backend_cuda::pipeline_error(_device)?_tests::[a-z_]+ \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      6 test backend_cuda::pipeline_error_device_tests::artifact_failures_name_the_failed_call_and_add_no_pipeline ... ignored, requires a CUDA device
      6 test backend_cuda::pipeline_error_device_tests::unsupported_inputs_are_rejected_without_entering_nvrtc_or_the_driver ... ignored, requires a CUDA device
      6 test backend_cuda::pipeline_error_tests::artifact_failures_carry_the_call_name_and_the_debug_text ... ok
      6 test backend_cuda::pipeline_error_tests::driver_failures_carry_the_call_name_and_the_debug_text ... ok
      6 test backend_cuda::pipeline_error_tests::unsupported_inputs_name_the_variant_and_the_accepted_inputs ... ok
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 受け付けない入力の理由に Metal 専用を載せる形と失敗の写しの値の試験を足す | #4152 |
| 2/3 | 受け付けない入力と成果物の生成の失敗の device の要る試験を ignore 付きで足す | #4153 |
| 3/3 | 完了の条件の確かめ | #4154 |

## 今の値の補い（base f3990c4c）

記録時点の表と既存の節は書き換えない。以下は base `f3990c4c` で確かめた今の値。

### 受け付けない入力の表の補い

「受け付けない入力の理由と失敗の写し（1/3）」の表の 6 入力のほかに、`PipelineInput::Wgsl` は `classify_pipeline_input` が `ContractError::Unsupported { op: "create_pipeline", backend: Cuda }` で返し、`ArtifactRejected` の `reason` を持たない。`create_pipeline` は `classify_pipeline_input` の結果を `?` で返すので、同じ値が `create_pipeline` から出る。分類の関数の試験は `cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified`、`create_pipeline` 越しの device の要る試験は `wgsl_is_unsupported_and_msl_is_unsupported_input`。2 つの試験の行は base `f3990c4c` の push の CI の run のログにある。

```text
$ git grep -n "PipelineInput::Wgsl { .. } =>" f3990c4ca0fd6af764ad605a35536eabe17f1947 -- crates/contract/src/backend_cuda.rs
f3990c4ca0fd6af764ad605a35536eabe17f1947:crates/contract/src/backend_cuda.rs:855:        PipelineInput::Wgsl { .. } => {
$ git grep -n "let accepted = classify_pipeline_input(&input)?;" f3990c4ca0fd6af764ad605a35536eabe17f1947 -- crates/contract/src/backend_cuda.rs
f3990c4ca0fd6af764ad605a35536eabe17f1947:crates/contract/src/backend_cuda.rs:3150:        let accepted = classify_pipeline_input(&input)?;
$ git grep -n "fn wgsl_is_unsupported_and_msl_is_unsupported_input(" f3990c4ca0fd6af764ad605a35536eabe17f1947 -- crates/contract/src/backend_cuda.rs
f3990c4ca0fd6af764ad605a35536eabe17f1947:crates/contract/src/backend_cuda.rs:5030:    fn wgsl_is_unsupported_and_msl_is_unsupported_input() {
$ gh run list --branch main --event push --commit f3990c4ca0fd6af764ad605a35536eabe17f1947 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44725812669
$ gh run view 44725812669 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136252800579
$ gh run view 44725812669 --job 136252800579 --log | awk -F'\t' '$2=="make verify"' | grep -oE 'test backend_cuda::(tests::cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified|device_tests::wgsl_is_unsupported_and_msl_is_unsupported_input) \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      1 test backend_cuda::device_tests::wgsl_is_unsupported_and_msl_is_unsupported_input ... ignored, requires a CUDA device
      1 test backend_cuda::tests::cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified ... ok
```

### 「完了の条件の確かめ（3/3）」の (5)(6) と時点のコミットの対応

(5) の試験の名前と結果は、時点のコミット `298023ca` の push の CI の run のログに同じ行がある。(6) の `make deps-check` の行も同じ run の `make deps-check` の段にある。run と job の番号は次の行で引ける。

```text
$ gh run list --branch main --event push --commit 298023cab4e686ba9487b1ad9264083c5df99782 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44700838580
$ gh run view 44700838580 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)" or .name=="make deny (cargo-deny)")|"\(.databaseId) \(.name)"'
136168551823 make deny (cargo-deny)
136168551924 make verify (fmt + clippy + test)
$ gh run view 44700838580 --job 136168551924 --log | awk -F'\t' '$2=="make verify"' | grep -oE 'test backend_cuda::pipeline_error(_device)?_tests::[a-z_]+ \.\.\. (ok|ignored, requires a CUDA device)'
test backend_cuda::pipeline_error_device_tests::artifact_failures_name_the_failed_call_and_add_no_pipeline ... ignored, requires a CUDA device
test backend_cuda::pipeline_error_device_tests::unsupported_inputs_are_rejected_without_entering_nvrtc_or_the_driver ... ignored, requires a CUDA device
test backend_cuda::pipeline_error_tests::artifact_failures_carry_the_call_name_and_the_debug_text ... ok
test backend_cuda::pipeline_error_tests::unsupported_inputs_name_the_variant_and_the_accepted_inputs ... ok
test backend_cuda::pipeline_error_tests::driver_failures_carry_the_call_name_and_the_debug_text ... ok
$ gh run view 44700838580 --job 136168551924 --log | awk -F'\t' '$2=="make verify"{print $3}' | awk '/test backend_cuda::pipeline_error_tests::unsupported_inputs_name_the_variant_and_the_accepted_inputs/{f=1} f&&/test result:/{print; exit}' | grep -oE 'test result: .*'
test result: ok. 412 passed; 0 failed; 71 ignored; 0 measured; 0 filtered out; finished in 0.17s
$ gh run view 44700838580 --job 136168551924 --log | awk -F'\t' '$2=="make verify"{print $3}' | awk '/test backend_cuda_unsafe_count_matches_expected/{f=1} f&&/test result:/{print; exit}' | grep -oE 'test result: .*'
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
$ gh run view 44700838580 --job 136168551823 --log | awk -F'\t' '$2=="make deps-check"' | grep -oE '\[deps-check.sh\] ok: .*'
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
```

1 つ目の `test result:` の行は `pipeline_error_tests` の 3 本を含む lib の試験の実行、2 つ目は `backend_cuda_unsafe_count` の試験の実行の行。段 `make verify` に絞るのは、同じ試験の名前が `make build-profiles` の段にも現れるため。

### 「完了の条件の確かめ（3/3）」の (7) の補い

(7) の `unsafe` の行の数は `backend_cuda.rs` 全体の数。`3dd568de^` から `298023ca` までの差の中で `unsafe` を含む追加・削除の行の数は次のとおり。

```text
$ git diff 3dd568dec28b556be7b014adcf235cea8e90de29^ 298023cab4e686ba9487b1ad9264083c5df99782 -- crates/contract/src/backend_cuda.rs | grep -c '^[-+].*unsafe'
0
```
