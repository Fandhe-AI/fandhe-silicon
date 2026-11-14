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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | request の検査と provider の入口 | #3710 |
| 2/5 | ash を足して device の取得と診断を実装する | #3711 |
