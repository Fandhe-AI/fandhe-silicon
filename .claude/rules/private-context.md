# private の抜粋の読み方（自動実装のエージェント向け・P0）

## 目的・適用範囲

公開の Issue の本文が `docs/spec` の TASK を指すだけのとき（例: 「`docs/spec/05-tasks.md` の TASK-N を進める」）に、
エージェントが実装の材料をどこから読むかを決める。implement-issue-tree の各段（計画・実装・レビュー・PR 作成・修正・親を閉じる段）にも適用する。

## 置き場

- `$(git rev-parse --path-format=absolute --git-common-dir)/private-context/TASK-<N>.md`
- `.git` の中なので git の管理の外にあり、commit・push されない。worktree からも同じコマンドで同じパスに届く
- 置くのは main セッションだけ。エージェントは作らない・書き換えない・消さない

## 読み方

1. Issue の本文から TASK の ID を取り、上のファイルを読む。子の Issue は、親の Issue が指す TASK の ID を使う
2. 抜粋があれば、それを内容・成果物・完了の条件・決定の正として扱う。抜粋と Issue の本文が食い違うときは抜粋に従う
3. 抜粋が無いとき、または要る決定が抜粋に無いときは、実装せずに止め、理由を返す。推測で形を決めない
4. `docs/spec` の clone や submodule の取得を自分でしない。private の spec リポ（`Fandhe-AI/fandhe-silicon-spec`）には、push・PR・Issue・コメントのどれも書き込まない

## 公開側に書かないもの

公開側（コード・コメント・文書・コミットメッセージ・PR 本文〔「対象外」の欄を含む〕・Issue のコメント・レビュー返信・親の Issue を閉じるときのコメント）には、次を書かない。

- 抜粋の文・表・節の書き写しや言い換え
- [spec-confidentiality](./spec-confidentiality.md) の禁止事項（private spec 側の状態・日付・内部判断など）

公開側に書けるのは、ID のポインタと「private spec 側の記録に従う（ポインタのみ）」の 1 行まで。
公開 API の rustdoc と契約の文書は、[spec-confidentiality](./spec-confidentiality.md) の「公開 API の振る舞いの契約の書き方」のとおり、コードと公開テストが示す振る舞いだけを書く。

## 受け入れ条件と親を閉じる段

- 受け入れ条件に spec への記録や照合の行があっても、エージェントは実行しない。spec 側の作業で、main セッションが行う。その行にチェックも付けない
- 親の Issue を閉じるときのコメントは、マージした PR の番号と「private spec 側の記録に従う（ポインタのみ）」だけにする

## レビュー

- レビューとセキュリティレビューでは、差分・コミットメッセージ・PR 本文を抜粋と突き合わせ、抜粋の文の書き写し・言い換えと、状態の語が無いかを P0 として見る

## 関連規約

- [spec-confidentiality](./spec-confidentiality.md)・[contract-shaping-escalation](./contract-shaping-escalation.md)・[security](./security.md)
