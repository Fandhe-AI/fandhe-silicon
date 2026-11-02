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

## 機能一覧との突き合わせの検査と CI への組み込み（5/6）

鍵の一覧を機能一覧の CSV に突き合わせる検査を、core の example・検査のスクリプト・回帰テストで持つ。

| 物 | 所在 |
| -- | ---- |
| example | `crates/core/examples/feature_keys.rs` |
| 検査 | `scripts/feature-key-check.sh`（`make feature-key-check`） |
| 回帰テスト | `scripts/test-feature-key-check.sh`（`make test-feature-key-check`） |

`cargo run -q -p fandhe-silicon-core --example feature_keys` は、`FEATURE_QUERY_KEYS` の並びで鍵 1 件を 1 行にして標準出力へ書く。見出しは無く、列は鍵の文字列そのものの 1 列で、79 行になる。引数は読まず、ファイルは作らない。

### 検査の引数

| 引数 | 既定 | 内容 |
| ---- | ---- | ---- |
| `--keys PATH` | なし | 上の 1 列のテキスト。省略時は example を cargo で実行して得る（cargo が無ければ終了コード 2） |
| `--csv PATH` | `docs/spec/03-poc/wgpu-feature-inventory/inventory.csv` | 機能一覧の CSV。先頭のレコードを見出しとし、先頭の列を鍵の列にする |
| `--key-prefix STR` | `feature:` | 鍵がこれで始まる行だけを CSV の対象にする |
| `--show-keys` | 無効 | 違反の診断に鍵の文字列も出す（既定は件数と行番号だけ） |

### 判定

| 判定 | 内容 |
| ---- | ---- |
| (a) 一覧の形 | 各行が 1 列（タブなし）で空でない・互いに異なる・どれも接頭辞で始まる |
| (b) 一覧との対応 | CSV の対象の行の鍵の集合と一覧の鍵の集合が過不足なく一致する。CSV の鍵の重複・列数の不一致も違反 |

空の一覧・空の CSV・対象の行が 0 件の CSV・読めない入力は、`FAILED` の行なしで終了コード 1。合格のときは標準出力に `[feature-key-check.sh] ok: keys=<一覧の件数> csv=<CSV の対象の行数>` の 1 行を出す。件数の期待の値はスクリプトに持たない。

### 終了コード

| コード | 意味 |
| ------ | ---- |
| 0 | 合格 |
| 1 | 違反（読めない・空の入力を含む） |
| 2 | 誤用・前提の不足（未知の引数・入力が無い・cargo が無い・example の失敗） |

### 流れる場所

| 検査 | 実行 | CI のジョブ | 記録時点の結果 |
| ---- | ---- | ----------- | -------------- |
| 突き合わせの検査の回帰テスト | `make test-feature-key-check` | `deny`（step 名は `make test-feature-key-check`） | 終了コード 0・23 件合格・0 件失敗 |
| core の試験・doctest・example のビルド | `make verify`（`cargo clippy --all-targets`・`cargo test --workspace --all-features`） | `verify`・`verify-macos` | 終了コード 0 |
| 試験の wasm32 の型検査 | `make wasm-verify`（core の `--tests` の check） | `deny`・`wasm-macos` | 終了コード 0 |
| 公開 API の検査 | `make public-api-check`・`public-api-sdk-check`・`public-api-webgpu-check`・`public-name-check`・`core-deps-check` | `deny` | 新しい module を覆う |

実物の機能一覧への実行（`make feature-key-check`）は、入力が private の submodule の中にあり、CI では流さない。

### 確かめ方

- `make test-feature-key-check`
- `cargo run -q -p fandhe-silicon-core --example feature_keys | wc -l`（記録時点は 79）
- `grep -n 'test-feature-key-check' .github/workflows/ci.yml Makefile scripts/help.sh`
- `make ci-frame-check`

記録時点の base は `26a651c`。

## 完了の条件の確かめ（6/6）

