# 実装に含める範囲の決定の記録（TASK-43）の所在と反映の方針（案）

TASK-43（親 Issue #193）について、着手時点の現状と前提、本リポへの反映の方針（案）を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・候補の一覧・項目 ID の範囲や件数・理由は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
「方針」は案であり、オーナーの確認を経るまで確定した方針として扱わない。

## 決定の記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-43、`docs/spec/04-behavior/README.md`）
- 本リポには一覧・理由・経緯を置かない（[契約の文書の分担](contract/README.md)）
- 前提タスクの有無は TASK-43 を参照する（ポインタのみ）
- 完了の宣言はオーナーの確認による。本リポからは宣言しない

## 着手時点の本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 製品 workspace の member | `crates/core`・`crates/contract`・`crates/exec`・`crates/upper` の 4 つ | ルート `Cargo.toml` の `members` |
| 背後実装の feature | `fandhe-silicon-contract` に `backend-metal` / `backend-vulkan` / `backend-cuda` があり、対応するモジュールは `src/lib.rs` で feature によりゲートされる | `crates/contract/Cargo.toml` の `[features]`、`crates/contract/src/lib.rs` |
| 中身の無い置き場 | `backend-dx12-compute` は中身の無い仮の置き場。GL（GLES）は feature を足す位置のコメントだけがあり、feature 自体は無い。どちらも受け持つタスクのポインタ付き | `crates/contract/Cargo.toml` |
| crate の公開 | 全 crate が `publish = false` | ルート `Cargo.toml` の `[workspace.package]` |
| 領域ごとの契約の文書 | `docs/contract/` には書式・手順書だけがあり、領域ごとの文書はまだ無い | `ls docs/contract` |
| wgpu 系の禁止 | wgpu 系・naga は依存グラフに入れない | `deny.toml` の `[bans] deny`、`make deny` |

## 本リポへの反映の方針（案）

- 範囲の決定を本リポに写すのはポインタだけにする。一覧・索引は作らない
- 個々の領域は、それを受け持つタスクが [procedure](contract/procedure.md) の 2 段（形を決める、実装し確かめる）で入れる。rustdoc と `docs/contract/<領域>.md` は実装・公開テストと同じ PR で書く
- feature・モジュールの置き場は、既存の流儀（`Cargo.toml` のコメントに受け持つタスクのポインタを付ける）に合わせ、受け持つタスクの PR で足す。事前に空の feature・モジュール・文書を作らない
- 依存の追加は [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに従う。提供しない背後実装は、暗黙のフォールバックをせず、能力の問合せか明示的なエラーで返す（[coding-rust](../.claude/rules/coding-rust.md)）
- README が示す背後実装の説明と既定の feature は、本作業では変えない
- 範囲に迷うときは自動運転で決めず、[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路でオーナーに伺う

## オーナー判断待ち

本範囲で、本リポ側から新たに伺う論点は無い（ポインタ: TASK-43）。
実装中に見つかった場合だけ、contract-shaping-escalation の形式で追記する。

## 後続の分担

- TASK-43 の 2/2（Issue #195）は、spec 側に決定の記録ができた後に、本書の「所在」の節を更新し、`crates/contract/Cargo.toml` のコメントのポインタと食い違いが無いことを確かめる
- spec 側への記録と進み具合の記録は spec リポ側の作業で、本リポからは書き換えない

## 関連

- [workspace-boundary](workspace-boundary.md)・[contract/procedure](contract/procedure.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)
