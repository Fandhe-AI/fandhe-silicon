# CUDA の背後実装の完了の通知と検証の報告の受け口（TASK-607）の所在と本リポの現状

TASK-607 の作業のうち、所在と本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-607。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `impl ValidationIntake for CudaDevice` | 同ファイル |
| 試験 | 同ファイルの `validation_intake_tests` | 下の「確かめ方」 |

## 検証の報告の受け口の登録（1/3）

| 操作 | 置き場 | 動き |
| ---- | ------ | ---- |
| `set_validation_callback` | `CudaDevice` の `ValidationIntake` の実装 | 登録を受け付ける |
| 登録の保持 | 欄 `validation`（`ValidationReporter`） | 登録済みなら置き換えて保持する |
| 報告 | なし | この背後実装からは呼ばれない |

device に触らない試験 `the_cuda_device_takes_a_validation_callback` が、`CudaDevice` が `ValidationIntake` を実装し `Send + Sync` であることを型で照らす。

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib validation_intake_tests
```

## device の要る試験（2/3）

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_reclaimed_token_calls_back_complete_once_inside_the_call` | 回収済みの印を `on_complete` に渡す | 呼び出しの中で `Complete` を 1 回、呼ばれたスレッドは呼び出し元、登録の表の件数は 0、続けて `poll` を呼んでも 1 件のまま |
| `a_foreign_token_calls_back_lost_once_without_registering` | 別 device の印を `on_complete` に渡す | 呼び出しの中で `Lost` を 1 回、登録の表の件数は 0、通し番号は呼ぶ前と同じ、続けて投入と `poll` を呼んでも 1 件のまま |
| `an_unissued_sequence_calls_back_lost_once_without_registering` | 払い出していない番号の印を `on_complete` に渡す | 呼び出しの中で `Lost` を 1 回、登録の表の件数は 0、続けて投入と `poll` を呼んでも 1 件のまま |
| `a_submitted_token_calls_back_complete_exactly_once_and_never_pending` | 投入した印を `on_complete` に渡し、`poll` と投入を続ける | `Complete` がちょうど 1 回で `Pending` は現れない、呼ばれたスレッドは呼び出し元、登録の表の件数は 0 |
| `the_validation_callback_is_not_called_by_normal_operations` | 登録した関数のあとに確保・書き込み・投入・`poll`・`on_complete`・解放を呼ぶ | 関数は呼ばれず、`delivered_count` と `undelivered_count` は 0。登録し直しても同じ |

| 項目 | 値 |
| ---- | -- |
| module | `on_complete_device_tests`（`crates/contract/src/backend_cuda.rs`） |
| `cfg` | `#[cfg(test)]` |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib on_complete_device_tests
```

5 件が `ignored` と数えられる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-607。ポインタのみ）。

時点のコミット: `f445c3d9`。`docs/spec` の gitlink は `becfefdc2fb47fa610c6ecc914f3bf601910f5f3`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `on_complete_device_tests` と `validation_intake_tests` の module の中の `#[test]`・`#[ignore` の行の数（awk と `grep -c`） | `on_complete_device_tests` は `#[test]` が 5、`#[ignore` が 5。`validation_intake_tests` は `#[test]` が 1 |
| 2 | 受け手の試験の名前 | 試験の名前ごとの `git grep -n "fn <名前>" -- crates`、module は `git grep -n "mod lost_state_tests" -- crates` | 下の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib validation_intake_tests`、同 `--lib on_complete_device_tests`、`cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count` | `1 passed; 0 failed; 0 ignored`、`0 passed; 0 failed; 5 ignored`、`1 passed; 0 failed; 0 ignored` |
| 4 | 依存 0・unsafe 0 | `git diff --stat f4ef7802..HEAD -- Cargo.lock deps-approved.toml crates/contract/Cargo.toml crates/contract/tests/backend_cuda_unsafe_count.rs`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 5 | 破棄とロストの値を変えていない | `git diff --numstat f4ef7802..HEAD -- crates/contract/src/backend_cuda.rs` の 2 列目 | 追加 205、削除 0 |
| 6 | CI | `gh run view 44412525980 --json conclusion,headSha,jobs`（`f445c3d9` の main の push の run）、`verify` のジョブのログの `on_complete_device_tests` の行 | conclusion `success`、headSha `f445c3d9d011c2b7a5858d7cc185c52e69a56c86`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`。ログの行は下の出力 |

(4)・(5) の `f4ef7802` は、1/3 の 1 つ前のコミット。

(2) の出力。

