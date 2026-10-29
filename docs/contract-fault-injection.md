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
| 公開のトレイト | `crates/`・`wasm/` にある `pub trait` は、`core` の `Capabilities`、`test-support` の裏の `rerun` の `Check`、`fault-injection` の裏の `fault` の `FaultHooks` 、`memory` の `MemoryOps`、`validation` の `ValidationIntake` の 5 件 | `git grep -n -E 'pub trait ' -- crates wasm` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。wasm32 でもビルドされる。`test-support` の裏のモジュールは `parity`・`test_frame`・`rerun`、`fault-injection` の裏のモジュールは `fault` | `crates/core/src/lib.rs`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| `contract` の形 | `#![deny(unsafe_code)]`。`unsafe` は `backend_*` だけに置く | `crates/contract/src/lib.rs`、[unsafe-policy](unsafe-policy.md) |
| workspace | member は `crates/` 配下の 5 つ。`make deny` が `crates/` の外を指す path を拒む | ルート `Cargo.toml`、[workspace-boundary](workspace-boundary.md) |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `core`・`contract` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-271 の言及は本書、`CLAUDE.md`、`README.md`、`crates/core/src/fault.rs`、`crates/core/src/fault/`、その試験、`crates/contract/` の転送の試験・部品の試験・doctest、`scripts/build-profiles.sh` | `git grep -n TASK-271 -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/8 | #4050 | 現状と前提の確認（本書） |
| 2/8 | #4051 | 形の設計（「現状の形（2/8）」の節） |
| 3/8 | #4052 | 模擬のロストと完了不明の差し込み（「現状の形（3/8）」の節） |
| 4/8 | #4053 | 能力不足と能力の取得の失敗の差し込み（「現状の形（4/8）」の節） |
| 5/8 | #4054 | 投入の回数の計数と模擬の OOM の差し込み（「現状の形（5/8）」の節） |
| 6/8 | #4055 | 模擬の実行時エラーと上限の差し替えの口（「現状の形（6/8）」の節） |
| 7/8 | #4056 | テスト（「試験（7/8）」の節） |
| 8/8 | #4057 | 手順書の作成（「使い分けの手順（8/8）」の節） |

## 現状の形（2/8）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

- feature: `core` の `fault-injection` と、`contract` の `fault-injection`（`core` の同名 feature へ転送）。`core` では `fault` モジュールが feature の裏にある。外部依存は足していない
- 口 `fault::FaultHooks`: 背後実装が実装するトレイト。メソッドは `simulate_device_lost(DeviceLostReason)`・`arm_gate`・`open_gate`・`simulate_runtime_error(String)`・`simulate_validation_report(String)` の 5 つで、戻り値は `Result<(), ContractError>`。`dyn` で使え、すべて既定の実装を持つ。既定の実装は黙って成功せず、`UnsupportedExtensionDefault` を自分のメソッド名の `op` で返す
- 種類 `fault::HookFault`（`non_exhaustive`）: `DeviceLost`・`ArmGate`・`OpenGate`・`RuntimeError`・`ValidationReport`
- 差し込みの部品 `fault::inject(Option<&dyn FaultHooks>, HookFault)`: 種類に対応する口を 1 回だけ呼び、戻り値をそのまま返す。`None` のときは何も呼ばず `UnsupportedExtensionDefault { op: "fault_hooks" }` を返す
- 試験: `crates/core/tests/fault_hooks_types.rs`・`fault_hooks_inject.rs`、`crates/contract/tests/fault_injection_forwarding.rs`、`crates/contract/src/cfg_gate_doctest.rs` の対（無効の構成では `core` の `fault` が解決できず compile_fail、有効の構成では通る）
- 組: `scripts/build-profiles.sh` の `NATIVE_SETS` に `fault-injection` 単独の組がある
- 確かめ方: `cargo test -p fandhe-silicon-core --features fault-injection`、`cargo test -p fandhe-silicon-contract --features fault-injection`、`make build-profiles`

## 現状の形（3/8）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

- `fault::stub::FaultStub`: 背後実装を持たない試験用のスタブ。`FaultHooks` を実装し、`Send + Sync`。`simulate_device_lost` は 1 回目だけ理由を記録し、登録済みのコールバックを lock の外で 1 回呼ぶ。2 回目以降は `Ok` を返すだけで、理由も回数も変えない。コールバックが未登録のとき `lost_callback_count` は 0 のまま。`arm_gate`・`open_gate` は門の状態の記録だけで、何度呼んでも `Ok`。`simulate_runtime_error` は既定の実装のまま。`ValidationIntake` も実装し、`simulate_validation_report` は登録された関数へ `ValidationFailed` を届ける（device はロストしない）。届けた数は `validation_delivered_count` で読める
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

## 現状の形（5/8）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

- `FaultInjecting::submit_with`: 内側を呼ぶときに計数をちょうど 1 増やし、内側の値（`Ok`・`Err` とも）をそのまま返す。どの筋書きでも内側へ渡して数える。`submit_call_count()` が回数を返す（初期値 0。`Send + Sync` を保つ）
- `FaultScript::AllocationFails`: `allocate_with(requested_bytes, ..)` が内側を呼ばず `Err(AllocationFailed { requested_bytes })` を返す。要求の大きさは引数の値をそのまま載せる。他の筋書きは内側へ同じ大きさで渡し、値をそのまま返す。実際の確保は行わない
- `wait_blocking_with`・`poll_ex_with`・`create_pipeline_with`・`query`・`query_ex` は新しい筋書きでも内側の値をそのまま返す
- 試験: `crates/core/tests/fault_decorator_submit_count.rs`・`fault_decorator_oom.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --features fault-injection`

## 現状の形（6/8）

型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。

- `FaultScript::RuntimeError { backend, detail }`: 投入（`submit_with`）は内側へ通して数える。`wait_blocking_with` は内側を呼ばず `Err(BackendFailure { op: "wait_blocking", backend, detail })` を返し、`poll_ex_with` は `CompletionStateEx::RuntimeError`、`poll_with`（閉じた完了状態）は `CompletionState::Lost` を返す。`Timeout`・`Unknown` とは戻り値で区別できる。`backend` は筋書きが持つ。実際の故障は起こさない（TASK-264 (3) のポインタ）
- `poll_with`: `RuntimeError` 以外は（`CompletionUnknown` を含め）内側の値をそのまま返す
- `create_pipeline_with`・`allocate_with`・`query`・`query_ex`・`FaultHooks` の口は `RuntimeError` でも内側へそのまま渡す
- `FaultInjecting::with_limits_override` / `limits_with`: 上限の型 `L` に依らず、型を消した置き換えを持つ。置き換えが無ければ内側を 1 回呼んでその値を返し、`L` の置き換えがあれば内側を呼ばずその複製を返す。型が `L` でなければ内側を呼ばず `Err(InvalidArgument)` を返す。筋書きとは独立で、`Send + Sync` を保つ。上限の表（TASK-157 のポインタ）: `LimitsTable` の置き換えは `Capabilities::limits()` にも効く（`FaultScript` の 6 種類すべてで確かめた）。別の型の置き換えは `limits()` に効かず内側の表を返し、`limits_with::<LimitsTable>` は `Err(InvalidArgument)` のまま。対応するテストは `fault_decorator_limits.rs`、詳細は [contract-limits-table](contract-limits-table.md)。行ごとの確かめは別の段の持ち分
- 試験: `crates/core/tests/fault_decorator_runtime_error.rs`・`fault_decorator_limits.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --features fault-injection`

## 試験（7/8）

`src` は変えず、段をまたぐ確かめの試験だけを足した。コードと公開テストが示す事実だけを置く。

- `crates/core/tests/fault_parts_matrix.rs`: デコレータの 6 つの筋書きと全ての差し込み（`submit_with`・`allocate_with`・`wait_blocking_with`・`poll_ex_with`・`poll_with`・`create_pipeline_with`・`query`・`query_ex`・上限の差し替え・`FaultHooks` の 4 つの口）の組で、内側を呼んだ回数（0 回か 1 回）と返す値を、内側の値と合成の値が必ず違う偽の内側で確かめる。内側の `Err` が通す組でそのまま返ること、合成の失敗（`Timeout`・`BackendFailure`・`AllocationFailed`・`CapabilityInsufficient`・`InvalidArgument`）が互いに値で区別できることも見る
- `crates/core/tests/fault_parts_composition.rs`: 内側の型（`!Sync` の偽物・`Send + Sync` の偽物・型を消した内側）と背後実装の種類（`BackendKind` の 4 種）に依らず同じに働くこと、デコレータが `Send + Sync` で `&dyn FaultHooks`・`&dyn Capabilities` に強制変換できること、`inject` が口を返す側（試験の中だけの見立ての trait）・デコレータ越し・スタブ越しに届くこと、筋書きと口の 2 系統が独立に働くことを確かめる
- `crates/core/tests/fault_validation_report.rs`: 検証の報告を模す口 `simulate_validation_report` の試験（TASK-554。所在は [validation-intake](validation-intake.md)）
- `crates/contract/tests/fault_injection_parts.rs`: contract の feature `fault-injection` だけで、デコレータとスタブが組み上がって働くことを確かめる
- `CompletionUnknown` の筋書きでの `poll_with` の値と、スタブの `simulate_runtime_error` は、本段の試験では現状の値を固めない
- 確かめ方: `cargo test -p fandhe-silicon-core --features fault-injection`・`cargo test -p fandhe-silicon-contract --no-default-features --features fault-injection`

## 使い分けの手順（8/8）

試験を書く人が、どの部品をどう使うかの手順。型・メソッド・種類の名前は仮。コードと公開テストが示す事実だけを置く。差し込むのは模擬であって、実際の故障ではない。

### 有効にする

- 試験の crate の dev-dependency で feature `fault-injection` を有効にする（`core` に直接、または `contract` の `fault-injection` 経由）。`default` と本番の組には入らない
- 依存先の feature と、試験 crate 自身の feature は別物。dev-dependency で依存先の `fault-injection` を有効にしても、試験 crate に同名の feature は定義されない。`required-features` は試験を所有する package の feature だけを参照する
    - 依存先の feature を常時有効にする dev-dependency なら、`required-features` は要らない
    - 条件付きにする（`cargo test --features fault-injection` で切り替える）なら、試験 crate 側にも `fault-injection = ["<依存先>/fault-injection"]` の転送用 feature を定義し、統合テストの `[[test]]` に `required-features = ["fault-injection"]` を付ける
    - `crates/core/Cargo.toml` の既存の `fault_*` の宣言は、`core` 自身が持つ feature を参照する形
- 確かめ: `crates/contract/tests/fault_injection_forwarding.rs`・`fault_injection_parts.rs`

### 2 系統の使い分け

| したいこと | 使う部品 | 例の試験 |
| ---------- | -------- | -------- |
| 内側を呼ばずに合成の失敗を返させる（`Timeout`・`Unknown`・`CapabilityInsufficient`・`S0`・`AllocationFailed`・実行時エラー） | (A) `decorator::FaultInjecting` と `FaultScript` | `fault_decorator_*.rs`・`fault_parts_matrix.rs` |
| 内側の投入の回数を数える | (A) `submit_with` と `submit_call_count()` | `fault_decorator_submit_count.rs` |
| 上限を差し替える | (A) `with_limits_override` と `limits_with` | `fault_decorator_limits.rs` |
| 背後実装なしで模擬のロスト・コールバックの計数・門の状態を扱う | (A) `stub::FaultStub` | `fault_stub_lost.rs` |
| 背後実装自身をロストさせる・門で止める・実行時エラーにする | (B) `FaultHooks` の口を `inject(Option<&dyn FaultHooks>, HookFault)` で呼ぶ | `fault_hooks_inject.rs`・`fault_parts_composition.rs` |

### (A) デコレータの使い方

1. 偽の内側を用意する（`Device` のトレイトはまだ無いので、包む値は型引数 `D`）
2. `FaultInjecting::new(inner, script)` で包む。筋書きは `PassThrough`・`CompletionUnknown`・`CapabilityInsufficient { feature, required }`・`CapabilityQueryFails`・`AllocationFails`・`RuntimeError { backend, detail }`
3. 対象の `*_with(クロージャ)` を呼び、戻り値と内側を呼んだ回数を見る。クロージャは内側 `&D` を受け取る
    - `submit_with`・`wait_blocking_with`・`poll_ex_with`・`poll_with`・`create_pipeline_with`・`limits_with`: `|inner| ...`
    - `allocate_with(requested_bytes, ..)`: `|inner, requested_bytes| ...`（`&D` と要求の大きさを受け取る）
4. `D: Capabilities` なら `Capabilities`、`D: FaultHooks` なら `FaultHooks` を実装する（口は内側へそのまま渡す）。`Send + Sync` は保たれる

筋書きごとの働き（内側を呼ばない差し込みだけ。ほかは内側の値をそのまま返す。詳細は 3/8〜6/8 の節）:

| 筋書き | 働き |
| ------ | ---- |
| `CompletionUnknown` | `wait_blocking_with` は `Err(Timeout)`、`poll_ex_with` は `Unknown` |
| `CapabilityInsufficient` | 対象の機能だけ `query` が `Unsupported`、`create_pipeline_with` が `Err(CapabilityInsufficient)` |
| `CapabilityQueryFails` | `query` が全機能で `S0` |
| `AllocationFails` | `allocate_with` が `Err(AllocationFailed { requested_bytes })` |
| `RuntimeError` | `wait_blocking_with` が `Err(BackendFailure)`、`poll_ex_with` が `RuntimeError`、`poll_with` が `Lost` |

試験: `crates/core/tests/fault_parts_matrix.rs`（筋書きと差し込みの全組）と段ごとの `fault_decorator_*.rs`。

### (A) スタブの使い方

`FaultStub::new()` → `set_lost_callback` → `inject(Some(&stub), HookFault::DeviceLost(..))` → `is_lost`・`lost_reason`・`lost_callback_count`・`gate_armed` で見る。2 回目のロストは無視され、コールバックは 1 回だけ呼ばれる。試験: `crates/core/tests/fault_stub_lost.rs`。

### (B) 口の使い方

- 口を返す側の戻り値（`Option<&dyn FaultHooks>`）を `inject` に渡す。`None` なら `UnsupportedExtensionDefault { op: "fault_hooks" }`、口を実装していなければ既定の実装がメソッド名の `op` で `UnsupportedExtensionDefault` を返す（黙って成功しない）。試験: `fault_hooks_inject.rs`・`fault_parts_composition.rs`
- 背後実装ごとの実装の持ち主は、TASK-2932〜TASK-2934（`simulate_device_lost`）、TASK-319・TASK-332・TASK-345（門）、TASK-280（実行時エラーの完了）、TASK-157（上限の型）のポインタに従う

### 2 系統を合わせる

デコレータは内側の `FaultHooks` をそのまま通すため、デコレータで包んだスタブにも `inject` が届く。筋書きと口は互いに干渉しない。試験: `fault_parts_composition.rs` の `inject_reaches_stub_through_decorator_accessor`・`script_and_hooks_work_independently`、`crates/contract/tests/fault_injection_parts.rs` の `decorator_over_stub_routes_simulated_loss_to_stub`。

`CompletionUnknown` の筋書きでの `poll_with` の値と、スタブの `simulate_runtime_error` は、試験で値を固めていない。

### 確かめ方

`cargo test -p fandhe-silicon-core --features fault-injection`・`cargo test -p fandhe-silicon-contract --no-default-features --features fault-injection`

### 完了の条件の確かめ

確かめた base は `origin/main` の `f1b0054`、`docs/spec` の gitlink は `f603cba4`。完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | (A) が背後実装の種類に依らず働く | `fault_parts_composition.rs` | `core` の試験 236 passed / 0 failed / 0 ignored |
| 2 | (A) の差し込みが指定した失敗を返す | `fault_parts_matrix.rs`・`fault_decorator_*.rs`・`fault_stub_lost.rs` | 同上 |
| 3 | `FaultHooks` の口の型がコンパイルでき、(B) を呼ぶ部品が組み上がる | `fault_hooks_types.rs`・`fault_hooks_inject.rs`、`crates/contract/tests/fault_injection_parts.rs`・`fault_injection_forwarding.rs`、`crates/contract/src/cfg_gate_doctest.rs` | `contract` の試験 57 passed / 0 failed / 0 ignored |
| 4 | 制約 | `cargo tree -p fandhe-silicon-core -e normal --depth 1` が自身のみ、`grep -c '^\[\[' deps-approved.toml` が 0 | 外部依存 0・承認 0 件 |

(B) 経由で実際の背後実装の無効化・門の働きを確かめる項目は、上の持ち主のタスクの持ち分で、本タスクの試験の範囲外。

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

## 後から足した口

検証レイヤーの報告を模す口 `simulate_validation_report` と種類 `HookFault::ValidationReport` は TASK-554 が足した。所在は [validation-intake](validation-intake.md)。
