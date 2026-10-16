# baseline-wgpu

比較基準の wgpu を置く、基盤の外の別 Cargo workspace（spec のポインタ: TASK-7・AC-2・BUILD-50）。

## 役割

- 基盤の workspace（ルートの `crates/` の 4 crate）の依存グラフに wgpu 系を入れないため、比較基準は本ディレクトリの独立した workspace に置く
- ルートの `Cargo.toml` は `exclude` で本ディレクトリを切り離し、本 workspace は自身の `[workspace]` と `Cargo.lock` を持つ
- `make check` / `make verify` はルートの workspace が対象で、本 workspace は範囲外。`make deny`（CI の `deny` ジョブも呼ぶ）は、本 workspace に `baseline-wgpu/deny.toml`（wgpu 系の禁止だけを外した設定）で cargo-deny をかけ、依存の承認の記録も検査する。ビルドはしない

## 現状

- 外部依存は `wgpu`（`=30.0.1` の完全固定）のみ。承認の記録は本ディレクトリの `deps-approved.toml`
- lib crate（`crates/public-api`、`baseline-wgpu-public-api`）は公開 API の戻り値に `wgpu::Backends` を出す。製品の公開 API の検査を `--manifest-path` で本 workspace に当てたときの入力で、製品からは呼ばれない
- smoke bin（`crates/smoke`）は `wgpu::Instance` で全 backend のアダプタを列挙し、名前・backend・device type を出力する。アダプタが 0 件でも正常終了する
- 依存の追加・更新はユーザー承認が要る（[dependency-policy](../.claude/rules/dependency-policy.md)）。CI には組み込んでいない
- 版の記録は [docs/baseline-wgpu.md](../docs/baseline-wgpu.md) を参照
- 本 workspace の承認の記録の検査: `make deny` が `scripts/deps-check.sh baseline-wgpu/Cargo.toml` を呼ぶ（`make deps-check` はルートのみ）

## 実行方法

```bash
cargo run --locked --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-smoke
```
