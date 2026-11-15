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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 鍵の一覧の識別と一覧の問合せ | #3788 |
