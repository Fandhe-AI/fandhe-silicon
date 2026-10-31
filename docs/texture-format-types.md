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

## 段の表

| Issue | 内容 |
| ----- | ---- |
| #6707 | 現状と前提の確認（本書） |
| #6708 | 実装 |
| #6709 | CI への組み込み |
| #6710 | テスト |
| #6711 | テスト |
| #6712 | 確かめ |
