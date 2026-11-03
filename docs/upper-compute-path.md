# 上の層の device・queue・記録先の所在（TASK-2386）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2386。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/upper/src/device.rs`・`crates/upper/src/encoder.rs` |
| 公開のパス | `fandhe_silicon_upper::{UpperDevice, Queue, CommandEncoder, CommandBuffer, SubmitId}` |
| feature の条件 | `Queue::wait_blocking` だけ `native-blocking-wait` の裏 |

補い: 上の表は 1/5 の時点の値で、書き換えていない。2/5 以降で次を足した。

| 段 | 足した実装・試験 | 公開のパス |
| -- | ---------------- | ---------- |
| 2/5 | `crates/upper/src/pass.rs`、`crates/upper/src/encoder.rs` の `begin_compute_pass` | `ComputePass` |
| 3/5 | `crates/upper/src/buffer.rs` | `Buffer` |
| 4/5 | `crates/upper/tests/upper_compute_path.rs`・`crates/upper/tests/upper_wait_blocking.rs`・`crates/upper/tests/common/mod.rs` | なし |
| 5/5 | なし（確かめの記録） | なし |

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

## 完了の条件の確かめ（5/5）

5/5（#6900）で、上の層の crate（`crates/upper`）について本リポで観測できる事実だけを記録する。対応する作業は `docs/spec/05-tasks.md` の TASK-2386（ポインタのみ）。GPU の実機の無い Linux で実行した。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 公開の型が上の層の crate の根にある | `grep -n 'pub use' crates/upper/src/lib.rs`、`cargo check -p fandhe-silicon-upper`（feature なし） | `lib.rs` の 15〜18 行に `Buffer`・`Queue`・`UpperDevice`・`CommandBuffer`・`CommandEncoder`・`SubmitId`・`ComputePass` の `pub use`。`cargo check` は終了コード 0 |
| (2) | `finish`・`end` が契約の queue の操作を呼ばない | `cargo test -p fandhe-silicon-upper --test upper_compute_path` の `encoder_pass_and_finish_call_no_queue_operation` | `ok`（7 passed; 0 failed; 0 ignored） |
| (3) | 投入・待ち・時刻の写し | 同 `--test upper_compute_path` の `submit_passes_empty_wait_and_wraps_the_token`・`submit_error_is_returned_as_is`・`gpu_timing_only_reads_and_returns_as_is`、`cargo test -p fandhe-silicon-upper --features native-blocking-wait --test upper_wait_blocking` の `wait_blocking_passes_token_and_timeout_as_is` | 4 本とも `ok`（後者の実行は 1 passed; 0 failed; 0 ignored） |
| (4) | `Buffer` の口と、複製・二重の取り出しのコンパイルでの拒否 | `grep -n 'pub fn' crates/upper/src/buffer.rs`、`RUSTC_BOOTSTRAP=1 cargo test -p fandhe-silicon-upper --doc`、`make doctest-error-code-check` | `pub fn` は `from_handle`・`handle`・`handle_mut`・`into_handle` の 4 つ（54・59・64・69 行）。`Buffer` の doctest（line 11・23 が compile fail、line 36 が compile）は `ok`。`make doctest-error-code-check` は `doctest error codes ok`、終了コード 0 |
| (5) | 記録先・投入前の値・パスの型のコンパイルでの拒否 | 同じ `--doc` の `CommandEncoder`（line 16・30）・`CommandBuffer`（line 74）・`ComputePass`（line 16）・`Queue`（line 48。feature の門）の行 | feature なしは `3 passed` と `7 passed`（`Queue` は compile fail）、`--features native-blocking-wait` は `4 passed` と `6 passed`（`Queue` は通る例）。いずれも 0 failed |
| (6) | device・queue が状態を持たない | `--test upper_compute_path` の `device_and_queue_hold_no_state`、`grep -rnE 'static\|thread_local\|Mutex\|RefCell\|Cell<\|Arc<' crates/upper/src` | `device_and_queue_hold_no_state` は `ok`。grep は 0 件 |
| (7) | 依存 0・unsafe 0 | 4 コミット（`d909cc7`・`ac32709`・`d87360e`・`734d544`）それぞれの `git show --stat --format= <sha> -- Cargo.lock deps-approved.toml Cargo.toml`、`make upper-layer-check`、`make deps-check`、`grep -n forbid crates/upper/src/lib.rs` | 4 コミットとも差分 0 件。`record:` の行は `rs_files=8 direct_deps=3 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0`、`ok:` で終了。`make deps-check` は `ok`、終了コード 0。`lib.rs` の 8 行に `#![forbid(unsafe_code)]` |
| (8) | 流れる場所と feature の組 | `make verify`、`cargo clippy -p fandhe-silicon-upper --all-targets [--features native-blocking-wait] -- -D warnings`、`cargo test -p fandhe-silicon-upper [--features native-blocking-wait]`、`cargo check -p fandhe-silicon-upper --target wasm32-unknown-unknown`（`--tests`、`--tests --features native-blocking-wait` も） | `make verify` は終了コード 0 で、upper は統合試験 7 件と 1 件、doctest 4 件（通る）と 6 件（compile fail）。clippy は 2 組とも警告 0。`cargo test` は feature なしが 7 passed と doctest 3・7 passed、`native-blocking-wait` が 7・1 passed と doctest 4・6 passed。wasm32 の `cargo check` は 3 組とも終了コード 0 |

