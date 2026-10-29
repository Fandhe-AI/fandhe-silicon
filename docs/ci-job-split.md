# CI の分け方（TASK-18）の決定の記録の所在と本リポの CI の現状

TASK-18 の決定の記録の所在と、本リポの CI（`.github/workflows/ci.yml`）のコードで確かめられる現状を案内する。
spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-18）
- 本リポには決定の内容・経緯を置かない。完了の条件との対応と確認の記録も spec 側の TASK-18 の状態を正とする

## 本リポの CI の現状

`ci.yml` のジョブ（`pull_request` と `main` への `push` で発火）。

| ジョブ ID | 表示名（check 名） | runner | 呼んでいるもの |
| --------- | ------------------ | ------ | -------------- |
| `check` | `make check (editorconfig-checker + shellcheck)` | `ubuntu-latest` | `make check` |
| `test-hooks` | `hook script tests (<os>)`（`ubuntu-latest`・`macos-latest` の 2 件） | matrix の `ubuntu-latest`・`macos-latest` | `make test-hooks`、macOS の標準 bash 3.2 での `scripts/hooks/test-hooks.sh`・`scripts/test-session-record-check.sh`・`scripts/test-metal-session.sh`、`make test-session-record-check`、`make test-metal-session`、macOS の標準 bash 3.2 での `scripts/test-record-summary.sh` |
| `verify` | `make verify (fmt + clippy + test)` | `ubuntu-latest` | `make verify`、`make build-profiles` |
| `deny` | `make deny (cargo-deny)` | `ubuntu-latest` | `make deny`、`make measure-verify`、`make wasm-verify`、`make deps-check`、`make test-deps-check`、`make test-inventory-trace-check`、`make test-session-record-check`、`make test-perf-record-check`、`make test-metal-session`、`make test-record-summary`、`make ci-frame-check`、`make test-ci-frame-check`、`make public-api-check`、`make test-public-api-check`、`make public-name-check`、`make test-public-name-check`、`make public-api-sdk-check`、`make test-public-api-sdk-check`、`make core-deps-check`、`make test-core-deps-check`、`make public-api-webgpu-check`、`make test-public-api-webgpu-check`、`make test-dep-graph-wgpu-check`、`make baseline-positive-control`、`make test-baseline-positive-control`、`make upper-layer-check`、`make test-upper-layer-check`、`make test-public-api-diff`、`make test-rerun-compare`、`make test-wasm-size`、`make test-wasm-build-check`、`make wasm-backend-check`、`make test-wasm-backend-check` |
| `wasm-macos` | `wasm32 build + size + node (macos-latest)` | `macos-latest`（arm64） | `make wasm-verify`、`make wasm-size`、`make wasm-node-check`（版固定・SHA256 検証済みの Node と wasm-bindgen の CLI を導入。キャッシュなし） |
| `verify-macos` | `make verify + build-profiles (macos-latest)` | `macos-latest`（arm64） | `make verify`、`make build-profiles`（`backend-metal` を有効にしたビルド・lint・device の要らない試験。device の要る試験と `xcrun metal` は流さない。キャッシュなし。ポインタ: TASK-18 の記録、TASK-158） |
| `pr-title` | `PR title (Conventional Commits)`（`pull_request` のみ） | `ubuntu-latest` | `scripts/hooks/commit-msg-check.sh` |
| `ci-complete` | `ci-complete` | `ubuntu-latest` | 上の 7 ジョブの結果の検証（`needs` に 7 件） |

確かめ方: `grep -n -E "^  [a-z-]+:|runs-on|make |needs" .github/workflows/ci.yml`

| 事実 | 確かめ方 |
| ---- | -------- |
| `ci.yml` の `runs-on` は GitHub ホステッドの `ubuntu-latest` と、`test-hooks` の matrix と `wasm-macos`・`verify-macos` の `macos-latest` だけ。self-hosted の指定は無い | `grep -n -E "runs-on|self-hosted" .github/workflows/ci.yml` |
| `verify-macos` は ruleset の必須チェックに登録している（`ci-complete` の `needs` にも入っている） | `sed -n '/^  ci-complete:/,$p' .github/workflows/ci.yml` |
| GPU・実機を要るジョブは `ci.yml` に無い | `grep -n -i gpu .github/workflows/ci.yml`（0 件） |
| `ci-complete` は集約ジョブで、`needs` の結果が success でなければ失敗にする（`pr-title` は `pull_request` 以外でだけ skipped を許す） | `sed -n '/^  ci-complete:/,$p' .github/workflows/ci.yml` |
| wasm32 のビルドの確かめは `verify` ジョブの `make build-profiles` が呼ぶ `scripts/build-profiles.sh` にある（`wasm32-unknown-unknown` に対する `cargo check` と、`crates/wasm-abi` の scalar 版・simd128 版の release ビルド。target が無ければ skip せず失敗する） | `grep -n -E "WASM_TARGET|WASM_SETS|WASM_ABI_PKG" scripts/build-profiles.sh` |

## 実機が要る試験の扱い

- 実機 GPU が必要なテストの分け方は [coding-rust](../.claude/rules/coding-rust.md) の「テスト」の節に従う
- 実機での実行の条件と結果の扱いは spec 側の決定に従う（ポインタ: `docs/spec/05-tasks.md` の TASK-18・TASK-19）

## 本リポでの反映の方針

- 機材の要らないジョブを足す・変える作業は、それを受け持つタスクの PR で行う（ポインタ: TASK-112）
    - 枠の対象・分類・失敗の扱い・検査と make ターゲットの対応は [ci-device-free-jobs](ci-device-free-jobs.md)
- ジョブを足すときの手順（`ci-complete` の `needs`・必須チェック）は [github-actions](../.claude/rules/github-actions.md) に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [github-actions](../.claude/rules/github-actions.md)
- [implementation-scope](implementation-scope.md)
