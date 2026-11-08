# CUDA の背後実装の device の破棄（TASK-191）の所在と本リポの現状

TASK-191 の作業のうち、device の明示の破棄の口の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-191。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/core/src/device.rs` の `Device::destroy` | `crates/core/src/device.rs` |
| cfg | 条件なし（どの feature の組にもある） | 同上 |
| 試験 | `crates/core/tests/device_destroy.rs` と `Device::destroy` の doctest | 下の「確かめ方」 |

## device の明示の破棄の口（1/5）

- `Device::destroy(self: Box<Self>)` は device の値を消費し、値を返さない
- 既定の本体は値を落とす（`Drop` と同じ）。背後実装が上書きできる
- 2 回目の呼び出しはコンパイルで拒まれる（doctest `compile_fail,E0382`。`make doctest-error-code-check` が照合する）

| 試験 | 確かめること |
| ---- | ------------ |
| `the_default_destroy_drops_the_device_once` | 既定の本体で `Box<dyn Device>` から `destroy()` を呼ぶと、`Drop` が 1 回走る |
| `an_overriding_destroy_is_reached_through_box_dyn_device` | 上書きした `destroy` が `Box<dyn Device>` 越しに届き、`Drop` が 1 回走る |

確かめ方:

```bash
cargo test -p fandhe-silicon-core --test device_destroy
cargo test -p fandhe-silicon-core --doc device
make doctest-error-code-check
```

## `Drop` の同期と解放（2/5）

| 項目 | 事実 |
| ---- | ---- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `impl Drop for CudaDevice` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` |
| 試験 | `backend_cuda.rs` の `mod teardown_tests`（device に触らない） |

`Drop` の順:

| 順 | 動き |
| -- | ---- |
| 1 | stream を同期する |
| 2 | 登録済みの完了の callback を、同期の結果に応じた値で 1 回ずつ呼ぶ（錠を持たずに呼ぶ） |
| 3 | 再利用の一覧を含むバッファの表・パイプラインの表・投入の表を解放する |

同期の結果ごとの扱い:

| 同期の結果 | callback の値 | 解放 |
| ---------- | ------------- | ---- |
| `Ok` | `poll` と同じ判定の値（`Complete` または `Lost`）。判定が `Pending` のままのものは `Lost` | 3 つの表を解放する |
| `Err`・panic | `Lost` | 3 つの表と stream を解放しない |

- ロストの通知は呼ばない
- `Drop` から panic を外へ出さない

| 試験 | 確かめること |
| ---- | ------------ |
| `release_if_drops_only_after_a_confirmed_sync` | 同期が `Ok` のときだけ値を落とす。`Err`・panic では落とさない |
| `release_if_does_not_unwind_when_a_drop_panics` | 落とす途中の panic が外へ出ない |
| `teardown_ready_judges_like_poll_after_a_confirmed_sync` | 同期を確かめた後は `Complete`・`Lost`・`Lost` を返し、表の callback が空になる |
| `teardown_ready_delivers_lost_without_a_confirmed_sync` | 同期を確かめられなければ `Lost` を返し、表の callback が空になる |

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda
```

## 明示の破棄（3/5）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `impl Device for CudaDevice` の `destroy` と、順を受け持つ私的な関数 `destroy_in_order` |
| cfg | `backend-cuda` かつ `not(target_arch = "wasm32")` |
| 試験 | 同ファイルの `mod teardown_tests` |

`destroy` の順は次のとおり。

| 順 | 動き |
| -- | ---- |
| 1 | 登録済みの完了の callback を待たずに `Lost` で呼び切る |
| 2 | stream を同期する |
| 3 | ロストの通知。`fault-injection` のビルドだけで、`Destroyed` で 1 回。先にロストしていれば呼ばない |
| 4 | 解放（2/5 の `Drop` の流れ） |

panic は最初の 1 つ（完了の callback のもの。無ければ通知のもの）を、解放の後に再送出する。

`Drop` との違いは次のとおり。

| 項目 | `Drop` | `destroy` |
| ---- | ------ | --------- |
| ロストの通知 | 呼ばない | `fault-injection` のビルドで 1 回 |
| callback の値 | 同期の結果に応じた値 | `Lost` |
| panic | 外へ出さない | 最初の 1 つを再送出する |

| 試験 | 確かめること |
| ---- | ------------ |
| `destroy_steps_run_in_order` | 4 つの段が `flush`・`sync`・`notify`・`release` の順に 1 回ずつ呼ばれる |
| `destroy_steps_return_the_first_panic_after_release` | 2 つの段が panic の中身を返すと、解放の後に最初の段のものを返す |
| `destroy_steps_return_the_notify_panic_when_flush_has_none` | 最初の段に panic が無ければ、通知の段のものを返す |

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda
cargo test -p fandhe-silicon-contract --features backend-cuda,fault-injection --lib backend_cuda
```

## device の要る試験（4/5）

| 項目 | 内容 |
| ---- | ---- |
| 実装の置き場 | `crates/contract/src/backend_cuda.rs` の `mod teardown_device_tests` と `crates/contract/tests/backend_cuda_destroy.rs` |
| cfg | module は `backend-cuda`。`lost_notify` の 3 本は加えて `fault-injection`。統合試験は `[[test]]` の `required-features = ["backend-cuda"]` |

準備は、長さ 4097 のバッファを 3 本確保して書き込み、1 本を `free` し、空の記録先を投入する。
どの試験も `#[ignore = "requires a CUDA device"]` 付きで、既定では流れない。

| 試験 | 確かめること |
| ---- | ------------ |
| `dropping_right_after_submit_records_no_driver_error` | 投入の直後に落としても、context にドライバのエラーが残らない |
| `dropping_right_after_on_complete_calls_it_once` | 完了の callback を登録した直後に落とすと、`Complete` で 1 回だけ呼ばれる |
| `destroying_right_after_submit_records_no_driver_error` | 投入の直後に `destroy` しても、ドライバのエラーが残らない |
| `destroying_right_after_on_complete_calls_it_once` | callback を登録した直後に `destroy` すると、`Pending` でない値で 1 回だけ呼ばれる |
| `destroy_notifies_destroyed_once` | `destroy` はロストの通知を `Destroyed` で 1 回呼ぶ |
| `destroy_after_a_simulated_loss_does_not_notify_again` | 先にロストしていれば、`destroy` は通知を重ねない |
| `drop_does_not_notify` | `Drop` はロストの通知を呼ばない |
| `a_requested_device_can_be_destroyed` | `request` で得た device で確保・投入の後に `destroy` が戻る |

確かめ方:

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait,fault-injection --lib backend_cuda::teardown_device_tests
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_destroy
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | device の明示の破棄の口 | #3854 |
| 2/5 | `Drop` の同期と解放 | #3855 |
| 3/5 | 明示の破棄 | #3856 |
| 4/5 | device の要る試験 | #3857 |
