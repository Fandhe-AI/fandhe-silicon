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
| 本書の対象の言及 | TASK-136 の言及は本書、`CLAUDE.md`、`README.md`、`crates/core/src/capability.rs`・`device.rs`・`error.rs`、`crates/core/tests/device_capability_types.rs`・`device_request_fields.rs`・`device_capability_non_exhaustive.rs`・`capability_query_default.rs`・`device_lost_reason_values.rs`・`error_*.rs`、[contract-error-types](contract-error-types.md) だけ | `git grep -n TASK-136 -- . ':!docs/spec' ':!.agents'` |

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

- `Device`・`DeviceProvider`・`DeviceDiagnostics` は `crates/core/src/device.rs` にある（ポインタ: TASK-158）。`Device` は `Capabilities` を継ぎ、`backend`・`ownership`・`generation`・`memory`・`diagnostics` の 5 つの method を持つ。`DeviceProvider::request` は `Result<Box<dyn Device>, ContractError>` を返す。`DeviceDiagnostics` は `adapter_name: String` と `backend_name: &'static str` を持つ。対応するテストは `crates/core/tests/device_trait_shape.rs`（確かめ方: `cargo test -p fandhe-silicon-core --test device_trait_shape`）。`limits()` と `LimitsTable` は `capability.rs` にあり、[contract-limits-table](contract-limits-table.md) に書く
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

## 現状の形（4/6）

型の形は 2/6 のまま変えていない。`NativeDeviceHandle` を含めた公開 API・依存グラフの検査を、CI の `deny` ジョブの既存の検査に組み込んだ。

| 確かめるもの | 仕組み | CI での走り方 |
| ------------ | ------ | ------------- |
| 公開 API の wgpu 系の参照の検査が、`NativeDeviceHandle` と `new` に加えて `#[doc(hidden)]` の読む口 `__backend_downcast_ref` も走査する | `scripts/public-api-check.sh` の検査の経路（引数なし・`--manifest-path`）が rustdoc に `--document-hidden-items` を渡す | `deny` ジョブの `make public-api-check`（`scanned: crate=fandhe_silicon_core` の行） |
| 隠した公開アイテムの署名やモジュールに wgpu 系の型が出ても見逃さない。`--emit-json` の経路は隠したアイテムを足さない | `scripts/test-public-api-check.sh` の `real_hidden_*`・`build_hidden_flag`・`emit_no_hidden_flag`・`real_emit_json_no_hidden`（合成 crate の一時 workspace。陽性対照は TASK-12 のポインタ） | `deny` ジョブの `make test-public-api-check` |
| 依存グラフの wgpu 系 crate の検査が、`NativeDeviceHandle` を定義する `core` と背後実装の `contract` を数える | `scripts/dep-graph-wgpu-check.sh` の数え方の定義（全 feature・全 target・製品の 4 crate）。スクリプトは変えていない | `deny` ジョブの `make deny`（`record:` の `workspace_members`） |

- 対象外: `--emit-json` を共有する検査（名前・SDK・WebGPU）の隠したアイテム、`Box<dyn Any>` に実行時に入る値の型（型検査では見えない。背後実装の持ち分）。エラー側の 3 型は [contract-error-types](contract-error-types.md) の持ち分で、照合は 5/6
- 確かめ方: `make public-api-check`・`make test-public-api-check`・`make dep-graph-wgpu-check`
- 詳細: [public-api-wgpu-check](public-api-wgpu-check.md)・[dep-graph-wgpu-check](dep-graph-wgpu-check.md)
- 残る段: 6/6

## 確かめ（5/6）

5/6（#3657）で記録した。GPU の実機なしで実行した値で、件数は実測。型の定義と TASK-269 の試験（`error.rs`・`error_*.rs`）は変えていない（ポインタ: DEV-65・DEV-142）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| D1 | エラー側の 3 型（`UnsupportedReason`・`DeviceLostReason`・`ContractError`）の `#[non_exhaustive]` | `grep -n -A1 'non_exhaustive' crates/core/src/error.rs` | 3 型に付いている |
| D2 | 3 型の拡張性の試験 | `cargo test -p fandhe-silicon-core --test error_non_exhaustive`（`contract_error_stays_non_exhaustive`・`device_lost_reason_stays_non_exhaustive`・`unsupported_reason_stays_non_exhaustive`。先頭に `#![deny(unreachable_patterns)]`） | 6 件が通る。0 failed / 0 ignored |
| D3 | `DeviceLostReason` の `Destroyed`・`Unknown`・`ValidationError(String)` | `error.rs` の定義と `cargo test -p fandhe-silicon-core --test device_lost_reason_values` | 3 値が定義にあり、新しい試験 3 件が通る。0 failed / 0 ignored |
| D4 | `ValidationError(String)` の中身 | `device_lost_reason_values.rs` と `error_fields.rs::device_lost_reason_strings_round_trip` | 文字列を読み戻せる（`error_fields` は 13 件が通る） |
| D5 | 定義の持ち主 | `error.rs` と `error_*.rs` に差分が無いこと | 変更なし |
| D6 | wasm32 でのビルド | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --tests` | 通る |

- `DeviceLostReason` には上の 3 値のほかに `Unrecoverable(String)` がある。定義と試験は TASK-269 の持ち分（ポインタ）。新しい試験は `_` の腕を持ち、値の個数は見ない
- 対象外: ERR-64〜ERR-66 の扱い（`error-format.md` のタスクの持ち分。ポインタ）、spec 側の記録（6/6）
- 確かめ方: `cargo test -p fandhe-silicon-core --test device_lost_reason_values --test error_non_exhaustive --test error_fields`
- 確かめた base: `d1b1563`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`
- 残る段: 6/6

