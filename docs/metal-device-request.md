# Metal の背後実装の device の取得・診断・確保（TASK-158）の所在と本リポの現状

TASK-158 の作業のうち、Metal の背後実装の `DeviceProvider::request`・診断・確保の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。ポインタ: DEV-1・DEV-16・DEV-67・DEV-68・DEV-206・DEV-210・DEV-211、TASK-265、MEM-79・MEM-80、TASK-121・TASK-122。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-158。ポインタのみ）。内容・理由・経緯は本書に置かない。
依存の承認の記録は `deps-approved.toml`（`objc2`・`objc2-foundation`・`objc2-metal`・`objc2-core-graphics` の 4 件）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の 1 ファイル。実装は `cfg(target_os = "macos")` の内側にあり、macOS 以外（Linux・wasm32）では feature を付けても公開の項目を持たない。device に触らない純粋な検査の関数（`wait` の検査・`after` の印の照合・記録先の通し番号の払い出し）だけは `cfg` の外に置き、どの OS でも試験で確かめる | `crates/contract/src/backend_metal.rs` |
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
- `allocate` は長さ 0 を `InvalidArgument` で拒む（`imp` の非公開の関数 `validate_allocation_request` が 0 の判定のあとに共通の `validate_allocation_size` を呼ぶ。共通の関数は 0 を見ないままで、この拒否は Metal の背後実装の中だけ）
- ハンドルの照合: 払い出し元の device が違えば `DeviceMismatch`、表に無い識別子は `InvalidArgument`。範囲の検査は `validate_buffer_range`（確保長は Metal のバッファの `length()`）。`free` は別の device のハンドルを何も解放せず、自 device の同じ識別子のバッファを残す。解放したハンドルでの `upload`・`download_*` は `InvalidArgument`。範囲外の `upload` は `InvalidArgument` で、バッファの内容を変えない
- `download_async` は読み出してからその場で `callback` を 1 回だけ呼ぶ（失敗も渡す）。`download_blocking` は feature `native-blocking-wait` の裏
- 投入（`Device::queue()` が返す `CommandQueue`。ポインタ: TASK-592）:
    - `create_encoder` は記録先の識別子（プロセス全体の通し番号。device をまたいで重ならない）を払い出し、この device の「開いている集合」に入れるだけで、コマンドバッファを作らず何も投入しない。投入せずに破棄すると集合から外れる
    - `submit` は、空でない `wait` を何も投入せず `Unsupported` で拒み（`submit_ex` も同じ経路）、記録先がこの device の開いている集合に無ければ（別 device の記録先を含む）`InvalidArgument` で何も投入しない。通れば、空のコマンドバッファを 1 つ作って確定し、device ごとに 1 から数える通し番号の印を返す
    - `submit_many` は、空の列（`InvalidArgument`）、`wait`（`Unsupported`）、`after` の各印の照合（別 device は `DeviceMismatch`、世代違いは `DeviceLost`）、全記録先の所属（`InvalidArgument`）の順に検査し、通れば空のコマンドバッファを 1 つだけ作って確定し（記録の操作が無いので 1 つで足りる。件数が多くてもキューの未完了の上限で詰まらない）、印を 1 つ返す。`after` は照合だけで、追加の待ちは入れない（単一の列で完了は投入順）
    - `poll` は、列の全部が完了なら `Complete`、どれかが `Error` なら `Lost`、他は `Pending`。別 device・世代違いの印は `Lost`。回収済みの番号は watermark と失敗の集合で答える（失敗の集合は watermark 以下の失敗した番号だけを持つので、増えるのは失敗の件数だけ）。失敗した投入を `Complete` とは返さない
    - `wait_blocking` は、失敗した投入を `DeviceLost` ではなく `BackendFailure` で返す。`None` なら表の錠を放してからコマンドバッファの完了を待ち、`Some` なら 1 ms 間隔で `poll` を確かめて期限で `Timeout`。`on_complete` は、回収した `poll` の値が終端ならその場で、未完了なら表の錠を放して呼び出し元のスレッドで完了を待ってから、終端の状態で callback を 1 回だけ呼ぶ（別 device の印・表に無い番号は待たずに `Lost`）
    - 投入済みの表は device の錠の中にあり、`MetalDevice` は `Send`・`Sync` ではない（`unsafe impl` は足していない）。ロストを返す経路は持たない
- 投入するコマンドバッファは記録の操作（エンコーダの生成・コピー・クリア・dispatch）を持たない。読み書きの `unsafe` の同期の根拠がこの前提に頼るため、記録の操作を足す変更では根拠を書き直す

## 対応するテスト

