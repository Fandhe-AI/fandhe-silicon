# CUDA の背後実装の PTX の入力のパイプラインの生成（TASK-804）の所在と本リポの現状

TASK-804 の作業のうち、CUDA の背後実装の `ComputeOps::create_pipeline` が `PipelineInput::Ptx` を受け付けて読み込む形と、NVRTC の呼び出し口の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-804。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入力の分類 | `crates/contract/src/backend_cuda.rs` の `classify_pipeline_input`。受け付けた入力は `AcceptedInput` で返す | `git grep -n "fn classify_pipeline_input\|enum AcceptedInput" -- crates/contract/src/backend_cuda.rs` |
| 読み込む PTX を作る | 同じファイルの `module_input` と `check_ptx_no_nul` | `git grep -n "fn module_input\|fn check_ptx_no_nul" -- crates/contract/src/backend_cuda.rs` |
| NVRTC の呼び出し口 | 同じファイルの `impl CudaDevice` の `compile_with_nvrtc` | `git grep -n "fn compile_with_nvrtc" -- crates/contract/src/backend_cuda.rs` |
| 口 | `impl ComputeOps for CudaDevice` の `create_pipeline` | `git grep -n "fn create_pipeline(" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `tests` module と `ptx_input_tests` module。device に触らない | 下の「確かめ方」 |

## PTX の入力の受け付けと NVRTC の呼び出し口（1/3）

`create_pipeline` は `module_input` に、device の NVRTC の呼び出し口 `compile_with_nvrtc` を渡して呼ぶ（`git grep -n "module_input(accepted" -- crates/contract/src/backend_cuda.rs`）。

入力ごとの分類と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| `CudaC` | 通る（`AcceptedInput::CudaC`） | `cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified` |
| `Ptx` | 通る（`AcceptedInput::Ptx`） | 同上 |
| `Wgsl` | `Unsupported { op: "create_pipeline", backend: Cuda }` | 同上 |
| `Msl`・`SpirV`・`Dxil`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions` | `ArtifactRejected { kind: "unsupported-input" }` | 同上 |

`Msl` と `MslSpecializedWithOptions` の理由の文は `pipeline input <variant> is not accepted by the CUDA backend; accepted inputs are CudaC and Ptx`（`<variant>` は受けた variant の名前）。試験は `cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified`。

`module_input` が返す値と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| `Ptx` | テキストをそのまま PTX にし、入口名と返す。渡した呼び出し口を呼ばない | `ptx_input_becomes_the_module_text_without_calling_the_compiler` |
| `CudaC` | 渡した呼び出し口をソースで 1 回呼び、その結果と入口名を返す。呼び出し口の失敗はそのまま返す | `cuda_c_input_calls_the_compiler_once_with_the_source` |
| `Ptx` のテキストの NUL のバイト | `ArtifactRejected { kind: "module" }`。呼び出し口を呼ばない | `nul_bytes_are_rejected_before_the_module_text_is_made` |
| `CudaC` のソースの NUL のバイト | `ArtifactRejected { kind: "cuda-c" }`。呼び出し口を呼ばない | 同上 |
| 入口名の NUL のバイト（`Ptx`・`CudaC`） | `ArtifactRejected { kind: "function" }`。呼び出し口を呼ばない | 同上 |

