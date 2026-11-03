# device からテクスチャを作る操作へ届く口の所在（TASK-150）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-150。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/ops.rs`・`crates/core/src/device.rs` |
| 公開のパス | `fandhe_silicon_core::texture::TextureOps`・`fandhe_silicon_core::device::Device::textures` |
| feature の条件 | なし |

記述子と handle の型の所在は [texture-types.md](texture-types.md)（TASK-371）。

## 現状の形（1/4）

- `Device::textures` は `crates/core/src/device.rs:191` にあり、署名は `fn textures(&self) -> Option<&dyn crate::texture::TextureOps>`。既定の本体は `None`
- `TextureOps` は `crates/core/src/texture/ops.rs` の trait で、`texture` モジュールの末尾（`crates/core/src/texture.rs:490`）から再公開される。メソッドは `fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Result<TextureHandle, ContractError>` の 1 つで、既定の本体は無い。持たない実装は `E0046` で拒まれる（trait の doctest）

確かめ方:

```sh
grep -n 'fn textures' crates/core/src/device.rs
grep -n 'pub use ops' crates/core/src/texture.rs
cargo test -p fandhe-silicon-core --doc texture::ops
```

## 試験用の device の答え（2/4）

- 所在は `crates/core/src/stub_device.rs`（feature `test-support` の裏）。非公開の `StubTextures` が `TextureOps` を満たし、`impl Device for StubDevice` の `textures()` が返す
- `textures()` は要求のプロファイルで変わり、所有形態によらない

| プロファイル | `textures()` |
| ------------ | ------------ |
| `CpuOnly` | `None` |
| `ComputeOnly` | `Some` |
| `Graphics` | `Some` |

- `Some` の `create_texture` は `Ok(TextureHandle)` を返す。handle の device の識別子と世代は `generation()` と等しく、通し番号は device ごとに 1 から増える。通し番号が尽きたら `BackendFailure { op: "create_texture", .. }` を返す
- 試験用の device は記述子を検査しない（この device の振る舞い）

確かめ方:

```sh
grep -n 'fn textures' crates/core/src/stub_device.rs
cargo test -p fandhe-silicon-core --features test-support --lib stub_device
```

## 到達の試験（3/4）

- 所在は `crates/core/tests/stub_device_textures.rs`（feature `test-support`。`crates/core/Cargo.toml` の `[[test]]` の `required-features`）と `crates/core/tests/device_trait_shape.rs`（feature なしでも流れる）
- 固定した値: `backend` は `BackendKind::Vulkan`、`Imported` は `NativeDeviceHandle::new(7u32)`。記述子は `D2`・64 × 32 × 1・`Rgba8Unorm`・`TEXTURE_BINDING`、`label` は `Some("reach")`

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `cpu_only_{product_independent,shared,imported}_textures_is_none`（3 本） | `CpuOnly` × 3 所有形態 | `textures()` が `None` |
| `compute_only_{product_independent,shared,imported}_textures_creates_texture`（3 本） | `ComputeOnly` × 3 所有形態、`label` あり | `Ok`。device の識別子が `generation()` と等しく、9 つの問い合わせが渡した値を返す |
| `graphics_{product_independent,shared,imported}_textures_creates_texture`（3 本） | `Graphics` × 3 所有形態、`label` あり | 同上 |
| `descriptor_without_label_also_reaches` | `label` が `None` | `Ok`。問い合わせが渡した値を返す |
| `two_textures_from_one_device_have_distinct_ids` | 1 つの device から 2 回 | 通し番号が違い、device の識別子はどちらも `generation()` と等しい |
| `default_textures_is_none_through_dyn_device`（`device_trait_shape.rs`） | 上書きしない device | `textures()` が `None` |

確かめ方:

```sh
cargo test -p fandhe-silicon-core --features test-support --test stub_device_textures
cargo test -p fandhe-silicon-core --test device_trait_shape
```

## 完了の条件の確かめ（4/4）

4/4（#3697）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-150）。GPU の実機なしの Linux で実行し、件数は実測。コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | `Device` からテクスチャを作る操作へ届く口が公開されている | `grep -n 'fn textures' crates/core/src/device.rs`、`grep -n 'pub use ops' crates/core/src/texture.rs`、`cargo check -p fandhe-silicon-core` | `device.rs:191` に `fn textures`、`texture.rs:490` に `pub use ops::TextureOps`。`cargo check` は feature なしで終了コード 0 |
| 2 | 既定の本体が `None` | `cargo test -p fandhe-silicon-core --test device_trait_shape` | `default_textures_is_none_through_dyn_device ... ok`。7 passed、0 failed |
| 3 | 試験用の device のプロファイル・所有形態ごとの答え | `cargo test -p fandhe-silicon-core --features test-support --test stub_device_textures` | `cpu_only_{product_independent,shared,imported}_textures_is_none` の 3 本が `ok`。11 passed、0 failed |
| 4 | 記述子（`label` あり・なし）を渡して作る操作へ届き、handle の問い合わせが渡した値を返す | 同上 | `compute_only_{…}_textures_creates_texture` と `graphics_{…}_textures_creates_texture` の 6 本、`descriptor_without_label_also_reaches`、`two_textures_from_one_device_have_distinct_ids` が `ok` |
| 5 | 背後実装を変えていない | `git show --stat --format= <sha> -- crates/contract`（`23bd9eb`・`6b60de8`・`1376f6a`） | 3 つとも出力なし（差分 0 件） |
| 6 | 依存 0・unsafe 0 | `git show --stat --format= <sha> -- Cargo.lock deps-approved.toml crates/core/Cargo.toml`、`grep -c unsafe crates/core/src/texture/ops.rs`、`grep -n forbid crates/core/src/lib.rs` | `Cargo.lock`・`deps-approved.toml` の差分は 0 件。`crates/core/Cargo.toml` は `1376f6a` の 4 行の追加（`[[test]]` の 1 項目）だけ。`unsafe` は 0 件。`lib.rs:38` に `#![forbid(unsafe_code)]` |
| 7 | 試験の件数と CI で流れる場所 | `make verify`、`make wasm-verify`。`grep -n 'run: make' .github/workflows/ci.yml` | `make verify` は終了コード 0 で `device_trait_shape` と `stub_device_textures`（11 tests）を流した。`make wasm-verify` は終了コード 0。ci.yml は `make verify` が 177・498 行目、`make wasm-verify` が 275・459 行目 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-150）。確かめた base は `1376f6a2f5ffd739e95b571614e4b2c9c096c6fa`。

確かめ方:

```text
grep -n 'fn textures' crates/core/src/device.rs
cargo test -p fandhe-silicon-core --test device_trait_shape
cargo test -p fandhe-silicon-core --features test-support --test stub_device_textures
make verify
make wasm-verify
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #3694 | 現状の形 |
| 2/4 | #3695 | 試験用の device の答え |
| 3/4 | #3696 | 到達の試験 |
| 4/4 | #3697 | 完了の条件の確かめ |

親 Issue は #419。

上の表の `lib.rs:38` は記録時点の値で、その後 `lib.rs` に行が入り、今は `lib.rs:39` が `#![forbid(unsafe_code)]` である。
