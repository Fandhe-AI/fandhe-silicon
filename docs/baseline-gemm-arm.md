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
| lints | crate の自前の表（下の節「unsafe の所在」） | `baseline-wgpu/crates/gemm-arm/Cargo.toml` |
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

## device の経路と門の run の bin（3/4）

### 要る機能と無いときの返し

| セル | 要る機能（アダプタ） | 無いときの返し |
| ---- | -------------------- | -------------- |
| f32 | `EXPERIMENTAL_COOPERATIVE_MATRIX` | device を要求する前に `ArmError::MissingFeatures` |
| f16acc | `EXPERIMENTAL_COOPERATIVE_MATRIX \| SHADER_F16` | 同上 |

アダプタが無いときは `ArmError::NoAdapter`。別の経路で計算しない。

### device の経路（`src/device.rs` の `run_g3`）

| 手順 | 内容 |
| ---- | ---- |
| 1 | セルが g3 でなければ `ArmError::WrongArm`、dtype と入力の種類が合わなければ `ArmError::InputMismatch`（どちらも device に触れない） |
| 2 | `GemmLayout::for_row` と `pad_*` で入力を 8 の倍数へ詰める（失敗は `ArmError::Layout`） |
| 3 | アダプタを選び、機能を確かめてから device を要求する（失敗は `ArmError::RequestDevice`） |
| 4 | 作成と投入を error scope（OutOfMemory と Validation）で囲み、検査つきの `create_shader_module` と明示の配置で 1 回だけ `submit` する。拒否は `ArmError::Validation`（panic しない） |
| 5 | C を読み戻し用の buffer へ写し、`map_async` と `poll(Wait)` で読み、`crop_c` で M x N にして `GateOutput::F32` で返す（失敗は `ArmError::Readback`） |

`ArmError` は `#[non_exhaustive]` で、`Display` の文言は固定の英語（wgpu のエラーの中身・アダプタの名前・入力の値を含まない）。

### bin `baseline-wgpu-gemm-gate`

- 環境変数 `FANDHE_SILICON_GEMM_GATE_CELL` に g3 のセルの識別名を 1 つ入れ、引数なしで起動する。1 プロセスにつき 1 セル
- 複数のセルは `scripts/metal-session.sh run --combination <名前> --validation-layer --gate-cell g3-<行>-<dtype> -- <bin>` をセルごとに呼ぶ

| 終了コード | 意味 |
| ---------- | ---- |
| 0 | 結果の 1 行を標準出力へ出し、比べが一致した |
| 1 | 結果の 1 行を標準出力へ出したが、一致しない |
| 2 | 誤用または失敗。標準出力は空で、標準エラーに固定の英語の文言だけを出す（環境変数の値・エラーの中身は出さない） |

### unsafe の所在

- `unsafe` を持つのは `src/bridge.rs` の関数 `experimental_features` の 1 か所だけ（`wgpu::ExperimentalFeatures::enabled()` が `unsafe fn`）
- `#[allow(unsafe_code)]` は `src/lib.rs` の `mod bridge;` の宣言だけに付ける
- この crate は workspace の lints を継がず、自前の `[lints.rust]`（`unsafe_code = "deny"`・`unsafe_op_in_unsafe_fn = "deny"`・`missing_docs = "warn"`）と `[lints.clippy]`（`undocumented_unsafe_blocks`・`missing_safety_doc` の deny）を持つ
- 個数の試験は `tests/unsafe_count.rs`。期待値の表 `EXPECTED` に載せる。形は [unsafe-policy](unsafe-policy.md)

### 試験（3/4）

| ファイル | 流れる場所 | 確かめる事項 |
| -------- | ---------- | ------------ |
| `tests/unsafe_count.rs` | 既定 | `src/` の `unsafe` の個数が表どおり。数え方の見本（コメント・文字列・識別子・種別） |
| `tests/gate_bin.rs` | 既定 | bin がセルの選択なし・不正な値・g3 以外のセル・引数ありを終了コード 2 と空の標準出力で拒否する |
| `tests/device_gate.rs` | `#[ignore]` | g3 の 2 行 x {f32, f16acc} の 4 本を device で流し、比べが一致しビット差が 0（PARITY-36 のポインタ） |

