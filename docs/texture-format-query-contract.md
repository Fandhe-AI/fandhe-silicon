# テクスチャの形式の問合せの契約テスト（TASK-1314）の所在と本リポの現状

TASK-1314 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1314。項目のポインタ COVT-1。ポインタのみ）。
内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/core/src/texture_format_query_check.rs` |
| 公開のパス | `fandhe_silicon_core::texture_format_query_check` |
| 公開の条件 | `crates/core/src/lib.rs` で feature `test-support` の裏 |

## 現状の形（1/4）

能力の問合せの口（`Capabilities` の実装。`&dyn Device` を含む）へ鍵の列を流し、戻り値を検査する試験用の関数。鍵の一覧は呼ぶ側が渡す（一覧の所在は `texture_format_key` モジュール）。

違反の型 `TextureFormatQueryViolation`（`#[non_exhaustive]`）

| variant | 持つ値 | 出る場合 |
| ------- | ------ | -------- |
| `Rule` | `CapabilityRuleViolation` | `check_query` が違反を返した |
| `LevelMismatch` | `feature`・`level`・`query` | `query` の段階と `query_ex` の段階が違う |
| `RowlessKeyMismatch` | `feature`・`level`・`query` | 行の無い鍵で、`query_ex` が `UNLISTED_FEATURE_QUERY` でない、または `query` が `Unsupported` でない |

関数

| 名前 | 形 | 返すもの |
| ---- | -- | -------- |
| `check_texture_format_queries` | `(device, keys)` | 全違反（鍵の順。鍵ごとに `Rule` → `LevelMismatch`）。無ければ `Ok(())` |
| `check_rowless_texture_format_keys` | `(device)` | 試しの鍵ごとに最大 1 件の `RowlessKeyMismatch`。無ければ `Ok(())` |
| `texture_format_keys_without_rows` | `(device, keys)` | `query_ex` の理由が `UnknownFeature` の鍵（`keys` の順） |

入力ごとの返し

| 入力 | 返し |
| ---- | ---- |
| 規則に適う行の表 | 3 つの関数が `Ok`・`Ok`・空 |
| 壊れた行（理由なしの `Unsupported` など） | `Rule` を鍵の順に全部集める |
| `query` と `query_ex` の段階が違う | `LevelMismatch` |
| 行の無い値（`UnknownFeature`） | `check_texture_format_queries` では違反にしない |
| 空の `keys` | `Ok(())`・空 |

試しの鍵は 5 件で、いずれも一覧（`TEXTURE_FORMAT_QUERY_KEYS`）に無い鍵。device の panic はそのまま呼び出し元へ伝わる。

対応するテスト（記録時点）: core の中の試験 6 件、doctest 3 件。

確かめ方

```text
cargo test -p fandhe-silicon-core --features test-support --lib texture_format_query_check
cargo test -p fandhe-silicon-core --features test-support --doc texture_format_query_check
```

## 試験用の device の形式の鍵の行（2/4）

試験用の device（`StubDevice`。TASK-156 のポインタ）の `query_ex` を、形式の問合せの鍵の行を持つ表の表引きにした。

| 項目 | 内容 |
| ---- | ---- |
| 表の所在 | `crates/core/src/stub_device.rs` の非公開の `static`（行の型は `capability_table::FeatureRow`、TASK-1114 のポインタ） |
| 鍵 | `texture_format_key::TEXTURE_FORMAT_QUERY_KEYS`（TASK-1313 のポインタ）の並びのまま |

`query_ex` の返し

| 鍵 | 返し |
| -- | ---- |
| 形式の問合せの鍵 | `Unsupported`・理由 `NotSupported`・根拠なし |
| WebGPU の口の 2 鍵 | `WEBGPU_PORT_UNSUPPORTED` |
| それ以外 | `UNLISTED_FEATURE_QUERY` |

`query` は変わらずどの鍵にも `Unsupported` を返し、`limits` も変わらない。

