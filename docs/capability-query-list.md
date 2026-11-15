# 能力の一覧の問合せと Metal の背後実装の一覧・形式単位・表示先の鍵・上限（TASK-179）の所在と本リポの現状

TASK-179 の作業のうち、共通の crate の鍵の一覧の識別と一覧の問合せの所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。書式は [metal-capability-query](metal-capability-query.md)（TASK-173）に合わせる。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-179。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 一覧の識別 | `fandhe_silicon_core::capability::KeyList`（4 値・`keys`） | `grep -n 'enum KeyList\|pub fn keys' crates/core/src/capability.rs` |
| 一覧の問合せ | `Capabilities::query_list`（既定の実装つき） | `grep -n 'fn query_list' crates/core/src/capability.rs` |
| 表示先の鍵 | `fandhe_silicon_core::feature_key::FEATURE_SURFACE` | `grep -n 'FEATURE_SURFACE' crates/core/src/feature_key.rs` |
| 試験 | `crates/core/tests/capability_query_list.rs`。device に触らない | `cargo test -p fandhe-silicon-core --all-features --test capability_query_list` |
| cfg | 条件なし。故障の差し込みの包み越しの試験は feature `fault-injection` の裏 | `crates/core/tests/capability_query_list.rs` |

## 鍵の一覧の識別と一覧の問合せ（1/4）

`KeyList` は 4 値で、`keys` が指す一覧と鍵の数は次のとおり。

| 値 | 一覧 | 鍵の数 |
| -- | ---- | ------ |
| `Features` | `FEATURE_QUERY_KEYS` | 79 |
| `Downlevel` | `DOWNLEVEL_QUERY_KEYS` | 26 |
| `Limits` | `LIMIT_QUERY_KEYS` | 57 |
| `TextureFormats` | `TEXTURE_FORMAT_QUERY_KEYS` | 118 |

`Capabilities::query_list(list)` は、`list.keys()` の順に `(鍵, query_ex の値)` の組を返す。

- 既定の実装は一覧の鍵ごとに `query_ex` を 1 回ずつ呼ぶ。返す組の数は一覧の長さと同じ
- `S0` の鍵と `Unsupported` の鍵も組に含める
- `dyn Capabilities` からも呼べる
- `FaultInjecting` で包んだ値は上書きしないので、各組は包みの `query_ex` の値と一致する

`FEATURE_SURFACE`（`feature:SURFACE`）は 4 つの一覧のどれにも含めない。`FEATURE_QUERY_KEYS` は 79 件のまま。

確かめ方:

```text
cargo test -p fandhe-silicon-core --all-features --test capability_query_list
```

`test result:` の行が 5 passed になる。`--all-features` を付けないと故障の差し込みの包み越しの 1 本が外れ、4 passed になる。
`KeyList::keys` の doctest は `cargo test -p fandhe-silicon-core --doc capability` で流れる。

## Metal の表示先の鍵の行（2/4）

`metal_capability_of` は `feature:SURFACE`（`FEATURE_SURFACE`）に、どの捕捉の値でも次の値を返す。`MetalDevice` の `query`・`query_ex` はこの関数を呼ぶ。

| 項目 | 値 |
| ---- | -- |
| 段 | `Unsupported` |
| 理由 | `Some(UnsupportedReason::NotSupported)` |
| 根拠 | `None` |

- 捕捉の失敗・Apple の family なし・family 1・6・9 のどれでも同じ値になる
- 値は 8 行の表 `METAL_FEATURE_TABLE` とは別の 1 行の表 `METAL_SURFACE_TABLE` が返す。`metal_capability_of` は 8 行の表に行があればその表を、なければ 1 行の表を引く
- 8 行の表の行と `FEATURE_QUERY_KEYS` の 79 鍵の値は変わらない。どちらの表にも無い鍵は `UNLISTED_FEATURE_QUERY` を返す

確かめ方:

```text
cargo test -p fandhe-silicon-contract --all-features --lib backend_metal::capability_tests
cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::capability_tests
```

`test result:` の行は、1 本目が 8 passed、2 本目が 6 passed になる（共通の規則の検査 2 本は feature `test-support` の裏）。

## device の要る試験（3/4）

`crates/contract/tests/backend_metal_capability_list.rs`（`required-features` は `backend-metal` と `test-support`。`#![cfg(target_os = "macos")]`）に置く。
どの試験も `#[ignore = "touches the Metal device; run only in a dedicated device session"]` 付きで、既定では流れない。

| 試験 | 確かめること |
| ---- | ------------ |
| `query_list_matches_query_ex_for_every_list` | `KeyList` の 4 値それぞれで、`query_list` の長さと各要素の鍵が `keys` と同じ並びで、各要素の値が `query_ex` と等しく、`check_query_ex` が通り、段階が `S0` の鍵が 0 件。`list:` の行（鍵の数・`S0` の数・`Unsupported` の数）を標準出力に出す |
| `texture_format_unit_matches_the_format_key` | `TextureFormat::all()` と `TEXTURE_FORMAT_QUERY_KEYS` の長さが等しく、同じ位置の組ごとに `texture_format_features(format).format_query()` が `query_ex(key)` と等しい |
| `surface_key_is_not_supported_on_a_requested_device` | `ComputeOnly`（feature `graphics` があれば `Graphics` も）の device で、`FEATURE_SURFACE` が `Unsupported`・`NotSupported`・根拠なし。`query` の段階が `Unsupported`。`surface:` の行を標準出力に出す |
| `limits_match_the_native_values_and_print_them` | `limits()` の `max_buffer_bytes` が Metal の `maxBufferLength` と等しく、`max_dispatch_dims` が `[u32::MAX; 3]`、`max_texture_size` が 16384。`limits:` の行を標準出力に出す |

確かめ方（device を使わない。macOS 以外では 0 件）: `cargo test -p fandhe-silicon-contract --all-features --test backend_metal_capability_list`

```text
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 鍵の一覧の識別と一覧の問合せ | #3788 |
| 2/4 | Metal の表示先の鍵の行 | #3789 |
| 3/4 | device の要る試験 | #3790 |
