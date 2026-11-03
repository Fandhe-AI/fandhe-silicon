# 描画パスの状態の値の型と範囲の型と検査（TASK-838）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-838。

## 所在

- 実装: `crates/core/src/graphics/pass_state.rs`・`crates/core/src/graphics/draw_range.rs`
- 公開のパス: `fandhe_silicon_core::graphics::…`（feature `graphics` の裏。`graphics` モジュールは [graphics-types.md](graphics-types.md)、TASK-837）

## 描画パスの状態の値の型と範囲の型と検査（1/4）

### 型

| 型 | 欄・値 | derive |
| -- | ------ | ------ |
| `Viewport` | `x`・`y`・`width`・`height`・`min_depth`・`max_depth`（すべて `f32`、公開） | `Debug`・`Clone`・`Copy`・`PartialEq` |
| `ScissorRect` | `x`・`y`・`width`・`height`（すべて `u32`、公開） | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` |
| `IndexFormat` | `Uint16`・`Uint32`（`#[non_exhaustive]`。`Default` は `Uint32`） | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash`・`Default` |
| `DrawRange` | `first_vertex`・`vertex_count`・`first_instance`・`instance_count`（すべて `u32`、公開） | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` |
| `IndexedDrawRange` | `first_index`・`index_count`（`u32`）・`base_vertex`（`i32`）・`first_instance`・`instance_count`（`u32`）。公開 | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` |

`index_format_bytes` は `Uint16` で 2、`Uint32` で 4 を返す。

### 検査の関数

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `validate_draw_range(&DrawRange)` | `first_vertex + vertex_count` が `u32` に収まり、`first_instance + instance_count` も収まる（数 0 も同じ規則） | `Ok(())` |
| 同上 | どちらかの和があふれる | `Err(ContractError::InvalidArgument)` |
| `validate_indexed_draw_range(&IndexedDrawRange, IndexFormat, offset, bound_end)` | 2 つの和が収まり、`offset + (first_index + index_count) * インデックス 1 個のバイト数` が `bound_end` 以下（ちょうども通す） | `Ok(())` |
| 同上 | どちらかの和のあふれ、`u64` の足し算のあふれ、終わりが `bound_end` 超え | `Err(ContractError::InvalidArgument)` |

`base_vertex` の値と `offset` の倍数は結果に影響しない。計算は `checked_*` で行う。

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --lib graphics::
cargo test -p fandhe-silicon-core --features graphics --doc graphics
```
