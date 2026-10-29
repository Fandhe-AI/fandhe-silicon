# 決定表の純関数と型（TASK-141）の所在と現状の事実

本リポのコードと公開テストから説明できる振る舞いを記録する。記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-141。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。置き場は TASK-132 の決定の記録に従う（ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/decision.rs`（`decision` モジュール） | `crates/core/src/lib.rs` |
| feature | 条件なし（feature の裏ではない） | `crates/core/Cargo.toml` |
| 依存 | 外部依存 0。`unsafe` を使わない（`core` は `#![forbid(unsafe_code)]`）。`capability` モジュールの `CapabilityLevel` だけを参照する | `crates/core/src/decision.rs` |
| 引数と戻り値 | 引数は `DecisionInput` 1 つ、戻り値は `Decision` 1 つ。全 144 通りで同じ入力に同じ値が返る | `crates/core/src/decision/tests.rs` |

## 公開名

| 名前 | 内容 |
| ---- | ---- |
| `decide(DecisionInput) -> Decision` | 純関数 |
| `DecisionInput` | 公開フィールドは `capability`・`scale`・`residency`・`scalar` の 4 つ。`#[non_exhaustive]` で、他の crate からは `DecisionInput::new` で作る |
| `Decision` | `output: RouteOutput` と `reason: DecisionReason` |
| `RouteOutput` | `Route(RouteCandidate)` と `NoRoute(NoRouteReason)`。`#[non_exhaustive]` |
| `RouteCandidate` | `GpuGeneric`・`GpuMatrixUnit`。`#[non_exhaustive]` |
| `NoRouteReason` | `Unknown`・`Unsupported`・`NotVerified`。`#[non_exhaustive]` |
| `DecisionReason` | 出力になった入力の値を表す。常駐位置を表す値は持たない。`#[non_exhaustive]` |
| `DataScale` / `Residency` / `ScalarKind` | 入力の区分（それぞれ 2 値・3 値・3 値）。いずれも `#[non_exhaustive]` |

出力は常駐位置で変わらない。データ規模の区分に実際のデータ量を当てる操作は持たない。CPU の経路を選んで返す値は持たない。

## 対応するテスト

| 振る舞い | テスト |
| -------- | ------ |
| 試験の段階の一覧が `CapabilityLevel` の全値と一致する（値が増えるとコンパイルで落ちる） | `crates/core/src/decision/tests.rs` の `levels_list_covers_every_variant_in_order` |
| 8 × 2 × 3 × 3 の 144 通りが重複なく並ぶ | 同 `input_space_has_144_distinct_combinations` |
| 全組み合わせで組み合わせごとの期待と一致し、同じ入力を 2 回呼んで同じ値が返る | 同 `every_combination_matches_expected_and_returns_same_value_twice` |
| 常駐位置だけを変えても出力・理由が変わらない | 同 `residency_does_not_change_the_decision` |
| `S0` が `NoRoute(Unknown)`、`Unsupported` が `NoRoute(Unsupported)` | 同 `s0_is_no_route_unknown_for_every_other_input`・`unsupported_is_no_route_unsupported_for_every_other_input` |
| 知らない段階が `NoRoute(Unknown)`（同じ crate の中では `CapabilityLevel` の値を作れないため、内側の関数に `None` を渡して模す） | 同 `unrecognized_level_is_no_route_unknown` |
| 期待の表に依らない性質: S5 に届かない段階は常に経路なし、小さい規模は常に経路なし、行列の演算器の候補は S6・大・F16 のときだけ、経路が出るのは S5 か S6 で大のときだけ | 同 `levels_below_s5_never_route`・`small_scale_never_routes`・`matrix_unit_route_only_for_s6_large_f16`・`any_route_only_for_s5_or_s6_large` |
| 他の crate から `DecisionInput::new` で作った入力を `decide` が受ける | `crates/core/tests/decision_table.rs` の `input_built_with_new_is_accepted_by_decide` |
| 経路の候補の名前にチップ・製品・カーネルの固有の語が入らない | 同 `route_candidate_names_carry_no_product_specific_words` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --test decision_table
cargo test -p fandhe-silicon-core --lib decision
```

`make verify` の `cargo test` でも走る。GPU の実機は要らない。
