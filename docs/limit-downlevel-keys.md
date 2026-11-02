# 上限と downlevel の問合せの鍵の定数と一覧（TASK-1362）の所在と本リポの現状

TASK-1362 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1362。ポインタのみ）。
内容・理由・経緯は本書に置かない。

## 所在

| ファイル | 公開のパス |
| -------- | ---------- |
| `crates/core/src/limit_key.rs` | `fandhe_silicon_core::limit_key` |
| `crates/core/src/downlevel_key.rs` | `fandhe_silicon_core::downlevel_key` |

- `crates/core/src/lib.rs` に `pub mod limit_key;` と `pub mod downlevel_key;` があり、feature の条件はない。crate の根の `pub use` は無い。

## 現状の形（1/5）

### 鍵の定数

| 種類 | 定数の名前 | 文字列 | 例 |
| ---- | ---------- | ------ | -- |
| 上限（57 個） | `LIMIT_` に続けて大文字の名前 | `limit:` に続けて小文字の名前 | `LIMIT_MAX_BUFFER_SIZE` は `"limit:max_buffer_size"` |
| downlevel（26 個） | `DOWNLEVEL_` に続けて大文字の名前 | `downlevel:` に続けて大文字の名前 | `DOWNLEVEL_COMPUTE_SHADERS` は `"downlevel:COMPUTE_SHADERS"` |

どの定数も `FeatureId` の値である。

### 一覧

| 名前 | 内容 |
| ---- | ---- |
| `LIMIT_QUERY_KEYS` | `&[FeatureId]`。57 件で、鍵は互いに異なる |
| `DOWNLEVEL_QUERY_KEYS` | `&[FeatureId]`。26 件で、鍵は互いに異なる |

鍵から何かを引く関数は持たない。鍵は `FeatureId` なので、[feature-keys.md](feature-keys.md) の能力の表の枠（`FeatureRow<R>` の `feature` 欄・`query_from_table` など）にそのまま置ける。

### 対応するテスト

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| 上限: core の中の試験（件数・相異・文字列の形・定数名と文字列の対応） | 4 | `cargo test -p fandhe-silicon-core --lib limit_key` |
| downlevel: core の中の試験（同上） | 4 | `cargo test -p fandhe-silicon-core --lib downlevel_key` |
| 上限の一覧の doctest | 1 | `cargo test -p fandhe-silicon-core --doc limit_key` |
| downlevel の一覧の doctest | 1 | `cargo test -p fandhe-silicon-core --doc downlevel_key` |

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/5 | #7702 | 上限と downlevel の問合せの鍵の定数と一覧 |
| 2/5 | #7703 | 値の鍵の型と上限の値の鍵の定数 |
| 3/5 | #7704 | 上限と downlevel の鍵の一覧と表の枠の試験 |
| 4/5 | #7705 | 上限と downlevel の鍵の一覧を機能一覧に突き合わせる検査 |
| 5/5 | #7706 | 完了の条件の確かめ |
