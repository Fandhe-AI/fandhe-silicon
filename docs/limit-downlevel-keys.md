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

補足: 上の表の 4 は記録時点の値である。2/5 の後は `--lib limit_key` が名前の部分一致で `value_key::tests::constants_match_limit_key_constants` も拾い、いまは 5 件になる。`--lib limit_key::` で絞ると 4 件である。

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

## 完了の条件の確かめ（5/5）

5/5（#7706）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-1362）。GPU の実機なしで実行し、件数は実測。コードと試験は変えていない。

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 鍵の定数と一覧が共通の crate に feature の条件なしで置かれている | `grep -n 'pub mod limit_key\|pub mod downlevel_key\|pub mod value_key' crates/core/src/lib.rs`、`cargo check -p fandhe-silicon-core`（feature なし） | `:46` の `pub mod downlevel_key;`・`:53` の `pub mod limit_key;`・`:61` の `pub mod value_key;`（どれも前の行に `#[cfg` なし）。`cargo check` は終了コード 0 |
| (2) | 上限 57 件・downlevel 26 件で、どれも互いに異なる | `cargo test -p fandhe-silicon-core --features test-support --test limit_downlevel_key_table`（`limit_and_downlevel_key_lists_are_complete`）、`cargo test -p fandhe-silicon-core --lib limit_key::`・`--lib downlevel_key::`、`cargo run -q -p fandhe-silicon-core --example limit_keys \| wc -l` と `\| sort -u \| wc -l`、`--example downlevel_keys` で同じ | `limit_and_downlevel_key_lists_are_complete` は ok（同じ試験の対象は 6 件で 6 passed）。`--lib limit_key::`・`--lib downlevel_key::` はそれぞれ 4 passed。limit は 57 行・異なる行 57、downlevel は 26 行・異なる行 26 |
| (3) | 載っていない鍵・余分な鍵で検査が失敗する | `--test limit_downlevel_key_table`（`missing_or_extra_key_is_detected`）、`make test-feature-key-check` | `missing_or_extra_key_is_detected` は ok。`make test-feature-key-check` は終了コード 0、`# passed: 31, failed: 0` |
| (4) | 行の無い鍵の値が `NoApi` でない | `--test limit_downlevel_key_table`（`unlisted_key_is_unknown_feature_not_no_api`） | ok |
| (5) | `S3` 以上の値に根拠がある | `--test limit_downlevel_key_table`（`s3_and_above_carry_evidence`） | ok |
| (6) | 値の鍵が上限の鍵と同じ文字列 | `cargo test -p fandhe-silicon-core --lib value_key`、`--test limit_downlevel_key_table`（`value_keys_follow_limit_keys`） | `--lib value_key` は 4 passed、`value_keys_follow_limit_keys` は ok |
| (7) | 一覧どうしが共通の鍵を持たない | `--test limit_downlevel_key_table`（`key_lists_are_disjoint`） | ok |
| (8) | 実物の機能一覧との突き合わせ | `scripts/feature-key-check.sh --example limit_keys --key-prefix limit:`、`scripts/feature-key-check.sh --example downlevel_keys --key-prefix downlevel:` | 入力が private の submodule の中にあり、本リポの CI と自動実装の作業ツリーでは流さない。結果は private spec 側の記録に従う（ポインタのみ） |
| (9) | 公開 API に wgpu の型が 0 件（TASK-10 のポインタ） | `make public-api-check` | 終了コード 0。core の行は `wgpu_type_refs=0 wgpu_other_refs=0`（production・all-features とも） |
| (10) | 公開 API にチップの SDK の型が 0 件（TASK-13 のポインタ） | `make public-api-sdk-check` | 終了コード 0。core の行は `sdk_type_refs=0 sdk_other_refs=0`（production・all-features とも） |
| (11) | ほかの公開 API の検査 | `make public-name-check`・`make public-api-webgpu-check`・`make core-deps-check` | すべて終了コード 0。`name_hits_total=0`・`webgpu_names_total=0`・`unexpected=0 missing=0` |
| (12) | CI で流れる場所 | `make verify`（1/5〜3/5 の試験と doctest、4/5 の example のビルド）、`make wasm-verify`、`deny` ジョブの `make test-feature-key-check`。`gh run list --branch main --workflow ci.yml --event push` | 1/5 のマージ `3b14e4b` は run 41395749496 success、2/5 の `8ecdcba` は run 41397517477 success、3/5 の `2ea8f74` は run 41399397222 success、4/5 の `80fd632` は run 41401196102（記録時点は queued で、結論は確かめていない） |
| (13) | 依存 0・unsafe の追加 0 | `32c8e0b`（1/5 の親）から `80fd632` までの `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` の差分、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n forbid crates/core/src/lib.rs`、新しい 6 つのファイル（`limit_key.rs`・`downlevel_key.rs`・`value_key.rs`・`examples/limit_keys.rs`・`examples/downlevel_keys.rs`・`tests/limit_downlevel_key_table.rs`）への `grep -c unsafe`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | `Cargo.lock`・`deps-approved.toml` は差分なし。`crates/core/Cargo.toml` の差分は `limit_downlevel_key_table` の `[[test]]`（`required-features = ["test-support"]`）の 1 項目だけ。承認の記録は 4 件。`:37` に `#![forbid(unsafe_code)]`。`unsafe` は 6 ファイルとも 0 件。`cargo tree` は core 自身の 1 行だけで、外部依存は 0 件 |
| (14) | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |

補足: 上の表の (12) の 4/5 の run 41401196102 は記録時点は queued だった。その後、この run と 5/5 の run 41403155522 はどちらも success で終わったことを `gh run view <id> --json conclusion` で確かめた。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1362）。確かめた base は `80fd63237a3251b38eeba84750a925f73d7cb286`。

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/5 | #7702 | 上限と downlevel の問合せの鍵の定数と一覧 |
| 2/5 | #7703 | 値の鍵の型と上限の値の鍵の定数（本書の「値の鍵（2/5）」） |
| 3/5 | #7704 | 上限と downlevel の鍵の一覧と表の枠の試験（本書の「鍵の一覧と表の枠の試験（3/5）」） |
| 4/5 | #7705 | 上限と downlevel の鍵の一覧を機能一覧に突き合わせる検査（本書の「機能一覧との突き合わせの検査（4/5）」） |
| 5/5 | #7706 | 完了の条件の確かめ（本書の「完了の条件の確かめ（5/5）」） |
