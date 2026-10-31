# SPIR-V のバイト列から語の列を作る操作（TASK-1682）の所在と本リポの現状

TASK-1682 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1682・`04-behavior/` の ART-138・ART-139・ART-140。ポインタのみ）。
内容・理由・経緯は本書に置かない。

前提のタスクと、本リポで対応する物。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-1599 | private spec 側の記録に従う（ポインタのみ） | なし |
| TASK-470 | `crates/core/src/execution.rs`（`PipelineInput` など） | [artifact-input-types](artifact-input-types.md) |
| TASK-22 | `wasm/`・`scripts/wasm-verify.sh` | [wasm32-node-foundation](wasm32-node-foundation.md) |
| TASK-23 | `crates/core/src/test_frame*`（feature `test-support` の裏） | [test-frame](test-frame.md) |

## 本リポの現状

確かめ方は、リポジトリのルートで流したコマンドである。

### core の構成

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 条件なしで公開する module | `artifact_cache`・`binding`・`capability`・`decision`・`device`・`device_lost`・`error`・`execution`・`hash`・`memory`・`spirv`・`texture_format`・`validation` の 13 個 | `grep -n "pub mod" crates/core/src/lib.rs` |
| feature の裏の module | `ext-interop` の裏に `interop`。`test-support` の裏に `capability_rule`・`parity`・`test_frame`・`rerun`。`fault-injection` の裏に `fault` | 同上 |
| feature | `test-support`・`fault-injection`・`native-blocking-wait`・`ext-interop` の 4 つ。どれも `= []` で、`default` は無い | `crates/core/Cargo.toml` の `[features]` |
| 外部依存 | 0 件（`cargo tree` に子が出ない） | `cargo tree -p fandhe-silicon-core -e normal --depth 1` |
| unsafe | `#![forbid(unsafe_code)]` | `grep -n forbid crates/core/src/lib.rs` |

### 名前の有無と既存の型

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `spirv_words_const`・`make_spirv` | `crates/`・`wasm/` に 0 件（終了コード 1）。`SpirvModule`・`spirv_words_from_bytes` は 2/5 で `spirv.rs` に入った（下の「現状の形（2/5）」） | `grep -rn -e spirv_words_const -e make_spirv crates wasm` |
| `#[macro_export]` | 1 件。`crates/core/src/test_frame.rs` の `declaration!`（module ごと feature `test-support` の裏）。feature の条件なしのものは 0 件 | `grep -rn macro_export crates wasm` |
| `PipelineInput` | `crates/core/src/execution.rs` の `pub enum PipelineInput<'a>`。`SpirV { words, entry_point }` の variant がある | `grep -n -e "pub enum PipelineInput" -e "SpirV {" crates/core/src/execution.rs` |
| `SpirV` の既存の試験 | `crates/core/tests/execution_pipeline_input.rs`・`execution_type_coverage.rs` | `grep -ln SpirV crates/core/tests/*` |
| `ContractError::ArtifactRejected` | `crates/core/src/error.rs` の `ArtifactRejected { kind, reason }`。`Display` は `artifact rejected ({kind}): {reason}`。`FailureClass::Initialization` に入る。`ContractError` は `PartialEq` を derive している | `grep -n -e ArtifactRejected -e "artifact rejected" -e derive crates/core/src/error.rs` |
| contract の SPIR-V の語の列を読む道具 | `crates/contract/src/backend_vulkan/spirv_operands.rs` に非公開の定数 `MAGIC`・`MAGIC_SWAPPED`（文書は [vulkan-operand-extraction](vulkan-operand-extraction.md)） | `grep -n MAGIC crates/contract/src/backend_vulkan/spirv_operands.rs` |
| core の `compile_fail` の doctest の先例 | `crates/core/src/binding.rs` に `compile_fail,E0xxx` が複数ある | `grep -n compile_fail crates/core/src/binding.rs` |