6/6（#6440）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-1114）。GPU の実機なしで実行し、件数は実測。コードと試験は変えていない。

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 鍵の定数と一覧、行の型が共通の crate に feature の条件なしで置かれている | `grep -n 'pub mod feature_key\|pub mod capability_table' crates/core/src/lib.rs`、`cargo check -p fandhe-silicon-core`（feature なし） | `:40` の `pub mod capability_table;` と `:46` の `pub mod feature_key;`（どちらも前の行に `#[cfg` なし）。grep は `:62` の `capability_table_check` にも当たるが、これは `:61` の `#[cfg(feature = "test-support")]` の裏に置くもの。`cargo check` は終了コード 0 |
| (2) | 鍵が 79 件・互いに異なる | `cargo test -p fandhe-silicon-core --features test-support --test feature_key_table`（`feature_query_keys_are_complete_list`）、`cargo test -p fandhe-silicon-core --lib feature_key`、`cargo run -q -p fandhe-silicon-core --example feature_keys \| wc -l`、同じ出力の `sort -u \| wc -l` | `feature_query_keys_are_complete_list` は ok（同じ試験の対象は 6 件で 6 passed）。`--lib feature_key` は 4 件 passed。行数 79・異なる行 79 |
| (3) | 載っていない鍵・余分な鍵で検査が失敗する | `--test feature_key_table`（`missing_or_extra_key_is_detected`）、`make test-feature-key-check` | `missing_or_extra_key_is_detected` は ok。`make test-feature-key-check` は終了コード 0、`# passed: 23, failed: 0` |
| (4) | 行の無い鍵の値が `NoApi` でない | `--test feature_key_table`（`unlisted_key_is_unknown_feature_not_no_api`）、`cargo test -p fandhe-silicon-core --lib capability_table`（feature なし） | `unlisted_key_is_unknown_feature_not_no_api` は ok。`--lib capability_table` は 3 件 passed（`test-support` を付けないので `capability_table_check` の試験は含まれない） |
| (5) | `S3` 以上の値に根拠がある | `--test feature_key_table`（`s3_and_above_carry_evidence`） | ok |
| (6) | 差分の台帳の行の型と検査 | `--test feature_key_table`（`ledger_entries_follow_key_list`）、`cargo test -p fandhe-silicon-core --features test-support --lib capability_table_check` | `ledger_entries_follow_key_list` は ok。`--lib capability_table_check` は 9 件 passed |
| (7) | 実物の機能一覧との突き合わせ | `make feature-key-check` | 入力が private の submodule の中にあり、本リポの CI と自動実装の作業ツリーでは流さない。結果は private spec 側の記録に従う（ポインタのみ） |
| (8) | 公開 API に wgpu の型が 0 件（TASK-10 のポインタ） | `make public-api-check` | 終了コード 0。core の行は `wgpu_type_refs=0 wgpu_other_refs=0`（production・all-features とも） |
| (9) | 公開 API にチップの SDK の型が 0 件（TASK-13 のポインタ） | `make public-api-sdk-check` | 終了コード 0。core の行は `sdk_type_refs=0 sdk_other_refs=0`（production・all-features とも） |
| (10) | ほかの公開 API の検査 | `make public-name-check`・`make public-api-webgpu-check`・`make core-deps-check` | すべて終了コード 0。`name_hits_total=0`・`webgpu_names_total=0`・`unexpected=0 missing=0` |
| (11) | CI で流れる場所 | 本書の 5/6 の「流れる場所」の表。`gh run list --branch main --workflow ci.yml --event push` | 1/6 のマージ `9950fdb` は run 41358114920 success、2/6 の `356e2b0` は run 41359320386 success、3/6 の `5ba9135` は run 41360949623 success、4/6 の `26a651c` は run 41362264239 success、5/6 の `274de7c` は run 41363951778 success（記録時点は in_progress で、記録の後に success を確かめた）、6/6 の `03db5ee` は run 41365279502 success（記録の後に確かめた） |
| (12) | 依存 0・unsafe の追加 0 | `e339fcc` から `274de7c` までの `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` の差分、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n forbid crates/core/src/lib.rs`、新しい 5 つのファイル（`feature_key.rs`・`capability_table.rs`・`capability_table_check.rs`・`examples/feature_keys.rs`・`tests/feature_key_table.rs`）への `grep -c unsafe`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | `Cargo.lock`・`deps-approved.toml` は差分なし。`crates/core/Cargo.toml` の差分は `feature_key_table` の `[[test]]` 1 項目（`required-features = ["test-support"]`）だけ。承認の記録は 4 件のまま、`:35` に `#![forbid(unsafe_code)]`、`unsafe` は 5 ファイルとも 0 件、core の外部依存は 0 件 |
| (13) | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1114）。確かめた base は `274de7cbd47a111cabd326a52e5529526afd12d8`。

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #6435 | 鍵の定数と一覧（本書の「現状の形（1/6）」） |
| 2/6 | #6436 | 能力の表の行の型と引き方（本書の「能力の表の行の型と引き方（2/6）」） |
| 3/6 | #6437 | 表の網羅の検査の道具と差分の台帳（本書の「表の網羅の検査の道具と差分の台帳（3/6）」） |
| 4/6 | #6438 | 鍵の一覧と表の枠の試験（本書の「鍵の一覧と表の枠の試験（4/6）」） |
| 5/6 | #6439 | 機能一覧との突き合わせの検査と CI への組み込み（本書の「機能一覧との突き合わせの検査と CI への組み込み（5/6）」） |
| 6/6 | #6440 | 完了の条件の確かめ（本書の「完了の条件の確かめ（6/6）」） |
