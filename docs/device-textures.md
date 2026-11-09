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

今の値の補い: 上の「現状の形（1/4）」の `TextureOps` のメソッドは、今は `create_texture`・`destroy_texture`・`destroy_view` の 3 つ（どれも既定の本体は無い）。破棄の 2 つの所在は [texture-view-destroy-copy.md](texture-view-destroy-copy.md)（TASK-372）。

今の値の補い（base 1890c9a）: `TextureOps` のメソッドは今は 4 つ（`create_texture`・`destroy_texture`・`destroy_view`・`create_sampler`）。

## サンプラーを作る操作（TASK-150）

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/texture/ops.rs`・`crates/core/src/stub_device.rs` |
| 試験 | `crates/core/tests/stub_device_samplers.rs`（feature `test-support`） |
| 公開のパス | `fandhe_silicon_core::texture::TextureOps::create_sampler` |

サンプラーの記述子と handle の型の所在は [sampler-types.md](sampler-types.md)。

### 現状の形

- 署名は `fn create_sampler(&self, desc: &SamplerDescriptor<'_>) -> Result<SamplerHandle, ContractError>`。既定の本体は無く、持たない実装は `E0046` で拒まれる
- `TextureOps` の操作は今は 4 つ
- 試験用の device（`StubDevice`）の `Device::textures` が `Some` のとき、`create_sampler` は記述子を検査せず `Ok(SamplerHandle)` を返す。handle の device の識別子と世代は `generation()` と等しい
- 通し番号はテクスチャとは別の源から device ごとに 1 から増える。尽きたら `BackendFailure { op: "create_sampler", .. }` を返す（panic しない）

| プロファイル | `Device::textures` | `create_sampler` |
| ------------ | ------------------ | ---------------- |
| `CpuOnly` | `None` | 届かない |
| `ComputeOnly` | `Some` | `Ok` |
| `Graphics` | `Some` | `Ok` |

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `cpu_only_*_has_no_sampler_path`（3 本） | `CpuOnly` × 3 つの所有形態 | `textures()` が `None` |
| `compute_only_*_creates_sampler`・`graphics_*_creates_sampler`（6 本） | `label` ありの既定の記述子 | `Ok`、handle の device の識別子と世代が `generation()` |
| `sampler_descriptor_without_label_also_reaches` | `label` なし | `Ok` |
| `two_samplers_from_one_device_have_distinct_ids` | 2 回作る | 識別子が違う |
| `create_sampler_numbers_from_one_and_uses_generation`（core の中） | 2 回作る | 識別子 1・2 |
| `sampler_numbering_is_separate_from_textures`（core の中） | テクスチャの後に作る | 識別子 1 |
| `create_sampler_reports_exhausted_id_space`（core の中） | 通し番号を末尾の手前にする | 1 回は `Ok`、次は `BackendFailure` |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --doc texture::ops
cargo test -p fandhe-silicon-core --features test-support --lib stub_device
cargo test -p fandhe-silicon-core --features test-support --test stub_device_samplers
```

## 今の値の補い（base 6b35dd9）

上の表の項目 7 の `ci.yml` の行番号は記録時点の値で、書き換えない。今の `.github/workflows/ci.yml` では `make verify` が 177・505 行目、`make wasm-verify` が 276・466 行目である（数えたコマンド: `grep -nE 'run: make (verify|wasm-verify|build-profiles|profile-test-count-check)$' .github/workflows/ci.yml`）。

## 作る操作の行の能力の問合せ（1/2）

テクスチャを作る操作・サンプラーを作る操作の行の能力の問合せの鍵と、試験用の device の答えの所在（TASK-150）。鍵の文字列は定数の定義を正とし、この頁には写さない。

| 項目 | 所在 |
| ---- | ---- |
| 鍵の定数 | `crates/core/src/capability.rs`（`fandhe_silicon_core::capability::API_DEVICE_CREATE_TEXTURE`・`API_DEVICE_CREATE_SAMPLER`。feature の条件なし） |
| 試験用の device の答え | `crates/core/src/stub_device.rs`（`query_ex` の分岐と、`query` が `query_ex` の段階を返す形） |
| 外の crate からの試験 | `crates/core/tests/stub_device_create_keys.rs`（feature `test-support`） |
| core の中の試験 | `crates/core/src/stub_device.rs` の `mod tests` の 2 本 |

### 試験用の device の答え

| 鍵 | プロファイル | `query` | `query_ex` |
| -- | ------------ | ------- | ---------- |
| `API_DEVICE_CREATE_TEXTURE` | どのプロファイルでも | `S2` | `S2`・理由なし・根拠なし |
| `API_DEVICE_CREATE_SAMPLER` | どのプロファイルでも | `S2` | `S2`・理由なし・根拠なし |

答えは所有形態と `backend` にもよらない。試験用の device の `query` は `query_ex` と同じ段階を返す。WebGPU の口の 2 鍵・形式の問合せの鍵・表に無い鍵の答えは変わらず、`query` は `Unsupported` を返す。

### 試験

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `{cpu_only,compute_only,graphics}_{product_independent,shared,imported}_answers_s2_for_create_keys`（9 本） | 3 つのプロファイル × 3 つの所有形態 | 2 つの鍵で `query` が `S2`、`query_ex` が `S2`・理由なし・根拠なし、`check_query_ex` が `Ok`。`CpuOnly` は `textures()` が `None`、ほかは `Some` で `create_texture`・`create_sampler` が `Ok` |
| `answers_do_not_depend_on_backend` | 4 つの `BackendKind` × `ComputeOnly`・`Shared`/`ProductIndependent` | 2 つの鍵の `query_ex` が同じ値 |
| `create_key_strings_are_fixed` | 2 つの定数 | 定義の文字列と等しい |
| `create_keys_are_outside_other_key_lists` | 2 つの定数 | 互いに違い、形式・機能・上限・downlevel の鍵の一覧と WebGPU の口の 2 鍵のどれとも違う |
| `other_keys_keep_their_answers` | `ComputeOnly`・`Shared` | `check_webgpu_ports` が `Ok`、表に無い鍵は `UNLISTED_FEATURE_QUERY`、`query` は `Unsupported` |
| `create_operation_keys_answer_s2_for_every_profile`（core の中） | 3 つのプロファイル | 2 つの鍵で `query` が `S2`、`query_ex` が `S2`・理由なし・根拠なし |
| `create_operation_keys_are_not_format_or_webgpu_keys`（core の中） | 2 つの定数 | 形式の鍵の一覧と WebGPU の口の 2 鍵に無く、互いに違う |

### 確かめ方

```sh
cargo test -p fandhe-silicon-core --features test-support --test stub_device_create_keys
cargo test -p fandhe-silicon-core --features test-support --lib stub_device
```

## 作る操作の行の鍵の完了の条件の確かめ（2/2）

2/2（#8134）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-150）。GPU の実機なしの Linux で実行し、件数は実測。コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 2 つの鍵の定数が公開されている | `grep -n 'pub const API_DEVICE_CREATE' crates/core/src/capability.rs`、`cargo check -p fandhe-silicon-core` | `capability.rs:187` に `API_DEVICE_CREATE_TEXTURE`、`:191` に `API_DEVICE_CREATE_SAMPLER`。`cargo check` は feature なしで終了コード 0 |
| 2 | 鍵の文字列が固定されている | `cargo test -p fandhe-silicon-core --features test-support --test stub_device_create_keys` | `create_key_strings_are_fixed ... ok`。13 passed、0 failed |
| 3 | 3 プロファイル × 3 所有形態で 2 つの鍵が `S2` を返し、`query`・`query_ex` が同じ段階 | 同上、`cargo test -p fandhe-silicon-core --features test-support --lib stub_device` | `{cpu_only,compute_only,graphics}_{product_independent,shared,imported}_answers_s2_for_create_keys` の 9 本が `ok`。`create_operation_keys_answer_s2_for_every_profile`・`create_operation_keys_are_not_format_or_webgpu_keys` が `ok`（`--lib stub_device` は 28 passed、0 failed） |
| 4 | `textures()` が `Some` の device で作る操作が `Ok` | 同上 | `compute_only_*`・`graphics_*` の 6 本が `ok` |
| 5 | ほかの鍵の答えが変わっていない | `other_keys_keep_their_answers`、`cargo test -p fandhe-silicon-core --features test-support --test stub_device_request` | `other_keys_keep_their_answers ... ok`。`stub_device_request` は 9 passed、0 failed |
| 6 | 背後実装を変えていない | 範囲 `dcc458ad1ad39fdb1795370fb4a271fdb91ddfc4..3f50c27f875afb2d9219439c631a3cfca4eddaf7` の `crates/contract` の差分の統計 | 出力なし（差分 0 件） |
| 7 | 依存 0・unsafe 0 | 同じ範囲の差分を `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` に当てる。`grep -n forbid crates/core/src/lib.rs` | `Cargo.lock`・`deps-approved.toml` の差分は 0 件。`crates/core/Cargo.toml` は 4 行の追加（`[[test]]` の 1 項目）だけ。`lib.rs:39` に `#![forbid(unsafe_code)]` |
| 8 | CI で流れる場所 | `make verify`、`make build-profiles`、`make wasm-verify`。`grep -nE 'run: make (verify\|wasm-verify\|build-profiles)$' .github/workflows/ci.yml` | 3 つとも終了コード 0。`make verify` の `test result:` の合計は 2570 passed、0 failed、66 ignored。ci.yml は `make verify` が 177・520 行目、`make build-profiles` が 181・523 行目、`make wasm-verify` が 282・481 行目 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-150）。確かめた base は `3f50c27f875afb2d9219439c631a3cfca4eddaf7`。

確かめ方:

```text
grep -n 'pub const API_DEVICE_CREATE' crates/core/src/capability.rs
cargo check -p fandhe-silicon-core
cargo test -p fandhe-silicon-core --features test-support --test stub_device_create_keys
cargo test -p fandhe-silicon-core --features test-support --lib stub_device
cargo test -p fandhe-silicon-core --features test-support --test stub_device_request
grep -n forbid crates/core/src/lib.rs
make verify
make build-profiles
make wasm-verify
```
