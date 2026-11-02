# 形式ごとの能力の値の型と問合せ（TASK-2566）の所在と本リポの現状

TASK-2566 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本書の事実はすべて、本リポのコードと下の「確かめ方」のコマンドの出力から書いている。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2566。ポインタのみ）。
内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/core/src/texture_format_features.rs` |
| 公開のパス | `fandhe_silicon_core::texture_format_features` |
| 公開の条件 | feature の条件なし（`crates/core/src/lib.rs`） |

| 名前 | 種類 | 役割 |
| ---- | ---- | ---- |
| `TextureFormatFeature` | 列挙（`#[non_exhaustive]`） | 形式ごとの能力の 18 の要素 |
| `TextureFormatFeature::ALL` | 定数 | 18 要素を宣言の順に並べた一覧 |
| `TextureFormatFeatures` | 構造体 | 形式の単位の値と要素ごとの値（欄は非公開） |
| `TextureFormatFeatures::new` | 関数 | 規則に照らして値を組み立てる。外れる入力は `ContractError::InvalidArgument` |
| `TextureFormatFeatures::capture_failed` | `const fn` | 形式の単位が `S0` で要素を持たない値 |
| `TextureFormatFeatures::format_query` | 読み出し | 形式の単位の値 |
| `TextureFormatFeatures::feature` | 読み出し | 要素の値 |
| `TextureFormatFeatures::allowed_usages` | 読み出し | 使える段階の用途の要素に対応する `TextureUsage` の和 |
| `TextureFormatFeatures::sample_count_supported` | 読み出し | サンプル数が使えるか |
| `TextureFormatFeatures::supported_sample_counts` | 読み出し | 使えるサンプル数の昇順の列 |

値の型は `CapabilityQuery`（TASK-136）、用途は `TextureUsage`（TASK-371）、形式の型は `TextureFormat`（TASK-1312）。

## 型と組み立ての規則（1/4）

要素（`TextureFormatFeature`。宣言の順）

| 順 | 名前 | 意味 |
| -- | ---- | ---- |
| 0 | `Filterable` | フィルタ可 |
| 1 | `Blendable` | ブレンド可 |
| 2〜5 | `Multisample2`・`Multisample4`・`Multisample8`・`Multisample16` | サンプル数 2・4・8・16 |
| 6 | `MultisampleResolve` | マルチサンプルの解決 |
| 7〜10 | `StorageReadOnly`・`StorageWriteOnly`・`StorageReadWrite`・`StorageAtomic` | storage の読み取り専用・書き込み専用・読み書き・atomic |
| 11〜17 | `UsageCopySrc`・`UsageCopyDst`・`UsageTextureBinding`・`UsageStorageBinding`・`UsageRenderAttachment`・`UsageTransientAttachment`・`UsageStorageAtomic` | 用途 `COPY_SRC`〜`STORAGE_ATOMIC` |

`new` の照らす順（最初に当たった 1 件を `InvalidArgument` で返す。panic しない）

| 群 | 拒否する入力 |
| -- | ------------ |
| 1 形式の単位 | `S0` が理由か根拠を持つ／`Unsupported` が理由を持たない／`S1`〜`S6` が理由を持つ／`S3`〜`S6` が根拠を持たない |
| 2 形式と要素 | 形式の単位が `S0` か `Unsupported` で要素の列が空でない |
| 3 要素 | 段階が `S0`／`Unsupported` で理由が無い／`Unsupported` でないのに理由がある／`S3`〜`S6` で根拠が無い／同じ要素が 2 回目に出る |
| 4 用途と旗 | `UsageStorageBinding` が `S1`〜`S6` でないのに storage の 4 つの旗のどれかが `S1`〜`S6`／`UsageStorageAtomic` が `S1`〜`S6` なのに `StorageAtomic` が `S1`〜`S6` でない |

「使える段階」は `S1`〜`S6`。`S0`・`Unsupported`・値の無い要素は使える段階でない。群 4 は上の 2 方向だけを照らす。

読み出し

