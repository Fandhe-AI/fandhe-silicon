# 能力の問合せの共通の規則（TASK-273）の所在と本リポの現状

TASK-273 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-273。ポインタのみ）。内容・理由・経緯は本書に置かない。
1/6 の範囲は、TASK-273 の内容の 1 文目に当たる部分の現状の確認だけである。2/6 の範囲は、その (1)・(2) に当たる試験用の部品（「現状の形（2/6）」節）である。3/6 の範囲は、その (1)・(2) の契約テストの組み合わせ（「現状の形（3/6）」節）である。4/6 の範囲は、その (3)・(4) の検査関数と部品の試験（「現状の形（4/6）」節）である。5/6 の範囲は、その (3)・(4) の組み合わせの契約テスト（「現状の形（5/6）」節）である。6/6 の範囲は、成果物の確かめ（「完了の条件の確かめ（6/6）」節）である。
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
| 背後実装の実装 | `Capabilities` の実装は、`crates/core/src/capability_rule.rs` の `CapabilityTable`（`test-support` の裏）と、`crates/core/src/fault/decorator.rs` の `FaultInjecting<D>`（`fault-injection` の裏）と、`crates/core/tests/` の試験用のスタブ（`capability_query_default.rs`・`device_capability_types.rs`・`fault_decorator_capability.rs`・`capability_rule_contract.rs`〔`dyn Capabilities` を包む入れ物〕・`capability_rule_check.rs`〔`query` だけを実装するスタブ〕・`capability_rule_check_contract.rs`〔`query` だけを実装するスタブと `dyn Capabilities` を包む入れ物〕）。`crates/contract`・`exec`・`upper`・`wasm/` には 0 件。背後実装の `query`・`query_ex` の実装は別タスクの持ち分（TASK-173〜TASK-175 のポインタ） | `git grep -n -E 'impl.* Capabilities for' -- crates wasm` |
| 戻り値の検査 | `crates/core/src/capability_rule.rs` の `check_query`・`check_query_ex`（`test-support` の裏）。製品の経路からは使わない | `git grep -n -e check_query -- crates wasm` |
| 2 段の手順に当たる口 | device の取得の口は `crates/core/src/device.rs` の `DeviceProvider::request`（TASK-158 のポインタ）。列挙の口は `crates/`・`wasm/` に無い。試験用の `query_after_acquire` がクロージャで受けるだけで、製品の列挙の口は無い | `git grep -n -e DeviceProvider -e 'fn enumerate' -e 'fn request_device' -- crates wasm` |
| 能力の差し込み | `fault-injection` の裏の `FaultInjecting<D>` が `Capabilities` を実装し、`FaultScript::CapabilityQueryFails` で問合せの結果を `S0`（理由 `None`・根拠 `None`）に差し替える。対象の機能だけ `Unsupported` にする `CapabilityInsufficient` もある。公開テストは `crates/core/tests/fault_decorator_capability.rs` | `git grep -n -E 'CapabilityQueryFails' -- crates/core` |
| テスト専用の feature | `core` の feature は `test-support`・`fault-injection`。`contract` は同名の feature を `core` へ転送する | `sed -n '/\[features\]/,/^\[\[test/p' crates/core/Cargo.toml crates/contract/Cargo.toml` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。wasm32 でもビルドされる | `crates/core/src/lib.rs`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| workspace | member は `crates/` 配下の 5 つ（`core`・`contract`・`exec`・`upper`・`wasm-abi`）。`make deny` が `crates/` の外を指す path を拒む | ルート `Cargo.toml`、[workspace-boundary](workspace-boundary.md) |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `core` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-273 の言及は本書、`CLAUDE.md`、`README.md` と、2/6 の試験用の部品の `crates/core/src/capability_rule.rs`、`crates/core/tests/capability_rule_stub.rs`、`crates/core/tests/capability_rule_two_step.rs` と、3/6 の `crates/core/tests/capability_rule_contract.rs` と、4/6 の `crates/core/tests/capability_rule_check.rs` と、5/6 の `crates/core/tests/capability_rule_check_contract.rs` の 9 つだけ | `git grep -l TASK-273 -- . ':!docs/spec' ':!.agents'` |

## 現状の形（2/6）

名前は仮で、列挙・device の本物の口が入る段（TASK-157 のポインタ）で形を合わせる。`crate::fault` は参照しない（`test-support` 単独の組でもビルドされる）。

