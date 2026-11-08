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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | device の明示の破棄の口 | #3854 |
