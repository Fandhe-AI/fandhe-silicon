# Vulkan の背後実装の device の取得・診断（TASK-159）の所在と本リポの現状

TASK-159 の作業のうち、Vulkan の背後実装の `DeviceProvider::request` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-159。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_vulkan.rs` | `crates/contract/src/backend_vulkan.rs` |
| 公開の型 | `VulkanDeviceProvider` | `grep -n 'pub ' crates/contract/src/backend_vulkan.rs` |
| cfg | feature `backend-vulkan` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | `crates/contract/tests/backend_vulkan_request.rs`。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-vulkan --test backend_vulkan_request` |

## request の検査（1/5）

`request` が要求を照らす順（device を作る前。最初に当たったものを返す）:

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 背後実装が Vulkan でない | `InvalidCombination` |
| 2 | `CpuOnly`、feature `graphics` の無いビルドの `Graphics` | `InvalidCombination` |
| 3 | `Shared`・`Imported` | `Unsupported { backend: Vulkan, .. }` |

- 背後実装の拒否は、プロファイル・所有形態の拒否より先に見る
- feature `graphics` の無いビルドでは `Graphics` を拒む

## device の取得と診断（2/5）

依存 `ash`（`deps-approved.toml`・approval_ref #422）は wasm32 以外の依存で、`backend-vulkan` が有効にする。
ローダは `Entry::load()` だけで読み込み、探す場所の指定も環境変数の読み取りも受け付けない。

検査を通った要求に対する呼び出しの順と、失敗が返す値（どれも `DeviceUnavailable`。文字列は「呼び出し: エラー」）:

| 順 | 呼び出し | 失敗の文字列の頭 |
| -- | -------- | ---------------- |
| 1 | `Entry::load` | `Entry::load: ` |
| 2 | `create_instance`（API 版 1.0・層なし・拡張なし） | `create_instance: ` |
| 3 | `enumerate_physical_devices` | `enumerate_physical_devices: ` |
| 4 | 先頭の物理 device を選ぶ（空なら失敗） | `enumerate_physical_devices: no physical device` |
| 5 | `get_physical_device_properties`（名前が NUL 終端でなければ失敗） | `get_physical_device_properties: ` |
| 6 | `get_physical_device_queue_family_properties` と家族の選択 | `get_physical_device_queue_family_properties: ` |
| 7 | `get_physical_device_features` | なし |
| 8 | `create_device`（queue 1 本・優先度 1.0・拡張なし） | `create_device: ` |
| 9 | `create_command_pool`（flags なし） | `create_command_pool: ` |

- queue family は `queue_count > 0` の最初の家族で、`ComputeOnly` は `COMPUTE`、`Graphics` は `COMPUTE | GRAPHICS` を含むもの。無ければ失敗
- 作成で有効にする機能は、物理 device が対応するときの `drawIndirectFirstInstance` だけ
- 呼ぶ Vulkan の関数は 1.0 core のものだけ。queue のハンドルは取らない

device の値:

| 口 | 返す値 |
| -- | ------ |
| `backend()` | `BackendKind::Vulkan` |
| `ownership()` | `ProductIndependent` |
| `generation()` | 背後実装の中で 1 から単調に増える値 |
| `diagnostics()` | `backend_name` は `"vulkan"`、`adapter_name` は `<名前> (vulkan api=<major>.<minor>.<patch> driver_version=0x<8 桁の 16 進>)` |
| `query` / `query_ex` | 全部 `Unsupported` / `UNLISTED_FEATURE_QUERY` |
| `limits()` | `max_buffer_bytes` と `max_texture_size` は 0、`max_dispatch_dims` は物理 device の `maxComputeWorkGroupCount` |
| `memory()` | `allocate`・`upload`・`download_blocking` は `Unsupported`、`download_async` は callback を `Unsupported` で 1 回呼ぶ、`free` は何もしない |
| `queue()` | `create_encoder` は識別子を払い出すだけ。`submit`・`submit_many`・`wait_blocking`・`dispatch_ex` は `Unsupported`、`poll` は `Lost`、`on_complete` は callback を `Lost` で 1 回呼ぶ、`max_buffer_bytes` は `limits()` と同じ値 |

