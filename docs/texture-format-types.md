# テクスチャの形式の型（TASK-1312）の所在と本リポの現状

TASK-1312 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1312・`04-behavior/` の COVT-2・COVT-117。ポインタのみ）。
内容・理由・経緯は本書に置かない。

前提のタスクと、本リポで対応する物。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-10 | `scripts/public-api-check.sh` | [public-api-wgpu-check](public-api-wgpu-check.md) |
| TASK-11 | `scripts/dep-graph-wgpu-check.sh` | [dep-graph-wgpu-check](dep-graph-wgpu-check.md) |
| TASK-13 | `scripts/public-api-sdk-check.sh` | [public-api-sdk-check](public-api-sdk-check.md) |
| TASK-23 | `crates/core/src/test_frame*`（feature `test-support` の裏） | [test-frame](test-frame.md) |
| TASK-136 | `crates/core/src/device.rs`・`crates/core/src/capability.rs` | [contract-device-types](contract-device-types.md) |
| TASK-1305 | private spec 側の記録に従う（ポインタのみ） | なし |

## 本リポの現状

確かめ方は、リポジトリのルートで流したコマンドである。

### core の構成

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 条件なしで公開する module | `artifact_cache`・`binding`・`capability`・`decision`・`device`・`device_lost`・`error`・`execution`・`hash`・`memory`・`validation` の 11 個 | `grep -n "pub mod" crates/core/src/lib.rs` |
| feature の裏の module | `ext-interop` の裏に `interop`。`test-support` の裏に `capability_rule`・`parity`・`test_frame`・`rerun`。`fault-injection` の裏に `fault` | 同上 |
| feature | `test-support`・`fault-injection`・`native-blocking-wait`・`ext-interop` の 4 つ。どれも `= []` で、`default` は無い | `crates/core/Cargo.toml` の `[features]` |
| 外部依存 | 0 件（`[dependencies]` が無く、`cargo tree` に子が出ない） | `cargo tree -p fandhe-silicon-core -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/core/src/lib.rs` |

### 型の有無と先例

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `PixelFormat`・`DepthFormat`・`TextureFormat` | `crates/`・`wasm/`・`measure/`・`baseline-wgpu/` に定義も参照も 0 件（終了コード 1） | `grep -rn -e PixelFormat -e DepthFormat -e TextureFormat crates wasm measure baseline-wgpu`（`target/` は除く） |
| `FeatureId` | `crates/core/src/capability.rs` の `pub struct FeatureId(pub &'static str);` | `grep -n "pub struct FeatureId" crates/core/src/capability.rs` |
| `FeatureId` の先例の試験 | `crates/core/tests/capability_query_default.rs` がある | `ls crates/core/tests` |
| `_` の腕の無い網羅 `match` の先例 | `crates/core/src/artifact_cache.rs` の `mod tests`（`RejectReason` の `match`） | `grep -n "mod tests\|match r" crates/core/src/artifact_cache.rs` |
| `#[non_exhaustive]` の先例の試験 | `crates/core/tests/` の `binding_non_exhaustive.rs`・`device_capability_non_exhaustive.rs`・`error_non_exhaustive.rs`・`error_non_exhaustive_eight.rs` の 4 本 | `ls crates/core/tests \| grep non_exhaustive` |

### 公開 API・依存の検査が core を既に覆っていること

どの検査も CI の `deny` ジョブ（`.github/workflows/ci.yml`）が呼ぶ。

| 検査 | 対象 | 入口 |
| ---- | ---- | ---- |
| 公開 API の wgpu 系の参照 | 製品の 4 crate（core を含む）の公開アイテム | `make public-api-check` |
| 公開 API の名前 | 製品の crate の公開アイテム名（enum の variant を含む）を `public-name-check.conf` の `[names]` に照らす。`[words]` は `--source` に渡したファイル（`crates/core/src/decision.rs`）の行に単語境界で当てる | `make public-name-check` |
| 公開 API の WebGPU 系の名前 | `fandhe-silicon-core`・`fandhe-silicon-contract`。`public-api-webgpu-check.conf` の `[blocking-words]`・`[webgpu-words]` | `make public-api-webgpu-check` |
| チップの SDK の crate | 製品の 4 crate の公開 API と `crates/core` の依存宣言 | `make public-api-sdk-check` |
| core の外部依存の一覧 | `crates/core` の宣言する外部の crate と、contract からの feature の転送 | `make core-deps-check` |
| 依存グラフの wgpu 系 crate | 製品の workspace 全体 | `make dep-graph-wgpu-check`（`make deny` の中でも流れる） |

