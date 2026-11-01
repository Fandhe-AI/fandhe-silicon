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

この節は 1/5 の時点の事実である。2/5 以降の変化は各段の節に書く。

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

## 現状の形（2/5）

### 置き場

`crates/core/src/stub_device.rs`。feature `test-support` の裏にあり、新しい feature・外部依存は足していない（`#![forbid(unsafe_code)]` のまま）。
公開のパスは `fandhe_silicon_core::stub_device::{StubDevice, StubDeviceProvider}`。

### 公開のアイテム

| 項目 | 事実 |
| ---- | ---- |
| `StubDevice` | `StubDevice::new(req: DeviceRequest)` で作る。`Capabilities` と `Device` を実装する。`Send + Sync` を満たす |
| `StubDeviceProvider` | 値を持たない型。`DeviceProvider::request` はどの要求にも `Ok(Box<dyn Device>)` を返す |
| メモリの口・投入の口 | module の中の非公開の型で、`StubDevice::memory()`・`queue()` 越しにだけ届く |
| feature の裏 | `download_blocking`・`wait_blocking` は feature `native-blocking-wait` の裏。`download_async`・`on_complete` は native と wasm32 の 2 版 |

### 1/5 の表からの変化

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature `test-support` の裏の module | `capability_rule`・`parity`・`test_frame`・`rerun` に `stub_device` が加わった | `grep -n "pub mod" crates/core/src/lib.rs` |
| 型の件数 | `git grep` の出力が 12 件から 14 件になった（`StubDevice`・`StubDeviceProvider`） | `git grep -nE 'pub (trait\|struct) [A-Za-z]*(Device\|Queue\|Stub)' -- crates wasm` |

### 対応するテスト

`crates/core/src/stub_device.rs` の `mod tests`。

| テスト | 確かめること |
| ------ | ------------ |
| `stub_device_request_returns_ok_and_echoes_backend_and_ownership` | `request` が `Ok` で、`backend()` と `ownership()` が要求の値 |
| `stub_device_round_trip_allocate_upload_download` | `allocate`・`upload`・`download_async` で同じバイト列が callback に 1 回届く |
| `stub_device_is_send_and_sync` | `StubDevice` が `Send + Sync` |
| `submit_many_rejects_out_of_range_sequence_without_claiming` | `submit_many` の `after` に範囲外の番号の印を渡すと `InvalidArgument` で、記録先は投入されないまま残り、新しい記録先で投入できる |
| `wait_blocking_errors_match_poll_lost_tokens` | feature `native-blocking-wait` の裏。`poll` が `Lost` を返す印は `wait_blocking` で `InvalidArgument`・`DeviceLost`・`DeviceMismatch` に分かれる |

### 確かめ方

`cargo test -p fandhe-silicon-core --features test-support --lib stub_device`

## 試験（3/5）

### 置き場

`crates/core/tests/` の 3 ファイル。`crates/core/Cargo.toml` の `[[test]]` で `required-features = ["test-support"]` にしてあり、feature なしでは走らない。公開のパスだけを使う。

### 対応するテスト

`stub_device_request.rs`（要求・能力問合せ・診断）。

| テスト | 確かめること |
| ------ | ------------ |
| `request_succeeds_for_every_backend_profile_ownership` | 4 つの backend・3 つのプロファイル・3 つの所有形態の 36 組すべてで `request` が `Ok`。`backend()` と `ownership()` が要求のとおりで、`Imported` は包んだ値を取り出せる |
| `stub_device_new_matches_provider` | `StubDevice::new` でも `backend()`・`ownership()` が要求の値 |
| `generation_is_positive_and_increases` | `generation()` は 1 以上で、後に作った device のほうが大きい |
| `diagnostics_are_fixed_strings` | `diagnostics()` は全 backend で `adapter_name`・`backend_name` とも `"stub"` |
| `query_is_unsupported_for_any_feature` | どの `FeatureId` でも `query` は `Unsupported` |
| `query_ex_uses_default_body` | `query_ex` は `Unsupported`・理由なし・根拠なし |
| `limits_table_is_fixed` | `max_buffer_bytes` が `1 << 28`、dispatch の各次元とテクスチャの大きさが 0 |
| `provider_returns_boxed_device_and_stub_is_send_sync` | `request` の戻りの型が `Result<Box<dyn Device>, ContractError>` で、`StubDevice` が `Send + Sync` |

