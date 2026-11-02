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
| `crates/core/src/value_key.rs` | `fandhe_silicon_core::value_key` |

- `crates/core/src/lib.rs` に `pub mod limit_key;`・`pub mod downlevel_key;`・`pub mod value_key;` があり、feature の条件はない。crate の根の `pub use` は無い。

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

## 値の鍵（2/5）

### 型

`ValueKey(pub &'static str)`。派生する trait は `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash`。`FeatureId` とは別の型で、相互の変換は持たない。

### 上限の値の鍵の定数

57 件。名前は `limit_key` の定数と同じ名前（`LIMIT_` に続けて大文字の名前）で、文字列も同じ（例: `value_key::LIMIT_MAX_BUFFER_SIZE` は `"limit:max_buffer_size"`）。
2 つの module を同時に glob で import すると名前があいまいになるので、完全なパスか module の別名で使う。

### 一覧

| 名前 | 内容 |
| ---- | ---- |
| `LIMIT_VALUE_KEYS` | `&[ValueKey]`。57 件で、鍵は互いに異なる。`LIMIT_QUERY_KEYS` と同じ並びで、同じ位置の鍵は同じ文字列を持つ |

### 対応するテスト

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| core の中の試験（件数・相異・`LIMIT_QUERY_KEYS` との位置ごとの一致・定数の抜き取り） | 4 | `cargo test -p fandhe-silicon-core --lib value_key` |
| 一覧の doctest | 1 | `cargo test -p fandhe-silicon-core --doc value_key` |

## 鍵の一覧と表の枠の試験（3/5）

`crates/core/tests/limit_downlevel_key_table.rs` に、公開のパスだけを使う外の crate の試験を置く。`crates/core/Cargo.toml` の `[[test]]` で feature `test-support` を要る（`required-features`）。
上限の一覧と downlevel の一覧を、それぞれ [feature-keys.md](feature-keys.md) の能力の表の枠（`FeatureRow`・`query_from_table`・`UNLISTED_FEATURE_QUERY`・`check_key_list`・`check_table_keys`・`check_table_queries`）に載せる。

| 試験 | 確かめること |
| ---- | ------------ |
| `limit_and_downlevel_key_lists_are_complete` | 上限の一覧が 57 件・downlevel の一覧が 26 件で、一覧ごとに `check_key_list` が通り、全鍵が `limit:`・`downlevel:` で始まる |
| `missing_or_extra_key_is_detected` | 一覧ごとに、全鍵の行の表が `check_table_keys` を通る。一覧から 1 件を除くと `RowKeyNotInList`、架空の鍵・もう一方の一覧の鍵の行を足すと `RowKeyNotInList`、同じ鍵の行を 2 つにすると `DuplicateRowKey`、一覧に重複を足すと `DuplicateKey` |
| `unlisted_key_is_unknown_feature_not_no_api` | 一覧ごとに、行の無い鍵は `UNLISTED_FEATURE_QUERY`（`Unsupported`・`UnknownFeature`・根拠なし）で `NoApi` ではなく、空の表では `eval` を呼ばない。行のある鍵は行の値を返す |
| `s3_and_above_carry_evidence` | 一覧ごとに、`S1` 以上と `Unsupported` の根拠つきの表は `check_table_queries` を通り、根拠の無い `S3` の行を足すと `MissingEvidence` の 1 件になる |
| `value_keys_follow_limit_keys` | `LIMIT_VALUE_KEYS` が `LIMIT_QUERY_KEYS` と同じ件数・同じ位置で同じ文字列で、互いに異なる |
| `key_lists_are_disjoint` | 上限・downlevel・機能・テクスチャの形式の 4 つの鍵の一覧のどの 2 つも共通の鍵を持たない |

| 試験 | 件数（記録時点の値） | 確かめ方 |
| ---- | -------------------- | -------- |
| 外の crate の試験 | 6 | `cargo test -p fandhe-silicon-core --features test-support --test limit_downlevel_key_table` |

## 機能一覧との突き合わせの検査（4/5）

鍵の一覧を 1 列のテキストで書き出す example を 2 本置き、既存の `scripts/feature-key-check.sh` に `--example` で渡す。

| example | 一覧 | 出力の行数 |
| ------- | ---- | ---------- |
| `crates/core/examples/limit_keys.rs` | `LIMIT_QUERY_KEYS` | 57 |
| `crates/core/examples/downlevel_keys.rs` | `DOWNLEVEL_QUERY_KEYS` | 26 |

出力は見出しなしの 1 列で、1 行が鍵 1 件に当たる。引数は読まず、ファイルは作らない。

`--example NAME` は `--keys` を省いたときに cargo で実行する example の名前で、既定は `feature_keys`。英小文字・数字・`_` だけを受け、外れたら終了コード 2。`--keys` と同時に渡しても終了コード 2。接頭辞は自動で決めないので、呼び手が `--key-prefix` で渡す。判定と終了コードは [feature-keys.md](feature-keys.md) の 5/6 の節に従う（TASK-1114 のポインタ）。

実物の機能一覧は private の submodule の中にあり、CI では流さない。手元での形は次のとおり。

```text
scripts/feature-key-check.sh --example limit_keys --key-prefix limit:
scripts/feature-key-check.sh --example downlevel_keys --key-prefix downlevel:
```

| 確かめること | 確かめ方 |
| ------------ | -------- |
| 回帰テスト（`--example` の誤用と、example の実物の出力の形・合成の CSV での合格） | `make test-feature-key-check`（CI の `deny` ジョブが呼ぶ） |
| 出力の行数 | `cargo run -q -p fandhe-silicon-core --example limit_keys \| wc -l`（57）、`--example downlevel_keys \| wc -l`（26） |

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/5 | #7702 | 上限と downlevel の問合せの鍵の定数と一覧 |
| 2/5 | #7703 | 値の鍵の型と上限の値の鍵の定数（本書の「値の鍵（2/5）」） |
| 3/5 | #7704 | 上限と downlevel の鍵の一覧と表の枠の試験（本書の「鍵の一覧と表の枠の試験（3/5）」） |
| 4/5 | #7705 | 上限と downlevel の鍵の一覧を機能一覧に突き合わせる検査（本書の「機能一覧との突き合わせの検査（4/5）」） |
| 5/5 | #7706 | 完了の条件の確かめ |
