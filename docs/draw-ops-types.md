# 描画の操作の型（TASK-839）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-839。

## 所在

- 実装: `crates/core/src/graphics.rs` の `GraphicsOps`（feature `graphics` の裏。`graphics` モジュールは [graphics-types.md](graphics-types.md)、TASK-837）
- 試験用の device: `crates/core/src/stub_device.rs` の `StubGraphics`（[stub-device.md](stub-device.md)、TASK-156）

## mesh shader の描画の操作と試験用の device の返し（1/3）

### 所在

| 内容 | 場所 |
| ---- | ---- |
| 4 操作の宣言 | `crates/core/src/graphics.rs` の `draw_mesh_tasks`（834 行）・`draw_mesh_tasks_indirect`（841 行）・`multi_draw_mesh_tasks_indirect`（849 行）・`multi_draw_mesh_tasks_indirect_count`（858 行） |
| 試験用の device の実装 | `crates/core/src/stub_device.rs` の `impl GraphicsOps for StubGraphics`（978・989・1001・1014 行） |
| 試験の局所の実装 | `crates/core/tests/graphics_types.rs` の `FixedGraphics`（170・178・187・197 行） |

### 操作

どれも既定の本体を持たず、実装する側が 4 操作とも実装する。パイプラインの引数は受けない。

| 操作 | 引数の型 | 戻り値の型 |
| ---- | -------- | ---------- |
| `draw_mesh_tasks` | `&mut RenderEncoder`・`[u32; 3]` | `Result<(), ContractError>` |
| `draw_mesh_tasks_indirect` | `&mut RenderEncoder`・`&BufferHandle`・`u64` | `Result<(), ContractError>` |
| `multi_draw_mesh_tasks_indirect` | `&mut RenderEncoder`・`&BufferHandle`・`u64`・`u32` | `Result<(), ContractError>` |
| `multi_draw_mesh_tasks_indirect_count` | `&mut RenderEncoder`・`&BufferHandle`・`u64`・`&BufferHandle`・`u64`・`u32` | `Result<(), ContractError>` |

### 試験用の device の返し

試験用の device の振る舞い。引数を照らさず、何も記録せず、状態も持たない。

| 入力 | 4 操作の返し |
| ---- | ------------ |
| 生きた記録先 | どれも `Unsupported`（`op` は操作名） |
| 拒否の値を持つ記録先 | 同上 |
| 別 device の記録先 | 同上 |
| 破棄した描画先の記録先 | 同上 |

### 試験

- `GraphicsOps` の rustdoc に、1 操作だけを持たない実装が E0046 で拒まれる `compile_fail` の doctest を 4 本足した。既存の 8 本の局所の実装には 4 操作を足した。
- `stub_device.rs` の `mod tests` に、上の表の 4 入力の試験を足した。

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features graphics --doc graphics
cargo test -p fandhe-silicon-core --features test-support,graphics --lib stub_device::
cargo test -p fandhe-silicon-core --features graphics --test graphics_types
```

## 外の crate からの試験（2/3）

公開のパスだけを使う統合テスト。

- `crates/core/tests/graphics_mesh_types.rs`（feature `graphics`）
- `crates/core/tests/graphics_mesh_stub.rs`（feature `test-support` と `graphics`。`backend` は `BackendKind::Vulkan`）

### 操作の型（graphics_mesh_types）

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `mesh_op_signatures_have_listed_types` | 4 操作を `dyn GraphicsOps` 越しに呼ぶクロージャを、型を書いた `fn` ポインタの変数へ入れる | コンパイルが通り、引数と戻り値の型が 1/3 の表のとおりに固定される |

### 試験用の device の返し（graphics_mesh_stub）

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `mesh_ops_are_unsupported_on_live_encoder` | 同じ device の描画先から作った記録先と同じ device のバッファ | 4 操作とも `Unsupported`、`op` は操作名と一致 |
| `mesh_ops_are_unsupported_on_other_device_encoder` | 別 device の描画先から作った記録先 | 同上 |
| `mesh_ops_are_unsupported_on_destroyed_target_encoder` | 描画先を破棄した後の記録先 | 同上 |
| `mesh_ops_ignore_argument_values` | グループ数・offset・個数の極端な値と、別 device のバッファ | 同上 |
| `mesh_ops_are_repeatable` | 同じ記録先・同じ引数で 4 操作を 2 回ずつ | 戻り値が等しい |
| `mesh_ops_do_not_consume_encoder` | 4 操作の後に同じ記録先を `submit_render` へ値で渡す | コンパイルが通り、panic しない |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --test graphics_mesh_types
cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_mesh_stub
```
