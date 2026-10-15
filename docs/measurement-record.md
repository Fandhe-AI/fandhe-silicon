# 測定の記録の形式と置き場（TASK-28）の所在と本リポの現状

TASK-28 の決定の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-28。関連: TASK-27・TASK-19・TASK-23）
- 測定の記録の実物は本リポに置かない

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 記録を出すコード | `crates/` に測定の記録を出す型・処理は無い（run ごとの記録の書き出しは `measure/` の `measure-perf` にある。[measurement-tool](measurement-tool.md)）。`crates/core` の公開面は `parity` と `test_frame`（どちらも `test-support` の裏）のみ | `grep -n "pub mod" crates/*/src/lib.rs`、`grep -rn -i -e serde -e serialize -e json crates --include='*.rs'`（0 件） |
| 書式の検査 | 回ごとの記録（TASK-19）の形を検査するスクリプトが `scripts/` にある（[metal-run-procedure](metal-run-procedure.md)）。測定の記録（run ごと）の書式を検査するものは無い | `ls scripts`、`grep -rn -i measurement scripts baseline-wgpu`（0 件） |
| 既にある書式の文書 | 比較基準の wgpu の版の記録の書式は [wgpu-baseline-record](wgpu-baseline-record.md) にある | `ls docs/wgpu-baseline-record.md` |
| `baseline-wgpu/` の smoke bin | 標準出力へ、アダプタの件数の行と、アダプタごとの `name`・`backend`・`device_type` の行を出す。記録の形式では出さない | `baseline-wgpu/crates/smoke/src/main.rs` の `println!` |

## 本リポでの反映の方針

- 記録の書式の定義（型・欄）と、それを出す道具・形の検査は、受け持つタスクの PR で公開テストとともに入れる（ポインタ: TASK-27。関連: TASK-19・TASK-23）
- 書式の中身は、その PR のコードとテストが示す
- 回ごとの記録の形は [metal-run-procedure](metal-run-procedure.md) の「3. 回の記録の形の検査」の節を指す

## 関連

- [measurement-tool](measurement-tool.md)
- [wgpu-baseline-record](wgpu-baseline-record.md)
- [baseline-wgpu](baseline-wgpu.md)
- [parity-tool](parity-tool.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
