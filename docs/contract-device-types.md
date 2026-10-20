# 契約の公開の型（TASK-136）の所在と本リポの現状

TASK-136 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-136。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-4 | workspace と 4 crate の骨格（ルート `Cargo.toml` の `members`） | [workspace-boundary](workspace-boundary.md) |
| TASK-10 | `scripts/public-api-check.sh`（`make public-api-check`。CI の `deny` ジョブが呼ぶ） | [public-api-wgpu-check](public-api-wgpu-check.md) |
| TASK-11 | `scripts/dep-graph-wgpu-check.sh`（`make deny` の中で流れる） | [dep-graph-wgpu-check](dep-graph-wgpu-check.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-269 | `crates/core/src/error.rs`・`capability.rs` と `crates/core/tests/error_*.rs` | [contract-error-types](contract-error-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 既にある型 | `crates/core/src/capability.rs` に `BackendKind`（`#[non_exhaustive]`）・`FeatureId`・`CapabilityLevel`（`#[non_exhaustive]`）がある。モジュールのコメントのとおり、3 型の定義の持ち主は TASK-136 | `crates/core/src/capability.rs` |
| エラー側の型 | `crates/core/src/error.rs` の `ContractError`・`DeviceLostReason`・`UnsupportedReason` などは TASK-269 の持ち分。定義と `#[non_exhaustive]` の確かめは [contract-error-types](contract-error-types.md) | `crates/core/src/error.rs` |
| 残りの型 | device の要求の型・能力問合せのトレイト・device の型に当たる物は `crates/`・`wasm/` に無い（当たるのは `CapabilityLevel` と `DeviceLostReason` の 2 件だけ） | `git grep -n -E 'pub (trait\|struct\|enum) [A-Za-z]*(Device\|Request\|Capabilit\|Profile\|Ownership\|Handle)' -- crates wasm` |
| `contract` の公開面 | 公開モジュールは feature の裏の `backend_*` と `doc(hidden)` の `cfg_gate_doctest` だけ。契約の共通の公開型は無い | `grep -n 'pub mod' crates/contract/src/lib.rs` |
| `exec`・`upper` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec crates/upper` |
| 公開 API の検査 | `scripts/public-api-check.sh` は workspace の lib crate を全部、2 組（production / all-features）で走査する。どの crate に型を足しても対象に入る | `scripts/public-api-check.sh` |
| 依存グラフの検査 | `scripts/dep-graph-wgpu-check.sh` は製品の 4 crate の依存グラフを全 feature・全 target で数える | `scripts/dep-graph-wgpu-check.sh` |
| 拡張性の試験の既存の形 | 別の crate（統合テスト）から `#![deny(unreachable_patterns)]` と `_` の腕で `#[non_exhaustive]` を固定する形がある | `crates/core/tests/error_non_exhaustive.rs`、`crates/core/tests/execution_type_coverage.rs` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。feature は `test-support` だけ。wasm32 でもビルドされる | `crates/core/Cargo.toml`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `core` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-136 の言及は本書、`CLAUDE.md`、`README.md`、`crates/core/src/capability.rs`・`device.rs`・`error.rs`、`crates/core/tests/device_capability_types.rs`・`device_request_fields.rs`・`device_capability_non_exhaustive.rs`・`capability_query_default.rs`・`error_*.rs`、[contract-error-types](contract-error-types.md) だけ | `git grep -n TASK-136 -- . ':!docs/spec' ':!.agents'` |

## 現状の形（2/6）

型は `crates/core` に置く（外部依存 0・`forbid(unsafe_code)` のまま）。置き場と形は TASK-136 の決定の記録に従う（ポインタのみ）。

| 型 | 置き場 | 値・フィールド | `#[non_exhaustive]` |
| -- | ------ | -------------- | ------------------- |
| `DeviceRequest` | `crates/core/src/device.rs` | `backend`・`profile`・`ownership` の 3 つ（`Debug` のみ） | なし |
| `Profile` | 同上 | `CpuOnly`・`ComputeOnly`・`Graphics` | あり |
| `DeviceOwnership` | 同上 | `ProductIndependent`・`Shared`・`Imported { native }` | あり |
| `NativeDeviceHandle` | 同上 | 非公開の 1 フィールド。safe な `new`、`doc(hidden)` の `__backend_downcast_ref`。`Debug` は中身を出さない | なし |
| `CapabilityEvidence` | `crates/core/src/capability.rs` | 6 値（`RuntimeApi`・`StaticVendorTable`・`ProbeCompile`・`ImplementationLedger`・`FieldRecord`・`PerformanceComparison`） | あり |
| `CapabilityQuery` | 同上 | `level`・`unsupported_reason`・`evidence` | なし |
| `Capabilities`（トレイト） | 同上 | 必須の `query`、既定の実装つきの `query_ex`。オブジェクト安全 | — |

- `limits()`・`Device`・`DeviceProvider` はここに無い（別の持ち分。ポインタ: TASK-157）
- 対応するテスト: `crates/core/tests/device_capability_types.rs`（構築と呼び出しだけを見る最小の試験）
- 確かめ方: `cargo test -p fandhe-silicon-core --test device_capability_types`
- 残る段: フィールド・`#[non_exhaustive]`・既定の実装の値の試験は 3/6 以降

## 現状の形（3/6）

型の形は 2/6 のまま変えていない。別の crate（統合テスト）から、次を固定する。

| 確かめるもの | 仕組み | 対応する項目（ポインタ） |
| ------------ | ------ | ------------------------ |
| `DeviceRequest` のフィールドが `backend`・`profile`・`ownership` の 3 つで、型が `BackendKind`・`Profile`・`DeviceOwnership` | `..` なしの分解と型注釈（フィールドが増えるとコンパイルが落ちる） | DEV-4 |
| `BackendKind`・`Profile`・`DeviceOwnership`・`CapabilityLevel`・`CapabilityEvidence` の 5 型が `#[non_exhaustive]` | `#![deny(unreachable_patterns)]` のもとで全値を並べた後に `_` の腕（属性が外れると落ちる） | DEV-65 |
| `CapabilityLevel` が 8 値（`S0`〜`S6`・`Unsupported`）で互いに別の値 | 全組の比較。`S0` と `Unsupported` も別 | DEV-81 |
| 機能 ID を鍵にして `query` が段階を返す | 機能 ID ごとに別の段階を返すスタブ | DEV-82 |
| `query_ex` を上書きしない実装がコンパイルでき、`level` は `query` の値、`unsupported_reason`・`evidence` は `None` | `query` だけを実装するスタブ。`dyn` 越しでも同じ | DEV-86 |

- 対象外: `NativeDeviceHandle` を含めた公開 API・依存グラフの検査は 4/6。エラー側の 3 型は [contract-error-types](contract-error-types.md) の持ち分で、照合は 5/6
- 対応するテスト: `crates/core/tests/device_request_fields.rs`（`device_request_has_exactly_three_fields`・`device_request_fields_read_back`）、`device_capability_non_exhaustive.rs`（`*_stays_non_exhaustive` の 5 本・`capability_level_has_eight_distinct_values`）、`capability_query_default.rs`（`query_returns_level_keyed_by_feature_id`・`query_ex_default_passes_query_level_through`・`query_ex_default_works_through_dyn`）
- 確かめ方: `cargo test -p fandhe-silicon-core --test device_request_fields --test device_capability_non_exhaustive --test capability_query_default`
- 残る段: 4/6 以降

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #3653 | 現状と前提の確認（本書） |
| 2/6 | #3654 | 実装 |
| 3/6 | #3655 | テスト |
| 4/6 | #3656 | CI への組み込み |
| 5/6 | #3657 | 確かめ |
| 6/6 | #3658 | 記録 |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・値・フィールド・名前・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場（`core` か `contract` か）と形は、TASK-136 の決定の記録に従う（ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- `docs/contract/` の項目ファイルは、対応するコードと公開テストを入れる段で作る（[docs/contract/README.md](contract/README.md)）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

- [contract-error-types](contract-error-types.md)
- [public-api-wgpu-check](public-api-wgpu-check.md)
- [dep-graph-wgpu-check](dep-graph-wgpu-check.md)
- [test-frame](test-frame.md)
- [workspace-boundary](workspace-boundary.md)
- [docs/contract/README.md](contract/README.md)
