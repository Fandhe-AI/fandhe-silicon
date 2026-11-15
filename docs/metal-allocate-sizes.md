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

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-314。ポインタのみ）。

時点のコミット: `9a1e3ac8`。`docs/spec` の gitlink は `e380dd445150e39139f92f6fc0ce05484d49ac0b`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `grep -c '#\[test\]'`、`grep -c '^[[:space:]]*#\[ignore'`（どちらも `crates/contract/tests/backend_metal_allocate_sizes.rs`） | `#[test]` は 1、`#[ignore` は 1 |
| 2 | 列の値 | `grep -A12 'const BYTE_LENS' crates/contract/tests/backend_metal_allocate_sizes.rs` | 下の (2) の出力 |
| 3 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait -- -D warnings` | 終了コード 0 |
| 4 | CI の macOS のビルド | `gh run view 43675781595 --log --job 132695505630`（`make verify + build-profiles (macos-latest)`）の `backend_metal_allocate_sizes` の部分。run は `9a1e3ac8` の main の push | `test result: ok. 0 passed; 0 failed; 1 ignored`（`make verify` と `make build-profiles` の 2 か所）、run の conclusion `success`、headSha `9a1e3ac8ae5783cdafcc4d932705d87ec0d0d392` |
| 5 | 依存 0・unsafe 0 | `git diff --stat 82160048..HEAD -- Cargo.lock deps-approved.toml crates/core crates/contract/src`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 6 | CI | `gh run view 43675781595 --json conclusion,headSha,jobs` | `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`、headSha `9a1e3ac8ae5783cdafcc4d932705d87ec0d0d392` |

(5) の `82160048` は、1/2 の 1 つ前のコミット。

(2) の出力。

```rust
const BYTE_LENS: [u64; 11] = [
    37 * 41 * 4,
    37 * 53 * 4,
    41 * 53 * 4,
    64 * 128 * 4,
    64 * 256 * 10 * 4,
    512 * 512 * 4,
    20_000 * 64 * 4,
    20_000 * 128 * 4,
    2048 * 2048 * 4,
    100_000 * 64 * 4,
    100_000 * 128 * 4,
];
```

(4) の出力。

```text
Running tests/backend_metal_allocate_sizes.rs (target/debug/build/fandhe-silicon-contract/f5da32b148bd74a5/out/backend_metal_allocate_sizes-f5da32b148bd74a5)

running 1 test
test each_listed_size_allocates_and_round_trips_bytes ... ignored, touches the Metal device; run only in a dedicated device session

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 確保と往復の大きさの試験 | #4309 |
| 2/2 | 完了の条件の確かめ | #4310 |