```text
$ git grep -n "fn a_reclaimed_token_calls_back_complete_once_inside_the_call" -- crates
crates/contract/src/backend_cuda.rs:5204:    fn a_reclaimed_token_calls_back_complete_once_inside_the_call() {
$ git grep -n "fn a_foreign_token_calls_back_lost_once_without_registering" -- crates
crates/contract/src/backend_cuda.rs:5222:    fn a_foreign_token_calls_back_lost_once_without_registering() {
$ git grep -n "fn an_unissued_sequence_calls_back_lost_once_without_registering" -- crates
crates/contract/src/backend_cuda.rs:5242:    fn an_unissued_sequence_calls_back_lost_once_without_registering() {
$ git grep -n "fn a_submitted_token_calls_back_complete_exactly_once_and_never_pending" -- crates
crates/contract/src/backend_cuda.rs:5261:    fn a_submitted_token_calls_back_complete_exactly_once_and_never_pending() {
$ git grep -n "fn the_validation_callback_is_not_called_by_normal_operations" -- crates
crates/contract/src/backend_cuda.rs:5281:    fn the_validation_callback_is_not_called_by_normal_operations() {
$ git grep -n "fn the_cuda_device_takes_a_validation_callback" -- crates
crates/contract/src/backend_cuda.rs:4732:    fn the_cuda_device_takes_a_validation_callback() {
$ git grep -n "fn a_token_from_before_the_loss_polls_and_completes_as_lost" -- crates
crates/contract/src/backend_cuda.rs:4046:        fn a_token_from_before_the_loss_polls_and_completes_as_lost() {
$ git grep -n "fn teardown_ready_judges_like_poll_after_a_confirmed_sync" -- crates
crates/contract/src/backend_cuda.rs:4790:    fn teardown_ready_judges_like_poll_after_a_confirmed_sync() {
$ git grep -n "fn teardown_ready_delivers_lost_without_a_confirmed_sync" -- crates
crates/contract/src/backend_cuda.rs:4813:    fn teardown_ready_delivers_lost_without_a_confirmed_sync() {
$ git grep -n "fn dropping_right_after_on_complete_calls_it_once" -- crates
crates/contract/src/backend_cuda.rs:4930:    fn dropping_right_after_on_complete_calls_it_once() {
crates/contract/src/backend_metal.rs:9836:        fn dropping_right_after_on_complete_calls_it_once_with_complete() {
$ git grep -n "fn destroying_right_after_on_complete_calls_it_once" -- crates
crates/contract/src/backend_cuda.rs:4954:    fn destroying_right_after_on_complete_calls_it_once() {
crates/contract/src/backend_metal.rs:9859:        fn destroying_right_after_on_complete_calls_it_once() {
$ git grep -n "fn take_ready_keeps_pending_callbacks_and_releases_terminal_ones" -- crates
crates/contract/src/backend_cuda.rs:3189:    fn take_ready_keeps_pending_callbacks_and_releases_terminal_ones() {
$ git grep -n "fn run_callbacks_calls_every_callback_once_despite_a_panic_and_returns_the_first" -- crates
crates/contract/src/backend_cuda.rs:3156:    fn run_callbacks_calls_every_callback_once_despite_a_panic_and_returns_the_first() {
$ git grep -n "fn states_cover_pending_complete_failed_reclaimed_and_unissued" -- crates
crates/contract/src/backend_cuda.rs:3117:    fn states_cover_pending_complete_failed_reclaimed_and_unissued() {
$ git grep -n "fn reclaim_stops_at_a_pending_head_and_moves_failures_aside" -- crates
crates/contract/src/backend_cuda.rs:3100:    fn reclaim_stops_at_a_pending_head_and_moves_failures_aside() {
$ git grep -n "mod lost_state_tests" -- crates
crates/contract/src/backend_cuda.rs:4463:mod lost_state_tests {
crates/contract/src/backend_metal.rs:13707:mod lost_state_tests {
```

(3) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 0.00s

running 5 tests
test result: ok. 0 passed; 0 failed; 5 ignored; 0 measured; 95 filtered out; finished in 0.00s

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(6) の出力（`gh run view 44412525980 --job 135184165308 --log | grep 'on_complete_device_tests' | sed 's/^.*Z //' | sort | uniq -c`）。

```text
      6 test backend_cuda::on_complete_device_tests::a_foreign_token_calls_back_lost_once_without_registering ... ignored, requires a CUDA device
      6 test backend_cuda::on_complete_device_tests::a_reclaimed_token_calls_back_complete_once_inside_the_call ... ignored, requires a CUDA device
      6 test backend_cuda::on_complete_device_tests::a_submitted_token_calls_back_complete_exactly_once_and_never_pending ... ignored, requires a CUDA device
      6 test backend_cuda::on_complete_device_tests::an_unissued_sequence_calls_back_lost_once_without_registering ... ignored, requires a CUDA device
      6 test backend_cuda::on_complete_device_tests::the_validation_callback_is_not_called_by_normal_operations ... ignored, requires a CUDA device
```

## 門で止めた投入に登録した callback の device の要る試験（4/5）

