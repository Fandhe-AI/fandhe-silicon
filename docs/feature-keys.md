# 機能の問合せの鍵の一覧と能力の表の枠（TASK-1114）の所在と本リポの現状

TASK-1114 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1114。項目のポインタ COVF-3・COVF-4・COVF-5。ポインタのみ）。
内容・理由・経緯は本書に置かない。

## 所在

- `crates/core/src/feature_key.rs`。公開のパスは `fandhe_silicon_core::feature_key`。
- `crates/core/src/lib.rs` に `pub mod feature_key;` があり、feature の条件はない。crate の根の `pub use` は無い。

## 現状の形（1/6）

### 鍵の定数

- `FeatureId` の定数が 79 個。名前は `FEATURE_` に続けて大文字の名前、文字列は `feature:` に続けて同じ名前。
- 例: `FEATURE_SHADER_F16` は `"feature:SHADER_F16"`。

### 一覧

| 名前 | 内容 |
| ---- | ---- |
| `FEATURE_QUERY_KEYS` | `&[FeatureId]`。79 件で、鍵は互いに異なる |

鍵から何かを引く関数は持たない。

### 対応するテスト

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| core の中の試験（件数・相異・文字列の形・定数名と文字列の対応） | 4 | `cargo test -p fandhe-silicon-core --lib feature_key` |
| 一覧の doctest | 1 | `cargo test -p fandhe-silicon-core --doc feature_key` |

## 能力の表の行の型と引き方（2/6）

### 所在

- `crates/core/src/capability_table.rs`。公開のパスは `fandhe_silicon_core::capability_table`。
- `crates/core/src/lib.rs` に `pub mod capability_table;` があり、feature の条件はない。

### 型・定数・関数

| 名前 | 形 | 内容 |
| ---- | -- | ---- |
| `FeatureRow<R>` | 構造体。欄は `feature`・`rule`・`source` | 鍵・規則の値・出どころの文字列 |
| `UNLISTED_FEATURE_QUERY` | `CapabilityQuery` の定数 | 段階 `Unsupported`、理由 `UnknownFeature`、根拠は無し |
| `find_row` | 表と鍵を受けて `Option<&'static FeatureRow<R>>` を返す | 先頭から探し、最初に鍵が等しい行を返す |
| `query_from_table` | 表・鍵・`eval` を受けて `CapabilityQuery` を返す | 行があれば `eval` の値、無ければ `UNLISTED_FEATURE_QUERY` |

### 入力ごとの返し

| 入力 | `find_row` | `query_from_table` |
| ---- | ---------- | ------------------ |
| 行がある | 行 | `eval` の値 |
| 行が無い | `None` | `UNLISTED_FEATURE_QUERY`（`eval` を呼ばない） |
| 鍵が重なる | 先の行 | 先の行の値 |

### 対応するテスト

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| core の中の試験 | 3 | `cargo test -p fandhe-silicon-core --lib capability_table` |
| doctest | 2 | `cargo test -p fandhe-silicon-core --doc capability_table` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #6435 | 鍵の定数と一覧（本書の「現状の形（1/6）」） |
| 2/6 | #6436 | 能力の表の行の型と引き方（本書の「能力の表の行の型と引き方（2/6）」） |
| 3/6 | #6437 | 実装 |
| 4/6 | #6438 | テスト |
| 5/6 | #6439 | 検査と CI への組み込み |
| 6/6 | #6440 | 完了の条件の確かめ |
