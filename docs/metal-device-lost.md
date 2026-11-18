# Metal の背後実装の模擬のロストの後の操作の拒否と作り直し（TASK-186）の所在と本リポの現状

TASK-186 の作業のうち、Metal の背後実装の模擬のロストの後の操作の拒否の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-186。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装のファイル | `crates/contract/src/backend_metal.rs` | 同左 |
| `lost_result` | `imp` の外の関数。`lost` と `generation` を受け、device に触らずに `Ok(())` か `DeviceLost { generation }` を返す | `grep -n 'fn lost_result' crates/contract/src/backend_metal.rs` |
| `check_not_lost` | `MetalDevice` のメソッド。`fault-injection` のビルドでは `LostState::is_lost`、無いビルドでは `false` を `lost_result` に渡す。世代は呼んだ時点の `generation()` | `grep -n 'fn check_not_lost' crates/contract/src/backend_metal.rs` |
| 模擬のロストの口 | `docs/contract-fault-injection.md` の「Metal の `fault_hooks`（TASK-2932）」の節 | 同左 |

## ロストの後の操作の拒否（1/3）

`check_not_lost` は、次の操作の先頭（ほかの照らしより前）で呼ぶ。ロスト済みなら、呼んだ時点の `generation()` を載せた値を返す。

| 操作 | 照らす位置 | ロストの後に返す値 |
| ---- | ---------- | ------------------ |
| `allocate` | 先頭と、駆動の直後 | `Err(DeviceLost { generation })` |
| `upload` | 先頭 | `Err(DeviceLost { generation })` |
| `download_blocking`・`download_async`（`read_range` を通る） | 先頭 | `Err(DeviceLost { generation })`（`download_async` は callback に渡す） |
| `dispatch_ex` | 先頭 | `Err(DeviceLost { generation })` |
| `dispatch_checked` | 先頭 | `Err(DeviceLost { generation })` |
| `copy_buffer_to_buffer`・`clear_buffer` | 先頭 | `Err(DeviceLost { generation })` |
| `submit`・`submit_many` | 先頭と、駆動の直後 | `Err(DeviceLost { generation })` |
| `wait_blocking` | 先頭（印の照合と `timeout` の分岐の前） | `Err(DeviceLost { generation })` |
| `poll` | 先頭 | `CompletionState::Lost` |
| `poll_ex` | 先頭 | `CompletionStateEx::Lost` |

変えない操作の今の値は次のとおり。

| 操作 | 今の値 |
| ---- | ------ |
| `free` | ロストの後もロスト前のハンドルで 1 回外せる |
| `on_complete` | 変更なし |
| `gpu_timing` | 変更なし |

`fault-injection` の無いビルドでは、照らしは常に通る。

device に触らない試験（`crates/contract/src/backend_metal.rs` の `lost_check_tests`）は次のとおり。

| 試験 | 内容 |
| ---- | ---- |
| `lost_result_passes_when_not_lost` | `lost` が偽なら複数の世代で `Ok(())` |
| `lost_result_carries_the_given_generation_when_lost` | `lost` が真なら複数の世代で、その世代を載せた `DeviceLost` |
| `is_lost_turns_true_after_the_first_lose_and_stays_true` | `fault-injection` のビルド。`lose` の前は偽、1 回目の後も 2 回目の後も真 |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection --lib lost_check_tests
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,fault-injection -- -D warnings
```

## device の要る試験（2/3）

どの試験も Metal の device に触るので `#[ignore]` を付け、通常の確認（下の 1 組目のコマンド）では走らず `ignored` になる。専用の実機の回では `-- --ignored` を付けて流す。

