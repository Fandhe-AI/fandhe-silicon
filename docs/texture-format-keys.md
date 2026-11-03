# テクスチャの形式の問合せの鍵の一覧と行の表（TASK-1313）の所在と本リポの現状

TASK-1313 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1313。ポインタのみ）。
内容・理由・経緯は本書に置かない。

## 本リポの現状

確かめ方は、リポジトリのルートで流したコマンドである。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 形式の型と全件の列 | `TextureFormat`・`AstcBlock`・`AstcChannel` と各 `all()` は `crates/core/src/texture_format.rs`（TASK-1312） | `grep -n "pub fn all" crates/core/src/texture_format.rs` |
| 鍵の型 | `FeatureId` は `crates/core/src/capability.rs` | `grep -n "pub struct FeatureId" crates/core/src/capability.rs` |
| `texformat:` の文字列 | この作業の前の main（`ea7567c`）の `crates/`・`docs/`・`scripts/` に 0 件 | `git grep -n texformat ea7567c -- crates docs scripts` |

### 公開 API・依存の検査が core を既に覆っていること

| 検査 | 入口 |
| ---- | ---- |
| 公開 API の wgpu 系の参照（TASK-10） | `make public-api-check` |
| チップの SDK の crate（TASK-13） | `make public-api-sdk-check` |
| 公開 API の名前 | `make public-name-check` |
| 公開 API の WebGPU 系の名前 | `make public-api-webgpu-check` |
| 共通の crate の外部依存 | `make core-deps-check` |

## 現状の形（1/4）

### module

- 所在は `crates/core/src/texture_format_key.rs`。`crates/core/src/lib.rs` に `pub mod texture_format_key;` があり、feature の条件はない。crate の根の `pub use` は無い。

### 鍵の定数

- `FeatureId` の定数が 118 個。名前は `TEXFORMAT_` に続けて形式の名前を大文字の snake case にしたもの（ASTC は `TEXFORMAT_ASTC_<block>_<channel>`）。
- 文字列は `texformat:` に続けて、ASTC 以外は `TextureFormat` の variant 名、ASTC は `Astc.<block>.<channel>`。
- 例: `TEXFORMAT_R8_UNORM` は `"texformat:R8Unorm"`、`TEXFORMAT_ASTC_B4X4_UNORM` は `"texformat:Astc.B4x4.Unorm"`。
- `"texformat:Astc"` の定数は無い。

### 型と一覧

| 名前 | 内容 |
| ---- | ---- |
| `TextureFormatKey` | 欄は `key: FeatureId`・`format: TextureFormat`。`Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash` を導出。`#[non_exhaustive]` は付かない |
| `TextureFormatRow` | 欄は `row: &'static str`・`keys: &'static [TextureFormatKey]`。導出と `#[non_exhaustive]` は同上 |
| `TEXTURE_FORMAT_QUERY_KEYS` | 118 件。並びは `TextureFormat::all()` と同じ |
| `TEXTURE_FORMAT_ROWS` | 77 行。並びは `TextureFormat` の宣言順で `"Astc"` の行が最後。ASTC 以外の行は鍵 1 件、`"Astc"` の行は 42 件（block の順を外、channel の順を内） |

形式の値から鍵を返す関数、鍵から形式を返す関数、鍵の文字列を解析する関数は持たない。

### 対応する試験

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| core の中の試験（全 variant の網羅 `match` による鍵の対応・名前の規則） | 3 | `cargo test -p fandhe-silicon-core --lib texture_format_key` |
| doctest | 2 | `cargo test -p fandhe-silicon-core --doc texture_format_key` |
| 外の crate からの試験（`crates/core/tests/texture_format_keys.rs`。件数・重複・並び・行の鍵・ASTC の行・文字種） | 8 | `cargo test -p fandhe-silicon-core --test texture_format_keys` |

## 書き出しと突き合わせの検査（2/4）

| 物 | 所在 |
| -- | ---- |
| 表の書き出し（example） | `crates/core/examples/texture_format_keys.rs` |
| 突き合わせの検査 | `scripts/texture-format-key-check.sh` |
| 検査の回帰テスト | `scripts/test-texture-format-key-check.sh` |

### example の出力

`cargo run -q -p fandhe-silicon-core --example texture_format_keys` は、`TEXTURE_FORMAT_ROWS` の並びで鍵 1 件を 1 行にして標準出力へ書く。見出しは無く、列は `<row>` / 鍵 / 形式の `Debug` のタブ区切りの 3 列で、118 行になる。引数は読まず、ファイルは作らない。

### 検査の引数

