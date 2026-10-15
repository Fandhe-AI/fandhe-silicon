# 性能を測る共通の道具（TASK-27）の所在と本リポの現状

TASK-27 の決定の記録の所在と、本リポのコードで確かめられる現状を案内する（親 Issue は #121）。
spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-27）
- 前提のタスクはポインタのみ（TASK-4・TASK-28・TASK-19）。対応する公開側の文書は次のとおり
    - TASK-4: ルートの `Cargo.toml` の `members` と [workspace-boundary](workspace-boundary.md)
    - TASK-28: [measurement-record](measurement-record.md)
    - TASK-19: [metal-run-procedure](metal-run-procedure.md)

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 製品の workspace | `members` は 4 crate、`exclude` は `baseline-wgpu`。外部依存の承認の記録は 0 件 | `grep -n -e members -e exclude Cargo.toml`、`grep -c '^\[\[' deps-approved.toml` |
| 性能を測る道具 | 時間を測る標準ライブラリの型の使用が `crates/`・`baseline-wgpu/` に無い | `grep -rn -e Instant -e SystemTime -e 'std::time' crates baseline-wgpu`（0 件） |

既にある関連の部品（名前とリンクのみ）:

- `crates/core` の `test_frame` と記録の集計 `scripts/record-summary.sh`: [test-frame](test-frame.md)
- 回ごとの記録の形の検査と手順のスクリプト: [metal-run-procedure](metal-run-procedure.md)
- 比較基準の wgpu の別 workspace: [baseline-wgpu](baseline-wgpu.md)
- 機材の要らない CI の枠: [ci-device-free-jobs](ci-device-free-jobs.md)

## 本リポでの反映の方針

- 道具の置き場・記録の欄・統計・検査は、受け持つ段の PR のコードと公開テストが示す（親 #121）
- 本書には、その PR で入った振る舞いを書き足す

## 関連

- [measurement-record](measurement-record.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
