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
| feature | `test-support`・`fault-injection`・`native-blocking-wait`・`ext-interop` の 4 つ（1/5 の時点の値。今は `graphics` を含む 5 つ）。どれも空の一覧（`= []`）で、`default` は無い | `sed -n '/\[features\]/,/^\[\[test\]\]/p' crates/core/Cargo.toml` |
| 外部依存 | 0 件 | `cargo metadata --format-version 1 --no-deps \| jq '.packages[] \| select(.name=="fandhe-silicon-core") \| {features, dependencies}'` と `cargo tree -p fandhe-silicon-core -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]`（`crates/core/src/lib.rs:28`） | `grep -n unsafe_code crates/core/src/lib.rs` |

上の表の `lib.rs:28` は記録時点の値で、今は `lib.rs:38` が `#![forbid(unsafe_code)]` である。
その後 `lib.rs` に行が入り、今は `lib.rs:39` が `#![forbid(unsafe_code)]` である。

### device の口と部品

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `Device` | `crates/core/src/device.rs:101`（1/5 の時点の値。今は `:127`）。親は `Capabilities`。必須の method は `backend`・`ownership`・`generation`・`memory`・`queue`・`diagnostics`。既定の本体を持つ method は `external_interop`・`physical_gpu_id`（feature `ext-interop`）と `fault_hooks`（feature `fault-injection`）。`compute`・`graphics` の method は持たない（1/5 の時点の値）。今の値は、`compute` を既定の本体つきで持ち（試験用の device の答えは [device-compute](device-compute.md)）、`graphics` も feature `graphics` の裏で既定の本体つきで持つ（試験用の device の答えは [graphics-types](graphics-types.md)） | `grep -nE '^\s*fn \|#\[cfg' crates/core/src/device.rs` |
| `DeviceProvider` | `crates/core/src/device.rs:151`（1/5 の時点の値。今は `:192`）。`request(req: DeviceRequest) -> Result<Box<dyn Device>, ContractError>`（`self` を取らない関連関数） | 同上 |
| `MemoryOps` | `crates/core/src/memory/ops.rs:10`。必須は `allocate`・`upload`・`download_async`（native と wasm32 の 2 版）・`free`。`download_blocking` は feature `native-blocking-wait` の裏。`pending_release_count` は既定の本体を持つ | `grep -nE '^\s*fn \|#\[cfg' crates/core/src/memory/ops.rs` |
| `CommandQueue` | `crates/core/src/execution/submit.rs:216`。必須は `create_encoder`・`submit`・`submit_many`・`poll`・`on_complete`（native と wasm32 の 2 版）・`dispatch_ex`。`wait_blocking` は feature `native-blocking-wait` の裏。`poll_ex`・`submit_ex`・`gpu_timing`・`dispatch` は既定の本体を持つ。`submit_exportable` は feature `ext-interop` の裏で既定の本体を持つ | `grep -nE '^\s*fn \|#\[cfg' crates/core/src/execution/submit.rs` |
| `Capabilities` | `crates/core/src/capability.rs:114`。必須は `query`・`limits`。`query_ex` は既定の本体を持つ | `grep -nE '^\s*fn \|^pub ' crates/core/src/capability.rs` |
| `BackendKind` | `crates/core/src/capability.rs:32`。値は `Metal`・`Vulkan`・`Cuda`・`Dx12Compute` の 4 つ | `sed -n 28,42p crates/core/src/capability.rs` |
| `LimitsTable` | `crates/core/src/capability.rs:102` | 同上 |

上の表の `Capabilities`・`BackendKind`・`LimitsTable` の行番号と method の一覧は記録時点の値で、書き換えない。今は `capability.rs` の `:118`・`:36`・`:106` で、`Capabilities` は既定の本体を持つ `texture_format_features` も持つ。試験用の device も全形式で値を返す（`stub_device_returns_value_for_every_key`）。`Device`・`DeviceProvider`・`DeviceDiagnostics` は今は `device.rs` の `:127`・`:192`・`:114`。

