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

## 表の網羅の検査の道具と差分の台帳（3/6）

### 所在

- `crates/core/src/capability_table_check.rs`。公開のパスは `fandhe_silicon_core::capability_table_check`。
- `crates/core/src/lib.rs` で feature `test-support` の裏に置かれ、製品の経路からは使わない。

### 関数

違反は全部集めて入力の順に返す。違反が無ければ `Ok(())`。重複は 2 回目以降の出現 1 つにつき 1 件。

| 名前 | 入力 | 確かめること | 返す違反の型 |
| ---- | ---- | ------------ | ------------ |
| `check_key_list` | 鍵の一覧 | 鍵が空でない・互いに異なる | `KeyTableViolation` |
| `check_table_keys` | 表と鍵の一覧 | 行の鍵が一覧にある・表の中で互いに異なる（一覧の鍵が表に無くてもよい） | `KeyTableViolation` |
| `check_table_queries` | 表・鍵の一覧・`eval` | 一覧の各鍵で `query_from_table` の値が `check_query` を通る（一覧の外の鍵の行は評価しない） | `CapabilityRuleViolation` |
| `check_ledger_keys` | 台帳と鍵の一覧 | 行の鍵が一覧にある・（鍵, 背後実装）が互いに異なる・試験の名前が空でない | `KeyTableViolation` |

### 違反

| variant | 出る条件 |
| ------- | -------- |
| `EmptyKey { index }` | 鍵の文字列が空、または最初の `:` の後が空 |
| `DuplicateKey { key }` | 一覧の中で鍵が重なる |
| `RowKeyNotInList { key }` | 表の行の鍵が一覧に無い |
| `DuplicateRowKey { key }` | 表の中で行の鍵が重なる |
| `LedgerKeyNotInList { key }` | 台帳の行の鍵が一覧に無い |
| `DuplicateLedgerEntry { key, backend }` | 台帳の中で（鍵, 背後実装）が重なる |
| `EmptyTestName { key, backend }` | 台帳の行の試験の名前が空 |

### 台帳の行の型

| 名前 | 内容 |
| ---- | ---- |
| `BaselineDiffEntry` | 欄は `feature`（鍵）・`backend`（背後実装）・`result`（`BaselineDiff`）・`test`（確かめる公開テストの名前） |
| `BaselineDiff` | `Same`（比較基準と一致）・`Differs`（比較基準と違う） |

台帳の行そのものは置かない。

### 対応するテスト

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| core の中の試験 | 9 | `cargo test -p fandhe-silicon-core --features test-support --lib capability_table_check` |
| doctest | 4 | `cargo test -p fandhe-silicon-core --features test-support --doc capability_table_check` |

## 鍵の一覧と表の枠の試験（4/6）

`crates/core/tests/feature_key_table.rs` に、公開のパスだけを使う外の crate の試験を置く。`crates/core/Cargo.toml` の `[[test]]` で feature `test-support` を要る（`required-features`）。

| 試験 | 確かめること |
| ---- | ------------ |
| `feature_query_keys_are_complete_list` | 一覧が 79 件で `check_key_list` が `Ok`、すべての鍵が `feature:` で始まる |
| `missing_or_extra_key_is_detected` | 79 行の表は `Ok`。一覧から 1 件除くと `RowKeyNotInList`、一覧に無い鍵の行で `RowKeyNotInList`、行の重なりで `DuplicateRowKey`、一覧の鍵の重なりで `DuplicateKey` がそれぞれ 1 件だけ返る |
| `unlisted_key_is_unknown_feature_not_no_api` | 空の表では 79 鍵すべてが `UNLISTED_FEATURE_QUERY`（`Unsupported`・`UnknownFeature`・根拠なし）で `NoApi` でなく、`eval` は呼ばれない。3 行の表では行の鍵は行の値、残り 76 件は `UNLISTED_FEATURE_QUERY` |
| `s3_and_above_carry_evidence` | 根拠つきの `S1`〜`S4` と `Unsupported` の表は `check_table_queries` が `Ok`。`S3` で根拠なしの行を 1 つ足すと `MissingEvidence` が 1 件だけ返る |
| `texture_format_required_features_are_listed` | `TextureFormat::all()` の `required_feature()` が `Some` の鍵がすべて一覧に入っており、`Some` は 1 件以上 |
| `ledger_entries_follow_key_list` | 空の台帳と有効な台帳は `Ok`。一覧の外の鍵は `LedgerKeyNotInList`、（鍵, 背後実装）の重なりは `DuplicateLedgerEntry`、空の試験の名前は `EmptyTestName` |

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| 外の crate の試験 | 6 | `cargo test -p fandhe-silicon-core --features test-support --test feature_key_table` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #6435 | 鍵の定数と一覧（本書の「現状の形（1/6）」） |
| 2/6 | #6436 | 能力の表の行の型と引き方（本書の「能力の表の行の型と引き方（2/6）」） |
| 3/6 | #6437 | 表の網羅の検査の道具と差分の台帳（本書の「表の網羅の検査の道具と差分の台帳（3/6）」） |
| 4/6 | #6438 | 鍵の一覧と表の枠の試験（本書の「鍵の一覧と表の枠の試験（4/6）」） |
| 5/6 | #6439 | 検査と CI への組み込み |
| 6/6 | #6440 | 完了の条件の確かめ |
