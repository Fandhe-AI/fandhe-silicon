# テクスチャの作成の記述子と操作の時点の検査（TASK-1942）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1942。
項目のポインタは `TEX-50`〜`TEX-54`・`TEX-56`・`TEX-58`〜`TEX-65`。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/descriptor_check.rs` |
| 実装（操作の時点） | `crates/core/src/texture/usage_check.rs` |
| 公開のパス | `fandhe_silicon_core::texture::validate_texture_descriptor`、`fandhe_silicon_core::texture::max_mip_level_count`、`fandhe_silicon_core::texture::TextureUsageRole`、`fandhe_silicon_core::texture::validate_texture_usage_for_role`、`fandhe_silicon_core::texture::validate_texture_view_format` |
| feature | 条件なし |

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `validate_texture_descriptor` | `&TextureDescriptor`、1 次元・2 次元・3 次元の大きさの上限、配列の層数の上限（すべて `u32`） | `Result<(), ContractError>` |
| `max_mip_level_count` | `TextureDimension`、`TextureExtent` | `u32` |
| `validate_texture_usage_for_role` | 作成の時に記録した `TextureUsage`、`TextureUsageRole` | `Result<(), ContractError>` |
| `validate_texture_view_format` | 元の `TextureFormat`、`&[TextureFormat]`（`view_formats`）、求める `TextureFormat` | `Result<(), ContractError>` |

型の所在は [texture-types.md](texture-types.md)（TASK-371）を参照。

## 現状の振る舞い（1/6）

- 記述子を次の順に照らし、最初に当たった 1 件を `ContractError::InvalidArgument` で返す。当たらなければ `Ok(())`。
    1. 用途が空
    2. 幅・高さ・`depth_or_array_layers` のどれかが 0
    3. 次元ごとの上限の超過
    4. サンプル数が 0、2 の冪でない、または次元ごとの上限（`D1`・`D3` は 1、`D2` は 32）の超過
- 上限ちょうどは通す。上限は呼び出し側が 4 つの `u32` で渡す。`label` は読まない。
- 2 つ以上に当たる入力でも返しは `InvalidArgument` で、文言は固定しない。

上限を 1 次元 4096・2 次元 8192・3 次元 1024・層数 256 としたときの返し。

| 入力 | 返し |
| ---- | ---- |
| `D2` 1280×720×1、用途 `TEXTURE_BINDING` | `Ok(())` |
| 用途が空（3 つの次元とも） | `InvalidArgument` |
| 幅・高さ・層のどれかが 0（3 つの次元とも） | `InvalidArgument` |
| `D1` 幅 4096 / 4097 | `Ok(())` / `InvalidArgument` |
| `D1` 高さ 2 または層 2 | `InvalidArgument` |
| `D2` 幅・高さ 8192 / 8193 | `Ok(())` / `InvalidArgument` |
| `D2` 層 256 / 257 | `Ok(())` / `InvalidArgument` |
| `D3` 3 辺 1024 / どれかが 1025 | `Ok(())` / `InvalidArgument` |
| `D2` サンプル数 1・2・4・8・16・32 | `Ok(())` |
| `D2` サンプル数 0・3・64 | `InvalidArgument` |
| `D1`・`D3` サンプル数 2 | `InvalidArgument` |

## 形式と次元の組・mip の段数（2/6）

サンプル数の検査に続けて、次の 4 つをこの順に照らす。どれも `ContractError::InvalidArgument` を返し、文言は固定しない。

5. 次元が `D2` でなく、形式が深度かステンシルを持つか、用途が `TRANSIENT_ATTACHMENT` を含む
6. 次元が `D1` で、形式が圧縮形式か用途が `RENDER_ATTACHMENT` を含む。または次元が `D3` で、形式が BC・ASTC 以外の圧縮形式（ETC2・EAC）
7. 形式が圧縮形式で、幅か高さがブロックの大きさの倍数でない
8. `mip_level_count` が 0 か `max_mip_level_count` の超過

`max_mip_level_count` は、`D1` が 1、`D2` が幅と高さの大きい方、`D3` が 3 辺の最大を `m` として `32 - m.leading_zeros()`、その値と 16 の小さい方を返す。

| 入力 | 返し |
| ---- | ---- |
| `D2` 1280×720×1 | 11 |
| `D2` 16384×16384×1 | 15 |
| `D2` 幅 32768 / 65536 / `u32::MAX` | 16 |
| `D2` 4×4×32 | 3 |
| `D3` 4×4×32 | 6 |
| `D1` 4096×1×1 | 1 |

`validate_texture_descriptor` の返し（上限は 1/6 と同じ）。

| 入力 | 返し |
| ---- | ---- |
| `D1`・`D3` の `Depth32Float`・`Stencil8` | `InvalidArgument` |
| `D1`・`D3` の用途 `TRANSIENT_ATTACHMENT` | `InvalidArgument` |
| `D2` の `Depth32Float` | `Ok(())` |
| `D1` の `Bc1RgbaUnorm`、`D1` の用途 `RENDER_ATTACHMENT` | `InvalidArgument` |
| `D3` の `Bc1RgbaUnorm` 8×8×2、ASTC 6x6 の 12×12×2 | `Ok(())` |
| `D3` の `Etc2Rgb8Unorm` 8×8×2 | `InvalidArgument` |
| `D3` の `Rgba8Unorm`、用途 `RENDER_ATTACHMENT` | `Ok(())` |
| `D2` の `Bc1RgbaUnorm` 8×8 / 6×8 / 8×6 | `Ok(())` / `InvalidArgument` / `InvalidArgument` |
| `D2` の ASTC 6x6 12×12 / 8×8 | `Ok(())` / `InvalidArgument` |
| `D2` の `Rgba8Unorm` 7×5 | `Ok(())` |
| `D2` 1280×720 の mip 11 / 12 / 0 | `Ok(())` / `InvalidArgument` / `InvalidArgument` |
| `D3` 4×4×32 の mip 6 / 7 / 0 | `Ok(())` / `InvalidArgument` / `InvalidArgument` |
| `D1` の mip 1 / 2 | `Ok(())` / `InvalidArgument` |

## 一時的な描画先・マルチサンプル・view_formats（3/6）

mip の段数の検査に続けて、次の 3 つをこの順に照らす（番号は上の続き）。どれも `ContractError::InvalidArgument` を返し、文言は固定しない。

9. 用途が `TRANSIENT_ATTACHMENT` を含み、用途が `TRANSIENT_ATTACHMENT | RENDER_ATTACHMENT` とちょうど同じでないか、`mip_level_count` が 1 でないか、`depth_or_array_layers` が 1 でないか、`view_formats` が空でない
10. サンプル数が 1 より大きく、`mip_level_count` が 1 でないか、用途が `STORAGE_BINDING` を含むか、`RENDER_ATTACHMENT` を含まない
11. `view_formats` のどれかの要素が、元の形式と同じでなく、`remove_srgb_suffix()` が元の形式の `remove_srgb_suffix()` と同じでもない

照らす順の全体は 1〜11 の番号の順（用途の空、大きさの 0、次元ごとの上限、サンプル数、次元との組、D1・D3 の圧縮と描画先、ブロックの倍数、mip の段数、9、10、11）。

`D2` 16×16 の `Rgba8Unorm`、上限は 1/6 と同じ。

| 入力 | 返し |
| ---- | ---- |
| 用途 `TRANSIENT_ATTACHMENT \| RENDER_ATTACHMENT`、mip 1、層 1、`view_formats` 空（サンプル数 1 / 4） | `Ok(())` |
| 上に `COPY_SRC` を足す、または `TRANSIENT_ATTACHMENT` だけ | `InvalidArgument` |
| 上の用途で mip 2、層 2、`view_formats` に `Rgba8UnormSrgb` | `InvalidArgument` |
| サンプル数 4、用途 `RENDER_ATTACHMENT`、mip 1 | `Ok(())` |
| サンプル数 4 で mip 2、`STORAGE_BINDING` を足す、`RENDER_ATTACHMENT` なし | `InvalidArgument` |
| サンプル数 1 で mip 2・`STORAGE_BINDING`・`RENDER_ATTACHMENT` なし | `Ok(())` |
| `Rgba8Unorm` の `view_formats` が `[Rgba8UnormSrgb]` / `[Rgba8Unorm]` | `Ok(())` |
| `Rgba8Unorm` の `view_formats` が `[Bgra8Unorm]` / `[Rgba8UnormSrgb, Bgra8Unorm]` | `InvalidArgument` |
| `Bgra8UnormSrgb` の `view_formats` が `[Bgra8Unorm]` | `Ok(())` |
| ASTC 6x6 `Unorm` の `view_formats` が `[UnormSrgb]` / `[Unorm]` / `[Hdr]` | `Ok(())` / `Ok(())` / `InvalidArgument` |

## 操作の時点の用途と view の形式（4/6）

作成の記述子の検査とは別の関数で、背後実装が操作の前に、作成の時に記録した値を渡して呼ぶ。どちらも `ContractError::InvalidArgument` を返し、文言は固定しない。

役割と要る用途（どれも 1 ビット）。

| `TextureUsageRole` | 要る用途 |
| ------------------ | -------- |
| `CopySource` | `COPY_SRC` |
| `CopyDestination` | `COPY_DST` |
| `TextureBinding` | `TEXTURE_BINDING` |
| `StorageBinding` | `STORAGE_BINDING` |
| `StorageAtomic` | `STORAGE_ATOMIC` |
| `RenderAttachment` | `RENDER_ATTACHMENT` |

`validate_texture_usage_for_role` の返し。

| 入力 | 返し |
| ---- | ---- |
| 要る用途だけ（6 役割とも） | `Ok(())` |
| 要る用途だけを除いた残り（6 役割とも） | `InvalidArgument` |
| 空の用途（6 役割とも） | `InvalidArgument` |
| 7 つの用途すべての和（6 役割とも） | `Ok(())` |
| `STORAGE_ATOMIC` だけで `StorageBinding` / `StorageAtomic` | `InvalidArgument` / `Ok(())` |
| `COPY_SRC` と `TEXTURE_BINDING` の和で `CopySource` / `CopyDestination` | `Ok(())` / `InvalidArgument` |

`validate_texture_view_format` の返し。

| 入力 | 返し |
| ---- | ---- |
| 求める形式が元の形式と同じ | `Ok(())` |
| 求める形式が `view_formats` にある | `Ok(())` |
| 元の形式とも `view_formats` のどれとも違う | `InvalidArgument` |
| `view_formats` が空で、求める形式が元の形式の sRGB の対 | `InvalidArgument` |

## 対応するテスト

`crates/core/src/texture/descriptor_check.rs` の `mod tests`:
`descriptor_check_empty_usage_is_rejected`・`descriptor_check_zero_extent_is_rejected`・`descriptor_check_d1_limits`・`descriptor_check_d2_limits`・`descriptor_check_d3_limits`・`descriptor_check_limits_are_not_swapped`・`descriptor_check_sample_count`・`descriptor_check_max_mip_level_count_values`・`descriptor_check_depth_stencil_and_transient_need_d2`・`descriptor_check_d1_rejects_compressed_and_render_attachment`・`descriptor_check_d3_compressed_formats`・`descriptor_check_compressed_block_multiple`・`descriptor_check_mip_level_count`・`descriptor_check_transient_attachment`・`descriptor_check_multisample`・`descriptor_check_view_formats_compatibility`。
doctest は `validate_texture_descriptor` の `///` に 1 本（`Ok`・用途が空・幅 8193）、`max_mip_level_count` の `///` に 1 本。

