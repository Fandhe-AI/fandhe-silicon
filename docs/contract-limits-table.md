# 能力の問合せの上限の表（TASK-157）の所在と本リポの現状

TASK-157 の作業の記録の所在と、本リポのコードと公開テストが示す事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-157。関連: DEV-110・DEV-114・DEV-115。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

| 物 | 置き場 |
| -- | ------ |
| `LimitsTable`・`Capabilities::limits()` | `crates/core/src/capability.rs` |
| `FaultInjecting<D>` の `limits()` | `crates/core/src/fault/decorator.rs`（`fault-injection` の裏） |
| `CapabilityTable` の上限の表 | `crates/core/src/capability_rule.rs`（`test-support` の裏） |

関連する文書: [contract-device-types](contract-device-types.md)・[contract-fault-injection](contract-fault-injection.md)・[capability-query-rules](capability-query-rules.md)

## 本リポの現状の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 型 | `LimitsTable` は公開フィールド 3 つ（`max_buffer_bytes: u64`・`max_dispatch_dims: [u32; 3]`・`max_texture_size: u32`）。`Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` を持ち、`#[non_exhaustive]` でも `Default` でもない | `crates/core/tests/capability_limits_table.rs` |
| トレイト | `Capabilities` に必須の `fn limits(&self) -> &LimitsTable` がある（既定の実装なし）。`query`・`query_ex` は変えていない。オブジェクト安全で、`&dyn Capabilities` と `Box<dyn Capabilities + Send + Sync>` から呼べる | 同上 |
| 実装する側 | `Capabilities` の実装は `limits()` を実装する必要がある。この変更は破壊的 | `git grep -n 'Capabilities for' -- crates` |
| 故障の差し込み | `FaultInjecting<D>` の `limits()` は、`with_limits_override` の置き換えが `LimitsTable` ならその参照を、そうでなければ（置き換えなし・別の型）内側の `limits()` を返す。`FaultScript` の 6 種類すべてで確かめた（筋書きでは分岐しない） | `crates/core/tests/fault_decorator_limits.rs` |
| 別の型の置き換え | `limits()` には効かない。`limits_with::<LimitsTable>` は `Err(InvalidArgument)` のまま | 同上 |
| `with_limits_override`・`limits_with` | 型と振る舞いは変えていない | 同上 |
| `CapabilityTable` | `limits()` の初期値は 3 項目とも 0。`with_limits(t)` の後は `t` を返す。`new()` の引数は変えていない | `crates/core/tests/capability_rule_stub.rs` |
| 値の意味 | `crates/contract` に `fn limits` の実装は無い（0 件） | `git grep -n 'fn limits' -- crates/contract` |

## 現状の形

- 型は `crates/core` に置く（外部依存 0・`forbid(unsafe_code)` のまま）
- `limits()` の戻り値は `&LimitsTable` の 1 つの型で、実装ごとに別の表を返せる（試験では異なる表を返す 2 つの実装が、それぞれ自分の表を返すことを見る）

## 対応するテスト

| テスト | 確かめるもの |
| ------ | ------------ |
| `crates/core/tests/capability_limits_table.rs` | フィールドの集合と型、`Copy`・`PartialEq`・`Debug`、異なる表を返す 2 つの実装が各自の表を返すこと、`dyn` 経由の呼び出し |
| `crates/core/tests/fault_decorator_limits.rs`（`fault-injection`） | 置き換えなしは内側の表、`LimitsTable` の置き換えは `limits()` と `limits_with` の両方に効く（`FaultScript` の 6 種類すべての筋書きで同じ）、別の型の置き換えは `limits()` に効かず `limits_with` は `InvalidArgument` |
| `crates/core/tests/capability_rule_stub.rs`（`test-support`） | `CapabilityTable` の初期値 0 と `with_limits` |

## 確かめ方

```text
cargo test -p fandhe-silicon-core --test capability_limits_table
cargo test -p fandhe-silicon-core --features fault-injection --test fault_decorator_limits
cargo test -p fandhe-silicon-core --features test-support --test capability_rule_stub
```

## 完了の条件の確かめ

完了の条件の中身は private spec 側の記録に従う（ポインタのみ）。