解放は持ち主の型の `Drop` が command pool → device → instance の順で行い、最後にローダが落ちる。
`request` の途中で失敗したときも、作り終えた分は同じ `Drop` が同じ順で解放する。

単体試験（`cargo test -p fandhe-silicon-contract --features backend-vulkan --lib backend_vulkan`。device に触らない）:

| 試験 | 確かめる内容 |
| ---- | ------------ |
| `adapter_name_has_the_documented_shape` | `adapter_name` の形 |
| `first_physical_device_*` | 空は `DeviceUnavailable`、あれば先頭 |
| `select_queue_family_*` | プロファイルごとの選択、`queue_count == 0` の飛ばし、見つからないときの `DeviceUnavailable` |
| `enabled_features_follow_the_physical_support` | 有効にする機能が物理 device の対応に従い、ほかの欄は立たない |
| `vk_failure_puts_the_call_first` | 失敗の文字列が呼び出し名で始まる |
| `release_in_order_releases_children_first` | 解放の順 |
| `absent_loader_is_device_unavailable` | ローダが読めなければ `Entry::load: ` で始まる `DeviceUnavailable`（読めれば `not-run` を出して戻る） |

## 初期化の失敗の差し込み（3/5）

feature `fault-injection` を有効にした `backend-vulkan` のビルド（wasm32 以外）には、初期化の失敗を差し込む 7 個の
公開の provider がある。どれも `request` で要求の検査（上の表と同じ順）を通した後に、固定の失敗を返す。
device は作らず、ローダにも物理 device にも触れない。返すのは `DeviceUnavailable` で、
`failure_class()` は `FailureClass::Initialization`、文字列の末尾は `(injected)`。

| 型 | 返す値の文字列の頭 |
| -- | ------------------ |
| `VulkanLoaderMissingProvider` | `Entry::load: ` |
| `VulkanCreateInstanceFailsProvider` | `create_instance: ` |
| `VulkanEnumeratePhysicalDevicesFailsProvider` | `enumerate_physical_devices: ` |
| `VulkanNoPhysicalDeviceProvider` | `enumerate_physical_devices: no physical device` |
| `VulkanNoQueueFamilyProvider` | `get_physical_device_queue_family_properties: ` |
| `VulkanCreateDeviceFailsProvider` | `create_device: ` |
| `VulkanCreateCommandPoolFailsProvider` | `create_command_pool: ` |

試験（`cargo test -p fandhe-silicon-contract --features backend-vulkan,fault-injection --test backend_vulkan_fault`。device に触らない）:

| 試験 | 確かめる内容 |
| ---- | ------------ |
| `injected_initialization_failures_are_device_unavailable` | 7 個とも `DeviceUnavailable` で、文字列に `injected` を含み、`Initialization` |
| `an_injected_failure_is_distinct_from_the_other_kinds` | 差し込みの値が `Timeout`・`CapabilityInsufficient` と値も種類も違い、`RuntimeError`・`Lost` の完了状態の種類とも違う |
| `injected_providers_check_the_request_first` | Vulkan でない背後実装と `CpuOnly` は 7 個とも `InvalidCombination`（差し込みより先に検査が走る） |

## device の要る試験（4/5）

置き場は `crates/contract/tests/backend_vulkan_device.rs`。feature `backend-vulkan` のビルド（wasm32 以外）にあり、
どの試験も `#[ignore = "requires a Vulkan device"]` を付けていて、`--ignored` を付けたときだけ走る。