補い（TASK-150 の後）: 上の段落の `DeviceProvider` の `:192` は、今は `device.rs` の `:199` である。`Device` の既定の本体を持つ method には `textures()` も加わり（既定の本体は `None`。`device.rs:191`）、`external_interop`・`physical_gpu_id`・`fault_hooks`・`compute`・`graphics` と並ぶ。

### 試験用の device に当たる型の有無

`git grep -nE 'pub (trait|struct) [A-Za-z]*(Device|Queue|Stub)' -- crates wasm` の出力は 12 件である。

| 型 | 所在 | 分類 |
| -- | ---- | ---- |
| `DeviceRequest`・`NativeDeviceHandle`・`DeviceDiagnostics` | `crates/core/src/device.rs:19`・`:58`・`:88`（1/5 の時点の値。今は `:29`・`:68`・`:114`） | device の要求・取り込み・診断の型 |
| `Device`・`DeviceProvider` | `crates/core/src/device.rs:101`・`:151`（1/5 の時点の値。今は `:127`・`:192`） | device の口の trait |
| `DeviceLostNotify` | `crates/core/src/device_lost.rs:49` | ロストの通知先の trait |
| `CommandQueue` | `crates/core/src/execution/submit.rs:216` | 投入の trait |
| `FaultStub` | `crates/core/src/fault/stub.rs:27` | feature `fault-injection` の裏の部品 |
| Metal の provider 3 件 | `crates/contract/src/backend_metal.rs:1367`・`:1390`・`:1413` | macOS だけ。うち 2 件は feature `fault-injection` の裏 |
| `DeviceFaultHooksGate` | `crates/contract/src/cfg_gate_doctest.rs:70`（1/5 の時点の値。今は `:95`） | feature の有無でコンパイルの成否を切り替える doctest 用の置き場 |

device の trait を実装する、公開の試験用の device の型は、この出力に無い（1/5 の時点の値）。今の値は、`StubDevice`・`StubDeviceProvider` が `crates/core/src/stub_device.rs` にある（feature `test-support` の裏）。

名前に `Stub` を含むほかの型は、試験の中の局所の型だけである。確かめ方は `git grep -nE '(struct|enum|trait) [A-Za-z]*Stub' -- crates wasm`。当たる場所は `crates/core/tests/fault_parts_composition.rs:52`・`crates/core/tests/memory_types.rs:10`・`crates/core/tests/callback_send_requirement.rs:17`・`crates/contract/tests/callback_send_requirement.rs:19`（上の `FaultStub` を除く）。

上の当たる場所は 1/5 の時点の値で、書き換えない。今は `crates/core/tests/fault_parts_composition.rs` の局所の型は `FaultTestDevice` で、この確かめ方では当たらない。

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

補い（TASK-150 の後）: 上の表の `device_trait_shape.rs` の行番号は、先頭の `//!` が 1 行増えて 1 ずつずれた。今は `DummyDevice` が `:140`、`DummyProvider` が `:186`、cfg が `:39`・`:49`・`:60` である。

### feature の有無の両側が流れる場所

| 場所 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `make verify` | `--all-features` で clippy と test を流す（doctest を含む） | `scripts/verify.sh` |
| `make build-profiles` | core を feature なしで clippy と test に流す段（`scripts/build-profiles.sh:84`〜`:86`。1/5 の時点の値で、今は `:85`〜`:86`。`test-support` だけの組の clippy と test が `:89`・`:90` に加わった）と、contract の `test-support` 単独の組（`:39`）・`backend-metal,test-support` の組（`:40`） | `sed -n 82,87p scripts/build-profiles.sh` |
| `make wasm-verify` | core の試験の wasm32 の型検査を既定（`scripts/wasm-verify.sh:36`）と `--all-features`（`:39`）で流す | `grep -n "cargo check -p fandhe-silicon-core" scripts/wasm-verify.sh` |
| feature の有無でコンパイルの成否を切り替える doctest の対の先例 | `crates/contract/src/cfg_gate_doctest.rs`（[feature-gate-compile-check](feature-gate-compile-check.md)） | `grep -n "compile_fail" crates/contract/src/cfg_gate_doctest.rs` |

