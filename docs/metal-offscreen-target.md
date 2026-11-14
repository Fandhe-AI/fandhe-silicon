# Metal の背後実装の描画先の作成（TASK-377）の所在と本リポの現状

Metal の背後実装は、feature `graphics` の下で、`Profile::Graphics` で作った device の `Device::graphics()` から描画先の作成の 2 つ（`create_offscreen_target`・`create_offscreen_target_with_depth`）を返す。その所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-377。ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 描画先の表 | `crates/contract/src/backend_metal.rs` の `TargetTable<T>`（`imp` の外。`insert`・`remove`。識別子は 1 から払い出し、再利用しない） | 同ファイル |
| 大きさの検査とバイト数 | 同ファイルの `offscreen_target_plan`（`imp` の外。共通の crate の検査の関数を呼ぶ。検査の所在は [offscreen-target-check.md](offscreen-target-check.md)・TASK-374） | 同ファイル |
| 組の作成の部品 | 同ファイルの `create_pair_or_release`（`imp` の外） | 同ファイル |
| 欄 | `imp` の中の `MetalDevice` の `graphics_enabled`・`targets` | 同ファイル |
| `graphics()` | `imp` の中の `impl Device for MetalDevice` | 同ファイル |
| 作成の 2 つ | `imp` の中の `impl GraphicsOps for MetalDevice` の `create_offscreen_target`・`create_offscreen_target_with_depth` | 同ファイル |
| ほかのメソッドの返し | 同じ `impl GraphicsOps` の残りのメソッド | 同ファイル |
| 試験の module | `backend_metal::offscreen_target_tests`（`crates/contract/src/backend_metal.rs`。`#[cfg(all(test, feature = "graphics"))]`。device に触らない） | 下の「確かめ方」 |
| feature | `crates/contract/Cargo.toml` の `graphics`（`objc2-metal` の `MTLPixelFormat`・`MTLTexture` を `?` 付きで有効にする） | `crates/contract/Cargo.toml` |
| SAFETY の分類 | `crates/contract/tests/backend_metal_no_recording.rs` の `backend_metal_target_descriptor_safety_comments_state_the_validated_size` | 同ファイル |

## 描画先の作成と描画の口（1/3）

- `Profile::Graphics` で作った device の `graphics()` は `Some` を返す。`Profile::ComputeOnly` で作った device は `None` を返す。
- `create_offscreen_target(width, height, format)`:
    - 幅または高さが 0、または `limits().max_texture_size` を超える値は、Metal の呼び出しの前に `InvalidArgument` を返す。上限ちょうどは通す。
    - 形式は `Rgba8Unorm`・`Bgra8Unorm`・`Rgba32Float` を受ける。ほかの値は `Unsupported` を返す。
    - 2 次元・Private・mip 1・サンプル 1 のテクスチャを作り、用途は `RenderTarget | ShaderRead`。
    - 作れなかった場合と識別子が尽きた場合は、色の形式の 1 画素のバイト数 × 幅 × 高さを `requested_bytes` にして `AllocationFailed` を返す。
    - 返す handle の世代は呼び出しの時点の device の世代、識別子は 1 から、device の識別子は device の値。
- `create_offscreen_target_with_depth(width, height, color_format, depth)`:
    - 大きさと形式は、色・深度のどちらの Metal の呼び出しよりも前に照らす。
    - 色は `create_offscreen_target` を呼んで作る。深度は `Depth32Float` の 2 次元・Private・mip 1・サンプル 1 で、用途は `RenderTarget`。深度の handle の世代は色の handle の世代と同じ。
    - 深度の作成に失敗したら、色のテクスチャを表から外して 1 回だけ落としてから、深度の形式のバイト数の `AllocationFailed` を返す。外した識別子は再び払い出さない。
    - `depth` の `compare`・`write_enabled`・`clear_value` は作成では使わない。
- 残りのメソッドのうち `Result` を返すものは、引数を検査せずに `Unsupported { op: <メソッド名>, backend: Metal }` を返す。`RenderEncoder` を返す `create_render_encoder`・`create_render_encoder_with_depth` は、同じ値を `__backend_new_rejected` で載せた記録先を返す。
- `destroy_offscreen_target(target)` は、この device の識別子を持つ handle の描画先のテクスチャを表から外して解放する。別の device の handle と、表に無い識別子（破棄済みを含む）は何もしない。破棄しない描画先は device を破棄するまで表に残る。
- feature `graphics` の無い組には、この節のコードは入らない。

### 確かめ方

device の要らない試験（Linux でも流れる）:

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,graphics --lib offscreen_target_tests
```

`test result: ok. 8 passed; 0 failed; 0 ignored` の形で通る。

macOS の型検査と unsafe の守り:

```bash
cargo clippy --target aarch64-apple-darwin -p fandhe-silicon-contract --all-targets --features backend-metal,graphics -- -D warnings
cargo test -p fandhe-silicon-contract --all-features --test backend_metal_unsafe_count --test backend_metal_no_recording
```

## device の要る試験（2/3）

どの試験も Metal の device に触るので `#[ignore]` を付けてあり、専用の実機の回でだけ流す。

