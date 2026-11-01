# 描画先の handle の型（TASK-375）の所在と本リポの現状

TASK-375 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは TEX-12 のポインタだけにする。事実はリポジトリのルートで流したコマンドの出力から書く。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-375。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 現状の形（1/4）

### RenderTargetHandle

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/render_target.rs:33`。`pub struct RenderTargetHandle`。`lib.rs` の `pub mod render_target;`。feature の条件はない | `grep -n 'pub struct' crates/core/src/render_target.rs` |
| 欄 | `generation`・`id`・`device_id`（どれも `u64`・非公開） | 同上 |
| derive | `Debug` だけ。`Clone`・`Copy` は持たない | `grep -n 'derive' crates/core/src/render_target.rs` |
| 背後実装向けの口 | `#[doc(hidden)]` の 4 つ。`__backend_new(generation, id, device_id)`（検査せず、safe）・`__backend_generation`・`__backend_id`・`__backend_device_id` | `grep -n 'pub fn' crates/core/src/render_target.rs` |
| 公開の読み出し | なし | 同上 |

### 対応するテスト

`crates/core/src/render_target.rs` の rustdoc の doctest:

| 位置 | 確かめること |
| ---- | ------------ |
| line 10 | `__backend_new(5, 3, 1)` の各値を 3 つの口で読むと 5・3・1 |
| line 20（`compile_fail,E0451`） | 構造体式が拒まれる |
| line 27（`compile_fail,E0599`） | `clone` が呼べない |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --doc render_target
```

`--doc` 全体では 3 本とも `ok`（`34 passed; 0 failed`）。
