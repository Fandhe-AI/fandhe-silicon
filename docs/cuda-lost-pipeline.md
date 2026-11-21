# CUDA の背後実装の模擬のロストの後のパイプラインの生成とロストの理由（TASK-561）の所在と本リポの現状

TASK-561 の作業のうち、CUDA の背後実装の模擬のロストの後のパイプラインの生成と、模擬のロストの理由の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-561。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_cuda.rs` | 同左 |
| `create_pipeline` | `CudaDevice` の `ComputeOps` の実装。最初に `check_not_lost` を呼ぶ | `git grep -n -F "fn create_pipeline(" -- crates/contract/src/backend_cuda.rs` |
| ロストの後の操作の拒否の部品 | [cuda-device-lost](cuda-device-lost.md) | 同左 |
| 検証の報告の受け口の登録 | [cuda-on-complete](cuda-on-complete.md) | 同左 |

## ロストの後のパイプラインの生成と device の要る試験（1/2）

どの試験も CUDA の device に触るので `#[ignore]` を付け、通常の確認では走らず `ignored` になる。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `create_pipeline_after_the_loss_returns_device_lost_without_compiling` | 具体型の device の `create_pipeline`・`create_pipeline_checked` と、`Device::compute` から得た `create_pipeline` | ロストの前の `create_pipeline` で、NVRTC の呼び出し口に入った回数と下位の呼び出しの前に数えた回数が増える。ロストの後は `CudaC`・`Ptx`・`Msl`・`Wgsl`・ソースに NUL のバイトを持つ `CudaC`・重複した slot の入力と、引数の宣言が合う `create_pipeline_checked` が `Err(DeviceLost { generation: generation() })`。2 回繰り返しても同じで、世代・パイプラインの表の件数と次の識別子・2 つの回数は変わらない |
| `a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback` | 具体型の device の `set_lost_callback`・`set_validation_callback`・`simulate_validation_report`・`fault_hooks` | 報告を模す差し込みは `Err(UnsupportedExtensionDefault { op: "simulate_validation_report" })` で、ロストせず世代も変わらない。理由に `Unknown`・`Unrecoverable` を指定した模擬のロストは、通知の列がその理由の 1 件で、世代が進む。検証の報告の受け口に登録した関数は呼ばれず、届けた数と届けなかった数は 0。ロストの後の `create_pipeline` は `Err(DeviceLost { generation: generation() })` |

| 置き場 | cfg・feature |
| ------ | ------------ |
| `crates/contract/src/backend_cuda.rs` の `lost_pipeline_device_tests` | `test`・`fault-injection` |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib backend_cuda::lost_pipeline_device_tests
```

2 件が `ignored` になる。

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-561。ポインタのみ）。

時点のコミット: `0a5b8a8d`。`docs/spec` の gitlink は `ac56f928301ffca0c15d857b56302916a3cce0fb`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `0a5b8a8d` の `crates/contract/src/backend_cuda.rs` の `lost_pipeline_device_tests` に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n` で 4 つの関数名を 1 つずつ引く | 出力 (2) |
| (3) | 口の所在 | `git grep -n -F` と `git show` で `create_pipeline` の頭と 3 つの字句を引く | 出力 (3) |
| (4) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 4 つの実行 | 出力 (4) |
| (5) | 依存と crates の差 | `0a5b8a8d^ 0a5b8a8d` の差 | 出力 (5) |
| (6) | CI | 1/2 のマージの後の main の push の run | 出力 (6) |

`0a5b8a8d^` は 1/2 の squash `0a5b8a8d` の親。(4) の `cargo test` は作業木で流した。

出力 (1):

```text
$ git show 0a5b8a8d:crates/contract/src/backend_cuda.rs | awk '/^mod lost_pipeline_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
2
$ git show 0a5b8a8d:crates/contract/src/backend_cuda.rs | awk '/^mod lost_pipeline_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
2
```

出力 (2):

