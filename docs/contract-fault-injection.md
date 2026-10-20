# 故障の差し込みの部品（TASK-271）の所在と本リポの現状

TASK-271 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-271。ポインタのみ）。内容・理由・経緯は本書に置かない。
持ち込みの区分は TASK-6 の決定の記録に従う（ポインタのみ）。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-269 | `crates/core/src/error.rs`・`capability.rs` と `crates/core/tests/error_*.rs`・`completion_state_closed.rs` | [contract-error-types](contract-error-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `fault-injection` の宣言 | `crates/core/Cargo.toml` に `fault-injection = []`、`crates/contract/Cargo.toml` に `fault-injection = ["fandhe-silicon-core/fault-injection"]`。どちらも `default` と本番の組に入らず、`test-support` とは含意しあわない | `sed -n '/\[features\]/,/^$/p' crates/contract/Cargo.toml crates/core/Cargo.toml` |
| `fault-injection` を囲む既存の検査 | `crates/contract/tests/feature_default_scope.rs` の `TEST_ONLY`（`default` と本番の組に入らない）、`scripts/upper-layer-check.sh` の `FORBIDDEN_FEATURES`（測定の workspace から有効にさせない）、公開 API の検査の本番の組から除く扱い（[public-api-contract-check](public-api-contract-check.md)）、[contract-feature-check](contract-feature-check.md) | `git grep -n fault-injection -- crates scripts docs ':!docs/spec'` |
| feature の裏の中身 | `fault-injection` の `cfg` で隠したコードは `crates/core/src/lib.rs` の `fault` モジュール 1 件（その下の `stub`・`decorator` は同モジュールの子で、別の `cfg` は持たない） | `git grep -n 'feature = "fault-injection"' -- crates wasm` |
| 共通の型 | TASK-269 のエラー・完了状態・理由の型と、TASK-136 の能力問合せの型は `crates/core` に条件なしで公開済み | [contract-error-types](contract-error-types.md)・[contract-device-types](contract-device-types.md) |
| 公開のトレイト | `crates/`・`wasm/` にある `pub trait` は、`core` の `Capabilities`、`test-support` の裏の `rerun` の `Check`、`fault-injection` の裏の `fault` の `FaultHooks` の 3 件 | `git grep -n -E 'pub trait ' -- crates wasm` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。wasm32 でもビルドされる。`test-support` の裏のモジュールは `parity`・`test_frame`・`rerun`、`fault-injection` の裏のモジュールは `fault` | `crates/core/src/lib.rs`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| `contract` の形 | `#![deny(unsafe_code)]`。`unsafe` は `backend_*` だけに置く | `crates/contract/src/lib.rs`、[unsafe-policy](unsafe-policy.md) |
| workspace | member は `crates/` 配下の 5 つ。`make deny` が `crates/` の外を指す path を拒む | ルート `Cargo.toml`、[workspace-boundary](workspace-boundary.md) |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `core`・`contract` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-271 の言及は本書、`CLAUDE.md`、`README.md`、`crates/core/src/fault.rs`、`crates/core/src/fault/`、その試験、`crates/contract/` の転送の試験・doctest、`scripts/build-profiles.sh` | `git grep -n TASK-271 -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/8 | #4050 | 現状と前提の確認（本書） |
| 2/8 | #4051 | 形の設計（「現状の形（2/8）」の節） |
| 3/8 | #4052 | 模擬のロストと完了不明の差し込み（「現状の形（3/8）」の節） |
| 4/8 | #4053 | 能力不足と能力の取得の失敗の差し込み（「現状の形（4/8）」の節） |
| 5/8〜6/8 | #4054〜#4055 | 実装 |
| 7/8 | #4056 | テスト |
| 8/8 | #4057 | 手順書の作成 |

## 現状の形（2/8）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

- feature: `core` の `fault-injection` と、`contract` の `fault-injection`（`core` の同名 feature へ転送）。`core` では `fault` モジュールが feature の裏にある。外部依存は足していない
- 口 `fault::FaultHooks`: 背後実装が実装するトレイト。メソッドは `simulate_device_lost(DeviceLostReason)`・`arm_gate`・`open_gate`・`simulate_runtime_error(String)` の 4 つで、戻り値は `Result<(), ContractError>`。`dyn` で使え、すべて既定の実装を持つ。既定の実装は黙って成功せず、`UnsupportedExtensionDefault` を自分のメソッド名の `op` で返す
- 種類 `fault::HookFault`（`non_exhaustive`）: `DeviceLost`・`ArmGate`・`OpenGate`・`RuntimeError`
- 差し込みの部品 `fault::inject(Option<&dyn FaultHooks>, HookFault)`: 種類に対応する口を 1 回だけ呼び、戻り値をそのまま返す。`None` のときは何も呼ばず `UnsupportedExtensionDefault { op: "fault_hooks" }` を返す
- 試験: `crates/core/tests/fault_hooks_types.rs`・`fault_hooks_inject.rs`、`crates/contract/tests/fault_injection_forwarding.rs`、`crates/contract/src/cfg_gate_doctest.rs` の対（無効の構成では `core` の `fault` が解決できず compile_fail、有効の構成では通る）
- 組: `scripts/build-profiles.sh` の `NATIVE_SETS` に `fault-injection` 単独の組がある
- 確かめ方: `cargo test -p fandhe-silicon-core --features fault-injection`、`cargo test -p fandhe-silicon-contract --features fault-injection`、`make build-profiles`

## 現状の形（3/8）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

- `fault::stub::FaultStub`: 背後実装を持たない試験用のスタブ。`FaultHooks` を実装し、`Send + Sync`。`simulate_device_lost` は 1 回目だけ理由を記録し、登録済みのコールバックを lock の外で 1 回呼ぶ。2 回目以降は `Ok` を返すだけで、理由も回数も変えない。コールバックが未登録のとき `lost_callback_count` は 0 のまま。`arm_gate`・`open_gate` は門の状態の記録だけで、何度呼んでも `Ok`。`simulate_runtime_error` は既定の実装のまま
- `fault::decorator::FaultInjecting<D>` と `FaultScript`（`non_exhaustive`。`PassThrough`・`CompletionUnknown`）: `Device` のトレイトが無いため、包む型を型引数にし、内側の呼び出しはクロージャで受ける。`wait_blocking_with` は `CompletionUnknown` で内側を呼ばず `Err(ContractError::Timeout)`、`poll_ex_with` は内側を呼ばず `CompletionStateEx::Unknown` を返す。`PassThrough` は内側の値をそのまま返す。`FaultHooks` の 4 つの口は内側へそのまま渡す
- 試験: `crates/core/tests/fault_stub_lost.rs`・`fault_decorator_completion_unknown.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --features fault-injection`

## 現状の形（4/8）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

- `FaultScript` に `CapabilityInsufficient { feature, required }` と `CapabilityQueryFails` を足した
- `FaultInjecting<D: Capabilities>` は `Capabilities` を実装する。`CapabilityQueryFails` では `query` が全機能で `S0`、`query_ex` が `S0`（理由・根拠なし）で、内側を呼ばず、`Unsupported` は返さない。`CapabilityInsufficient` では対象の機能だけ `query` が `Unsupported`、`query_ex` が `Unsupported` と `NotSupported` で、内側を呼ばない。対象外の機能と他の筋書きは内側の `query`・`query_ex` へそのまま渡す
- `create_pipeline_with`: `CapabilityInsufficient` で内側を呼ばず `Err(CapabilityInsufficient { feature, required, actual: Unsupported })` を返す。それ以外は内側の値をそのまま返す。`wait_blocking_with`・`poll_ex_with` は新しい 2 つの筋書きでは内側の値をそのまま返す
- 試験: `crates/core/tests/fault_decorator_capability.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --features fault-injection`

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・口・名前・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場と形は TASK-271 の決定の記録に従う（ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ。テスト・診断専用の feature の裏に置き、`default` と本番の組に入れない
- `docs/contract/` の項目ファイルは、対応するコードと公開テストを入れる段で作る（[docs/contract/README.md](contract/README.md)）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

- [contract-error-types](contract-error-types.md)
- [contract-device-types](contract-device-types.md)
- [test-frame](test-frame.md)
- [contract-feature-check](contract-feature-check.md)
- [public-api-contract-check](public-api-contract-check.md)
- [upper-layer-check](upper-layer-check.md)
- [workspace-boundary](workspace-boundary.md)
- [docs/contract/README.md](contract/README.md)
