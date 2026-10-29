# ロストの通知先の関数の型の確かめ（TASK-138）の所在と本リポの現状

TASK-138 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-138。対象は DEV-136。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 現状の形

コードと公開テストが示す事実だけを置く。

| 項目 | 事実 | 試験 |
| ---- | ---- | ---- |
| 通知先の口 | `core::device_lost` に `DeviceLostNotify`（`set_lost_callback`。`dyn` で使える）と関数の型 `LostCallback` | `crates/core/tests/device_lost_notify.rs` の `send_closure_is_accepted_through_dyn_and_receives_the_reason` |
| native の型 | `Box<dyn FnOnce(DeviceLostReason) + Send>`。`Send` でない関数（`Rc` を捕まえる）はコンパイルできない | 同上の `native_callback_type_is_send`、`device_lost` モジュールの `compile_fail` の例（同じ形で `Arc` にした通る例と対にしてある） |
| wasm32 の型 | `Box<dyn FnOnce(DeviceLostReason)>`（`Send` なし）。`Rc` を捕まえる関数が通る | 同上の `wasm32_accepts_a_closure_that_is_not_send`（`cargo check --target wasm32-unknown-unknown`）、`wasm/crates/glue` の同名の試験。CI で効くのは glue の試験（`make wasm-verify` の wasm32 の clippy が型検査する）で、core 側の同名の試験は `cargo check --target wasm32-unknown-unknown --test device_lost_notify` の手元の確かめ |
| 既存の型 | `fault::stub::LostCallback` は同じ型の再公開。`FaultStub` は `DeviceLostNotify` も実装する。wasm32 では登録された関数が `Send` でないので `FaultStub` は `Send` でない（native は `Send + Sync`） | `fault_stub_lost.rs` の `lost_is_notified_through_the_dyn_notify` |
| 依存・unsafe | 外部依存は足していない（`wasm/` の glue に試験用の `core` への path 依存を足した）。`core` は `#![forbid(unsafe_code)]` | `make deps-check`・`make wasm-verify` |

## 確かめ方

- `cargo test -p fandhe-silicon-core --test device_lost_notify`・`cargo test -p fandhe-silicon-core --doc device_lost`
- `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --test device_lost_notify`
- `make wasm-verify`