### 比較基準の wgpu-types

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 固定されている版 | `baseline-wgpu/Cargo.lock` が `wgpu-types` 30.0.1 を固定 | `grep -A2 'name = "wgpu-types"' baseline-wgpu/Cargo.lock` |
| ソースの所在 | `~/.cargo/registry/src/*/wgpu-types-30.0.1/src/texture/format.rs`（取得は `cargo fetch --manifest-path baseline-wgpu/Cargo.toml`） | `ls` で上の glob を確かめる |
| 公開データの型 | `TextureFormat`（77 variant）・`AstcBlock`（14）・`AstcChannel`（3） | 下の手順 |
| 語の一覧との照合 | この 3 つの型名と variant 名のいずれにも、`public-name-check.conf` の `[names]` の 6 語・`public-api-webgpu-check.conf` の `[webgpu-words]`・`[blocking-words]` の語は部分一致でも当たらない | 下の手順 |

照合の手順は、`python3 -I` で上のソースから 3 つの `pub enum` の本体を正規表現で切り出し、variant 名を取り出す。
型名と合わせた名前を小文字の英数字だけに正規化し、語の一覧の語が含まれるものを数える。結果は 0 件である。
`[words]`（`GB10` など）は `--source` のファイルの行に単語境界で当てる別の照合で、公開アイテム名の照合には使われない。

## 現状の形（2/6）

置き場は `crates/core/src/texture_format.rs`（feature の条件なし・外部依存 0・`forbid(unsafe_code)` のまま）。

### 型

| 項目 | 事実 |
| ---- | ---- |
| 型 | `TextureFormat`・`AstcBlock`・`AstcChannel` の 3 つ |
| variant の数 | `TextureFormat` は 77（ASTC 以外の 76 と `Astc { block, channel }` の 1）、`AstcBlock` は 14、`AstcChannel` は 3 |
| derive | 3 つとも `Debug, Clone, Copy, PartialEq, Eq, Hash` |
| `#[non_exhaustive]` | 3 つの型に付ける。`Astc` の variant には付けない（別の crate が値を作れる） |
| `#[repr]` | 付けない（判別子の数値は公開しない） |
| 名前と順 | `TextureFormat`・`AstcBlock`・`AstcChannel` の variant の名前と順は、比較基準の `wgpu-types` 30.0.1 の同名の型と同じ（上の照合と同じ手順で確かめられる） |
| 値の一覧 | `TextureFormat::all()` は 118 件、`AstcBlock::all()` は 14 件、`AstcChannel::all()` は 3 件のスライス（長さは型に出さない） |
| `TextureFormat::all()` の順 | ASTC 以外の 76 値を宣言順に並べ、続けて ASTC をブロックの順（外）× チャネルの順（内）の 42 件 |

### 性質の関数

どれも `&self` を取り、機材を引数に取らず、`Result` を返さず、panic しない。形式ごとの値の表は本書に置かず、`texture_format.rs` の `match` を正とする。

| 関数 | 返すもの |
| ---- | -------- |
| `has_color_aspect`・`has_depth_aspect`・`has_stencil_aspect` | 色・深度・ステンシルの面を持つか（`bool`）。`NV12`・`P010` は色の面を持たない |
| `is_depth_stencil_format` | 深度またはステンシルの面を持つか |
| `is_combined_depth_stencil_format` | 深度とステンシルの両方を持つか |
| `is_multi_planar_format`・`planes` | 複数平面か。`planes` は `NV12`・`P010` で `Some(2)`、ほかは `None` |
| `block_dimensions`・`is_compressed` | ブロックの大きさ（非圧縮は `(1, 1)`、BC・ETC2・EAC は `(4, 4)`、ASTC はブロックの大きさ）。圧縮か（`(1, 1)` でないか） |
| `remove_srgb_suffix`・`add_srgb_suffix`・`is_srgb` | sRGB の対を外す・付ける（対の無い形式は自身）。sRGB か |
| `required_feature` | 要る機能を `Option<FeatureId>` で返す。値は `"feature:<名前>"` の形で、要らない形式は `None` |

