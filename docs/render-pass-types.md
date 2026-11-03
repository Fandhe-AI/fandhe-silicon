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

## 描画パスの状態を設定する操作と試験用の device の返し（2/4）

### 所在

- 操作: `crates/core/src/graphics.rs` の `GraphicsOps`（feature `graphics`）。
- 試験用の device: `crates/core/src/stub_device.rs` の `StubGraphics`。
- 外の crate の局所の実装例: `crates/core/tests/graphics_types.rs` の `FixedGraphics`（8 操作とも `Ok(())`）。

### 操作

どれも既定の本体を持たず、実装する側が 8 操作とも実装する。

| 操作 | 引数 | 戻り値 |
| ---- | ---- | ------ |
| `set_pipeline` | `&mut RenderEncoder`・`&PipelineHandle` | `Result<(), ContractError>` |
| `set_vertex_buffer` | `&mut RenderEncoder`・`slot: u32`・`&BufferHandle`・`offset: u64`・`size: Option<u64>` | 同上 |
| `set_index_buffer` | `&mut RenderEncoder`・`&BufferHandle`・`IndexFormat`・`offset: u64`・`size: Option<u64>` | 同上 |
| `set_immediates` | `&mut RenderEncoder`・`offset: u32`・`data: &[u8]` | 同上 |
| `set_viewport` | `&mut RenderEncoder`・`Viewport` | 同上 |
| `set_scissor_rect` | `&mut RenderEncoder`・`ScissorRect` | 同上 |
| `set_blend_constant` | `&mut RenderEncoder`・`[f32; 4]` | 同上 |
| `set_stencil_reference` | `&mut RenderEncoder`・`u32` | 同上 |

### 試験用の device の返し

試験用の device の振る舞い。記録先以外の引数は照らさず、状態も持たない。

| 入力 | `set_pipeline` | ほかの 7 操作 |
| ---- | -------------- | ------------- |
| 生きた記録先 | `Unsupported`（`op` は操作名） | 同左 |
| 拒否の値を持つ記録先 | その値（別 device の描画先から作ると `DeviceMismatch`） | 同左 |
| 別 device の記録先 | `DeviceMismatch` | 同左 |
| 破棄した描画先の記録先 | `InvalidArgument` | 同左 |
| 別 device のパイプライン | `DeviceMismatch` | 該当なし |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features test-support,graphics --lib stub_device::
cargo test -p fandhe-silicon-core --features graphics --test graphics_types
cargo test -p fandhe-silicon-core --features graphics --doc graphics
```
