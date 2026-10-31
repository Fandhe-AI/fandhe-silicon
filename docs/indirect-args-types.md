# 間接実行の引数の型（TASK-1878）の所在と本リポの現状

TASK-1878 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1878・`04-behavior/` の CMD-133〜CMD-136。ポインタのみ）。
内容・理由・経緯は本書に置かない。

前提のタスクと、本リポで対応する物。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-309 | `crates/core/src/memory/buffer_check.rs`（`validate_buffer_range`） | [buffer-range-check](buffer-range-check.md) |
| TASK-23 | `crates/core/src/test_frame*`（feature `test-support` の裏） | [test-frame](test-frame.md) |
| TASK-1864 | private spec 側の記録に従う（ポインタのみ） | なし |

用途の検査（`BufferUsage::INDIRECT`・`UsageRole::IndirectArguments`）の文書は [buffer-usage-check](buffer-usage-check.md)（TASK-1796）。

## 本リポの現状

確かめ方は、リポジトリのルートで流したコマンドである。

### core の構成

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 条件なしで公開する module | `artifact_cache`・`binding`・`capability`・`decision`・`device`・`device_lost`・`error`・`execution`・`hash`・`memory`・`texture_format`・`validation` の 12 個 | `grep -n "pub mod" crates/core/src/lib.rs` |
| feature の裏の module | `ext-interop` の裏に `interop`。`test-support` の裏に `capability_rule`・`parity`・`test_frame`・`rerun`。`fault-injection` の裏に `fault` | 同上 |
| feature | `test-support`・`fault-injection`・`native-blocking-wait`・`ext-interop` の 4 つ。どれも `= []` で、`default` は無い | `crates/core/Cargo.toml` の `[features]` |
| 外部依存 | 0 件（`[dependencies]` が無く、`cargo tree` に子が出ない） | `cargo tree -p fandhe-silicon-core -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/core/src/lib.rs` |

### `execution` module の構成

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 子 module と再公開 | `compute`・`dispatch_check`・`submit` の 3 つの子 module を持ち、`pub use` で再公開している | `sed -n 8,20p crates/core/src/execution.rs` |
| `ComputeOps` | 操作は `create_pipeline` だけを持つ | `grep -n "fn " crates/core/src/execution/compute.rs` |

### 型の有無

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 間接実行の引数に当たる名前 | `crates/` に 3 つの型の名前の定義も参照も 0 件（終了コード 1） | `git grep -n -e DrawIndirectArgs -e DrawIndexedIndirectArgs -e DispatchIndirectArgs -- crates` |
| `first_instance` | `crates/` に 0 件（終了コード 1） | `git grep -n -F first_instance -- crates` |
| `repr(C)` | `crates/` に 0 件（終了コード 1） | `git grep -n -F 'repr(C)' -- crates` |
| `INDIRECT_FIRST_INSTANCE` | `crates/` に 0 件（終了コード 1） | `git grep -n INDIRECT_FIRST_INSTANCE -- crates` |
| `indirect` を含む語 | 当たるのは用途の値と対応表（`memory/buffer.rs`・`memory/usage_check.rs`・`memory/usage_mapping.rs`）とその試験だけ | `git grep -n -i indirect -- crates` |

3 つの型の名前は、比較基準の wgpu-types 30.0.1（`baseline-wgpu/Cargo.lock` が固定）の公開の型の名前である。
ソースは `~/.cargo/registry/src/*/wgpu-types-30.0.1/src/render.rs`（取得は `cargo fetch --manifest-path baseline-wgpu/Cargo.toml`）。

### 既にある部品

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 範囲の検査 | `validate_buffer_range(offset, len, allocation_len)`（`crates/core/src/memory/buffer_check.rs`）。拒否は `ContractError::InvalidArgument` で、文は 2 種（`buffer range end overflows u64 (offset {offset}, len {len})`・`buffer range end ({end}) exceeds allocation length ({allocation_len})`）。終端がちょうど確保長なら通る。試験は `crates/core/tests/memory_buffer_check.rs` | `grep -n "pub fn validate_buffer_range" crates/core/src/memory/buffer_check.rs` |
| 用途 | `BufferUsage::INDIRECT`（`1 << 9`）・`UsageRole::IndirectArguments`・`validate_usage_for_role`・`BufferUsageKind::Indirect` | `git grep -n -i indirect -- crates/core/src` |
| エラー | `ContractError`（`#[non_exhaustive]`）の `InvalidArgument(String)`・`ValidationFailed(String)` | `grep -n "InvalidArgument(\|ValidationFailed(\|non_exhaustive" crates/core/src/error.rs` |
| 機能の ID | `FeatureId(pub &'static str)`。core にある機能の ID の定数は `WEBGPU_DEVICE_PORT`・`WEBGPU_HANDLE_PORT` の 2 つだけ | `grep -n "FeatureId(" crates/core/src/capability.rs` |
| 整列の拒否の文の先例 | `crates/core/src/execution/dispatch_check.rs` の `C5: slot {slot}: offset ({offset}) is not a multiple of alignment ({alignment})` | `grep -n "C5:" crates/core/src/execution/dispatch_check.rs` |
| `#[non_exhaustive]` の先例の試験 | `crates/core/tests/` に `*_non_exhaustive*.rs` が 5 本ある | `ls crates/core/tests \| grep non_exhaustive` |

