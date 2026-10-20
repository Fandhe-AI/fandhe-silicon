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

## 実装（2/3）（TASK-226）

2/3（#7356）で、契約の crate の features を読む試験を足した。CI への組み込みの確かめは 3/3（#7357）。

### 所在と読み方

| 項目 | 内容 |
| ---- | ---- |
| 試験 | `crates/contract/tests/feature_default_scope.rs` |
| 読み方 | `cargo metadata --format-version 1 --no-deps` を `env!("CARGO")` で起動し、`fandhe-silicon-contract` の `features` を試験の中の小さな JSON 読み取りで取り出す（外部 crate なし）。該当 package が 0 件・2 件以上、形の違いは panic で止める |
| feature の組 | `cargo metadata` は有効な feature に左右されないので、どの組で流しても結果は同じ |
| 既存の試験との分担 | `feature_gate_scope.rs`（TASK-233）は `native-blocking-wait`・`ext-interop` の範囲として残す |

### 判定

| 判定 | 内容 |
| ---- | ---- |
| `default` | `contract-core`・`cpu-isa`・`cpu-ops` の 3 つと集合・件数が一致する |
| 既定で無効 | `backend-metal`・`backend-vulkan`・`backend-cuda`・`graphics`・`ext-interop`・`native-blocking-wait` が宣言されていて、`default` の含意の閉包に入らない |
| テスト・診断専用 | `test-support`・`fault-injection`・`artifact-inspection` が宣言されていて、`default` と本番の 4 組（BUILD-1〜BUILD-4）の含意の閉包に入らない。`artifact-inspection` は `backend-metal` を要する |
| 本番の組の写し | 試験の中の 4 組は `scripts/build-profiles.sh` の `NATIVE_SETS` の注記つき 4 行と一致することを別の試験が確かめる |

他の crate への転送（`/` を含む項目）と `dep:` は辿らない。`backend-dx12-compute` は判定の対象に入れていない。

### 対応するテスト

| 試験 | 内容 |
| ---- | ---- |
| `real_manifest_has_no_violations` | 実物の features の違反が 0 件 |
| `production_sets_match_build_profiles_script` | 本番の組の写しとスクリプトの一致 |
| `json_reader_*`・`contract_features_*` | JSON 読み取りと package の取り出し（不正入力・0 件・重複・文字列でない項目） |
| `control_*` | 陽性対照。実物の features を 1 か所だけ変えると違反が出る（既定で無効な 6 つ・テスト・診断専用の 3 つを `default` に足す、含意で届く 3 経路、`artifact-inspection` を空にする、`default` の削除と縮小、宣言の削除） |

### 確かめ方

- `cargo test -p fandhe-silicon-contract --test feature_default_scope`（終了コード 0。15 件）
- 手元の陽性対照: `crates/contract/Cargo.toml` の `default` に `graphics` を足すと `real_manifest_has_no_violations` が失敗する（終了コード 101。`default` の不一致と `graphics is reachable from default`）。`fault-injection` を足しても同様に失敗する（`fault-injection is reachable from default`）。どちらも確かめたあと戻した
- 流れる場所: `make verify`（`--all-features`）と、`scripts/build-profiles.sh` の各組の `cargo test -p fandhe-silicon-contract`

確かめた base は `cc2b977`。