### 関連する文書の記述

[contract-device-stub-check](contract-device-stub-check.md) の「前提の部品と持ち主」の表と「契約テストが要る部品の所在」の表に、「試験用の device の置き場」の行がある。置き場の列は「無い」、前提の列は TASK-126、Issue の列は #1100 である。

## 現状の形（2/5）

### 置き場

`crates/core/src/stub_device.rs`。feature `test-support` の裏にあり、新しい feature・外部依存は足していない（`#![forbid(unsafe_code)]` のまま）。
公開のパスは `fandhe_silicon_core::stub_device::{StubDevice, StubDeviceProvider}`。

### 公開のアイテム

| 項目 | 事実 |
| ---- | ---- |
| `StubDevice` | `StubDevice::new(req: DeviceRequest)` で作る。`Capabilities` と `Device` を実装する。`Send + Sync` を満たす |
| `StubDeviceProvider` | 値を持たない型。`DeviceProvider::request` はどの要求にも `Ok(Box<dyn Device>)` を返す |
| メモリの口・投入の口 | module の中の非公開の型で、`StubDevice::memory()`・`queue()` 越しにだけ届く。今は compute の口（`crates/core/src/stub_device.rs` の非公開の型 `StubCompute`）も同じく module の中の非公開の型で、`StubDevice::compute()` 越しにだけ届く（[device-compute](device-compute.md)）。今は描画の口（非公開の型 `StubGraphics`。feature `graphics` の裏）も同じく、`StubDevice::graphics()` 越しにだけ届く（[graphics-types](graphics-types.md)） |
| feature の裏 | `download_blocking`・`wait_blocking` は feature `native-blocking-wait` の裏。`download_async`・`on_complete` は native と wasm32 の 2 版 |

### 1/5 の表からの変化

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature `test-support` の裏の module | `capability_rule`・`parity`・`test_frame`・`rerun` に `stub_device` が加わった | `grep -n "pub mod" crates/core/src/lib.rs` |
| 型の件数 | `git grep` の出力が 12 件から 14 件になった（`StubDevice`・`StubDeviceProvider`） | `git grep -nE 'pub (trait\|struct) [A-Za-z]*(Device\|Queue\|Stub)' -- crates wasm` |

### 対応するテスト

`crates/core/src/stub_device.rs` の `mod tests`。今は `create_pipeline_reports_exhausted_id_space` も持つ（[device-compute](device-compute.md)）。以下の表は記録時点の値で、書き換えない。

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
| `query_ex_returns_unlisted_value_for_keys_without_rows` | WebGPU の口の 2 つの鍵を除く行の無い鍵（`feature_ids()` の 3 件）で `query_ex` は `UNLISTED_FEATURE_QUERY`（`Unsupported`・理由 `UnknownFeature`・根拠なし） |
| `query_ex_returns_webgpu_port_value_and_passes_shared_check` | 4 つの backend で、2 つの鍵の `query` は `Unsupported`、`query_ex` は `WEBGPU_PORT_UNSUPPORTED`。`check_webgpu_ports` が `Ok`（TASK-156・TASK-155。DEV-35・DEV-36 のポインタ） |
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

## 公開 API の検査（4/5）

### 置き場

`crates/core/src/device.rs` の module doc に、feature `test-support` の有無でコンパイルの成否が切り替わる doctest の対を置いた。公開のアイテムは増えていない。新しいスクリプト・make ターゲット・CI の step も足していない。
snippet は `use fandhe_silicon_core::stub_device::{StubDevice, StubDeviceProvider};` と、両方の型を引数に取る空の関数で、型が解決できるかだけを見る。形の先例は [feature-gate-compile-check](feature-gate-compile-check.md) の対。

### 組ごとの見え方

| 組 | doctest の扱い | 実行の出力 |
| -- | -------------- | ---------- |
| feature なし | `compile_fail`。`stub_device` が解決できず失敗することが期待どおり | `test crates/core/src/device.rs - device (line 13) - compile fail ... ok` |
| `--features test-support` | 通常の doctest。コンパイルして通る | `test crates/core/src/device.rs - device (line 13) ... ok` |
| `--all-features` | 同上 | 同上 |