### 公開 API・依存の検査が core を既に覆っていること

どの検査も CI の `deny` ジョブ（`.github/workflows/ci.yml`）が呼ぶ。

| 検査 | 対象 | 入口 |
| ---- | ---- | ---- |
| 公開 API の wgpu 系の参照 | 製品の 4 crate（core を含む）の公開アイテム | `make public-api-check` |
| 公開 API の名前 | 公開アイテム名（enum の variant を含む）を `public-name-check.conf` の `[names]` に照らす | `make public-name-check` |
| 公開 API の WebGPU 系の名前 | `fandhe-silicon-core`・`fandhe-silicon-contract`。`public-api-webgpu-check.conf` の `[blocking-words]`・`[webgpu-words]` | `make public-api-webgpu-check` |
| チップの SDK の crate | 製品の 4 crate の公開 API と `crates/core` の依存宣言 | `make public-api-sdk-check` |
| core の外部依存の一覧 | `crates/core` の宣言する外部の crate と、contract からの feature の転送 | `make core-deps-check` |
| 依存グラフの wgpu 系 crate | 製品の workspace 全体 | `make dep-graph-wgpu-check`（`make deny` の中でも流れる） |
| wasm32 の型検査 | `crates/core` の試験を含む | `make wasm-verify` |

語の一覧との照合: 比較基準の 3 つの型の名前を小文字の英数字に正規化しても、`[names]` の 6 語・`[webgpu-words]`・`[blocking-words]` の語は部分一致で 0 件。
手順は `python3 -I -c` で、正規化した名前に語が含まれるかを数える。
`[words]` は `--source` に渡したファイルの行に当てる別の照合で、公開アイテム名の照合には使われない。

## 現状の形（2/6）

置き場は `crates/core/src/execution/indirect_args.rs`（feature の条件なし・外部依存 0・`forbid(unsafe_code)` のまま。`execution` から再公開される）。

### 型

| 型 | 欄（名前: 型。宣言順） | `SIZE` |
| -- | ---------------------- | ------ |
| `DrawIndirectArgs` | `vertex_count: u32`・`instance_count: u32`・`first_vertex: u32`・`first_instance: u32` | 16 |
| `DrawIndexedIndirectArgs` | `index_count: u32`・`instance_count: u32`・`first_index: u32`・`base_vertex: i32`・`first_instance: u32` | 20 |
| `DispatchIndirectArgs` | `x: u32`・`y: u32`・`z: u32` | 12 |

| 項目 | 事実 |
| ---- | ---- |
| derive | 3 つとも `Clone, Copy, Debug, Default, PartialEq, Eq` |
| `#[repr(C)]` | 3 つとも付く |
| `#[non_exhaustive]` | 付けない。欄はすべて公開 |
| 名前と順 | 型の名前・欄の名前と型と順は、比較基準の `wgpu-types` 30.0.1 の同名の型と同じ |

### バイト列との変換

| 関数 | 動き |
| ---- | ---- |
| `to_bytes(&self)` | 欄の宣言順に各欄をリトルエンディアンで並べた配列（16・20・12 バイト）を返す |
| `from_bytes(&[u8; N])` | `to_bytes` の逆で、同じ並びから値を組み立てる |

変換は欄ごとの `to_le_bytes`・`from_le_bytes` で書いており、unsafe を使わない。

### 対応するテスト

`crates/core/tests/indirect_args_size.rs`（feature 不要）。

| テスト | 内容 |
| ------ | ---- |
| `draw_indirect_args_size_matches_layout` | `size_of` と `SIZE` が一致し、16 |
| `draw_indexed_indirect_args_size_matches_layout` | 同じく 20 |
| `dispatch_indirect_args_size_matches_layout` | 同じく 12 |
| `bytes_round_trip` | 3 型で `from_bytes(&x.to_bytes()) == x`（`base_vertex` は負の値を含む） |

