# CUDA の背後実装の模擬のロスト（TASK-2934）の所在と本リポの現状

TASK-2934 の作業のうち、CUDA の背後実装の device の世代の扱いの所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2934。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | `backend_cuda.rs` の `mod tests`。device に触らない | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda` |

device の取得・診断・確保は [cuda-device-request.md](./cuda-device-request.md) を参照。

## 世代の読み方と `free` の照合（1/4）

device の世代は `AtomicU64` で持ち、`current_generation()` で読む。1 つの操作の中の照合と払い出しには、その操作で 1 回読んだ値を使う。

世代を読む操作は次のとおり。

- `Device::generation()`
- ハンドルの照合（`range`）
- `allocate` の払い出し
- 投入の印の照合と払い出し（`submit`・`submit_many`・`poll`）
- `on_complete` の登録

`free` の照らし方は次のとおり。

| 入力 | 動き |
| ---- | ---- |
| 別 device のハンドル | 何もしない |
| 表の項目の払い出しの世代と同じ世代のハンドル | 表から外し、今までの解放の流れに進む |
| 世代が違う・表に無い識別子 | 何もしない |

完了の callback の登録は、登録した印の世代を持つ。

| 登録の世代 | 駆動の動き |
| ---------- | ---------- |
| 駆動が読んだ世代より小さい | 列の状態に関わらず `Lost` |
| 等しい | 列の状態 |
| 大きい | 表に残す |

呼ぶ直前に世代が進んでいれば、決めた値に関わらず `Lost` で呼ぶ。`on_complete` は表の錠の中で世代を読み直し、違えば積まずに `Lost` で呼ぶ。

| 試験 | 確かめること |
| ---- | ------------ |
| `remove_issued_matches_the_issue_generation_not_the_current_one` | 世代違い・表に無い識別子は外さず、一致した項目だけを 1 回外す |
| `remove_issued_ignores_forged_ids_and_an_empty_table` | 空の表と偽の識別子は何も外さず、別の項目を巻き込まない |
| `take_ready_answers_lost_for_an_older_generation_and_keeps_a_newer_one` | 登録の世代が小さければ `Lost`、大きければ表に残す |
| `a_complete_judged_before_the_generation_moved_is_delivered_as_lost` | 世代が進んだ後は `Lost` で呼ぶ |

確かめ方: `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda`

## 模擬のロスト（2/4）

部品は feature `backend-cuda` と `fault-injection` の両方を付けたビルドにある（`Submissions::take_stale` は試験のビルドにもある）。
`FaultHooks::simulate_device_lost(reason)` の動きは次のとおり。

| 呼び出し | 動き |
| -------- | ---- |
| 1 回目 | 理由を記録し、`generation()` を前より大きい値に取り直す。登録済みのコールバックを錠の外で 1 回呼び、その後に登録の世代が古い完了の callback を表から外して `Lost` で 1 回ずつ呼ぶ |
| 2 回目以降 | 何もせず `Ok`。世代もコールバックの呼び出しの数も変わらない |
| 世代の源が尽きている | 状態を変えず `BackendFailure` |

- 理由は `Destroyed`・`Unknown`・`ValidationError`・`Unrecoverable` の 4 値ともそのまま渡す
- ロストの通知先は `DeviceLostNotify::set_lost_callback` で登録する。登録済みなら上書きし、ロストの後の登録は保持するだけで呼ばない。`Box<dyn Device>` からは届かないので、crate の内側の試験から登録する
- `Drop` ではロストのコールバックを呼ばない
- `arm_gate`・`open_gate`・`simulate_runtime_error`・`simulate_validation_report` は既定の本体のまま（`UnsupportedExtensionDefault`）。ロストの後の操作の拒否はこの段では持たない

| 試験 | 確かめること |
| ---- | ------------ |
| `take_stale_returns_older_registrations_as_lost_once` | 登録の世代が古いものだけを `Lost` で外し、2 回目は空 |
| `the_callback_runs_once_and_a_second_lose_is_ignored` | コールバックは 1 回で、2 回目は無視する |
| `every_reason_is_passed_through_unchanged` | 4 値の理由をそのまま渡す |
| `the_generation_is_taken_once` | 世代は 1 回だけ取る |
| `an_exhausted_source_leaves_the_state_unchanged` | 源が尽きたら状態を変えない |
| `a_callback_registered_after_the_loss_is_kept_but_not_called` | ロストの後の登録は呼ばない |
| `a_loss_without_a_callback_then_a_registration_never_calls_it` | コールバック無しのロストの後の登録も呼ばない |
| `dropping_an_overwritten_callback_may_reenter_without_deadlock` | 上書きで捨てる値の `Drop` が再入しても詰まらない |
| `concurrent_loses_call_the_callback_once_and_advance_the_source_once` | 並行のロストでも 1 回だけ |
| `a_later_registration_replaces_the_earlier_one` | 後の登録が前の登録を置き換える |
| `the_callback_runs_outside_the_lock` | コールバックは錠の外で呼ぶ |
| `a_poisoned_lock_does_not_panic` | 毒化した錠でも panic しない |

確かめ方: `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda`

## device の要る試験（3/4）

試験は `backend_cuda.rs` の `mod device_tests` の子 `mod lost` にある。feature `backend-cuda` と `fault-injection` の両方を付けたビルドでだけコンパイルされ、どれも `#[ignore = "requires a CUDA device"]` 付き。

