# 失敗の種類を戻り値だけで区別する共通の契約テスト（TASK-272）の所在と本リポの現状

TASK-272 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-272。対象は ERR-20・ERR-21・ERR-58。ポインタのみ）。内容・理由・経緯は本書に置かない。
決定の記録は private spec 側の記録に従う（ポインタのみ）。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-266・TASK-554 | `crates/core/src/error.rs` の `FailureClass`・`ContractError::failure_class()`・`CompletionStateEx::failure_class()` | [validation-intake](validation-intake.md) |
| TASK-269 | `crates/core/src/error.rs` のエラー・完了状態の型 | [contract-error-types](contract-error-types.md) |
| TASK-271 | `crates/core/src/fault*`（`fault-injection` の裏） | [contract-fault-injection](contract-fault-injection.md) |

## 現状の形

コードと公開テストが示す事実だけを置く。差し込むのは模擬であって、実際の故障ではない。

- 試験は `crates/core/tests/failure_class_contract.rs` の 1 本（13 件）。`crates/core/Cargo.toml` の `[[test]]` で `required-features = ["fault-injection"]`。`src` は変えていない。外部依存・`unsafe` は足していない
- 値の出どころ（すべて戻り値で受ける）:

| 値 | 出どころ |
| -- | -------- |
| `DeviceUnavailable` | device に結び付かない合成の関数 |
| `CompletionStateEx::RuntimeError`・`CompletionState::Lost` | 筋書き `RuntimeError` の `poll_ex_with`・`poll_with` |
| `CompletionStateEx::Lost` | 模擬のロストを入れた `FaultStub` を内側にしたデコレータの `poll_ex_with` |
| `Timeout`・`CompletionStateEx::Unknown` | 筋書き `CompletionUnknown` の `wait_blocking_with`・`poll_ex_with`（`wait_blocking_with` は時間を受けない） |
| `CapabilityInsufficient` | 筋書き `CapabilityInsufficient` の `create_pipeline_with` |
| `ValidationFailed` | `FaultStub` の受け口へ `HookFault::ValidationReport` を差し込んだときに届く値 |
| `InvalidArgument` | 型の違う上限の置き換えを読む `limits_with` |
| `BackendFailure` | 筋書き `RuntimeError` の `wait_blocking_with` |
| 呼び出し時点の `DeviceLost` | ロスト済みの `FaultStub` を内側にした `submit_with` の内側の戻り値 |
| `InvalidCombination`・`ArtifactRejected` | 試験の中で作る値 |

- 種類は、戻り値の `failure_class` と、`_` を持つ `match`（別 crate の利用側と同じ形）の両方で得て、一致を見る

## 対応する試験

| 対象 | 確かめる内容 | 試験 |
| ---- | ------------ | ---- |
| ERR-20 | 出どころが期待の値を返す。各組の代表と、`CompletionState::Lost`・`CompletionStateEx::Unknown` が互いに異なる値で、各組が互いに異なる種類になる（主張の主体は種類の比較）。`Unknown` が他のどの値とも異なる。`Timeout` が `Runtime` ではなく `CompletionUnknown` | `err20_sources_return_the_expected_values`・`err20_four_groups_are_pairwise_distinct_values`・`err20_groups_get_distinct_classes`・`err20_unknown_differs_from_every_other_value`・`err20_timeout_is_completion_unknown_not_a_confirmed_failure` |
| ERR-58 | `InvalidCombination`・`ArtifactRejected`・`ValidationFailed`・`InvalidArgument`・`BackendFailure`・呼び出し時点の `DeviceLost`・`Timeout` が、入口と `match` の両方で期待の種類になり、ERR-20 の同じ組の代表とだけ同じ種類になる。呼び出し時点の `DeviceLost` は `Runtime` ではない | `err58_sources_return_the_expected_variants`・`err58_values_get_the_expected_class_by_entry_and_by_match`・`err58_values_are_distinct_from_the_err20_groups`・`err58_call_time_device_lost_is_not_a_runtime_error` |
| ERR-21 | `ContractError` の全 15 値と `CompletionStateEx` の全 5 値で入口が `match` と一致する（`Pending`・`Complete` は `None`）。2 つの型で同じ組が同じ種類になる。`FailureClass` の全部の値が返る | `err21_entry_returns_a_class_for_every_contract_error_and_agrees_with_match`・`err21_entry_on_completion_state_ex_agrees_with_match`・`err21_both_types_give_the_same_class_for_the_same_group`・`err21_all_five_classes_are_returned` |

## 段の表

| 段 | イシュー | 内容 | 本リポの物 |
| -- | -------- | ---- | ---------- |
| 1/6 | #4058 | 現状と前提の確認 | 本書の「記録の所在」 |
| 2/6 | #4059 | 実装（ERR-20 の値の出どころ） | `failure_class_contract.rs` の「ERR-20 の各組の出どころ」 |
| 3/6 | #4060 | 実装（ERR-58 の値の出どころ） | 同「ERR-58 の値の出どころ」 |
| 4/6 | #4061 | 実装（ERR-21 の入口と `match` の突き合わせ） | `class_by_match`・`class_by_match_ex` |
| 5/6 | #4062 | テスト | 本書の「対応する試験」 |
| 6/6 | #4063 | 確かめ | 下の「完了の条件の確かめ」 |

## 確かめ方

- `cargo test -p fandhe-silicon-core --features fault-injection --test failure_class_contract`
- `make verify`・`make check`・`make deny`・`make deps-check`

## 完了の条件の確かめ

完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

| # | 対象 | 対応する試験 | 結果 |
| - | ---- | ------------ | ---- |
| 1 | ERR-20 | 上の ERR-20 の 5 件 | 通った |
| 2 | ERR-58 | 上の ERR-58 の 4 件 | 通った |
| 3 | ERR-21 | 上の ERR-21 の 4 件 | 通った |
| 4 | 制約 | 依存は足していない（`make deps-check`）。`core` は `#![forbid(unsafe_code)]` | 通った |

注:

- 完了不明は、時間を受けない `wait_blocking_with` が返す `Timeout` で確かめている。本リポには時間を受ける `wait_blocking` が無いため
- ERR-21 の試験の対象は、本リポの `ContractError` の現存の 15 値と `CompletionStateEx` の 5 値。`#[non_exhaustive]` の外からは網羅を取れないので、値が増えたら試験の一覧と件数を足す
- 結果は `make verify`・`make check`・`make deny`・`make deps-check` を流した時点のもの
