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

## 既定の本体（2/6）

### 所在

| 項目 | 値 |
| ---- | -- |
| 定義 | `crates/core/src/execution/submit.rs` の `CommandQueue::dispatch_checked` |
| 公開のパス | `fandhe_silicon_core::execution::CommandQueue` |
| feature の条件 | なし |

### 振る舞い

既定の本体は次の 3 段で動く。

1. 写す: 束縛と定数を、宣言の検査が受ける形（slot と offset、slot とバイト列）に写す。
2. 照らす: 束縛ごとの容量に同じ slot の最初の束縛のハンドルの `size()` を、上限に `max_buffer_bytes()` を渡して `validate_dispatch_operands` を呼ぶ。違反は `dispatch_ex` を呼ばずに `Err` のまま返す。
3. 委ねる: 通れば `dispatch_ex` を 1 回呼び、結果を値のまま返す。

panic する経路は無い。受けた引数は変えずに `dispatch_ex` に渡す。

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --doc execution::submit
```

`dispatch_checked` の例（容量を超える宣言は先頭 `C4:` の `Err` で `dispatch_ex` は 0 回、収まる宣言は `Ok(())` で 1 回）が通る。

## 容量と上限の拒否の試験（3/6）

背後実装を使わない最小の queue で、`dispatch_checked` の既定の本体を外の crate から公開のパスだけで呼ぶ。feature なしで動く。

### 所在

| 項目 | 値 |
| ---- | -- |
| 試験 | `crates/core/tests/dispatch_checked_default.rs` |
| 最小の queue | `dispatch_ex` の呼び出し回数を数え、`max_buffer_bytes` に欄の値を返す。`dispatch_checked` は上書きしない |

### 試験・入力・期待

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `dispatch_checked_rejects_binding_larger_than_buffer_size_with_c4` | 大きさ 16 のハンドル、F32 `[100]`、上限 4096 | 先頭 `C4:` の `InvalidArgument`、`dispatch_ex` は 0 回 |
| `dispatch_checked_rejects_one_byte_over_max_buffer_bytes_with_c8` | 上限 4000、束縛なし、`Bool` `[4001]` | 先頭 `C8:` の `InvalidArgument`、0 回 |
| `dispatch_checked_accepts_required_bytes_equal_to_max_buffer_bytes` | 上限 4000、束縛なし、`Bool` `[4000]` | `Ok(())`、1 回 |
| `dispatch_checked_accepts_binding_that_fits_buffer_size` | 大きさ 16 のハンドル、F32 `[4]` | `Ok(())`、1 回 |
| `dispatch_checked_c4_uses_size_minus_offset` | 大きさ 32 のハンドル、F32 `[4]`、offset 16 と 20 | offset 16 は `Ok(())`、offset 20 は先頭 `C4:` で回数は増えない |
| `dispatch_checked_c4_uses_each_binding_handle_size` | slot 0 が大きさ 16、slot 1 が大きさ 400、宣言は両方 F32 `[100]` | 先頭 `C4:` で文言に `slot 0` を含み 0 回。slot 0 も 400 にすると `Ok(())` で 1 回 |

照合は `InvalidArgument` と文言の先頭までで、文言の全文は固定しない。どの試験も `Result` を受けて `assert!` だけで通る。

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --test dispatch_checked_default
make verify
```

## 検査の順と受け渡しの試験（4/6）

最小の queue と、`dispatch_ex` の引数の写しを積み欄で決めた結果を返す queue で、`dispatch_checked` の既定の本体を外の crate から公開のパスだけで呼ぶ。feature なしで動く。

### 所在

| 項目 | 値 |
| ---- | -- |
| 試験 | `crates/core/tests/dispatch_checked_default.rs` |
| 記録する queue | `dispatch_ex` が受けた引数の写しを積み、欄の結果を返す。`dispatch_checked` は上書きしない |

### 試験・入力・期待

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `dispatch_checked_rejects_other_checks_before_dispatch_ex` | 上限は十分大きい。F32 `[0]`、F32 `[u64::MAX]`、宣言に無い slot の束縛、alignment 16 に対し offset 4、定数が必要バイト数より短い宣言 | 順に先頭 `C8:`・`C8:`・`C7:`・`C5:`・`C6:` の `InvalidArgument`、`dispatch_ex` は 0 回 |
| `dispatch_checked_checks_in_order_with_queue_limit` | 大きさ 16 のハンドル、`Bool` `[4001]`。上限 4000 と 4001 | 上限 4000 は先頭 `C8:`、4001 は先頭 `C4:`。どちらも 0 回 |
| `dispatch_checked_passes_arguments_to_dispatch_ex_unchanged` | 束縛 2 件・定数 1 件・起動の大きさ・共有メモリを持つ通る入力 | `Ok(())`、`dispatch_ex` は 1 回で、受けた引数が渡した値と一致 |
| `dispatch_checked_returns_dispatch_ex_error_unchanged` | 通る入力。`dispatch_ex` が `DeviceMismatch` または `InvalidArgument` を返す | 同じ値が返り、`dispatch_ex` は 1 回 |
| `dispatch_checked_reaches_through_dyn_command_queue` | `&dyn CommandQueue` 越しに大きさ 16 のハンドル、F32 `[100]` | 先頭 `C4:`、0 回 |
| `dispatch_checked_return_type_is_pinned` | 引数 9 個の関数ポインタ型へ代入して呼ぶ | 型が合い、通る入力で `Ok(())`、1 回 |

照合は `InvalidArgument` と文言の先頭までで、`dispatch_ex` の `Err` は値の一致まで見る。

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --test dispatch_checked_default
make verify
```
