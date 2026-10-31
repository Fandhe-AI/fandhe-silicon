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
| `spirv_words_const`・`make_spirv` | `spirv_words_const` は 3/5 で `spirv.rs` に入った（下の「現状の形（3/5）」）。`make_spirv` は 0 件。`SpirvModule`・`spirv_words_from_bytes` は 2/5 で入った | `grep -rn -e spirv_words_const -e make_spirv crates wasm` |
| `#[macro_export]` | 2 件。feature の条件なしは `spirv_words_const!`（`spirv.rs`）。`test_frame.rs` の `declaration!` は module ごと feature `test-support` の裏 | `grep -rn macro_export crates wasm` |
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

## 現状の形（3/5）

置き場は `crates/core/src/spirv.rs`（2/5 と同じ。外部依存 0・unsafe なし）。

### マクロと展開用の口

| 名前 | 引数 | 返すもの |
| ---- | ---- | -------- |
| `spirv_words_const!`（`#[macro_export]`・crate の根） | `const` 式の `&[u8]` | `[u32; 入力のバイト数 / 4]`。`const` と `static` の初期化子に置ける |
| `__spirv_words_const::<N>`（`#[doc(hidden)] pub const fn`・マクロの展開から呼ぶ） | `&[u8]` | `Result<[u32; N], &'static str>`。panic しない。`N * 4` が入力のバイト数と合わないときは長さの句 |

マクロの展開は内側の `const` 2 つ（`__FANDHE_SILICON_SPIRV_` で始まる名前）を持つブロック式で、利用者の定数名と衝突しない。

### コンパイル時の入力ごとの結果

句と照らす順は実行時の `spirv_words_from_bytes` と同じ（バイト数の付記だけが無い）。

| 入力 | 結果 |
| ---- | ---- |
| ホストの語順の magic で始まる 4 の倍数の長さ | 語の列 |
| 逆の語順の magic で始まる 4 の倍数の長さ | 全語を入れ替えた語の列 |
| 空 | コンパイルエラー（`input is empty`） |
| どちらの語順でも magic でない | コンパイルエラー（`first word is not the SPIR-V magic number`） |
| magic で始まり長さが 4 の倍数でない | コンパイルエラー（`byte length is not a multiple of 4`） |