| 関数 | 返し |
| ---- | ---- |
| `feature(f)` | 形式の単位が `S0` か `Unsupported` なら形式の単位の値（理由・根拠ごと）。そうでなく値があればその値。値が無ければ `UNLISTED_FEATURE_QUERY` |
| `allowed_usages()` | 7 つの用途の要素のうち `feature(..)` が使える段階のものの `TextureUsage` の和 |
| `sample_count_supported(n)` | 1 は常に `true`。2・4・8・16 は対応する要素が使える段階のとき `true`。ほか（0・3・32 など）は `false` |
| `supported_sample_counts()` | `[1, 2, 4, 8, 16]` のうち `sample_count_supported` が `true` のものを昇順に（1 を含む） |
| `capture_failed()` | 形式の単位が `S0`（理由・根拠なし）の値。`feature` は全要素で形式の単位の値を返す |

## 対応するテスト

`crates/core/src/texture_format_features.rs` の `mod tests`（10 本）

| 名前 | 確かめること |
| ---- | ------------ |
| `all_has_eighteen_distinct` | `ALL` が 18 値で互いに違う |
| `index_is_bijection` | 添字が 18 値で `0..18` への全単射 |
| `format_query_rules` | 群 1 の各行 |
| `unit_without_value_rejects_features` | 群 2 |
| `feature_entry_rules` | 群 3 の各行 |
| `usage_flag_pairs` | 群 4 の 2 方向と、通る組 |
| `capture_failed_value` | `capture_failed()` の値 |
| `feature_reads` | `feature` の 3 通り |
| `allowed_usages_map_each_bit` | 用途の 7 要素と `TextureUsage` の 7 ビットの対応 |
| `sample_counts` | `sample_count_supported`・`supported_sample_counts` |

doctest（`TextureFormatFeatures`）: 値の組み立てと読み出し、規則に外れる入力の拒否。

## 確かめ方

```sh
cargo test -p fandhe-silicon-core --lib texture_format_features
cargo test -p fandhe-silicon-core --doc TextureFormatFeatures
grep -n 'pub mod texture_format_features' crates/core/src/lib.rs
```

## 能力の問合せの口の操作（2/4）

`Capabilities` に既定の実装つきの操作 `fn texture_format_features(&self, format: TextureFormat) -> TextureFormatFeatures` がある（`crates/core/src/capability.rs`）。
ジェネリクスを持たないので `&dyn Capabilities`・`&dyn Device` から呼べる。

既定の実装は、形式からその形式の `texformat:` の鍵を引き（`TEXTURE_FORMAT_ROWS` を先頭から探す内部の補助。公開しない）、`query_ex` を 1 回だけ呼ぶ（`query` は呼ばない）。得た値を形式の単位にして、要素を持たない値を組み立てる。

| `query_ex` の値 | 返る値 |
| --------------- | ------ |
| 共通の規則に合う値（`S1`〜`S6`、理由つきの `Unsupported`） | 形式の単位がその値で、要素なし |
| `S0`（理由・根拠なし） | 形式の単位が `S0` |
| 共通の規則に外れる値 | `TextureFormatFeatures::capture_failed()` と等しい値 |

`test-support` の裏の `texture_format_query_check::check_texture_format_features(device, rows)` は、`rows` の各鍵で `texture_format_features(format).format_query()` と `query_ex(key)` を 1 回ずつ呼ぶ。

| 入力 | 返り値 |
| ---- | ------ |
| 全鍵で一致 | `Ok(())` |
| 食い違う鍵がある | 鍵の順に全部集めた `FeaturesFormatQueryMismatch` の `Err` |
| 空の `rows` | `Ok(())` |

2/4 で足した試験: `texture_format_features::tests::default_op` の 5 本（`format_key_covers_all_formats_in_key_order`・`calls_query_ex_once_and_query_never`・`valid_values_become_the_format_query_without_features`・`s0_value_stays_s0`・`rule_breaking_values_become_capture_failed`）、`texture_format_query_check::tests` の `features_check_accepts_empty_rows`・`features_check_collects_all_mismatches_in_key_order`、doctest（`Capabilities::texture_format_features`・`check_texture_format_features`）。