| 試験 | 確かめること |
| ---- | ------------ |
| `compute_only_request_returns_a_vulkan_device` | `ComputeOnly` の `request` が `Ok`。`backend()` が `Vulkan`、`ownership()` が `ProductIndependent`、`diagnostics().backend_name` が `vulkan`、`adapter_name` が ` (vulkan api=`（前が空でない）と ` driver_version=0x` を含む（値は固定しない）。`adapter_name=<値>` の 1 行を標準出力に出す |
| `two_requests_take_increasing_generations` | 2 回の `request` が `Ok` で、2 つ目の `generation()` が 1 つ目より大きい |
| `query_ex_has_no_rows` | `FEATURE_SHADER_F16` の `query_ex` が `UNLISTED_FEATURE_QUERY`、`query` が `Unsupported` |
| `memory_and_queue_operations_are_unsupported` | `memory().allocate` と `queue().submit` が `Unsupported`（背後実装は `Vulkan`）。`create_encoder` は panic しない。`queue().max_buffer_bytes()` が `limits().max_buffer_bytes` と等しい |
| `graphics_request_returns_a_device_with_feature_graphics` | feature `graphics` のビルドにだけある。`Graphics` の `request` が `Ok` で、`backend()` が `Vulkan`、`graphics()` が `None` |
| `dropping_a_device_does_not_panic` | `request` → `drop` を 2 回繰り返して panic しない |

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-vulkan --test backend_vulkan_device` の `test result:` の行は
`0 passed; 0 failed; 5 ignored`、`--features backend-vulkan,graphics` では `6 ignored`。

「所在」の表の試験の行は 1/5 の時点の値で、今は `crates/contract/tests/backend_vulkan_device.rs` も加わっている。

## 完了の条件の確かめ（5/5）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-159。ポインタのみ）。

時点のコミット: `33018d16`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 検査で弾く入力 | `cargo test -p fandhe-silicon-contract --features backend-vulkan --test backend_vulkan_request` | 5 passed、終了コード 0 |
| 2 | 単体 | `cargo test -p fandhe-silicon-contract --features backend-vulkan --lib backend_vulkan` | 11 passed、終了コード 0。`-- --nocapture` では `absent_loader_is_device_unavailable` が `not-run: Vulkan loader is present` を出力 |
| 3 | 差し込み | `cargo test -p fandhe-silicon-contract --features backend-vulkan,fault-injection --test backend_vulkan_fault` | 3 passed、終了コード 0 |
| 4 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-vulkan,graphics --test backend_vulkan_device` | 0 passed; 0 failed; 6 ignored、終了コード 0 |
| 5 | 個数テスト | `cargo test -p fandhe-silicon-contract --test backend_vulkan_unsafe_count` | 1 passed、終了コード 0 |
| 6 | 依存 | `make deps-check`、`make deny`、`grep -c '^\[\[approval\]\]' deps-approved.toml` | `make deps-check` と `make deny` は終了コード 0。`[[approval]]` は 6 件 |
| 7 | 公開 API | `make public-api-sdk-check` | `sdk_type_refs_total=0`、終了コード 0 |
| 8 | wasm32 | `make wasm-backend-check` | `name=ash wasm32_n=0 native_n=1 result=match`（`backend-vulkan`）、`ok: all 3 configurations match`、終了コード 0 |
| 9 | CI | `gh pr checks 8277`・`8280`・`8282`・`8283` | 4 つの PR とも `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `pass` |

(2) の出力（`test result:` の行）。

```text
running 11 tests
not-run: Vulkan loader is present
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.01s
```

(4) の出力。

```text
running 6 tests
test result: ok. 0 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(8) の出力。

```text
deps: feature=backend-metal name=ash wasm32_n=0 native_n=0 result=match
deps: feature=backend-vulkan name=ash wasm32_n=0 native_n=1 result=match
deps: feature=backend-cuda name=ash wasm32_n=0 native_n=0 result=match
ok: all 3 configurations match
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | request の検査と provider の入口 | #3710 |
| 2/5 | ash を足して device の取得と診断を実装する | #3711 |
| 3/5 | 初期化の失敗の差し込みと失敗の種類の試験 | #3712 |
| 4/5 | device の要る試験を ignore 付きで足す | #3713 |
| 5/5 | 完了の条件の確かめ | #3714 |
