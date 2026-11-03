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

## 完了の条件の確かめ（3/3）

3/3（#5827）の時点で本リポで観測できた事実だけを記録する（ポインタ: `docs/spec/05-tasks.md` の TASK-839）。GPU の実機なしで実行した。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 4 操作が共通の crate の feature `graphics` の `GraphicsOps` にある | `grep -n 'fn draw_mesh_tasks\|fn multi_draw_mesh_tasks' crates/core/src/graphics.rs`、`cargo check -p fandhe-silicon-core --features graphics` | 宣言は `graphics.rs` の 834・841・849・858 行目の 4 行（ほかは doc コメントの例の行）。`cargo check` は終了コード 0 |
| (2) | 4 操作の引数の型・戻り値の型 | `cargo test -p fandhe-silicon-core --features graphics --test graphics_mesh_types` | `mesh_op_signatures_have_listed_types` が ok（1 passed） |
| (3) | 4 操作が既定の本体を持たない | `cargo test -p fandhe-silicon-core --features graphics --doc graphics`、`grep -n 'compile_fail,E0046' crates/core/src/graphics.rs`、`make doctest-error-code-check`、`grep -n 'impl GraphicsOps for\|fn draw_mesh_tasks\|fn multi_draw_mesh_tasks' crates/core/src/stub_device.rs crates/core/tests/graphics_types.rs` | doctest は 23 passed と 26 passed の 2 つの実行がいずれも ok。`compile_fail,E0046` は 247・282・317・352・387・422・457・492・527・562・597・632 行目の 12 個で、527・562・597・632 行目の 4 個が 4 操作の分。`make doctest-error-code-check` は終了コード 0。`impl` は `stub_device.rs:782` と `graphics_types.rs:21`。4 操作は前者に 978・989・1001・1014 行目、後者に 170・178・187・197 行目 |
| (4) | 試験用の device の返し | `cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_mesh_stub`、`cargo test -p fandhe-silicon-core --features test-support,graphics --lib stub_device::` | `mesh_ops_are_unsupported_on_live_encoder`・`mesh_ops_are_unsupported_on_other_device_encoder`・`mesh_ops_are_unsupported_on_destroyed_target_encoder`・`mesh_ops_ignore_argument_values`・`mesh_ops_are_repeatable`・`mesh_ops_do_not_consume_encoder` が ok（6 passed）。`--lib stub_device::` は 47 passed で、`mesh_ops_on_live_encoder_return_unsupported_with_op_name`・`mesh_ops_ignore_encoder_carrying_a_rejection`・`mesh_ops_ignore_encoder_of_another_device`・`mesh_ops_ignore_encoder_of_destroyed_target` が ok |
| (5) | 公開 API に wgpu の型が 0 件 | `make public-api-check`・`make public-api-webgpu-check`・`make public-api-sdk-check`・`make public-name-check` | 終了コードはいずれも 0。`record:` の行はいずれも `commit=5bf9d3f293dc31ea42bc72c80a042806971985c2`。`public-api-check` は `documents=10 public_items_total=15207 wgpu_type_refs_total=0 wgpu_other_refs_total=0`、`public-api-webgpu-check` は `documents=2 public_items_total=3814 submit_token_ops_total=11 submit_token_violations_total=0`、`public-api-sdk-check` は `documents=10 public_items_total=13108 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0`、`public-name-check` は `documents=10 public_items_total=13108 name_hits_total=0` |
| (6) | 依存 0・unsafe 0 | コミットごとの `git show --stat <sha> -- Cargo.lock deps-approved.toml crates/contract`（`232bbbe`・`5bf9d3f`）、`git show 5bf9d3f -- crates/core/Cargo.toml`、`git show 232bbbe -- crates/core/Cargo.toml`、`grep -n forbid crates/core/src/lib.rs` | 2 コミットとも出力 0 行。`Cargo.toml` の差分は `5bf9d3f` の `[[test]]` 2 項目（`graphics_mesh_types`、`graphics_mesh_stub`。+8 行）だけで、`232bbbe` は 0 行。`lib.rs:39` に `#![forbid(unsafe_code)]` |
| (7) | CI で流れる場所 | `make verify`（`scripts/verify.sh` は `cargo test --workspace --all-features`）、`make wasm-verify`、`make build-profiles` | `cargo test --workspace --all-features` の中で `graphics_mesh_types` が 1 passed、`graphics_mesh_stub` が 6 passed、`graphics_types` が 4 passed。`make verify`・`make wasm-verify`・`make build-profiles` は終了コード 0 |

品質ゲートの終了コード: `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify`・`make doctest-error-code-check` はいずれも 0。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-839）。確かめた base は `5bf9d3f293dc31ea42bc72c80a042806971985c2`。

### 確かめ方（3/3）

```sh
grep -n 'fn draw_mesh_tasks\|fn multi_draw_mesh_tasks' crates/core/src/graphics.rs
cargo check -p fandhe-silicon-core --features graphics
cargo test -p fandhe-silicon-core --features graphics --test graphics_mesh_types
cargo test -p fandhe-silicon-core --features graphics --doc graphics
grep -n 'compile_fail,E0046' crates/core/src/graphics.rs
make doctest-error-code-check
grep -n 'impl GraphicsOps for\|fn draw_mesh_tasks\|fn multi_draw_mesh_tasks' crates/core/src/stub_device.rs crates/core/tests/graphics_types.rs
cargo test -p fandhe-silicon-core --features test-support,graphics --test graphics_mesh_stub
cargo test -p fandhe-silicon-core --features test-support,graphics --lib stub_device::
make public-api-check
make public-api-webgpu-check
make public-api-sdk-check
make public-name-check
git show --stat 232bbbe -- Cargo.lock deps-approved.toml crates/contract
git show --stat 5bf9d3f -- Cargo.lock deps-approved.toml crates/contract
git show 5bf9d3f -- crates/core/Cargo.toml
grep -n forbid crates/core/src/lib.rs
make verify
make wasm-verify
make build-profiles
```
