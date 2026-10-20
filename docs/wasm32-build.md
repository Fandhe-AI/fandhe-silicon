# wasm32 向けの cdylib のビルド

wasm32 向けの cdylib（製品の workspace の `crates/wasm-abi`）の所在と、scalar 版のビルドについて、本リポのコードから説明できる現状を記録する。
記録の所在は private spec の TASK-237 とその前提（`docs/spec/05-tasks.md`。ポインタのみ。内容は転記しない）。
親 Issue は #462、この文書は 1/4（#3940）・2/4（#3941）の範囲。型の存在の確かめ・CI への組み込みは後続の sub-issue（#3942・#3943）で追記する。

## 所在

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| crate | `crates/wasm-abi/`（package `fandhe-silicon-wasm-abi`。名前は仮。`publish = false`） | `ls crates/wasm-abi` |
| 依存 | 契約の crate（`fandhe-silicon-contract`、`default-features = false`）だけ。外部依存は 0 | `cargo tree -p fandhe-silicon-wasm-abi --no-default-features` |
| feature | `contract-core`・`cpu-isa`・`cpu-ops` を契約の crate へそのまま転送する（`default` はこの 3 つ） | `crates/wasm-abi/Cargo.toml` |
| `crate-type` | `["cdylib", "lib"]`。`lib` は公開 API の検査が lib target を要るために入れている | `cargo metadata --format-version 1 --no-deps` の kind |
| export・公開アイテム | 無い（入口の `//!` と `forbid(unsafe_code)` のみ） | `crates/wasm-abi/src/lib.rs` |
| 契約の crate の CPU 向けの feature | まだコードを持たない | `crates/contract/src` |
| 別 workspace `wasm/` の glue | 別の物。本 crate は `wasm/` を参照しない | [workspace-boundary](workspace-boundary.md) |

## scalar 版のビルド（BUILD-19 のポインタ）

`RUSTFLAGS` を付けずに、ルートで次を流す。

```sh
env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS cargo build -p fandhe-silicon-wasm-abi \
  --no-default-features --features contract-core,cpu-isa,cpu-ops \
  --target wasm32-unknown-unknown --release --locked
```

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 成果物 | `target/wasm32-unknown-unknown/release/fandhe_silicon_wasm_abi.wasm` が実在し非空 | `ls -l` |
| 形式 | 先頭 4 バイトが `00 61 73 6d` | `od -An -tx1 -N4 <成果物>` |
| rustflags | リポジトリに `.cargo/config.toml` は無く、scalar 版は追加の rustflags を使わない | `ls .cargo`（無いこと） |
| rustflags（simd128 版） | `RUSTFLAGS` に `-C target-feature=+simd128` を付ける（下の節） | 下の節 |

## simd128 版のビルド（BUILD-20 のポインタ）

同じ feature の組で、`RUSTFLAGS` に `-C target-feature=+simd128` を付け、`--target-dir` を分けて、ルートで次を流す。

```sh
env -u CARGO_ENCODED_RUSTFLAGS RUSTFLAGS="-C target-feature=+simd128" \
  cargo build -p fandhe-silicon-wasm-abi \
  --no-default-features --features contract-core,cpu-isa,cpu-ops \
  --target wasm32-unknown-unknown --release --locked \
  --target-dir target/wasm32-simd128
```

`CARGO_ENCODED_RUSTFLAGS` が環境にあると、cargo は `RUSTFLAGS` より先にそちらを使う。`+simd128` が付かないまま scalar 版と同じものができるので、外してから流す。

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 成果物 | `target/wasm32-simd128/wasm32-unknown-unknown/release/fandhe_silicon_wasm_abi.wasm` が実在し非空 | `ls -l` |
| 形式 | 先頭 4 バイトが `00 61 73 6d` | `od -An -tx1 -N4 <成果物>` |
| `--target-dir` を分ける理由 | 2 版の成果物は target-dir から下が同じ相対パスで、分けないと後から作った方が前の方を上書きする | 2 つのパスが別であること |
| 置き場 | `target/` の下なので `.gitignore` の `/target/` に入る | `git check-ignore -v target/wasm32-simd128/x` |

2 版の成果物の違いの確かめ・型が含まれることの確かめ・`cfg` の確かめは 3/4（#3942）、CI への組み込みは 4/4（#3943）で書き足す。
