# サンプラーの記述子の型の所在（TASK-373）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-373。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/sampler.rs` |
| 公開のパス | `fandhe_silicon_core::sampler::{AddressMode, FilterMode, MipmapFilterMode, SamplerBorderColor}` |
| feature の条件 | なし |

## 値の型（1/4）

4 つの列挙はどれも `#[non_exhaustive]` で、外の crate の `match` には `_` の腕が要る（`AddressMode` の doctest で確かめている）。

| 型 | 値（宣言の順） | 既定の値 |
| -- | -------------- | -------- |
| `AddressMode` | `ClampToEdge`・`Repeat`・`MirrorRepeat`・`ClampToBorder` | `ClampToEdge` |
| `FilterMode` | `Nearest`・`Linear` | `Nearest` |
| `MipmapFilterMode` | `Nearest`・`Linear` | `Nearest` |
| `SamplerBorderColor` | `TransparentBlack`・`OpaqueBlack`・`OpaqueWhite`・`Zero` | なし |

確かめ方:

```sh
cargo test -p fandhe-silicon-core --lib sampler
cargo test -p fandhe-silicon-core --doc sampler
```