```text
$ git grep -n "fn create_pipeline_after_the_loss_returns_device_lost_without_compiling(" 0a5b8a8d -- crates/contract/src/backend_cuda.rs
0a5b8a8d:crates/contract/src/backend_cuda.rs:11276:    fn create_pipeline_after_the_loss_returns_device_lost_without_compiling() {
$ git grep -n "fn a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback(" 0a5b8a8d -- crates/contract/src/backend_cuda.rs
0a5b8a8d:crates/contract/src/backend_cuda.rs:11376:    fn a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback() {
$ git grep -n "fn a_token_from_before_the_loss_polls_and_completes_as_lost(" 0a5b8a8d -- crates/contract/src/backend_cuda.rs
0a5b8a8d:crates/contract/src/backend_cuda.rs:5703:        fn a_token_from_before_the_loss_polls_and_completes_as_lost() {
$ git grep -n "fn a_callback_registered_on_a_gated_token_is_called_lost_once_by_a_simulated_loss(" 0a5b8a8d -- crates/contract/src/backend_cuda.rs
0a5b8a8d:crates/contract/src/backend_cuda.rs:7398:    fn a_callback_registered_on_a_gated_token_is_called_lost_once_by_a_simulated_loss() {
```

出力 (3):

```text
$ git grep -n -F 'let accepted = classify_pipeline_input(&input)?;' 0a5b8a8d -- crates/contract/src/backend_cuda.rs
0a5b8a8d:crates/contract/src/backend_cuda.rs:3608:        let accepted = classify_pipeline_input(&input)?;
$ git grep -n 'fn create_pipeline_checked' 0a5b8a8d -- crates/contract/src/backend_cuda.rs | wc -l
0
$ git grep -n 'fn create_pipeline_checked' 0a5b8a8d -- crates/core/src/execution/compute.rs
0a5b8a8d:crates/core/src/execution/compute.rs:262:    fn create_pipeline_checked(
$ git grep -n 'fn simulate_validation_report' 0a5b8a8d -- crates/contract/src/backend_cuda.rs | wc -l
0
$ git show 0a5b8a8d:crates/contract/src/backend_cuda.rs | awk '/^    fn create_pipeline\($/{f=1} f{print; n++} n==7{exit}'
    fn create_pipeline(
        &self,
        input: PipelineInput<'_>,
        layout: &BindingLayout,
    ) -> Result<PipelineHandle, ContractError> {
        self.check_not_lost()?;
        let accepted = classify_pipeline_input(&input)?;
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib backend_cuda::lost_pipeline_device_tests 2>&1 | grep -E '^(running|test result:)|\.\.\. ignored, requires a CUDA device$'
running 2 tests
test backend_cuda::lost_pipeline_device_tests::a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback ... ignored, requires a CUDA device
test backend_cuda::lost_pipeline_device_tests::create_pipeline_after_the_loss_returns_device_lost_without_compiling ... ignored, requires a CUDA device
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 190 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib backend_cuda::lost_state_tests 2>&1 | grep -E '^(running|test result:)'
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 180 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::lost_check_tests 2>&1 | grep -E '^(running|test result:)'
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 149 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

出力 (5):

```text
$ git diff --stat 0a5b8a8d^ 0a5b8a8d -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml crates/core crates/contract/tests
$ git diff --name-only 0a5b8a8d^ 0a5b8a8d -- crates
crates/contract/src/backend_cuda.rs
```

出力 (6):

```text
$ gh run list --branch main --event push --commit 0a5b8a8d96b1e577a44a524fe8da176db6332a54 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44909456221
$ gh run view 44909456221 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
0a5b8a8d96b1e577a44a524fe8da176db6332a54
make verify + build-profiles (macos-latest): success
make verify (fmt + clippy + test): success
make deny (cargo-deny): success
$ gh run view 44909456221 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136861945617
$ gh run view 44909456221 --job 136861945617 --log | grep -oE 'test [a-z_:]*(create_pipeline_after_the_loss_returns_device_lost_without_compiling|a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback) \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      2 test backend_cuda::lost_pipeline_device_tests::a_simulated_loss_keeps_its_reason_and_calls_no_validation_callback ... ignored, requires a CUDA device
      2 test backend_cuda::lost_pipeline_device_tests::create_pipeline_after_the_loss_returns_device_lost_without_compiling ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後のパイプラインの生成を拒み、device の要る試験を足す | #4214 |
| 2/2 | 完了の条件の確かめ | #4215 |
