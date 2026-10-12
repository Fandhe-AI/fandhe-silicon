# 共通の試験の枠（TASK-23）の現状と前提

TASK-23（親 Issue #91）について、着手時点（1/8・#92）の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-23 と、そこから辿る決定の記録）。本リポには決定の内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | `c1cb4c9`（`update = none`） |
| gitlink と記録の前後 | 未検証（本段では spec を取得していない）。後続の段が gitlink から記録を辿れるかは、取り込みの後に確かめる |
| gitlink の更新 | `update-external.yml` の PR 経由。本作業では進めない |
| 完了の条件との対応づけの記録 | spec 側が正。本リポからは書き換えない |

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | PR #3 で追加済み。`cargo metadata --format-version 1 --all-features --locked` で member 4・パッケージ 4・外部依存 0 件（[parity-tool](./parity-tool.md) の既存の記述と同じ） |
| TASK-18（CI の分け方） | 済 | Issue #72 が CLOSED、PR #7048 がマージ済み。現状は [ci-job-split](./ci-job-split.md)。決定の内容は書かない（正は spec 側） |

## 着手時点の本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | member 4（core / contract / exec / upper）。外部依存 0 | `cargo metadata --format-version 1 --all-features --locked` の集計 |
| 結果を比べる道具 | `crates/core/src/parity.rs` と配下 5 モジュール（`cpu_reference`・`pixel`・`special_values`・`tolerance`・`topk`）。`core` の feature `test-support` の裏 | `crates/core/src/parity/`、[parity-tool](./parity-tool.md) |
| feature の転送 | `contract` の `test-support` が `core` へ転送する。`exec`・`upper` に転送は無い | 各 `Cargo.toml` |
| `core` の feature | backend の feature は無い（制約は [parity-tool](./parity-tool.md) の「走る場所」の行） | `crates/core/Cargo.toml` |
| `test-support` を含む feature の組 | `scripts/build-profiles.sh` の `NATIVE_SETS` に 4 組。`make verify` は `--all-features` で流れる | 同スクリプト |
| 既存のテスト | `crates/core/tests/`: `parity_*.rs` の 5 本。`crates/contract/tests/`: `backend_*`・`common_*`・`test_support_*` ほか（`common/` を含む） | `ls crates/*/tests` |
| CI | GPU・self-hosted のジョブは無い。表は複製せず [ci-job-split](./ci-job-split.md) を指す | 同文書 |
| 試験の枠・結果のラベルの出力・集計に当たるコード・テスト | 0 件 | `git grep -n -i -e 試験の枠 -e test-frame -e harness -- crates scripts` |
| 既存の文書の言及 | 枠への言及は「委ねる」「まだ無い」のポインタだけ（[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)・[parity-non-finite](./parity-non-finite.md)・[vulkan-extraction-tool](./vulkan-extraction-tool.md)） | `git grep -n "TASK-23" -- docs` |

## 関連タスクの状態

状態は 2026-10-12 時点の `gh issue view` による。向きや関係の説明は書かない。

| 関連 | Issue | 状態 |
| ---- | ----- | ---- |
| TASK-24 | #100 | CLOSED |
| TASK-26 | #114 | CLOSED |
| TASK-25 | #109 | CLOSED |
| TASK-18 | #72 | CLOSED |
| TASK-112 | #393 | OPEN |
| TASK-53 | #196 | OPEN |
| TASK-6 | #35 | CLOSED |
| ID のみ | — | TASK-19・TASK-20・TASK-21・TASK-27・TASK-28・TASK-51・TASK-106・TASK-110・TASK-922 |

- 依存: 外部依存は 0。追加が要るときは [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに乗せる。

## 後続で扱う論点

名前だけを挙げる。推奨案・答えは本書に書かない。

- 置き場
- 試験の宣言の形
- 結果の出力
- 集計
- 既存の `parity` との接続
- CI との関係

決定の記録は spec 側にあり、後続はそれに従う。一意に決まらない点は [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路でオーナーに伺う。

## オーナー判断待ち

本範囲で本リポ側から新たに伺う論点は無い。後続で見つかった場合だけ、同じ形式で追記する。

## 後続の分担

| Issue | 段 |
| ----- | -- |
| #93 | 2/8 形の設計 |
| #94 | 3/8 実装 |
| #95 | 4/8 形の設計 |
| #96 | 5/8 実装 |
| #97 | 6/8 実装 |
| #98 | 7/8 テスト |
| #99 | 8/8 確かめ |

- [parity-tool](./parity-tool.md) と [pixel-parity-tool](./pixel-parity-tool.md) の枠に関する記述は、実装とテストが入る PR で更新する。
- 完了の条件の確かめは未実施。#99 で、本リポで観測できる事実により埋める（条件の本文は転記しない）。
- 親 #91 の「進み具合を spec に記録する」は spec リポ側の作業。

## 確かめた日付

2026-10-12（base: `origin/main` の `3d2ef33`、gitlink: `c1cb4c9`）。

## 関連

[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)・[ci-job-split](./ci-job-split.md)・[workspace-boundary](./workspace-boundary.md)・[contract/README](./contract/README.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)