| ファイル | required-features | 内容 | device に触るか |
| -------- | ----------------- | ---- | ---------------- |
| `crates/contract/tests/backend_metal_request.rs` | `backend-metal` | 背後実装違い・`CpuOnly`・`Graphics`（`graphics` 無効）・`Shared`・`Imported` の検査の値。背後実装の検査がプロファイル・所有形態より先であること（`request_reports_the_backend_mismatch_when_profile_and_ownership_also_fail`）、プロファイルの検査が所有形態より先であること（`request_checks_profile_before_ownership`） | 触らない |
| `crates/contract/tests/backend_metal_fault.rs` | `backend-metal`, `fault-injection` | 差し込みの入口でも検査が device を作る前であること、2 つの順序の検査が本物の入口と同じ値になること（`injected_provider_reports_the_backend_mismatch_first`・`injected_provider_checks_profile_before_ownership`）、`DeviceUnavailable` が `FailureClass::Initialization` で完了不明・実行時エラー・ロスト・能力不足と区別できること。`command_queue_none_is_an_initialization_failure` は device に触る | 1 件だけ触る（`#[ignore]`） |
| `crates/contract/tests/backend_metal_device.rs` | `backend-metal`, `native-blocking-wait` | `request` の成功・診断・能力・確保→書き込み→読み出し（`request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes`）、2 つの device の取り違えの拒否（`two_devices_do_not_accept_each_others_buffers`）、解放したハンドルの拒否（`upload_to_a_freed_handle_is_rejected_as_invalid_argument` は `upload`・`download_blocking`、`download_async_of_a_freed_handle_calls_back_once_with_invalid_argument` は `download_async` が `callback` を 1 回だけ呼び `InvalidArgument` を渡すこと）、別の device のハンドルの `free` で自 device のバッファが残ること（`freeing_another_devices_handle_keeps_the_buffer_with_the_same_id`）、範囲外の書き込みの拒否と内容の不変（`out_of_range_upload_fails_and_leaves_the_contents_unchanged`）、`download_async` の往復（`download_async_calls_back_once_with_the_same_bytes`）、確保の上限（`allocate_past_the_buffer_limit_is_invalid_argument`）、上限の表の値（`limits_report_the_dispatch_and_texture_bounds`）、長さ 0 の確保の拒否（`allocate_of_zero_bytes_is_invalid_argument`） | 触る（`#[ignore]`） |
| `crates/contract/src/backend_metal.rs` の `imp::tests`（単体） | `backend-metal` | `validate_allocation_request` の境界。0 は `InvalidArgument`（上限が 0 でも）、1 と上限ちょうどは通る、上限 +1 は `InvalidArgument`（`zero_length_is_invalid_argument`・`zero_length_is_rejected_even_when_the_limit_is_zero`・`one_byte_and_the_limit_itself_pass`・`one_past_the_limit_is_invalid_argument`） | 触らない |
| `crates/contract/tests/backend_metal_ext_interop.rs`（TASK-195） | `backend-metal`, `ext-interop` | `None` の識別子が同一 GPU の判定で `Undeterminable` になること（`a_missing_identity_is_never_a_match`。どの OS でも流れ、device に触らない）。Metal の device の `external_interop()`・`physical_gpu_id()` が `None` であること（`metal_device_reports_no_interop_and_no_physical_gpu_id`）、device の識別子を使った判定が両側とも `Undeterminable`（`metal_physical_gpu_id_is_undeterminable_in_the_same_gpu_check`）、取り込み先を device の識別子にすると `import_memory_checked` が拒否して `import_memory` を呼ばないこと（`import_into_a_metal_target_is_rejected_before_import_memory`）。後者 3 件は macOS のみ | 3 件は触る（`#[ignore]`） |
| `crates/contract/tests/backend_metal_unsafe_count.rs` | なし | `unsafe` の個数 | 触らない |
| `crates/contract/tests/backend_metal_no_recording.rs` | なし | ソース（コメント行を除く）に記録の操作（エンコーダの生成・バッファの束縛・dispatch・完了ハンドラ）の語が無いこと、`contents()` 越しの読み書きの 2 つの `// SAFETY:` が新しい根拠の語句を含むこと（`backend_metal_code_has_no_recording_words`・`backend_metal_safety_comments_state_the_no_recording_rationale`） | 触らない |
| `crates/contract/src/backend_metal.rs` の `precheck_tests`（単体） | `backend-metal` | device に触らない検査。投入済みの表の約束（失敗した列は `Lost` で失敗の集合へ、番号 0 と未払い出しは `Lost`、先頭が未完了なら後ろを回収しない、失敗でない回収済みは `Complete`）を偽の状態で、開いている集合の `claim`（id 0・未知・重複の拒否）と `close` を、空でない `wait` は `Unsupported`、`submit_many` の検査の順（空の列、`wait`、`after`）、`after` の照合（`DeviceMismatch`・`DeviceLost`）、記録先の通し番号が複数スレッドでも重ならず 0 を払い出さないこと。どの OS でも流れる | 触らない |
| `crates/contract/src/backend_metal.rs` の `imp::device_tests`（単体） | `backend-metal` | 投入した回数・開いている記録先の数を試験専用の口で読み、記録先の作成と破棄で投入が増えないこと、別 device の記録先と空でない `wait` が何も投入しないことを確かめる | 触る（`#[ignore]`） |
| `crates/contract/tests/backend_metal_device.rs` の投入の試験 | `backend-metal`, `native-blocking-wait` | 最初の通し番号が 1 で device ごとに数えること、世代の一致、`wait_blocking` の後の `Complete`、`submit_many` が印を 1 つ返すこと、空の列・別 device の `after`・別 device の記録先・空でない `wait` の拒否、`on_complete` | 触る（`#[ignore]`） |

- 3 つの試験ファイルは `cfg(target_os = "macos")` の裏にあり、他の OS では 0 件
- `backend_metal.rs` の `imp::tests` も `cfg(target_os = "macos")` の `imp` の中にあり、他の OS では 0 件
- device に触る試験は `#[ignore = "touches the Metal device; run only in a dedicated device session"]` を付け、既定の `cargo test` では流れない

## 確かめ方

- 既定の確かめ（device に触らない）: `cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection,native-blocking-wait`
- device に触る試験（専用の実機の回だけ）: 同じ組に `-- --ignored` を付ける。手順は [metal-run-procedure](metal-run-procedure.md)
- `request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes` は、得た `adapter_name` を `adapter_name=<値>` の 1 行で標準出力に出す（`-- --ignored --show-output` で見える）。この出力は照合に使わない
- 他の OS で壊れていないこと: `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings`、`make wasm-backend-check`
