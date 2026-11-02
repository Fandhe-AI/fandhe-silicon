# テクスチャの値・記述子・handle の型（TASK-371）の所在と本リポの現状

TASK-371 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-371。ポインタのみ）。
内容・理由・経緯は本書に置かない。

## 本リポの現状

確かめ方は、リポジトリのルートで流したコマンドである。

### core の構成

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 条件なしで公開する module | `artifact_cache`・`binding`・`capability`・`decision`・`device`・`device_lost`・`error`・`execution`・`hash`・`memory`・`render_target`・`spirv`・`texture`・`texture_format`・`validation` の 15 個（記録時点の値。今の一覧は右の確かめ方の結果を正とする） | `grep -n "pub mod" crates/core/src/lib.rs` |
| feature の裏の module | `ext-interop` の裏に `interop`（記録時点の値）。`test-support` の裏に `capability_rule`・`parity`・`test_frame`・`rerun`・`stub_device`。`fault-injection` の裏に `fault`。`graphics` の裏に `graphics`（記録時点の値） | 同上 |
| 外部依存 | 0 件 | `cargo tree -p fandhe-silicon-core -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/core/src/lib.rs` |

### 先例の所在

| 物 | 所在 | ポインタ |
| -- | ---- | -------- |
| `TextureFormat` | `crates/core/src/texture_format.rs` | TASK-1312 |
| `BufferUsage`・`BufferHandle` | `crates/core/src/memory/buffer.rs` | TASK-308・TASK-310 |
| `RenderTargetHandle` | `crates/core/src/render_target.rs` | TASK-375 |

### 公開 API・依存の検査が core を既に覆っていること

| 検査 | 入口 |
| ---- | ---- |
| 公開 API の wgpu 系の参照（TASK-10） | `make public-api-check` |
| チップの SDK の crate（TASK-13） | `make public-api-sdk-check` |
| 公開 API の名前 | `make public-name-check` |
| 公開 API の WebGPU 系の名前 | `make public-api-webgpu-check` |
| core の外部依存と feature の転送 | `make core-deps-check` |
| 依存グラフの wgpu 系 crate | `make dep-graph-wgpu-check` |

## 現状の形（1/6）

所在は `crates/core/src/texture.rs`（`pub mod texture;`。feature の条件なし。crate の根の `pub use` は無い）。

| 型 | 形 | derive | `#[non_exhaustive]` | 持たないもの |
| -- | -- | ------ | ------------------- | ------------ |
| `TextureDimension` | variant は `D1`・`D2`・`D3` | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash` | 付く | なし |
| `TextureExtent` | 公開の欄 `width`・`height`・`depth_or_array_layers`（すべて `u32`） | 同上 | 付かない | `Default`・組み立ての関数 |
| `TextureUsage` | 非公開の `u32`。定数 `COPY_SRC`=1・`COPY_DST`=2・`TEXTURE_BINDING`=4・`STORAGE_BINDING`=8・`RENDER_ATTACHMENT`=16・`TRANSIENT_ATTACHMENT`=32・`STORAGE_ATOMIC`=64。`empty`・`is_empty`・`bits`・`contains`・`\|`・`\|=` | 同上 | 付かない | ビットから作る口・`Default` |

### 対応する doctest

| 型 | 確かめること |
| -- | ------------ |
| `TextureDimension` | 外の crate の `_` の腕の無い `match` が `compile_fail,E0004`。`_` の腕つきは通る |
| `TextureExtent` | 構造体式で作れる。コピー後も元を使える。`==` で比べられる |
| `TextureUsage` | 7 定数のビット値。`\|` と `contains`。`empty`・`is_empty`。`\|=` |

確かめ方: `cargo test -p fandhe-silicon-core --doc texture.rs`（通常 3 件・`compile_fail` 1 件、記録時点の値）。

## 現状の形（2/6）

所在は `crates/core/src/texture.rs`（`fandhe_silicon_core::texture::TextureDescriptor`）。

| 型 | derive | `#[non_exhaustive]` | 持たないもの |
| -- | ------ | ------------------- | ------------ |
| `TextureDescriptor<'a>` | `Debug`・`Clone`・`PartialEq`・`Eq` | 付く | `Copy`・`Hash`・`Default` |

欄はすべて公開で、順と型は次のとおり。

| 欄 | 型 |
| -- | -- |
| `label` | `Option<&'a str>` |
| `dimension` | `TextureDimension` |
| `size` | `TextureExtent` |
| `mip_level_count` | `u32` |
| `sample_count` | `u32` |
| `format` | `TextureFormat` |
| `usage` | `TextureUsage` |
| `view_formats` | `&'a [TextureFormat]` |