### 対応するテスト

`crates/core/tests/texture_format_counts.rs`。

| テスト | 内容 |
| ------ | ---- |
| `texture_format_all_has_118_concrete_formats` | `TextureFormat::all()` の長さが 118 |
| `texture_format_all_has_42_astc_formats` | `all()` のうち `Astc` が 42 |
| `astc_block_all_has_14_values` | `AstcBlock::all()` の長さが 14 |
| `astc_channel_all_has_3_values` | `AstcChannel::all()` の長さが 3 |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --test texture_format_counts
cargo test -p fandhe-silicon-core --all-features
```

## CI への組み込み（3/6）

範囲は `docs/spec/05-tasks.md` の TASK-1312 の内容（ポインタのみ）。
本段では CI の設定・Makefile・スクリプトを変えていない（新しいジョブ・step・make ターゲットは足していない）。
2/6 で足した `TextureFormat`・`AstcBlock`・`AstcChannel` は、既存の検査が core 全体に当てる範囲に入っている。

### 検査と CI のジョブ

| 検査 | 入口 | CI のジョブ | 結果（記録時点） |
| ---- | ---- | ----------- | ---------------- |
| 公開 API の wgpu 系の参照 | `make public-api-check` | `deny` | 終了コード 0。core は production で `public_items=2655`・all-features で `public_items=6430`、`wgpu_type_refs=0`・`wgpu_other_refs=0` |
| 公開 API のチップの SDK の crate | `make public-api-sdk-check` | `deny` | 終了コード 0。core は production で `public_items=2341`・all-features で `public_items=5718`、`sdk_type_refs=0`・`sdk_other_refs=0` |
| 公開 API の WebGPU 系の名前 | `make public-api-webgpu-check` | `deny` | 終了コード 0。`record:` は `public_items_total=2345`・`webgpu_names_total=0`・`blocking_wait_names_total=0` |
| 公開 API の名前 | `make public-name-check` | `deny` | 終了コード 0。core は production で `named_items=822`・all-features で `named_items=2011`、いずれも `name_hits=0` |
| core の外部依存と feature の転送 | `make core-deps-check` | `deny` | 終了コード 0。`record:` は `unexpected=0`・`missing=0`・`forward_violations=0` |
| 依存グラフの wgpu 系 crate | `make deny` の中の `dep-graph-wgpu-check` | `deny` | 終了コード 0。`record:` は `crates_total=13`・`narrow_total=0`・`wide_total=0` |
| core の試験を含む wasm32 の型検査 | `make wasm-verify` | `deny`・`wasm-macos` | 終了コード 0 |
| core の wasm32 の型検査 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown` | CI では `make wasm-verify` の `--tests` の check と、`verify`・`verify-macos` の `make build-profiles` の wasm32 の組が core をコンパイルする | 終了コード 0 |

ジョブは `ci.yml` のジョブ id で書く（`deny` は make deny (cargo-deny)、`wasm-macos` は wasm32 build + size + node (macos-latest)、`verify` は make verify + build-profiles、`verify-macos` は macOS 版）。
件数は記録時点（origin/main の 1df20ff）の値で、後の段で型や公開アイテムが増えても本節は書き換えない。

### 確かめ方

```text
grep -n 'run: make' .github/workflows/ci.yml
grep -n 'cargo check' scripts/wasm-verify.sh
make public-api-check
make public-api-sdk-check
make public-api-webgpu-check
make public-name-check
make core-deps-check
make deny
make wasm-verify
cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown
```

## 試験（4/6）

`crates/core` の `src/` は変えず、試験のファイルを 2 つ足した。公開 API は変わらない。

| 確かめること | 内容 |
| ------------ | ---- |
| 既存の 4 値 | `Rgba8Unorm`・`Bgra8Unorm`・`Rgba32Float`・`Depth32Float` が同じ名前で `TextureFormat` にあり、`Debug` の綴りが名前と合い、`all()` に含まれる |
| 性質の関数の値 | 性質の関数の値が、比較基準の `wgpu-types` 30.0.1 の同じ名前の関数の値と合う。対象は上の 4 値と、色・深度ステンシル・複数平面・BC・ETC2/EAC・ASTC の代表値。期待値は試験の中に定数で持ち、実行時に wgpu は使わない |