### 検査・lint・wasm32 の経路

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 公開 API の検査 | `scripts/public-api-check.sh` は rustdoc に `--document-hidden-items` を渡し、`#[doc(hidden)]` の公開アイテムも走査する | `grep -n document-hidden scripts/public-api-check.sh` |
| 公開名の語の一覧 | `public-name-check.conf` の `[names]` は tensor・broadcast・tiebreak・scene・renderer・tenant の 6 語。`public-api-webgpu-check.conf` は blocking・wait・webgpu | `cat public-name-check.conf public-api-webgpu-check.conf` |
| lint | `[workspace.lints.rust]` に `missing_docs = "warn"`。`make verify` は clippy を `-D warnings` で流す | `grep -n -A4 "workspace.lints.rust" Cargo.toml`・`grep -n clippy scripts/verify.sh` |
| doctest の実行 | `make verify` の `cargo test --workspace --all-features` で流れる | `grep -n "cargo test" scripts/verify.sh` |
| wasm32 | `scripts/wasm-verify.sh` が `cargo check -p fandhe-silicon-core --tests --locked --target wasm32-unknown-unknown` を既定と `--all-features` で流す。doctest は wasm32 では流れない | `grep -n "fandhe-silicon-core" scripts/wasm-verify.sh` |
| ツールチェーン | `rust-toolchain.toml` は `channel = "stable"` で、target に `wasm32-unknown-unknown` がある。`rust-version` の宣言は無い（終了コード 1） | `cat rust-toolchain.toml`・`grep -rn rust-version Cargo.toml crates/*/Cargo.toml` |

## 現状の形（2/5）

置き場は `crates/core/src/spirv.rs`（feature の条件なし・外部依存 0・`forbid(unsafe_code)` のまま）。

### 型

| 項目 | 事実 |
| ---- | ---- |
| 型 | `SpirvModule`（フィールドは非公開の `Vec<u32>`） |
| derive | `Clone, PartialEq, Eq`。`Debug` は手書きで、語の数だけを出す |

### 操作

| 操作 | 返すもの |
| ---- | -------- |
| `spirv_words_from_bytes(&[u8])` | `Result<Vec<u32>, ContractError>`。4 バイトずつ語にする。先頭の語が逆の語順の magic number のときは全語のバイトを入れ替える |
| `SpirvModule::from_bytes(&[u8])` | 上と同じ検査を通した `Result<SpirvModule, ContractError>` |
| `SpirvModule::words` | `&[u32]` |
| `SpirvModule::into_words` | `Vec<u32>` |
| `SpirvModule::pipeline_input(&self, entry_point)` | `PipelineInput::SpirV { words, entry_point }`（どちらも借用） |

入力の扱い。拒むときは `ContractError::ArtifactRejected { kind: "spir-v", reason }` で、`reason` は `<句> (<入力のバイト数> bytes)` の形。入力の中身は載せない。照らす順は 空、先頭の語、長さ。

| 入力 | 句 |
| ---- | -- |
| 空 | `input is empty` |
| 先頭の 4 バイトが（どちらの語順でも）magic number でない。1〜3 バイトもここ | `first word is not the SPIR-V magic number` |
| 長さが 4 の倍数でない | `byte length is not a multiple of 4` |

### 対応するテスト

`crates/core/src/spirv.rs` の `mod tests`。

| テスト | 内容 |
| ------ | ---- |
| `host_order_magic_is_read_as_is` | ホストの語順の magic だけの入力が 1 語になる |
| `swapped_magic_is_swapped_back` | 逆の語順の magic だけの入力が同じ 1 語になる |
| `empty_input_is_rejected_with_length` | 空の入力が `input is empty (0 bytes)` で拒まれる |
| `pipeline_input_borrows_words_and_entry_point` | `pipeline_input` が `PipelineInput::SpirV` を返す |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --lib spirv
cargo test -p fandhe-silicon-core --all-features
```
