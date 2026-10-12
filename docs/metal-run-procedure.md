# Metal の実機での試験の実行手順（TASK-19）の現状と前提

TASK-19（親 Issue #76）について、着手時点（1/4・#77）の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-19 と、そこから辿る決定の記録）。本リポには決定の内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | `0621b49`（`update = none`）。spec の main の先端と同じ |
| gitlink の更新 | `update-external.yml` の PR 経由。本作業では進めない |
| 完了の条件との対応づけの記録 | spec 側が正。本リポからは書き換えない |

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-18（CI の分け方） | 済 | Issue #72 が CLOSED、PR #7048 がマージ済み。現状は [ci-job-split](./ci-job-split.md)。決定の内容は書かない（正は spec 側） |

## 着手時点の本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| Metal の背後実装 | `crates/contract/src/backend_metal.rs` は骨格のみ（6 行）。feature `backend-metal` の裏。`artifact-inspection` が `backend-metal` を要求する | `wc -l crates/contract/src/backend_metal.rs`、`crates/contract/Cargo.toml` |
| 外部依存 | workspace の member 4・パッケージ 4（外部依存 0） | `cargo metadata --format-version 1 --all-features --locked` の集計 |
| `backend-metal` を含む feature の組 | `scripts/build-profiles.sh` に 4 か所 | `grep -n backend-metal scripts/build-profiles.sh` |
| CI | GPU・self-hosted のジョブは無い。`macos-latest` は `test-hooks` の matrix だけ。表は複製せず [ci-job-split](./ci-job-split.md) を指す | `.github/workflows/ci.yml` の `runs-on` |
| 実機が要るテストの分離 | `#[ignore]` は 0 件 | `grep -rn '#\[ignore' crates` |
| 実行手順・記録の書式を扱うスクリプト | `scripts/` と `scripts/hooks/` に該当するものは無い | `ls scripts scripts/hooks` |
| 記録の置き場 | `records/` ディレクトリは無い | `ls -d records` |
| 既にある書式の文書 | [wgpu-baseline-record](./wgpu-baseline-record.md)・[measurement-record](./measurement-record.md)（TASK-19 の書式は 0 件） | `ls docs` |
| 既存の文書の言及 | [ci-job-split](./ci-job-split.md)・[measurement-record](./measurement-record.md)・[test-frame](./test-frame.md) に ID のポインタがある | `git grep -n "TASK-19" -- docs README.md CLAUDE.md` |

## 関連タスクの状態

状態は 2026-10-12 時点の `gh issue view` による。向きや関係の説明は書かない。

| 関連 | Issue | 状態 |
| ---- | ----- | ---- |
| TASK-18 | #72 | CLOSED |
| TASK-20 | #81 | OPEN |
| TASK-21 | #86 | OPEN |
| TASK-23 | #91 | OPEN |
| TASK-27 | #121 | OPEN |
| TASK-28 | #130 | CLOSED |
| TASK-110 | #391 | OPEN |
| TASK-112 | #393 | OPEN |
| TASK-2943 | #396 | OPEN |

## 後続で扱う論点

名前だけを挙げる。推奨案・答えは本書に書かない。

- 手順書の置き場と形式
- 記録の書式の置き場と、既存の書式の文書との関係
- 手順を助けるスクリプトの要否と `scripts/`・`Makefile` の規約
- CI との関係
- 確かめの方法

決定の記録は spec 側にあり、後続はそれに従う。

## オーナー判断待ち

1/4（#77）の時点で、本リポ側から新たに伺う論点は無い。後続で一意に決まらない点が見つかった段は、[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の形式で、その段の PR 本文に残す。

## 後続の分担

| Issue | 段 |
| ----- | -- |
| #78 | 2/4 手順書の作成 |
| #79 | 3/4 形の設計 |
| #80 | 4/4 確かめ |

- 完了の条件の確かめは未実施。#80 が、本リポで観測できる事実により埋める（条件の本文は転記しない）。
- 親 #76 の「進み具合を spec に記録する」は spec リポ側の作業。

## 確かめた日付

2026-10-12（1/4。base: `origin/main` の `bf2cf48`、gitlink: `0621b49`）。

## 関連

[ci-job-split](./ci-job-split.md)・[measurement-record](./measurement-record.md)・[wgpu-baseline-record](./wgpu-baseline-record.md)・[test-frame](./test-frame.md)・[baseline-wgpu](./baseline-wgpu.md)・[workspace-boundary](./workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)
