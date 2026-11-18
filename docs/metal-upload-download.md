# Metal の背後実装の `upload`・`download_*` の用途の検査（TASK-316）の所在と本リポの現状

TASK-316 の作業のうち、Metal の背後実装の `MemoryOps::upload`・`download_blocking`・`download_async` が行う用途の検査の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-316。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の 1 ファイル | `crates/contract/src/backend_metal.rs` |
| 純粋な関数 | `usage_access_range`（`imp` の外） | `grep -n 'fn usage_access_range' crates/contract/src/backend_metal.rs` |
| 記録する欄 | `BufferEntry` の `usage`（`allocate` が受けた用途を入れる） | 同ファイルの `struct BufferEntry` |
| cfg | feature `backend-metal`。device に触る部分（`imp`）は macOS だけ | `crates/contract/src/lib.rs` |
| 試験 | 同ファイルの `usage_role_check_tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-metal --lib usage_role_check_tests` |
| device の取得・確保 | [metal-device-request.md](./metal-device-request.md)（TASK-158） | 同頁 |
| 用途と操作の対応 | [buffer-usage-check.md](./buffer-usage-check.md)（TASK-1796） | 同頁 |
| device の要る試験 | `crates/contract/tests/backend_metal_usage_role.rs`。`[[test]]` の feature は `backend-metal`・`native-blocking-wait`（`crates/contract/Cargo.toml`）、cfg は `target_os = "macos"`（macOS 以外では 0 件） | 下の「確かめ方」 |

## `upload`・`download_*` の用途の検査（1/3）

操作ごとの役割と、確保のときの用途に要る値:

| 操作 | 役割 | 要る用途 |
| ---- | ---- | -------- |
| `upload` | `UploadDestination` | `UPLOAD` |
| `download_blocking`・`download_async` | `ReadbackSource` | `DOWNLOAD` |

照らす順:

| 順 | 照らすもの | 返す値 |
| -- | ---------- | ------ |
| 1 | 別の device のハンドル | `DeviceMismatch` |
| 2 | 表にあるか | `InvalidArgument` |
| 3 | 払い出しの世代 | `InvalidArgument` |
| 4 | 記録した用途が役割を満たすか | `InvalidArgument` |
| 5 | 最後に使った投入が終わっているか | `InvalidArgument` |
| 6 | 範囲（`host_range`） | `InvalidArgument` |

- 用途の拒否の文字列は `<操作の名前>: buffer usage lacks <値> required for <役割>` の形。
- 判定に使うのは確保のときに記録した用途で、ハンドルの `usage()` は使わない。
- 長さ 0 の `upload` と読み出しも同じ順で照らす。
- `download_async` も同じ検査で、結果はコールバックに 1 回渡す。
- `free` のゼロ化は用途の検査を通さない。

device に触らない試験:

| 試験 | 確かめること |
| ---- | ------------ |
| `a_usage_without_the_required_value_is_rejected_with_op_and_value_names` | 足りない用途が 2 つの役割とも拒まれ、文字列に操作の名前と値の名前が入ること |
| `an_empty_usage_is_rejected_even_for_zero_length` | 空の用途が長さ 0 でも拒まれること |
| `the_full_usage_passes_both_roles` | `STORAGE \| UPLOAD \| DOWNLOAD` が 2 つの役割とも通ること |
| `a_usage_rejection_wins_over_a_range_rejection` | 用途が足りず範囲も外れる入力が用途の拒否で返ること |
| `a_usage_rejection_wins_over_an_in_use_rejection` | 用途が足りず使用中の入力が用途の拒否で返ること |
| `in_use_is_rejected_when_the_usage_passes` | 用途が足り使用中の入力が使用中の拒否で返ること |
| `an_out_of_range_input_is_rejected_by_the_range_check` | 用途が足り使用中でなく範囲の外の入力が `host_range` と同じ拒否で返ること |
| `a_passing_input_returns_the_host_range` | 全部通る入力が `host_range` と同じ値を返すこと |

確保の用途を `STORAGE | UPLOAD | DOWNLOAD` に替えた。期待値は変えていない。替えた試験・補助:

