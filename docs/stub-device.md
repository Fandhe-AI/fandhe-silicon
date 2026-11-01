# 試験用の device（TASK-156）の所在と本リポの現状

TASK-156 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
事実はリポジトリのルートで流したコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-156・`04-behavior/` の DEV-34。ポインタのみ）。内容・理由・経緯は本書に置かない。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-126 | private spec 側の記録に従う（ポインタのみ） | なし |
| TASK-136 | `crates/core/src/device.rs`・`crates/core/src/capability.rs` | [contract-device-types](contract-device-types.md) |
| TASK-23 | `crates/core/src/test_frame*`（feature `test-support` の裏） | [test-frame](test-frame.md) |

## 本リポの現状

各表の「確かめ方」は、リポジトリのルートで流すコマンドである。

### core の構成

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 条件なしの module | 13 個（`artifact_cache`・`binding`・`capability`・`decision`・`device`・`device_lost`・`error`・`execution`・`hash`・`memory`・`spirv`・`texture_format`・`validation`） | `grep -n "pub mod" crates/core/src/lib.rs` |
| feature `ext-interop` の裏の module | `interop` | 同上 |
| feature `test-support` の裏の module | `capability_rule`・`parity`・`test_frame`・`rerun` | 同上 |
| feature `fault-injection` の裏の module | `fault` | 同上 |
| feature | `test-support`・`fault-injection`・`native-blocking-wait`・`ext-interop` の 4 つ。どれも空の一覧（`= []`）で、`default` は無い | `sed -n '/\[features\]/,/^\[\[test\]\]/p' crates/core/Cargo.toml` |
| 外部依存 | 0 件 | `cargo metadata --format-version 1 --no-deps \| jq '.packages[] \| select(.name=="fandhe-silicon-core") \| {features, dependencies}'` と `cargo tree -p fandhe-silicon-core -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]`（`crates/core/src/lib.rs:28`） | `grep -n unsafe_code crates/core/src/lib.rs` |

### device の口と部品

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `Device` | `crates/core/src/device.rs:101`。親は `Capabilities`。必須の method は `backend`・`ownership`・`generation`・`memory`・`queue`・`diagnostics`。既定の本体を持つ method は `external_interop`・`physical_gpu_id`（feature `ext-interop`）と `fault_hooks`（feature `fault-injection`）。`compute`・`graphics` の method は持たない | `grep -nE '^\s*fn \|#\[cfg' crates/core/src/device.rs` |
| `DeviceProvider` | `crates/core/src/device.rs:151`。`request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError>`（`self` を取らない関連関数） | 同上 |
| `MemoryOps` | `crates/core/src/memory/ops.rs:10`。必須は `allocate`・`upload`・`download_async`（native と wasm32 の 2 版）・`free`。`download_blocking` は feature `native-blocking-wait` の裏。`pending_release_count` は既定の本体を持つ | `grep -nE '^\s*fn \|#\[cfg' crates/core/src/memory/ops.rs` |
| `CommandQueue` | `crates/core/src/execution/submit.rs:216`。必須は `create_encoder`・`submit`・`submit_many`・`poll`・`on_complete`（native と wasm32 の 2 版）・`dispatch_ex`。`wait_blocking` は feature `native-blocking-wait` の裏。`poll_ex`・`submit_ex`・`gpu_timing`・`dispatch` は既定の本体を持つ。`submit_exportable` は feature `ext-interop` の裏で既定の本体を持つ | `grep -nE '^\s*fn \|#\[cfg' crates/core/src/execution/submit.rs` |
| `Capabilities` | `crates/core/src/capability.rs:114`。必須は `query`・`limits`。`query_ex` は既定の本体を持つ | `grep -nE '^\s*fn \|^pub ' crates/core/src/capability.rs` |
| `BackendKind` | `crates/core/src/capability.rs:32`。値は `Metal`・`Vulkan`・`Cuda`・`Dx12Compute` の 4 つ | `sed -n 28,42p crates/core/src/capability.rs` |
| `LimitsTable` | `crates/core/src/capability.rs:102` | 同上 |

### 試験用の device に当たる型の有無

`git grep -nE 'pub (trait|struct) [A-Za-z]*(Device|Queue|Stub)' -- crates wasm` の出力は 12 件である。

| 型 | 所在 | 分類 |
| -- | ---- | ---- |
| `DeviceRequest`・`NativeDeviceHandle`・`DeviceDiagnostics` | `crates/core/src/device.rs:19`・`:58`・`:88` | device の要求・取り込み・診断の型 |
| `Device`・`DeviceProvider` | `crates/core/src/device.rs:101`・`:151` | device の口の trait |
| `DeviceLostNotify` | `crates/core/src/device_lost.rs:49` | ロストの通知先の trait |
| `CommandQueue` | `crates/core/src/execution/submit.rs:216` | 投入の trait |
| `FaultStub` | `crates/core/src/fault/stub.rs:27` | feature `fault-injection` の裏の部品 |
| Metal の provider 3 件 | `crates/contract/src/backend_metal.rs:1367`・`:1390`・`:1413` | macOS だけ。うち 2 件は feature `fault-injection` の裏 |
| `DeviceFaultHooksGate` | `crates/contract/src/cfg_gate_doctest.rs:70` | feature の有無でコンパイルの成否を切り替える doctest 用の置き場 |