```sh
cargo test -p fandhe-silicon-core --lib texture_format_features
cargo test -p fandhe-silicon-core --features test-support --lib texture_format_query_check
cargo test -p fandhe-silicon-core --features test-support --doc texture_format_features
cargo test -p fandhe-silicon-core --features test-support --doc check_texture_format_features
grep -n 'fn texture_format_features' crates/core/src/capability.rs
```

## 形式の鍵ごとの値と非対応の device の試験（3/4）

外の crate の試験が公開のパスだけで確かめる。

| ファイル | feature の条件 |
| -------- | -------------- |
| `crates/core/tests/texture_format_features_values.rs` | なし |
| `crates/core/tests/texture_format_features_contract.rs` | `test-support`・`fault-injection` |

`texture_format_features_values.rs`（試験の中だけの `Capabilities` の実装を使う）

| 名前 | 入力 | 期待 |
| ---- | ---- | ---- |
| `feature_kinds_are_eighteen` | `TextureFormatFeature::ALL` | 18 値で互いに違う |
| `builder_rules_reject_each_violation` | `new` の 4 群それぞれの代表の入力 | `InvalidArgument` だけで、同じ引数で同じ結果。通る入力は `Ok` |
| `every_texture_format_key_returns_a_value` | 全形式の鍵が `S2`・`RuntimeApi` の実装（`&dyn Capabilities` 越しを含む）。1 つの鍵だけ `Unsupported`・`NotSupported` にした実装 | 118 形式の全部で形式の単位が `query_ex(鍵)` と等しく、18 要素が行の無い値、用途が空、サンプル数が `[1]`。1 つの鍵だけ変えると、その形式だけ形式の単位と 18 要素が変わる |
| `format_values_do_not_depend_on_adapter_specific_feature_row` | `FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` の `query_ex` だけが違う 2 つの実装（上書きあり・なし） | 118 形式の全部で戻りが等しい。上書きした形式では `Unsupported` の要素が使える段階に変わらない |
| `overridden_values_follow_read_rules` | 1 形式を上書きした実装 | サンプル数 1・2・4・8・16・3・32 の可否、`supported_sample_counts`、`allowed_usages`。形式の単位が `Unsupported` の形式は全要素がその値 |

`texture_format_features_contract.rs`

| 名前 | 入力 | 期待 |
| ---- | ---- | ---- |
| `stub_device_returns_value_for_every_key` | 試験用の device（背後実装 4 値 × プロファイル 3 値の 12 組。`&dyn Device`） | `check_texture_format_features` が `Ok`。118 形式の全部で形式の単位・18 要素が `Unsupported`・`NotSupported`・根拠なし。上の鍵の `query_ex` が行の無い値（`Unsupported`）でも値が返る |
| `checker_reports_mismatch` | 1 形式だけ `capture_failed()` を返す実装 | `FeaturesFormatQueryMismatch` が 1 件で、その形式 |
| `decorator_keeps_format_query_consistent` | `FaultInjecting` で包んだ試験用の device と表。筋書き `PassThrough`・`CapabilityQueryFails`・`CapabilityInsufficient` | どれも `check_texture_format_features` が `Ok`。`PassThrough` は包まない値と等しい。`CapabilityQueryFails` は全形式の形式の単位・要素が `S0`。`CapabilityInsufficient` は指定した形式だけ `Unsupported`・`NotSupported` で、ほかの 117 形式は変わらない |

```sh
cargo test -p fandhe-silicon-core --test texture_format_features_values
cargo test -p fandhe-silicon-core --features test-support,fault-injection --test texture_format_features_contract
```

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #7726 | 形式ごとの能力の値の型と組み立ての規則 |
| 2/4 | #7727 | 能力の問合せの口に形式ごとの能力の値の操作を足す |
| 3/4 | #7728 | 形式の鍵ごとの値と非対応の device の試験 |
| 4/4 | #7729 | 完了の条件の確かめ |