確かめた時点は main の `734d5442c7c15642a8ec7396c24b76158d7c5a59`（`docs/spec` の gitlink は `d15921d873c6ce20039ecc7eb25a799d25e7748a`）。

追加で流したコマンドと結果。

```text
make deny                  終了コード 0
make public-api-check      ok: 10 public API document(s) checked, no wgpu-family references
make public-name-check     ok: 10 public API document(s) and 1 source file(s) checked, no listed words
make public-api-sdk-check  ok: 10 public API document(s) checked, common crate dependencies checked, no chip SDK references
```

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-2386）。

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/5 | #7804 | feat(upper): TASK-2386 の 1/5 device・queue・記録先・投入の印の型 |
| 2/5 | #7806 | feat(upper): TASK-2386 の 2/5 compute のパスの開始と終了 |
| 3/5 | #7808 | feat(upper): TASK-2386 の 3/5 バッファの包み |
| 4/5 | #7811 | test(upper): TASK-2386 の 4/5 試験用の device での投入・待ち・時刻の写しと状態を持たないことの試験 |
| 5/5 | #7814 | docs: TASK-2386 の 5/5 完了の条件の確かめ |

## 関連

- [upper-layer-crate-check](./upper-layer-crate-check.md)（上の層の crate の実物に当てた検査の記録）
- [upper-layer-check](./upper-layer-check.md)

## 今の値の補い（base 2f36d3d）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 試験用の device での試験（4/5）の「試験」（131 行目） | 試験のファイルは `upper_compute_path.rs`・`upper_wait_blocking.rs` の 2 つ → `crates/upper/tests/` の試験は 3 つ。ほかに `cross_layer_types.rs`（[cross-layer-types.md](cross-layer-types.md)）がある。本数は `upper_compute_path` 7・`upper_wait_blocking` 1・`cross_layer_types` 4 | `ls crates/upper/tests`、`cargo test -p fandhe-silicon-upper --features native-blocking-wait` |
| 同じ節の「組む条件」（133 行目） | `upper_compute_path` は feature の条件なし、`upper_wait_blocking` は `native-blocking-wait` の下だけ → `cross_layer_types` も `native-blocking-wait` の下だけ（`Cargo.toml` の `[[test]]` の `required-features`）。feature なしで流れる試験は `upper_compute_path` の 7 本だけ | `grep -n -A2 'cross_layer_types\|upper_wait_blocking' crates/upper/Cargo.toml`、`cargo test -p fandhe-silicon-upper` |
