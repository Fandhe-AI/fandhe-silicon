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

## 外の crate からの試験（3/4）

置き場は `crates/core/tests/sampler_types.rs`。feature なしで流れる、公開のパス `fandhe_silicon_core::sampler::…` だけを使う外の crate の試験で、記述子は `default()` の後に欄を書き換えて作る。

| 試験の名前 | 入力 | 期待 |
| ---------- | ---- | ---- |
| `descriptor_default_values` | `SamplerDescriptor::default()` | 11 欄が記述子の表の既定の値と等しい |
| `descriptor_fields_hold_written_values` | 既定から 11 欄を別の値に書き換える | 読み戻した値が書いた値と等しい |
| `descriptor_holds_values_unchecked` | 下限が上限より大きい LOD、`anisotropy_clamp` の 0 と `u16::MAX`、`lod_min_clamp` の NaN、境界色なしの `ClampToBorder` | そのまま保持される |
| `descriptor_clone_and_eq` | 既定と、1 欄だけ書き換えた記述子 | `clone` は元と等しく、1 欄違いは既定と等しくない |
| `value_defaults` | 3 つの列挙の `default()` | `ClampToEdge`・`Nearest`・`Nearest` |
| `value_enums_are_distinct` | 4 つの列挙の全値を `HashSet` に入れる | 要素数が 4・2・2・4 |
| `value_enums_match_with_wildcard` | `_` の腕つきの `match` で各値を名前に写す | 各値が期待の名前に写る |
| `handle_round_trip` | `__backend_new` に 0・1・`u64::MAX` を含む組 | 3 つの読み出しが入れた値を返す |

確かめ方:

```sh
cargo test -p fandhe-silicon-core --test sampler_types
```