| 引数 | 既定 | 内容 |
| ---- | ---- | ---- |
| `--table PATH` | なし | 上の 3 列の TSV。省略時は example を cargo で実行して得る（cargo が無ければ終了コード 2） |
| `--csv PATH` | `scripts/inventory-trace-check.sh` と同じ一覧のパス | 先頭のレコードを見出しとし、先頭の列を鍵の列にする |
| `--key-prefix STR` | `texformat:` | CSV の鍵がこれで始まる行だけを対象にする |
| `--enumerate-json PATH` | なし（必須） | 実行時列挙の記録の JSON |
| `--enumerate-list KEY` | なし（必須） | JSON の最上位にある配列の鍵の名前 |
| `--enumerate-item KEY` | なし（必須） | 配列の要素の中の形式の綴りの鍵の名前 |
| `--show-keys` | off | 違反の診断に鍵・綴りの文字列も出す（既定は件数と行番号だけ） |

### 判定

| 判定 | 内容 |
| ---- | ---- |
| (a) 表の形 | 各行が 3 列・空の欄が無い・鍵が互いに異なる・形式が互いに異なる |
| (b) 一覧との対応 | CSV の対象の行の鍵の集合と、表の `<row>` に接頭辞を付けた集合が過不足なく一致する。CSV の鍵の重複と、列の数が見出しと合わない行も違反 |
| (c) 列挙との対応 | 列挙の綴りの集合と、表の形式の集合が過不足なく一致する。重複も違反 |

空の表・空の CSV・対象の行が 0 件の CSV・空の列挙・読めない入力も違反になる。合格のときは標準出力に件数を 1 行で出す（表の行数・`<row>` の数・鍵が 2 件以上の `<row>` ごとの鍵の数・CSV の対象の行数・列挙の件数）。

### 終了コード

| コード | 意味 |
| ------ | ---- |
| 0 | 合格 |
| 1 | 違反（読めない・空の入力を含む） |
| 2 | 誤用・前提の不足（未知の引数・入力が無い・cargo が無い・JSON の 3 引数の欠け） |

### 確かめ方

| 確かめ | コマンド |
| ------ | -------- |
| 回帰テスト（件数と終了コード） | `scripts/test-texture-format-key-check.sh` |
| 書き出しの行数 | `cargo run -q -p fandhe-silicon-core --example texture_format_keys \| wc -l`（118） |
| 書き出しの row の種類 | `cargo run -q -p fandhe-silicon-core --example texture_format_keys \| cut -f1 \| sort -u \| wc -l`（77） |

実物の入力（機能一覧と実行時列挙の記録）は private の submodule の中にあり、CI では流さない。

## CI への組み込み（3/4）

範囲の正は `docs/spec/05-tasks.md` の TASK-1313（ポインタのみ）。本段で足したのは make ターゲット 1 つ（`test-texture-format-key-check`）と、CI の `deny` ジョブの step 1 つで、ジョブは足していない（`ci-complete` の `needs` と必須チェックの名前は変わらない）。

| 検査 | 入口 | CI のジョブ | 結果（記録時点） |
| ---- | ---- | ----------- | ---------------- |
| 突き合わせの検査の回帰テスト | `make test-texture-format-key-check` | `deny`（step 名は `make test-texture-format-key-check`） | 終了コード 0・35 件合格・0 件失敗 |
| core の試験・doctest・example のビルド | `make verify`（`cargo clippy --all-targets`・`cargo test --workspace --all-features`） | `verify`・`verify-macos` | 終了コード 0 |
| 1/4 の試験の wasm32 の型検査 | `make wasm-verify`（core の `--tests` の check） | `deny`・`wasm-macos` | 終了コード 0 |
| CI の枠 | `make ci-frame-check`・`make test-ci-frame-check` | `deny` | 終了コード 0（48 件合格） |
| 公開 API の検査 | `make public-api-check`・`make public-api-sdk-check`・`make public-api-webgpu-check`・`make public-name-check`・`make core-deps-check` | `deny` | いずれも終了コード 0。`record:` の `wgpu_type_refs_total=0`・`sdk_type_refs_total=0`・`name_hits_total=0`・`unexpected=0` |
| 検査本体 | `scripts/texture-format-key-check.sh`（make ターゲットなし） | CI では流さない | 入力が private の submodule の中にある |

値は記録時点（origin/main の `a202e0c`）のもの。

### 確かめ方

```bash
grep -n 'test-texture-format-key-check' .github/workflows/ci.yml Makefile scripts/help.sh
make ci-frame-check
make test-texture-format-key-check
make public-api-check && make public-api-sdk-check && make public-api-webgpu-check && make public-name-check && make core-deps-check
make wasm-verify
```

## 完了の条件の確かめ（4/4）

