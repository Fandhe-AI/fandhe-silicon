# 上の層と下-2 を 1 つの device で通す試験の所在（TASK-261）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-261。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験 | `crates/upper/tests/upper_exec_same_device.rs` |
| feature の条件 | `crates/upper/Cargo.toml` の `[[test]]` の `required-features = ["native-blocking-wait"]` |
| 使う公開のパス | `fandhe_silicon_exec::{Exec, OptIn, Outcome, OpId, F16FmaOperands}`（`Exec` の所在は TASK-919）、`fandhe_silicon_upper::{UpperDevice, Buffer, SubmitId}`（TASK-2386）、`fandhe_silicon_core::device::Device`、`fandhe_silicon_core::memory::BufferUsage`、`fandhe_silicon_core::feature_key::FEATURE_SHADER_F16` |
| 試験だけの型 | `Recorder`（`crates/upper/tests/common/mod.rs`。TASK-2386）。`StubDevice`（TASK-156）を包み、記録の列に `QueryEx`（`query_ex` の鍵）が加わった |
| 関連 | 層をまたぐ型の受け渡しの試験は `docs/cross-layer-types.md`（TASK-260） |

## 上の層と下-2 を 1 つの device で通す試験（1/3）

1 つの `Recorder` を `&dyn Device` で `UpperDevice` と `Exec` の両方に渡す。
buffer は 16 バイト・用途 `STORAGE` で、`Buffer::from_handle` で包み、`handle_mut()` の値を下-2 の device の形に渡す。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `upper_and_exec_calls_reach_one_device` | 既定が「許さない」の `Exec`、上書き `None`。記録先を作り、`query_route(OpId::F16Fma, None)`、`f16_fma`（`n` は 8）、pass の開始と終了、`finish`、`submit`、`wait_blocking(id, None)`、`gpu_timing` の順に呼ぶ | `f16_fma` が `Ok(Outcome::Unavailable { report })` で `report` は `query_route` の報告と等しい。`submit` は `Ok`、`wait_blocking` は `Ok(())`。buffer の中身は埋めた値のまま。`Recorder` の記録は列 1 のとおり（長さ 6）で、`Poll`・`SubmitMany`・`OnComplete`・`DispatchEx` は無い |
| `exec_failure_does_not_change_upper_submit` | 上書き `Some(OptIn { allow_gpu_emulated: false, allow_cpu: true })`、`query_route` を呼ばずに同じ並び | `f16_fma` の戻りが `Ok(Outcome::Unavailable { .. })` ではない。`submit` は `Ok`、`wait_blocking` は `Ok(())`。各 buffer は大きさ 16・用途 `STORAGE`、読み出しの長さは 16。`Recorder` の記録は列 2 のとおり（長さ 5）で、`Poll`・`SubmitMany`・`OnComplete`・`DispatchEx` は無い |

記録の列（`t` は `Submit` が返した印）。

```text
1. CreateEncoder, QueryEx(FEATURE_SHADER_F16), QueryEx(FEATURE_SHADER_F16), Submit(wait_len 0, t), WaitBlocking(t, None), GpuTiming(t)
2. CreateEncoder, QueryEx(FEATURE_SHADER_F16), Submit(wait_len 0, t), WaitBlocking(t, None), GpuTiming(t)
```

### 確かめ方

```text
cargo test -p fandhe-silicon-upper --features native-blocking-wait --test upper_exec_same_device
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
```