- `imp::device_tests::a_fresh_allocation_reads_back_all_zero`
- `imp::device_tests::reused_buffers_never_expose_previous_contents_over_1000_rounds`
- `imp::device_tests::dispatch` の補助 `buffer_of`・`zeroed`
- `dispatch_through_the_public_device_is_recorded_and_submit_is_unsupported`（`crates/contract/tests/backend_metal_device.rs`）

## 用途の検査の device の要る試験（2/3）

入口は `MetalDeviceProvider::request`（`Metal`・`ComputeOnly`・`ProductIndependent`）で作った device の `Device::memory()` の `allocate`・`upload`・`download_blocking`・`download_async`。6 本とも `#[ignore]` が付く。

| # | 試験 | 確保の用途 | 呼ぶ口 | 返る値 |
| - | ---- | ---------- | ------ | ------ |
| 1 | `upload_without_upload_usage_is_rejected_and_leaves_the_contents_unchanged` | `STORAGE \| DOWNLOAD`（対照は `STORAGE \| UPLOAD \| DOWNLOAD`） | `upload` | `InvalidArgument`（`upload`・`UPLOAD` を含む）。読み戻しは全バイト 0。対照は `Ok` で書いたバイト列と一致 |
| 2 | `download_blocking_without_download_usage_is_rejected` | `STORAGE \| UPLOAD` | `download_blocking` | `InvalidArgument`（`download_blocking`・`DOWNLOAD` を含む） |
| 3 | `download_async_without_download_usage_delivers_one_invalid_argument` | `STORAGE \| UPLOAD` | `download_async` | コールバックが 1 回、`InvalidArgument`（`download_async`・`DOWNLOAD` を含む） |
| 4 | `an_empty_usage_rejects_upload_and_download_even_for_zero_length` | 空 | 長さ 0 の `upload`・`download_blocking` | どちらも `InvalidArgument` |
| 5 | `the_recorded_usage_decides_even_if_the_handle_claims_more` | `STORAGE \| DOWNLOAD` | 用途を多く名乗るハンドルへの `upload` | `InvalidArgument`（`UPLOAD` を含む）。元のハンドルの読み戻しは全バイト 0 |
| 6 | `usage_is_checked_before_the_range` | `STORAGE \| DOWNLOAD`・`STORAGE \| UPLOAD`（16 バイト） | 範囲の外の `upload`・`download_blocking` | 用途の拒否の文字列で返る |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-metal --lib usage_role_check_tests
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait --test backend_metal_usage_role
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait -- -D warnings
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal -- -D warnings
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --all-features -- -D warnings
```

`backend_metal_usage_role` は macOS 以外では `running 0 tests`、macOS では `ignored` の件数が 6 になる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-316。ポインタのみ）。

時点のコミット: `0235afb2`。`docs/spec` の gitlink は `becfefdc2fb47fa610c6ecc914f3bf601910f5f3`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `grep -c '#\[test\]'`、`grep -c '^[[:space:]]*#\[ignore'`（どちらも `crates/contract/tests/backend_metal_usage_role.rs`）、`grep -c '^[[:space:]]*#\[ignore'`（`crates/contract/src/backend_metal.rs`） | `#[test]` は 6、`#[ignore` は 6。`backend_metal.rs` の `#[ignore` は 98 |
| 2 | 受け手の試験の名前 | 2/3 の 6 本・`usage_role_check_tests`・往復と範囲の既存の試験の名前の `git grep -n -E "fn (<名前>)\b\|mod usage_role_check_tests" -- crates/contract` | 下の (2) の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib usage_role_check_tests` | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 173 filtered out` |
| 4 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,native-blocking-wait -- -D warnings`、同じコマンドの `--features backend-metal`、同じコマンドの `--all-features` | 3 組とも終了コード 0 |
| 5 | CI の macOS のビルド | `gh run view 44265772364 --log --job 134669791508`（`make verify + build-profiles (macos-latest)`）の `backend_metal_usage_role` の部分。run は `0235afb2` の main の push | `test result: ok. 0 passed; 0 failed; 6 ignored`（`make verify` と `make build-profiles` の 2 か所）、run の conclusion `success`、headSha `0235afb23625bcedce2673d77ae55c27df7a938e` |
| 6 | 依存 0・unsafe の数 | `git diff --stat a0653759..HEAD -- Cargo.lock deps-approved.toml crates/core`、`cargo test -p fandhe-silicon-contract --test backend_metal_unsafe_count`、`make deps-check` | diff は出力なし、`test result: ok. 1 passed; 0 failed`、`make deps-check` は終了コード 0 |
| 7 | CI | `gh run view 44265772364 --json conclusion,headSha,jobs` | `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`、headSha `0235afb23625bcedce2673d77ae55c27df7a938e` |

(6) の `a0653759` は、1/3 の 1 つ前のコミット。

(2) の出力。

```text
crates/contract/src/backend_cuda.rs:4377:    fn the_recorded_usage_decides_even_if_the_handle_claims_more() {
crates/contract/src/backend_metal.rs:9438:    fn host_range_passes_in_range_and_rejects_the_rest() {
crates/contract/src/backend_metal.rs:9460:    fn host_range_rejects_an_overflowing_end_even_for_the_largest_allocation() {
crates/contract/src/backend_metal.rs:11799:mod usage_role_check_tests {
crates/contract/tests/backend_cuda_device.rs:148:fn download_async_calls_back_once_with_the_same_bytes() {
crates/contract/tests/backend_cuda_device.rs:279:fn out_of_range_upload_fails_and_leaves_the_contents_unchanged() {
crates/contract/tests/backend_metal_device.rs:41:fn request_returns_a_device_that_reports_diagnostics_and_round_trips_bytes() {
crates/contract/tests/backend_metal_device.rs:386:fn out_of_range_upload_fails_and_leaves_the_contents_unchanged() {
crates/contract/tests/backend_metal_device.rs:410:fn upload_with_an_overflowing_end_fails_and_leaves_the_contents_unchanged() {
crates/contract/tests/backend_metal_device.rs:433:fn upload_that_ends_exactly_at_the_allocation_is_accepted() {
crates/contract/tests/backend_metal_device.rs:449:fn download_blocking_rejects_out_of_range_and_overflowing_ranges() {
crates/contract/tests/backend_metal_device.rs:479:fn download_blocking_of_a_sub_range_matches_the_cpu_reference() {
crates/contract/tests/backend_metal_device.rs:503:fn download_async_rejects_an_out_of_range_read_with_one_callback() {
crates/contract/tests/backend_metal_device.rs:534:fn download_async_calls_back_once_with_the_same_bytes() {
crates/contract/tests/backend_metal_usage_role.rs:40:fn upload_without_upload_usage_is_rejected_and_leaves_the_contents_unchanged() {
crates/contract/tests/backend_metal_usage_role.rs:69:fn download_blocking_without_download_usage_is_rejected() {
crates/contract/tests/backend_metal_usage_role.rs:86:fn download_async_without_download_usage_delivers_one_invalid_argument() {
crates/contract/tests/backend_metal_usage_role.rs:117:fn an_empty_usage_rejects_upload_and_download_even_for_zero_length() {
crates/contract/tests/backend_metal_usage_role.rs:130:fn the_recorded_usage_decides_even_if_the_handle_claims_more() {
crates/contract/tests/backend_metal_usage_role.rs:152:fn usage_is_checked_before_the_range() {
```

(5) の出力（`make verify` の 1 か所目）。

```text
Running tests/backend_metal_usage_role.rs (target/debug/build/fandhe-silicon-contract/2b844ef1b72aeb7b/out/backend_metal_usage_role-2b844ef1b72aeb7b)

running 6 tests
test an_empty_usage_rejects_upload_and_download_even_for_zero_length ... ignored, touches the Metal device; run only in a dedicated device session
test download_async_without_download_usage_delivers_one_invalid_argument ... ignored, touches the Metal device; run only in a dedicated device session
test download_blocking_without_download_usage_is_rejected ... ignored, touches the Metal device; run only in a dedicated device session
test the_recorded_usage_decides_even_if_the_handle_claims_more ... ignored, touches the Metal device; run only in a dedicated device session
test upload_without_upload_usage_is_rejected_and_leaves_the_contents_unchanged ... ignored, touches the Metal device; run only in a dedicated device session
test usage_is_checked_before_the_range ... ignored, touches the Metal device; run only in a dedicated device session

test result: ok. 0 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | upload・download から用途の検査を呼ぶ | #4323 |
| 2/3 | 用途の検査の device の要る試験を ignore 付きで足す | #4324 |
| 3/3 | 完了の条件の確かめ | #4325 |