### 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| `spirv_words_const` の doc の通る例（3 つ） | ホストの語順・逆の語順・`static` の初期化子で、`spirv_words_from_bytes` と同じ語の列になる |
| `spirv_words_const` の doc の `compile_fail` の例（3 つ） | 長さが 4 の倍数でない入力・magic でない入力・空の入力でコンパイルが失敗する |
| `const_macro_matches_runtime`（`mod tests`） | `const`・`static` で使い、実行時の結果と一致する |
| `const_entry_rejects_mismatched_length_at_runtime`（`mod tests`） | 展開用の口が `N` の不一致・空・magic 違いを句で返す |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --doc spirv_words_const
cargo test -p fandhe-silicon-core --lib spirv
make wasm-verify
```

## 試験（4/5）

`crates/core` の `src/` は変えず、試験のファイルを 3 つ足した。公開 API は変わらない。

| 確かめること | 内容 |
| ------------ | ---- |
| 受け付けの入力 | ホストの語順・逆の語順の magic で始まる 4 の倍数の長さの入力で、語の列の先頭が `0x0723_0203`、語の数がバイト数の 4 分の 1 になり、逆の語順の入力は全語を入れ替えた値（ホストの語順の入力と同じ列）になる。入力は `to_ne_bytes`・`swap_bytes` で作り、ホストの語順に依らない |
| 拒否の入力と句 | 空は空の句、1〜3 バイトと magic でない入力（長さが 4 の倍数かどうかに依らない）は magic の句、magic で始まり長さが 4 の倍数でない入力は長さの句。どれも `ArtifactRejected { kind: "spir-v", .. }` |
| `reason` の中身 | 句の後ろに入力のバイト数が付き、入力の中身（先頭の語の値）は含まない |
| 3 つの操作の一致 | `SpirvModule::from_bytes`・`spirv_words_from_bytes`・`spirv_words_const!` が同じ入力に同じ語の列を返す。展開用の口を実行時に呼んだ `Err` の句は、実行時の `reason` の句と一致する |
| `pipeline_input` | `PipelineInput::SpirV` を返し、`words` は `SpirvModule::words()` と同じ slice、`entry_point` は渡した文字列と同じ借用 |
| マクロ | `const`・`static` の初期化子に置け、利用者の定数名と衝突せず、同じ入力を 2 回渡すと同じ値になる |

### 対応するテスト

| ファイル | テスト | 内容 |
| -------- | ------ | ---- |
| `crates/core/tests/spirv_words_raw.rs` | `accepts_host_order_input`・`accepts_swapped_order_input_and_swaps_every_word`・`word_count_is_byte_len_over_four` | 受け付けの語の列と語の数 |
| 同上 | `rejects_empty_with_empty_phrase`・`rejects_one_to_three_bytes_with_magic_phrase`・`rejects_non_magic_first_word_regardless_of_length`・`rejects_magic_with_trailing_bytes_with_length_phrase` | 拒否の入力ごとの句とバイト数 |
| 同上 | `reason_does_not_leak_input_contents`・`three_phrases_are_distinct`・`same_input_gives_same_result` | 中身を載せないこと・句の区別・同じ入力に同じ結果 |
| `crates/core/tests/spirv_words_runtime.rs` | `from_bytes_accepts_and_exposes_words`・`swapped_input_equals_host_order_module`・`same_input_builds_equal_modules` | `SpirvModule` の受け付けと等価 |
| 同上 | `from_bytes_rejection_matches_raw_function` | 拒否が `spirv_words_from_bytes` と同じ |
| 同上 | `pipeline_input_borrows_words_and_entry_point` | `pipeline_input` の借用 |
| `crates/core/tests/spirv_words_const.rs` | `macro_in_const_and_static_matches_runtime`・`macro_does_not_clash_with_user_names`・`same_input_twice_gives_same_value` | `const`・`static` でのマクロ |
| 同上 | `const_entry_accepts_like_runtime`・`const_entry_phrases_match_runtime_reason` | 展開用の口と実行時の変換の一致 |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --test spirv_words_runtime
cargo test -p fandhe-silicon-core --test spirv_words_raw
cargo test -p fandhe-silicon-core --test spirv_words_const
cargo test -p fandhe-silicon-core --all-features
make wasm-verify
```

## 完了の条件の確かめ（5/5）

5/5（#7537）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-1682）。GPU の実機なしで実行し、件数は実測。本体のコードと試験は変えていない。

### 成果物と置き場の対応