`new(dimension, size, format, usage)` は `label` を `None`、`mip_level_count` と `sample_count` を 1、`view_formats` を空にし、ほかは引数をそのまま入れる。値は検査しない。

### 対応する doctest

| 確かめること | 種別 |
| ------------ | ---- |
| `new` の既定の値と、渡した値の保持 | 通常 |
| 欄の書き換え（`label`・`mip_level_count`・`view_formats`） | 通常 |
| 0 の大きさと空の用途を `new` がそのまま保持する | 通常 |
| 外の crate の構造体式 | `compile_fail,E0639` |

確かめ方: `cargo test -p fandhe-silicon-core --doc texture.rs`（この節で通常 3 件・`compile_fail` 1 件が増える）。

## 現状の形（3/6）

所在は `crates/core/src/texture.rs`（`fandhe_silicon_core::texture::TextureHandle`）。

| 型 | derive | `#[non_exhaustive]` | 持たないもの |
| -- | ------ | ------------------- | ------------ |
| `TextureHandle` | `Debug` | 付かない | `Clone`・`Copy`・`PartialEq`・寿命の型引数 |

欄はすべて非公開で、順と型は次のとおり。`label` と `view_formats` は持たない。

| 欄 | 型 |
| -- | -- |
| `generation` | `u64` |
| `id` | `u64` |
| `device_id` | `u64` |
| `dimension` | `TextureDimension` |
| `size` | `TextureExtent` |
| `mip_level_count` | `u32` |
| `sample_count` | `u32` |
| `format` | `TextureFormat` |
| `usage` | `TextureUsage` |

背後実装向けの口（`#[doc(hidden)]`。検査しない）。

| 口 | 内容 |
| -- | ---- |
| `__backend_new(generation, id, device_id, &TextureDescriptor)` | 記述子の `dimension`・`size`・`mip_level_count`・`sample_count`・`format`・`usage` を写して持つ |
| `__backend_generation` | 世代を返す |
| `__backend_id` | 識別子を返す |
| `__backend_device_id` | device の識別子を返す |

問い合わせ（公開。`&self`。`Result` を返さない）。

| 名前 | 戻り値 | 返す値（記述子の欄） |
| ---- | ------ | -------------------- |
| `size` | `TextureExtent` | `size` |
| `width` | `u32` | `size.width` |
| `height` | `u32` | `size.height` |
| `depth_or_array_layers` | `u32` | `size.depth_or_array_layers` |
| `mip_level_count` | `u32` | `mip_level_count` |
| `sample_count` | `u32` | `sample_count` |
| `dimension` | `TextureDimension` | `dimension` |
| `format` | `TextureFormat` | `format` |
| `usage` | `TextureUsage` | `usage` |

### 対応する doctest

| 確かめること | 種別 |
| ------------ | ---- |
| `__backend_new` に渡した値の往復と、9 つの問い合わせが記述子の値を返すこと | 通常 |
| 外の crate の構造体式 | `compile_fail,E0451` |
| `clone` の呼び出し | `compile_fail,E0599` |

確かめ方: `cargo test -p fandhe-silicon-core --doc texture.rs`（この節で通常 1 件・`compile_fail` 2 件が増える）。

## 試験（4/6）

`crates/core` の `src/` は変えず、試験のファイルを 1 つ足した。公開 API は変わらない。

### 対応するテスト

| ファイル | テスト | 確かめること |
| -------- | ------ | ------------ |
| `crates/core/tests/texture_types.rs` | `queries_return_descriptor_values` | 次元・形式・用途・層・mip・サンプルの異なる 5 つの記述子で、9 つの問い合わせが記述子の値を返す |
| 同上 | `queries_return_boundary_values_unchanged` | 大きさ・mip・サンプルが 0 と `u32::MAX` でも、値を検査せずそのまま返す |
| 同上 | `descriptor_new_defaults` | `new` が `label` なし・mip 1・サンプル 1・`view_formats` 空にし、渡した 4 つの値を保つ |
| 同上 | `handle_outlives_descriptor` | 記述子と借りた値を破棄した後でも、handle の問い合わせが値を返す |
| 同上 | `backend_readers_round_trip` | 背後実装向けの 3 つの読み出しが、渡した値（0・`u64::MAX` を含む）を返す |
| 同上 | `usage_constants_have_listed_bits` | 用途の 7 定数が 1 ビットずつ（1 から 64）で互いに重ならず、論理和が `0x7f` |
| 同上 | `dimension_values_from_outside` | 次元の 3 値が外の crate から照合でき、互いに異なる |