device の trait を実装する、公開の試験用の device の型は、この出力に無い。

名前に `Stub` を含むほかの型は、試験の中の局所の型だけである。確かめ方は `git grep -nE '(struct|enum|trait) [A-Za-z]*Stub' -- crates wasm`。当たる場所は `crates/core/tests/fault_parts_composition.rs:52`・`crates/core/tests/memory_types.rs:10`・`crates/core/tests/callback_send_requirement.rs:17`・`crates/contract/tests/callback_send_requirement.rs:19`（上の `FaultStub` を除く）。

### feature の転送と、本番の組・上の層の検査

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| contract から core への転送 | `test-support = ["fandhe-silicon-core/test-support"]`（`crates/contract/Cargo.toml:45`）。試験は `crates/contract/tests/test_support_forwarding.rs` | `grep -n test-support crates/contract/Cargo.toml` |
| 公開 API の検査の本番の組 | `scripts/public-api-check.sh:86` の `CONTRACT_PRODUCTION`。テスト専用の feature を含まない。core は既定の feature で JSON を作る | `sed -n 86,100p scripts/public-api-check.sh` |
| `default` と本番の組 | テスト・診断専用の feature が入らないことを `crates/contract/tests/feature_default_scope.rs` が確かめる（[contract-feature-check](contract-feature-check.md)） | `cargo test -p fandhe-silicon-contract --test feature_default_scope` |
| 上の層の検査 | `scripts/upper-layer-check.sh:87` の `FORBIDDEN_FEATURES`（`fault-injection`・`test-support`） | `grep -n FORBIDDEN_FEATURES scripts/upper-layer-check.sh` |
| core の feature と contract の転送の照合 | `scripts/core-deps-check.sh` の規則 (B)（`:25`・`:198`） | `grep -n "(B)" scripts/core-deps-check.sh` |
| contract による core の再公開 | 無い | `git grep -n 'pub use fandhe_silicon_core' -- crates`（0 件） |

### test-support の裏の部品と、試験の中のダミーの device の先例

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `capability_rule::CapabilityTable` | `crates/core/src/capability_rule.rs:75`。feature `test-support` の裏 | `grep -n "pub struct CapabilityTable" crates/core/src/capability_rule.rs` |
| `fault::stub::FaultStub` | `crates/core/src/fault/stub.rs:27`。feature `fault-injection` の裏 | `grep -n "pub struct FaultStub" crates/core/src/fault/stub.rs` |
| 試験の中のダミー | `crates/core/tests/device_trait_shape.rs` の `DummyDevice`（`:139`）・`DummyProvider`（`:185`）。`MemoryOps`・`CommandQueue` の実装は cfg の 2 版（`:38`・`:48`・`:59` など）で書かれている | `grep -nE 'Dummy\|cfg' crates/core/tests/device_trait_shape.rs` |

### feature の有無の両側が流れる場所

| 場所 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `make verify` | `--all-features` で clippy と test を流す（doctest を含む） | `scripts/verify.sh` |
| `make build-profiles` | core を feature なしで clippy と test に流す段（`scripts/build-profiles.sh:84`〜`:86`）と、contract の `test-support` 単独の組（`:39`）・`backend-metal,test-support` の組（`:40`） | `sed -n 82,87p scripts/build-profiles.sh` |
| `make wasm-verify` | core の試験の wasm32 の型検査を既定（`scripts/wasm-verify.sh:36`）と `--all-features`（`:39`）で流す | `grep -n "cargo check -p fandhe-silicon-core" scripts/wasm-verify.sh` |
| feature の有無でコンパイルの成否を切り替える doctest の対の先例 | `crates/contract/src/cfg_gate_doctest.rs`（[feature-gate-compile-check](feature-gate-compile-check.md)） | `grep -n "compile_fail" crates/contract/src/cfg_gate_doctest.rs` |

### 関連する文書の記述

[contract-device-stub-check](contract-device-stub-check.md) の 74 行目と 109 行目の表に、「試験用の device の置き場」の行がある。置き場の列は「無い」、前提の列は TASK-126、Issue の列は #1100 である。

## 段の表

TASK-156 は次の 5 つの Issue に分けて進める。

| Issue | 内容 |
| ----- | ---- |
| #7562 | 現状と前提の確認（本書） |
| #7563 | 実装 |
| #7564 | テスト |
| #7565 | 公開 API の検査 |
| #7566 | 確かめ |