形式ごとの値の表は本書に置かず、試験のファイルを正とする。`PixelFormat`・`DepthFormat` は本リポに無く、この段の試験はそれらとそこからの変換を含まない。

### 対応するテスト

| ファイル | テスト | 内容 |
| -------- | ------ | ---- |
| `crates/core/tests/texture_format_existing_values.rs` | `four_values_debug_spelling_matches_name` | 4 値の `Debug` の綴りが名前と合う |
| 同上 | `four_values_are_listed_in_all` | 4 値が `TextureFormat::all()` に含まれる |
| `crates/core/tests/texture_format_properties.rs` | `properties_match_wgpu_types_30_0_1` | 各代表値で性質の関数の値が期待の定数と合う |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --test texture_format_existing_values
cargo test -p fandhe-silicon-core --test texture_format_properties
cargo test -p fandhe-silicon-core --all-features
```

## 試験（5/6）

`crates/core/src/texture_format.rs` には `#[cfg(test)] mod tests` だけを足し、公開 API は変わらない。試験のファイルを 2 つ足した。

| 確かめること | 内容 |
| ------------ | ---- |
| 名前の対応 | 77 個の variant 名と `TextureFormat` の値が 1 対 1 で対応する（ASTC 以外の 76 は具体形式 1 件ずつ、`Astc` は 42 件）。名前の綴りと順は比較基準の `wgpu-types` 30.0.1 と合う |
| 重複なし | `TextureFormat::all()` の 118 件に重複が無い |
| ASTC の組 | ASTC の具体形式がちょうど 42 件で、`AstcBlock::all()`（14）と `AstcChannel::all()`（3）の組ごとにちょうど 1 件。block・channel の綴りが期待と合う |
| 足し忘れの検出 | 同じ crate の中の `_` の腕の無い網羅 `match` で、値の一覧への足し忘れが検出される |
| `#[non_exhaustive]` | 3 つの型が `#[non_exhaustive]` で、別の crate は `_` の腕付きの `match` で書ける |
| `Astc` の構築と束縛 | `Astc` の variant 自体には `#[non_exhaustive]` が付かず、別の crate が値を作り、フィールドを `..` なしで束縛して `match` できる |

名前の一覧の表は本書に置かず、試験のファイルを正とする。`PixelFormat`・`DepthFormat` は本リポに無く、この段の試験はそれらを含まない。

### 対応するテスト

| ファイル | テスト | 内容 |
| -------- | ------ | ---- |
| `crates/core/src/texture_format.rs`（`mod tests`） | `all_formats_cover_every_variant` | 各 variant の番号が期待の件数だけ `all()` に現れる |
| 同上 | `all_formats_follow_declaration_order` | `all()` が宣言順で、ASTC が block（外）× channel（内）の順 |
| 同上 | `astc_blocks_cover_every_variant`・`astc_channels_cover_every_variant` | block・channel の全 variant が一覧に過不足なく並ぶ |
| `crates/core/tests/texture_format_names.rs` | `every_name_maps_to_values` | 各名前に対応する値が 1 件（`Astc` は 42 件） |
| 同上 | `every_value_maps_to_a_name` | 全値が名前に対応し、初出の順が名前の並びと合う |
| 同上 | `all_has_no_duplicates` | `all()` に重複が無く 118 件 |
| 同上 | `astc_concrete_formats_are_block_by_channel` | ASTC の 42 件が組ごとに 1 件で `Debug` の形が合う |
| 同上 | `astc_block_names_match`・`astc_channel_names_match` | block・channel の `Debug` が期待の綴りと順に合う |
| `crates/core/tests/texture_format_non_exhaustive.rs` | `texture_format_stays_non_exhaustive`・`astc_block_stays_non_exhaustive`・`astc_channel_stays_non_exhaustive` | `unreachable_patterns` を deny した別の crate で全 variant と `_` の腕を並べる |
| 同上 | `astc_variant_is_constructible_and_matchable` | `Astc` の構築と `..` なしの束縛 |

件数だけの確かめは `texture_format_counts.rs`（上の「対応するテスト」）が持つ。

### 確かめ方

```text
cargo test -p fandhe-silicon-core --lib texture_format
cargo test -p fandhe-silicon-core --test texture_format_names
cargo test -p fandhe-silicon-core --test texture_format_non_exhaustive
cargo test -p fandhe-silicon-core --all-features
```

