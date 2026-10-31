# 見つけた事項はその場で対応する

## 目的・適用範囲

実装・レビュー・照合・調査の中で見つけた、いずれ対応が要る事項（機能の不足・拡張、文書と実装のずれ、検査の穴、古くなった記述など）を、後回しにせず見つけたうちに対応する。
本リポは今後も機能を足していくので、いずれ実装するものは見つけた時点で片づけておく。
main セッション・subagent・implement-issue-tree の各段のすべてに適用する。

## 原則

- 「後に回す」「別の機会に」「別の TASK のときに」は、下の「後に回してよい事情」に当たるときだけにする
- いまの PR の範囲に入る事項は、その PR で対応する
- 範囲に入らない事項は、同じ作業の流れの中で別の PR（要るなら Issue）を出して対応する。いまの PR に範囲の外の変更は混ぜない（[out-of-scope-tracking](./out-of-scope-tracking.md)）
- 自動実装のエージェント（implement-issue-tree の各段・builder）は、自分の PR の範囲の外を直さない。見つけた事項は作業の summary で main セッションへ返し、main セッションがその場で対応する

## 後に回してよい事情

| 事情 | 例 |
| ---- | -- |
| 対応に要る材料・決定がまだ無い | 前提のコードが無い、形を決める記録が無い（推測で形を決めない。[private-context](./private-context.md)・[contract-shaping-escalation](./contract-shaping-escalation.md)） |
| 承認が要る | 依存の追加・更新（[dependency-policy](./dependency-policy.md)）、機材での実行など、規約がユーザーの承認を求めるもの |
| 別の規約が順番を決めている | [docs/contract/README.md](../../docs/contract/README.md) の「更新の順」など、先行を禁じる規約があるもの |
| セキュリティ上の疑義 | 作業を止めてユーザーに警告する（[security](./security.md)） |
| ユーザーが範囲の外とした | ユーザーが明示に除いたもの |

## 後に回すときの書き方

- 当たる事情（上の表のどれか）と、再開の条件（何がそろったら対応するか）を、作業の summary とユーザーへの報告に書く。「後に回す」とだけ書いて終えない
- 再開の条件がそろったら、その時点で対応する
- 公開側に書く場合は、private spec 側の状態を書かない（[spec-confidentiality](./spec-confidentiality.md)）

## 関連規約

- [out-of-scope-tracking](./out-of-scope-tracking.md)・[private-context](./private-context.md)・[contract-shaping-escalation](./contract-shaping-escalation.md)・[dependency-policy](./dependency-policy.md)・[security](./security.md)・[spec-confidentiality](./spec-confidentiality.md)
