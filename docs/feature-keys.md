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

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #6435 | 鍵の定数と一覧（本書の「現状の形（1/6）」） |
| 2/6 | #6436 | 実装 |
| 3/6 | #6437 | 実装 |
| 4/6 | #6438 | テスト |
| 5/6 | #6439 | 検査と CI への組み込み |
| 6/6 | #6440 | 完了の条件の確かめ |
