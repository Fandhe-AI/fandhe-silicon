# 描画の公開の型と操作の trait（TASK-837）の所在と本リポの現状

TASK-837 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは DEV-63 のポインタだけにする。事実はリポジトリのルートで流したコマンドの出力から書く。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-837。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 現状の形（1/4）

### feature と転送

| 項目 | 事実 |
| ---- | ---- |
| core の feature | `crates/core/Cargo.toml:26` の `graphics = []` |
| 契約の crate の feature | `crates/contract/Cargo.toml:34` の `graphics = ["fandhe-silicon-core/graphics"]` |
| module | `crates/core/src/lib.rs` の `#[cfg(feature = "graphics")] pub mod graphics;`。feature を有効にしたときだけ公開される |

### 型

所在は `crates/core/src/graphics.rs`。

| 型 | 所在 | 内容 | derive | `#[non_exhaustive]` |
| -- | ---- | ---- | ------ | ------------------- |
| `PixelFormat` | `:27` | `Rgba8Unorm`・`Bgra8Unorm`・`Rgba32Float` | `Debug`・`Clone`・`Copy`・`PartialEq`・`Eq` | あり |
| `DepthFormat` | `:48` | `Depth32Float` | 同上 | あり |
| `CompareFunction` | `:67` | `Less`・`LessEqual`・`Always` | 同上 | あり |
| `DepthConfig` | `:89` | 公開の欄 `format`・`clear_value`・`compare`・`write_enabled` | `Debug`・`Clone`・`Copy`・`PartialEq` | なし |
| `RenderEncoder` | `:132` | 非公開の欄 4 つ。`Clone`・`Copy` を持たない | `Debug` だけ | なし |

`RenderEncoder` の背後実装向けの口は `#[doc(hidden)]` の 6 つ。`__backend_new`・`__backend_new_rejected`（どちらも検査せず、safe）・`__backend_generation`・`__backend_id`・`__backend_device_id`・`__backend_rejected`。

描画先の handle は `crates/core/src/render_target.rs` の `RenderTargetHandle` を使う（TASK-375）。

### GraphicsOps

`crates/core/src/graphics.rs:202` の `pub trait GraphicsOps`。既定の本体は持たない。操作は 10 個。署名は rustdoc を見る。

`create_render_pipeline`・`create_render_pipeline_with_depth`・`create_offscreen_target`・`create_offscreen_target_with_depth`・`create_render_encoder`・`create_render_encoder_with_depth`・`draw`・`draw_ex`・`submit_render`・`copy_target_to_buffer`

### 対応するテスト

`crates/core/src/graphics.rs` の rustdoc の doctest:

| 位置 | 確かめること |
| ---- | ------------ |
| `PixelFormat` line 15（`compile_fail,E0004`） | 外の crate からワイルドカードの腕の無い `match` が拒まれる |
| `DepthFormat` line 38（`compile_fail,E0004`） | 同上 |
| `CompareFunction` line 55（`compile_fail,E0004`） | 同上 |
| `DepthConfig` line 78 | 公開の欄で構造体式を書ける |
| `RenderEncoder` line 105 | `__backend_new(5, 3, 1)` の 3 値と `__backend_rejected()` が `None`。`__backend_new_rejected` の値が `Some(&ContractError::DeviceMismatch)` で読める |
| `RenderEncoder` line 119（`compile_fail,E0451`） | 構造体式が拒まれる |
| `RenderEncoder` line 126（`compile_fail,E0599`） | `clone` が呼べない |
| `GraphicsOps` line 196 | `&dyn GraphicsOps` で書け、`submit_render` に記録先を 1 回渡せる |
| `GraphicsOps::submit_render` line 269（`compile_fail,E0382`） | 同じ記録先を 2 回渡すことが拒まれる |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features graphics --doc graphics
```

実行の出力は、通常の doctest が 3 件、`compile_fail` が 6 件で、いずれも `ok`。

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #5817 | 現状の形 |
| 2/4 | #5818 | device の口と試験用の device |
| 3/4 | #5819 | CI への組み込み |
| 4/4 | #5820 | 型の試験と完了の条件の確かめ |
