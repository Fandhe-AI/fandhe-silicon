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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 描画先の作成と描画の口 | #4581 |
