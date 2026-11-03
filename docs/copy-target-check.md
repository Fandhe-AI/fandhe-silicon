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

## 境界の入力の全組の試験（2/2）

試験: `crates/core/tests/graphics_copy_target_check.rs`。外の crate から公開のパス `fandhe_silicon_core::graphics::` だけを使い、feature `graphics` の裏でだけ走る。3 つの形式（`Rgba8Unorm`・`Bgra8Unorm`・`Rgba32Float`）を回す。大きさは 1280 × 720（必要なバイト数は 3,686,400・3,686,400・14,745,600）と 1 × 1（4・4・16）。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `exact_length_from_zero_offset_is_accepted` | `dst_offset` 0、`dst_len` が必要なバイト数 | `Ok(())` |
| `exact_length_from_nonzero_offset_is_accepted` | `dst_offset` 256・4096、`dst_len` が `dst_offset` + 必要なバイト数 | `Ok(())` |
| `longer_destination_is_accepted` | `dst_offset` 0・256、`dst_len` が `dst_offset` + 必要なバイト数 + 512 | `Ok(())` |
| `one_byte_short_is_rejected` | `dst_offset` 0・256、`dst_len` が `dst_offset` + 必要なバイト数 - 1 | `InvalidArgument`（先頭 `copy_target_to_buffer: `・`exceeds`） |
| `offset_past_end_is_rejected` | `dst_len` が必要なバイト数、`dst_offset` がそれ + 1 と + 256 | `InvalidArgument`（先頭 `copy_target_to_buffer: `・`exceeds`） |
| `offset_plus_need_overflow_is_rejected` | `dst_len` が `u64::MAX`、`dst_offset` が `u64::MAX` - 必要なバイト数 + 1 と `u64::MAX` | `InvalidArgument`（先頭 `copy_target_to_buffer: `・`overflows u64`） |
| `byte_count_overflow_is_rejected` | 幅・高さが `u32::MAX`、`dst_offset` 0、`dst_len` が `u64::MAX` | `InvalidArgument`（先頭 `copy_target_to_buffer: `・`overflows u64`） |
| `need_matches_offscreen_target_bytes` | 1280 × 720、`dst_offset` 0。`offscreen_target_bytes` の値を `dst_len` に使う | その値で `Ok(())`、1 小さいと `InvalidArgument`。値は上の必要なバイト数と一致 |
| `rejection_is_repeatable` | 上の拒否の入力の代表 4 組を同じ引数で 2 回 | どちらも `Err` で、2 回の `Result` が等しい |

```sh
cargo test -p fandhe-silicon-core --features graphics --test graphics_copy_target_check
```

`make verify` の `cargo test` と `make wasm-verify` の型検査にも入る。

## 完了の条件の確かめ（2/2）

2/2（#5109）の時点で本リポで観測できた事実だけを記録する（ポインタ: `docs/spec/05-tasks.md` の TASK-591、項目 `CMD-86`）。GPU の実機なしで実行した。確かめた base は `0c36dd4`（1/2 のマージ commit）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 検査が共通の crate の純粋関数として公開されている | `grep -n validate_copy_target_to_buffer crates/core/src/graphics.rs crates/core/src/graphics/target_check.rs`、`cargo check -p fandhe-silicon-core --features graphics` | `graphics.rs:329` の再公開と `target_check.rs:141` の `pub fn`。`cargo check` は終了コード 0 |
| 2 | 範囲とあふれは `validate_buffer_range` を呼ぶ | `grep -n validate_buffer_range crates/core/src/graphics/target_check.rs` | `target_check.rs:8` の `use` と `:150` の呼び出し |
| 3 | 0・ちょうど・1 バイト足りない・あふれ（和・積）の組（`CMD-86`） | `cargo test -p fandhe-silicon-core --features graphics --test graphics_copy_target_check`。3 形式 | `exact_length_from_zero_offset_is_accepted`・`one_byte_short_is_rejected`・`offset_plus_need_overflow_is_rejected`・`byte_count_overflow_is_rejected` ほか計 9 本が ok（9 passed） |
| 4 | 0 でない `dst_offset` で残りが足りる入力を受け付ける | 同上 | `exact_length_from_nonzero_offset_is_accepted`・`longer_destination_is_accepted` が ok |
| 5 | 背後実装を呼ばない・`BufferHandle` を受けない | 署名（`grep -n 'pub fn validate_copy_target_to_buffer' -A6 crates/core/src/graphics/target_check.rs`）、`grep -n '^use ' crates/core/src/graphics/target_check.rs`、`git diff --stat 73cbd93 0c36dd4 -- crates/core/src/stub_device.rs crates/contract`、`git diff --stat origin/main -- crates/core/src/stub_device.rs crates/contract` | 引数は `u32`・`u32`・`PixelFormat`・`u64`・`u64`。`use` は `super`・`crate::error`・`crate::memory` の 3 行。2 つの差分とも 0 |
| 6 | 依存 0・unsafe 0 | `git diff --stat 73cbd93 0c36dd4 -- Cargo.lock deps-approved.toml`、`git diff --stat origin/main -- Cargo.lock deps-approved.toml`、`git diff origin/main -- crates/core/Cargo.toml`、`grep -c unsafe crates/core/src/graphics/target_check.rs`、`grep -n forbid crates/core/src/lib.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | 2 つの差分とも 0。`Cargo.toml` の差分は `[[test]]` 1 項目。`unsafe` は 0 件。`lib.rs:33` に `#![forbid(unsafe_code)]`。`cargo tree` は `fandhe-silicon-core` 1 行のみ |
| 7 | CI で流れる場所 | `make verify`、`cargo test -p fandhe-silicon-core --features graphics --lib target_check`、`--doc validate_copy_target_to_buffer`、`make wasm-verify` | `make verify` の `cargo test` の中で `graphics_copy_target_check` が 9 passed。`--lib target_check` は 15 passed、`--doc` は 1 passed。`make wasm-verify` は終了コード 0 |

品質ゲートの終了コード: `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` はいずれも 0。

再実行の契機: `target_check.rs` または試験ファイルを変えたとき、`PixelFormat` に値が増えたとき。

### 今の値の補い（base 8deff9f）

表の 1 の `graphics.rs:329` は記録時点の行で、今は `crates/core/src/graphics.rs:677` が `validate_copy_target_to_buffer` の再公開の行である。`target_check.rs:141` の `pub fn` は変わらない。数えたコマンドは `grep -n validate_copy_target_to_buffer crates/core/src/graphics.rs crates/core/src/graphics/target_check.rs`。

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/2 | #5108 | 本頁の関数 |
| 2/2 | #5109 | `crates/core/tests/graphics_copy_target_check.rs`（境界の入力の全組の試験）と完了の条件の確かめ（本頁の 2/2 の節） |
