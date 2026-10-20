# `default` feature とテスト・診断専用 feature の検査（TASK-226）の所在と現状の事実

本リポのコードと公開テストから観測できる事実だけを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-226。関連: BUILD-8・BUILD-65。本番の組は BUILD-1〜BUILD-4。ポインタのみ）。本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 現状と前提の確認（1/3）（TASK-226）

1/3（#7355）で、コードと公開テストから観測できる事実だけを記録した。前提・完了の条件の正は private spec 側の記録に従う（ポインタのみ）。コードは変えていない。

### 前提

| 前提 | 本リポの物 | 確かめ方 |
| ---- | ---------- | -------- |
| TASK-5 | `crates/contract/Cargo.toml` の `[features]`・`crates/contract/src/lib.rs` の `cfg`・`scripts/build-profiles.sh` | `gh issue view 30 --json state`（CLOSED） |
| TASK-18 | CI の分け方（[ci-job-split](ci-job-split.md)） | `gh issue view 72 --json state`（CLOSED） |
| TASK-112 | 機材の要らないジョブの枠（[ci-device-free-jobs](ci-device-free-jobs.md)） | `gh issue view 393 --json state`（CLOSED） |

### 現状

feature の一覧は `[features]` を正とし、本書に全部を列挙し直さない。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `default` | `contract-core`・`cpu-isa`・`cpu-ops` の 3 つ | `sed -n '/\[features\]/,/^$/p' crates/contract/Cargo.toml` |
| 既定で無効の feature | `backend-*`・`graphics`・`ext-interop`・`native-blocking-wait` は `default` に入っていない。`backend-*` は `contract-core` を要する | 同上 |
| テスト・診断専用の feature | `test-support`（core の同名 feature へ転送）・`fault-injection`（含意なし）・`artifact-inspection`（`backend-metal` を要する）。`Cargo.toml` のコメントに「テスト・診断専用（本番ビルドに含めない）」とある | 同上 |
| 既存の試験 | `crates/contract/tests/feature_gate_scope.rs` は外部 crate を使わず `Cargo.toml` の文字列を読む。対象は `native-blocking-wait` と `ext-interop` の 2 つで、確かめるのは宣言・`default` に入らないこと・他の feature の含意にならないこと。`default` の中身の完全一致と、テスト・診断専用の 3 つは確かめていない | `cargo test -p fandhe-silicon-contract --test feature_gate_scope`（終了コード 0） |
| 組ごとの build / clippy / test | `scripts/build-profiles.sh` の `NATIVE_SETS` が、本番の組の 4 組・`artifact-inspection` 単独などで流す | `grep -n 'NATIVE_SETS' -A16 scripts/build-profiles.sh` |
| 依存グラフの組 | `scripts/profile-dep-graph-check.sh` の `PROFILES` は `cpu-only` と `compute-only-metal` の 2 つ | `grep -n '"cpu-only"\|"compute-only-metal"' scripts/profile-dep-graph-check.sh` |
| 公開 API の本番の組 | `scripts/public-api-check.sh` の `CONTRACT_PRODUCTION` は 9 件で、`backend-dx12-compute` とテスト・診断専用の feature は入っていない（[public-api-contract-check](public-api-contract-check.md)） | `grep -n 'CONTRACT_PRODUCTION' -A3 scripts/public-api-check.sh` |
| CI | `verify` ジョブが `make verify` と `make build-profiles` を、`deny` ジョブが `make deny` を呼ぶ | `grep -n 'make build-profiles\|make deny\|make verify' .github/workflows/ci.yml` |

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 現状と前提の確認 | #7355 |
| 2/3 | 実装 | #7356 |
| 3/3 | CI への組み込み | #7357 |

親は #1148。後の段で足す試験の振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す。守る規約は [coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)。

確かめた base は `0568b89d3c5ddc48973ea0133aba7f0447867bf3`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。