| 試験 | 確かめること |
| ---- | ------------ |
| `each_reason_reaches_the_callback_once_and_the_generation_moves_once` | 理由ごとにコールバックは 1 回で世代は 1 回進み、2 回目は何も変えない |
| `a_handle_from_before_the_loss_can_still_be_freed_once` | ロストの前のハンドルは `upload` が `Err` で、`free` で表から 1 回外れ、写しの `free` は何も変えない |
| `a_token_from_before_the_loss_polls_and_completes_as_lost` | ロストの前の印の `poll` は `Lost`、その印の `on_complete` は `Lost` を 1 回渡す |

確かめ方: `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda` の `ignored` が 3/4 の前より 3 本多い（12 から 15）。`passed` は変わらない。

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2934。ポインタのみ）。

時点のコミット: `81dc1f9`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 世代の読み方と `free` の照合・登録の世代 | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda`、`grep -n 'self\.generation' crates/contract/src/backend_cuda.rs` | `49 passed; 0 failed; 11 ignored`、終了コード 0。grep は 3 行 |
| 2 | 模擬のロストの部品 | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda::lost_state_tests`、同じ組で `--lib backend_cuda::tests::take_stale` | `12 passed; 0 failed; 0 ignored`、`1 passed; 0 failed; 0 ignored`。どちらも終了コード 0 |
| 3 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda::device_tests::lost` | `0 passed; 0 failed; 3 ignored`、終了コード 0 |
| 4 | 門ごとの型と未使用 | `cargo clippy -p fandhe-silicon-contract --all-targets --no-default-features --features <組> -- -D warnings`（`<組>` は `backend-cuda`・`backend-cuda,ext-interop`・`backend-cuda,fault-injection`・`backend-cuda,ext-interop,fault-injection,native-blocking-wait`） | 4 組とも終了コード 0 |
| 5 | 公開 API | `make public-api-sdk-check`、`make public-api-webgpu-check` | どちらも終了コード 0。`sdk_type_refs_total=0` を含む `record:` の行と `ok:` の行が出る |
| 6 | 個数テスト | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_unsafe_count`、`git diff --stat 6d98ade HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs` | `1 passed; 0 failed`、終了コード 0。diff は空 |
| 7 | 依存 | `make deps-check`、`git diff --stat 6d98ade HEAD -- Cargo.lock deps-approved.toml crates/contract/Cargo.toml` | 終了コード 0。`[deps-check.sh] ok: ...` の行が出る。diff は空 |
| 8 | CI | `gh run view 42514750106 --json status,conclusion,headSha,jobs`（`81dc1f9` の main の push の run） | conclusion は `success`。headSha は `81dc1f94da5e764cf65041d0fb28e668cdcd5647`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` の conclusion は `success` |

(1) の出力（該当の行）。

```text
test result: ok. 49 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 0.00s
1340:        self.generation.load(Ordering::Acquire)
1639:            .lose_catching(reason, &self.generation, &NEXT_GENERATION)
3876:                .lose(DeviceLostReason::Unknown, &self.generation, &self.source);
```

(2) の出力（該当の行）。

```text
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 64 filtered out; finished in 0.00s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 75 filtered out; finished in 0.00s
```

(3) の出力（該当の行）。

```text
running 3 tests
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 73 filtered out; finished in 0.00s
```

(5) の出力（該当の行）。

```text
record: commit=81dc1f94da5e764cf65041d0fb28e668cdcd5647 documents=10 public_items_total=13838 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
ok: 10 public API document(s) checked, common crate dependencies checked, no chip SDK references
ok: 2 public API document(s) checked, no blocking-wait names, pipeline input ok, ext-interop ok, submit token ops ok
```

(6) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

(7) の出力。

```text
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 世代の読み方と `free` の照合 | #4228 |
| 2/4 | 模擬のロスト | #4229 |
| 3/4 | device の要る試験 | #4230 |
| 4/4 | 完了の条件の確かめ | #4231 |

## 今の値の補い（base 1417dc5）