snippet の import を存在しない名前に変えると有効側の doctest は `FAILED` になることを手元で確かめた（変更は戻してあり、差分に入っていない）。

### 流れる場所

| 側 | 場所 | 確かめた出力 |
| -- | ---- | ------------ |
| feature なし | `make build-profiles` の core を feature なしで試験する段（`scripts/build-profiles.sh:86` の `cargo test -p fandhe-silicon-core`）。CI は `verify` ジョブ（`.github/workflows/ci.yml:180`〜`:181`） | ログに `- compile fail ... ok` の行 |
| 有効 | `make verify` の `--all-features` の test。CI は `verify` ジョブ（同 `:176`〜`:177`） | ログに `... ok` の行 |

`make wasm-verify` の core の型検査は `cargo check --tests` なので、doctest は入らない。

### 確かめ方

- `cargo test -p fandhe-silicon-core --doc`（feature なし。`compile fail ... ok` の行）
- `cargo test -p fandhe-silicon-core --features test-support --doc`（`... ok` の行）
- `cargo test -p fandhe-silicon-core --all-features --doc`（同上）

### rustdoc の JSON での件数

`scripts/public-api-check.sh --emit-json <dir>` で 5 crate × 2 組（`production`・`all-features`）の 10 本を作り（`emitted:` の行が 10 本）、各 JSON の `index` で `crate_id` が 0 のアイテムのうち、`name` または再公開（`inner.use`・`inner.import`）の `name` が `StubDevice`・`StubDeviceProvider`・`stub_device` のものを `python3 -I` の短いスクリプトで名前ごとに数えた。`<dir>` は作業の後に消した。

| crate | `production` | `all-features` |
| ----- | ------------ | -------------- |
| core | 3 つの名前とも 0 件 | 3 つの名前とも 1 件 |
| contract | 3 つの名前とも 0 件 | 3 つの名前とも 0 件 |
| exec | 3 つの名前とも 0 件 | 3 つの名前とも 0 件 |
| upper | 3 つの名前とも 0 件 | 3 つの名前とも 0 件 |
| wasm-abi | 3 つの名前とも 0 件 | 3 つの名前とも 0 件 |

core の `all-features` の 1 件は、数え方が空振りでないことの陽性の対照になる。contract が core を再公開しないことは `git grep -c "pub use fandhe_silicon_core" -- crates` の出力が空（0 件）であることで確かめた。

### 既存の検査の結果

上の「流れる場所」と、この表の `ci.yml` の行番号は記録時点の値で、書き換えない。今の main（`e4386fe`）では `verify` ジョブの `make verify` が `:177`、`make build-profiles` が `:181`、`deny` ジョブの `make deny` が `:246`、`make upper-layer-check` が `:254`、`make wasm-verify` が `:262`、`make public-api-check` が `:303`、`make public-name-check` が `:318`、`make public-api-sdk-check` が `:324`、`make core-deps-check` が `:330`、`make public-api-webgpu-check` が `:336` である（`grep -nE 'run: make ' .github/workflows/ci.yml`）。

