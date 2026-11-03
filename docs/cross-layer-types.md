# 層をまたぐ型の受け渡しの試験の所在（TASK-260）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-260。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験 | `crates/upper/tests/cross_layer_types.rs` |
| feature の条件 | `crates/upper/Cargo.toml` の `[[test]]` の `required-features = ["native-blocking-wait"]` |
| 使う公開のパス | `fandhe_silicon_exec::{Exec, OptIn, Outcome, OpId, F16FmaOperands, SubgroupSumU32Operands, SubgroupSumF32Operands, AtomicAddOperands}`（`Exec` の所在は TASK-919）、`fandhe_silicon_upper::{UpperDevice, Buffer}`（TASK-2386）、`fandhe_silicon_core::stub_device::StubDevice`（TASK-156） |

## 層をまたぐ受け渡しの試験（1/2）

試験用の device 1 つを、下-2 の `Exec` と上の層の `UpperDevice` の両方に `&dyn Device` で渡す。
buffer は 16 バイト・用途 `STORAGE` で、`Buffer::from_handle` で包み、`handle_mut()` の値を下-2 の device の形に渡す。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `upper_buffers_pass_to_exec_device_operands` | 既定が「許さない」の `Exec`、上書き `None`、4 つの入口を device の形で呼ぶ（`n` は 8、`atomic_add` の `m` は 1） | どれも `Ok(Outcome::Unavailable { report })` で `report.route` は `None`、`report.op` は入口の操作。呼んだ後の各 buffer の中身は埋めた値のまま |
| `override_call_returns_contract_error_type` | 上書き `Some(OptIn { allow_gpu_emulated: false, allow_cpu: true })` で 4 つの入口を呼ぶ | 戻りが `Result<Outcome, ContractError>` で、`Ok(Outcome::Unavailable { .. })` ではない。呼んだ後の各 `Buffer` は大きさ 16・用途 `STORAGE`、読み出しの長さは 16 |
| `exec_report_level_matches_device_query` | 4 つの操作と能力の鍵（`FEATURE_SHADER_F16`・`FEATURE_SUBGROUP`・`FEATURE_SHADER_FLOAT32_ATOMIC`）の組、`query_route(op, None)` | `observed_level` が `query_ex(鍵).level`、`native_reason` が `CapabilityUnsupported(query_ex(鍵).unsupported_reason)`。`f16_fma` の入口の `Unavailable` の `report` が `query_route` の報告と等しい |
| `upper_submit_and_wait_on_the_same_device` | 下-2 の入口を呼んだ後、同じ device から作った `UpperDevice` の `queue()` で空の `CommandBuffer` を `submit` し `wait_blocking(id, None)` | `submit` が `Ok(SubmitId)`、`wait_blocking` が `Ok(())`。下-2 の値も続けて使える |

### 確かめ方

```text
cargo test -p fandhe-silicon-upper --features native-blocking-wait --test cross_layer_types
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
```
