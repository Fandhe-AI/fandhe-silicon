# baseline-wgpu

比較基準の wgpu を置く、基盤の外の別 Cargo workspace（spec のポインタ: TASK-7・AC-2・BUILD-50）。

## 役割

- 基盤の workspace（ルートの `crates/` の 4 crate）の依存グラフに wgpu 系を入れないため、比較基準は本ディレクトリの独立した workspace に置く
- ルートの `Cargo.toml` は `exclude` で本ディレクトリを切り離し、本 workspace は自身の `[workspace]` と `Cargo.lock` を持つ
- `make check` / `make verify` / `make deny` はルートの workspace が対象で、**本 workspace は監査しない**（`make deny` の範囲外）

## 現状

外部依存 0 の骨格のみ。比較基準は稼働していない。wgpu の依存追加はユーザー承認後に `=x.y.z` 完全固定で行う
（[dependency-policy](../.claude/rules/dependency-policy.md)）。

## 実行方法

```bash
cargo run --locked --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-smoke
```