| 試験 | 置き場 | 呼ぶ口 | 照らす値 |
| ---- | ------ | ------ | -------- |
| `two_losses_call_the_callback_once_and_move_the_generation_once` | `lost_invalidation_device_tests` | 具体型の device の `set_lost_callback` と `fault_hooks` | 通知の列が `[Unknown]` の 1 件。1 回目の後に世代が進み、2 回目の後は同じ値 |
| `the_listed_operations_issue_nothing_after_the_loss` | `lost_invalidation_device_tests` | 具体型の device の 13 の操作と `poll`・`poll_ex` | どの操作も `Err(DeviceLost { generation: device.generation() })`、`poll`・`poll_ex` は `Lost`。2 回繰り返しても同じで、バッファの表の件数・次の識別子・投入済みの表の次の通し番号は変わらない。ロスト前のバッファの `free` は表から外れる |
| `a_new_request_after_the_loss_runs_a_dispatch_and_reads_back` | `lost_invalidation_device_tests` | `MetalDeviceProvider::request` の新しい device | 世代がロストの前後のどちらよりも大きい。確保・`upload`・dispatch の投入・読み戻しが期待と全バイト一致。ロストした device の `allocate` は `DeviceLost` のまま |
| `the_listed_operations_are_lost_through_the_public_device` | `crates/contract/tests/backend_metal_device_lost.rs` | 公開の `Device` の `memory()`・`queue()`・`compute()` | 上の 2 本目と同じ操作が `DeviceLost`。dispatch を記録した記録先の `submit` も `Unsupported` ではなく `DeviceLost` |
| `a_new_request_after_the_loss_round_trips_through_an_empty_submit` | `crates/contract/tests/backend_metal_device_lost.rs` | 公開の `Device` | 世代がロストした device の後の値より大きい。確保・`upload`・空の記録先の `submit`・読み戻しが一致。ロストした device の `allocate` は `DeviceLost` のまま |

| 置き場 | cfg・feature |
| ------ | ------------ |
| `crates/contract/src/backend_metal.rs` の `imp` の中の `lost_invalidation_device_tests` | `test`・`native-blocking-wait`・`fault-injection`（`backend-metal` 下の macOS） |
| `crates/contract/tests/backend_metal_device_lost.rs` | `target_os = "macos"`。`[[test]]` の `required-features` は `backend-metal`・`native-blocking-wait`・`fault-injection` |

通常の確認（`#[ignore]` の試験は走らず、macOS では `ignored` になる）:

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,fault-injection --lib lost_invalidation_device_tests
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,fault-injection --test backend_metal_device_lost
```

専用の実機での実行（`-- --ignored` を付ける）:

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,fault-injection --lib lost_invalidation_device_tests -- --ignored
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,fault-injection --test backend_metal_device_lost -- --ignored
```

macOS 以外では 0 件になる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-186。ポインタのみ）。

