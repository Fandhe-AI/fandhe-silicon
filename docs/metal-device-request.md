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
| 依存 | macOS 向けだけ・optional の 4 crate（`objc2-metal` は feature `objc2-core-foundation` を付け、`GPUStartTime`・`GPUEndTime` を使う。`create_pipeline` のために `MTLComputePipeline`・`MTLDataType`・`MTLFunctionConstantValues` も付ける）。`backend-metal` から `dep:` で有効にする。`block2`・`dispatch2` はコンパイルされない（`cargo tree` に出ない）。`dispatch2` は `objc2-core-foundation` の弱い依存のために `Cargo.lock` と `cargo metadata` にだけ載り、`scripts/dep-graph-wgpu-check.sh` の `METADATA_ONLY_CRATES` の 1 件として確かめる | `crates/contract/Cargo.toml`、`cargo tree -p fandhe-silicon-contract --target aarch64-apple-darwin -e features --features backend-metal,fault-injection` |
| Core Graphics のリンク | コマンドラインのプログラムで既定の device を得るため、`use objc2_core_graphics as _;` で参照を置く。試験の実行ファイルに CoreGraphics が載る | `otool -L target/debug/deps/backend_metal_request-*` |
| `unsafe` | 読み書きのコピー（`contents()` 越しの `copy_nonoverlapping`）2 つと、function constant の設定（`setConstantValue_type_atIndex`）1 つ。個数は `crates/contract/tests/backend_metal_unsafe_count.rs` の表が持つ | [unsafe-policy](unsafe-policy.md) |

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
- 再利用の一覧（pool）とゼロ化（`MemoryOps::allocate`・`free`）:
    - `free` は、表から外したバッファの全バイトを 0 にしてから、同じ `byte_len` ごとの一覧へ回す。一覧に入っているのは全バイトが 0 のバッファだけ。`allocate` は同じ `byte_len` の一覧にあればそれを返し、無ければ Metal のバッファを新しく作る（Metal の確保 API が全バイトを 0 にする）。大きさの異なるバッファは再利用しない
    - 一覧の分類は、ハンドルが申告する大きさではなく Metal のバッファの実際の長さで決める
    - 一覧の上限は、件数 16 と、大きさの合計 64 MiB（計算は `checked_add`）。超える分は一覧に入れずに解放する。ゼロ化に失敗したバッファも一覧に入れずに解放する。一覧が受け入れない大きさはゼロ化もしない（ゼロ化の前に確かめ、入れるときに再度確かめる）。一覧に入ったバッファは device の Drop まで保持される（合計 64 MiB まで）。解放と一覧への流れは、バッファの型とゼロ化の関数を引数にした純粋な関数（`retire`）を通る
    - ハンドルの識別子は再利用のたびに新しく払い出す。解放したハンドルは、同じ大きさの再利用の後も新しいバッファに当たらず `InvalidArgument`。Metal のバッファごとの内部の識別子（slot）は一覧を出入りしても変わらない
    - ゼロ化は `upload` と同じ書き込みの関数（`write_range`）を通す。`write_range` は表の項目を `&mut` で受け、共有参照からは書けない。`unsafe` ブロックは 2 つのまま
    - 一覧から取り出した回数と、取り出した slot が直前に一覧へ入れた slot と一致した回数を、試験専用の口（`#[cfg(test)]`）で読む。公開の API は増えない
