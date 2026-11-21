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

どの試験も、拒む呼び出しの前に同じ記録先へ通る dispatch を記録し、拒んだ後の記録先の記録の件数と、投入した後の数え先の値を照らす。

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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | dispatch の dispatch_dims の 0 と上限の照らしを実装する | #5735 |
| 2/3 | dispatch の記録時の照らしの device の要る試験を ignore 付きで足す | #5736 |