`crates/core/src/texture/usage_check.rs` の `mod tests`:
`texture_usage_check_roles_accept_required_usage`・`texture_usage_check_roles_reject_missing_usage`・`texture_usage_check_empty_usage_is_rejected`・`texture_usage_check_all_usages_pass`・`texture_usage_check_storage_atomic_alone_is_not_storage_binding`・`texture_usage_check_roles_require_distinct_single_bits`・`texture_usage_check_error_names_role_and_usage`・`texture_usage_check_view_format`。
doctest は `validate_texture_usage_for_role` と `validate_texture_view_format` の `///` に 1 本ずつ。

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib descriptor_check
cargo test -p fandhe-silicon-core --doc validate_texture_descriptor
cargo test -p fandhe-silicon-core --doc max_mip_level_count
cargo test -p fandhe-silicon-core --lib texture::usage_check
cargo test -p fandhe-silicon-core --doc validate_texture_usage_for_role
cargo test -p fandhe-silicon-core --doc validate_texture_view_format
make verify
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/6 | #7656 | 記述子の検査（用途の空・大きさの 0・次元ごとの上限・サンプル数） |
| 2/6 | #7657 | 形式と次元の組・圧縮のブロック・mip の段数の検査と上限 |
| 3/6 | #7658 | 一時的な描画先・マルチサンプル・view_formats の互換の検査 |
| 4/6 | #7659 | 操作の時点の用途と view の形式の検査 |
| 5/6 | #7660 | 項目ごとの受理と拒否の入力の試験 |
| 6/6 | #7661 | 完了の条件の確かめ |
