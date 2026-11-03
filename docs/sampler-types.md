# サンプラーの記述子の型の所在（TASK-373）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-373。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/sampler.rs` |
| 公開のパス | `fandhe_silicon_core::sampler::{AddressMode, FilterMode, MipmapFilterMode, SamplerBorderColor}` |
| feature の条件 | なし |

今の補い: `SamplerDescriptor` と `SamplerHandle` の公開のパスは `fandhe_silicon_core::sampler::{SamplerDescriptor, SamplerHandle}`（実装は同じ `sampler.rs`）。
`crates/core/tests/sampler_types.rs` は #7858 で 10 本になった（`cargo test -p fandhe-silicon-core --test sampler_types`）。

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

## 完了の条件の確かめ（4/4）

4/4（#4572）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-373）。GPU の実機なしの Linux で実行し、件数は実測。コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 型が共通の crate に条件なしで公開されている | `grep -n 'pub mod sampler' crates/core/src/lib.rs`、`cargo check -p fandhe-silicon-core`（feature なし） | 58 行目に `pub mod sampler;`、終了コード 0 |
| 2 | 記述子の欄の値の 4 つの列挙と既定の値 | `cargo test -p fandhe-silicon-core --test sampler_types` | `value_defaults`・`value_enums_are_distinct`・`value_enums_match_with_wildcard` が `ok` |
| 3 | 記述子の既定の値と欄の保持 | 同じ試験 | `descriptor_default_values`・`descriptor_fields_hold_written_values`・`descriptor_holds_values_unchecked`・`descriptor_clone_and_eq` が `ok` |
| 4 | handle の往復と、外の crate から作れない・複製できない形 | 同じ試験、`cargo test -p fandhe-silicon-core --doc sampler`、`make doctest-error-code-check` | `handle_round_trip` が `ok`、doctest は通常 5 件が `ok`・`compile fail` 4 件が `ok`（`E0004`・`E0639`・`E0451`・`E0599`）、`make doctest-error-code-check` の終了コード 0 |
| 5 | 背後実装を呼ばない | `grep -c '^use ' crates/core/src/sampler.rs`、1/4〜3/4 の 3 コミット（`8c57a88`・`213308d`・`b8119b4`）それぞれの `git show --stat --format= <sha> -- crates/core/src/stub_device.rs crates/core/src/device.rs crates/core/src/graphics.rs crates/core/src/texture.rs crates/core/src/texture crates/contract` | `use` の行 0 件、3 コミットとも出力 0 行（差分 0） |
| 6 | 依存 0・unsafe 0 | 3 コミットそれぞれの `git show --stat --format= <sha> -- Cargo.lock deps-approved.toml crates/core/Cargo.toml`、`grep -c unsafe crates/core/src/sampler.rs`、`grep -n forbid crates/core/src/lib.rs` | 3 コミットとも差分 0、`unsafe` 0 件、39 行目に `#![forbid(unsafe_code)]` |
| 7 | 公開 API の検査 | `make public-api-check`・`make public-api-sdk-check`・`make public-name-check`・`make public-api-webgpu-check` | `wgpu_type_refs_total=0`、`sdk_type_refs_total=0`、`name_hits_total=0`、`webgpu_names_total=0`、4 つとも終了コード 0 |
| 8 | CI で流れる場所と件数 | `make verify`（`.github/workflows/ci.yml` 177 行目・498 行目）、`make wasm-verify`（同 275 行目・459 行目）、`cargo test -p fandhe-silicon-core --lib sampler` | `make verify` の終了コード 0（`tests/sampler_types.rs` は 8 件）、`--lib sampler` は 6 件が `ok`、`make wasm-verify` の終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-373）。確かめた base は `b8119b42b5578b2db21c875ee59f6ecc4aa06d75`。

確かめ方:

```text
grep -n 'pub mod sampler' crates/core/src/lib.rs
cargo check -p fandhe-silicon-core
cargo test -p fandhe-silicon-core --test sampler_types
cargo test -p fandhe-silicon-core --lib sampler
cargo test -p fandhe-silicon-core --doc sampler
make doctest-error-code-check
make public-api-check
make public-api-sdk-check
make public-name-check
make public-api-webgpu-check
make verify
make wasm-verify
```
