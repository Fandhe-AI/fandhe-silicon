# baseline-wgpu

比較基準の wgpu を置く、基盤の外の別 Cargo workspace（spec のポインタ: TASK-7・AC-2・BUILD-50）。

## 役割

- 基盤の workspace（ルートの `crates/` の 4 crate）の依存グラフに wgpu 系を入れないため、比較基準は本ディレクトリの独立した workspace に置く
- ルートの `Cargo.toml` は `exclude` で本ディレクトリを切り離し、本 workspace は自身の `[workspace]` と `Cargo.lock` を持つ
- `make check` / `make verify` はルートの workspace が対象で、本 workspace は範囲外。`make deny`（CI の `deny` ジョブも呼ぶ）は、本 workspace に `baseline-wgpu/deny.toml`（wgpu 系の禁止だけを外した設定）で cargo-deny をかけ、依存の承認の記録も検査する。ビルドはしない

## 現状

- 外部依存は `wgpu`（`=30.0.1` の完全固定）のみ。承認の記録は本ディレクトリの `deps-approved.toml`
- lib crate（`crates/public-api`、`baseline-wgpu-public-api`）は、公開の struct の field、trait のメソッドの引数と戻り値、その trait の impl に wgpu の型を出す。製品の公開 API の検査を `--manifest-path` で本 workspace に当てたときの入力で、製品からは呼ばれない
- smoke bin（`crates/smoke`）は `wgpu::Instance` で全 backend のアダプタを列挙し、名前・backend・device type を出力する。アダプタが 0 件でも正常終了する
- 依存の追加・更新はユーザー承認が要る（[dependency-policy](../.claude/rules/dependency-policy.md)）。
- `make baseline-verify`（CI の `deny` ジョブも呼ぶ）が本 workspace の fmt --check / clippy / test を流す
- 版の記録は [docs/baseline-wgpu.md](../docs/baseline-wgpu.md) を参照
- 本 workspace の承認の記録の検査: `make deny` が `scripts/deps-check.sh baseline-wgpu/Cargo.toml` を呼ぶ（`make deps-check` はルートのみ）
- lib crate（`crates/gemm-arm`、`baseline-wgpu-gemm-arm`）は、cooperative matrix の WGSL 2 本（f32・f16acc）と、詰め物・切り出し・起動の引数を作る純粋関数を持つ。試験用に製品の `crates/core`（feature `test-support`）へ path で依存する。device には触れない（所在は [docs/baseline-gemm-arm.md](../docs/baseline-gemm-arm.md)）
- lib crate（`crates/mps`、`baseline-wgpu-mps`）は、セルの計画と行の間隔つきの詰め替えの純粋関数を持つ。試験用に製品の `crates/core`（feature `test-support`）へ path で依存する（所在は [docs/baseline-mps-arm.md](../docs/baseline-mps-arm.md)）

## 実行方法

```bash
cargo run --locked --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-smoke
```

## 今の値の補い（base d082bc1）

上の節は書き換えていない。以下は base `d082bc1` で確かめた今の値。

| 節・行 | 前の値 → 今の値 | 確かめ方 |
| ------ | --------------- | -------- |
| 「役割」の「基盤の workspace（ルートの `crates/` の 4 crate）」 | 4 crate → 5 crate（`crates/core`・`crates/contract`・`crates/exec`・`crates/upper`・`crates/wasm-abi`） | ルート `Cargo.toml` の `members`（14 行目） |
| 「現状」の「本 workspace の承認の記録の検査: `scripts/deps-check.sh baseline-wgpu/Cargo.toml`」 | `scripts/deps-check.sh baseline-wgpu/Cargo.toml` を呼ぶ → `make deny` は `scripts/deps-check.sh --allow-path-root "${ROOT_DIR}/crates" "${ROOT_DIR}/baseline-wgpu/Cargo.toml"` の形で呼ぶ（`crates/` 配下への path だけを免除する）。`make deps-check` はルートのみのまま | `scripts/deny.sh` の 115〜116 行目 |
| 「現状」の `crates/gemm-arm` の「device には触れない」 | device には触れない → `crates/gemm-arm` に device の経路（`src/device.rs`）と bin `baseline-wgpu-gemm-gate` がある。device の要る試験（`tests/device_gate.rs` の 4 本）は `#[ignore]` で、既定の `cargo test` と `make baseline-verify` では流れない。所在は [docs/baseline-gemm-arm.md](../docs/baseline-gemm-arm.md) | `baseline-wgpu/crates/gemm-arm/Cargo.toml` の `[[bin]]`（18〜20 行目）、`grep -n 'ignore' baseline-wgpu/crates/gemm-arm/tests/device_gate.rs` |

## 今の値の補い（base 5fa953d）

記録時点の表と既存の節は書き換えない。以下は base `5fa953d` で観測した今の値（実行環境は GPU なしの Linux）。

| 節・行 | 前の値 → 今の値 | 確かめ方 |
| ------ | --------------- | -------- |
| 「現状」の 1 つ目の項目（`:13`） | 外部依存は `wgpu`（`=30.0.1` の完全固定）のみ → registry の外部依存は `wgpu` に加えて、`crates/mps` が macOS 向けだけで持つ objc2 系 3 件（`objc2`・`objc2-metal`・`objc2-core-graphics`。完全固定）。承認の記録は本ディレクトリの `deps-approved.toml`（4 件） | `grep -c '^\[\[approval\]\]' baseline-wgpu/deps-approved.toml`（4 が返る）、`baseline-wgpu/crates/mps/Cargo.toml` |
| 同 `crates/mps` の項目（`:21`） | セルの計画と行の間隔つきの詰め替えの純粋関数を持つ → これに加えて、macOS でだけコンパイルされる `run_matmul`（`#[cfg(target_os = "macos")]`。`src/mps.rs` が MPS の行列積を 1 回投入して出力を返す）と、macOS 向けの objc2 系の依存を持つ。device の要る試験は 2 本（`tests/mps_gate.rs`・`tests/mps_gate_f32_output.rs`。どちらも `#[ignore]`）。`unsafe` を持つのは `src/mps.rs` だけで、個数は `tests/unsafe_count.rs` が照合する | `grep -n 'run_matmul' baseline-wgpu/crates/mps/src/lib.rs`、`grep -n 'ignore' baseline-wgpu/crates/mps/tests/mps_gate*.rs` |