| 成果物 | 置き場 |
| ------ | ------ |
| 3 つの操作（`SpirvModule::from_bytes` と `words`・`into_words`・`pipeline_input`、`spirv_words_from_bytes`、`spirv_words_const!` と展開用の口 `__spirv_words_const`） | `crates/core/src/spirv.rs`（上の「現状の形（2/5）」「現状の形（3/5）」） |
| ART-138（`SpirvModule` の操作）の試験 | `crates/core/tests/spirv_words_runtime.rs` |
| ART-139（`spirv_words_from_bytes`）の試験 | `crates/core/tests/spirv_words_raw.rs` |
| ART-140（`spirv_words_const!`）の試験 | `crates/core/tests/spirv_words_const.rs` と `spirv_words_const!` の doctest |
| 共通の単体試験 | `crates/core/src/spirv.rs` の `mod tests` |

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | ART-138 の受け付け・拒否・`pipeline_input`。拒否は `from_bytes_rejection_matches_raw_function` が、空・1〜3 バイト・どちらの語順でも magic でない入力・magic の後に 1〜3 バイト余る入力（両語順）で `spirv_words_from_bytes` と同じ結果になることを見る | `cargo test -p fandhe-silicon-core --test spirv_words_runtime`（`from_bytes_accepts_and_exposes_words`・`swapped_input_equals_host_order_module`・`same_input_builds_equal_modules`・`from_bytes_rejection_matches_raw_function`・`pipeline_input_borrows_words_and_entry_point`） | 5 件 passed |
| (2) | ART-139 の受け付け・拒否の句・バイト数・入力の中身を載せないこと。長さが 4 の倍数でない入力と先頭の語が magic でない入力は `rejects_non_magic_first_word_regardless_of_length`・`rejects_magic_with_trailing_bytes_with_length_phrase` が覆う | `--test spirv_words_raw`（`accepts_host_order_input`・`accepts_swapped_order_input_and_swaps_every_word`・`word_count_is_byte_len_over_four`・`rejects_empty_with_empty_phrase`・`rejects_one_to_three_bytes_with_magic_phrase`・`rejects_non_magic_first_word_regardless_of_length`・`rejects_magic_with_trailing_bytes_with_length_phrase`・`reason_does_not_leak_input_contents`・`three_phrases_are_distinct`・`same_input_gives_same_result`） | 10 件 passed |
| (3) | ART-140 の `const`・`static` での一致と、展開用の口を実行時に呼んだ句の一致 | `--test spirv_words_const`（`macro_in_const_and_static_matches_runtime`・`macro_does_not_clash_with_user_names`・`same_input_twice_gives_same_value`・`const_entry_accepts_like_runtime`・`const_entry_phrases_match_runtime_reason`） | 5 件 passed |
| (4) | ART-140 のコンパイル時の拒否。`compile_fail,E0080` の例が覆うのは、ホストの語順の magic に 1 バイト余る入力（長さ）・`[1, 2, 3, 4]`（どちらの語順でも magic でない）・空の 3 つ。逆の語順の magic で長さが 4 の倍数でない入力は `compile_fail` の例に無く、展開用の口を実行時に呼ぶ `const_entry_phrases_match_runtime_reason` が両語順で句を確かめる | `cargo test -p fandhe-silicon-core --doc spirv_words_const` | 6 件 passed（通る例 3・`compile_fail` 3） |
| (5) | 内部の単体試験 | `cargo test -p fandhe-silicon-core --lib spirv::`（`host_order_magic_is_read_as_is`・`swapped_magic_is_swapped_back`・`later_words_follow_the_first_word_byte_order`・`empty_input_is_rejected_with_length`・`one_to_three_bytes_are_rejected_as_magic`・`wrong_magic_is_rejected`・`magic_with_trailing_bytes_is_rejected_by_length`・`pipeline_input_borrows_words_and_entry_point`・`const_macro_matches_runtime`・`const_entry_rejects_mismatched_length_at_runtime`） | 10 件 passed |
| 全 feature | (1)〜(5) を `--all-features` 付きで流す | 同じ件数で passed |
| feature なし | `grep -c spirv crates/core/Cargo.toml` | 0 件（試験に `[[test]]` の宣言が無く、feature なしで走る） |
| wasm32 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --test spirv_words_raw --test spirv_words_runtime --test spirv_words_const`・`make wasm-verify` | どちらも終了コード 0。`spirv_words_const.rs` の `const` の評価は wasm32 の型検査でも走る。doctest は wasm32 では流れない |
| 公開 API・依存の検査 | `make public-api-check`・`make public-api-sdk-check`・`make public-api-webgpu-check`・`make public-name-check`・`make core-deps-check` | すべて終了コード 0。core の行は `wgpu_type_refs=0 wgpu_other_refs=0`・`sdk_type_refs=0 sdk_other_refs=0`・`webgpu_names=0`・`name_hits=0`・`unexpected=0 missing=0` |
| 外部依存と unsafe | `cargo tree -p fandhe-silicon-core -e normal --depth 1`、`grep -n forbid crates/core/src/lib.rs` | 依存 0 件、`#![forbid(unsafe_code)]` |
| 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |
| base の CI | `gh run list --branch main --commit <base> --workflow ci.yml` | run 41224029284 は success |

### 再実行の契機

- `crates/core/src/spirv.rs` を変えたとき
- 上の 3 本の試験・`mod tests`・マクロの doctest を変えたとき
- `PipelineInput::SpirV` や `ContractError::ArtifactRejected` の形が変わったとき
- `scripts/wasm-verify.sh` や公開 API の検査を変えたとき
- `docs/spec` の参照先を進めたとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1682）。確かめた base は `6c083ded3e01db89c22ed192222125678fd21322`。