テキストと入口名の両方に NUL のバイトがある `Ptx` は `module`（テキストを先に見る）。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::ptx_input_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::tests::cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified
```

1 つ目は 3 件、2 つ目は 1 件が通る。

## device の要る試験（2/3）

device に触る試験は `crates/contract/src/backend_cuda.rs` の `ptx_pipeline_device_tests` module にあり、どれも `#[ignore = "requires a CUDA device"]` を付けている。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。試験のビルドの device は `compile_with_nvrtc` に入った回数を持ち、試験はその回数を読む。PTX のテキストは、試験が CUDA C のカーネルを、`--gpu-architecture=compute_<major><minor>` と `--frandom-seed=0x5eed` を付けて NVRTC で作る（device の `compile_with_nvrtc` を通らない）。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `nvrtc_ptx_text_creates_a_pipeline_of_this_device` | `Ptx` の入力で `create_pipeline` を 2 回 | 2 回とも通り、その device の識別子を持つ。2 つの識別子は異なる。表の件数は 1 回ごとに 1 増える |
| `ptx_creation_does_not_enter_the_nvrtc_call_site` | `Ptx` の入力で `create_pipeline` を 2 回、続けて同じカーネルの `CudaC` の入力で 1 回 | `Ptx` の 2 回の後も `compile_with_nvrtc` に入った回数は 0。`CudaC` の 1 回の後は 1 |
| `ptx_load_failures_are_artifact_rejected_without_entering_the_nvrtc_call_site` | 読めないテキスト・NUL のバイトを含むテキスト・無い入口名・NUL のバイトを含む入口名の `Ptx` の入力で `create_pipeline`、続けて `CudaC` の入力で 1 回 | 読めないテキストと NUL のバイトを含むテキストは `ArtifactRejected { kind: "module" }`、無い入口名と NUL のバイトを含む入口名は `ArtifactRejected { kind: "function" }`。表の件数は増えない。4 つの後も `compile_with_nvrtc` に入った回数は 0。`CudaC` の 1 回の後は 1 |

| 項目 | 値 |
| ---- | -- |
| module | `crates/contract/src/backend_cuda.rs` の `ptx_pipeline_device_tests` |
| feature | `backend-cuda` |
| `#[ignore]` | `requires a CUDA device` |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::ptx_pipeline_device_tests
```

3 件が `ignored` と数えられる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-804。ポインタのみ）。

時点のコミット: `b745ad06`。`docs/spec` の gitlink は `96476a8254c0c1fba240acf062644b4433123e99`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `ptx_input_tests` と `ptx_pipeline_device_tests` を `awk` で切り出し、`#[test]` と `#[ignore` の行頭の数を `grep -c` で数える | 出力 (1) |
| 2 | 受け手の試験の名前 | 7 本の関数を `git grep -n "fn <名前>" -- crates` で 1 つずつ引く | 出力 (2) |
| 3 | NVRTC の呼び出し口 | `mod tests {` の行より前を `grep -n` で絞る | 出力 (3) |
| 4 | device に触らない試験 | `ptx_input_tests`・`ptx_pipeline_device_tests`・`backend_cuda_unsafe_count` の `cargo test` | 出力 (4) |
| 5 | 依存 | `bb70bb12` から HEAD までの `git diff --stat` と `make deps-check` | 出力 (5) |
| 6 | unsafe の数 | 同じ範囲の `backend_cuda_unsafe_count.rs` の差と、`unsafe` の語を含む行の数 | 出力 (6) |
| 7 | CI | 2/3 のマージの後の main の push の run | 出力 (7) |

(5)・(6) の `bb70bb12` は、1/3 の 1 つ前のコミット。

出力 (1)

```text
$ awk '/^mod ptx_input_tests \{$/{f=1} f{print} f&&/^}$/{exit}' crates/contract/src/backend_cuda.rs | grep -c '^[[:space:]]*#\[test\]'
3
$ awk '/^mod ptx_input_tests \{$/{f=1} f{print} f&&/^}$/{exit}' crates/contract/src/backend_cuda.rs | grep -c '^[[:space:]]*#\[ignore'
0
$ awk '/^mod ptx_pipeline_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' crates/contract/src/backend_cuda.rs | grep -c '^[[:space:]]*#\[test\]'
3
$ awk '/^mod ptx_pipeline_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' crates/contract/src/backend_cuda.rs | grep -c '^[[:space:]]*#\[ignore'
3
```

出力 (2)