4/4（#6716）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-1313）。GPU の実機なしで実行し、件数は実測。コードと試験は変えていない。

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 共通の crate に feature の条件なしで置かれている | `crates/core/src/lib.rs:51` の `pub mod texture_format_key;`（前後の行に `#[cfg` なし）、`cargo check -p fandhe-silicon-core`（feature なし） | 終了コード 0 |
| (2) | 鍵の件数・行の数・ASTC の行の鍵の数と名前の規則 | `cargo test -p fandhe-silicon-core --test texture_format_keys`（`query_keys_are_118_and_unique`・`query_keys_follow_all_formats`・`rows_are_77_with_unique_names`・`rows_flatten_to_query_keys`・`non_astc_rows_have_one_key_named_after_row`・`astc_row_has_block_by_channel_keys`・`astc_row_key_is_not_listed`・`keys_use_limited_characters`）、`--lib texture_format_key`、`--doc texture_format_key` | 8 件・3 件・2 件とも passed。`--all-features` 付きの `--test texture_format_keys` も 8 件 passed |
| (3) | 表の書き出し | `cargo run -q -p fandhe-silicon-core --example texture_format_keys` の出力を `wc -l`、`cut -f1 \| sort -u \| wc -l`、`awk -F'\t' '$1=="Astc"' \| wc -l` | 118 行・先頭の列の異なる値 77・`Astc` の行 42 |
| (4) | 突き合わせの検査の振る舞い | `make test-texture-format-key-check` | 終了コード 0。`# passed: 35, failed: 0`。実物の example の出力を通す 2 件を含む。`docs/spec` の取得は要らない |
| (5) | 実物の入力での検査 | `scripts/texture-format-key-check.sh --enumerate-json <path> --enumerate-list <key> --enumerate-item <key>` | 入力が private の submodule の中にあり、本リポの CI と自動実装の作業ツリーでは流さない。結果は private spec 側の記録に従う（ポインタのみ） |
| (6) | 公開 API に wgpu の型が 0 件（TASK-10 のポインタ） | `make public-api-check` | 終了コード 0。core の行は `wgpu_type_refs=0 wgpu_other_refs=0`（production・all-features とも） |
| (7) | 公開 API にチップの SDK の型が 0 件（TASK-13 のポインタ） | `make public-api-sdk-check` | 終了コード 0。core の行は `sdk_type_refs=0 sdk_other_refs=0`（production・all-features とも） |
| (8) | ほかの公開 API の検査 | `make public-name-check`・`make public-api-webgpu-check`・`make core-deps-check` | すべて終了コード 0。`name_hits_total=0`・`webgpu_names_total=0`・`unexpected=0 missing=0` |
| (9) | CI で流れる場所 | 上の 3/4 の節の表。`gh run list --branch main --workflow ci.yml` | 1/4 のマージ `1019d00` は run 41338767250 success、2/4 の `a202e0c` は run 41341611934 success、3/4 の `0667bc4` は run 41343032363 で記録時点は in_progress（結論は未確定） |
| (10) | 依存 0・unsafe の追加 0 | `ea7567c` から `0667bc4` までの `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` の差分、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n forbid crates/core/src/lib.rs`、`grep -c unsafe crates/core/src/texture_format_key.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | 3 ファイルとも差分なし、承認の記録 4 件のまま、`:33` に `#![forbid(unsafe_code)]`、`unsafe` は 0 件、core の外部依存は 0 件 |
| (11) | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |

上の表の `lib.rs:51` は記録時点の値で、今は `lib.rs:61` が `pub mod texture_format_key;` である。
その後 `lib.rs` に行が入り、今は `lib.rs:63` が `pub mod texture_format_key;` である。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1313）。確かめた base は `0667bc4796ec9d0e1a6052ec4fd0bee7fa621ff5`。

### 再実行の契機

- `crates/core/src/texture_format_key.rs`・`crates/core/tests/texture_format_keys.rs`・`crates/core/examples/texture_format_keys.rs`・`scripts/texture-format-key-check.sh` を変えたとき。
- `TextureFormat` に値が増えたとき。
- 公開 API の検査（`scripts/public-api-check.sh` ほか）を変えたとき。

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #6713 | 形式の問合せの鍵の一覧と行の表 |
| 2/4 | #6714 | 表の書き出しと一覧との突き合わせの検査 |
| 3/4 | #6715 | CI への組み込み |
| 4/4 | #6716 | 確かめ |

関連: `Capabilities::texture_format_features` の既定の実装は、`TEXTURE_FORMAT_ROWS` を先頭から探して形式の鍵を引く（内部の補助で、公開の関数ではない）。所在は [texture-format-features.md](texture-format-features.md)。
