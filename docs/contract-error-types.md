# 契約のエラー・完了状態の型（TASK-269）の所在と本リポの現状

TASK-269 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-269。対象の項目は `docs/spec/04-behavior/error-format.md` の ERR-8・ERR-18・ERR-53・ERR-54。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-4 | workspace と 4 crate の骨格（ルート `Cargo.toml` の `members`） | [workspace-boundary](workspace-boundary.md) |
| TASK-22 | `wasm/`（glue の cdylib）と `scripts/wasm-node-check.sh` ほか | [wasm32-node-foundation](wasm32-node-foundation.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-264・TASK-266・TASK-267・TASK-268・TASK-306 | —（本リポに対応する物は無い。private spec 側の記録に従う〔ポインタのみ〕） | — |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 対象の型の有無 | TASK-269 が定義する型は `crates/core/src/error.rs`・`capability.rs` にある（下の「現状の形（2/8）」）。ほかの既存の公開 enum は、各機能の内側の暫定・個別の型で、契約の共通の型として置かれたものではない | `git grep -n -E 'pub enum [A-Za-z]*(Error\|Reason)' -- crates wasm` |
| 既存のエラー風の型（実行まわり） | 暫定の `OperandCheckError` は外した。`execution` の `validate_dispatch_operands` は `ContractError::InvalidArgument` を返す（TASK-2951） | `crates/core/src/execution/dispatch_check.rs` |
| 既存のエラー風の型（成果物キャッシュ） | `core` の `artifact_cache` に `RejectReason`・`LoadOutcome`（`#[non_exhaustive]`） | `crates/core/src/artifact_cache.rs` |
| 既存のエラー風の型（背後実装） | `contract` の `backend_vulkan::spirv_operands` に `SpirvExtractError`（feature `backend-vulkan` の裏。`#[non_exhaustive]`。`fmt::Display` と `std::error::Error` を実装） | `crates/contract/src/backend_vulkan/spirv_operands.rs` |
| 既存のエラー風の型（試験の道具） | `parity`・`test_frame`・`rerun`（`test-support` の裏）にも個別のエラー型がある。契約の型ではなく検証用の道具の内側 | `git grep -n -E 'pub enum [A-Za-z]*(Error\|Reason)' -- crates/core/src` |
| 拡張性の試験の既存の形 | 別の crate（統合テスト）から `#![deny(unreachable_patterns)]` と `_` の腕で `#[non_exhaustive]` を固定する形がある | `crates/core/tests/execution_type_coverage.rs` |
| `core` の形 | 公開モジュールは `artifact_cache`・`capability`・`error`・`execution`・`hash`（条件なし）と、`test-support` の裏の `parity`・`test_frame`・`rerun`。feature は `test-support` だけ。外部依存 0。`#![forbid(unsafe_code)]` | `crates/core/src/lib.rs`、`crates/core/Cargo.toml` |
| 依存の向き | contract → core、exec → core、upper → core・exec。`wasm/crates/glue` が `contract` に path で依存する | 各 `Cargo.toml`、`wasm/crates/glue/Cargo.toml` |
| wasm32 での `core` | `scripts/build-profiles.sh` が wasm32 向けのビルドを流し、`wasm/` の `make wasm-verify` も `contract` 経由で `core` を wasm32 でビルドする。`core` に足すものは std だけで wasm32 でも通る必要がある。`core` 単体は `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown`（`--tests` を付けた形も）で確かめられる | `scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| 依存の承認 | `deps-approved.toml` に `fandhe-silicon-core` を `used_by` に持つ記録は無い（記録は `crates/contract` の macOS 向けの 4 件だけ） | `grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml`（0 が返る） |
| `exec`・`upper` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec crates/upper` |
| 走る場所 | `core` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-269 の言及は本書、`CLAUDE.md`、`README.md`、`crates/core/src/error.rs`・`capability.rs`・`tests/error_types.rs`・`tests/completion_state_closed.rs`・`tests/error_non_exhaustive.rs`・`tests/error_fields.rs`・`tests/error_debug_format.rs`・`tests/error_display.rs`（と本書への関連のリンク）だけ | `git grep -n TASK-269 -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/8 | #4040 | 現状と前提の確認（本書） |
| 2/8〜3/8 | #4041・#4042 | 実装 |
| 4/8〜7/8 | #4043〜#4046 | テスト |
| 8/8 | #4047 | 確かめ（本書の「完了の条件の確かめ（8/8）」） |

## 現状の形（2/8）

名前は仮で、公開名は変わりうる。型は `crates/core` に feature の条件なしで置く（外部依存 0・`forbid(unsafe_code)`・std だけ）。`Display`・`std::error::Error` は実装しない。variant には `#[non_exhaustive]` を付けない（別の crate からも値を構築できる）。

| 型 | 置き場 | 値・フィールド | derive | `#[non_exhaustive]` |
| -- | ------ | -------------- | ------ | ------------------- |
| `ContractError` | `crates/core/src/error.rs` | 15 値（`BackendFailure`・`CapacityExceeded` を含む。`CapacityExceeded.capacity` は `u32`） | `Debug, Clone, PartialEq, Eq` | あり |
| `DeviceLostReason` | 同上 | `Destroyed`・`Unknown`・`ValidationError(String)`・`Unrecoverable(String)` | `Debug, Clone, PartialEq, Eq` | あり |
| `CompletionState` | 同上 | `Pending`・`Complete`・`Lost` | `Debug, Clone, Copy, PartialEq, Eq` | なし（閉じた 3 値） |
| `CompletionStateEx` | 同上 | `Pending`・`Complete`・`RuntimeError`・`Lost`・`Unknown` | 同上 | あり |
| `RetryPolicy` | 同上 | `IdempotentSafe`・`NotIdempotent` | 同上 | あり |
| `IdentityMismatchReason` | 同上 | `Mismatch`・`Undeterminable` | 同上 | あり |
| `UnsupportedReason` | 同上 | `NoApi`・`NotSupported`・`UnknownFeature` | 同上 | あり |
| `BackendKind` | `crates/core/src/capability.rs` | `Metal`・`Vulkan`・`Cuda`・`Dx12Compute` | `Debug, Clone, Copy, PartialEq, Eq, Hash` | あり |
| `FeatureId` | 同上 | `FeatureId(pub &'static str)` | 同上 | なし |
| `CapabilityLevel` | 同上 | `S0`〜`S6`・`Unsupported`（`S0` と `Unsupported` は別の値） | `Debug, Clone, Copy, PartialEq, Eq` | あり |

- `BackendKind`・`FeatureId`・`CapabilityLevel` は `ContractError` のフィールドが参照する分だけの最小の定義
- 持たないもの: `Display`・`Error` の実装、失敗の種類の入口、`DeviceRequest` ほかの device・能力問合せの型
- 対応するテスト: `crates/core/tests/error_types.rs`（全 value を構築して `Clone` と `PartialEq` で自分と等しいことだけを見る）
- 確かめ方: `cargo test -p fandhe-silicon-core --test error_types`

## 現状の形（3/8）

- 値の一覧は 2/8 のまま。投入状態の観測のための値は `CompletionState`・`CompletionStateEx` に足していない（値の一覧は TASK-306 の決定に従う。ポインタのみ）。`CompletionState` は閉じた 3 値、`CompletionStateEx` は 5 値
- 持ち分の境界: `error.rs` はエラーに関する 7 型。`capability.rs` は `ContractError` のフィールドが参照する最小の 3 型だけで、device・能力問合せ・診断の型は置かない（TASK-136。ポインタ）。`#[non_exhaustive]` の確かめの対象は `error.rs` の 6 型で、`CompletionState` には付けない
- 対応するテスト: `crates/core/tests/completion_state_closed.rs`（`CompletionState` を `_` なしで網羅する `match`）と、`error.rs` の単体テスト（`CompletionStateEx` の 5 値の一覧）
- 確かめ方: `cargo test -p fandhe-silicon-core --test completion_state_closed`・`cargo test -p fandhe-silicon-core --lib error`
- 残る試験は 4/8〜7/8（`#[non_exhaustive]` な型の `_` の試験・フィールドの読み戻し・書式・`Display`/`Error` と wasm32）

## 現状の形（4/8）

- 確かめの対象は `error.rs` の `#[non_exhaustive]` な 6 型（`ContractError` 15 値・`DeviceLostReason` 4 値・`CompletionStateEx` 5 値・`RetryPolicy` 2 値・`IdentityMismatchReason` 2 値・`UnsupportedReason` 3 値）。型の形は 2/8 のまま変えていない
- 仕組み: 別の crate（統合テスト）で `#![deny(unreachable_patterns)]` のもと、全値を並べた後に `_` の腕を置く。属性が外れると `_` が到達不能になりコンパイルが落ちる（ERR-18。ポインタのみ）
- 対象外: `CompletionState`（3/8 の試験）、`capability.rs` の 3 型（TASK-136。ポインタ）
- 対応するテスト: `crates/core/tests/error_non_exhaustive.rs`（型ごとに 1 本、計 6 本）
- 確かめ方: `cargo test -p fandhe-silicon-core --test error_non_exhaustive`
- 残る試験は 5/8〜7/8（フィールドの読み戻し・書式・`Display`/`Error` と wasm32）

## 現状の形（5/8）

- 確かめの対象は `ContractError` のフィールドを持つ 13 値と、`DeviceLostReason` の文字列を持つ 2 値。型の形は 2/8 のまま変えていない
- 仕組み: 別の crate（統合テスト）で全フィールドを `..` なしで束縛して読み戻し、構築時の値と照合する。複数フィールドを持つ値は、1 フィールドだけ変えた値と等しくならないことも見る
- 対象外: `{:?}` の書式と `ValidationFailed` の定義の確かめ（6/8）、`Display`/`Error` と wasm32（7/8）、`capability.rs` の 3 型（TASK-136。ポインタ）
- 対応するテスト: `crates/core/tests/error_fields.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --test error_fields`
- 残る試験は 6/8〜7/8（書式・`Display`/`Error` と wasm32）

## 現状の形（6/8）

- 確かめの対象は `ContractError` の 15 値の `{:?}` と、`ValidationFailed(String)` の定義。型の形は 2/8 のまま変えていない
- 仕組み: 別の crate（統合テスト）で、`{:?}` の出力に境界つきの値の名前の前置と `名前: 値の {:?}` の部分文字列があることを見る（派生の `Debug` の出力全体は照合しない。ERR-54。ポインタのみ）。`ValidationFailed` は `fn(String) -> ContractError` への強制と、同じ文字列のほかの値と等しくならないことで固定する（ERR-8。ポインタのみ）
- 対象外: `Display`/`Error` と wasm32（7/8）、`capability.rs` の 3 型（TASK-136。ポインタ）
- 対応するテスト: `crates/core/tests/error_debug_format.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --test error_debug_format`
- 残る試験は 7/8（`Display`/`Error` と wasm32）

## 現状の形（7/8）

- 確かめの対象は `ContractError` の `Display` と `std::error::Error`（この段で実装した。2/8 の節の「持たないもの」はその段の記録）。値とフィールドの形は 2/8 のまま変えていない。ほかの型には `Display` を実装していない
- 仕組み: 値ごとに英語の決まった句で始まり、フィールドが続く（ERR-54。ポインタのみ）。文字列のフィールドは載せられたまま出すので、改行や非 ASCII を含みうる（エスケープしない）。`FeatureId` は名前の文字列だけを出す。`source()` は `None`。試験は前置とフィールドの部分文字列を見て、文言の全体は固定しない。`core` は診断のためにホスト情報を集めない（`detail`・`reason` に何を載せるかは背後実装の責任）
- 対象外: 失敗の種類の入口（TASK-554。ポインタ）、`CompletionStateEx`・`DeviceLostReason` の `Display`、`capability.rs` の 3 型（TASK-136。ポインタ）、完了の条件の確かめ（8/8）
- 対応するテスト: `crates/core/tests/error_display.rs`
- 確かめ方: `cargo test -p fandhe-silicon-core --test error_display`、`cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown`（`--tests` を付けた形も）、`make build-profiles`
- 残る段は 8/8（確かめ）

## 完了の条件の確かめ（8/8）

8/8（#4047）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-269）。
結果は GPU の実機なしで実行した値で、件数は実測。本体のコード（`src/`）と試験は変えていない。

### 成果物と本書の置き場の対応

| 成果物 | 置き場 | 本書の節 |
| ------ | ------ | -------- |
| 型の定義 | `crates/core/src/error.rs`・`capability.rs` | 「現状の形（2/8）」「現状の形（3/8）」 |
| 試験 | `crates/core/tests/error_types.rs`・`completion_state_closed.rs`・`error_non_exhaustive.rs`・`error_fields.rs`・`error_debug_format.rs`・`error_display.rs` と `error.rs` の単体テスト | 「現状の形（2/8）」〜「現状の形（7/8）」 |

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| B1 | 統合テスト（feature なし） | `cargo test -p fandhe-silicon-core --test <名前>` を `error_types`・`completion_state_closed`・`error_non_exhaustive`・`error_fields`・`error_debug_format`・`error_display` で | 順に 2・1・6・13・6・9 件。いずれも 0 failed / 0 ignored |
| B2 | モジュール内の試験 | `cargo test -p fandhe-silicon-core --lib error` | `completion_state_ex_has_five_values` の 1 件が通る。0 failed / 0 ignored |
| B3 | `--all-features` | `cargo test -p fandhe-silicon-core --all-features` | B1 の 6 本は同じ件数が通る。`--lib` 全体は feature なしで 26 件、`--all-features` で 36 件。0 failed |
| B4 | 拡張性の試験の実在 | `grep -n '^fn ' crates/core/tests/error_non_exhaustive.rs`、ファイル先頭の `#![deny(unreachable_patterns)]` | 6 型ごとの 6 本（`contract_error_`・`device_lost_reason_`・`completion_state_ex_`・`retry_policy_`・`identity_mismatch_reason_`・`unsupported_reason_` の `…_stays_non_exhaustive`）が実在し、B1 の 6 passed に含まれる。説明は「現状の形（4/8）」に従う |
| B5 | `ValidationFailed` と書式の試験の実在 | `crates/core/tests/error_debug_format.rs` | `validation_failed_is_a_string_constructor`・`validation_failed_is_distinct_from_other_string_values`・`debug_output_has_name_prefix_and_fields` が実在し、B1 の 6 passed に含まれる。説明は「現状の形（6/8）」に従う |
| B6 | `Display`・`Error` の試験の実在 | `crates/core/tests/error_display.rs` | `source_is_none_for_all_values`・`coerces_to_dyn_error`・`display_carries_fields_verbatim` が実在し、B1 の 9 passed に含まれる。説明は「現状の形（7/8）」に従う |
| B7 | 閉じた `CompletionState` | `crates/core/tests/completion_state_closed.rs`、`grep -n -B2 'pub enum CompletionState\b' crates/core/src/error.rs` | `_` なしの網羅の `match` が通る。`#[non_exhaustive]` は付いていない |
| C1 | 置き場と制約 | `crates/core/src/lib.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1`、`grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml` | `pub mod error;`・`pub mod capability;` は feature の条件なし。`#![forbid(unsafe_code)]`。外部依存 0。`core` を `used_by` に持つ承認の記録 0 件 |
| C2 | `#[non_exhaustive]` の付き方 | `grep -n -A1 'non_exhaustive' crates/core/src/error.rs` | `error.rs` の 6 型（`ContractError`・`DeviceLostReason`・`CompletionStateEx`・`RetryPolicy`・`IdentityMismatchReason`・`UnsupportedReason`）に付き、`CompletionState` には付いていない |
| C3 | 製品の経路からの参照 | `git grep -n -e ContractError -e CompletionState -e DeviceLostReason -e RetryPolicy -- crates/exec crates/contract crates/upper wasm measure baseline-wgpu` | 0 件 |
| C4 | wasm32 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown`（`--tests` を付けた形も）、`make build-profiles`、`make wasm-verify` | いずれも終了コード 0 |
| C5 | 公開 API の検査 | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check` | いずれも終了コード 0 |
| E1 | base の CI | `gh run list --branch main --workflow ci.yml` | base のコミットの run（id 39470945757）が completed / success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | いずれも終了コード 0 |

### 再実行の契機

- `crates/core/src/error.rs`・`capability.rs` の型・値・欄・derive・`#[non_exhaustive]`・`Display` を変えたとき
- 上の 6 本の試験のファイルを変えたとき
- `docs/spec` の gitlink が進んだとき
- `core` の feature・依存・wasm32 のビルドの経路（`scripts/build-profiles.sh`・`scripts/wasm-verify.sh`）を変えたとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-269）。

確かめた base は `555c2fdfd47a3d61a34a31e46434e945408fa832`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・値・フィールド・名前・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場と形は TASK-269 の決定の記録に従う（ポインタのみ）
- `core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[dispatch-operand-check](dispatch-operand-check.md)・[artifact-cache-check](artifact-cache-check.md)・[test-frame](test-frame.md)・[wasm32-node-foundation](wasm32-node-foundation.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
