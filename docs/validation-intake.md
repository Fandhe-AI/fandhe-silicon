# 検証レイヤーの不正の受け口と失敗の種類の入口（TASK-554）の所在と本リポの現状

TASK-554 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-554。対象は ERR-8・ERR-18・ERR-21。ポインタのみ）。内容・理由・経緯は本書に置かない。
決定の記録は private spec 側の記録に従う（ポインタのみ）。関連する本リポの物は次のとおり。

| 物 | 文書 |
| -- | ---- |
| `crates/core/src/error.rs` と `tests/error_*.rs` | [contract-error-types](contract-error-types.md) |
| `crates/core/src/fault*`（`fault-injection` の裏） | [contract-fault-injection](contract-fault-injection.md) |
| `crates/core/src/capability.rs`・`device.rs` | [contract-device-types](contract-device-types.md) |

## 現状の形

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

| 項目 | 事実 | 試験 |
| ---- | ---- | ---- |
| 受け口（ERR-8） | `core::validation` に `ValidationIntake`（`set_validation_callback`）、部品 `ValidationReporter`、関数の型 `ValidationCallback`（native は `FnMut + Send`、wasm32 は `Send` なし）。`report(detail)` は `ContractError::ValidationFailed(detail)` を作り、登録された関数へ報告ごとに渡し、同じ値を返す。未登録の報告と、関数の実行中の報告は届けず `undelivered_count` に数える。`CompletionState` と `on_complete` の引数は変えていない | `crates/core/tests/validation_intake.rs` |
| 検証の報告を模す差し込み | `fault::FaultHooks::simulate_validation_report`（既定は `UnsupportedExtensionDefault`）、`HookFault::ValidationReport`。`FaultStub` が受け口を実装し、`FaultInjecting` は内側へ転送する。報告だけでは device はロストしない | `crates/core/tests/fault_validation_report.rs` |
| ロストの理由 `ValidationError` | `simulate_device_lost` が、指定された理由として渡す。報告だけでは device はロストしない（`report_alone_does_not_lose_the_device`）。報告とロストが続いたときは、報告は受け口へ、ロストは指定した理由で各 1 回届く | 同上 |
| 失敗の種類の入口（ERR-21） | `error::FailureClass`（`non_exhaustive`、5 値）。`ContractError::failure_class()` と `CompletionStateEx::failure_class()`（`Pending`・`Complete` は `None`）。内側の `match` は `_` を持たず、値を足すと種類も決めないとコンパイルが通らない | `crates/core/tests/error_failure_class.rs` |
| 拡張性の 8 型（ERR-18） | `ContractError`・`DeviceLostReason`・`CompletionStateEx`・`RetryPolicy`・`UnsupportedReason`・`IdentityMismatchReason`・`CapabilityLevel`・`CapabilityEvidence` のワイルドカード付きの `match` を別の crate の 1 つの試験にまとめた | `crates/core/tests/error_non_exhaustive_eight.rs` |
| 既存の実装への影響 | 新しい口は既定の実装を持つ。新しい口を知らない `FaultHooks` の実装は変更なしでコンパイルでき、既定の実装が `Err` を返す | `fault_validation_report.rs` の `hooks_without_the_new_method_compile_and_default_to_error` |
| 依存・unsafe | 依存は足していない。`core` は `#![forbid(unsafe_code)]` のまま | `make deps-check` |

## 段の表

| 段 | イシュー | 内容 | 本リポの物 |
| -- | -------- | ---- | ---------- |
| 1/8 | #4180 | 現状と前提の確認 | 本書の「記録の所在」 |
| 2/8 | #4181 | 実装 | `crates/core/src/validation.rs` |
| 3/8 | #4182 | テスト | `tests/validation_intake.rs` |
| 4/8 | #4183 | 方針の整理 | 本書の「現状の形」 |
| 5/8 | #4184 | 実装 | `crates/core/src/fault*`・`error.rs` |
| 6/8 | #4185 | テスト | `tests/fault_validation_report.rs`・`tests/error_failure_class.rs` |
| 7/8 | #4186 | テスト | `tests/error_non_exhaustive_eight.rs` |
| 8/8 | #4187 | 確かめ | 下の「完了の条件の確かめ」 |

## 確かめ方

- `cargo test -p fandhe-silicon-core --all-features --test validation_intake --test fault_validation_report --test error_failure_class --test error_non_exhaustive_eight`
- `make verify`・`make check`・`make deny`・`make deps-check`

## 完了の条件の確かめ

完了の条件の正は private spec 側の記録に従う（ポインタのみ）。下の試験は `make verify` で通った。

| # | 対象 | 対応する試験 | 結果 |
| - | ---- | ------------ | ---- |
| 1 | ERR-8 | `validation_intake.rs` の `report_reaches_callback_as_validation_failed`・`every_report_is_delivered_in_order`・`panicking_callback_stays_registered`、`fault_validation_report.rs` の `injected_report_reaches_the_intake_as_validation_failed`・`report_alone_does_not_lose_the_device`・`validation_caused_loss_is_notified_as_validation_error`・`hooks_without_the_new_method_compile_and_default_to_error` | 通った |
| 2 | ERR-21 | `error_failure_class.rs` の `every_contract_error_value_has_the_expected_class`・`every_completion_state_ex_value_has_the_expected_class`・`match_based_discrimination_agrees_with_the_entry` | 通った |
| 3 | ERR-18 | `error_non_exhaustive_eight.rs` の `eight_types_accept_wildcard_match` | 通った |
| 4 | 制約 | 依存は足していない（`make deps-check`）。`core` は `#![forbid(unsafe_code)]` | 通った |
