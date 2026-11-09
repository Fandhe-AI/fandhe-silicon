# 比較基準の別 workspace の cooperative matrix の GEMM の腕の crate（TASK-1010）の所在と本リポの現状

比較基準の別 workspace `baseline-wgpu/` にある、cooperative matrix の WGSL で行列積を流す腕の crate の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1010。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `baseline-wgpu/crates/gemm-arm`。package は `baseline-wgpu-gemm-arm`、lib は `baseline_wgpu_gemm_arm` | `baseline-wgpu/crates/gemm-arm/Cargo.toml` |
| 依存 | `wgpu`（`=30.0.1`）と、製品の `crates/core`（feature `test-support`）への path の 2 つだけ。`[dev-dependencies]` は無い | `baseline-wgpu/crates/gemm-arm/Cargo.toml`、`make deny` |
| lints | `baseline-wgpu/` の workspace の lints（`unsafe_code = "forbid"`・`missing_docs = "warn"`） | `baseline-wgpu/Cargo.toml` |
| 流れる場所 | `make baseline-verify`（fmt --check / clippy `-D warnings` / test）に含まれる。device は使わない | `scripts/baseline-verify.sh` |
| 門の run の枠 | `fandhe_silicon_core::parity::gemm_gate`（TASK-1005 の所在は [gemm-test-inputs](gemm-test-inputs.md)） | 同左 |

## WGSL と詰め物（2/4）

### WGSL

`src/lib.rs` が `include_str!` で次の 2 本を `pub const` に置く。

| 定数 | ファイル | A・B の型 | 累積と C | 要る naga の能力 |
| ---- | -------- | --------- | -------- | ---------------- |
| `COOPMAT_F32_WGSL` | `src/shaders/coopmat_f32.wgsl` | `array<f32>` | f32 | `COOPERATIVE_MATRIX` |
| `COOPMAT_F16ACC_WGSL` | `src/shaders/coopmat_f16acc.wgsl` | `array<f16>` | f32 | `COOPERATIVE_MATRIX \| SHADER_FLOAT16` |

- 束縛は `@group(0)` の 0 = A（storage, read）・1 = B（storage, read）・2 = C（storage, read_write）・3 = 定数（uniform。`m`・`k`・`n` の `u32` をこの順に持つ構造体）
- 入口の名前は 2 本とも `ENTRY_POINT`（`coopmat_gemm`）、workgroup の大きさは `WORKGROUP_SIZE`（`[32, 1, 1]`）
- 1 workgroup が C の 8x8 のタイル 1 枚を持つ（`TILE` = 8）。タイルの位置は `row0 = workgroup_id.y * 8`・`col0 = workgroup_id.x * 8`

### 詰め物と切り出し（`src/layout.rs`）

| 関数 | 入力ごとの返し |
| ---- | -------------- |
| `padded_dim(x)` | `x` を 8 の倍数へ切り上げた値。`u32` をあふれるとき `None` |
| `GemmLayout::new(m, n, k)` | 元の大きさと詰めた後の大きさを持つ値。次元が `u32` に収まらない・詰めてあふれるとき `LayoutError::DimensionOverflow`、要素数があふれるとき `LayoutError::SizeOverflow` |
| `GemmLayout::for_row(row)` | `GemmGateRow` の `m`・`n`・`k` から `new` を呼ぶ |
| `pad_a_f32` / `pad_b_f32` | Mp x Kp / Kp x Np の列。余りは `0.0`。入力の長さが元の要素数と違うとき `LayoutError::LengthMismatch` |
| `pad_a_binary16` / `pad_b_binary16` | 同じ形で、余りは `0x0000` |
| `crop_c` | 詰めた後の C（Mp x Np）から元の M x N を切り出した列。長さが違うとき `LayoutError::LengthMismatch` |
| `workgroup_count` | `[np / 8, mp / 8, 1]` |
| `constants_bytes` | 詰めた後の `m`・`k`・`n` を little-endian の `u32` でこの順に並べ、後ろ 4 バイトを 0 にした 16 バイト |

`LayoutError` は `#[non_exhaustive]` で、`Display` の文言は固定の英語。確保の失敗は `LayoutError::AllocationFailed`。

### 試験

| ファイル | 確かめる事項 |
| -------- | ------------ |
| `tests/wgsl_validate.rs` | 2 本が `wgpu::naga` の解析と検証を、上の表の能力で通る。能力が無いと拒否される。f16acc は `SHADER_FLOAT16` が無いと拒否される。入口が 1 つで、名前と workgroup の大きさが定数と一致する |
| `tests/layout.rs` | 5 行の詰めた後の大きさ（`odd37x41x53` は `(m, n, k)` = `(40, 48, 56)`、ほかは変わらない）、`padded_dim` の境界、`u32` の近くのあふれが `Err`、詰め物の 0 の位置と元の値の保持、詰めて切り出すと元に戻ること、長さの不一致が `Err`、workgroup の数、定数のバイト列 |

### 確かめ方

worktree のルートで実行する。

- `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm`
- `make baseline-verify`
- `make deny`
