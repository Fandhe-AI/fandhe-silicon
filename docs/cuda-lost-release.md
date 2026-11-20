# CUDA の背後実装の模擬のロストの後の解放の保留と回収（TASK-348）の所在と本リポの現状

TASK-348 の作業のうち、CUDA の背後実装で、模擬のロストの後に、門で止めたコピーに使ったバッファを `free` したときの解放の保留と、門を開けた後の回収を照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-348。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `free` | `crates/contract/src/backend_cuda.rs` の `impl MemoryOps for CudaDevice` の `free` と、関数 `hold_or_release` | `git grep -n "fn free(" -- crates/contract/src/backend_cuda.rs` と `git grep -n "fn hold_or_release" -- crates/contract/src/backend_cuda.rs` |
| 保留の回収 | `crates/contract/src/backend_cuda.rs` の `impl CudaDevice` の `release_finished` と、それを呼ぶ `poll` | `git grep -n "fn release_finished" -- crates/contract/src/backend_cuda.rs` と `git grep -n "fn poll(" -- crates/contract/src/backend_cuda.rs` |
| 模擬のロストと門 | `crates/contract/src/backend_cuda.rs` の `impl FaultHooks for CudaDevice` の `simulate_device_lost`・`arm_gate`・`open_gate` | `git grep -n "fn simulate_device_lost" -- crates/contract/src/backend_cuda.rs` |
| 試験 | `crates/contract/tests/backend_cuda_lost_release.rs` | 下の「device の要る試験（1/2）」 |

## device の要る試験（1/2）

公開の口（`Device::memory`・`Device::queue`・`Device::fault_hooks`）だけを呼ぶ統合の試験。印の写しは `SubmitToken::__backend_new` で作る。本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes` | 3 つ確保して 1 つ目に書き、門を仕込んで 1 つ目から 2 つ目へのコピーを投入し、`simulate_device_lost` の後に 3 つ目・1 つ目・2 つ目を `free` し、`pending_release_count` を読み、ロストの前の印で `poll` を呼び、`open_gate` の後に `poll` を繰り返す | `simulate_device_lost` は `Ok(())`。`free` の後の数は `Some(2)` で、`poll` は `Lost`、その後も `Some(2)`。`free` の後と 2 回目の数の読みの後の、投入からの経過は 250 ms より短い。`open_gate` の後の `poll` はどの回も `Lost` で、数は 10 秒より前に `Some(0)` になる |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_lost_release.rs` |
| `required-features` | `backend-cuda`・`fault-injection`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_lost_release
```

1 件が `ignored` と数えられる。

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-348。ポインタのみ）。

時点のコミット: `e45dc446`。`docs/spec` の gitlink は `96476a8254c0c1fba240acf062644b4433123e99`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `crates/contract/tests/backend_cuda_lost_release.rs` に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n "fn <名前>" -- crates` で引き、`git ls-files` で置き場を引く | 出力 (2) |
| (3) | 照らしの口の所在 | `git grep -n` で `free`・`hold_or_release`・`release_finished`・`poll`・`simulate_device_lost` を 1 つずつ引く | 出力 (3) |
| (4) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 2 つの実行 | 出力 (4) |
| (5) | 依存と `[[test]]` の 1 節 | `5809c267..e45dc446` の差と `make deps-check` | 出力 (5) |
| (6) | unsafe の数と本体の差 | `5809c267..e45dc446` の差と `unsafe` の行の数 | 出力 (6) |
| (7) | CI | 1/2 のマージの後の main の push の run | 出力 (7) |

(5)・(6) の `e45dc446` の親は `5809c267`。1/2 の 1 つ前のコミット。

出力 (1):

```text
$ grep -c '^[[:space:]]*#\[test\]' crates/contract/tests/backend_cuda_lost_release.rs
1
$ grep -c '^[[:space:]]*#\[ignore' crates/contract/tests/backend_cuda_lost_release.rs
1
```

出力 (2):

```text
$ git grep -n "fn buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes" -- crates
crates/contract/tests/backend_cuda_lost_release.rs:131:fn buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes() {
$ git ls-files crates/contract/tests/backend_cuda_lost_release.rs
crates/contract/tests/backend_cuda_lost_release.rs
```

出力 (3):

