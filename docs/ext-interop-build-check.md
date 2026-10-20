# `ext-interop` と片方の背後実装の組のビルドの確かめ（TASK-231）の所在と現状の事実

本リポのコードと公開テストから観測できる事実だけを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-231。関連: `docs/spec/04-behavior/build-profiles.md` の BUILD-14。ポインタのみ）。本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 現状と前提の確認（1/4）（TASK-231）

1/4（#7364）で、コードと公開テストから観測できる事実だけを記録した。前提・完了の条件の正は private spec 側の記録に従う（ポインタのみ）。コードは変えていない。

### 前提

| 前提 | 本リポの物 | 確かめ方 |
| ---- | ---------- | -------- |
| TASK-220 | 決定の記録は private spec 側（ポインタのみ） | `gh issue view 457 --json state`（CLOSED） |
| TASK-5 | `crates/contract/Cargo.toml` の `[features]`・`crates/contract/src/lib.rs` の `cfg`・`scripts/build-profiles.sh` | `gh issue view 30 --json state`（CLOSED） |
| TASK-18 | CI の分け方（[ci-job-split](ci-job-split.md)） | `gh issue view 72 --json state`（CLOSED） |
| TASK-112 | 機材の要らないジョブの枠（[ci-device-free-jobs](ci-device-free-jobs.md)） | `gh issue view 393 --json state`（CLOSED） |

### 現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature の宣言 | `ext-interop = []`（含意も依存も無い）。`default` に入っていない。`backend-vulkan`・`backend-cuda` は `contract-core` だけを含意し、`ext-interop` を含意しない | `sed -n '/\[features\]/,/^$/p' crates/contract/Cargo.toml`、`cargo test -p fandhe-silicon-contract --test feature_gate_scope` |
| 背後実装のモジュールの `cfg` | `backend_vulkan`・`backend_cuda` は、それぞれの feature と `not(target_arch = "wasm32")` の両方でゲートされる。組の対象は native のみ | `grep -n 'cfg(' crates/contract/src/lib.rs` |
| 背後実装の中の `ext-interop` | `backend_vulkan.rs`・`backend_vulkan/`・`backend_cuda.rs` に `ext-interop` の条件付きコンパイルは 0 件。両モジュールとも骨格のみで、相互運用の実装は無い | `grep -rn 'ext-interop\|ext_interop' crates/contract/src/backend_vulkan.rs crates/contract/src/backend_vulkan crates/contract/src/backend_cuda.rs`（出力なし） |
| `crates/` で `ext-interop` の `cfg` を持つ物 | 門の目印（`crates/contract/src/cfg_gate_doctest.rs`）と、その有効側の試験（`crates/contract/tests/feature_gate_enabled.rs`）だけ（[feature-gate-compile-check](feature-gate-compile-check.md)） | `grep -rln 'feature = "ext-interop"' crates/` |
| 組ごとの build / clippy / test | `scripts/build-profiles.sh` の `NATIVE_SETS` で `ext-interop` を含むのは、`backend-vulkan` と `backend-cuda` の両方を含む本番の組の行と、`ext-interop` 単独の行の 2 つだけ。`backend-vulkan` 単独・`backend-cuda` 単独の行はあるが、そのどちらか片方に `ext-interop` を足した組の行は無い。`WASM_SETS` に `ext-interop` を含む組は無い | `grep -n 'ext-interop' scripts/build-profiles.sh` |
| 既存の試験が持つ組の写し | `crates/contract/tests/feature_default_scope.rs` の `PRODUCTION_SETS` は、`NATIVE_SETS` のうち BUILD-1〜BUILD-4 の注記がある 4 行の写し。試験は各組から含意をたどって到達できる feature を調べ、`build-profiles.sh` の注記行と写しが一致することも読んで照合する | `grep -n 'PRODUCTION_SETS' crates/contract/tests/feature_default_scope.rs` |
| CI | `verify` ジョブが `make verify` と `make build-profiles` を呼ぶ。この件のための新しいジョブは無い | `grep -n 'make verify\|make build-profiles' .github/workflows/ci.yml` |
| 依存 | 契約の crate の依存は `fandhe-silicon-core` だけ。本段で依存の記録は変えていない | `sed -n '/\[dependencies\]/,/^$/p' crates/contract/Cargo.toml` |

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 現状と前提の確認 | #7364 |
| 2/4 | 実装 | #7365 |
| 3/4 | テスト | #7366 |
| 4/4 | 確かめ | #7367 |

親は #1152。後の段で足す振る舞いは、その PR のコードと公開テストが示し、その PR で本書に節を足す。

確かめた base は `32a9cb4`、`docs/spec` の gitlink は `f603cba`。

## 実装（2/4）（TASK-231）

上の「組ごとの build / clippy / test」の行は `32a9cb4` 時点の記録で、本節がその後の状態を示す。

`scripts/build-profiles.sh` の `NATIVE_SETS` に、片方の背後実装だけに `ext-interop` を足した 2 組を加えた。既存の両方の組（BUILD-4 の行）と合わせて、`ext-interop` を含む背後実装つきの構成は 3 つになる。

| 構成 | `NATIVE_SETS` の行 | 各組で流れるコマンド |
| ---- | ------------------ | -------------------- |
| 両方 | `contract-core,cpu-isa,cpu-ops,backend-vulkan,backend-cuda,graphics,ext-interop`（既存の BUILD-4 の行） | `cargo build`・`cargo clippy --all-targets -- -D warnings`・`cargo test`（いずれも `-p fandhe-silicon-contract --no-default-features --features <組>`） |
| Vulkan のみ | `backend-vulkan,ext-interop`（追加） | 同上 |
| CUDA のみ | `backend-cuda,ext-interop`（追加） | 同上 |

- 流れる場所は CI の `verify` ジョブの `make build-profiles` で、新しいジョブは無い
- 追加した 2 組は `backend-vulkan`・`backend-cuda` の含意で `contract-core` が入り、`cpu-isa`・`cpu-ops` は入らない（`backend-vulkan` 単独・`backend-cuda` 単独の行と同じ扱い）
- 背後実装の中の `ext-interop` の条件付きコンパイルは 0 件のまま（相互運用の実装が無いため）。門で囲むのは相互運用の実装を持つ PR で、その PR で本書に節を足す（BUILD-14 のポインタのみ）
- 手元の結果: 追加した 2 組の `cargo test` はいずれも終了コード 0（`feature_gate_enabled` の `ext_interop_gate_is_visible` と各 doctest を含む）
- 行の存在を固定する試験は 3/4、3 構成の結果の記録と完了の条件の照合は 4/4 で扱う

確かめ方:

- `grep -n 'ext-interop' scripts/build-profiles.sh`（3 構成の行が現れる）
- `cargo test -p fandhe-silicon-contract --no-default-features --features backend-vulkan,ext-interop`（`backend-cuda,ext-interop` でも同様）
- `cargo test -p fandhe-silicon-contract --test feature_default_scope`（追加行が本番の組の照合に影響しないこと）

確かめた base は `9f2a5fc`、`docs/spec` の gitlink は `f603cba`。