### 確かめ方

- `make baseline-verify`（`device_gate` は 4 件が ignored になる）
- device の要る回でだけ: `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test device_gate -- --ignored`

## 完了の条件の確かめ（4/4）

TASK-1010 の 4/4（#6322）として、ここまでの成果物を本リポで観測できる事実だけで確かめた記録。環境は GPU なしの Linux。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 腕の crate が `baseline-wgpu/` の member で、製品の workspace の外にある | `baseline-wgpu/Cargo.toml` の `members`、ルート `Cargo.toml` の `exclude`、`make deny` の製品の依存グラフの検査の行 | `members = ["crates/gemm-arm", "crates/public-api", "crates/smoke"]`、`exclude = ["baseline-wgpu", "measure", "wasm"]`、`ok: 17 crate(s) in the dependency graph checked, no wgpu-project crates (narrow=0 wide=0)`、`make deny` の終了コード 0 |
| 2 | WGSL の検証の試験が通る | `cargo test --manifest-path baseline-wgpu/Cargo.toml --locked -p baseline-wgpu-gemm-arm --test wgsl_validate` | `ok. 5 passed; 0 failed; 0 ignored` |
| 3 | 詰め物と切り出しの試験が通る | 同 `--test layout` | `ok. 8 passed; 0 failed; 0 ignored` |
| 4 | bin の誤用の試験が通る | 同 `--test gate_bin` | `ok. 4 passed; 0 failed; 0 ignored` |
| 5 | unsafe の個数の試験が通る | 同 `--test unsafe_count`、`grep -rn 'allow(unsafe_code)' baseline-wgpu/crates/*/src` | `ok. 7 passed; 0 failed; 0 ignored`、`allow(unsafe_code)` は 1 件（`baseline-wgpu/crates/gemm-arm/src/lib.rs` の `mod bridge;` の行） |
| 6 | device の要る試験の本数 | 同 `--test device_gate`（`-- --ignored` は付けない） | `ok. 0 passed; 0 failed; 4 ignored` |
| 7 | 依存の記録 | `grep -c '^\[\[approval\]\]' baseline-wgpu/deps-approved.toml`、`grep -n '^used_by' baseline-wgpu/deps-approved.toml`、`make deny` の `deps-check (baseline-wgpu/)` | 記録は 1 件、`used_by = ["baseline-wgpu-gemm-arm", "baseline-wgpu-public-api", "baseline-wgpu-smoke"]`、`ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml` |
| 8 | CI の `deny` ジョブが呼ぶ `make baseline-verify` | `make baseline-verify` | 終了コード 0。gemm-arm の `device_gate` は `0 passed; 0 failed; 4 ignored`、`gate_bin` は 4 passed、`layout` は 8 passed、`unsafe_count` は 7 passed、`wgsl_validate` は 5 passed。ほかの target は `0 passed; 0 failed; 0 ignored` |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1010）。

確かめた base は `cd916d512cef9cbcfffd5bcbc2103793e23fdd7a`、`docs/spec` の gitlink は `050073a33856cff4559718a8e760289ec3d5a65e`。

## 今の値の補い（base d082bc1）

記録時点の表と既存の節は書き換えない。以下は base `d082bc1` で確かめた今の値。

| 節・行 | 前の値 → 今の値 | 確かめ方 |
| ------ | --------------- | -------- |
| 本リポの現状の `流れる場所`（17 行目） | `make baseline-verify` に含まれる。device は使わない → `make baseline-verify` に含まれるが、device の経路（`src/device.rs`）と bin `baseline-wgpu-gemm-gate` が加わっている。device の要る試験は `tests/device_gate.rs` の 4 本で、すべて `#[ignore]`。既定の `cargo test` と `make baseline-verify` では `4 ignored` になり、device を使わない | `grep -n 'ignore' baseline-wgpu/crates/gemm-arm/tests/device_gate.rs`、`baseline-wgpu/crates/gemm-arm/Cargo.toml` の `[[bin]]`（18〜20 行目） |
