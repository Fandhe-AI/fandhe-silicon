# DX12 の compute に関する点検の記録（TASK-236）の所在と方針（案）

TASK-236（親 Issue #461）の点検の記録について、置き場・書き手・本リポへの映し方の方針を、本リポで公開済みの事実とポインタだけで記録する。
本書は案で、オーナーの確認は済んでいない。
spec の本文・点検の対象の名前・候補・結果・件数は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
現状と前提の節は 1/8（Issue #3932）が追記する。

## 記録の所在

- 点検の記録（表・結果）の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-236。対象の項目は `docs/spec/04-behavior/build-profiles.md` の BUILD-18・BUILD-64）
- 本リポには表・結果・件数を置かない（[契約の文書の分担](contract/README.md)）
- 前提タスクの有無と完了の条件は TASK-236 を参照する（ポインタのみ）
- 完了の条件との対応づけと進み具合の記録の正も private spec 側（TASK-236 の状態。ポインタのみ）

## 方針（2/8）が前提にする本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| DX12 の置き場 | `backend-dx12-compute` は中身の無い仮の feature。受け持つタスクのポインタ付きのコメントがある | `crates/contract/Cargo.toml` |
| DX12 のコード | `crates/` の `*.rs` に DX12 の記述は無い。背後実装のモジュールは Metal・Vulkan・CUDA の 3 つ | `grep -rni dx12 crates --include='*.rs'`、`ls crates/contract/src` |
| 契約の文書 | `docs/contract/` は書式・手順書だけで、領域ごとの文書は無い | `ls docs/contract` |
| spec の取り込み | `docs/spec` は gitlink（`update = none`）。CI は取得しない | `.gitmodules`、`git ls-tree HEAD docs/spec` |

- 確かめた日付: 2026-10-12（base: `3e78751`、gitlink: `c1cb4c9`）

## 本リポへの反映の方針（2/8）

- 点検の記録は private spec 側に置き、本リポには置かない。根拠は、記録の内容が本リポのコード・公開テストから説明できない spec 由来の情報であること（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)、[契約の文書の分担](contract/README.md)）
- PoC の記録は書き換えない・写さない（[poc-carry-over](../.claude/rules/poc-carry-over.md)）
- spec 側の置き場・書式は spec リポ側の作業。本リポから出すのは提案までで、`docs/spec` 配下と gitlink は変えない（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）
- 本作業で feature・モジュール・テスト・領域ごとの契約の文書を足さない。`backend-dx12-compute` の中身は `crates/contract/Cargo.toml` のコメントが指すタスクの受け持ち
- 点検の入力は gitlink の指す commit の spec とし、確かめの記録には base と gitlink の短い sha を残す
- 外部仕様の確認は reference-researcher に委譲し、`windows-graphics-media` スキルを先に当たる。調べた結果の本体も spec 側に置く
- 見直しが要る点が見つかった場合は、文書・テストで黙って寄せない。contract-shaping-escalation の差し戻しの手順に従い、本リポには ID のポインタと「spec 側の判断待ち」の事実だけを書く
- 後続の確かめの段（4/8・5/8・8/8）の本リポ側の作業は、本リポで観測できる事実による確かめの記録とする（[implementation-scope](implementation-scope.md) の「完了の条件の確かめ」の流儀）

## 2/8 と 3/8 の分け方

| 段 | 受け持ち |
| -- | -------- |
| 2/8（Issue #3933） | 方針: 記録の置き場・書き手・本リポへの映し方・伺う経路・後続への引き継ぎ |
| 3/8（Issue #3934） | 形: 記録の行の単位・列・結果の書き方・ファイルの構成（本体は spec 側の提案。本リポには節の構成とポインタだけ） |

## オーナー判断待ち

論点 1: spec 側の点検の記録を、誰がどの経路で書くか（ポインタ: TASK-236、#461）

1. 要約: 記録の正は spec 側だが、自動運転の実装エージェントは spec リポへ書けない
2. 選択肢と影響
    - A: main セッションまたはオーナーが spec リポ側で書く（進み具合の記録と同じ経路）。本リポの sub-issue は観測できる事実の確かめだけを残す
    - B: 公開してよい範囲をオーナーが決め、その範囲だけ本リポに置く。spec 漏えいの検査の負担が増える
    - C: 記録の作成を自動運転の対象から外し、後続の sub-issue を止める
3. 推奨: A（既存の分担と同じ。公開側に新しい内容を出さない）
4. 止まるもの: 記録を埋める作業、#461 のクローズ、TASK-236 を前提に引くタスク（`docs/spec/05-tasks.md` を参照。ポインタのみ）
5. 回答の形: 「推奨で」の一言で A

推奨案は先行確定しない。判断が出るまで本書は案のままとする。

## 後続の分担

| Issue | 段 | 本リポ側で行うこと |
| ----- | -- | ------------------ |
| #3932 | 1/8 | 現状と前提の節の追記 |
| #3934 | 3/8 | 形（節の構成とポインタ） |
| #3935〜#3939 | 4/8〜8/8 | 本リポで観測できる事実による確かめの記録。6/8 はオーナーへの確認（経路は上の論点に従う）、8/8 は完了の条件の確かめ |

- spec 側への記録は spec リポ側の作業で、本リポからは書き換えない

## 関連

- [contract/README.md](contract/README.md)・[contract/procedure.md](contract/procedure.md)・[implementation-scope.md](implementation-scope.md)・[workspace-boundary.md](workspace-boundary.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)