確かめ方: `cargo test -p fandhe-silicon-core --test texture_types`（記録時点で 7 件）。

## CI への組み込み（5/6）

範囲は `docs/spec/05-tasks.md` の TASK-371 の内容（ポインタのみ）。
本段では CI の設定・Makefile・スクリプトを変えていない（新しいジョブ・step・make ターゲットは足していない）。
1/6〜4/6 で足した `texture` module の型・doctest と `crates/core/tests/texture_types.rs` は、既存の検査が core 全体に当てる範囲に入っている。

### 検査と CI のジョブ

| 検査 | 入口 | CI のジョブ | 結果（記録時点） |
| ---- | ---- | ----------- | ---------------- |
| 公開 API の wgpu 系の参照 | `make public-api-check` | `deny` | 終了コード 0。core は production で `public_items=3079`・all-features で `public_items=7088`、`record:` は `public_items_total=10844`・`wgpu_type_refs_total=0`・`wgpu_other_refs_total=0` |
| 公開 API のチップの SDK の crate | `make public-api-sdk-check` | `deny` | 終了コード 0。`record:` は `public_items_total=9205`・`sdk_type_refs_total=0`・`sdk_other_refs_total=0`・`common_chip_deps=0` |
| 公開 API の WebGPU 系の名前 | `make public-api-webgpu-check` | `deny` | 終了コード 0。`record:` は `public_items_total=2714`・`blocking_wait_names_total=0`・`webgpu_names_total=0` |
| 公開 API の名前 | `make public-name-check` | `deny` | 終了コード 0。core は production で `named_items=949`・all-features で `named_items=2198`、`record:` は `public_items_total=9205`・`name_hits_total=0` |
| core の外部依存と feature の転送 | `make core-deps-check` | `deny` | 終了コード 0。`record:` は `unexpected=0`・`missing=0`・`forward_violations=0` |
| 依存グラフの wgpu 系 crate | `make deny` の中の `dep-graph-wgpu-check` | `deny` | 終了コード 0。`record:` は `crates_total=13`・`narrow_total=0`・`wide_total=0` |
| core の試験を含む wasm32 の型検査 | `make wasm-verify`（`scripts/wasm-verify.sh` の 36 行目・39 行目の `cargo check -p fandhe-silicon-core --tests`。既定と `--all-features`） | `deny`・`wasm-macos` | 終了コード 0 |
| fmt・clippy・test | `make verify`（`--all-features`） | `verify`・`verify-macos` | 終了コード 0。`texture_types` は `test result: ok. 7 passed`、`texture.rs` の doctest は `test result: ok. 4 passed` |
| feature の組ごとの build・clippy・test | `make build-profiles`（`scripts/build-profiles.sh` の 85 行目・86 行目の core の feature なしの `cargo clippy`・`cargo test`。`texture_types` は `[[test]]` の項目が無く feature なしでも流れる） | `verify`・`verify-macos` | 終了コード 0 |

ジョブは `ci.yml` のジョブ id で書く（`deny` は make deny (cargo-deny) と各検査、`wasm-macos` は wasm32 build + size + node (macos-latest)、`verify` は make verify + build-profiles、`verify-macos` は macOS 版）。
件数は記録時点（origin/main の fe83bbf）の値で、後の段で型や公開アイテムが増えても本節は書き換えない。

### 確かめ方

```text
grep -n 'run: make' .github/workflows/ci.yml
grep -n 'cargo check' scripts/wasm-verify.sh
grep -n 'CORE_PKG' scripts/build-profiles.sh
make public-api-check
make public-api-sdk-check
make public-api-webgpu-check
make public-name-check
make core-deps-check
make deny
make wasm-verify
make verify
make build-profiles
cargo test -p fandhe-silicon-core --test texture_types
cargo test -p fandhe-silicon-core --doc texture.rs
```

## 完了の条件の確かめ（6/6）

