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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | device の明示の破棄の口 | #3854 |
| 2/5 | `Drop` の同期と解放 | #3855 |
