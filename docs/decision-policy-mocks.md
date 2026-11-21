# 決定表の出力を受ける 2 つの利用コードの模擬（TASK-219）の所在と本リポの現状

TASK-219 の作業のうち、共通の crate の決定表 `decide` の戻り値を受ける 2 つの利用コードの模擬（fail-fast と縮退）と、それを照らす試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-219。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 決定表 | `crates/core/src/decision.rs` の `decide` | [decision-table](decision-table.md) |
| 利用コードの模擬 | `crates/contract/tests/policy_mocks/mod.rs`（試験のファイルが `mod policy_mocks;` で取り込む。このファイル自体は試験のバイナリにならない） | 同ファイル |
| device に触らない試験 | `crates/contract/tests/policy_mocks_decision.rs` | 下の「利用コードの模擬と device に触らない試験（1/3）」 |

## 利用コードの模擬と device に触らない試験（1/3）

2 つの模擬は `decide` の戻り値の `Decision` だけを受ける。決定表に渡す入力は、能力の段階のほかは規模 `Large`・常駐位置 `Unified`・数値型 `F32` に固定する（`decision_for`）。

| 口 | 動き |
| -- | ---- |
| `decision_for(level)` | `DecisionInput::new(level, Large, Unified, F32)` で `decide` を 1 回呼ぶ |
| `route_or_fail(decision)`（fail-fast） | `Route(候補)` なら `Ok(候補)`、それ以外は `Err(FailFastError::NoRoute(decision))` |
| `request_or_fail::<P>(req)`（fail-fast） | `P::request` を 1 回呼び、失敗 `e` は `Err(FailFastError::Request(e))` |
| `Degrading::slot(feature, decision)`（縮退） | `Route(候補)` なら `PrimarySlot::Route(候補)`。それ以外は `DegradeEvent { feature, decision }` を 1 つ残して `PrimarySlot::Unavailable`。経路を上書きする引数は無い |

| 試験 | 入力 | 照らす値 |
| ---- | ---- | -------- |
| `fail_fast_routes_or_fails_for_every_level` | 段階の 8 つの値 | `decision_for` の戻り値が下の表の値。`S5`・`S6` は `Ok(GpuGeneric)`、ほかの 6 つは `Err(NoRoute(<その段階の値>))` |
| `degrading_falls_back_with_a_reason_for_every_level` | 同上 | `S5`・`S6` は `Route(GpuGeneric)`、ほかの 6 つは `Unavailable`。イベントは 6 つで、残った順と値が下の表の候補なしの 6 行と同じ |
| `both_policies_take_the_same_decision_values` | 同上 | 同じ `Decision` を 2 つの模擬に渡すと、候補は同じ値になり、候補なしは両方とも候補なしで、イベントの `decision` は渡した値 |
| `the_decision_source_names_no_product` | `crates/core/src/decision.rs` のソース | `fandhe-ai`・`fandhe_ai`・`fandhe-db`・`fandhe_db`・`vector-db`・`vector_db`・`fandhe-3d`・`fandhe_3d` を含まない（大小文字を区別しない） |
| `fail_fast_returns_a_request_failure_as_is` | `DeviceUnavailable("synthetic")` を返す、試験のファイルの中の provider | `Err(Request(DeviceUnavailable("synthetic")))`。provider の `request` の呼び出しは 1 回 |

段階ごとの `decision_for` の戻り値。

| 段階 | `output` | `reason` |
| ---- | -------- | -------- |
| `S0` | `NoRoute(Unknown)` | `CapabilityS0` |
| `Unsupported` | `NoRoute(Unsupported)` | `CapabilityUnsupported` |
| `S1`〜`S4` | `NoRoute(NotVerified)` | `CapabilityBelowS5` |
| `S5` | `Route(GpuGeneric)` | `CapabilityS5ScaleLarge` |
| `S6` | `Route(GpuGeneric)` | `CapabilityS6ScaleLargeOtherScalar` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --test policy_mocks_decision
```

5 件が流れる。背後実装の feature は要らない。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 決定表の出力を受ける 2 つの利用コードの模擬と試験を足す | #3896 |