## 完了の条件の確かめ（6/6）

6/6（#6712）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-1312）。GPU の実機なしで実行し、件数は実測。本体のコードと試験は変えていない。

### 成果物と置き場の対応

| 成果物 | 置き場 |
| ------ | ------ |
| 形式の型（`TextureFormat`・`AstcBlock`・`AstcChannel`）と性質の関数 | `crates/core/src/texture_format.rs`（上の「現状の形（2/6）」） |
| 型の試験 | `crates/core/tests/texture_format_counts.rs`・`texture_format_existing_values.rs`・`texture_format_properties.rs`・`texture_format_names.rs`・`texture_format_non_exhaustive.rs` と `texture_format.rs` の `mod tests`（上の「試験（4/6）」「試験（5/6）」） |

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 既存の 4 値の綴りと一覧、性質の照合 | `cargo test -p fandhe-silicon-core --test texture_format_existing_values`（`four_values_debug_spelling_matches_name`・`four_values_are_listed_in_all`）、`--test texture_format_properties`（`properties_match_wgpu_types_30_0_1`） | 2 件・1 件とも passed |
| (2) | 77 の名前と値の対応・118 件・ASTC の 42 件 | `--test texture_format_names`（`every_name_maps_to_values`・`every_value_maps_to_a_name`・`all_has_no_duplicates`・`astc_concrete_formats_are_block_by_channel`・`astc_block_names_match`・`astc_channel_names_match`）、`--test texture_format_counts`（`texture_format_all_has_118_concrete_formats`・`texture_format_all_has_42_astc_formats`・`astc_block_all_has_14_values`・`astc_channel_all_has_3_values`）、`--lib texture_format`（`all_formats_cover_every_variant`・`all_formats_follow_declaration_order`・`astc_blocks_cover_every_variant`・`astc_channels_cover_every_variant`） | 6 件・4 件・4 件とも passed |
| (3) | 利用側の `match` が壊れない扱い | `--test texture_format_non_exhaustive`（`texture_format_stays_non_exhaustive`・`astc_block_stays_non_exhaustive`・`astc_channel_stays_non_exhaustive`・`astc_variant_is_constructible_and_matchable`） | 4 件 passed |
| 全 feature | 上の組を `--all-features` 付きで流す | 同じ件数で passed |
| feature なし | `grep -c texture crates/core/Cargo.toml` | 0 件（試験に `[[test]]` の宣言が無く、feature なしで走る） |
| wasm32 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --test <5 本>` | 終了コード 0 |
| 公開 API・依存の検査 | `make public-api-check`・`make public-api-sdk-check`・`make public-api-webgpu-check`・`make public-name-check`・`make core-deps-check` | すべて終了コード 0。core の行は `wgpu_type_refs=0 wgpu_other_refs=0`・`sdk_type_refs=0 sdk_other_refs=0`・`webgpu_names=0`・`unexpected=0 missing=0` |
| 外部依存と unsafe | `cargo tree -p fandhe-silicon-core -e normal --depth 1`、`grep -n forbid crates/core/src/lib.rs` | 依存 0 件、`#![forbid(unsafe_code)]` |
| `PixelFormat`・`DepthFormat` | `grep -rn -e PixelFormat -e DepthFormat crates wasm measure baseline-wgpu`（`target/` を除く） | 0 件。両方とも本リポに存在しない。private spec 側の記録に従う（ポインタのみ） |
| 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |
| base の CI | `gh run list --branch main --commit <base> --workflow ci.yml` | 記録時点で run 41202408462 は in_progress（結論は未確定） |

### 再実行の契機

- `crates/core/src/texture_format.rs` を変えたとき
- 上の 5 本の試験か `mod tests` を変えたとき
- `TextureFormat`・`AstcBlock`・`AstcChannel` に値が増えたとき
- `scripts/public-api-check.sh` などの検査を変えたとき
- `docs/spec` の参照先を進めたとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1312）。確かめた base は `627d9fa36189ef0b34db37d3845bf1cfbaf98d10`。

## 段の表

| Issue | 内容 |
| ----- | ---- |
| #6707 | 現状と前提の確認（本書） |
| #6708 | 実装 |
| #6709 | CI への組み込み |
| #6710 | テスト |
| #6711 | テスト |
| #6712 | 確かめ |
