# CUDA の背後実装の dispatch の記録時の照らし（TASK-806）の所在と本リポの現状

TASK-806 の作業のうち、CUDA の背後実装の `CommandQueue::dispatch_ex`（簡易版の `dispatch` を含む）が記録の時点に `dispatch_dims` を照らす形と、記録の時点の拒否を確かめる試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-806。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/contract/src/backend_cuda.rs` の `impl CommandQueue for CudaDevice` の `dispatch_ex`。`dispatch` は上書きしない | `git grep -n -F -e "fn dispatch_ex(" -e "fn dispatch(" -- crates/contract/src/backend_cuda.rs` |
| `dispatch_dims` の照らし | 同じファイルの `check_dispatch_dims`。`dispatch_ex` は上限に `limits().max_dispatch_dims` と同じ値を渡す | `git grep -n -F -e "fn check_dispatch_dims(" -e "check_dispatch_dims(dispatch_dims, self.limits.max_dispatch_dims)?;" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `dispatch_check_tests` module。device に触らない | 下の「確かめ方」 |

## dispatch_dims の 0 と上限の照らし（1/3）

`check_dispatch_dims` は `dispatch_dims` と軸ごとの上限を受け、最初に当たったものを `InvalidArgument` で返す。

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| 各軸が上限ちょうどまで | 通る | `dispatch_dims_pass_up_to_the_limit_on_each_axis` |
| どれかの軸が 0 | `dispatch_dims contains zero`。上限を超える軸があっても、この文を返す | `dispatch_dims_reject_a_zero_on_any_axis_before_the_limit` |
| どれかの軸が上限を超える | `dispatch_dims exceeds the limit` | `dispatch_dims_reject_one_more_than_the_limit_on_each_axis` |

試験が渡す上限は `[2147483647, 65535, 65535]`・`[u32::MAX; 3]`・`[4, 4, 4]`。

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_check_tests
```

3 件が通る。

## device の要る試験（2/3）

device に触る試験は `crates/contract/src/backend_cuda.rs` の `dispatch_check_device_tests` module にあり、どれも `#[ignore = "requires a CUDA device"]` を付けている。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。カーネルは試験が CUDA C で渡し（各スレッドが数え先に 1 を足す）、`create_pipeline` が NVRTC で PTX にする。

記録先の試験のほかは、拒む呼び出しの前に同じ記録先へ通る dispatch を記録し、拒んだ後の記録先の記録の件数と、投入した後の数え先の値を照らす。記録先の試験は、別の記録先へ通る dispatch を記録して投入してから開いていない記録先を渡し、渡した記録先の記録が 0 件であることと数え先の値を照らす。

| 試験 | 呼ぶ口と入力 | 照らす値 |
| ---- | ------------ | -------- |
| `dispatch_and_dispatch_ex_reject_a_forged_pipeline_before_recording` | 内部 id 999,999 の偽造したパイプラインを渡す `dispatch` と、同じパイプラインに 0 を含む `dispatch_dims` を渡す `dispatch_ex` | どちらも `dispatch_ex: unknown pipeline`。記録は 1 件のまま、数え先は 4 |
| `dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_with_the_same_id` | 別の device で作った、自 device のパイプラインと同じ内部 id のパイプラインを渡す `dispatch` と `dispatch_ex` | どちらも `DeviceMismatch`。記録は 1 件のまま、数え先は 4 |
| `dispatch_and_dispatch_ex_reject_an_encoder_that_is_not_open` | 作っていない id の記録先・投入した記録先と同じ id の値・別の device の記録先を渡す `dispatch` と `dispatch_ex` | どれも `encoder is not open on this device`。どの記録先の記録も 0 件、数え先は 4 |
| `dispatch_and_dispatch_ex_reject_dispatch_dims_with_a_zero_before_recording` | x・y・z のどれかが 0 の `dispatch_dims` を渡す `dispatch` と `dispatch_ex`。表に無いバッファと 0、渡さない slot と 0 の組を渡す `dispatch_ex` | 0 は `dispatch_dims contains zero`。表に無いバッファとの組は `dispatch_ex: unknown buffer handle`、渡さない slot との組は `dispatch_dims contains zero`。記録は 1 件のまま、数え先は 4 |
| `dispatch_and_dispatch_ex_record_the_y_and_z_limit_and_reject_one_more` | `limits().max_dispatch_dims` の y・z。`[1, 65535, 1]` の `dispatch` と `[1, 1, 65535]` の `dispatch_ex`。x が `limits().max_dispatch_dims` の x を 1 超える値・`[1, 65536, 1]`・`[1, 1, 65536]` の `dispatch` と `dispatch_ex`。渡さない slot と `[1, 65536, 1]` の組の `dispatch_ex` | y・z の上限は 65535。65535 の 2 件は記録し、投入した後の数え先はどちらも 65535。上限を超える値と組は `dispatch_dims exceeds the limit` で、記録は 2 件のまま |

| 項目 | 値 |
| ---- | -- |
| module | `crates/contract/src/backend_cuda.rs` の `dispatch_check_device_tests` |
| feature | `backend-cuda`・`native-blocking-wait` |
| `#[ignore]` | `requires a CUDA device` |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --lib backend_cuda::dispatch_check_device_tests
```

5 件が `ignored` と数えられる。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-806。ポインタのみ）。

時点のコミット: `acf00420`。`docs/spec` の gitlink は `ac56f928301ffca0c15d857b56302916a3cce0fb`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `acf00420` の `crates/contract/src/backend_cuda.rs` の `dispatch_check_tests`・`dispatch_check_device_tests` に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n` で 8 本の名前を 1 つずつ引く | 出力 (2) |
| (3) | 口の所在 | `git grep -n -F` で `fn check_dispatch_dims(`・`check_dispatch_dims(dispatch_dims, self.limits.max_dispatch_dims)?;`・`max_dispatch_dims: grid_dims,`・`fn ensure_encoder_open(` を 1 つずつ引く | 出力 (3) |
| (4) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 2 つの実行 | 出力 (4) |
| (5) | 依存の差 | `6ea39955^ acf00420` の差と `make deps-check` | 出力 (5) |
| (6) | CI | 2/3 のマージの後の main の push の run | 出力 (6) |

