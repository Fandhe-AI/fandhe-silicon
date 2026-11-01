# 描画先の handle の型と照合の関数（TASK-375）の所在と本リポの現状

TASK-375 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは TEX-12 のポインタだけにする。事実はリポジトリのルートで流したコマンドの出力から書く。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-375。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 現状の形（1/4）

### RenderTargetHandle

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/render_target.rs:34`。`pub struct RenderTargetHandle`。`lib.rs` の `pub mod render_target;`。feature の条件はない | `grep -n 'pub struct' crates/core/src/render_target.rs` |
| 欄 | `generation`・`id`・`device_id`（どれも `u64`・非公開） | 同上 |
| derive | `Debug` だけ。`Clone`・`Copy` は持たない | `grep -n 'derive' crates/core/src/render_target.rs` |
| 背後実装向けの口 | `#[doc(hidden)]` の 4 つ。`__backend_new(generation, id, device_id)`（検査せず、safe）・`__backend_generation`・`__backend_id`・`__backend_device_id` | `grep -n 'pub fn' crates/core/src/render_target.rs` |
| 公開の読み出し | なし | 同上 |

### 対応するテスト

`crates/core/src/render_target.rs` の rustdoc の doctest:

| 位置 | 確かめること |
| ---- | ------------ |
| line 11 | `__backend_new(5, 3, 1)` の各値を 3 つの口で読むと 5・3・1 |
| line 21（`compile_fail,E0451`） | 構造体式が拒まれる |
| line 28（`compile_fail,E0599`） | `clone` が呼べない |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --doc render_target
```

`--doc` 全体では 3 本とも `ok`（`34 passed; 0 failed`）。

## 現状の形（2/4）

### check_render_target_device

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/render_target.rs:113`。`pub fn check_render_target_device(handle: &RenderTargetHandle, device_id: u64, generation: u64, lost: bool) -> Result<(), ContractError>`。feature の条件はない | `grep -n 'pub fn check_render_target_device' crates/core/src/render_target.rs` |
| 入力 | 照らす handle・自 device の識別子・呼んだ時点の世代・ロストの有無 | 同上 |
| 返す値 | ロストなら `Err(DeviceLost { generation })`（渡した世代）。ロストでなく handle の device の識別子が違えば `Err(DeviceMismatch)`。それ以外は `Ok(())`。ロストの判定が先 | 下の doctest |
| 見ないもの | handle の世代・識別子 | 下の doctest |
| 副作用 | なし（状態を持たず、何も読み書きしない） | 関数の本体 |

### 対応するテスト

`crates/core/src/render_target.rs` の rustdoc の doctest（line 81 の 1 本。中に 6 つの `assert_eq!`）:

| 確かめること | 期待 |
| ------------ | ---- |
| 同じ識別子・ロストなし | `Ok(())` |
| 違う識別子・ロストなし | `Err(DeviceMismatch)` |
| 同じ識別子・ロスト | `Err(DeviceLost { generation: 9 })`（渡した世代） |
| 違う識別子・ロスト | `Err(DeviceLost { generation: 9 })`（ロストが先） |
| handle の世代・識別子が `0` と `u64::MAX` | 同じ device の識別子なら `Ok(())` |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --doc render_target
```

`render_target` の doctest は 4 本とも `ok`。`cargo test -p fandhe-silicon-core --doc` は既定の feature で `23 passed; 0 failed`。

## 試験（3/4）

試験用の device（`StubDeviceProvider::request`）と `FaultStub` を使い、外の crate から公開のパスだけで `check_render_target_device` の返す値を確かめる統合テスト。device の要求は `BackendKind::Metal`・`Profile::ComputeOnly`・`DeviceOwnership::ProductIndependent`。自 device の識別子には `generation()` を使う。

| ファイル | 試験名 | 確かめること |
| -------- | ------ | ------------ |
| `crates/core/tests/render_target_device_check.rs` | `handle_from_other_stub_device_is_device_mismatch` | 別の device の識別子で照らすと `Err(DeviceMismatch)`、同じ識別子なら `Ok(())` |
| 同上 | `rejection_matches_buffer_handle_rejection` | 別の device へバッファの handle を渡した `upload` の `Err` と、描画先の handle の照合の `Err` が等しく `DeviceMismatch` |
| 同上 | `handle_from_recreated_device_is_device_mismatch` | device を作り直すと識別子が変わり、前の識別子の handle は `Err(DeviceMismatch)` |
| 同上 | `forged_handle_is_judged_by_device_id_only` | 世代・識別子が `0`・`u64::MAX` の handle も device の識別子だけで判定される |
| `crates/core/tests/render_target_lost_check.rs` | `lost_stub_returns_device_lost_with_given_generation` | ロスト前は `Ok(())`、`simulate_device_lost` の後は `Err(DeviceLost { generation })`（渡した世代） |
| 同上 | `lost_is_checked_before_device_id` | ロスト後は別の device の識別子でも `DeviceMismatch` でなく `DeviceLost` |

### 流れる組

| ファイル | 流れる場所 |
| -------- | ---------- |
| `render_target_device_check` | `make verify`（`--all-features`）と `make build-profiles` の core の `test-support` だけの組（`scripts/build-profiles.sh:88`〜`:90`） |
| `render_target_lost_check` | `make verify` |

2 つとも `make wasm-verify` の wasm32 の型検査（`scripts/wasm-verify.sh:39`）で通る。

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --features test-support --test render_target_device_check
cargo test -p fandhe-silicon-core --features test-support,fault-injection --test render_target_lost_check
```

