# バッファのコピーとクリアの記録の操作と Metal の背後実装（TASK-610）の所在と本リポの現状

`MemoryOps` の、記録先へバッファ間のコピーとバッファのクリアを記録する 2 つの操作の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-610。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 操作の口 | `crates/core/src/memory/ops.rs` の `MemoryOps` の 2 つのメソッド `copy_buffer_to_buffer`・`clear_buffer` | `grep -n 'fn copy_buffer_to_buffer\|fn clear_buffer' crates/core/src/memory/ops.rs` |
| 試験用の device | `crates/core/src/stub_device.rs` の `StubMemory` | 同ファイル |
| 背後実装 | `crates/contract/src/backend_cuda.rs`・`backend_vulkan.rs`・`backend_metal.rs` の `impl MemoryOps` | 各ファイル |
| 試験 | `crates/core/tests/memory_copy_clear_stub.rs`（feature `test-support`） | 下の「確かめ方」 |
| device の要る試験（lib） | `crates/contract/src/backend_metal.rs` の `imp::copy_clear_device_tests`（cfg は `all(test, feature = "native-blocking-wait")`。macOS だけ） | 下の「確かめ方」 |
| device の要る試験（公開の口） | `crates/contract/tests/backend_metal_copy_clear.rs`（`required-features = ["backend-metal", "native-blocking-wait"]`、`#![cfg(target_os = "macos")]`） | 下の「確かめ方」 |

## 記録の操作の口（1/4）

`MemoryOps` に、既定の本体を持たない必須のメソッドを 2 つ持つ。feature・target の cfg は付かない。

```rust
fn copy_buffer_to_buffer(
    &self,
    encoder: &mut CommandEncoder,
    src: &BufferHandle,
    src_offset: u64,
    dst: &BufferHandle,
    dst_offset: u64,
    size: u64,
) -> Result<(), ContractError>;

fn clear_buffer(
    &self,
    encoder: &mut CommandEncoder,
    buffer: &BufferHandle,
    offset: u64,
    size: u64,
) -> Result<(), ContractError>;
```

試験用の device・CUDA・Vulkan・Metal の本体は、引数を使わずに `ContractError::Unsupported` を返す。`op` の値は `"copy_buffer_to_buffer"`・`"clear_buffer"`。
試験用の device の `backend` は、要求した背後実装の値。

足した試験は `copy_and_clear_on_the_stub_return_unsupported_and_record_nothing`。2 つの操作が `Unsupported` を返し、記録先の `Debug` 出力が操作の前後で変わらず `recorded: 0` のままであることを照らす。

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --features test-support --test memory_copy_clear_stub
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait --lib copy_clear_device_tests
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait --test backend_metal_copy_clear
```

下の 2 本の `cargo test` は、macOS 以外では 0 件、macOS では `ignored` になる。

## Metal の記録と投入（2/4）

`crates/contract/src/backend_metal.rs` の `impl MemoryOps for MetalDevice` の 2 つの本体は、記録先の列に plain な値（バッファの識別子・世代・offset・長さ）を積む。
記録先の列は dispatch の記録とコピー・クリアの記録を記録の順に持つ。Metal の API は記録の時点では呼ばない。

### 記録の時点の照らす順

各行は、全部のバッファに当ててから次の行へ進む（コピーはどの行でも `src` → `dst`）。1 行でも当たれば、その行の値を返して以降の行は見ない。

| 順 | 照らすもの | 返す値 |
| -- | ---------- | ------ |
| 1 | 記録先がこの device の開いている表にある（`0` は無い） | `InvalidArgument`（`encoder is not open on this device`） |
| 2 | バッファのハンドルの払い出し元がこの device | `DeviceMismatch` |
| 3 | 表に有る・ハンドルの世代が今の世代と同じ・ハンドルの `size()` が表のバッファの長さと同じ | `InvalidArgument` |
| 4 | 用途（コピーの `src` は `DOWNLOAD`、`dst` は `UPLOAD`。クリアは `UPLOAD`）。表の項目の用途で判定する | `InvalidArgument`（操作の名前と足りない用途の名前を含む） |
| 5 | `validate_buffer_range`（`offset + size` が確保長以下。`u64` のあふれを含む） | `InvalidArgument` |
| 6 | offset（コピーは 2 つ）と `size` が 4 の倍数 | `InvalidArgument` |
| 7 | （コピーだけ）`src` と `dst` が別のバッファ | `InvalidArgument` |
| 8 | `size` が 0 でない | `Ok(())`（何も記録しない） |

通ったときは記録先の列の末尾に積み、記録先に記録があることを印す。長さ 0 は 3〜7 に当たらなければ記録せず、当たれば拒む。
拒否したときは、記録先の列・バッファの内容・表を変えず、記録先は有効のまま続けて記録し投入できる。

### 投入の時点

`submit`・`submit_many` は、記録の順を保ち、続く dispatch の並びを compute encoder に、続くコピー・クリアの並びを blit encoder 1 つに積んで commit する。

- 積む前に、全部の記録先の全部の記録のバッファ（dispatch の束縛、コピーの `src`・`dst`、クリアの対象）が、表にあり、表の項目の世代が記録の世代と同じで、記録の世代が今の世代と同じことを、バッファの表の錠の中で確かめる。1 件でも外れれば何も積まず、通し番号も払い出さずに `InvalidArgument` を返す。
- クリアは `fillBuffer_range_value`（値 0）、コピーは `copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size` で積む。`blitCommandEncoder()` が `None` ならcommit せずに `BackendFailure`（`op` は `submit`）を返す。
- commit したら、dispatch の束縛・コピーの `src`・`dst`・クリアの対象の各バッファの最後に使った投入を、払い出した通し番号にする。
- fence・event は足さない。
- 記録を含む投入は、製品のビルドでは何も積まず commit せず `Unsupported` を返す（`DISPATCH_EXECUTION_ENABLED`。コピーとクリアだけの投入も同じ）。積み込みの形は単体試験のビルドだけが確かめる。
- 記録の後・投入の前に `free` したバッファを含む投入は `InvalidArgument`。投入の後の `free` は、投入の完了まで解放を保留する。

### device に触らない試験

`backend_metal.rs` の `copy_clear_check_tests`（どの OS でも流れる）。

| 試験 | 確かめること |
| ---- | ------------ |
| `copy_passes_and_records_the_operands`・`clear_passes_and_records_the_operand` | 通った入力が記録の値になる |
| `copy_rejects_each_row`・`clear_rejects_each_row` | 3〜7 の各行の拒否 |
| `copy_reports_the_earlier_row_when_two_adjacent_rows_apply`・`clear_reports_the_earlier_row_when_two_adjacent_rows_apply` | 隣り合う 2 行に同時に当たる入力が前の行の値を返す |
| `u64_overflow_is_rejected` | offset と `size` の和のあふれの拒否 |
| `zero_length_records_nothing_unless_an_earlier_row_applies` | 長さ 0 の扱い |
| `usage_rejections_name_the_operation_and_the_missing_usage` | 用途の拒否の文字列 |
| `segments_keep_the_record_order_and_group_the_same_kind` | 並びの切り方 |
| `recorded_transfers_pass_when_every_buffer_is_alive`・`recorded_transfers_reject_a_freed_or_stale_buffer_in_each_position` | 投入の時点の確かめ |

```bash
cargo test -p fandhe-silicon-contract --features backend-metal --lib copy_clear_check_tests
cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count --test backend_metal_no_recording
```

## device の要る試験（3/4）

9 本の試験は、どれも `#[ignore]` 付きで、device のある環境でだけ流れる。lib の 8 本は単体試験のビルドで投入が開くので、投入の後の内容を読み戻す。公開の口の 1 本は製品のビルドの動きを確かめる。