| 検査 | 呼ぶ CI のジョブ（`ci.yml`） | 結果 | 確かめたこと |
| ---- | ---------------------------- | ---- | ------------ |
| `make check` | `check` | 終了コード 0 | 整形の規約 |
| `make verify` | `verify`（`:176`） | 終了コード 0 | fmt・clippy・test（doctest を含む） |
| `make build-profiles` | `verify`（`:180`） | 終了コード 0 | feature の組ごとの build・test |
| `make deny` | `deny`（`:245`） | 終了コード 0 | cargo-deny・依存グラフの検査 |
| `make public-api-check` | `deny`（`:302`） | 終了コード 0。`documents=10 public_items_total=10160 wgpu_type_refs_total=0 wgpu_other_refs_total=0` | wgpu 系の参照が増えていない |
| `make public-name-check` | `deny`（`:317`） | 終了コード 0。`documents=10 name_hits_total=0 source_hits_total=0 field_hits_total=0` | `public-name-check.conf` の語に新しい名前が当たらない |
| `make public-api-webgpu-check` | `deny`（`:335`） | 終了コード 0。`blocking_wait_names_total=0` | 新しい公開の method の名前に `wait`・`blocking` の語が無い。この検査が見るのは本番の組から 1 つの feature を外した組で、`test-support` の裏の物は対象に入らない |
| `make public-api-sdk-check` | `deny`（`:323`） | 終了コード 0。`sdk_type_refs_total=0 common_declared_deps=0` | チップの SDK の参照が無い |
| `make core-deps-check` | `deny`（`:329`） | 終了コード 0。`unexpected=0 missing=0 forward_violations=0` | core の外部依存が 0 件で、feature の転送が変わらない |
| `make upper-layer-check` | `deny`（`:252`） | 終了コード 0。`forbidden_features=0` | 上の層が `test-support` を有効にしない |
| `make wasm-verify` | `deny`（`:260`） | 終了コード 0 | wasm32 の型検査 |

`Cargo.lock`・`deps-approved.toml` に差分は無い。

## 完了の条件の確かめ（5/5）

TASK-156（DEV-34）の 5/5（#7566）として、本リポで観測できる事実だけを記録する。GPU の実機のない Linux で流し、件数と出力は実測の値。本体のコードと試験は変えていない。

### 成果物と置き場の対応

| 成果物 | 置き場 |
| ------ | ------ |
| 実装 | `crates/core/src/stub_device.rs`。feature `test-support` の裏（`lib.rs` の `#[cfg(feature = "test-support")] pub mod stub_device;`）。上の「現状の形（2/5）」 |
| 振る舞いの試験 | `crates/core/tests/stub_device_request.rs`・`stub_device_memory.rs`・`stub_device_queue.rs` と `stub_device.rs` の `mod tests`。上の「試験（3/5）」 |
| 公開 API の検査 | `crates/core/src/device.rs` の module doc の doctest の対と、既存の公開 API の検査。上の「公開 API の検査（4/5）」 |

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 実装が feature `test-support` の裏にある | `grep -n 'stub_device' crates/core/src/lib.rs`、`grep -n -A1 'stub_device_' crates/core/Cargo.toml`、feature なしの `cargo test -p fandhe-silicon-core` | `lib.rs` の `pub mod stub_device;` は cfg の直下。3 つの `[[test]]` は `required-features = ["test-support"]` だけを持つ。feature なしの実行に `Running tests/stub_device_` の行は 0 件 |
| 2 | 振る舞いの試験が通る | `cargo test -p fandhe-silicon-core --features test-support --test stub_device_request --test stub_device_memory --test stub_device_queue` と `--lib stub_device` | request 8・memory 11・queue 9・lib 4 がすべて `ok`、failed 0、ignored 0 |
| 3 | `--all-features` でも通る | 同じ組を `--all-features` で流す | request 8・memory 12・queue 10・lib 5 がすべて `ok`。`native-blocking-wait` の裏の分が memory・queue・lib で 1 件ずつ増える |
| 4 | 公開の契約に現れない（feature なし） | `cargo test -p fandhe-silicon-core --doc` | `test crates/core/src/device.rs - device (line 13) - compile fail ... ok` |
| 5 | 同（有効側） | `--features test-support --doc`・`--all-features --doc` | どちらも `test crates/core/src/device.rs - device (line 13) ... ok` |
| 6 | 公開 API の検査 | `make public-api-check` | 終了コード 0。`documents=10 public_items_total=10160 wgpu_type_refs_total=0 wgpu_other_refs_total=0` |
| 7 | 公開 API の名前の検査 | `make public-name-check` | 終了コード 0。`documents=10 name_hits_total=0 source_hits_total=0 field_hits_total=0` |
| 8 | WebGPU の語の検査 | `make public-api-webgpu-check` | 終了コード 0。`documents=2 blocking_wait_names_total=0 webgpu_names_total=0` |
| 9 | チップの SDK の参照 | `make public-api-sdk-check` | 終了コード 0。`sdk_type_refs_total=0 common_declared_deps=0` |
| 10 | core の外部依存と feature の転送 | `make core-deps-check` | 終了コード 0。`unexpected=0 missing=0 forward_violations=0` |
| 11 | 上の層が `test-support` を有効にしない | `make upper-layer-check` | 終了コード 0。`forbidden_features=0` |
| 12 | 背後実装と機材に依らない | `git grep -n 'backend-' -- crates/core/src/stub_device.rs`、`grep -c '#\[ignore'`（4 ファイル） | `backend-` の行は 0 件、`#[ignore` は 4 ファイルとも 0。試験は `crates/core` にあり、背後実装の feature を求めない。2・3 は GPU のない Linux で `ok` |
| 13 | CI で流れる場所 | `.github/workflows/ci.yml` | `verify` ジョブの `make verify`（`:176`〜`:177`）と `make build-profiles`（`:180`〜`:181`） |
| 14 | wasm32 の型検査 | `make wasm-verify` | 終了コード 0 |
| 15 | 外部依存 0 と unsafe | `cargo tree -p fandhe-silicon-core -e normal --depth 1`、`grep -n unsafe_code crates/core/src/lib.rs` | 依存の行は core 自身だけ。`#![forbid(unsafe_code)]` |
| 16 | 依存の記録 | `git diff --stat 56493bb^ origin/main -- Cargo.lock deps-approved.toml`、`make deps-check` | 差分は空。`deps-check` は終了コード 0 |
| 17 | 品質ゲート | `make check`・`make verify`・`make deny`・`make build-profiles` | すべて終了コード 0 |
| 18 | base の CI | `gh run list --branch main --commit 8736023d571a6b2f881e3d10169ba115d3db7e0d --workflow ci.yml` | run 41272447161、status `completed`、conclusion `success` |