`stub_device_memory.rs`（メモリの口）。

| テスト | 確かめること |
| ------ | ------------ |
| `allocate_returns_zeroed_buffer_with_requested_size_and_usage` | 確保の直後は全バイトが 0 で、`size()`・`usage()` が確保の値 |
| `upload_then_download_round_trips` | 書き込んだ範囲が読み出しで返る |
| `download_async_calls_callback_once` | 成功でも失敗でも callback が 1 回 |
| `allocation_over_limit_is_rejected` | 上限を 1 超えると `InvalidArgument`、上限ちょうどは `Ok` |
| `range_end_equal_to_length_is_accepted` | 終端が確保長ちょうどの範囲（`offset` が確保長で長さ 0 を含む）は通る |
| `range_past_end_or_overflow_is_rejected_without_change` | 終端超過・オーバーフローは `InvalidArgument` で、拒否の後も内容は変わらない |
| `handle_from_other_device_is_device_mismatch` | 別 device のハンドルは `DeviceMismatch` |
| `forged_handles_are_invalid_argument` | 表に無い id・世代違い・長さ違いのハンドルは `InvalidArgument` |
| `freed_handle_is_invalid_argument` | `free` 済みのハンドルは `InvalidArgument` |
| `free_ignores_foreign_and_unknown_handles` | 別 device のハンドルや表に無い id を `free` に渡しても、持ち主の元のハンドルは使える |
| `pending_release_count_is_none` | `pending_release_count()` は `None` |
| `download_blocking_matches_download_async` | feature `native-blocking-wait` の裏。成功と拒否の値が `download_async` と同じ |

`stub_device_queue.rs`（投入の口）。

| テスト | 確かめること |
| ------ | ------------ |
| `submit_then_poll_is_complete` | `submit` が `Ok` で、印の `poll` は `Complete`。印の `generation` は device の `generation()` と等しい |
| `on_complete_reports_complete_once` | 払い出した印では `Complete`、別 device の印では `Lost` が、どちらも callback に 1 回 |
| `submit_many_empty_is_invalid_argument` | 記録先が空の `submit_many` は `InvalidArgument` |
| `submit_with_wait_is_unsupported` | `wait` が空でない `submit`・`submit_many` は、要求の backend を持つ `Unsupported` |
| `submit_rejects_foreign_submitted_and_zero_encoders` | 別 device・投入済み・id 0 の記録先は `InvalidArgument`。拒否の後も新しい記録先で投入できる |
| `dropped_encoder_does_not_block_next_submit` | 投入せずに破棄した記録先は、次の投入を妨げない |
| `submit_many_with_foreign_after_is_device_mismatch` | `after` に別 device の印があると `DeviceMismatch` |
| `poll_reports_lost_for_foreign_or_unissued_tokens` | 別 device の印・番号 0・未払い出しの番号・世代違いの印は `Lost` |
| `dispatch_ex_and_dispatch_are_unsupported` | 4 つの backend それぞれで `dispatch_ex`・`dispatch` が `Unsupported { op: "dispatch_ex", backend }` |
| `wait_blocking_returns_ok_for_issued_token` | feature `native-blocking-wait` の裏。払い出した印で `Ok(())` |

### 確かめ方

- `cargo test -p fandhe-silicon-core --features test-support --test stub_device_request --test stub_device_memory --test stub_device_queue`
- `cargo test -p fandhe-silicon-core --all-features --test stub_device_request --test stub_device_memory --test stub_device_queue`（`native-blocking-wait` の裏の 2 件も走る）
- feature なしの `cargo test -p fandhe-silicon-core` では、この 3 ファイルは `required-features` により走らない

### 流れる場所

- `make verify`（`--all-features` の test）
- `make wasm-verify`（`--all-features` の wasm32 の型検査に 3 ファイルが入る）

## 段の表

TASK-156 は次の 5 つの Issue に分けて進める。

| Issue | 内容 |
| ----- | ---- |
| #7562 | 現状と前提の確認（本書） |
| #7563 | 実装 |
| #7564 | テスト |
| #7565 | 公開 API の検査 |
| #7566 | 確かめ |