```text
$ git grep -n "fn free(" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:3083:    fn free(&self, buffer: BufferHandle) {
$ git grep -n "fn hold_or_release" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:2034:fn hold_or_release<B>(
crates/contract/src/backend_cuda.rs:7410:    fn hold_or_release_holds_only_while_the_last_submission_is_unfinished() {
$ git grep -n "fn release_finished" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:2516:    fn release_finished(&self) {
$ git grep -n "fn poll(" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:3270:    fn poll(&self, token: SubmitToken) -> CompletionState {
$ git grep -n "fn simulate_device_lost" -- crates/contract/src/backend_cuda.rs
crates/contract/src/backend_cuda.rs:2858:    fn simulate_device_lost(&self, reason: DeviceLostReason) -> Result<(), ContractError> {
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --test backend_cuda_lost_release
running 1 test
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (5):

```text
$ git diff --stat 5809c267..e45dc446 -- Cargo.lock deps-approved.toml Cargo.toml
$ git diff -U0 5809c267..e45dc446 -- crates/contract/Cargo.toml | grep '^[-+]' | grep -v -e '^+++ ' -e '^--- '
+[[test]]
+name = "backend_cuda_lost_release"
+required-features = ["backend-cuda", "fault-injection"]
+
$ make deps-check
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
```

出力 (6):

```text
$ git diff --stat 5809c267..e45dc446 -- crates/contract/tests/backend_cuda_unsafe_count.rs crates/contract/src crates/core
$ git show 5809c267:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
$ git show e45dc446:crates/contract/src/backend_cuda.rs | grep -c unsafe
6
```

出力 (7):

```text
$ gh run view 44675328056 --json conclusion,headSha,jobs --jq '"\(.conclusion) \(.headSha)", (.jobs[]|select(.name=="make verify (fmt + clippy + test)" or .name=="make verify + build-profiles (macos-latest)" or .name=="make deny (cargo-deny)")|"\(.name) \(.conclusion)")'
success e45dc446d1bfe96e7c1049a27accd8aa141fd7ef
make verify + build-profiles (macos-latest) success
make verify (fmt + clippy + test) success
make deny (cargo-deny) success
$ gh run view 44675328056 --job 136085675961 --log | grep -oE 'test buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes \.\.\. ignored, requires a CUDA device' | sort | uniq -c
      2 test buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes ... ignored, requires a CUDA device
```

## device の要る試験の強め（3/4）

上の「device の要る試験（1/2）」の節の表と文は記録時点の値で、書き換えていない。3/4 の後の試験の形は次のとおり。公開の口（`Device::memory`・`Device::queue`・`Device::fault_hooks`）だけを呼び、印は `submit` が返した値をそのまま `poll` に渡す。本体を別のスレッドで流し、30 秒で打ち切る。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `buffers_freed_after_a_loss_stay_held_until_the_gated_copy_completes` | 3 つ確保して 1 つ目に書き、門を仕込んで 1 つ目から 2 つ目へのコピーを投入し、`simulate_device_lost` の後に 3 つ目・1 つ目・2 つ目を `free` してそれぞれの後に `pending_release_count` を読み、ロストの前の印で `poll` を呼び、`open_gate` の後に `poll` を繰り返す | `simulate_device_lost` は `Ok(())`。`free` ごとの数は `Some(0)`・`Some(1)`・`Some(2)`。`poll` は `Lost` で、その後も `Some(2)`。`open_gate` の後の `poll` はどの回も `Lost` で、数は `Some(0)` になる。門を仕込む前からの経過は、3 つ目の `free` の後の数の読みの後、`poll` の後の数の読みの後、`open_gate` の後の数の読みのたび（`Some(0)` を読んだ回を含む）に 250 ms より短い |

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/contract/tests/backend_cuda_lost_release.rs` |
| `required-features` | `backend-cuda`・`fault-injection`（`crates/contract/Cargo.toml` の `[[test]]`） |
| `#[ignore]` | `requires a CUDA device` |

確かめのコマンドは上の「確かめ方」と同じで、1 件が `ignored` と数えられる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 模擬のロストの後の解放の保留と回収の device の要る試験を ignore 付きで足す | #4475 |
| 2/2 | 完了の条件の確かめ | #4476 |
| 3/4 | 模擬のロストの後の解放の保留と回収の device の要る試験を強める | #8407 |
