# Metal の背後実装の確保と往復の大きさの試験（TASK-314）の所在と本リポの現状

TASK-314 の作業のうち、確保と往復の大きさの試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-314。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験のファイル | `crates/contract/tests/backend_metal_allocate_sizes.rs` | 同左 |
| `[[test]]` の feature | `backend-metal` と `native-blocking-wait` | `crates/contract/Cargo.toml` |
| cfg | `target_os = "macos"`。macOS 以外では 0 件 | 下の「確かめ方」 |

## 確保と往復の大きさの試験（1/2）

試験は `each_listed_size_allocates_and_round_trips_bytes` の 1 本で、`#[ignore]` が付く。
入口は `MetalDeviceProvider::request`（`Metal`・`ComputeOnly`・`ProductIndependent`）で作った device の `Device::memory()` の `allocate`・`upload`・`download_blocking`・`free`。

入力は定数 `BYTE_LENS`（11 個のバイト数）の各値。用途は `STORAGE | UPLOAD | DOWNLOAD`。

| 値 | バイト数 |
| -- | -------- |
| `37 * 41 * 4` | 6,068 |
| `37 * 53 * 4` | 7,844 |
| `41 * 53 * 4` | 8,692 |
| `64 * 128 * 4` | 32,768 |
| `64 * 256 * 10 * 4` | 655,360 |
| `512 * 512 * 4` | 1,048,576 |
| `20_000 * 64 * 4` | 5,120,000 |
| `20_000 * 128 * 4` | 10,240,000 |
| `2048 * 2048 * 4` | 16,777,216 |
| `100_000 * 64 * 4` | 25,600,000 |
| `100_000 * 128 * 4` | 51,200,000 |

各値について確かめること。

| 順 | 確かめること |
| -- | ------------ |
| 1 | 値が `limits().max_buffer_bytes` 以下（超えれば試験が落ちる） |
| 2 | `allocate` が `Ok` |
| 3 | `handle.size()` が値と同じ |
| 4 | `upload`（先頭から全長）が `Ok` |
| 5 | `download_blocking`（先頭から全長）が `Ok` で、長さが値と同じ、中身が `i % 251 + 1` の並びと同じ |
| 6 | `free` |

macOS 以外ではファイルの全体が 0 件になる。

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait --test backend_metal_allocate_sizes
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait -- -D warnings
```

1 本目は macOS 以外では `running 0 tests`、macOS では `ignored` の件数が 1 になる。device の所在は `docs/metal-device-request.md`（TASK-158）。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 確保と往復の大きさの試験 | #4309 |