| 項目 | 内容 |
| ---- | ---- |
| 所在 | `crates/core/src/capability_rule.rs`（feature `test-support` の裏。外部依存 0） |
| 試験 | `crates/core/tests/capability_rule_stub.rs`（`test-support`）、`capability_rule_two_step.rs`（`test-support` と `fault-injection`） |

| 公開アイテム | 振る舞い | テスト |
| ------------ | -------- | ------ |
| `CapabilityTable` | 機能ごとの `CapabilityQuery` を表で返す。表に無い機能は既定値（指定なしは `S0`・理由 `None`・根拠 `None`）。同じ機能は後から足した値が当たる。`Unsupported` と `S0` を別の値として作れる | `table_returns_registered_values_and_distinguishes_s0_from_unsupported`・`table_unknown_feature_uses_default`・`later_entry_wins_for_same_feature` |
| `StepLog`・`Step` | 呼び出しを積んだ順に記録する。`CapabilityTable` に付けると読み出しのたびに `ReadFeature` を積む。`Send + Sync` | `log_records_each_read_in_order`・`types_are_send_sync_and_dyn_usable` |
| `query_after_acquire` | 列挙、取得、読み出しの順。列挙か取得が `Err` なら後の段を呼ばず `S0`。両方 `Ok` のときだけ値を返す | `enumerate_failure_returns_s0_and_stops`・`acquire_failure_returns_s0_without_reading`・`success_reads_value_after_both_steps` |
| デコレータとの区別 | 素のスタブの `Unsupported` と、`CapabilityQueryFails` 越しの `S0` は異なる値で、後者は内側を読まない | `confirmed_unsupported_differs_from_failed_query_via_decorator` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support,fault-injection --test capability_rule_stub --test capability_rule_two_step`

## 現状の形（3/6）

(1)・(2) の契約テストの組み合わせ。`src/` は変えず、2/6 の部品とデコレータを統合テストで組み合わせる。

| 項目 | 内容 |
| ---- | ---- |
| 所在 | `crates/core/tests/capability_rule_contract.rs`（`test-support` と `fault-injection`。`Cargo.toml` の `[[test]]` で宣言） |
| 確かめ方の仕組み | `StepLog` の記録で呼び出しの順と読み出しの件数を照合する。`assert_ne!` で取得の失敗の `S0` と確認済みの `Unsupported` を区別する |
| 対象外 | `unsupported_reason` にどの理由が入るかの規則と `evidence` の規則は 4/6・5/6。`query_ex` 版の 2 段の手順は無く、`query_ex` は `CapabilityTable` と `FaultInjecting` を直接呼んで確かめる。背後実装の実機は TASK-173〜TASK-175 のポインタ |

| 確かめる組み合わせ | テスト |
| ------------------ | ------ |
| 列挙の失敗（複数のエラー値 × 機能）は `S0`、記録は `Enumerate` だけ | `enumerate_failure_yields_s0_for_every_error_and_feature` |
| 取得の失敗は `S0`、読み出しは 0 件 | `acquire_failure_yields_s0_for_every_error_and_feature` |
| 両方成功で内側の値が順に読まれる | `success_returns_inner_value_in_order` |
| 取得の失敗の `S0` と確認済みの `Unsupported` は等しくない | `failed_acquisition_s0_is_not_confirmed_unsupported` |
| `CapabilityQueryFails` で包んだ device は内側を読まず `S0` | `decorated_device_with_query_failure_yields_s0_without_reading_inner` |
| `PassThrough` は内側の値を通し、失敗の `S0` と異なる | `decorated_pass_through_differs_from_query_failure` |
| `CapabilityInsufficient` は対象の機能だけ内側を読まず `Unsupported` | `decorated_capability_insufficient_overrides_only_target_feature` |
| `query_ex` の失敗は `S0`・理由 `None`・根拠 `None`、確認済みの非対応は理由つきで区別できる | `query_ex_separates_failed_query_from_confirmed_unsupported` |
| `dyn Capabilities` を包んでも区別が保たれる | `dyn_device_keeps_unsupported_and_s0_distinct` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support,fault-injection --test capability_rule_contract`

## 現状の形（4/6）

(3)・(4) の `query_ex` の戻り値 1 件の検査。名前は仮で、本物の口が入る段（TASK-157 のポインタ）で形を合わせる。`crate::fault` は参照しない。

