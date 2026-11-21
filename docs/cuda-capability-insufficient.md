# CUDA の背後実装の能力不足の模擬（TASK-178）の所在と本リポの現状

TASK-178 の作業のうち、CUDA の背後実装の device に故障の差し込みのデコレータを被せ、能力不足の模擬が返す値とほかの失敗との区別を公開の口から照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-178。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| デコレータ | `crates/core/src/fault/decorator.rs` の `FaultInjecting`・`FaultScript` | `git grep -n "pub struct FaultInjecting" -- crates/core/src/fault/decorator.rs` |
| 能力の問合せ | [cuda-capability-query](cuda-capability-query.md) | 同左 |
| 模擬のロスト | [cuda-simulated-device-lost](cuda-simulated-device-lost.md) | 同左 |
| 初期化の失敗の差し込み | [cuda-device-request](cuda-device-request.md) | 同左 |
| 試験 | `crates/contract/tests/backend_cuda_capability_insufficient.rs` | 下の「device の要る試験（1/2）」 |

## device の要る試験（1/2）

公開の口（`Device` の能力の問合せ・`Device::compute`・`Device::queue`・`Device::memory`・`Device::fault_hooks`）と、デコレータの `create_pipeline_with`・`poll_ex_with`・`query`・`query_ex` を呼ぶ統合の試験。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_simulated_shortage_names_the_feature_and_both_levels_over_the_cuda_device` | `feature:SHADER_FLOAT32_ATOMIC` を `S2` で要る筋書き `CapabilityInsufficient` と、筋書き `PassThrough` のデコレータを同じ device に被せ、問合せとパイプラインの生成を呼ぶ | device の問合せは `S2`。筋書き `CapabilityInsufficient` の問合せは `Unsupported`（`query_ex` の理由は `NotSupported`、根拠は無し）で、`feature:EXPERIMENTAL_MESH_SHADER` の `query_ex` は device の値と同じ。生成は内側を 0 回呼び、`CapabilityInsufficient { feature: feature:SHADER_FLOAT32_ATOMIC, required: S2, actual: Unsupported }` を返し、種類は `Capability`。筋書き `PassThrough` の生成は前後とも `Ok` |
| `the_simulated_shortage_is_distinct_from_the_other_failures_over_the_cuda_device` | 同じ device で、能力不足の模擬の生成、空の投入の印への筋書き `RuntimeError`・`CompletionUnknown` の `poll_ex`、模擬のロストの後の `allocate`・`poll_ex` を呼び、初期化の失敗の差し込みの `request` を呼ぶ | 能力不足は上と同じ値。`poll_ex` は `RuntimeError`・`Unknown`、ロストの後の `allocate` は `DeviceLost { generation }`（device の世代）、`poll_ex` は `Lost`。能力不足・初期化の失敗・ロストの 3 つの値は互いに異なり、種類は順に `Capability`・`Initialization`・`DeviceLost`・`Runtime`・`CompletionUnknown` で互いに異なる |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_capability_insufficient.rs` |
| `required-features` | `backend-cuda`・`fault-injection`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_capability_insufficient
```

2 件が `ignored` と数えられる。

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-178。ポインタのみ）。

時点のコミット: `7402ec0d`。`docs/spec` の gitlink は `ac56f928301ffca0c15d857b56302916a3cce0fb`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `7402ec0d` の `crates/contract/tests/backend_cuda_capability_insufficient.rs` に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n` で 2 つの関数名を 1 つずつ引く | 出力 (2) |
| (3) | デコレータの所在 | `git grep -n -F` で `create_pipeline_with` の頭と `Capabilities` の実装の頭を引く | 出力 (3) |
| (4) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` と `cargo test -p fandhe-silicon-core` の 4 つの実行 | 出力 (4) |
| (5) | 依存と crates の差 | `7402ec0d^ 7402ec0d` の差 | 出力 (5) |
| (6) | CI | 1/2 のマージの後の main の push の run | 出力 (6) |

`7402ec0d^` は 1/2 の squash `7402ec0d` の親。(4) の `cargo test` は作業木で流した。

出力 (1):

```text
$ git show 7402ec0d:crates/contract/tests/backend_cuda_capability_insufficient.rs | grep -c '^[[:space:]]*#\[test\]'
2
$ git show 7402ec0d:crates/contract/tests/backend_cuda_capability_insufficient.rs | grep -c '^[[:space:]]*#\[ignore'
2
```

出力 (2):

```text
$ git grep -n "fn a_simulated_shortage_names_the_feature_and_both_levels_over_the_cuda_device(" 7402ec0d -- crates/contract/tests/backend_cuda_capability_insufficient.rs
7402ec0d:crates/contract/tests/backend_cuda_capability_insufficient.rs:127:fn a_simulated_shortage_names_the_feature_and_both_levels_over_the_cuda_device() {
$ git grep -n "fn the_simulated_shortage_is_distinct_from_the_other_failures_over_the_cuda_device(" 7402ec0d -- crates/contract/tests/backend_cuda_capability_insufficient.rs
7402ec0d:crates/contract/tests/backend_cuda_capability_insufficient.rs:180:fn the_simulated_shortage_is_distinct_from_the_other_failures_over_the_cuda_device() {
```

出力 (3):

```text
$ git grep -n -F 'pub fn create_pipeline_with<T, F>(&self, call_inner: F) -> Result<T, ContractError>' 7402ec0d -- crates/core/src/fault/decorator.rs
7402ec0d:crates/core/src/fault/decorator.rs:212:    pub fn create_pipeline_with<T, F>(&self, call_inner: F) -> Result<T, ContractError>
$ git grep -n -F 'impl<D: Capabilities> Capabilities for FaultInjecting<D> {' 7402ec0d -- crates/core/src/fault/decorator.rs
7402ec0d:crates/core/src/fault/decorator.rs:237:impl<D: Capabilities> Capabilities for FaultInjecting<D> {
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_capability_insufficient 2>&1 | grep -E '^(running|test result:)|\.\.\. ignored, requires a CUDA device$'
running 2 tests
test a_simulated_shortage_names_the_feature_and_both_levels_over_the_cuda_device ... ignored, requires a CUDA device
test the_simulated_shortage_is_distinct_from_the_other_failures_over_the_cuda_device ... ignored, requires a CUDA device
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_fault 2>&1 | grep -E '^(running|test result:)'
running 4 tests
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-core --features fault-injection --test failure_class_contract 2>&1 | grep -E '^(running|test result:)'
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (5):

```text
$ git diff --stat 7402ec0d^ 7402ec0d -- Cargo.lock deps-approved.toml Cargo.toml crates/core crates/contract/src crates/contract/tests/backend_cuda_unsafe_count.rs
$ git diff -U0 7402ec0d^ 7402ec0d -- crates/contract/Cargo.toml | grep '^[-+]' | grep -v -e '^+++ ' -e '^--- '
+[[test]]
+name = "backend_cuda_capability_insufficient"
+required-features = ["backend-cuda", "fault-injection"]
+
$ git diff --name-only 7402ec0d^ 7402ec0d -- crates
crates/contract/Cargo.toml
crates/contract/tests/backend_cuda_capability_insufficient.rs
```

出力 (6):

```text
$ gh run list --branch main --event push --commit 7402ec0df6aebad29cff2589d1b7cf8b538b9e17 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44919390754
$ gh run view 44919390754 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
7402ec0df6aebad29cff2589d1b7cf8b538b9e17
make verify + build-profiles (macos-latest): success
make verify (fmt + clippy + test): success
make deny (cargo-deny): success
$ gh run view 44919390754 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136891919110
$ gh run view 44919390754 --job 136891919110 --log | grep -oE 'test (a_simulated_shortage_names_the_feature_and_both_levels_over_the_cuda_device|the_simulated_shortage_is_distinct_from_the_other_failures_over_the_cuda_device) \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      2 test a_simulated_shortage_names_the_feature_and_both_levels_over_the_cuda_device ... ignored, requires a CUDA device
      2 test the_simulated_shortage_is_distinct_from_the_other_failures_over_the_cuda_device ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 能力不足の模擬の device の要る試験を ignore 付きで足す | #3782 |
| 2/2 | 完了の条件の確かめ | #3783 |
