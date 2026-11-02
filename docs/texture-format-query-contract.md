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

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #6717 | 形式の問合せの鍵を流す検査の関数 |
| 2/4 | #6718 | 試験用の device の形式の鍵の行 |
| 3/4 | #6719 | 契約テストと陽性対照 |
| 4/4 | #6720 | 確かめ |