時点のコミット: `3e9bb6d8`。`docs/spec` の gitlink は `becfefdc2fb47fa610c6ecc914f3bf601910f5f3`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `crates/contract/src/backend_metal.rs` の `lost_invalidation_device_tests`（字下げ 4）の範囲で `#[test]` と `^[[:space:]]*#\[ignore` を数える awk、`lost_check_tests`（字下げ 0）の範囲で `#[test]` を数える awk、`grep -c '#\[test\]'`・`grep -c '^[[:space:]]*#\[ignore'`（`crates/contract/tests/backend_metal_device_lost.rs`） | `lost_invalidation_device_tests` は `#[test]` 3・`#[ignore` 3。`lost_check_tests` の `#[test]` は 3。`backend_metal_device_lost.rs` は `#[test]` 2・`#[ignore` 2 |
| 2 | 受け手の試験の名前 | 5 本の device の要る試験・3 本の device に触らない試験・2 つの module の名前の `git grep -n -E "fn (<名前>)\b\|mod (lost_invalidation_device_tests\|lost_check_tests)\b" -- crates` | 下の (2) の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection --lib lost_check_tests`、同じコマンドの `--lib lost_state_tests`、`cargo test -p fandhe-silicon-contract --test backend_metal_no_recording --test backend_metal_unsafe_count` | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 208 filtered out`、`test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out`、`test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`（`backend_metal_no_recording`）、`test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`（`backend_metal_unsafe_count`） |
| 4 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features <組> -- -D warnings`。`<組>` は `backend-metal`・`backend-metal,native-blocking-wait`・`backend-metal,fault-injection`・`backend-metal,native-blocking-wait,fault-injection`。5 組目は `--features` の代わりに `--all-features` | 5 組とも終了コード 0 |
| 5 | CI の macOS のビルド | `gh run view 44387608503 --log --job 135096317902`（`make verify + build-profiles (macos-latest)`）の `backend_metal_device_lost` の部分。run は `3e9bb6d8` の main の push | `test result: ok. 0 passed; 0 failed; 2 ignored`（`make verify` と `make build-profiles` の 2 か所）、run の conclusion `success`、headSha `3e9bb6d82bb0dc6c029abc23fae8d6c39472fc86` |
| 6 | 依存・unsafe 0 | `git diff --stat 8f628ec2..HEAD -- Cargo.lock deps-approved.toml crates/contract/tests/backend_metal_unsafe_count.rs`、`make deps-check` | 1 つ目は出力なし。`make deps-check` は終了コード 0 |
| 7 | 門の字句 | `git grep -c -F 'const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);' -- crates/contract/src/backend_metal.rs`、`git grep -c -F 'if !DISPATCH_EXECUTION_ENABLED && records.iter().any(\|group\| !group.is_empty())' -- crates/contract/src/backend_metal.rs` | どちらも 1 |
| 8 | CI | `gh run view 44387608503 --json conclusion,headSha,jobs` | `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`、headSha `3e9bb6d82bb0dc6c029abc23fae8d6c39472fc86` |

(6) の `8f628ec2` は、1/3 の 1 つ前のコミット。

(2) の出力。

```text
crates/contract/src/backend_metal.rs:10337:    mod lost_invalidation_device_tests {
crates/contract/src/backend_metal.rs:10420:        fn two_losses_call_the_callback_once_and_move_the_generation_once() {
crates/contract/src/backend_metal.rs:10435:        fn the_listed_operations_issue_nothing_after_the_loss() {
crates/contract/src/backend_metal.rs:10564:        fn a_new_request_after_the_loss_runs_a_dispatch_and_reads_back() {
crates/contract/src/backend_metal.rs:14835:mod lost_check_tests {
crates/contract/src/backend_metal.rs:14840:    fn lost_result_passes_when_not_lost() {
crates/contract/src/backend_metal.rs:14847:    fn lost_result_carries_the_given_generation_when_lost() {
crates/contract/src/backend_metal.rs:14858:    fn is_lost_turns_true_after_the_first_lose_and_stays_true() {
crates/contract/tests/backend_metal_device_lost.rs:79:fn the_listed_operations_are_lost_through_the_public_device() {
crates/contract/tests/backend_metal_device_lost.rs:189:fn a_new_request_after_the_loss_round_trips_through_an_empty_submit() {
```

## 今の値の補い（base c1cf679）

記録時点の表と既存の節は書き換えない。

- 「device の要る試験（2/3）」の表の `the_listed_operations_issue_nothing_after_the_loss` の行の「呼ぶ口」の「具体型の device の 13 の操作と `poll`・`poll_ex`」は、今の試験と違う。試験が呼ぶのは 13 で、`Result` を返す 10 の操作（`allocate`・`upload`・`download_blocking`・`wait_blocking`・`dispatch`（既定の本体が `dispatch_ex` を呼ぶ）・`submit`・`submit_many`・`dispatch_checked`・`copy_buffer_to_buffer`・`clear_buffer`）、callback に結果を渡す `download_async`、`poll`・`poll_ex`。ほかに `free` と `generation()` を呼ぶ
- 「変えない操作の今の値」の表の `gpu_timing` の今の値は、ロストの後の同じ印で `GpuTiming::Unavailable`。確かめる試験は `crates/contract/tests/backend_metal_gpu_timing_lost.rs` の `gpu_timing_is_unavailable_after_the_simulated_loss`（`#[ignore]` 付きの device の試験。所在は [metal-gpu-timing-lost](metal-gpu-timing-lost.md)）
- 同じ表の `on_complete` の今の値は、ロストの前に登録した未完了の投入の callback が、ロストの時点で `CompletionState::Lost` を 1 回だけ受け取り、その後に完了させて駆動を重ねても `Complete` は届かず、二重にも呼ばれない。確かめる試験は `crates/contract/src/backend_metal.rs` の `a_pending_completion_callback_gets_lost_once_after_the_loss`（`#[ignore]` 付きの device の試験）。ロストの後に登録した場合の値は、この TASK では変えていない
- 確かめ方:

```bash
grep -n 'fn the_listed_operations_issue_nothing_after_the_loss' crates/contract/src/backend_metal.rs
grep -n 'fn a_pending_completion_callback_gets_lost_once_after_the_loss' crates/contract/src/backend_metal.rs
grep -n 'fn gpu_timing_is_unavailable_after_the_simulated_loss' crates/contract/tests/backend_metal_gpu_timing_lost.rs
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 模擬のロストの後の操作を `DeviceLost` で拒む | #3818 |
| 2/3 | ロストの後の操作と作り直しの device の要る試験を ignore 付きで足す | #3819 |
| 3/3 | 完了の条件の確かめ | #3820 |
