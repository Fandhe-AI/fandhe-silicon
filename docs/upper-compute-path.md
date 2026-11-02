# 上の層の device・queue・記録先の所在（TASK-2386）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2386。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/upper/src/device.rs`・`crates/upper/src/encoder.rs` |
| 公開のパス | `fandhe_silicon_upper::{UpperDevice, Queue, CommandEncoder, CommandBuffer, SubmitId}` |
| feature の条件 | `Queue::wait_blocking` だけ `native-blocking-wait` の裏 |

## device・queue・記録先・投入の印の型（1/5）

### 本リポの現状の事実

| 項目 | 内容 |
| ---- | ---- |
| upper の依存 | `fandhe-silicon-core`・`fandhe-silicon-exec` の path のみ（外部依存なし） |
| unsafe | `#![forbid(unsafe_code)]` |
| exec の項目 | 使わない |
| module | 非公開。crate の根から `pub use` |

### 型ごとの動き

| 型 | 持つもの | 操作 | 呼ぶ契約の操作と回数 | 返す値 |
| -- | -------- | ---- | -------------------- | ------ |
| `UpperDevice<'d>` | `&'d dyn Device` | `new`・`queue` | なし | `queue` は `Queue<'d>`（失敗しない） |
| `Queue<'d>` | `&'d dyn Device` | `create_command_encoder` | `create_encoder` を 1 回 | `CommandEncoder<'d>` |
| 同上 | 同上 | `submit` | `submit(encoder, &[])` を 1 回 | `Result<SubmitId, ContractError>`（`Err` は値のまま） |
| 同上 | 同上 | `wait_blocking`（feature の裏） | `wait_blocking(token, timeout)` を 1 回 | `Result<(), ContractError>`（値のまま） |
| 同上 | 同上 | `gpu_timing` | `gpu_timing(token)` を 1 回 | `GpuTiming`（値のまま） |
| `CommandEncoder<'d>` | 契約の `CommandEncoder` | `finish` | なし（値を移すだけ・失敗しない） | `CommandBuffer<'d>` |
| `CommandBuffer<'d>` | 契約の `CommandEncoder` | 公開の操作なし | なし | なし |
| `SubmitId` | 契約の `SubmitToken`（非公開） | 取り出す口・作る口なし | なし | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` |

`Queue` は契約の queue を保持せず、操作のたびに `device.queue()` で得る。
コンパイルで拒否されるコードは doctest で確かめる。`finish` の後の再使用（E0382）、`finish` 前の `submit`（E0308）、投入済みの `CommandBuffer` の再投入（E0382）。

### feature

| feature | 転送先 | 無効のビルド |
| ------- | ------ | ------------ |
| `native-blocking-wait` | `fandhe-silicon-core/native-blocking-wait`・`fandhe-silicon-exec/native-blocking-wait` | `Queue::wait_blocking` が無い（E0599。doctest で確かめる）。既定では無効 |

### 確かめ方

```text
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
cargo test -p fandhe-silicon-upper
cargo test -p fandhe-silicon-upper --features native-blocking-wait
cargo check -p fandhe-silicon-upper --target wasm32-unknown-unknown
RUSTC_BOOTSTRAP=1 cargo test -p fandhe-silicon-upper --doc
```

## compute のパスの開始と終了（2/5）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/upper/src/pass.rs`・`crates/upper/src/encoder.rs` |
| 公開のパス | `fandhe_silicon_upper::ComputePass` |

| 型 | 操作 | 呼ぶ契約の操作 | 返す値 |
| -- | ---- | -------------- | ------ |
| `CommandEncoder<'d>` | `begin_compute_pass` | なし（値を移すだけ・失敗しない） | `ComputePass<'d>` |
| `ComputePass<'d>` | `end` | なし（値を移すだけ・失敗しない） | `CommandEncoder<'d>` |

`ComputePass` が持つのは契約の `CommandEncoder` だけ。

コンパイルで拒否されるコードは doctest で確かめる。パスを始めた後の元の記録先の `finish`（E0382）。
通る例は `begin_compute_pass().end().finish()` の後の `submit`。

### 確かめ方

```text
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
cargo test -p fandhe-silicon-upper
cargo test -p fandhe-silicon-upper --features native-blocking-wait
cargo check -p fandhe-silicon-upper --target wasm32-unknown-unknown
RUSTC_BOOTSTRAP=1 cargo test -p fandhe-silicon-upper --doc
```

## バッファの包み（3/5）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/upper/src/buffer.rs` |
| 公開のパス | `fandhe_silicon_upper::Buffer` |

| 操作 | 受けるもの | 呼ぶ契約の操作 | 返す値 |
| ---- | ---------- | -------------- | ------ |
| `from_handle` | `BufferHandle` | なし（値を移すだけ・失敗しない） | `Buffer` |
| `handle` | `&Buffer` | なし | `&BufferHandle` |
| `handle_mut` | `&mut Buffer` | なし | `&mut BufferHandle` |
| `into_handle` | `Buffer` | なし（値を移すだけ・失敗しない） | `BufferHandle` |

`Buffer` が持つのは契約の `BufferHandle` だけ。`Clone`・`Copy`・`Drop` を持たない。
大きさと用途は `handle().size()`・`handle().usage()` で読む。

コンパイルで拒否されるコードは doctest で確かめる。複製（E0599）、`into_handle` の 2 回目の呼び出し（E0382）。
通る例は `from_handle` の後に `handle`・`handle_mut` を借りて `into_handle` で返すもの。

### 確かめ方

```text
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
cargo test -p fandhe-silicon-upper
cargo test -p fandhe-silicon-upper --features native-blocking-wait
cargo check -p fandhe-silicon-upper --target wasm32-unknown-unknown
RUSTC_BOOTSTRAP=1 cargo test -p fandhe-silicon-upper --doc
```
