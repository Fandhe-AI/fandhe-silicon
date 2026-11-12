# 投入の前の検査の既定の本体と上限を返す必須のメソッド（TASK-569）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-569。

## 上限を返す必須のメソッド（1/6）

### 所在

| 項目 | 値 |
| ---- | -- |
| 定義 | `crates/core/src/execution/submit.rs` の `CommandQueue::max_buffer_bytes` |
| 公開のパス | `fandhe_silicon_core::execution::CommandQueue` |
| feature の条件 | なし |

### 振る舞い

`fn max_buffer_bytes(&self) -> u64` は投入の前の検査が上限に使う値（バイト）を返す。
既定の本体は無く、`CommandQueue` を実装する型はすべて持つ。持たない実装は `E0046` で拒まれる。

### 実装ごとに返す値

| 実装 | 返す値 |
| ---- | ------ |
| 試験用の device（`crates/core/src/stub_device.rs`） | `LIMITS.max_buffer_bytes` |
| `backend_metal.rs`・`backend_cuda.rs` | `self.limits.max_buffer_bytes`（`limits()` の `max_buffer_bytes` と同じ） |
| 試験の偽の queue | `0` |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --doc execution::submit
```

`dispatch_ex` の 2 つの例（欠落の例と陽性対照）と `max_buffer_bytes` の欠落の例が通る。
