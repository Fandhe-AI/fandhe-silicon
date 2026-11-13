# Metal の device で PVRTC の形式のテクスチャの作成を試みる試験（TASK-1380）の所在と本リポの現状

Metal の背後実装に、PVRTC の 8 つの画素形式で 2D テクスチャの作成だけを試みる、device の要る単体試験を 1 本置いた。その所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1380。ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験の module | `crates/contract/src/backend_metal.rs` の `imp` の中の `pvrtc_create_tests` | 同ファイル |
| 試験の関数 | `pvrtc_formats_attempt_texture_creation`（`#[ignore]` 付き） | 同ファイル |
| cfg | `target_os = "macos"` の `imp` の中で `all(test, feature = "pvrtc-probe")` | 同ファイル |
| feature | `crates/contract/Cargo.toml` の `pvrtc-probe`（`backend-metal` を要求し、`objc2-metal` の `MTLPixelFormat`・`MTLTexture` をこの feature の裏でだけ有効にする）。`backend-metal` と本番の組には入れない | `crates/contract/Cargo.toml` |
| feature の範囲の守り | `crates/contract/tests/feature_default_scope.rs`（試験・診断専用の一覧と `backend-metal` を要ることの確認）、`scripts/upper-layer-check.sh` の `FORBIDDEN_FEATURES` | 下の「確かめ方」 |
| unsafe | 記述子を作る呼び出し 1 つを包む `unsafe` ブロック 1 つ。個数は `crates/contract/tests/backend_metal_unsafe_count.rs` の `EXPECTED` が持つ | 同ファイル |
| SAFETY の分類 | `crates/contract/tests/backend_metal_no_recording.rs` の `backend_metal_descriptor_safety_comment_keeps_its_rationale` | 同ファイル |

## 試験の振る舞い

- 既定の device を取る（取れなければ `expect` で失敗する）。
- 形式は `PVRTC_RGB_2BPP`・`PVRTC_RGB_2BPP_sRGB`・`PVRTC_RGB_4BPP`・`PVRTC_RGB_4BPP_sRGB`・`PVRTC_RGBA_2BPP`・`PVRTC_RGBA_2BPP_sRGB`・`PVRTC_RGBA_4BPP`・`PVRTC_RGBA_4BPP_sRGB` の 8 つ。
- 形式ごとに、幅 32・高さ 32・mipmap なし・storage mode `Shared`・用途 `ShaderRead` だけの 2D の記述子を作り、`newTextureWithDescriptor` を呼ぶ。
- 標準出力に 1 形式 1 行で `pvrtc: format=<定数名> created=<true|false>` を出す。
- `Apple2`〜`Apple9` と `Mac2` の判定を 1 行ずつ `family: <名前>=<true|false>` で出す。
- assert するのは、device が取れたことと、8 形式の行がそれぞれちょうど 1 回出たことだけ。作成の成否は assert しない。
- `#[ignore = "touches the Metal device; run only in a dedicated device session"]` を付け、`--ignored` を付けない実行では流れない。
- データの写し・view・標本化・型の値・背後実装の表の行・公開の口は持たない。

## 確かめ（2/2）

| コマンド | 結果 |
| -------- | ---- |
| `cargo clippy -p fandhe-silicon-contract --all-targets --target aarch64-apple-darwin --all-features -- -D warnings` | 終了コード 0 |
| 同 `--no-default-features --features pvrtc-probe` | 終了コード 0 |
| 同 `--features backend-metal`（probe なし） | 終了コード 0 |
| `cargo test -p fandhe-silicon-contract --all-features --test backend_metal_unsafe_count --test backend_metal_no_recording --test feature_default_scope --test feature_gate_scope` | 全て通過 |
| `make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make upper-layer-check`・`make test-upper-layer-check` | 終了コード 0 |

Linux ではこの module は空になる。macOS のビルドでは `1 ignored` の試験が 1 本ある。`Cargo.lock` に差は出ない。

### 確かめ方

```bash
cargo clippy -p fandhe-silicon-contract --all-targets --target aarch64-apple-darwin --no-default-features --features pvrtc-probe -- -D warnings
cargo test -p fandhe-silicon-contract --test backend_metal_unsafe_count --test backend_metal_no_recording --test feature_default_scope
make test-upper-layer-check
```