### 確かめ方

- `cargo test -p fandhe-silicon-core --test indirect_args_size`
- `cargo test -p fandhe-silicon-core --all-features`

## 現状の形（3/6）

置き場は同じ `crates/core/src/execution/indirect_args.rs` で、`execution` から再公開される。

### 範囲・整列の検査

| 項目 | 内容 |
| ---- | ---- |
| 関数 | `validate_indirect_args_range(offset: u64, args_size: u64, buffer_len: u64) -> Result<(), ContractError>` |
| 判定の順 | 開始位置が 4 の倍数か、終端のあふれ、終端が長さを超えるか（後の 2 つは `validate_buffer_range` に渡す） |
| 拒否 | どれも `ContractError::InvalidArgument` |
| 文 | 整列は `indirect args offset (…) is not a multiple of 4`。あふれ・超過は [buffer-range-check](buffer-range-check.md) の既存の 2 種のまま |
| 入力 | `args_size` は任意の `u64`。`BufferHandle` は受けず、長さは呼び手が渡す |

### first_instance の判定

| 項目 | 内容 |
| ---- | ---- |
| enum | `FirstInstanceDecision { Apply, Reject }`。derive は `Debug, Clone, Copy, PartialEq, Eq, Hash`。`#[non_exhaustive]` は付けない |
| 関数 | `first_instance_decision(feature_enabled: bool, first_instance: u32) -> FirstInstanceDecision`（`Result` ではない） |
| 対応 | `first_instance` が 0 なら `Apply`、そうでなく `feature_enabled` なら `Apply`、それ以外は `Reject` |

### 対応するテスト

`crates/core/tests/indirect_args_rules.rs`（feature 不要）。

| テスト | 内容 |
| ------ | ---- |
| `range_rejects_misaligned_offset` | 開始位置 1 が整列の文で拒否される |
| `range_rejects_end_overflow` | 終端があふれる組があふれの文で拒否される |
| `range_rejects_end_beyond_length` | 終端が長さを超える組が超過の文で拒否される |
| `range_accepts_end_equal_to_length` | 終端がちょうど長さなら通る |
| `first_instance_decision_cases` | `(false, 0)` と `(true, 7)` は `Apply`、`(false, 1)` は `Reject` |

### 確かめ方

- `cargo test -p fandhe-silicon-core --test indirect_args_rules`
- `cargo test -p fandhe-silicon-core --all-features`

## 試験（4/6）

`crates/core/src/` は変えず、試験のファイルを 1 つ足した。公開 API は変わらない。

### 確かめること

| 項目 | 内容 |
| ---- | ---- |
| 大きさと整列 | 3 型で `size_of` が `SIZE`（16・20・12）と一致し、整列が 4 |
| 欄の順とリトルエンディアン | 欄ごとに別のバイトの模様の値を入れ、`to_bytes` が試験の中に手で書いた配列と一致する |
| `base_vertex` の符号 | `-1`・`i32::MIN`・`i32::MAX` が 2 の補数のリトルエンディアンで 12〜15 バイト目に入り、ほかの欄の位置は動かない |
| 境界値の往復 | `u32` の欄は `{0, 1, u32::MAX}`、`base_vertex` は `{i32::MIN, -1, 0, i32::MAX}` の直積（Draw 81 件・DrawIndexed 324 件・Dispatch 27 件）で `from_bytes(&x.to_bytes()) == x` |
| 既定値 | 3 型の `default()` の全欄が 0 で、`to_bytes` も全部 0 |

### 対応するテスト

`crates/core/tests/indirect_args_layout.rs`（feature 不要）。

| テスト | 内容 |
| ------ | ---- |
| `sizes_match_size_constants` | 3 型の `size_of` と `SIZE` の一致と値 |
| `alignment_is_four` | 3 型の整列が 4 |
| `draw_bytes_follow_field_order` | Draw の欄の並び |
| `draw_indexed_bytes_follow_field_order` | DrawIndexed の欄の並び |
| `dispatch_bytes_follow_field_order` | Dispatch の欄の並び |
| `base_vertex_is_twos_complement_le` | `base_vertex` の符号の境界 |
| `round_trip_over_boundary_values` | 境界値の直積の往復 |
| `bytes_round_trip_from_expected_arrays` | 手で書いた配列からの組み立てと書き戻し |
| `default_is_all_zero` | 既定値 |

### 確かめ方