## 完了の条件の確かめ（6/6）

6/6（#3658）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-136）。結果は GPU の実機なしで実行した値で、件数は実測。本体のコード（`src/`）と試験は変えていない。

### 成果物と本書の置き場の対応

| 成果物 | 置き場 | 本書の節 |
| ------ | ------ | -------- |
| 実装（型の定義） | `crates/core/src/device.rs`・`crates/core/src/capability.rs` | 「現状の形（2/6）」 |
| テスト（型の試験） | `crates/core/tests/device_capability_types.rs`・`device_request_fields.rs`・`device_capability_non_exhaustive.rs`・`capability_query_default.rs`・`device_lost_reason_values.rs` | 「現状の形（2/6）」「現状の形（3/6）」「確かめ（5/6）」 |
| CI への組み込み | `scripts/public-api-check.sh`・`scripts/test-public-api-check.sh`（`deny` ジョブ）と、変えていない `scripts/dep-graph-wgpu-check.sh` | 「現状の形（4/6）」 |
| 確かめ（エラー側の 3 型と 3 値の照合） | `crates/core/tests/device_lost_reason_values.rs` と TASK-269 の `error_non_exhaustive.rs`・`error_fields.rs`（変えていない） | 「確かめ（5/6）」と本節 |

### 確かめの表

各項目と DEV の ID の対応は「現状の形（3/6）」と「確かめ（5/6）」の行のとおり（ここでは節を指すだけにする）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| B1 | 統合テスト（feature なし） | `cargo test -p fandhe-silicon-core --test <名>` | `device_capability_types` 4 件・`device_request_fields` 2 件・`device_capability_non_exhaustive` 6 件・`capability_query_default` 3 件・`device_lost_reason_values` 3 件が通る。各 0 failed / 0 ignored |
| B2 | 照合に使った TASK-269 の試験 | `--test error_non_exhaustive --test error_fields` | 6 件・13 件が通る（5/6 の記録と同じ）。0 failed / 0 ignored |
| B3 | `--all-features` | `cargo test -p fandhe-silicon-core --all-features` | B1・B2 の 7 本が同じ件数で通る |
| B4 | 拡張性の試験が実在する | `grep -n '^fn ' crates/core/tests/device_capability_non_exhaustive.rs` | 先頭に `#![deny(unreachable_patterns)]`。`*_stays_non_exhaustive` が 5 本（ほかに値の個数の試験 1 本） |
| C1 | `#[non_exhaustive]` の付き方 | `grep -n -A1 'non_exhaustive' crates/core/src/device.rs crates/core/src/capability.rs` | `BackendKind`・`Profile`・`DeviceOwnership`・`CapabilityLevel`・`CapabilityEvidence` の 5 型に付く。`DeviceRequest`・`CapabilityQuery`・`NativeDeviceHandle`・`FeatureId` には付かない |
| C2 | 置き場と制約 | `lib.rs` の `pub mod device;`・`pub mod capability;`、`cargo tree -p fandhe-silicon-core -e normal --depth 1`、`deps-approved.toml` の記録の数 | 2 つの `mod` に feature の条件なし。`#![forbid(unsafe_code)]`。外部依存 0。承認の記録 0 件 |
| C3 | 他の経路からの参照 | `crates/exec`・`crates/contract`・`crates/upper`・`wasm`・`measure`・`baseline-wgpu` を `DeviceRequest`・`NativeDeviceHandle`・`CapabilityEvidence`・`CapabilityQuery` で検索 | 0 件 |
| C4 | wasm32 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown`（`--tests` 付きも）・`make build-profiles` | いずれも終了コード 0 |
| C5 | 公開 API・依存グラフの検査 | `make public-api-check`・`make test-public-api-check`・`make dep-graph-wgpu-check`・`make public-name-check`・`make public-api-sdk-check` | すべて終了コード 0。`scanned: crate=fandhe_silicon_core` は production が `public_items=1097`・all-features が `public_items=4243`、いずれも `wgpu_type_refs=0`・`wgpu_other_refs=0`（DEV-21） |
| E1 | base の CI | `gh run list --branch main --workflow ci.yml` | 確かめた base のコミットの run `39491463730` が completed / success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | いずれも終了コード 0 |

- 実機の run は含めない。`make inventory-trace-check` は `docs/spec` の取得が要るので含めない。
- 完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-136）。

### 再実行の契機

- `crates/core/src/device.rs`・`capability.rs` の型・値・フィールド・derive・`#[non_exhaustive]`・既定の実装を変えたとき
- B1 の 5 本の試験のファイル、または照合に使った `error_non_exhaustive.rs`・`error_fields.rs`・`crates/core/src/error.rs` を変えたとき
- `scripts/public-api-check.sh`・`scripts/test-public-api-check.sh`・`scripts/dep-graph-wgpu-check.sh`、CI の `deny` ジョブの手順を変えたとき
- `core` の feature・依存・wasm32 のビルドの経路（`scripts/build-profiles.sh`・`scripts/wasm-verify.sh`）を変えたとき
- `docs/spec` の gitlink が進んだとき

確かめた base は `c279b3007d773eb122dc32debe9e450e2555244b`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #3653 | 現状と前提の確認（本書） |
| 2/6 | #3654 | 実装 |
| 3/6 | #3655 | テスト |
| 4/6 | #3656 | CI への組み込み |
| 5/6 | #3657 | 確かめ |
| 6/6 | #3658 | 記録（本書の「完了の条件の確かめ（6/6）」） |

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
