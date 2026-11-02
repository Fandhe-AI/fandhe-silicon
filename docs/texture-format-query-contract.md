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

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #6717 | 形式の問合せの鍵を流す検査の関数 |
| 2/4 | #6718 | 試験用の device の形式の鍵の行 |
| 3/4 | #6719 | 契約テストと陽性対照 |
| 4/4 | #6720 | 確かめ |