TASK-371 の 6/6（#4562）として、本リポで観測できる事実だけを記録する。GPU の実機のない Linux の隔離した作業ツリー（base `51be40a`）で流した出力による。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 型が共通の crate に条件なしで置かれている | `grep -n 'pub mod texture' crates/core/src/lib.rs`、`cargo check -p fandhe-silicon-core`（feature なし） | `lib.rs:48` が `pub mod texture;` で `cfg` は付かない。`cargo check` の終了コードは 0 |
| 2 | 9 つの問い合わせが記述子の値と一致する値を返す（`size`・`width`・`height`・`depth_or_array_layers`・`mip_level_count`・`sample_count`・`dimension`・`format`・`usage`） | `cargo test -p fandhe-silicon-core --test texture_types queries_return_descriptor_values`、`cargo test -p fandhe-silicon-core --doc texture.rs` | `queries_return_descriptor_values` は `ok`。`texture::TextureHandle (line 294)` の doctest も `ok`（`check_queries` が 9 つの問い合わせを行ごとに突き合わせる） |
| 3 | 作成時の値を保持して返す | `--test texture_types` | `handle_outlives_descriptor`・`queries_return_boundary_values_unchanged` が `ok`。`texture_types` 全体は `test result: ok. 7 passed` |
| 4 | 記述子の形式の欄が `TextureFormat` を受ける | `grep -n 'pub format' crates/core/src/texture.rs`、`grep -n Astc crates/core/tests/texture_types.rs` | `texture.rs:254` が `pub format: TextureFormat`。試験の `:54` に `TextureFormat::Astc { block: AstcBlock::B4x4, channel: AstcChannel::Unorm }` を含む行がある |
| 5 | 公開 API に wgpu の型が 0 件（TASK-10） | `make public-api-check` | 終了コード 0。`public_items_total=10844 wgpu_type_refs_total=0 wgpu_other_refs_total=0` |
| 6 | 公開 API にチップの SDK の型が 0 件（TASK-13） | `make public-api-sdk-check` | 終了コード 0。`public_items_total=9205 sdk_type_refs_total=0 sdk_other_refs_total=0 common_chip_deps=0` |
| 7 | ほかの公開 API の検査 | `make public-name-check`・`make public-api-webgpu-check`・`make core-deps-check` | 終了コードはすべて 0。`name_hits_total=0`・`blocking_wait_names_total=0 webgpu_names_total=0`・`unexpected=0 missing=0 forward_violations=0` |
| 8 | CI で流れる場所 | 5/6 の節の表。`grep -n 'run: make' .github/workflows/ci.yml`。`gh run list --branch main --workflow ci.yml` | 5/6 の表の呼び出し（`make verify`・`make build-profiles`・`make deny`・`make wasm-verify`・`make public-api-check` など）が `ci.yml` に並ぶ。main の push の run は 1/6 `1a5c264`（41322889975）・2/6 `a6fc3d9`（41324185827）・3/6 `b63103e`（41325608467）・4/6 `fe83bbf`（41327418646）が success（completed）、5/6 `51be40a`（41328825333）は記録時点で in_progress |
| 9 | 依存 0・unsafe の追加 0 | `git diff --stat 124f4da HEAD -- Cargo.lock deps-approved.toml crates/core/Cargo.toml`、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n unsafe_code crates/core/src/lib.rs`、`grep -c unsafe crates/core/src/texture.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | 3 つのファイルの差分は空。承認の記録は 4 件。`lib.rs:32` は `#![forbid(unsafe_code)]`。`texture.rs` の `unsafe` は 0。`cargo tree` は core 自身の 1 行だけ |
| 10 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | 終了コードはすべて 0 |

上の表の `lib.rs:48` は記録時点の値で、今は `lib.rs:58` が `pub mod texture;` である。

`texture.rs` の doctest は `cargo test --doc texture.rs` が 2 つの結果行を出し、通常の 7 件（`ok. 7 passed`）と `compile_fail` の 4 件（`ok. 4 passed`。`TextureDimension (line 13)`・`TextureDescriptor (line 223)`・`TextureHandle (line 325)`・`TextureHandle (line 345)`）である。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-371）。確かめた base は `51be40a`。

### 確かめ方

```text
grep -n 'pub mod texture' crates/core/src/lib.rs
cargo check -p fandhe-silicon-core
cargo test -p fandhe-silicon-core --test texture_types
cargo test -p fandhe-silicon-core --doc texture.rs
make public-api-check
make public-api-sdk-check
make public-api-webgpu-check
make public-name-check
make core-deps-check
make check
make verify
make deny
make deps-check
make build-profiles
make wasm-verify
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/6 | #4557 | 次元・大きさ・用途の型 |
| 2/6 | #4558 | 記述子 |
| 3/6 | #4559 | handle と問い合わせ |
| 4/6 | #4560 | 試験 |
| 5/6 | #4561 | CI への組み込み |
| 6/6 | #4562 | 完了の条件の確かめ |

関連: テクスチャの作成の記述子の検査は [texture-descriptor-check.md](texture-descriptor-check.md)。

関連: storage texture のアクセスの型と、束縛の用途・シェーダーとの照合の検査は [storage-texture-access.md](storage-texture-access.md)（TASK-1944）。
