# CUDA の背後実装の解放の保留の数・ゼロ化の完了待ち・回収の点（TASK-346）の所在と本リポの現状

TASK-346 の作業のうち、CUDA の背後実装の解放の保留の数（`pending_release_count`）・投入済みの表の件数・ゼロ化の完了を待つバッファと、保留したバッファを回収する呼び出しを照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-346。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `pending_release_count` | `crates/contract/src/backend_cuda.rs` の `impl MemoryOps for CudaDevice` の `pending_release_count` | `git grep -n "fn pending_release_count" -- crates/contract/src/backend_cuda.rs` |
| 解放の保留と門 | [cuda-fault-gate](cuda-fault-gate.md) | 同左 |
| 再利用の一覧とゼロ化 | [cuda-buffer-pool](cuda-buffer-pool.md) | 同左 |
| 試験 | `crates/contract/src/backend_cuda.rs` の `release_backlog_device_tests` | 下の「device の要る試験（1/3）」 |

## device の要る試験（1/3）

`crates/contract/src/backend_cuda.rs` の module `release_backlog_device_tests`（`#[cfg(all(test, feature = "fault-injection"))]`）。crate の中の投入済みの表と再利用の一覧の件数も読む。どの試験も本体を別のスレッドで流し、上限の時間で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `the_backlog_stays_within_nine_over_ten_thousand_rounds_and_ends_at_zero` | 確保・0 埋めの記録と投入・`free` を 10,000 回くり返す。各回の初めに、待っていない印が 8 つあれば、いちばん古い印を `poll` で `Complete` まで待つ。くり返しの後に残りの印を同じく待ち、最後の印にもう 1 回 `poll` を呼ぶ | 各回の後の `pending_release_count` は `Some` で 9 以下、投入済みの表の件数は 9 以下。最後の `poll` は `Complete` で、その後の `pending_release_count` は `Some(0)`、投入済みの表の件数は 0。2 つの数の最大を `max_pending_release_count=`・`max_submissions=` の 1 行で標準出力に出す。上限は 120 秒 |
| `buffers_waiting_for_zeroing_are_not_counted` | 門を仕込んで 0 埋めを投入し、そのバッファを `free` する。門を開けて別のバッファへ `upload` し、門を仕込んで空の投入をする。門を開けて空の投入を `Complete` まで待ち、`upload` の後に同じ長さ・用途で確保する | `free` の後は `Some(1)` で、ゼロ化の完了を待つバッファは 0。`upload` の後も `Some(1)`。空の投入の後は `Some(0)` で、ゼロ化の完了を待つバッファは 1、空の投入の `poll` は `Pending`、前の投入の `poll` は `Complete`。2 回目に門を仕込んでからここまでは 250 ms より早く終わる。確保の後は、ゼロ化の完了を待つバッファは 0 で、`Some(0)`。上限は 30 秒 |

| 項目 | 値 |
| ---- | -- |
| module | `release_backlog_device_tests` |
| `#[ignore]` | `requires a CUDA device` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib release_backlog_device_tests
```

2 件が `ignored` と数えられる。

## device の要る試験（2/3）

公開の口（`Device::memory`・`Device::queue`・`Device::fault_hooks`）だけを呼ぶ統合の試験。どの試験も本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `more_than_nine_unfinished_submissions_are_held_without_waiting` | 32 個のバッファを確保してから門を仕込み、バッファごとに 0 埋めを記録して投入し、`free` する。全部の印に `poll` を呼び、門を開けて全部の印を `poll` で `Complete` まで待ち、最後の印にもう 1 回 `poll` を呼ぶ | 門を仕込む前は `Some(0)`。32 回の投入と `free` は 250 ms より早く終わり、どの投入も成功する。その後は `Some(32)` で、どの印の `poll` も `Pending`、その後も `Some(32)` で、門を仕込んでからここまでは 250 ms より早く終わる。最後の `poll` は `Complete` で、その後は `Some(0)` |
| `held_buffers_are_released_by_each_of_the_five_calls` | `submit`・`poll`・`poll_ex`・`wait_blocking`（上限 10 秒）・`allocate` の 5 通りに、門を仕込んで 0 埋めを投入し、そのバッファを `free` して門を開け、別のバッファへ `upload` して 50 ms 待ってから、その 1 つを呼ぶ | `free` の後は `Some(1)`。`upload` と 50 ms の後も `Some(1)`。`poll` は `Complete`、`poll_ex` は `Complete`、`wait_blocking` は `Ok(())`。呼んだ後は 5 通りとも `Some(0)` |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_release_backlog.rs` |
| `required-features` | `backend-cuda`・`fault-injection`・`native-blocking-wait`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_release_backlog
```

2 件が `ignored` と数えられる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-346。ポインタのみ）。

時点のコミット: `88b8312c`。`docs/spec` の gitlink は `bfc545a1462bd69953ff876a1ff231348d3352f8`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | module `release_backlog_device_tests` と `crates/contract/tests/backend_cuda_release_backlog.rs` の行頭の `#[test]`・`#[ignore` を `grep -c` で数える | 下の出力 (1) |
| (2) | 受け手の試験の名前 | 4 本の関数を 1 つずつ `git grep -n "fn <名前>" -- crates` で引く | 下の出力 (2) |
| (3) | 数の口の本体 | `git grep -n "fn pending_release_count"` を `crates/contract/src/backend_cuda.rs` と `crates/core/src/memory/ops.rs` に当てる | 下の出力 (3) |
| (4) | device に触らない試験 | `--lib release_backlog_device_tests`・`--test backend_cuda_release_backlog`・`--test backend_cuda_unsafe_count` を流す | 下の出力 (4) |
| (5) | 依存 0 と `[[test]]` の 1 節 | `63683abe` から HEAD までの `Cargo.lock`・`deps-approved.toml`・`Cargo.toml` の差分、`crates/contract/Cargo.toml` の差分の行、`make deps-check` | 下の出力 (5) |
| (6) | unsafe の数と本体の差 | `63683abe` から HEAD までの `backend_cuda_unsafe_count.rs`・`crates/core`・`crates/contract/src` の差分と、`backend_cuda.rs` の `unsafe` の行数 | 下の出力 (6) |
| (7) | CI | 時点のコミットの main の push の run のジョブの conclusion と headSha、`verify` のログの 4 本の `ignored` の行 | 下の出力 (7) |

