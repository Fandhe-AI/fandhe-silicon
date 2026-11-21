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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 能力不足の模擬の device の要る試験を ignore 付きで足す | #3782 |
