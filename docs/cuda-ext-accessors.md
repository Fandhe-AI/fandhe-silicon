# CUDA の背後実装の拡張アクセサ（TASK-197）の所在と本リポの現状

TASK-197 の作業のうち、CUDA の背後実装の device が返す物理 GPU の識別子の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-197。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `grep -n 'physical_gpu_id\|read_physical_gpu_id\|uuid_bytes' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")`。その中で feature `ext-interop`・`fault-injection` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda,ext-interop --lib backend_cuda` |

## 物理 GPU の識別子（1/4）

feature `ext-interop` のビルドでは、`request`（device の作成）の中で CUDA context の UUID を 1 回だけ読み、device の欄に持つ。
`Device::physical_gpu_id()` は持っている値を返すだけで、ドライバを呼び直さない。

| 入力 | 返す値 |
| ---- | ------ |
| UUID が読めた | `physical_gpu_id()` が `Some(PhysicalGpuId { device_uuid, driver_uuid: None })`。`device_uuid` は 16 個の `c_char` をビットを変えずに写したバイト列 |
| UUID が読めない | `request` が `DeviceUnavailable` を返す（device を返さない） |
| `ext-interop` の無いビルド | UUID を読まない |

対応する単体試験（どちらも feature `ext-interop`、device に触らない）:

- `uuid_bytes_keep_every_bit`: 高い位のビットを含む 16 バイトが変わらずに写る
- `uuid_read_maps_to_an_id_without_driver_uuid_or_to_device_unavailable`: 読めた値が `driver_uuid` なしの識別子になり、読めない閉包が `DeviceUnavailable` になる

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 物理 GPU の識別子を device の作成で読んで返す | #3884 |
