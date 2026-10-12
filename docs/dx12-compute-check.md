# DX12 の compute に関する点検（TASK-236）の現状と前提

TASK-236（親 Issue #461、1/8 は Issue #3932）について、着手時点の本リポの現状と前提、後続で決める論点を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・候補の名前・操作や型の名前・件数・判定・理由は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
本範囲では点検そのものを行わず、点検表も作らない。形・置き場・方針も決めない。

## 点検の入力の所在

- タスクの内容と完了の条件の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-236）
- 対象の項目のポインタ: `docs/spec/04-behavior/build-profiles.md` の BUILD-18・BUILD-64
- 点検の入力は private の submodule の中にだけあり、本リポに写しは無い。中身・状態・判定は本リポに書かない
- 前提タスクの有無は TASK-236 を参照する（ポインタのみ）
- 完了の条件との対応づけの記録の正も private spec 側にある（TASK-236 の状態。ポインタのみ）
- 上記のポインタが gitlink の指す commit に見出しとして在るかは、本範囲では確かめていない（未検証）

## 着手時点の本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| DX12 の compute の feature | `fandhe-silicon-contract` に `backend-dx12-compute` があり、`contract-core` を有効にするだけ。`default` には入っていない。コメントに受け持つタスクのポインタ（TASK-222）がある | `crates/contract/Cargo.toml` の `[features]` |
| DX12 のモジュール | `crates/contract/src/lib.rs` に DX12 のモジュール宣言は無く、`crates/contract/src/` に DX12 のファイルも無い | `crates/contract/src/lib.rs`、`ls crates/contract/src` |
| build-profiles の組 | `backend-dx12-compute` を含む組は `NATIVE_SETS`・`WASM_SETS` に無い。`make verify` は `--all-features` なので feature 自体は有効になる | `scripts/build-profiles.sh`、`scripts/verify.sh` |
| 製品の公開面 | 製品 crate の公開面はほぼ空で、点検の対象になる操作・型は本リポに無い | 各 crate の `src/lib.rs` |
| 領域ごとの契約の文書 | `docs/contract/` には書式・手順書だけがあり、領域ごとの文書は無い | `ls docs/contract` |
| `docs/spec` | gitlink で `update = none`。既定の clone では空で、`ci.yml` も取得しない | `.gitmodules`、`git ls-tree HEAD docs/spec`、`.github/workflows/ci.yml` |
| 外部依存 | 0 件。追加は承認制 | `cargo metadata --format-version 1 --all-features --locked`、[dependency-policy](../.claude/rules/dependency-policy.md) |
| 外部の一次資料の調べ方 | Direct3D の調査は reference-researcher が `windows-graphics-media` スキルで受け持つ | `.claude/agents/research/reference-researcher.md`、`skills-lock.json` |
| 持ち込みと spec の扱い | PoC の記録は本リポへ写さない。`docs/spec` 配下は書き換えない | [poc-carry-over](../.claude/rules/poc-carry-over.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) |

## 関連タスクの状態

`crates/contract/Cargo.toml` のコメントが指すタスクに限る。spec 内の依存の関係は書き起こさない。

| 対象 | Issue | 状態（2026-10-12 時点） |
| ---- | ----- | ------------------------ |
| TASK-236（親） | #461 | OPEN |
| TASK-222 | #1145 | OPEN |
| TASK-234 | #1154 | OPEN |

## 後続で決める論点

列挙のみ。向きは決めず、推奨案・既定も書かない。

| # | 論点 | 関わる制約（事実） |
| - | ---- | ------------------ |
| D1 | 点検表そのものの置き場（本リポの文書か、private spec 側か） | 入力が private 側にだけある。[spec-confidentiality](../.claude/rules/spec-confidentiality.md)（公開テストの無い約束・復元できる要約を書かない）、[poc-carry-over](../.claude/rules/poc-carry-over.md)、[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)（本リポから `docs/spec` を書き換えない）。前例として [implementation-scope](implementation-scope.md) は記録の正を private 側に置いた |
| D2 | 本リポの文書に残す範囲（ポインタだけか、確かめの方法と結果の有無までか。件数を公開側に書くか） | D1 と同じ |
| D3 | 外部の一次資料との突き合わせの記録の書き方（出典の示し方、確かめた範囲と確かめていない範囲の書き分け） | reference-researcher の報告形式、`windows-graphics-media` スキル |
| D4 | 見直しの提案が出た場合の経路 | 既存の規約で決まる（contract-shaping-escalation の「spec 側への差し戻しの提案手順」）。新しい経路は作らない |
| D5 | 確かめの記録の置き場と書式 | D1・D2 に従属 |

## オーナー判断待ち

本範囲で、本リポ側から新たに伺う論点は無い（ポインタ: TASK-236）。論点 D1〜D5 は 2/8・3/8 で整理し、6/8 で伺う。

## 後続の分担

| 番号 | Issue | 題名 |
| ---- | ----- | ---- |
| 1/8 | #3932 | 現状と前提の確認（本文書） |
| 2/8 | #3933 | 方針の整理 |
| 3/8 | #3934 | 形の設計 |
| 4/8 | #3935 | 確かめ |
| 5/8 | #3936 | 確かめ |
| 6/8 | #3937 | オーナーへの確認 |
| 7/8 | #3938 | 決定の記録 |
| 8/8 | #3939 | 確かめ |

spec 側への進み具合の記録は spec リポ側の作業で、本リポからは書き換えない。

## 確かめた日付

2026-10-12（base: `6cca009`、gitlink: `c1cb4c9`）

## 関連

- [implementation-scope](implementation-scope.md)・[workspace-boundary](workspace-boundary.md)・[contract/procedure](contract/procedure.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)