| 所在 | 試験 | 確かめる動き |
| ---- | ---- | ------------ |
| `src/backend_metal.rs` の `imp::offscreen_target_device_tests` | `color_target_has_the_expected_texture_shape` | `create_offscreen_target(1280, 720, Rgba8Unorm)` のテクスチャが、幅 1280・高さ 720・`RGBA8Unorm`・`Private`・`RenderTarget \| ShaderRead`・`Type2D`・mip 1・サンプル 1 |
| 同上 | `color_and_depth_targets_have_the_expected_texture_shapes` | `create_offscreen_target_with_depth` が 2 つの描画先を返し、深度が幅 1280・高さ 720・`Depth32Float`・`Private`・`RenderTarget`・`Type2D`・mip 1・サンプル 1、色が単独に作った描画先と同じ形で、識別子が違う |
| `tests/backend_metal_offscreen_target.rs` | `graphics_device_creates_a_target_owned_by_the_same_device` | `Profile::Graphics` の device の `graphics()` が `Some` で、作った handle の device の識別子と世代が、同じ device の `allocate` の handle の値と等しい |
| 同上 | `graphics_device_creates_color_and_depth_targets_with_distinct_ids` | 深度つきの作成が `Ok((色, 深度))` を返し、識別子が違い、device の識別子と世代が等しい |
| 同上 | `compute_only_device_has_no_graphics_ops` | `Profile::ComputeOnly` の device の `graphics()` が `None` |

形を読む口は、試験の module の外の `imp` の中にある値の型 `TargetShape` と `MetalDevice::target_shape` で、`#[cfg(all(test, feature = "graphics"))]` の下にだけある。公開の面と製品のビルドには出ない。

### 確かめ方（2/3）

macOS で、device を使わずに数だけ確かめる（`--ignored` は付けない）:

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,graphics --test backend_metal_offscreen_target
cargo test -p fandhe-silicon-contract --features backend-metal,graphics --lib offscreen_target_device_tests
```

前者は `3 ignored`、後者は `2 ignored` の形で通る。Linux では統合試験のファイルが 0 件になり、crate の中の module はコンパイルされない。

## 完了の条件の確かめ（3/3）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-377。ポインタのみ）。

時点のコミット: `ede5116`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | device の要らない試験 | `cargo test -p fandhe-silicon-contract --features backend-metal,graphics --lib offscreen_target_tests` | `8 passed; 0 failed; 0 ignored; 0 measured; 160 filtered out` |
| 2 | macOS の型検査 | `cargo clippy --target aarch64-apple-darwin -p fandhe-silicon-contract --all-targets --features backend-metal,graphics -- -D warnings`、同じコマンドの `--features backend-metal` | どちらも終了コード 0 |
| 3 | unsafe の守り | `cargo test -p fandhe-silicon-contract --all-features --test backend_metal_unsafe_count --test backend_metal_no_recording` | `8 passed; 0 failed; 0 ignored` と `1 passed; 0 failed; 0 ignored` |
| 4 | 依存 | `make deps-check`、`gh pr diff 8278 --name-only`、`gh pr diff 8279 --name-only`、この PR の差分の `--name-only` | `make deps-check` は終了コード 0。3 つの差分のどれにも `Cargo.lock` が無い |
| 5 | 依存グラフ | `make profile-dep-graph-check` | 下の (5) の行、終了コード 0 |
| 6 | 公開 API | `make public-api-check`、`make public-api-sdk-check` | どちらも終了コード 0 |
| 7 | device の要る試験の本数 | `grep -c '^#\[ignore' crates/contract/tests/backend_metal_offscreen_target.rs`、2/3 の module の中の行頭の `#[ignore` の数 | 3 と 2 |
| 8 | CI | `gh run view 43573191511`（2/3 のマージのコミットの main の push の run） | conclusion `success`、headSha `ede5116615d6635fb34f3e78cb4da9065ce7c242`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success` |

(1) の出力。

```text
running 8 tests
test backend_metal::offscreen_target_tests::pair_failure_in_color_calls_neither_depth_nor_release ... ok
test backend_metal::offscreen_target_tests::pair_failure_in_depth_releases_the_color_once_and_returns_the_depth_error ... ok
test backend_metal::offscreen_target_tests::pair_success_returns_both_and_does_not_release ... ok
test backend_metal::offscreen_target_tests::table_exhaustion_leaves_the_table_unchanged_and_drops_the_resource_once ... ok
test backend_metal::offscreen_target_tests::plan_accepts_the_exact_limit_and_returns_byte_counts ... ok
test backend_metal::offscreen_target_tests::plan_rejects_zero_and_over_limit_sizes ... ok
test backend_metal::offscreen_target_tests::table_issues_ids_from_one_and_never_reuses_them ... ok
test backend_metal::offscreen_target_tests::plan_returns_the_color_bytes_without_depth ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 160 filtered out; finished in 0.00s
```

(3) の出力。

```text
running 8 tests
test objc2_metal_features_do_not_include_other_encoder_headers ... ok
test backend_metal_dispatch_execution_gate_stays_closed_outside_unit_tests ... ok
test backend_metal_constant_safety_comment_keeps_its_rationale ... ok
test backend_metal_target_descriptor_safety_comments_state_the_validated_size ... ok
test backend_metal_encoder_safety_comments_state_the_cpu_side_preconditions ... ok
test backend_metal_descriptor_safety_comment_keeps_its_rationale ... ok
test backend_metal_contents_safety_comments_state_the_completed_submission_rationale ... ok
test backend_metal_code_has_no_words_beyond_compute_dispatch ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
running 1 test
test backend_metal_unsafe_count_matches_expected ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

(5) の出力。

```text
ok: 4 profile(s) checked, no judged crates (judged=0)
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 描画先の作成と描画の口 | #4581 |
| 2/3 | device の要る試験 | #4582 |
| 3/3 | 完了の条件の確かめ | #4583 |
