# 描画先の大きさとバイト数の純粋関数（TASK-374）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-374、項目 `TEX-8`・`TEX-9`・`TEX-10`。

## 所在

- 実装: `crates/core/src/graphics/target_check.rs`
- 公開のパス: `fandhe_silicon_core::graphics::`（feature `graphics` の裏。`graphics` モジュールは [graphics-types.md](graphics-types.md)）

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `pixel_format_bytes` | `PixelFormat` | `u32` |
| `depth_format_bytes` | `DepthFormat` | `u32` |
| `offscreen_target_bytes` | 幅・高さ・1 画素のバイト数（いずれも `u32`） | `Result<u64, ContractError>` |

## 現状の振る舞い

- `pixel_format_bytes`: `Rgba8Unorm` は 4、`Bgra8Unorm` は 4、`Rgba32Float` は 16
- `depth_format_bytes`: `Depth32Float` は 4
- `offscreen_target_bytes`: 1 画素のバイト数 × 幅 × 高さを `u64` で求める。積が `u64` であふれるときは `ContractError::InvalidArgument`（文言は英語で `overflows u64` を含む）。0 や上限は見ない（0 を渡せば `Ok(0)`）
- いずれも背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）

## 対応するテスト

- doctest 3 本（各関数の `///`）
- `mod tests` 5 本: `target_check_pixel_format_bytes`・`target_check_depth_format_bytes`・`target_check_bytes_1280x720`・`target_check_bytes_overflow_is_rejected`・`target_check_bytes_max_without_overflow`

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --lib target_check
cargo test -p fandhe-silicon-core --features graphics --doc target_check
make verify
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #4573 | 本頁の関数 |
| 2/4 | #4574 | — |
| 3/4 | #4575 | — |
| 4/4 | #4576 | — |