### 再実行の契機

- `crates/core/src/stub_device.rs` を変えたとき
- 3 本の試験か `mod tests` を変えたとき
- `crates/core/src/device.rs` の doctest の対を変えたとき
- `Device`・`DeviceProvider`・`MemoryOps`・`CommandQueue`・`Capabilities` に必須の method が増えたとき
- `scripts/public-api-check.sh` などの検査を変えたとき
- `docs/spec` の参照先を進めたとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-156）。確かめた base は `8736023d571a6b2f881e3d10169ba115d3db7e0d`。

### 今の main（`e4386fe`）での流し直し

「再実行の契機」の最初の項目（`stub_device.rs` の変更）に当たる TASK-2956 の 1/3（#7590）の後に、上の表の方法を GPU のない Linux で流し直した。上の表は記録時点の値で、書き換えない。

| 上の # | 今の結果 |
| ------ | -------- |
| 2 | 試験は 4 本になった（`stub_device_compute` が加わった）。`--features test-support` で compute 10・memory 11・queue 9・request 8・lib 5 がすべて `ok`、failed 0、ignored 0 |
| 3 | `--all-features` で compute 10・memory 12・queue 10・request 8・lib 6 がすべて `ok` |
| 4・5 | feature なしの `--doc` は `device.rs - device (line 13) - compile fail ... ok`（21 passed）、`--features test-support --doc` と `--all-features --doc` は `device.rs - device (line 13) ... ok`（26・27 passed） |
| 6〜11 | `make public-api-check` は `public_items_total=10163`（記録時点は 10160）、ほかの `wgpu_*`・`*_hits_total`・`sdk_*`・`unexpected`・`missing`・`forward_violations`・`forbidden_features` は上の表と同じく 0。`make public-name-check`・`make public-api-webgpu-check`・`make public-api-sdk-check`・`make core-deps-check`・`make upper-layer-check` は終了コード 0 |
| 12 | `crates/core/src/stub_device.rs` の `backend-` は 0 件。4 本の試験の `#[ignore` は 0 |
| 14・16 | `make wasm-verify`・`make deps-check` は終了コード 0 |

落ちたものは無い。`make verify`・`make deny`・`make build-profiles` は流していない（CI の `verify`・`deny` ジョブで流れる）。