```text
$ git grep -n "fn ptx_input_becomes_the_module_text_without_calling_the_compiler" -- crates
crates/contract/src/backend_cuda.rs:8230:    fn ptx_input_becomes_the_module_text_without_calling_the_compiler() {
$ git grep -n "fn cuda_c_input_calls_the_compiler_once_with_the_source" -- crates
crates/contract/src/backend_cuda.rs:8249:    fn cuda_c_input_calls_the_compiler_once_with_the_source() {
$ git grep -n "fn nul_bytes_are_rejected_before_the_module_text_is_made" -- crates
crates/contract/src/backend_cuda.rs:8282:    fn nul_bytes_are_rejected_before_the_module_text_is_made() {
$ git grep -n "fn nvrtc_ptx_text_creates_a_pipeline_of_this_device" -- crates
crates/contract/src/backend_cuda.rs:8413:    fn nvrtc_ptx_text_creates_a_pipeline_of_this_device() {
$ git grep -n "fn ptx_creation_does_not_enter_the_nvrtc_call_site" -- crates
crates/contract/src/backend_cuda.rs:8450:    fn ptx_creation_does_not_enter_the_nvrtc_call_site() {
$ git grep -n "fn ptx_load_failures_are_artifact_rejected_without_entering_the_nvrtc_call_site" -- crates
crates/contract/src/backend_cuda.rs:8478:    fn ptx_load_failures_are_artifact_rejected_without_entering_the_nvrtc_call_site() {
$ git grep -n "fn cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified" -- crates
crates/contract/src/backend_cuda.rs:4697:    fn cuda_c_and_ptx_are_accepted_and_other_inputs_are_classified() {
```

出力 (3)

```text
$ awk '/^mod tests \{$/{exit} {print}' crates/contract/src/backend_cuda.rs | grep -n 'compile_cuda_c(\|compile_with_nvrtc(\|compile_ptx_with_opts\|Ptx::from_src'
1025:fn compile_cuda_c(source: &str, capability: (i32, i32)) -> Result<Ptx, ContractError> {
1034:        cudarc::nvrtc::compile_ptx_with_opts(source, options).map_err(nvrtc_failure)
1058:            Ok((Ptx::from_src(source), entry_point))
2250:    fn compile_with_nvrtc(&self, source: &str) -> Result<Ptx, ContractError> {
2253:        compile_cuda_c(source, self.compute_capability)
3142:        let (ptx, entry_point) = module_input(accepted, |source| self.compile_with_nvrtc(source))?;
```

出力 (4)

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::ptx_input_tests
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 128 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::ptx_pipeline_device_tests
running 3 tests
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 128 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (5)

```text
$ git diff --stat bb70bb12..HEAD -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml crates/core
$ make deps-check; echo $?
0
```

出力 (6)

```text
$ git diff --stat bb70bb12..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs
$ git show bb70bb12:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show HEAD:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (7)

```text
$ gh run view 44660780805 --json conclusion,headSha --jq '.conclusion+" "+.headSha'
success b745ad065d90c0e61842106eba50a537f1b3748e
$ gh run view 44660780805 --json jobs --jq '.jobs[]|select(.name|test("verify|deny"))|"\(.name)\t\(.conclusion)"'
make verify (fmt + clippy + test)	success
make verify + build-profiles (macos-latest)	success
make deny (cargo-deny)	success
$ gh run view 44660780805 --log --job <make verify (fmt + clippy + test) の job> | grep -oE 'test backend_cuda::ptx_(pipeline_device|input)_tests::[a-z_]+ \.\.\. .*' | sort | uniq -c
      6 test backend_cuda::ptx_input_tests::cuda_c_input_calls_the_compiler_once_with_the_source ... ok
      6 test backend_cuda::ptx_input_tests::nul_bytes_are_rejected_before_the_module_text_is_made ... ok
      6 test backend_cuda::ptx_input_tests::ptx_input_becomes_the_module_text_without_calling_the_compiler ... ok
      6 test backend_cuda::ptx_pipeline_device_tests::nvrtc_ptx_text_creates_a_pipeline_of_this_device ... ignored, requires a CUDA device
      6 test backend_cuda::ptx_pipeline_device_tests::ptx_creation_does_not_enter_the_nvrtc_call_site ... ignored, requires a CUDA device
      6 test backend_cuda::ptx_pipeline_device_tests::ptx_load_failures_are_artifact_rejected_without_entering_the_nvrtc_call_site ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | PTX の入力を NVRTC を呼ばずに読み込む形と NVRTC の呼び出し口を実装する | #5721 |
| 2/3 | PTX の入力のパイプラインの生成の device の要る試験を ignore 付きで足す | #5722 |
| 3/3 | 完了の条件の確かめ | #5723 |