- ハンドルの照合: 払い出し元の device が違えば `DeviceMismatch`、表に無い識別子は `InvalidArgument`。範囲の検査は非公開の関数 `host_range` が共通の `validate_buffer_range` を呼んで行い（確保長は Metal のバッファの `length()`）、検査を通った値を `usize` に直す。`upload` の宛先と `download_*` の元が同じ関数を通る。`free` は別の device のハンドルを何も解放せず、自 device の同じ識別子のバッファを残す。解放したハンドルでの `upload`・`download_*` は `InvalidArgument`。範囲外の `upload`・`download_*`（終端が確保長を超える、`offset` が `u64::MAX`、終端が `u64` であふれる）は `InvalidArgument` で、`upload` はバッファの内容を変えない。終端が確保長ちょうど、`len` が 0 で `offset` が確保長ちょうどの範囲は通る
- `download_async` は読み出してからその場で `callback` を 1 回だけ呼ぶ（失敗も渡す）。`download_blocking` は feature `native-blocking-wait` の裏
- 投入（`Device::queue()` が返す `CommandQueue`。ポインタ: TASK-592）:
    - `create_encoder` は記録先の識別子（プロセス全体の通し番号。device をまたいで重ならない）を払い出し、この device の「開いている集合」に入れるだけで、コマンドバッファを作らず何も投入しない。投入せずに破棄すると集合から外れる
    - `submit` は、空でない `wait` を何も投入せず `Unsupported` で拒み（`submit_ex` も同じ経路）、記録先がこの device の開いている集合に無ければ（別 device の記録先を含む）`InvalidArgument` で何も投入しない。通れば、空のコマンドバッファを 1 つ作って確定し、device ごとに 1 から数える通し番号の印を返す
    - `submit_many` は、空の列（`InvalidArgument`）、`wait`（`Unsupported`）、`after` の各印の照合（別 device は `DeviceMismatch`、世代違いは `DeviceLost`）、全記録先の所属（`InvalidArgument`）の順に検査し、通れば空のコマンドバッファを 1 つだけ作って確定し（記録の操作が無いので 1 つで足りる。件数が多くてもキューの未完了の上限で詰まらない）、印を 1 つ返す。`after` は照合だけで、追加の待ちは入れない（単一の列で完了は投入順）
    - `poll` は、列の全部が完了なら `Complete`、どれかが `Error` なら `Lost`、他は `Pending`。別 device・世代違いの印は `Lost`。回収済みの番号は watermark と失敗の集合で答える（失敗の集合は watermark 以下の失敗した番号だけを持つので、増えるのは失敗の件数だけ）。失敗した投入を `Complete` とは返さない
    - `poll_ex` は、未完了は `Pending`、完了は `Complete`、`Error` で終わった列（失敗の集合にある番号を含む）は `RuntimeError`。別 device・世代違いの印は `Lost`。表にも失敗の集合にも無く watermark を超える番号（0 と払い出していない番号を含む）は `Unknown`。回収済みの番号は `Complete`。失敗した投入と払い出していない番号を `Complete` とは返さない。`poll` は失敗した列と払い出していない番号を `Lost` とし、`poll_ex` はこれを `RuntimeError`・`Unknown` に分ける（`Pending`・`Complete` は `poll` と同じ）。別 device・世代違いの印のほかに `Lost` を返す経路は持たない
    - `wait_blocking` は、失敗した投入を `DeviceLost` ではなく `BackendFailure` で返す。`None` なら表の錠を放してからコマンドバッファの完了を待ち、`Some` なら 1 ms 間隔で `poll` を確かめて期限で `Timeout`。`on_complete` は待たずに戻り、終端の状態（`Complete` か `Lost`。`Pending` は渡さない）で callback をちょうど 1 回呼ぶ。別 device の印・世代違いの印・払い出していない番号・完了済みの番号は登録の呼び出しの中でその場で呼ぶ。未完了なら登録し、`submit`・`submit_many`・`poll`・`poll_ex`・`wait_blocking`・`allocate` のどれかの中で終端と分かった時に、その呼び出しを行ったスレッドで呼ぶ（待ちのスレッドも完了ハンドラも使わない）。登録の表は投入済みの表と同じ錠の中にあり、callback は錠を放してから呼ぶので、callback の中から `poll`・`submit`・`on_complete` を呼んでも詰まらない。callback が panic しても残りは呼び切り（2 個目以降の panic の中身の `Drop` が panic しても止めて残りを呼ぶ）、最初の panic はその駆動の呼び出しから伝わる（登録の呼び出しの中でその場で呼んだ callback の panic は `on_complete` から伝わり、その場で呼ぶときも錠の外で呼ぶので、表の錠は毒化しない）。`MetalDevice` を落とすときに登録が残っていれば、その列の完了を待ってから `poll` と同じ判定の値で 1 回ずつ呼ぶ（`Drop` からは panic を伝えない）。
        - 注意: 駆動する呼び出し以外（`upload`・`download_*`・`free` など）では callback を呼ばない。呼ばれるには駆動する呼び出しを続ける必要があり、駆動しなければ `MetalDevice` を落とすまで残る
        - callback の panic は駆動した呼び出し（`submit` 系・`allocate`・`poll`・`wait_blocking`）から伝わる。登録の呼び出しの中でその場で呼んだ callback の panic は `on_complete` から伝わる（錠の外で呼ぶので毒化せず、そのあとも登録・駆動を続けられる）。`submit` 系に渡した `CommandEncoder` は閉じられて戻らず、`wait_blocking` は結果を返さずに巻き戻る。利用者の callback の panic を伝えるもので、背後実装自身は panic しない
        - `Drop` の待ちに上限は無く、確定済みの command buffer が終わるまで戻らない（この前提は投入するコマンドバッファが空である現在の形に対するもので、記録の操作を足す変更では見直す）。`Drop` で出た callback の panic と、その中身の破棄の panic は捨てる
    - `gpu_timing`（ポインタ: TASK-596、CMD-44〜CMD-48）は読み取り専用で、回収も `on_complete` の駆動も状態の変更もしない。時刻は device ごとの表（上限 1,024 件。超えたら通し番号の最も小さいものから捨てる）に、feature `native-blocking-wait` の `wait_blocking(token, None)` が完了を検出した時にだけ書く（`poll`・`poll_ex`・`submit` の回収と、`Some` の期限付きの待ち、`on_complete` は書かない）。`wait_blocking(None)` が観測する前に `poll` などで回収済みだった投入は時刻を残さず、`gpu_timing` は `Reclaimed` を返す。時刻は列のコマンドバッファの `GPUStartTime` の最小と `GPUEndTime` の最大（秒）で、有限でない値・0 以下の値（時刻を得られないときの 0 を含む）・終了が開始より前の組は残さない。返し:
        - 表に時刻がある: `Available { start_seconds, end_seconds }`（終了は開始以上。差分は呼び出し側）
        - 投入済みで未完了: `Pending`
        - 完了して回収された、時刻を残さずに完了した（完了済みで未回収を含む）、表から捨てられた: `Reclaimed`
        - `Error` で終わった投入、別 device の印、世代違いの印、払い出していない番号（0 を含む）: `Unavailable`
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
| `crates/contract/tests/backend_metal_device.rs` の転送の試験 | `backend-metal`, `native-blocking-wait` | `upload` の終端のあふれ（`offset` が `u64::MAX` 付近）の拒否と内容の不変（`upload_with_an_overflowing_end_fails_and_leaves_the_contents_unchanged`）、終端が確保長ちょうどの書き込みと末尾での空の書き込み（`upload_that_ends_exactly_at_the_allocation_is_accepted`）、`download_blocking` の範囲外・あふれの拒否と拒否後の内容の不変（`download_blocking_rejects_out_of_range_and_overflowing_ranges`）、部分範囲の読み戻しが CPU の参照と一致すること（`download_blocking_of_a_sub_range_matches_the_cpu_reference`） | 触る（`#[ignore]`） |
| `crates/contract/src/backend_metal.rs` の `precheck_tests`（単体）の `host_range` | `backend-metal` | 範囲内（確保長ちょうど・`len` 0 を含む）は通り、確保長超過・`offset` が `u64::MAX`・終端のあふれは `InvalidArgument`（`host_range_passes_in_range_and_rejects_the_rest`・`host_range_rejects_an_overflowing_end_even_for_the_largest_allocation`）。どの OS でも流れる | 触らない |
| `crates/contract/tests/backend_metal_unsafe_count.rs` | なし | `unsafe` の個数 | 触らない |
| `crates/contract/tests/backend_metal_no_recording.rs` | なし | ソース（コメント行を除く）に記録の操作（エンコーダの生成・バッファの束縛・dispatch・完了ハンドラ）の語が無いこと、`contents()` 越しの読み書きの 2 つの `// SAFETY:` が新しい根拠の語句を含むこと（`backend_metal_code_has_no_recording_words`・`backend_metal_safety_comments_state_the_no_recording_rationale`） | 触らない |
| `crates/contract/src/backend_metal.rs` の `precheck_tests`（単体） | `backend-metal` | device に触らない検査。投入済みの表の約束（失敗した列は `Lost` で失敗の集合へ、番号 0 と未払い出しは `Lost`、先頭が未完了なら後ろを回収しない、失敗でない回収済みは `Complete`）を偽の状態で、開いている集合の `claim`（id 0・未知・重複の拒否）と `close` を、空でない `wait` は `Unsupported`、`submit_many` の検査の順（空の列、`wait`、`after`）、`after` の照合（`DeviceMismatch`・`DeviceLost`）、記録先の通し番号が複数スレッドでも重ならず 0 を払い出さないこと。再利用の一覧の部品（同じ大きさだけ返す、再利用と slot の一致の数え方、件数・大きさの合計・あふれの上限、受け入れの事前の確認。`pool_*`・`would_accept_*`）と、`retire` の流れ（受け入れるとゼロ化して入る、ゼロ化に失敗すると入らない、受け入れない大きさはゼロ化を呼ばない。`retire_*`）。どの OS でも流れる | 触らない |
| `crates/contract/src/backend_metal.rs` の `precheck_tests` の `poll_ex` の単体 | `backend-metal` | `poll_ex` の分類を偽の状態で確かめる。未完了・完了・失敗・払い出していない番号が それぞれ `Pending`・`Complete`・`RuntimeError`・`Unknown` であること、回収済みの完了は `Complete`、番号 0 と払い出していない番号は `Unknown`、`poll` との対応。どの OS でも流れる | 触らない |
| `crates/contract/src/backend_metal.rs` の `precheck_tests` の `create_pipeline` の純粋な部分（`classify_*`・`slots_*`・`duplicate_constant_indexes_*`・`constant_wire_*`・`prepare_constants_*`。TASK-773 のポインタ） | `backend-metal` | 入力の分類（`Msl`・`MslSpecialized` は受け付け、`MslWithOptions`・`MslSpecializedWithOptions`・`Wgsl` は `Unsupported`、`SpirV`・`CudaC`・`Ptx`・`Dxil` は `ArtifactRejected { kind: "unsupported-input" }` で、理由は `pipeline input <variant> is not accepted by the Metal backend; accepted inputs are Msl and MslSpecialized` の形。試験は 4 variant の理由の全文を比べる）、空・重複・重複なしの `slots`、function constant の `index` の重複、`U32`（0・`u32::MAX`）と `Bool` の（種別, バイト列）への写し。どの OS でも流れる | 触らない |
| `crates/contract/src/backend_metal.rs` の `imp::device_tests`（単体。`create_pipeline`） | `backend-metal` | MSL から `Ok(PipelineHandle)` が返り device の識別子が入ること（`create_pipeline_returns_a_handle_of_this_device`）、2 回作ると識別子が違うこと（`create_pipeline_twice_gives_different_ids`）、`U32` と `Bool` の function constant で特殊化できること（`specialized_msl_with_u32_and_bool_constants_is_created`）、文法の誤りと存在しない入口名が `ArtifactRejected { kind: "msl" }` になること（`invalid_msl_is_artifact_rejected`・`missing_entry_point_is_artifact_rejected`）、重複した `slots` が `InvalidArgument` で表に何も入らないこと（`duplicate_slots_are_invalid_argument_and_leave_the_table_empty`）、`slots` がカーネルの引数と食い違っても生成でき表に `layout` の写しが入ること（`mismatched_slots_still_create_and_the_layout_copy_is_stored`）、`uint` の定数に `Bool` を渡すと `ArtifactRejected { kind: "msl" }`（`constant_of_the_wrong_type_is_artifact_rejected`）、`function_constant` を宣言したソースに空の定数の `MslSpecialized` を渡すと `ArtifactRejected`（`specialized_msl_with_no_constants_is_artifact_rejected`）。パイプラインの表に破棄の口は無い | 触る（`#[ignore]`） |
| `crates/contract/src/backend_metal.rs` の `imp::device_tests`（単体） | `backend-metal` | 投入した回数・開いている記録先の数を試験専用の口で読み、記録先の作成と破棄で投入が増えないこと、別 device の記録先と空でない `wait` が何も投入しないことを確かめる | 触る（`#[ignore]`） |
| `crates/contract/src/backend_metal.rs` の `imp::device_tests`（単体。再利用） | `backend-metal` | 一覧が空の状態の確保が全バイト 0（`a_fresh_allocation_reads_back_all_zero`）、`upload` → `free` → `allocate` を 1,000 回くり返して毎回 slot が一致し以前の内容が 1 バイトも読めないこと、再利用の回数 1,000 と slot の一致の回数が等しいこと（`reused_buffers_never_expose_previous_contents_over_1000_rounds`。`pool_reuse_count=<n> pool_slot_match_count=<n>` を 1 行出す）、解放したハンドルが再利用後のバッファに当たらないこと（`a_freed_handle_does_not_alias_the_reused_buffer`）、一覧が上限を超えて溜まらず別の大きさは再利用されないこと（`the_pool_is_bounded_and_matches_only_the_same_length`） | 触る（`#[ignore]`） |
| `crates/contract/tests/backend_metal_device.rs` の投入の試験 | `backend-metal`, `native-blocking-wait` | 最初の通し番号が 1 で device ごとに数えること、世代の一致、`wait_blocking` の後の `Complete`、空の `wait` の `submit_ex` が印を返し再試行せず `Complete` になること（`submit_ex_with_an_empty_wait_returns_a_token_that_completes`）、`submit_many` が印を 1 つ返すこと、空の列・別 device の `after`・別 device の記録先・空でない `wait` の拒否、`on_complete`（完了済みの印・別 device の印・`poll` と `wait_blocking` による駆動・device を落としたときの残りの呼び出し・callback の panic〔登録の時点で終端ならその場で、未完了なら後の駆動で、どちらか一方からちょうど 1 回伝わること。どちらの経路かは標準出力に出し、`--show-output` か `--nocapture` を付けると見える。駆動の経路は、下の `imp::device_tests` の `drive_path_*` が決定的に通す。panic の後ろも呼ぶことは下の `precheck_tests` の試験も確かめる〕。`submit`・`submit_many`・`allocate`・`poll_ex` が `poll` なしで駆動すること） | 触る（`#[ignore]`） |
| `crates/contract/src/backend_metal.rs` の `imp::device_tests`（単体。`on_complete` の駆動の経路） | `backend-metal`, `native-blocking-wait` | `drive_path_*` の 8 件（`drive_path_submit_calls_a_pending_registration`・`drive_path_submit_many_calls_a_pending_registration`・`drive_path_allocate_calls_a_pending_registration`・`drive_path_poll_calls_a_pending_registration`・`drive_path_poll_ex_calls_a_pending_registration`・`drive_path_wait_blocking_calls_a_pending_registration`・`drive_path_panic_propagates_from_drive_once_and_the_rest_are_called`・`drive_path_dropping_the_device_calls_a_pending_registration_once`）。試験専用の `#[cfg(test)]` の口でコマンドバッファの確定を遅らせ、登録の時点を未完了にしたうえで、`submit`・`submit_many`・`allocate`・`poll`・`poll_ex`・`wait_blocking(None)` のそれぞれが登録済みの callback をちょうど 1 回呼ぶこと、`poll` の駆動から panic が 1 回だけ伝わり残りの callback も呼ばれること、`MetalDevice` を落とすと残っていた登録が 1 回呼ばれることを確かめる。範囲: panic が `submit` 系・`allocate`・`wait_blocking` の駆動から伝わることは確かめない | 触る（`#[ignore]`） |
| `crates/contract/src/backend_metal.rs` の `precheck_tests` の `gpu_timing` の単体（`gpu_timing_*`・`the_timing_table_drops_the_smallest_sequence_past_1024_entries`・`span_*`） | `backend-metal` | 4 通りの返し、番号 0・未払い出し・失敗・回収済みの境界、回収後も時刻が残ること、1,025 件の記録で最も古いものが `Reclaimed`・最も新しいものが `Available`、問合せが回収も表の変更も起こさないこと、列の時刻の (最小の開始, 最大の終了) と0 以下を含む不正な時刻の除外。どの OS でも流れる | 触らない |
| `crates/contract/tests/backend_metal_device.rs` の `gpu_timing` の試験（`gpu_timing_*`） | `backend-metal`, `native-blocking-wait` | `wait_blocking(None)` の後の `Available`（終了は開始以上で両方 0 より大きい。値は標準出力）、`submit` の後に 50 ms 置いてからの `wait_blocking(None)` の後も `Available`（`gpu_timing_is_available_after_a_sleep_then_wait_blocking`）、待つ前は `Pending` か（完了済みの）`Reclaimed` で `Available` にならず、どちらだったかを出力で分けること、`poll` で回収した投入の `Reclaimed`（その後の `wait_blocking(None)` でも `Reclaimed` のまま）、`Some` の期限付きの待ちの後と `on_complete` の後に `Available` にならないこと、別 device の印・払い出していない番号の `Unavailable`、1,025 回の `submit` + `wait_blocking(None)` の後に最初の印が `Reclaimed`・最後の印が `Available` | 触る（`#[ignore]`） |
| `crates/contract/tests/backend_metal_device.rs` の `poll_ex` の試験（`poll_ex_is_complete_after_waiting`・`reclaimed_tokens_poll_as_complete`・`poll_ex_of_unissued_is_unknown_and_of_a_foreign_token_is_lost`） | `backend-metal`, `native-blocking-wait` | 待った後の `Complete`、回収後の `poll`・`poll_ex` の `Complete`、払い出していない番号が `Unknown`、待った後の自分の印が `Complete`、別 device の印が `Lost` であること。実機の試験は `Pending` を確かめない。`Pending` は単体の試験で確かめる | 触る（`#[ignore]`） |
| `crates/contract/src/backend_metal.rs` の `precheck_tests` の登録の表の試験 | `backend-metal` | 登録は終端の番号だけその場で返すこと、駆動が終端の登録だけを 1 回ずつ外すこと（先頭が未完了でも後ろの完了済みを外す）、回収済みの番号の判定、残りを全部外すときに `Pending` を渡さないこと、panic した callback の後ろも呼ぶこと、終端の番号でその場で呼んだ callback の panic が `register_table` から伝わり表の錠が毒化しないこと、駆動の本体・`Drop` の本体を通して callback の中から登録・駆動しても詰まらないこと、最初の panic の再送出と `Drop` で捨てること、捨てる中身の `Drop` が panic しても残りの callback を呼び切り巻き戻らないこと、判定が panic しても登録が残ること | 触らない |

- 3 つの試験ファイルは `cfg(target_os = "macos")` の裏にあり、他の OS では 0 件
- `backend_metal.rs` の `imp::tests` も `cfg(target_os = "macos")` の `imp` の中にあり、他の OS では 0 件
- device に触る試験は `#[ignore = "touches the Metal device; run only in a dedicated device session"]` を付け、既定の `cargo test` では流れない

## 確かめ方

- 既定の確かめ（device に触らない）: `cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection,native-blocking-wait`
- device に触る試験（専用の実機の回だけ）: 同じ組に `-- --ignored` を付ける。手順は [metal-run-procedure](metal-run-procedure.md)
- `request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes` は、得た `adapter_name` を `adapter_name=<値>` の 1 行で標準出力に出す（`-- --ignored --show-output` で見える）。この出力は照合に使わない
- 他の OS で壊れていないこと: `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings`、`make wasm-backend-check`
