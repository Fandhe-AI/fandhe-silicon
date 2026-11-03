# 表示先の設定の大きさの検査と取得の結果の型（TASK-654）

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

描画先と同じ規則で幅・高さを `max_texture_size` と比べる。比較は部品に任せ、拒否の文言の先頭に `surface configuration: ` を足す（元の文言は後ろに残る）。状態を持たず、背後実装の API は呼ばない。

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

## 取得の結果の型（2/4）

- 実装: `crates/core/src/graphics/surface.rs`
- 公開のパス: `fandhe_silicon_core::graphics::CurrentSurfaceTexture`・`fandhe_silicon_core::graphics::SurfaceStatus`（feature `graphics` の裏）
- `CurrentSurfaceTexture<T>` から `status()` で `SurfaceStatus` を得る。`SurfaceStatus::ALL` は 7 値を宣言の順に並べる。どちらも `#[non_exhaustive]`

| 結果の値 | 取得した値を持つか | `status()` の値 |
| -------- | ------------------ | --------------- |
| `Success(T)` | 持つ | `Good` |
| `Suboptimal(T)` | 持つ | `Suboptimal` |
| `Timeout` | 持たない | `Timeout` |
| `Occluded` | 持たない | `Occluded` |
| `Outdated` | 持たない | `Outdated` |
| `Lost` | 持たない | `Lost` |
| `Validation` | 持たない | `Validation` |

### 対応するテスト

- doctest: `CurrentSurfaceTexture` の `///` に通常 1 本、`compile_fail,E0004` が `CurrentSurfaceTexture` と `SurfaceStatus` に各 1 本
- `mod tests`: `surface_status_mapping`・`surface_status_all_distinct`・`surface_status_all_matches_results`

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --lib graphics::surface
cargo test -p fandhe-silicon-core --features graphics --doc CurrentSurfaceTexture
cargo test -p fandhe-silicon-core --features graphics --doc SurfaceStatus
```

## 境界の入力と対応の試験（3/4）

- 所在: `crates/core/tests/graphics_surface_types.rs`（`crates/core/Cargo.toml` の `[[test]]`、`required-features = ["graphics"]`）
- 外の crate から公開のパスだけで確かめる。上限は 16384 を使う

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `zero_size_is_rejected` | `(0, 720)`・`(1280, 0)`・`(0, 0)` | `InvalidArgument` |
| `exact_limit_is_accepted` | `(上限, 上限)`・`(上限, 1)`・`(1, 上限)` | `Ok(())` |
| `limit_plus_one_is_rejected` | 上限 +1 を幅・高さ・両方に置いた 3 組 | `InvalidArgument` |
| `same_result_as_offscreen_check` | 上の 9 組 | `validate_offscreen_target_size`（TASK-374）と通る・拒否の別が一致 |
| `rejection_is_repeatable` | 拒否 2 組と通る 1 組 | 同じ引数で 2 回呼んだ結果が等しい |
| `only_success_and_suboptimal_carry_value` | 7 値 | 値を取り出せるのは `Success`・`Suboptimal` だけ |
| `status_mapping` | 7 値 | `status()` が `Good`・`Suboptimal`・`Timeout`・`Occluded`・`Outdated`・`Lost`・`Validation` の順 |
| `outdated_carries_no_value` | `Outdated` | 値なし・`status()` は `Outdated` |
| `status_all_values` | `SurfaceStatus::ALL` | 長さ 7・重複なし |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features graphics --test graphics_surface_types
```

## 完了の条件の確かめ（4/4）