出力はそれぞれ `4 passed`、`2 passed`。

## 完了の条件の確かめ（4/4）

TASK-375 の 4/4（#4580）として、本リポで観測できる事実だけを記録する。GPU の実機のない Linux の隔離した作業ツリー（base `e599bceef2ca1fa9ad1959f7f1b59343b618d7da`）で流した出力による。コード・試験・CI は変えていない。ID は対応づけのポインタ（TEX-12）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 型の形（欄が非公開・`Clone`/`Copy` なし） | `grep -n 'derive' crates/core/src/render_target.rs`、`grep -n 'pub struct' crates/core/src/render_target.rs`、`cargo test -p fandhe-silicon-core --doc render_target` | 終了コード 0。`33:#[derive(Debug)]`、`34:pub struct RenderTargetHandle {`。`RenderTargetHandle` の doctest は 3 本（line 11 の往復、line 21 の `compile_fail,E0451`、line 28 の `compile_fail,E0599`）で、どれも `ok` |
| 2 | 照合の関数の順（TEX-12） | 同じ `cargo test -p fandhe-silicon-core --doc render_target` | `render_target::check_render_target_device (line 81) ... ok`。`render_target` の doctest は合計 4 本で、実行の結果は通常の 2 本が `2 passed; 0 failed`、compile fail の 2 本が `2 passed; 0 failed` |
| 3 | 試験用の device 2 つの拒否と、`BufferHandle` の拒否と同じ値 | `cargo test -p fandhe-silicon-core --features test-support --test render_target_device_check` | 終了コード 0。`4 passed; 0 failed`（`handle_from_other_stub_device_is_device_mismatch`・`rejection_matches_buffer_handle_rejection`・`handle_from_recreated_device_is_device_mismatch`・`forged_handle_is_judged_by_device_id_only`） |
| 4 | ロスト | `cargo test -p fandhe-silicon-core --features test-support,fault-injection --test render_target_lost_check` | 終了コード 0。`2 passed; 0 failed`（`lost_stub_returns_device_lost_with_given_generation`・`lost_is_checked_before_device_id`） |
| 5 | CI で流れる場所 | `grep -n 'run: make' .github/workflows/ci.yml`、`grep -n 'CORE_PKG' scripts/build-profiles.sh`、`grep -n 'cargo check -p fandhe-silicon-core' scripts/wasm-verify.sh`、`make wasm-verify`、main の push の ci.yml の run の一覧 | `make verify` は `:177`・`:465`、`make build-profiles` は `:181`・`:468`、`make wasm-verify` は `:262`・`:427`。`build-profiles.sh` は core の feature なし（`:85`・`:86`。doctest が流れる）と `test-support` だけ（`:89`・`:90`。`render_target_device_check` が流れる）の clippy と test を流す。`wasm-verify.sh` は core の試験の wasm32 の型検査を既定（`:36`）と `--all-features`（`:39`）で流す。`make wasm-verify` は終了コード 0。main の push の run は、1/4 の `36338ba` が 41297383208、2/4 の `039285d` が 41298723471、3/4 の `e599bce` が 41299804348（3 件とも completed / success） |
| 6 | 公開 API の検査 5 つ | `make public-api-check`・`make public-name-check`・`make public-api-webgpu-check`・`make public-api-sdk-check`・`make core-deps-check` | すべて終了コード 0。`documents=10 public_items_total=10215 wgpu_type_refs_total=0 wgpu_other_refs_total=0`／`documents=10 public_items_total=8720 name_hits_total=0 sources=1 source_hits_total=0 field_specs=1 field_hits_total=0`／`documents=2 … blocking_wait_names_total=0 … webgpu_names_total=0`／`documents=10 public_items_total=8720 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0`／`unexpected=0 missing=0 … core_features=4 forwarded=4 forward_violations=0` |
| 7 | 依存 0・unsafe の追加 0 | 1/4 の親 `55e2ae2` から HEAD への `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` の差分、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -n unsafe_code crates/core/src/lib.rs`、`grep -c unsafe crates/core/src/render_target.rs` | `Cargo.lock`・`deps-approved.toml` の差分は空。`crates/core/Cargo.toml` の差分は `[[test]]` の 2 項目（`render_target_device_check`: `test-support`、`render_target_lost_check`: `test-support`・`fault-injection`）と、`ext-interop` の直前のコメント行（#7599）だけで、`[features]` は 4 つ（`test-support`・`fault-injection`・`native-blocking-wait`・`ext-interop`）のまま。`[[approval]]` は 4。`29:#![forbid(unsafe_code)]`。`render_target.rs` の `unsafe` は 0 |
| 8 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles` | すべて終了コード 0（Linux・GPU なし） |

- 完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-375）。確かめた base は `e599bceef2ca1fa9ad1959f7f1b59343b618d7da`。

## 段の表

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 現状の形（型） | #4577 |
| 2/4 | 現状の形（照合の関数） | #4578 |
| 3/4 | 試験 | #4579 |
| 4/4 | 完了の条件の確かめ | #4580 |
