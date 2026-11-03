# `cpu_ops` の公開の型と feature の有無の確かめ（TASK-2457）

## feature `cpu-ops` を外した構成のコンパイル失敗の試験（1/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2457。親 Issue は #1074、この節（1/4）は #6962。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 目印の型 | `CpuOpsGate`（`crates/contract/src/cfg_gate_doctest.rs`。先例は TASK-233 のポインタ） |
| fence の切り替え | `cpu-ops` 無効で `compile_fail`、有効で通常の doctest |
| snippet | `use fandhe_silicon_contract::cpu_ops;` と `fn probe(_: &dyn cpu_ops::CpuOps) {}` の 2 行 |
| 有効側の試験 | `cpu_ops_gate_is_visible`（`crates/contract/tests/feature_gate_enabled.rs`） |
| `cpu_ops` の所在 | `docs/cpu-ops.md`（TASK-1351 のポインタ） |

### 流れる構成

- `compile_fail` で流れるのは `cpu-ops` を含まない組（`scripts/build-profiles.sh` の `NATIVE_SETS` のうち `cpu-ops` を含まない組。手元では `--no-default-features --features contract-core`）
- 通常の doctest として通るのは既定の組、`cpu-ops` を含む組、`--all-features`（`make verify`）

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --doc --no-default-features --features contract-core
cargo test -p fandhe-silicon-contract --doc
cargo test -p fandhe-silicon-contract --all-features --test feature_gate_enabled
```

記録時点の値（コミット `dc8b673` の上）の観測は次のとおり。

- 1 本目は `CpuOpsGate ... compile fail ... ok`
- 2 本目は `CpuOpsGate ... ok`（`9 passed; 0 failed`）
- 3 本目は `cpu_ops_gate_is_visible ... ok`（`3 passed; 0 failed`）

## `contract-core` だけの組（2/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2457。親 Issue は #1074、この節（2/4）は #6963。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 足した組 | `scripts/build-profiles.sh` の `NATIVE_SETS` の `"contract-core"` の行 |
| 流れる段 | `cargo build`・`cargo clippy --all-targets -D warnings`・`cargo test`（`--no-default-features --features contract-core`） |
| 流れる場所 | `make build-profiles`（CI の `verify`・`verify-macos` ジョブ） |

### 振る舞い

この組の `cargo test` の doctest で、`CpuOpsGate` が `compile fail ... ok` になる。

### 確かめ方

```sh
cargo build -p fandhe-silicon-contract --no-default-features --features contract-core
cargo clippy -p fandhe-silicon-contract --all-targets --no-default-features --features contract-core -- -D warnings
cargo test -p fandhe-silicon-contract --no-default-features --features contract-core
grep -n '"contract-core"' scripts/build-profiles.sh
```