4/4（#5350）の時点で本リポで観測できた事実だけを記録する（ポインタ: `docs/spec/05-tasks.md` の TASK-654）。GPU の実機なしで実行した。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 検査と型が共通の crate の `graphics` に公開されている | `grep -n 'mod surface\|pub use surface' crates/core/src/graphics.rs`、`cargo check -p fandhe-silicon-core --features graphics`、`cargo check -p fandhe-silicon-core` | `graphics.rs:356` の `mod surface;` と `:357` の `pub use surface::{CurrentSurfaceTexture, SurfaceStatus, validate_surface_size};`。2 つの `cargo check` はどちらも終了コード 0 |
| 2 | 大きさの検査が TASK-374 の部品を使う | `grep -n validate_offscreen_target_size crates/core/src/graphics/surface.rs` | `surface.rs:6` の `use` と `:36` の呼び出し（`:4` は doc の言及） |
| 3 | 0・上限ちょうど・上限 +1 の入力の返し | `cargo test -p fandhe-silicon-core --features graphics --test graphics_surface_types` | `zero_size_is_rejected`・`exact_limit_is_accepted`・`limit_plus_one_is_rejected`・`same_result_as_offscreen_check` ほか計 9 本が ok（9 passed） |
| 4 | 取得の結果の型 | 同じ試験、`cargo test -p fandhe-silicon-core --features graphics --lib graphics::surface`、`cargo test -p fandhe-silicon-core --features graphics --doc surface` | `only_success_and_suboptimal_carry_value`・`status_mapping`・`outdated_carries_no_value`・`status_all_values` が ok。`--lib` は 8 passed。`--doc` は `CurrentSurfaceTexture`・`validate_surface_size` の 2 passed と、`compile_fail` の `CurrentSurfaceTexture`・`SurfaceStatus` の 2 passed |
| 5 | 公開 API の検査の通過 | `make public-api-check`・`make public-api-webgpu-check`・`make public-api-sdk-check`・`make public-name-check` | 終了コードはいずれも 0。`record:` の行はいずれも `commit=e04133f758d39cfc9407573e976e54462dc9b63e`。`public-api-check` は `documents=10 public_items_total=14778`、`public-api-sdk-check` は `documents=10 public_items_total=12761 sdk_type_refs_total=0 sdk_other_refs_total=0`、`public-name-check` は `documents=10 public_items_total=12761 name_hits_total=0` |
| 6 | 背後実装を呼ばない | `grep -n '^use ' crates/core/src/graphics/surface.rs`、コミットごとの `git show --stat <sha> -- crates/core/src/stub_device.rs crates/contract`（`3448ff9`・`52f4438`・`e04133f`） | `use` は `super::validate_offscreen_target_size`（`:6`）と `crate::error::ContractError`（`:7`）の 2 行。3 コミットとも出力 0 行 |
| 7 | 依存 0・unsafe 0 | コミットごとの `git show --stat <sha> -- Cargo.lock deps-approved.toml`、`git show e04133f -- crates/core/Cargo.toml`、`grep -c unsafe crates/core/src/graphics/surface.rs`、`grep -n forbid crates/core/src/lib.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | 3 コミットとも出力 0 行。`Cargo.toml` の差分は `[[test]]` 1 項目（`graphics_surface_types`、`required-features = ["graphics"]`）。`unsafe` は 0 件。`lib.rs:39` に `#![forbid(unsafe_code)]`。`cargo tree` は `fandhe-silicon-core` 1 行のみ |
| 8 | CI で流れる場所 | `make verify`、`make wasm-verify` | `make verify` の `cargo test` の中で `tests/graphics_surface_types.rs` が実行され、終了コード 0。`make wasm-verify` は終了コード 0 |

品質ゲートの終了コード: `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` はいずれも 0。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-654）。確かめた base は `e04133f758d39cfc9407573e976e54462dc9b63e`。

### 今の値の補い（base 1890c9a）

表の 4 の `--lib graphics::surface` は 8 passed、`--doc surface` は 2 passed と 2 passed で変わらない。表の 3 の `graphics_surface_types` は、拒否の文言が描画先の検査の文言の先頭に `surface configuration: ` を足したものと一致することを確かめる `rejection_message_keeps_offscreen_message` を足して 10 passed。表の 5 の 4 検査は今の base でいずれも終了コード 0 で、`record:` の値は次のとおり。

| 検査 | `record:` の値 |
| ---- | -------------- |
| `public-api-check` | `documents=10 public_items_total=14780 wgpu_type_refs_total=0 wgpu_other_refs_total=0` |
| `public-api-webgpu-check` | `documents=2 public_items_total=3814 public_functions_total=522 blocking_wait_names_total=0 pipeline_create_functions_total=1 pipeline_input_checked=1 pipeline_input_violations_total=0 interop_documents=2 ext_interop_checked=1 external_interop_items_total=0 semaphore_handle_defined=1 submit_wait_params_total=3 interop_violations_total=0 webgpu_names_total=0 submit_token_checked=1 submit_token_ops_total=11 submit_token_violations_total=0` |
| `public-api-sdk-check` | `documents=10 public_items_total=12763 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0` |
| `public-name-check` | `documents=10 public_items_total=12763 name_hits_total=0 sources=1 source_hits_total=0 field_specs=1 field_hits_total=0` |

### 今の値の補い（base 8deff9f）

表の 1 の `graphics.rs:356`・`:357` は記録時点の行で、今は `crates/core/src/graphics.rs` の `:681` が `mod surface;`、`:682` が `pub use surface::{CurrentSurfaceTexture, SurfaceStatus, validate_surface_size};`。数えたコマンドは `grep -n 'mod surface\|pub use surface' crates/core/src/graphics.rs`。

### 再実行の契機

`crates/core/src/graphics/surface.rs`・`crates/core/tests/graphics_surface_types.rs` を変えたとき、`scripts/public-api-check.sh` などの公開 API の検査のスクリプトを変えたとき。

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #5347 | 本頁の関数 |
| 2/4 | #5348 | 取得の結果の 2 つの型と対応 |
| 3/4 | #5349 | 外の crate からの境界の入力と対応の試験 |
| 4/4 | #5350 | 完了の条件の確かめ（本頁の 4/4 の節） |
