# wasm32 のビルドと Node での照合の土台（TASK-22）の所在と本リポの現状

TASK-22（親 Issue #368）の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-22）。本リポには内容・理由・経緯を置かない
- 前提のタスク（ポインタのみ）と、対応する本リポの物

| 前提 | 本リポの物 |
| ---- | ---------- |
| TASK-4 | ルート `Cargo.toml` の `members`・`exclude` と [workspace-boundary](workspace-boundary.md) |
| TASK-5 | `crates/contract/Cargo.toml` の `[features]`、`crates/contract/src/lib.rs` の `cfg`、`scripts/build-profiles.sh` |
| TASK-112 | [ci-device-free-jobs](ci-device-free-jobs.md)、`scripts/ci-frame-check.sh` |

- `docs/spec` は submodule（`update = none`）。指している版は `git ls-tree origin/main docs/spec` で確かめる

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| wasm32 の target | `rust-toolchain.toml` の `targets` は `wasm32-unknown-unknown` | `grep -n targets rust-toolchain.toml` |
| wasm32 の check | `scripts/build-profiles.sh` の `WASM_SETS` は 4 組。`fandhe-silicon-contract` を `cargo check --target wasm32-unknown-unknown --no-default-features` で確かめるだけ（build・link・test はしない）。target が無ければ skip せず失敗する | `grep -n -e WASM_SETS -e 'cargo check' -e 'not installed' scripts/build-profiles.sh` |
| wasm32 の check の呼び出し元 | `make build-profiles`、CI の `verify` ジョブ | `grep -n build-profiles Makefile .github/workflows/ci.yml` |
| wasm32 での背後実装 | Vulkan・CUDA のモジュールは `not(target_arch = "wasm32")` で外れる。Metal は feature のみでゲートされる | `grep -n wasm32 crates/contract/src/lib.rs` |
| CPU 部分 | `cpu-isa`・`cpu-ops` の feature は `crates/contract/Cargo.toml` にあり `default` に入るが、それでゲートされるモジュール・コードは無い。`crates/contract/src` は `lib.rs` と `backend_*` だけ | `ls crates/contract/src`、`grep -n 'pub mod' crates/contract/src/lib.rs`（背後実装の 3 つだけ）、`git grep -n -e 'feature = "cpu-' -- crates`（0 件）、`grep -rn -i cpu crates/contract/src`（0 件） |
| cdylib | どの manifest にも `crate-type` の指定は無い | `git grep -n crate-type -- '*.toml'`（0 件） |
| 製品の workspace | `members` は 4 crate、`exclude` は `baseline-wgpu` と `measure` | `grep -n -e members -e exclude Cargo.toml` |
| 別 workspace の先例 | `measure/`・`baseline-wgpu/` は自身の `Cargo.toml`・`Cargo.lock`・`deps-approved.toml` を持つ（`baseline-wgpu/` は自身の `deny.toml` も持つ） | `ls measure baseline-wgpu`、[measurement-tool](measurement-tool.md)・[baseline-wgpu](baseline-wgpu.md) |
| path の検査 | `scripts/deny.sh` の `ALLOWED_ROOT` は `crates/`（製品の workspace の path が外を指さないことの検査） | `grep -n ALLOWED_ROOT scripts/deny.sh` |
| 別 workspace の依存の検査 | `scripts/deny.sh` が `measure/`・`baseline-wgpu/` に cargo-deny をかけ、`baseline-wgpu/` の deps-check も呼ぶ。`measure/` の deps-check は `scripts/measure-verify.sh` が呼ぶ | `grep -n -e 'cargo deny' -e deps-check scripts/deny.sh scripts/measure-verify.sh` |
| deps-check の対象 | `scripts/deps-check.sh` は manifest のパスを引数で受け、その workspace ルート直下の `deps-approved.toml` と突き合わせる | `sed -n 1,20p scripts/deps-check.sh` |
| deps-approved.toml の件数 | ルート 0 件・`measure/` 0 件・`baseline-wgpu/` 1 件 | `grep -c '^\[\[approval\]\]' deps-approved.toml measure/deps-approved.toml baseline-wgpu/deps-approved.toml` |
| CI のジョブ | `check`・`test-hooks`・`verify`・`deny`・`pr-title`・`ci-complete` | `grep -n -E '^  [a-z-]+:$' .github/workflows/ci.yml` |
| CI の runner | `runs-on` は `ubuntu-latest` と `test-hooks` の matrix（`ubuntu-latest`・`macos-latest`）。`ci-frame-check.sh` の許可ラベルも同じ 2 つ | `grep -n runs-on .github/workflows/ci.yml`、`grep -n ALLOWED_LABELS scripts/ci-frame-check.sh` |
| CI の Node | `ci.yml` に Node の導入・実行は無い。wasm32 に触れるのは `make build-profiles` の 1 ステップだけ | `grep -n -i -e node -e wasm .github/workflows/ci.yml` |

## 本リポでの反映の方針

- 本書は現状の事実だけを置く。形の具体は、受け持つ段のコードと公開テストの PR で入れ、その PR で本書に節を足す
- 製品の workspace の外に置く物は [workspace-boundary](workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)、依存は [dependency-policy](../.claude/rules/dependency-policy.md)、CI に足す扱いは [ci-device-free-jobs](ci-device-free-jobs.md)・[github-actions](../.claude/rules/github-actions.md) に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[implementation-scope](implementation-scope.md)・[ci-job-split](ci-job-split.md)・[test-frame](test-frame.md)
