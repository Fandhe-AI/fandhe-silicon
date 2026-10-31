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

## 段の表

| Issue | 内容 |
| ----- | ---- |
| #7538 | 現状と前提の確認（本書） |
| #7539 | 実装 |
| #7540 | 実装 |
| #7541 | テスト |
| #7542 | テスト |
| #7543 | 確かめ |