- `cargo test -p fandhe-silicon-core --test indirect_args_layout`
- `cargo test -p fandhe-silicon-core --all-features`
- `cargo check -p fandhe-silicon-core --tests --target wasm32-unknown-unknown`

### 公開 API・依存の検査

型を足したあとの本リポで実際に流した結果。設定・Makefile・CI は変えていない。

| 検査 | 入口 | CI のジョブ | 結果（記録時点） |
| ---- | ---- | ----------- | ---------------- |
| 公開 API の wgpu 系の参照 | `make public-api-check` | `deny` | 終了コード 0。`record:` は `documents=10 public_items_total=10100 wgpu_type_refs_total=0 wgpu_other_refs_total=0` |
| 公開 API のチップの SDK の crate | `make public-api-sdk-check` | `deny` | 終了コード 0。`public_items_total=8625 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0` |
| 公開 API の WebGPU 系の名前 | `make public-api-webgpu-check` | `deny` | 終了コード 0。`webgpu_names_total=0 blocking_wait_names_total=0 interop_violations_total=0` |
| 公開 API の名前 | `make public-name-check` | `deny` | 終了コード 0。`name_hits_total=0 source_hits_total=0 field_hits_total=0` |
| core の外部依存と feature の転送 | `make core-deps-check` | `deny` | 終了コード 0。`unexpected=0 missing=0 forward_violations=0` |
| 依存の監査と依存グラフの wgpu 系 crate | `make deny` | `deny` | 終了コード 0。依存グラフの `record:` は `crates_total=13 workspace_members=5 narrow_total=0 wide_total=0` |
| core の試験を含む wasm32 の型検査 | `make wasm-verify` | `deny`・`wasm-macos` | 終了コード 0 |

件数は記録時点の値である。流した base の commit は `35a851be02f62e3be0c2deddd20c2557ee886fee`。

## 試験（5/6）

`crates/core/src/` は変えず、試験のファイルを 2 つ足した。公開 API は変わらない。

### 確かめること

| 項目 | 内容 |
| ---- | ---- |
| 範囲・整列の格子 | `args_size` 5 値 × `buffer_len` の境界 × `offset` の境界（0〜4、長さと終端の前後、`u64::MAX` 付近）の各点で、`u128` で求めた期待と結果が一致する |
| 判定の順 | 4 の倍数でなければ整列の拒否、次に終端が `u64` に収まらないあふれの拒否、次に長さを超える拒否、それ以外は `Ok(())` |
| 終端がちょうど長さ | 終端と長さが等しければ通る（`u64::MAX` 付近を含む） |
| 開始位置が長さを超える | `args_size` が 0 でも超過で拒否される |
| 拒否の種類 | 格子のすべての拒否が `InvalidArgument` |
| `first_instance` の判定の境界 | `feature_enabled` の 2 値 × `{0, 1, 2, 4, u32::MAX - 1, u32::MAX}` で参照と一致する |
| `first_instance` の判定の刻み | 刻み 65537 で `u32` の全域を回し、両方の `feature_enabled` で参照と一致する |
| 閉じた 2 値 | `FirstInstanceDecision` を `_` の腕なしの `match` で扱える |

### 対応するテスト

`crates/core/tests/indirect_args_range.rs`・`crates/core/tests/indirect_args_first_instance.rs`（feature 不要）。

| ファイル | テスト | 内容 |
| -------- | ------ | ---- |
| `indirect_args_range.rs` | `range_grid_matches_u128_reference` | 格子と参照の一致 |
| `indirect_args_range.rs` | `every_rejection_is_invalid_argument` | 拒否の種類 |
| `indirect_args_range.rs` | `misalignment_is_reported_before_overflow_and_excess` | 整列の拒否が先に出る |
| `indirect_args_range.rs` | `end_exactly_at_length_is_accepted` | 終端がちょうど長さ |
| `indirect_args_range.rs` | `start_beyond_length_is_rejected_even_for_empty_args` | 開始位置が長さを超える |
| `indirect_args_range.rs` | `end_overflow_boundary_near_u64_max` | あふれの境界 |
| `indirect_args_first_instance.rs` | `boundary_values_match_reference` | 判定の境界 |
| `indirect_args_first_instance.rs` | `named_claims` | `0` は常に適用、`0` 以外は有効のときだけ適用 |
| `indirect_args_first_instance.rs` | `stride_sweep_over_u32_matches_reference` | 刻みでの全域 |
| `indirect_args_first_instance.rs` | `decision_is_closed_two_values` | 閉じた 2 値 |

### 確かめ方

