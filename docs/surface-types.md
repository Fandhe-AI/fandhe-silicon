# 表示先の設定の大きさの検査（TASK-654）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-654。

## 所在

- 実装: `crates/core/src/graphics/surface.rs`
- 公開のパス: `fandhe_silicon_core::graphics::validate_surface_size`（feature `graphics` の裏。`graphics` モジュールは [graphics-types.md](graphics-types.md)、TASK-837）
- 使う部品: `validate_offscreen_target_size`（[offscreen-target-check.md](offscreen-target-check.md)、TASK-374）

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `validate_surface_size` | 幅・高さ・`max_texture_size`（いずれも `u32`） | `Result<(), ContractError>` |

## 設定の大きさの検査（1/4）

描画先と同じ規則で幅・高さを `max_texture_size` と比べる。比較は部品に任せ、拒否の文言の先頭だけを付け替える。状態を持たず、背後実装の API は呼ばない。

| 入力 | 返し |
| ---- | ---- |
| 幅または高さが 0 | `InvalidArgument` |
| 幅と高さがどちらも `max_texture_size` 以下（ちょうどを含む） | `Ok(())` |
| 幅または高さが `max_texture_size` を 1 超える | `InvalidArgument` |
| `max_texture_size` が 0 | `InvalidArgument` |

拒否の文言の先頭は `surface configuration: `。

### 対応するテスト

- doctest 1 本（`validate_surface_size` の `///`）
- `mod tests`: `surface_size_zero_is_rejected`・`surface_size_at_limit_passes`・`surface_size_over_limit_is_rejected`・`surface_size_limit_extremes`・`surface_size_message_has_prefix`

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --lib graphics::surface
cargo test -p fandhe-silicon-core --features graphics --doc validate_surface_size
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #5347 | 本頁の関数 |
