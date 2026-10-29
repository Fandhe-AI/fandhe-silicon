# Metal の背後実装の device の取得・診断・確保（TASK-158）の所在と本リポの現状

TASK-158 の作業のうち、Metal の背後実装の `DeviceProvider::request`・診断・確保の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。ポインタ: DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211、TASK-265、MEM-79・MEM-80、TASK-121・TASK-122。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-158。ポインタのみ）。内容・理由・経緯は本書に置かない。
依存の承認の記録は `deps-approved.toml`（`objc2`・`objc2-foundation`・`objc2-metal`・`objc2-core-graphics` の 4 件）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の 1 ファイル。中身は `cfg(target_os = "macos")` の内側にあり、macOS 以外（Linux・wasm32）では feature を付けても空のモジュール | `crates/contract/src/backend_metal.rs` |
| 公開の型 | `MetalDeviceProvider`。feature `fault-injection` の裏に `MetalDefaultDeviceNoneProvider`・`MetalCommandQueueNoneProvider`（実際の故障ではない差し込み）。device の型 `MetalDevice` は非公開で、`request` は `Box<dyn Device>` を返す | `grep -n 'pub ' crates/contract/src/backend_metal.rs` |
| 依存 | macOS 向けだけ・optional の 4 crate。`backend-metal` から `dep:` で有効にする。`block2`・`dispatch2` は依存グラフに入らない | `crates/contract/Cargo.toml`、`cargo tree -p fandhe-silicon-contract --target aarch64-apple-darwin -e features --features backend-metal,fault-injection` |
| Core Graphics のリンク | コマンドラインのプログラムで既定の device を得るため、`use objc2_core_graphics as _;` で参照を置く。試験の実行ファイルに CoreGraphics が載る | `otool -L target/debug/deps/backend_metal_request-*` |
| `unsafe` | 読み書きのコピー（`contents()` 越しの `copy_nonoverlapping`）だけ。個数は `crates/contract/tests/backend_metal_unsafe_count.rs` の表が持つ | [unsafe-policy](unsafe-policy.md) |

## 振る舞い

`request` が要求を照らす順（device を作る前。最初に当たったものを返す）:

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 背後実装が Metal でない | `InvalidCombination` |
| 2 | `Graphics` で、contract の feature `graphics` が無効 | `InvalidCombination` |
| 3 | `CpuOnly` | `InvalidCombination` |
| 4 | `Shared`・`Imported` | `Unsupported { backend: Metal, .. }`（`op` は `request(DeviceOwnership::Shared)`・`request(DeviceOwnership::Imported)`） |
| 5 | 既定の device が得られない | `DeviceUnavailable` |
| 6 | コマンドキューが得られない | `DeviceUnavailable` |

- `ProductIndependent` は呼ぶたびに新しい device とコマンドキューを作り、共有しない
- device の識別子はプロセスの中で device ごとに別の値、世代は Metal の背後実装の中で後に作った device ほど大きい値（1 以上）
- `diagnostics()` は `adapter_name`（`MTLDevice` の名前）と `backend_name: "metal"`
- `Capabilities`: `query` は常に `Unsupported`、`query_ex` は理由 `UnknownFeature` を添える（この段の問合せは、どの機能も未知として扱う）。`limits()` の `max_buffer_bytes` は `MTLDevice` の `maxBufferLength`、`max_dispatch_dims` は `[u32::MAX; 3]`、`max_texture_size` は 16384
- `MemoryOps`: shared の storage で確保する。確保の上限（`limits().max_buffer_bytes`）を超えると `InvalidArgument`。バッファの識別子は device ごとに払い出す（別々の device の最初のバッファは同じ識別子を持つ）。返すハンドルの `size`・`usage` は確保で渡した値
- ハンドルの照合: 払い出し元の device が違えば `DeviceMismatch`、表に無い識別子は `InvalidArgument`。範囲の検査は `validate_buffer_range`（確保長は Metal のバッファの `length()`）。`free` は別の device のハンドルを何も解放せず、自 device の同じ識別子のバッファを残す。解放したハンドルでの `upload`・`download_*` は `InvalidArgument`。範囲外の `upload` は `InvalidArgument` で、バッファの内容を変えない
- `download_async` は読み出してからその場で `callback` を 1 回だけ呼ぶ（失敗も渡す）。`download_blocking` は feature `native-blocking-wait` の裏
- この段ではコマンドバッファを積まない（読み書きの `unsafe` の同期の根拠がこの前提に頼るため、投入を足す変更では根拠を書き直す）

## 対応するテスト

| ファイル | required-features | 内容 | device に触るか |
| -------- | ----------------- | ---- | ---------------- |
| `crates/contract/tests/backend_metal_request.rs` | `backend-metal` | 背後実装違い・`CpuOnly`・`Graphics`（`graphics` 無効）・`Shared`・`Imported` の検査の値。背後実装の検査がプロファイル・所有形態より先であること（`request_reports_the_backend_mismatch_when_profile_and_ownership_also_fail`）、プロファイルの検査が所有形態より先であること（`request_checks_profile_before_ownership`） | 触らない |
| `crates/contract/tests/backend_metal_fault.rs` | `backend-metal`, `fault-injection` | 差し込みの入口でも検査が device を作る前であること、2 つの順序の検査が本物の入口と同じ値になること（`injected_provider_reports_the_backend_mismatch_first`・`injected_provider_checks_profile_before_ownership`）、`DeviceUnavailable` が `FailureClass::Initialization` で完了不明・実行時エラー・ロスト・能力不足と区別できること。`command_queue_none_is_an_initialization_failure` は device に触る | 1 件だけ触る（`#[ignore]`） |
| `crates/contract/tests/backend_metal_device.rs` | `backend-metal`, `native-blocking-wait` | `request` の成功・診断・能力・確保→書き込み→読み出し（`request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes`）、2 つの device の取り違えの拒否（`two_devices_do_not_accept_each_others_buffers`）、解放したハンドルの拒否（`upload_to_a_freed_handle_is_rejected_as_invalid_argument` は `upload`・`download_blocking`、`download_async_of_a_freed_handle_calls_back_once_with_invalid_argument` は `download_async` が `callback` を 1 回だけ呼び `InvalidArgument` を渡すこと）、別の device のハンドルの `free` で自 device のバッファが残ること（`freeing_another_devices_handle_keeps_the_buffer_with_the_same_id`）、範囲外の書き込みの拒否と内容の不変（`out_of_range_upload_fails_and_leaves_the_contents_unchanged`）、`download_async` の往復（`download_async_calls_back_once_with_the_same_bytes`）、確保の上限（`allocate_past_the_buffer_limit_is_invalid_argument`）、上限の表の値（`limits_report_the_dispatch_and_texture_bounds`） | 触る（`#[ignore]`） |
| `crates/contract/tests/backend_metal_unsafe_count.rs` | なし | `unsafe` の個数 | 触らない |

- 3 つの試験ファイルは `cfg(target_os = "macos")` の裏にあり、他の OS では 0 件
- device に触る試験は `#[ignore = "touches the Metal device; run only in a dedicated device session"]` を付け、既定の `cargo test` では流れない

## 確かめ方

- 既定の確かめ（device に触らない）: `cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection,native-blocking-wait`
- device に触る試験（専用の実機の回だけ）: 同じ組に `-- --ignored` を付ける。手順は [metal-run-procedure](metal-run-procedure.md)
- `request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes` は、得た `adapter_name` を `adapter_name=<値>` の 1 行で標準出力に出す（`-- --ignored --show-output` で見える）。値は照合しない
- 他の OS で壊れていないこと: `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings`、`make wasm-backend-check`