記録時点の表と既存の節は書き換えない。以下は base `1417dc5` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「所在」の表の試験の行 | `backend_cuda.rs` の `mod tests` → `mod tests`（2032 行目）に加えて、`mod device_tests`（3273 行目。`#[cfg(test)]`）、その子の `mod lost`（3519 行目。feature `fault-injection` の裏）、`mod lost_state_tests`（3745 行目。`#[cfg(all(test, feature = "fault-injection"))]`）、`mod pool_device_tests`（3602 行目。`native-blocking-wait` の裏）がある | `grep -nE '^\s*mod (tests|device_tests|lost|lost_state_tests|pool_device_tests)\b' crates/contract/src/backend_cuda.rs` |
| 「device の要る試験（3/4）」の表（3 行）と「`mod device_tests` の子 `mod lost` にある」の文 | 3 本 → 表の 3 本のほかに、`device_tests` の直下に `a_panicking_lost_callback_still_delivers_lost_to_completion_callbacks`（feature `fault-injection` の裏、`#[ignore = "requires a CUDA device"]` 付き）がある。`--features backend-cuda,fault-injection --lib backend_cuda::device_tests` は `0 passed; 0 failed; 15 ignored`（`fault-injection` 無しの `backend-cuda` は 11 ignored、15 は 11 に直下の 1 本と `lost` の 3 本が加わった値） | `cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib backend_cuda::device_tests`、`cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::device_tests`、`grep -n 'a_panicking_lost_callback' crates/contract/src/backend_cuda.rs`（3307 行目） |
| 「世代の読み方と `free` の照合（1/4）」の確かめ方・(1) の行と出力（`--no-default-features --features backend-cuda --lib backend_cuda`） | `49 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out` → `50 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out`（`running 61 tests`） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda` |
| 「模擬のロスト（2/4）」の確かめ方と「3/4」の `ignored` の文（`--no-default-features --features backend-cuda,fault-injection --lib backend_cuda`） | `ignored` は 12 から 15、`passed` は変わらない → 15 ignored のまま。`passed` は `62 passed; 0 failed; 15 ignored; 0 measured; 0 filtered out`（`running 77 tests`） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda` |
| (2) の出力（`lost_state_tests`・`take_stale`） | `12 passed; 0 failed; 0 ignored; 0 measured; 64 filtered out` → `65 filtered out`。`1 passed; 75 filtered out` → `76 filtered out`（passed の数は同じ） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda::lost_state_tests`、同じ組で `--lib backend_cuda::tests::take_stale` |
| (3) の出力（`device_tests::lost`） | `0 passed; 0 failed; 3 ignored; 0 measured; 73 filtered out` → `74 filtered out`（ignored の数は同じ） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda::device_tests::lost` |

## 今の値の補い（base 70ab958）

記録時点の表と既存の節は書き換えない。以下は base `70ab958` で観測した今の値（実行環境は GPU なしの Linux）。

「今の値の補い（base 1417dc5）」の件数と行番号は、その後に足した試験で古くなった。今の値は次のとおり。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「所在」の表の試験の行（「今の値の補い（base 1417dc5）」の module の行番号） | `mod tests`（2032 行目）・`mod device_tests`（3273 行目）・`mod lost`（3519 行目）・`mod lost_state_tests`（3745 行目）・`mod pool_device_tests`（3602 行目）→ `mod tests`（2154 行目）・`mod device_tests`（3395 行目）・`mod lost`（3641 行目）・`mod lost_state_tests`（3867 行目）・`mod pool_device_tests`（3724 行目）。ほかに `mod teardown_tests`（4132 行目）・`mod teardown_device_tests`（4278 行目）・`mod checked_pipeline_device_tests`（4408 行目）がある | `grep -nE '^\s*mod [a-z_]+' crates/contract/src/backend_cuda.rs` |
| `a_panicking_lost_callback_still_delivers_lost_to_completion_callbacks` の所在 | 3307 行目 → 3429 行目 | `grep -n 'a_panicking_lost_callback' crates/contract/src/backend_cuda.rs` |
| 「device の要る試験（3/4）」の `device_tests` の ignored（`backend-cuda,fault-injection --lib backend_cuda::device_tests`／`backend-cuda` のみ） | `15 ignored`／`11 ignored` → 同じ（`running 15 tests`・`0 passed; 0 failed; 15 ignored; 0 measured; 93 filtered out`／`running 11 tests`・`0 passed; 0 failed; 11 ignored; 0 measured; 78 filtered out`） | `cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib backend_cuda::device_tests`、`cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::device_tests` |
| 「世代の読み方と `free` の照合（1/4）」の確かめ方・(1) の行と出力（`--no-default-features --features backend-cuda --lib backend_cuda`） | `50 passed; 0 failed; 11 ignored`（`running 61 tests`）→ `57 passed; 0 failed; 19 ignored; 0 measured; 0 filtered out`（`running 76 tests`） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda` |
| 「模擬のロスト（2/4）」の確かめ方と「3/4」の `ignored` の文（`--no-default-features --features backend-cuda,fault-injection --lib backend_cuda`） | `62 passed; 0 failed; 15 ignored`（`running 77 tests`）→ `69 passed; 0 failed; 26 ignored; 0 measured; 0 filtered out`（`running 95 tests`） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda` |
| (2) の出力（`lost_state_tests`・`take_stale`） | `12 passed; 0 failed; 0 ignored; 0 measured; 65 filtered out`・`1 passed; 76 filtered out` → `83 filtered out`・`94 filtered out`（passed は同じ 12・1） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda::lost_state_tests`、同じ組で `--lib backend_cuda::tests::take_stale` |
| (3) の出力（`device_tests::lost`） | `0 passed; 0 failed; 3 ignored; 0 measured; 74 filtered out` → `92 filtered out`（ignored は同じ 3） | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda,fault-injection --lib backend_cuda::device_tests::lost` |
