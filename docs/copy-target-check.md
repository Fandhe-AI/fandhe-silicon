# 描画先からバッファへの写しの書き込み先の検査（TASK-591）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-591、項目 `CMD-86`。

## 所在

- 実装: `crates/core/src/graphics/target_check.rs`
- 公開のパス: `fandhe_silicon_core::graphics::validate_copy_target_to_buffer`（feature `graphics` の裏。`graphics` モジュールは [graphics-types.md](graphics-types.md)）
- 使う関数: `pixel_format_bytes`・`offscreen_target_bytes`（[offscreen-target-check.md](offscreen-target-check.md)）、`validate_buffer_range`（[buffer-range-check.md](buffer-range-check.md)）

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `validate_copy_target_to_buffer` | 幅・高さ（`u32`）、`PixelFormat`、`dst_offset`・`dst_len`（`u64`） | `Result<(), ContractError>` |

## 現状の振る舞い（1/2）

必要なバイト数は幅 × 高さ × 形式の 1 画素のバイト数。`dst_offset` から数えてそれが `dst_len` に収まるかを見る。

| 入力 | 返し |
| ---- | ---- |
| 必要なバイト数の積があふれる | `InvalidArgument`（文言に `overflows u64`） |
| `dst_offset` との和があふれる | `InvalidArgument`（文言に `overflows u64`） |
| `dst_offset` から数えて `dst_len` を超える | `InvalidArgument`（文言に `exceeds`） |
| ちょうど収まる（`dst_offset` が 0 でも 0 でなくても） | `Ok(())` |
| 余りがある | `Ok(())` |

拒否の文言の先頭は `copy_target_to_buffer: `。背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。

## 対応するテスト

- doctest 1 本（`validate_copy_target_to_buffer` の `///`）
- `mod tests`: `target_check_copy_exact_passes`・`target_check_copy_short_is_rejected`・`target_check_copy_product_overflow_is_rejected`・`target_check_copy_sum_overflow_is_rejected`・`target_check_copy_message_has_prefix`

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --lib target_check
cargo test -p fandhe-silicon-core --features graphics --doc validate_copy_target_to_buffer
make verify
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/2 | #5108 | 本頁の関数 |
| 2/2 | #5109 | 境界の入力の全組の試験と完了の条件の確かめ |
