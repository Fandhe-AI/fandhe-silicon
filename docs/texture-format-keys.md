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

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #6713 | 形式の問合せの鍵の一覧と行の表 |
| 2/4 | #6714 | 表の書き出しと一覧との突き合わせの検査 |
| 3/4 | #6715 | CI への組み込み |
| 4/4 | #6716 | 確かめ |
