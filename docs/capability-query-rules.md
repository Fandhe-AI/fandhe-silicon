# 能力の問合せの共通の規則（TASK-273）の所在と本リポの現状

TASK-273 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-273。ポインタのみ）。内容・理由・経緯は本書に置かない。
本段（1/6）の範囲は、TASK-273 の内容の 1 文目に当たる部分の現状の確認だけである。
持ち込みの区分は TASK-6 の決定の記録に従う（ポインタのみ）。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-269 | `crates/core/src/error.rs`（`UnsupportedReason` など）と `crates/core/tests/error_*.rs` | [contract-error-types](contract-error-types.md) |
| TASK-271 | `crates/core/src/fault.rs`・`fault/stub.rs`・`fault/decorator.rs`（`fault-injection` の裏）と `crates/core/tests/fault_*.rs` | [contract-fault-injection](contract-fault-injection.md) |
| TASK-136 | `crates/core/src/capability.rs`（`CapabilityLevel`・`CapabilityEvidence`・`CapabilityQuery`・`Capabilities`）と `crates/core/tests/capability_query_default.rs`・`device_capability_non_exhaustive.rs`・`device_capability_types.rs` | [contract-device-types](contract-device-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 能力の段階 | `CapabilityLevel` は 8 値（`S0`〜`S6`・`Unsupported`）で `#[non_exhaustive]`。`S0` と `Unsupported` は別の値 | `crates/core/src/capability.rs` |
| 根拠の種類 | `CapabilityEvidence` は 6 値（`RuntimeApi`・`StaticVendorTable`・`ProbeCompile`・`ImplementationLedger`・`FieldRecord`・`PerformanceComparison`）で `#[non_exhaustive]` | 同上 |
| 問合せの結果 | `CapabilityQuery` は `level`・`unsupported_reason: Option<UnsupportedReason>`・`evidence: Option<CapabilityEvidence>` の 3 フィールド | 同上 |
| 問合せのトレイト | `Capabilities` は必須の `query` と、既定の実装つきの `query_ex` を持つ。既定の `query_ex` は `query` の値に、理由 `None`・根拠 `None` を添えて返す | 同上、`crates/core/tests/capability_query_default.rs` |
| 非対応の理由 | `UnsupportedReason` は 3 値（`NoApi`・`NotSupported`・`UnknownFeature`）で `#[non_exhaustive]` | `crates/core/src/error.rs` |
| 背後実装の実装 | `Capabilities` の実装は `crates/core/tests/` の試験用のスタブの 2 件だけ。`crates/contract`・`exec`・`upper`・`wasm/` には 0 件。背後実装の `query`・`query_ex` の実装は別タスクの持ち分（TASK-173〜TASK-175 のポインタ） | `git grep -n -E 'impl .*Capabilities for' -- crates wasm` |
| 戻り値の検査 | `CapabilityQuery` を検査する関数は `crates/`・`wasm/` に無い（参照は `capability.rs` と上記 2 件の試験だけ） | `git grep -n CapabilityQuery -- crates wasm` |
| 2 段の手順に当たる口 | 列挙・device の取得の口（`DeviceProvider` などに当たる物）は `crates/`・`wasm/` に無い（持ち分は TASK-157 のポインタ） | `git grep -n -E 'DeviceProvider\|fn enumerate\|fn request_device' -- crates wasm` |
| 能力の差し込み | `fault-injection` の裏の部品に、能力の問合せの結果を差し替える差し込みは無い | `git grep -n -E 'CapabilityQueryFails' -- crates/core/src/fault` |
| テスト専用の feature | `core` の feature は `test-support`・`fault-injection`。`contract` は同名の feature を `core` へ転送する | `sed -n '/\[features\]/,/^\[\[test/p' crates/core/Cargo.toml crates/contract/Cargo.toml` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。wasm32 でもビルドされる | `crates/core/src/lib.rs`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| workspace | member は `crates/` 配下の 5 つ（`core`・`contract`・`exec`・`upper`・`wasm-abi`）。`make deny` が `crates/` の外を指す path を拒む | ルート `Cargo.toml`、[workspace-boundary](workspace-boundary.md) |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `core` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-273 の言及は本書、`CLAUDE.md`、`README.md` の 3 つだけ | `git grep -n TASK-273 -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #4064 | 現状と前提の確認（本書） |
| 2/6 | #4065 | 実装 |
| 3/6 | #4066 | テスト |
| 4/6 | #4067 | 実装 |
| 5/6 | #4068 | テスト |
| 6/6 | #4069 | 確かめ |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（検査関数・スタブ・試験用の crate の名前と置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場と形は TASK-273 の決定の記録に従う（ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保ち、テスト専用の feature の裏に置く。`default` と本番の組には入れない。新しい member を足す場合も `crates/` 配下に置く
- [docs/contract/](contract/README.md) の項目ファイルは、対応するコードと公開テストを入れる段で作る
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[contract-error-types](contract-error-types.md)・[contract-device-types](contract-device-types.md)・[contract-fault-injection](contract-fault-injection.md)・[contract-feature-check](contract-feature-check.md)・[public-api-contract-check](public-api-contract-check.md)・[workspace-boundary](workspace-boundary.md)・[docs/contract/README.md](contract/README.md)