| 試験 | 記録 | 呼ぶ口 | 照らす値 |
| ---- | ---- | ------ | -------- |
| `copy_buffer_to_buffer_matches_the_cpu_reference` | 64 バイトの `src`（0..64）から、`dst`（0xEE で埋めた 64 バイト）の `[16, 48)` へ 32 バイトのコピー | `MetalDevice` の `copy_buffer_to_buffer`・`submit`・`wait_blocking`・`download_blocking` | 読み戻しが host で作った期待と全バイト一致 |
| `clear_buffer_zeroes_only_the_range` | 0xEE で埋めた 64 バイトの `[12, 32)` のクリア | `clear_buffer` | `[12, 32)` だけ 0、ほかは 0xEE |
| `a_copy_after_a_dispatch_in_the_same_encoder_sees_the_dispatch_output` | 同じ記録先に dispatch の後にコピー | `dispatch`・`copy_buffer_to_buffer` | コピーの先が dispatch の出力と一致 |
| `copy_and_clear_take_effect_in_record_order` | 「コピー → 同じ範囲のクリア」と「クリア → 同じ範囲へのコピー」 | `copy_buffer_to_buffer`・`clear_buffer` | 前者の後は 0、後者の後はコピーの元の値 |
| `record_time_rejections_leave_the_encoder_usable` | 範囲の超過・`u64` のあふれ・整列・同じバッファ・用途の欠け 3 種・別の device のバッファと記録先 | 同上 | 拒否の variant、記録の件数が 0 のまま、続けて記録した 1 件のコピーだけが結果に出る |
| `zero_length_records_nothing` | 長さ 0 のコピーとクリア、用途の無いバッファへの長さ 0 のクリア | 同上 | `Ok` と件数 0、内容が変わらない、用途の無い側は `InvalidArgument` |
| `a_buffer_freed_before_submit_rejects_the_submit_and_enqueues_nothing` | 記録の後に `dst`（クリアは対象）を `free` | `free`・`submit` | `InvalidArgument`、通し番号が進まない |
| `freeing_after_submit_defers_the_release_until_completion` | 確定を止めてコピーを投入し、`src`・`dst` を `free` | `free`・`test_pending_release_count`・`test_pool_len` | 2 つとも解放の保留にあり再利用の一覧は増えず、完了の後に保留が 0 |
| `copy_and_clear_through_the_public_device_are_recorded_and_submit_is_unsupported` | 公開の `Device` の記録先にコピーとクリア | `Device::memory()`・`Device::queue()` | 別の device のバッファは `DeviceMismatch`、`submit` は `Unsupported { op: "submit", backend: BackendKind::Metal }`、`dst` の読み戻しが投入の前と同じ |

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | MemoryOps にバッファのコピーとクリアの記録の操作を足す | #5220 |
| 2/4 | バッファのコピーとクリアを記録して投入で積む | #5221 |
| 3/4 | バッファのコピーとクリアの device の要る試験を ignore 付きで足す | #5222 |
