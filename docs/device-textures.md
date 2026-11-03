# device からテクスチャを作る操作へ届く口の所在（TASK-150）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-150。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/ops.rs`・`crates/core/src/device.rs` |
| 公開のパス | `fandhe_silicon_core::texture::TextureOps`・`fandhe_silicon_core::device::Device::textures` |
| feature の条件 | なし |

記述子と handle の型の所在は [texture-types.md](texture-types.md)（TASK-371）。

## 現状の形（1/4）

- `Device::textures` は `crates/core/src/device.rs:191` にあり、署名は `fn textures(&self) -> Option<&dyn crate::texture::TextureOps>`。既定の本体は `None`
- `TextureOps` は `crates/core/src/texture/ops.rs` の trait で、`texture` モジュールの末尾（`crates/core/src/texture.rs:490`）から再公開される。メソッドは `fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Result<TextureHandle, ContractError>` の 1 つで、既定の本体は無い。持たない実装は `E0046` で拒まれる（trait の doctest）

確かめ方:

```sh
grep -n 'fn textures' crates/core/src/device.rs
grep -n 'pub use ops' crates/core/src/texture.rs
cargo test -p fandhe-silicon-core --doc texture::ops
```