門の口（`arm_gate`・`open_gate`）の所在は [cuda-fault-gate](cuda-fault-gate.md) を引く（TASK-345 のポインタ）。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `a_callback_registered_on_a_gated_token_is_called_complete_once_by_a_later_poll` | 門で止めた投入の印を `on_complete` に渡し、`poll`・`open_gate` を呼んでから完了まで `poll` を続ける | 登録の直後・`Pending` の間の `poll` の後・`open_gate` の後は受けた列が空で登録の表の件数は 1。完了の後の `poll` の中で `Complete` を 1 回、呼ばれたスレッドは試験のスレッド、登録の表の件数は 0、続けて `poll` を呼んでも 1 件のまま |
| `a_callback_registered_on_a_gated_token_is_called_lost_once_by_a_simulated_loss` | 門で止めた投入の印を `on_complete` に渡し、`simulate_device_lost` を呼ぶ | 登録の直後は受けた列が空で登録の表の件数は 1。`simulate_device_lost` が戻った直後に `Lost` を 1 回、呼ばれたスレッドは試験のスレッド、登録の表の件数は 0。`open_gate`・`poll`（`Lost`）・`drop` の後も 1 件のまま |

| 項目 | 値 |
| ---- | -- |
| module | `on_complete_gate_device_tests`（`crates/contract/src/backend_cuda.rs`） |
| `cfg` | `#[cfg(all(test, feature = "fault-injection"))]` |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib on_complete_gate_device_tests
```

2 件が `ignored` と数えられる。

## 完了の条件の確かめ（5/5）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-607。ポインタのみ）。

時点のコミット: `599ae9b7`。`docs/spec` の gitlink は `bfc545a1462bd69953ff876a1ff231348d3352f8`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `on_complete_gate_device_tests` の module の中の `#[test]`・`#[ignore` の行の数（awk と `grep -c`） | `#[test]` が 2、`#[ignore` が 2 |
| 2 | 受け手の試験の名前 | 試験の名前ごとの `git grep -n "fn <名前>" -- crates` | 下の出力 |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib on_complete_gate_device_tests`、`cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | `0 passed; 0 failed; 2 ignored`、`1 passed; 0 failed; 0 ignored` |
| 4 | 依存 0・unsafe 0 | `git diff --stat ed97ee62..HEAD -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml crates/contract/tests/backend_cuda_unsafe_count.rs`、`make deps-check` | diff は出力なし、`make deps-check` は終了コード 0 |
| 5 | 変えない本体 | `git diff --numstat ed97ee62..HEAD -- crates/contract/src/backend_cuda.rs` の 2 列目 | 追加 131、削除 0 |
| 6 | CI | `gh run view 44444207390 --json conclusion,headSha,jobs`（`599ae9b7` の main の push の run）、`verify` のジョブのログの `on_complete_gate_device_tests` の行 | conclusion `success`、headSha `599ae9b746cc2eb9d19c1b7490eae8b2641d11e1`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`。ログの行は下の出力 |

(4)・(5) の `ed97ee62` は、4/5 の 1 つ前のコミット。

(2) の出力。

```text
$ git grep -n "fn a_callback_registered_on_a_gated_token_is_called_complete_once_by_a_later_poll" -- crates
crates/contract/src/backend_cuda.rs:5848:    fn a_callback_registered_on_a_gated_token_is_called_complete_once_by_a_later_poll() {
$ git grep -n "fn take_ready_keeps_pending_callbacks_and_releases_terminal_ones" -- crates
crates/contract/src/backend_cuda.rs:3383:    fn take_ready_keeps_pending_callbacks_and_releases_terminal_ones() {
$ git grep -n "fn a_callback_registered_on_a_gated_token_is_called_lost_once_by_a_simulated_loss" -- crates
crates/contract/src/backend_cuda.rs:5874:    fn a_callback_registered_on_a_gated_token_is_called_lost_once_by_a_simulated_loss() {
$ git grep -n "fn take_stale_returns_older_registrations_as_lost_once" -- crates
crates/contract/src/backend_cuda.rs:3494:    fn take_stale_returns_older_registrations_as_lost_once() {
```

(3) の出力。

```text
running 2 tests
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 133 filtered out; finished in 0.00s

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

(6) の出力（`gh run view 44444207390 --job 135292247738 --log | grep 'on_complete_gate_device_tests' | sed 's/^.*Z //' | sort | uniq -c`）。

```text
      2 test backend_cuda::on_complete_gate_device_tests::a_callback_registered_on_a_gated_token_is_called_complete_once_by_a_later_poll ... ignored, requires a CUDA device
      2 test backend_cuda::on_complete_gate_device_tests::a_callback_registered_on_a_gated_token_is_called_lost_once_by_a_simulated_loss ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 検証の報告の受け口の登録を CUDA の device で受け付ける | #5206 |
| 2/3 | on_complete と検証の報告の受け口の device の要る試験を ignore 付きで足す | #5207 |
| 3/3 | 完了の条件の確かめ | #5208 |
| 4/5 | 門で止めた投入に登録した完了の callback の device の要る試験を ignore 付きで足す | #5209 |
| 5/5 | 完了の条件の確かめ | #5210 |