対応するテスト: `stub_device` の中の試験（`texture_format_table_follows_query_key_list`・`texture_format_table_keys_and_queries_pass_shared_checks`・`query_ex_returns_row_value_for_texture_format_keys`・`query_ex_returns_unlisted_value_for_keys_without_rows`・`query_level_matches_query_ex_level`・`webgpu_port_keys_are_not_texture_format_keys`）と、`tests/stub_device_request.rs` の `query_ex_returns_unlisted_value_for_keys_without_rows`。

確かめ方

```text
cargo test -p fandhe-silicon-core --features test-support --lib stub_device
cargo test -p fandhe-silicon-core --features test-support --test stub_device_request
```

## 契約テスト（3/4）

外の crate の統合テスト `crates/core/tests/texture_format_query_contract.rs`（公開のパスだけを使う。feature `test-support` と `fault-injection` が要る）。スレッド・ファイル・環境変数は使わず、設定は `RunSetting::from_lookup` へ渡す。

| 試験 | 入力 | 確かめること |
| ---- | ---- | ------------ |
| `stub_passes_texture_format_query_checks` | 背後実装 4 値 × プロファイル 3 値の試験用の device（`&dyn Device`） | 3 つの関数が `Ok`・`Ok`・空（12 組） |
| `stub_returns_row_values_for_query_keys` | 一覧の 118 鍵 | `query` が `Unsupported`、`query_ex` が `Unsupported`・`NotSupported`・根拠なしで `UNLISTED_FEATURE_QUERY` と違う |
| `astc_row_key_returns_unlisted_value` | `texformat:Astc` | `query_ex` が `UNLISTED_FEATURE_QUERY`、`query` が `Unsupported`。一覧に無い |
| `failed_acquisition_s0_differs_from_stub_unsupported` | `FaultInjecting` + `CapabilityQueryFails` の試験用の device | 118 鍵で `query` と `query_ex` の段階が `S0`、理由・根拠なし、素の device の `query` と違う、`check_texture_format_queries` が `Ok` |
| `checks_accept_key_subsets` | 一覧の先頭 15 件だけ行を持つ表と試験用の device | 部分の列で `Ok`・行の無い鍵が空、全体の一覧では残り 103 件が順に返る |
| `broken_implementations_are_reported` | 壊れた行を 1 件ずつ入れた表と、段階が食い違う実装 | `Rule` の 3 種・`LevelMismatch`・`RowlessKeyMismatch`・行を外した鍵の検出、複数の違反が鍵の順に集まる |
| `panic_in_query_reaches_caller` | 特定の鍵で panic する実装 | panic が呼び出し元へ伝わる |
| `checks_compose_with_test_frame_judge` | 試験の枠の `judge` の本体で 3 つの関数をかける | 試験用の device で `Label::Ok`、壊れた表で `Label::Ng`、未設定で本体を呼ばず `NotRun` |

対応するテスト（記録時点）: 8 本。

確かめ方

```text
cargo test -p fandhe-silicon-core --features test-support,fault-injection --test texture_format_query_contract
```

流れる場所: `make verify`（`--all-features` の clippy・test）と `make wasm-verify`（`crates/core` の試験の wasm32 の型検査）。

## 完了の条件の確かめ（4/4）