| 項目 | 内容 |
| ---- | ---- |
| 所在 | `crates/core/src/capability_rule.rs`（`test-support` の裏。外部依存 0）、試験は `crates/core/tests/capability_rule_check.rs`（`test-support` だけ。`Cargo.toml` の `[[test]]` で宣言） |
| 対象外 | デコレータ・`dyn Capabilities` の入れ物・機能をまとめて流す形との組み合わせは 5/6。`query` と `query_ex` の level の一致の検査は無い。背後実装の実機は TASK-173〜TASK-175 のポインタ |

| 規則 | 内容 | テスト |
| ---- | ---- | ------ |
| 理由 | `Unsupported` のときだけ理由が要る。理由の値は問わない（`UnknownFeature` も通る） | `unsupported_with_any_reason_passes`・`unsupported_without_reason_is_violation`・`reason_on_non_unsupported_is_violation`・`non_unsupported_without_reason_passes` |
| 根拠 | `S3`〜`S6` は根拠が要る。`S0`〜`S2`・`Unsupported` に根拠が付いていても受け入れる | `high_levels_without_evidence_are_violations`・`high_levels_with_each_evidence_pass`・`evidence_on_low_levels_is_accepted`・`level_requires_evidence_table` |
| 違反の返し方 | 違反を理由 → 根拠の順で全部 `Err` に積む。違反は機能と戻り値を持つ。panic しない | `multiple_violations_are_reported_in_order`・`violation_carries_feature_and_query` |
| `check_query_ex` | `query_ex` を 1 回だけ呼び、通れば値を返す。`&dyn Capabilities` も渡せる。既定の `query_ex` は理由を `None` にするので、`Unsupported` を返す背後実装は `query_ex` を実装しないと通らない | `check_query_ex_reads_device_once_and_returns_value`・`check_query_ex_returns_violations_for_bad_value`・`default_query_ex_on_unsupported_fails_missing_reason`・`check_query_ex_accepts_dyn_capabilities` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support --test capability_rule_check`

## 現状の形（5/6）

(3)・(4) の検査関数の契約テストの組み合わせ。`src/` は変えず、4/6 の検査関数と 2/6 の部品とデコレータを統合テストで組み合わせる。

| 項目 | 内容 |
| ---- | ---- |
| 所在 | `crates/core/tests/capability_rule_check_contract.rs`（`test-support` と `fault-injection`。`Cargo.toml` の `[[test]]` で宣言） |
| 確かめ方の仕組み | `StepLog` の記録で読み出しの有無・件数・順を照合し、結果は `check_query_ex` の `Result` を `assert_eq!` で照合する |
| 対象外 | `query` と `query_ex` の level の一致の検査は無い。`query_ex` 版の 2 段の手順の関数は `src/` に無く、列挙 → 取得 → 検査の順は試験の中で手で組む。背後実装の実機は TASK-173〜TASK-175 のポインタ |

| 確かめる組み合わせ | テスト |
| ------------------ | ------ |
| `CapabilityQueryFails` で包むとどの機能も内側を読まず `S0` で検査を通り、素の表の確認済みの `Unsupported` と等しくない | `query_fails_masks_inner_and_passes_check` |
| `CapabilityInsufficient` は対象の機能だけ内側を読まず理由つきの `Unsupported`、対象外は内側の値（違反も）をそのまま返す | `capability_insufficient_target_passes_and_others_pass_through` |
| `PassThrough`・`CompletionUnknown`・`AllocationFails`・`RuntimeError` で包んでも、素の表と結果が全部一致する | `non_capability_scripts_match_bare_table` |
| 既定の `query_ex` の違反はデコレータ越しでも残り、`CapabilityInsufficient` の対象の機能だけ通る | `default_query_ex_violation_survives_decorator` |
| `&dyn Capabilities` と `dyn Capabilities` を包む入れ物でも結果が直接の呼び出しと一致し、`S0` と `Unsupported` の区別が保たれる | `dyn_wrappers_give_same_results` |
| 機能の並びを順に流すと、違反は該当の機能の位置にだけ現れ、読み出しの記録は並びの順 | `sweep_reports_violations_per_feature_in_order` |
| 実データの無い段階（`S1`、`S3`〜`S6` の根拠の有無）の検査の結果がデコレータ・入れ物越しでも変わらない | `levels_without_runtime_data_through_wrappers` |
| 列挙か取得が失敗すると検査を呼ばず読み出し 0 件、両方成功のときだけ `Enumerate`・`RequestDevice`・`ReadFeature` の順 | `acquire_then_check_in_order` |

確かめ方: `cargo test -p fandhe-silicon-core --features test-support,fault-injection --test capability_rule_check_contract`

## 完了の条件の確かめ（6/6）

6/6（#4069）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-273）。
結果は GPU の実機なしで実行した値で、件数は実測。本体のコード（`src/`）と試験は変えていない。

### 成果物と置き場の対応

| 成果物 | 置き場 | 本書の節 |
| ------ | ------ | -------- |
| 検査関数 | `crates/core/src/capability_rule.rs` の `check_query`・`check_query_ex`・`level_requires_evidence`・`CapabilityRuleViolation`（`test-support` の裏） | 「現状の形（4/6）」 |
| 試験 | 試験用の部品 `CapabilityTable`・`StepLog`・`Step`・`query_after_acquire` と、`crates/core/tests/` の 5 本（`capability_rule_stub.rs`・`capability_rule_two_step.rs`・`capability_rule_contract.rs`・`capability_rule_check.rs`・`capability_rule_check_contract.rs`） | 「現状の形（2/6）」〜「現状の形（5/6）」 |

- 置き場は別の crate ではなく `core` の中の `test-support` の裏のモジュール（TASK-273 の決定の記録に従う。ポインタのみ）

### 規則と試験の対応

| 規則（TASK-273 の内容の番号） | 試験（関数名） |
| ----------------------------- | -------------- |
| (1) 取得の失敗の `S0` と確認済みの `Unsupported` の区別 | `table_returns_registered_values_and_distinguishes_s0_from_unsupported`（stub）、`confirmed_unsupported_differs_from_failed_query_via_decorator`（two_step）、`failed_acquisition_s0_is_not_confirmed_unsupported`・`query_ex_separates_failed_query_from_confirmed_unsupported`・`dyn_device_keeps_unsupported_and_s0_distinct`（contract）、`query_fails_masks_inner_and_passes_check`・`dyn_wrappers_give_same_results`（check_contract） |
| (2) 2 段の手順の順序 | `enumerate_failure_returns_s0_and_stops`・`acquire_failure_returns_s0_without_reading`・`success_reads_value_after_both_steps`（two_step）、`enumerate_failure_yields_s0_for_every_error_and_feature`・`acquire_failure_yields_s0_for_every_error_and_feature`・`success_returns_inner_value_in_order`（contract）、`acquire_then_check_in_order`（check_contract） |
| (3) 理由 | `unsupported_with_any_reason_passes`・`unsupported_without_reason_is_violation`・`reason_on_non_unsupported_is_violation`・`non_unsupported_without_reason_passes`・`default_query_ex_on_unsupported_fails_missing_reason`（check）、`default_query_ex_violation_survives_decorator`（check_contract） |
| (4) 根拠 | `high_levels_without_evidence_are_violations`・`high_levels_with_each_evidence_pass`・`evidence_on_low_levels_is_accepted`・`level_requires_evidence_table`（check）、`levels_without_runtime_data_through_wrappers`（check_contract） |

- (3) は理由の値を問わない（`UnknownFeature` も通る）。各規則の説明は「現状の形」の各節に従う

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| B1 | `test-support` だけで流す試験 | `cargo test -p fandhe-silicon-core --features test-support --test capability_rule_stub --test capability_rule_check` | 順に 5・14 件。0 failed / 0 ignored |
| B2 | `test-support,fault-injection` で流す試験 | `cargo test -p fandhe-silicon-core --features test-support,fault-injection --test capability_rule_two_step --test capability_rule_contract --test capability_rule_check_contract` | 順に 4・9・8 件。0 failed / 0 ignored |
| B3 | `--all-features` | `cargo test -p fandhe-silicon-core --all-features` に 5 本の `--test` を並べた形 | B1・B2 と同じ件数。0 failed / 0 ignored |
| B4 | モジュール内の試験 | `cargo test -p fandhe-silicon-core --features test-support --lib capability_rule` | 0 件（`capability_rule.rs` に単体テストは無い） |
| B5 | `required-features` の宣言 | `crates/core/Cargo.toml` の `[[test]]` | `capability_rule_stub`・`capability_rule_check` は `test-support`、残る 3 本は `test-support` と `fault-injection`。B1・B2 の分け方と一致 |
| B6 | feature なしでは走らない | `cargo test -p fandhe-silicon-core --test capability_rule_stub` | `requires the features: test-support` で止まる |
| C1 | 置き場と制約 | `crates/core/src/lib.rs`、`cargo tree -p fandhe-silicon-core -e normal --depth 1`、`grep -c '^\[\[' deps-approved.toml` | `capability_rule` は `test-support` の裏。`forbid(unsafe_code)`。`capability_rule.rs` の `crate::fault` は先頭コメントの 1 件だけ（参照ではない）。外部依存 0。承認の記録 0 件 |
| C2 | 製品の経路からの参照 | `git grep -n -e capability_rule -e check_query -e CapabilityTable -e query_after_acquire -- crates/contract crates/exec crates/upper crates/wasm-abi wasm measure baseline-wgpu` | 0 件 |
| C3 | `Capabilities` の実装の所在 | `git grep -n -E 'impl.* Capabilities for' -- crates wasm` | `core` の `src/` に 2 件（`CapabilityTable`・`FaultInjecting<D>`）、残りは `crates/core/tests/`。`contract`・`exec`・`upper`・`wasm/` は 0 件 |
| C4 | wasm32 と本番の組 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --features test-support,fault-injection --test <5 本>`、`make build-profiles`、`make wasm-verify` | 5 本とも終了コード 0、後の 2 つも 0。`--tests` で全部を一度に検査する形は、本件と別の `test_frame_decide` が wasm32 でコンパイルできず落ちる（本件の範囲外。直していない） |
| C5 | 公開 API の検査 | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check` | いずれも終了コード 0 |
| C6 | 対象外の事実 | 「現状の形（4/6）」「現状の形（5/6）」の対象外 | `query` と `query_ex` の level が一致するかの検査は無い。背後実装の実機での確かめは TASK-173〜TASK-175 のポインタ。直さず事実として引き継ぐ |
| E1 | base の CI | `gh run list --branch main --workflow ci.yml` | 確かめた base の 1 つ前のコミット `487d857` の run（id 39510016363）は completed / success。base `a2ba112` の run（id 39510518348）は記録時に in_progress で、未確認 |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | 4 つとも終了コード 0 |

### 再実行の契機

- `crates/core/src/capability_rule.rs` の公開アイテム・規則・違反の返し方を変えたとき
- 上の 5 本の試験のファイル、または `Cargo.toml` の `[[test]]` の宣言を変えたとき
- `crates/core/src/capability.rs`・`error.rs` の能力の型と `UnsupportedReason`、`fault/decorator.rs` の `Capabilities` の実装を変えたとき
- 背後実装の `query`・`query_ex` が入ったとき（TASK-173〜TASK-175 のポインタ）、列挙・device の本物の口が入ったとき（TASK-157 のポインタ）
- `docs/spec` の gitlink が進んだとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-273）。
確かめた base は `a2ba112ecab49c380b1dda7e5f159fca6e55f670`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #4064 | 現状と前提の確認（本書） |
| 2/6 | #4065 | 実装（部品と各部品の試験） |
| 3/6 | #4066 | テスト |
| 4/6 | #4067 | 実装 |
| 5/6 | #4068 | テスト |
| 6/6 | #4069 | 確かめ（本書の「完了の条件の確かめ（6/6）」） |

## 上限の表（TASK-157）

- `CapabilityTable`（`test-support` の裏）は `limits()` を持ち、初期値は `max_buffer_bytes`・`max_dispatch_dims`・`max_texture_size` の 3 項目とも 0。`with_limits(LimitsTable)` で差し替える。`new()` の引数は変えていない
- 対応するテスト: `crates/core/tests/capability_rule_stub.rs`
- `Capabilities` の実装は必須の `limits()` を持つ。詳細は [contract-limits-table](contract-limits-table.md)

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（検査関数・スタブ・試験用の crate の名前と置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場と形は TASK-273 の決定の記録に従う（ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保ち、テスト専用の feature の裏に置く。`default` と本番の組には入れない。新しい member を足す場合も `crates/` 配下に置く
- 2/6 は部品と各部品の試験まで。ERR-50・ERR-62 の契約テストの組み合わせは 3/6 で足した
- (3)・(4) の検査関数と部品の試験は 4/6 で足した。組み合わせは 5/6 で足した
- [docs/contract/](contract/README.md) の項目ファイルは、対応するコードと公開テストを入れる段で作る
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[contract-error-types](contract-error-types.md)・[contract-device-types](contract-device-types.md)・[contract-fault-injection](contract-fault-injection.md)・[contract-feature-check](contract-feature-check.md)・[public-api-contract-check](public-api-contract-check.md)・[workspace-boundary](workspace-boundary.md)・[docs/contract/README.md](contract/README.md)
