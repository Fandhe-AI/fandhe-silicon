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