`6ea39955^` は 1/3 の squash `6ea39955` の親。(4) の `cargo test` と (5) の `make deps-check` は作業木で流した。

出力 (1):

```text
$ git show acf00420:crates/contract/src/backend_cuda.rs | awk '/^mod dispatch_check_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
3
$ git show acf00420:crates/contract/src/backend_cuda.rs | awk '/^mod dispatch_check_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
5
$ git show acf00420:crates/contract/src/backend_cuda.rs | awk '/^mod dispatch_check_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
5
```

出力 (2):

```text
$ git grep -n "fn dispatch_dims_pass_up_to_the_limit_on_each_axis(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10380:    fn dispatch_dims_pass_up_to_the_limit_on_each_axis() {
$ git grep -n "fn dispatch_dims_reject_a_zero_on_any_axis_before_the_limit(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10396:    fn dispatch_dims_reject_a_zero_on_any_axis_before_the_limit() {
$ git grep -n "fn dispatch_dims_reject_one_more_than_the_limit_on_each_axis(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10415:    fn dispatch_dims_reject_one_more_than_the_limit_on_each_axis() {
$ git grep -n "fn dispatch_and_dispatch_ex_reject_a_forged_pipeline_before_recording(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10529:    fn dispatch_and_dispatch_ex_reject_a_forged_pipeline_before_recording() {
$ git grep -n "fn dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_with_the_same_id(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10566:    fn dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_with_the_same_id() {
$ git grep -n "fn dispatch_and_dispatch_ex_reject_an_encoder_that_is_not_open(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10605:    fn dispatch_and_dispatch_ex_reject_an_encoder_that_is_not_open() {
$ git grep -n "fn dispatch_and_dispatch_ex_reject_dispatch_dims_with_a_zero_before_recording(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10648:    fn dispatch_and_dispatch_ex_reject_dispatch_dims_with_a_zero_before_recording() {
$ git grep -n "fn dispatch_and_dispatch_ex_record_the_y_and_z_limit_and_reject_one_more(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:10703:    fn dispatch_and_dispatch_ex_record_the_y_and_z_limit_and_reject_one_more() {
```

出力 (3):

```text
$ git grep -n -F "fn check_dispatch_dims(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:1535:fn check_dispatch_dims(
$ git grep -n -F "check_dispatch_dims(dispatch_dims, self.limits.max_dispatch_dims)?;" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:3851:        check_dispatch_dims(dispatch_dims, self.limits.max_dispatch_dims)?;
$ git grep -n -F "max_dispatch_dims: grid_dims," acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:578:                max_dispatch_dims: grid_dims,
$ git grep -n -F "fn ensure_encoder_open(" acf00420 -- crates/contract/src/backend_cuda.rs
acf00420:crates/contract/src/backend_cuda.rs:2916:    fn ensure_encoder_open(&self, encoder: &CommandEncoder) -> Result<(), ContractError> {
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_check_tests 2>&1 | grep -E '^(running|test result:)'
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 146 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (5):

```text
$ git diff --stat 6ea39955^ acf00420 -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml crates/core
$ make deps-check; echo exit=$?
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
exit=0
```

出力 (6):

```text
$ gh run list --branch main --event push --commit acf00420de903ab09e872972b242d564509530be --workflow ci.yml --json databaseId --jq '.[].databaseId'
44868855660
$ gh run view 44868855660 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
acf00420de903ab09e872972b242d564509530be
make verify (fmt + clippy + test): success
make deny (cargo-deny): success
make verify + build-profiles (macos-latest): success
$ gh run view 44868855660 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136738592394
$ gh run view 44868855660 --job 136738592394 --log | grep -oE 'test backend_cuda::dispatch_check_(device_)?tests::[a-z_]+ \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      2 test backend_cuda::dispatch_check_device_tests::dispatch_and_dispatch_ex_record_the_y_and_z_limit_and_reject_one_more ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_check_device_tests::dispatch_and_dispatch_ex_reject_a_forged_pipeline_before_recording ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_check_device_tests::dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_with_the_same_id ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_check_device_tests::dispatch_and_dispatch_ex_reject_an_encoder_that_is_not_open ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_check_device_tests::dispatch_and_dispatch_ex_reject_dispatch_dims_with_a_zero_before_recording ... ignored, requires a CUDA device
      6 test backend_cuda::dispatch_check_tests::dispatch_dims_pass_up_to_the_limit_on_each_axis ... ok
      6 test backend_cuda::dispatch_check_tests::dispatch_dims_reject_a_zero_on_any_axis_before_the_limit ... ok
      6 test backend_cuda::dispatch_check_tests::dispatch_dims_reject_one_more_than_the_limit_on_each_axis ... ok
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | dispatch の dispatch_dims の 0 と上限の照らしを実装する | #5735 |
| 2/3 | dispatch の記録時の照らしの device の要る試験を ignore 付きで足す | #5736 |
| 3/3 | 完了の条件の確かめ | #5737 |