(5)・(6) の `63683abe` は、1/3 の 1 つ前のコミット。

出力 (1):

```text
$ awk '/^mod release_backlog_device_tests \{$/{f=1} f' crates/contract/src/backend_cuda.rs | grep -c '^[[:space:]]*#\[test\]'
2
$ awk '/^mod release_backlog_device_tests \{$/{f=1} f' crates/contract/src/backend_cuda.rs | grep -c '^[[:space:]]*#\[ignore'
2
$ grep -c '^[[:space:]]*#\[test\]' crates/contract/tests/backend_cuda_release_backlog.rs
2
$ grep -c '^[[:space:]]*#\[ignore' crates/contract/tests/backend_cuda_release_backlog.rs
2
```

出力 (2):

```text
$ git grep -n "fn the_backlog_stays_within_nine_over_ten_thousand_rounds_and_ends_at_zero" -- crates
crates/contract/src/backend_cuda.rs:8027:    fn the_backlog_stays_within_nine_over_ten_thousand_rounds_and_ends_at_zero() {
$ git grep -n "fn buffers_waiting_for_zeroing_are_not_counted" -- crates
crates/contract/src/backend_cuda.rs:8079:    fn buffers_waiting_for_zeroing_are_not_counted() {
$ git grep -n "fn more_than_nine_unfinished_submissions_are_held_without_waiting" -- crates
crates/contract/tests/backend_cuda_release_backlog.rs:132:fn more_than_nine_unfinished_submissions_are_held_without_waiting() {
$ git grep -n "fn held_buffers_are_released_by_each_of_the_five_calls" -- crates
crates/contract/tests/backend_cuda_release_backlog.rs:178:fn held_buffers_are_released_by_each_of_the_five_calls() {
```

出力 (3):

```text
$ git grep -n "fn pending_release_count" -- crates/contract/src/backend_cuda.rs crates/core/src/memory/ops.rs
crates/contract/src/backend_cuda.rs:3036:    fn pending_release_count(&self) -> Option<u64> {
crates/core/src/memory/ops.rs:96:    fn pending_release_count(&self) -> Option<u64> {
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib release_backlog_device_tests
running 2 tests
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 162 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection,native-blocking-wait --test backend_cuda_release_backlog
running 2 tests
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (5):

```text
$ git diff --stat 63683abe..HEAD -- Cargo.lock deps-approved.toml Cargo.toml
$ git diff -U0 63683abe..HEAD -- crates/contract/Cargo.toml | grep '^[-+]' | grep -v -e '^+++ ' -e '^--- '
+[[test]]
+name = "backend_cuda_release_backlog"
+required-features = ["backend-cuda", "fault-injection", "native-blocking-wait"]
+
$ make deps-check
終了コード 0
```

出力 (6):

```text
$ git diff --stat 63683abe..HEAD -- crates/contract/tests/backend_cuda_unsafe_count.rs crates/core
$ git diff -U0 63683abe..HEAD -- crates/contract/src | grep '^@@'
@@ -7912,0 +7913,201 @@ mod wait_blocking_tests {
$ git show 63683abe:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show HEAD:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (7):

```text
$ gh run view 44618302645 --json conclusion,headSha,jobs
conclusion: success
headSha: 88b8312c2d96f17288c9afa0d8a988141f6a6d6f
make verify (fmt + clippy + test): success
make verify + build-profiles (macos-latest): success
make deny (cargo-deny): success
$ gh run view 44618302645 --job <verify のジョブ> --log | grep -E 'test ([a-z_]+::)*(<4 本の名前>) \.\.\. ignored, requires a CUDA device' | sed 's/^.*Z //' | sort | uniq -c
      2 test backend_cuda::release_backlog_device_tests::buffers_waiting_for_zeroing_are_not_counted ... ignored, requires a CUDA device
      2 test backend_cuda::release_backlog_device_tests::the_backlog_stays_within_nine_over_ten_thousand_rounds_and_ends_at_zero ... ignored, requires a CUDA device
      2 test held_buffers_are_released_by_each_of_the_five_calls ... ignored, requires a CUDA device
      2 test more_than_nine_unfinished_submissions_are_held_without_waiting ... ignored, requires a CUDA device
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 解放の保留の数とゼロ化の完了待ちの device の要る試験を ignore 付きで足す | #4463 |
| 2/3 | 上限を超える負荷と回収の点の device の要る試験を ignore 付きで足す | #4464 |
| 3/3 | 完了の条件の確かめ | #4465 |