### main（`f72b280`）での流し直し

「再実行の契機」の最初の項目（`stub_device.rs` の変更）に当たる #7670（WebGPU の口の 2 鍵の値）と #7676（形式の鍵の行）の後に、上の表の方法を GPU のない Linux で流し直した。上の表と直前の流し直しは記録時点の値で、書き換えない。`--lib stub_device` の件数は、名前に `stub_device` を含む試験の数。流したのは `f72b280` に `StubDevice` の rustdoc の文言だけを直した変更（コードと doc テストは変えていない）を足した状態。

| 上の # | 今の結果 |
| ------ | -------- |
| 2 | `--features test-support` で compute 10・memory 11・queue 9・request 9・lib 11 がすべて `ok`、failed 0、ignored 0（各 `cargo test` の終了コード 0）。request は 8 から 9 に増えた（`query_ex` の試験が加わった） |
| 3 | `--all-features` で compute 10・memory 12・queue 10・request 9・lib 14 がすべて `ok`（終了コード 0） |
| 4・5 | feature なしの `--doc` は `device.rs - device (line 13) - compile fail ... ok`（39 passed）、`--features test-support --doc` は `device.rs - device (line 13) ... ok`（51 passed）、`--all-features --doc` も同じ行が `ok`（62 passed）。終了コードはいずれも 0 |
| 6〜11 | `make public-api-check` は `public_items_total=11752`（`--all-features` の値）、`wgpu_type_refs_total=0`・`wgpu_other_refs_total=0`。`make public-name-check` は `name_hits_total=0 source_hits_total=0 field_hits_total=0`、`make public-api-webgpu-check` は `documents=2 blocking_wait_names_total=0`、`make public-api-sdk-check` は `sdk_type_refs_total=0 common_declared_deps=0`、`make core-deps-check` は `unexpected=0 missing=0 forward_violations=0`、`make upper-layer-check` は `forbidden_features=0`。6 本とも終了コード 0 |
| 12 | `crates/core/src/stub_device.rs` の `backend-` は 0 件。4 本の試験の `#[ignore` は 0 |
| 14・16 | `make wasm-verify`・`make deps-check` は終了コード 0 |

落ちたものは無い。`make verify`・`make deny`・`make build-profiles` は流していない（CI の `verify`・`deny` ジョブで流れる）。

## 段の表

TASK-156 は次の 5 つの Issue に分けて進める。

| Issue | 内容 |
| ----- | ---- |
| #7562 | 現状と前提の確認（本書） |
| #7563 | 実装 |
| #7564 | テスト |
| #7565 | 公開 API の検査 |
| #7566 | 確かめ |

- 関連: 描画先の破棄の操作と試験用の device の描画先の表は [texture-view-destroy-copy.md](texture-view-destroy-copy.md)（TASK-372）。
- 関連: 描画パスの状態を設定する操作と、試験用の device のその返しは [render-pass-types.md](render-pass-types.md)（TASK-838）。
- 関連: mesh shader の描画の操作と、試験用の device のその返しは [draw-ops-types.md](draw-ops-types.md)（TASK-839）。

## 今の値の補い（base 6b35dd9）

上の「既存の検査の結果」の表と、その前の `ci.yml` の行番号の補いは記録時点の値で、書き換えない。今の `.github/workflows/ci.yml` の `run:` の行は次のとおり（`grep -nE 'run: make ' .github/workflows/ci.yml`）。

| 検査 | ジョブ | 今の `run:` の行 |
| ---- | ------ | ---------------- |
| `make verify` | `verify` | `:177` |
| `make build-profiles` | `verify` | `:181` |
| `make deny` | `deny` | `:260` |
| `make upper-layer-check` | `deny` | `:268` |
| `make wasm-verify` | `deny` | `:276` |
| `make public-api-check` | `deny` | `:326` |
| `make public-name-check` | `deny` | `:341` |
| `make public-api-sdk-check` | `deny` | `:347` |
| `make core-deps-check` | `deny` | `:353` |
| `make public-api-webgpu-check` | `deny` | `:371` |