4/4（#6720）で、本リポで観測できる事実だけを記録した（ポインタ: `docs/spec/05-tasks.md` の TASK-1314）。GPU の実機なしで実行し、件数は実測。コードと試験は変えていない。

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 検査の関数の module が feature `test-support` の裏にある | `crates/core/src/lib.rs:65` の `pub mod texture_format_query_check;`（直前に `#[cfg(feature = "test-support")]`）、`cargo check -p fandhe-silicon-core --features test-support` | 終了コード 0 |
| (2) | 一覧の 118 鍵で試験用の device が `CapabilityQuery` を返し、規則と段階の一致の検査に通る | `cargo test -p fandhe-silicon-core --all-features --test texture_format_query_contract` の `stub_passes_texture_format_query_checks`・`stub_returns_row_values_for_query_keys` | 2 件とも ok |
| (3) | 取得失敗の `S0` と `Unsupported` が別の値 | 同上の `failed_acquisition_s0_differs_from_stub_unsupported` | ok |
| (4) | 壊れた値の検出 | `cargo test -p fandhe-silicon-core --features test-support --lib -- texture_format_query_check`、同 `--doc -- texture_format_query_check`、契約テストの `broken_implementations_are_reported` | `--lib` 6 件・`--doc` 3 件 passed、契約テストの 1 件 ok |
| (5) | `texformat:Astc` と一覧に無い鍵が行の無い値 | `astc_row_key_returns_unlisted_value`、(2) の `stub_passes_texture_format_query_checks` の中の `check_rowless_texture_format_keys` | ok |
| (6) | 試験用の device で 118 鍵が行の無い値に落ちない | `stub_passes_texture_format_query_checks` の `texture_format_keys_without_rows`、`cargo test -p fandhe-silicon-core --features test-support --lib -- stub_device` | 空の結果で ok、`--lib` 11 件 passed |
| (7) | 実装を差し替えて流せる | `&dyn Device` を渡す `stub_passes_texture_format_query_checks`、部分の列の `checks_accept_key_subsets`、試験の枠（TASK-23 のポインタ）の `judge` に載せる `checks_compose_with_test_frame_judge` | 3 件とも ok |
| (8) | 試験用の device での契約テストの記録 | `cargo test -p fandhe-silicon-core --all-features --test texture_format_query_contract` | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`（base `dc1228d`） |
| (9) | 公開 API の検査 | `make public-api-check`（TASK-10 のポインタ）・`make public-api-sdk-check`（TASK-13 のポインタ）・`make public-name-check`・`make public-api-webgpu-check`・`make core-deps-check` | すべて終了コード 0。core の行は `wgpu_type_refs=0 wgpu_other_refs=0`・`sdk_type_refs=0 sdk_other_refs=0`・`name_hits=0`（production・all-features とも）。集計は `webgpu_names_total=0`・`unexpected=0 missing=0` |
| (10) | CI で流れる場所 | 上の 3/4 の節の表。`gh run list --branch main --workflow ci.yml` | 1/4 のマージ `db4c842` は run 41370940336 success、2/4 の `ac8cb31` は run 41372460298 success、3/4 の `dc1228d` は run 41374030020 success |
| (11) | 依存 0・unsafe の追加 0 | `7bbc838` から `dc1228d` までの `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` の差分、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n forbid crates/core/src/lib.rs`、`grep -c unsafe crates/core/src/texture_format_query_check.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1` | `Cargo.lock`・`deps-approved.toml` は差分なし、`crates/core/Cargo.toml` は `[[test]] texture_format_query_contract`（`required-features = ["test-support", "fault-injection"]`）の 4 行の追加、承認の記録 4 件のまま、`:35` に `#![forbid(unsafe_code)]`、`unsafe` は 0 件、core の外部依存は 0 件 |
| (12) | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1314）。確かめた base は `dc1228d54be1b56045e9ac93848f13cb5a31b8ee`。

### 再実行の契機

- `crates/core/src/texture_format_query_check.rs`・`crates/core/src/stub_device.rs`・`crates/core/tests/texture_format_query_contract.rs`・`crates/core/tests/stub_device_request.rs` を変えたとき。
- `TEXTURE_FORMAT_QUERY_KEYS` の鍵が変わったとき。
- 公開 API の検査（`scripts/public-api-check.sh` ほか）を変えたとき。

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #6717 | 形式の問合せの鍵を流す検査の関数 |
| 2/4 | #6718 | 試験用の device の形式の鍵の行 |
| 3/4 | #6719 | 契約テストと陽性対照 |
| 4/4 | #6720 | 確かめ |

関連: 形式ごとの能力の値の形式の単位の一致の検査は [texture-format-features.md](texture-format-features.md)。
