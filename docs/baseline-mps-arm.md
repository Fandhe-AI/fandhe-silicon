# 比較基準の MPS の腕（TASK-1011）の所在と本リポの現状

比較基準の別 workspace `baseline-wgpu/` にある、MPS の腕の lib crate の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1011。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `baseline-wgpu/crates/mps`。package は `baseline-wgpu-mps`、lib は `baseline_wgpu_mps` | `baseline-wgpu/crates/mps/Cargo.toml` |
| 依存 | 製品の `crates/core`（feature `test-support`）への path だけ。外部依存と `[dev-dependencies]` は無い | `baseline-wgpu/crates/mps/Cargo.toml`、`make deny` |
| lints | workspace の表を継ぐ（`unsafe_code = "forbid"`） | `baseline-wgpu/crates/mps/Cargo.toml` |
| 流れる場所 | `make baseline-verify`（fmt --check / clippy `-D warnings` / test）に含まれる。device は使わない | `scripts/baseline-verify.sh` |
| 門の run の枠 | `fandhe_silicon_core::parity::gemm_gate`（TASK-1005 の所在は [gemm-test-inputs](gemm-test-inputs.md)） | 同左 |
| 別 workspace の置き場 | TASK-7 の所在は [baseline-wgpu](baseline-wgpu.md) | 同左 |

## セルの計画と入力の詰め替え（1/3）

どの OS でもコンパイルされる純粋な関数だけを `src/lib.rs` に置く。

### `plan(cell, output)`

| セルの腕 | セルの dtype | `OutputKind` | 返し |
| -------- | ------------ | ------------ | ---- |
| `Mps` 以外 | 任意 | 任意 | `PlanError::NotMpsCell` |
| `Mps` | `F32` | `SameAsInput` | 入力 `F32`・出力 `F32` |
| `Mps` | `F32` | `F32` | `PlanError::OutputNotForCell` |
| `Mps` | `F16Acc` | `SameAsInput` | 入力 `F16`・出力 `F16` |
| `Mps` | `F16Acc` | `F32` | 入力 `F16`・出力 `F32` |

返す `MpsPlan` の `m`・`n`・`k` はセルの行の値。

### `check_row_bytes(columns, element, row_bytes, rows)`

- `row_bytes` が `columns × size` より小さいとき `PlanError::RowBytesTooSmall`、`size` の倍数でないとき `PlanError::RowBytesNotAligned`、計算があふれるとき `PlanError::SizeOverflow`
- 満たすとき `rows × row_bytes`（バッファの長さ）を返す。0 列は `row_bytes` が `size` の倍数なら受け付ける

### 詰め替え

| 関数 | 返し |
| ---- | ---- |
| `pack_f32` / `pack_f16` | 行優先の密な列を、行の間隔つきの little-endian のバイト列にする。行の末尾の余りは 0。列の長さが `rows × columns` と違うとき `PlanError::LengthMismatch` |
| `unpack_f32` / `unpack_f16` | 逆向き。各行の先頭の `columns × size` バイトだけを読み、余りは読まない。バイト列の長さが `check_row_bytes` の値と違うとき `PlanError::LengthMismatch` |

行の間隔の不備とあふれは `check_row_bytes` と同じ `Err`。確保の失敗は `PlanError::AllocationFailed`。`PlanError` は `#[non_exhaustive]` で、`Display` の文言は固定の英語。

### 試験

| ファイル | 確かめる事項 |
| -------- | ------------ |
| `tests/plan.rs` | `Mps` の全セルの計画（大きさと要素の型）、`F16Acc` のセルの f32 の出力、`F32` のセルの `OutputNotForCell`、`Mps` 以外の腕の全セルの `NotMpsCell`、要素のバイト数、`check_row_bytes` の境界、詰めて戻すと元に戻ること（行の間隔が最小のときと余りがあるとき）、余りが 0 で埋まること、余りを読まないこと、長さの不一致と行の間隔の不備が `Err` |

### 確かめ方

worktree のルートで実行する。

- `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-mps --test plan`
- `make baseline-verify`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | crate とセルの計画・入力の詰め替え | #6325 |
