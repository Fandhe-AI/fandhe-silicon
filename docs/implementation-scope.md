# 実装に含める範囲の決定の記録（TASK-43）の所在と反映の方針

TASK-43（親 Issue #193）について、決定の記録の所在、着手時点の現状と前提、本リポへの反映の方針を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・候補の一覧・項目 ID の範囲や件数・理由は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 決定の記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-43 と、そこから辿る決定の記録、`docs/spec/04-behavior/README.md`）
- 本リポには一覧・理由・経緯を置かない（[契約の文書の分担](contract/README.md)）
- 前提タスクの有無は TASK-43 を参照する（ポインタのみ）
- 完了の条件との対応づけの記録の正も private spec 側にある（TASK-43 の状態。ポインタのみ）

## 着手時点の本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 製品 workspace の member | `crates/core`・`crates/contract`・`crates/exec`・`crates/upper` の 4 つ | ルート `Cargo.toml` の `members` |
| 背後実装の feature | `fandhe-silicon-contract` に `backend-metal` / `backend-vulkan` / `backend-cuda` があり、対応するモジュールは `src/lib.rs` で feature によりゲートされる | `crates/contract/Cargo.toml` の `[features]`、`crates/contract/src/lib.rs` |
| 中身の無い置き場 | `backend-dx12-compute` は中身の無い仮の置き場。GL（GLES）は feature を足す位置のコメントだけがあり、feature 自体は無い。どちらも受け持つタスクのポインタ付き | `crates/contract/Cargo.toml` |
| crate の公開 | 全 crate が `publish = false` | ルート `Cargo.toml` の `[workspace.package]` |
| 領域ごとの契約の文書 | `docs/contract/` には書式・手順書だけがあり、領域ごとの文書はまだ無い | `ls docs/contract` |
| wgpu 系の禁止 | wgpu 系・naga は依存グラフに入れない | `deny.toml` の `[bans] deny`、`make deny` |

## 本リポへの反映の方針

- 範囲の決定を本リポに写すのはポインタだけにする。一覧・索引は作らない
- 個々の領域は、それを受け持つタスクが [procedure](contract/procedure.md) の 2 段（形を決める、実装し確かめる）で入れる。rustdoc と `docs/contract/<領域>.md` は実装・公開テストと同じ PR で書く
- feature・モジュールの置き場は、既存の流儀（`Cargo.toml` のコメントに受け持つタスクのポインタを付ける）に合わせ、受け持つタスクの PR で足す。事前に空の feature・モジュール・文書を作らない
- 依存の追加は [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに従う。提供しない背後実装は、暗黙のフォールバックをせず、能力の問合せか明示的なエラーで返す（[coding-rust](../.claude/rules/coding-rust.md)）
- README が示す背後実装の説明と既定の feature は、本作業では変えない
- 範囲に迷うときは自動運転で決めず、[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路でオーナーに伺う

## オーナー判断待ち

本範囲で、本リポ側から新たに伺う論点は無い（ポインタ: TASK-43）。
実装中に見つかった場合だけ、contract-shaping-escalation の形式で追記する。

## 完了の条件の確かめ

本リポで観測できる事実による確かめの記録（ポインタ: TASK-43）。
spec の完了の条件との対応づけの記録の正は private spec 側（TASK-43 の状態。ポインタのみ）。条件の本文は転記しない。

| 確かめる項目 | 方法 | 現時点の結果 |
| ------------ | ---- | ------------ |
| 1/2（方針の整理）が済んでいる | Issue #194・PR #7019 の状態 | 済（クローズ・マージ済み） |
| `crates/contract/Cargo.toml` のコメントのタスクのポインタ（TASK-222・TASK-234・TASK-2581・TASK-2583）が、gitlink の指す commit の `docs/spec/05-tasks.md` に見出しとして在る | gitlink の commit の当該ファイルを一時領域へ読み取りで取得し、見出しの有無を数える（取得物は削除） | 4 件とも在る。コメントの役割分けとの照合は未検証 |
| 「着手時点の本リポの事実」の表がいまも成り立つ | 表の「確かめ方」の列を再実行 | 成り立つ |
| 事前に空の feature・モジュール・領域ごとの文書を足していない | `crates/contract/Cargo.toml` の `[features]`、`ls docs/contract` | 足していない（`docs/contract/` は書式・手順書のみ） |
| 品質ゲート | `make check` | 通過（`make verify`・`make deny` は未実行。コード・設定の変更なし） |
| spec 側の決定の記録と進み具合の記録 | 本リポからは確かめない | 記録の正は private spec 側（TASK-43 の状態。ポインタのみ） |

- 判定: 本リポで確かめられる項目に問題は見つからなかった。完了の条件との対応づけの記録の正は private spec 側（TASK-43 の状態。ポインタのみ）
- 再実行の契機: gitlink の更新（`update-external.yml` の PR）、`crates/contract/Cargo.toml` のコメントのポインタの変更のあと、同じ手順で表を更新する
- 確かめた日付: 2026-10-11（base: `40817ac`、gitlink: `e86d319`）

## 後続の分担

- TASK-43 の 2/2（Issue #195）の本リポ側の作業（上の確かめの記録）は済
- spec 側への記録と進み具合の記録は spec リポ側の作業で、本リポからは書き換えない

## 関連

- [workspace-boundary](workspace-boundary.md)・[contract/procedure](contract/procedure.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)
