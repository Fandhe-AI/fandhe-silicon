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

## 記述子と handle（2/4）

公開のパスは `fandhe_silicon_core::sampler::{SamplerDescriptor, SamplerHandle}`。

### 記述子

`SamplerDescriptor<'a>` は `#[non_exhaustive]` で、`Debug`・`Clone`・`PartialEq` を持つ（`Eq`・`Hash`・`Copy` は持たない）。値は検査せず、どの欄もどの値でも保持する。外の crate の構造体式は拒まれる（doctest の `E0639`）ので、`default()` の後に欄を書き換えて作る。

| 欄 | 型 | 既定の値 |
| -- | -- | -------- |
| `label` | `Option<&'a str>` | `None` |
| `address_mode_u` | `AddressMode` | `ClampToEdge` |
| `address_mode_v` | `AddressMode` | `ClampToEdge` |
| `address_mode_w` | `AddressMode` | `ClampToEdge` |
| `mag_filter` | `FilterMode` | `Nearest` |
| `min_filter` | `FilterMode` | `Nearest` |
| `mipmap_filter` | `MipmapFilterMode` | `Nearest` |
| `lod_min_clamp` | `f32` | `0.0` |
| `lod_max_clamp` | `f32` | `32.0` |
| `anisotropy_clamp` | `u16` | `1` |
| `border_color` | `Option<SamplerBorderColor>` | `None` |

### handle

`SamplerHandle` は欄が 3 つ（世代・識別子・device の識別子、どれも `u64` で非公開）。derive は `Debug` だけで、`Clone`・`Copy` を持たない（doctest の `E0599`）。外の crate の構造体式は拒まれる（`E0451`）。背後実装向けの口は `#[doc(hidden)]` の `__backend_new`・`__backend_generation`・`__backend_id`・`__backend_device_id` の 4 つで、`__backend_new` は検査しない。

確かめ方:

```sh
cargo test -p fandhe-silicon-core --lib sampler
cargo test -p fandhe-silicon-core --doc sampler
```
