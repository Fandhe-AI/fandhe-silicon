# テクスチャの作成の記述子と操作の時点の検査（TASK-1942）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-1942。
項目のポインタは `TEX-50`〜`TEX-54`・`TEX-56`・`TEX-58`〜`TEX-65`。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/descriptor_check.rs` |
| 公開のパス | `fandhe_silicon_core::texture::validate_texture_descriptor` |
| feature | 条件なし |

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `validate_texture_descriptor` | `&TextureDescriptor`、1 次元・2 次元・3 次元の大きさの上限、配列の層数の上限（すべて `u32`） | `Result<(), ContractError>` |

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

## 対応するテスト

`crates/core/src/texture/descriptor_check.rs` の `mod tests`:
`descriptor_check_empty_usage_is_rejected`・`descriptor_check_zero_extent_is_rejected`・`descriptor_check_d1_limits`・`descriptor_check_d2_limits`・`descriptor_check_d3_limits`・`descriptor_check_limits_are_not_swapped`・`descriptor_check_sample_count`。
doctest は `validate_texture_descriptor` の `///` に 1 本（`Ok`・用途が空・幅 8193）。

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib descriptor_check
cargo test -p fandhe-silicon-core --doc validate_texture_descriptor
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
