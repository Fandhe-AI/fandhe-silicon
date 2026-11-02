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

## 試験用の device での試験（4/5）

| 項目 | 内容 |
| ---- | ---- |
| 試験 | `crates/upper/tests/upper_compute_path.rs`・`crates/upper/tests/upper_wait_blocking.rs` |
| 試験用の device | `crates/upper/tests/common/mod.rs`（core の `StubDevice` を包み、契約の queue の操作の呼び出しを順に記録する。戻り値の差し替えは `Overrides`） |
| 組む条件 | `upper_compute_path` は feature の条件なし。`upper_wait_blocking` は feature `native-blocking-wait` の下だけ |
| dev の依存 | `fandhe-silicon-core`（feature `test-support`）。製品の依存は変えない |

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `encoder_pass_and_finish_call_no_queue_operation` | 記録先の作成、パスの開始と終了、`finish` | その間の契約の queue の呼び出しは `create_encoder` の 1 件だけ。続く `submit` で 1 件増える |
| `submit_passes_empty_wait_and_wraps_the_token` | `submit` の後に `gpu_timing` | `submit` が 1 回、待ちは空。`gpu_timing` が受ける印は `submit` が返した印。`SubmitId` と `SubmitToken` は同じ大きさ |
| `submit_error_is_returned_as_is` | 契約の `submit` を `Err` にする | 上の層の `submit` が同じ `Err` を返す |
| `gpu_timing_only_reads_and_returns_as_is` | `Pending`・`Reclaimed`・`Available` の差し替えと差し替えなし | 契約の `gpu_timing` が 1 回だけ呼ばれ、`poll` などは呼ばれない。値はそのまま返り、差し替えなしは `Unavailable` |
| `wait_blocking_passes_token_and_timeout_as_is` | `None` と `Some(5ms)` の待ち、`Err` の差し替え | 契約の `wait_blocking` が印と上限を値のまま受け、結果を値のまま返す |
| `device_and_queue_hold_no_state` | 2 つの device と、同じ `UpperDevice` からの 2 つの `Queue` | `UpperDevice`・`Queue` の大きさは `&dyn Device` と等しい。投入は対応する device の記録にだけ出る |
| `buffer_wraps_the_handle` | 確保した handle を `Buffer` に包んで戻す | 大きさと用途が保たれ、戻した handle で `upload` が通る |
| `checks_compose_with_test_frame_judge` | 投入と時刻の写しの確かめを試験の枠へ通す | 設定ありで `Ok`、壊した入力で `Ng`、設定なしで `NotRun` |

### 確かめ方

```text
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
cargo test -p fandhe-silicon-upper --test upper_compute_path
cargo test -p fandhe-silicon-upper --features native-blocking-wait --test upper_wait_blocking
cargo test -p fandhe-silicon-upper --features native-blocking-wait
cargo check -p fandhe-silicon-upper --target wasm32-unknown-unknown --tests
```
