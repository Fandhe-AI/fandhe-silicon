# private spec の機密保持規約（リポ固有・P0）

## 前提

- 本リポジトリは **public**
- 仕様・フェーズ文書の SSOT は **private** リポジトリ `Fandhe-AI/fandhe-silicon-spec`（`docs/spec` submodule。`update = none`）にあり、**意図的に非公開を維持**する

## 禁止事項

以下を public な資産（コード・コメント・ドキュメント・Issue・PR 本文・コミットメッセージ）へ持ち込まない:

- spec 本文の長文引用・ファイルコピー
- 非公開の内部判断・設計議論の転記
- spec の構成・内容を実質的に復元できる要約

## 許可される参照（ポインタ表記）

- spec 内のファイルパス（例: `docs/spec/<dir>/<file>.md`）
- タスク ID・ビヘイビア ID・フェーズ ID 等の識別子
- 1〜2 行程度の、README で**既にオーナーが公開済みの範囲**に収まる要約

## 運用

- 公開可否に迷った場合は**転記しない側**に倒し、ユーザーに確認する
- subagent への指示・subagent からの報告でも同じ規約を適用する（報告が PR 等へ転記されうるため）
- レビュー時（reviewer / security-auditor）は spec 漏えいを P0 として検査する

## コミットメッセージ・PR 本文での運用

- コミットメッセージ（件名・本文）・PR 本文・レビュー返信も public 資産であり、上記の禁止事項・ポインタ表記が同様に適用される
- squash merge 後もブランチ上の中間コミットは GitHub の PR refs（`refs/pull/<n>/head`）から参照可能であり、
  main の history rewrite では除去できない。**マージ前の検査で防ぐ**ことを原則とする
- レビュー指摘（spec 転記）への対応コミットでは、「何を削除したか」の説明に spec の内容を再転記しない。
  「private spec 由来の記述を削除（ID のポインタのみ残置）」のように ID ポインタと削除の事実だけを書く
- PR 作成時（create-pr スキル実行時）に `git log <base>..HEAD --format="%H %s%n%b"` で base から HEAD までの
  **すべての中間コミット**の件名・本文を検査し、禁止事項への抵触が無いことを確認する。抵触を検出した場合は
  ポインタ表記へ書き換えて解消するまで**マージを停止**する
- マージ後に発見した場合は Issue で扱いを決め、history rewrite は原則行わない