- `cargo test -p fandhe-silicon-core --test indirect_args_range`
- `cargo test -p fandhe-silicon-core --test indirect_args_first_instance`
- `cargo test -p fandhe-silicon-core --all-features`
- `cargo check -p fandhe-silicon-core --tests --target wasm32-unknown-unknown`

## 完了の条件の確かめ（6/6）

6/6（#7543）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-1878）。GPU の実機なしで実行し、件数は実測。本体のコードと試験は変えていない。

### 成果物と置き場の対応

| 成果物 | 置き場 |
| ------ | ------ |
| 引数の型・範囲と整列の検査・`first_instance` の判定 | `crates/core/src/execution/indirect_args.rs`（`execution` から再公開。上の「現状の形（2/6）」「現状の形（3/6）」） |
| 試験 | `crates/core/tests/indirect_args_size.rs`・`indirect_args_layout.rs`・`indirect_args_rules.rs`・`indirect_args_range.rs`・`indirect_args_first_instance.rs`（上の「試験（4/6）」「試験（5/6）」） |

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 配置（CMD-133 のポインタ） | `cargo test -p fandhe-silicon-core --test indirect_args_size`（`draw_indirect_args_size_matches_layout`・`draw_indexed_indirect_args_size_matches_layout`・`dispatch_indirect_args_size_matches_layout`・`bytes_round_trip`）、`--test indirect_args_layout`（`sizes_match_size_constants`・`alignment_is_four`・`draw_bytes_follow_field_order`・`draw_indexed_bytes_follow_field_order`・`dispatch_bytes_follow_field_order`・`base_vertex_is_twos_complement_le`・`round_trip_over_boundary_values`・`bytes_round_trip_from_expected_arrays`・`default_is_all_zero`） | 4 件・9 件とも passed |
| (2) | 範囲・整列の検査（CMD-134・CMD-135 のポインタ） | `--test indirect_args_rules range_`（`range_rejects_misaligned_offset`・`range_rejects_end_overflow`・`range_rejects_end_beyond_length`・`range_accepts_end_equal_to_length`）、`--test indirect_args_range`（`range_grid_matches_u128_reference`・`every_rejection_is_invalid_argument`・`misalignment_is_reported_before_overflow_and_excess`・`end_exactly_at_length_is_accepted`・`start_beyond_length_is_rejected_even_for_empty_args`・`end_overflow_boundary_near_u64_max`） | 4 件・6 件とも passed |
| (3) | `first_instance` の判定（CMD-136 のポインタ） | `--test indirect_args_rules first_instance`（`first_instance_decision_cases`）、`--test indirect_args_first_instance`（`boundary_values_match_reference`・`named_claims`・`stride_sweep_over_u32_matches_reference`・`decision_is_closed_two_values`） | 1 件・4 件とも passed |
| 全 feature | 上の組を `--all-features` 付きで流す | 同じ件数で passed |
| feature なし | `grep -c indirect crates/core/Cargo.toml` | 0 件（試験に `[[test]]` の宣言が無く、feature なしで走る。5 本とも `#[ignore]` は 0 件） |
| wasm32 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --test <5 本>` | 終了コード 0 |
| 公開 API・依存の検査 | `make public-api-check`・`make public-api-sdk-check`・`make public-api-webgpu-check`・`make public-name-check`・`make core-deps-check` | すべて終了コード 0。core の行は `wgpu_type_refs=0 wgpu_other_refs=0`・`sdk_type_refs=0 sdk_other_refs=0`・`webgpu_names=0`・`name_hits=0`・`unexpected=0 missing=0` |
| 外部依存と unsafe | `cargo tree -p fandhe-silicon-core -e normal --depth 1`、`grep -n forbid crates/core/src/lib.rs` | 依存 0 件、`#![forbid(unsafe_code)]` |
| 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |
| base の CI | `gh run list --branch main --commit <base> --workflow ci.yml` | run 41225202952 は success |

### 再実行の契機

- `crates/core/src/execution/indirect_args.rs` を変えたとき
- 上の 5 本の試験を変えたとき
- `validate_buffer_range` を変えたとき
- `scripts/public-api-check.sh` などの検査を変えたとき
- `docs/spec` の参照先を進めたとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1878）。確かめた base は `a97ac58f10a9ac80ccdfaec92d5c044c8d1ebf1a`。

## 段の表

| Issue | 内容 |
| ----- | ---- |
| #7538 | 現状と前提の確認（本書） |
| #7539 | 実装 |
| #7540 | 実装 |
| #7541 | テスト |
| #7542 | テスト |
| #7543 | 確かめ |
