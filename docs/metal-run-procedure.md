# Metal の実機での試験の実行手順（TASK-19）の現状と前提・実行手順

TASK-19（親 Issue #76）について、着手時点（1/4・#77）の現状と前提、および実行手順（2/4・#78）と記録の書式（3/4・#79）を、本リポで公開済みの事実・手順・ポインタだけで記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-19 と、そこから辿る決定の記録）。本リポには決定の内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | `a7de5c2`（`update = none`。3/4 の base で `git ls-tree` により確認。1/4 の時点は `0621b49`） |
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
| 実行手順・記録の書式を扱うスクリプト | 1/4 の時点では無かった。3/4 で記録の形の検査が入った（「記録の書式（3/4）」の節） | `ls scripts scripts/hooks` |
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

- 手順を助けるスクリプトの要否と `scripts/`・`Makefile` の規約
- CI との関係
- 確かめの方法

決定の記録は spec 側にあり、後続はそれに従う。

## 実行手順（2/4）

実機の上で試験を流すセッションが読む手順の入口。決定の所在は private spec 側（`docs/spec/05-tasks.md` の TASK-18・TASK-19・TASK-28 と、そこから辿る決定の記録）。
本リポのコードと公開テストで裏付けられていない具体（集計値・取得コマンド・採る時点・持ち帰りの手順・書かないものの一覧など）は、実装と公開テストの PR まで本書に書かない。実行のときは spec 側の決定の記録に従う。

### 適用範囲と前提

- 対象は Metal の実機（Mac mini）で流す試験。実機の試験は CI に載っていない（事実は [ci-job-split](./ci-job-split.md)）
- 本書は手で流す手順の枠。対応するスクリプトが本リポに入った PR で、該当の手順をそのスクリプトの呼び出しに置き換える
- 記録の形は「記録の書式（3/4）」の節が指すスクリプトと回帰テストが示す。本書は欄を列挙しない

### 役割

実機に届く端末で開いたセッションが流す。自動実装のホストからは実機へ接続しない。

| 作業 | 担当 |
| ---- | ---- |
| 専有の許可、時間帯の決定、回のあいだ止めるものを止める | オーナー |
| 導入・設定の変更・管理者権限の操作 | オーナー（本手順に含まない） |
| 準備・専有の確かめ・実行・記録・持ち帰り | セッション |

### 着手前の確認

1. 専有の許可と時間帯を、回のたびにオーナーへ確かめる
2. 流す対象の commit を決め、worktree が clean であること
3. 確かめの結果の置き先は spec 側（形の検査は「記録の書式（3/4）」の節）

どれかが満たせないときは始めない。専有の確かめの方法と、準備・実行の規則・持ち帰りは、spec 側の決定の記録（TASK-19・TASK-28）に従う。

### 実行のコマンド

本リポで今確かめられる範囲だけを挙げる。

```sh
cargo test -p fandhe-silicon-contract --features backend-metal
```

- 現状の `backend_metal.rs` は骨格のみで、実機でしか確かめられない動作の試験はまだ無い。実機での動作の証拠は、そうした試験が入った PR で手順に足す
- feature の組は `scripts/build-profiles.sh` を指す
- 試験の枠が読む設定は、枠の実装の PR のコードとテストが示す（TASK-23・[test-frame](./test-frame.md)）
- コンパイルの成功を、実機での動作の証拠にしない
- 受け取った文字列をコマンドに連結しない。値は引用する

### 記録と持ち帰り

- 回ごとの記録は、持ち帰る前に `scripts/session-record-check.sh <path>` で形を確かめる（終了コード 0 が合格）。欄は本書に書かず、検査のコードが示す。決定の所在は TASK-28 の決定の記録から辿る先
- 値を含む記録は spec リポ側への PR で戻す。本リポの Issue・PR には「流した事実」と「記録の正は spec 側」のポインタだけを書く
- バイナリは git に入れない
- 実ホスト名・アドレス・資格情報などの秘密情報は書かない（[security](../.claude/rules/security.md)）

## 記録の書式（3/4）

回ごとの記録（専有の確かめと時間帯の調整の記録）の形は、本リポのスクリプトと回帰テストが示す。欄の一覧は本書に複製しない（正は `scripts/session-record-check.sh` の定数表）。欄の意味と決定の所在は spec 側（ポインタ: TASK-19・TASK-28）。

### 現状の形

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入口と実行環境 | `scripts/session-record-check.sh <path>`。make ターゲットは持たない（引数が要るため）。bash 3.2 互換・Python 3.8 以上の標準ライブラリだけ | スクリプトの先頭コメント |
| 入力 | JSON 1 ファイル（通常ファイル・1 MiB 以下・BOM なしの UTF-8・トップレベルはオブジェクト） | 回帰テストの `input:` のケース |
| 終了コード | 0 合格 / 1 違反 / 2 誤用・前提の不足（パスが無い場合を含む） | 回帰テストの `misuse:` のケース |
| 欄の集合の検査 | 表にある欄がすべてあり、ほかの欄・重複した欄が無いこと | 回帰テストの `set:` のケース |
| 値の形の検査 | 型・語の形・時刻の形・版・`machine` の受ける値 | 回帰テストの `type:`・`form:`・`word:` のケース |
| 値の一覧を検査していない欄 | 2 欄は語の形だけを見る | 定数表の `word`、回帰テストの `word:` のケース |
| 出力 | 入力の値と、表に無い欄の名前を出さない | 回帰テストの `secret:` のケース |
| 記入例 | [session-record.example.json](./session-record.example.json)（値はすべてダミー）。回帰テストが合格を確かめる | `scripts/session-record-check.sh docs/session-record.example.json` |
| 回帰テストと CI | `make test-session-record-check`。CI は既存の `deny` ジョブが呼ぶ（check 名は増えない） | `.github/workflows/ci.yml` |
| 入れていないもの | 欄どうしの前後関係、run ごとの記録との突き合わせ、低電力モードの値による採否 | スクリプトの先頭コメント |

### 実装で決めた形

版の値・ID の形・時刻の形・`machine` の受ける値は、コードの定数が示す。綴りは実装で決めたもので、`machine` は `crates/core/src/test_frame/combination.rs` の `Machine::name` と揃える（回帰テストの `sync:` のケース）。

### 保留

値の一覧を検査していない 2 欄は spec 側の判断待ち。決まるまで値の一覧を固定しない。

### 4/4（#80）への引き継ぎ

#80 が本リポで観測できる事実で確かめる対象は、スクリプト・回帰テスト・CI のステップ・記入例の所在。実機で流した記録の実物は spec 側にある。

## 後続の分担

| Issue | 段 |
| ----- | -- |
| #78 | 2/4 手順書の作成（本書の実行手順） |
| #79 | 3/4 記録の書式（形の検査・回帰テスト・記入例） |
| #80 | 4/4 確かめ |

- 完了の条件の確かめは未実施。#80 が、本リポで観測できる事実により埋める（条件の本文は転記しない）。
- 手順のスクリプト（開発者ディレクトリの指定・試験の枠の設定・置き換えの道具）は #78〜#80 の範囲に明記が無い。記録の形の検査は 3/4 の範囲で入った。
- 親 #76 の「進み具合を spec に記録する」は spec リポ側の作業。

## 確かめた日付

2026-10-12（1/4。base: `origin/main` の `bf2cf48`、gitlink: `0621b49`）。

2026-10-12（2/4。base: `origin/main` の `dcd2237`、gitlink: `a7de5c2`）。

2026-10-12（3/4。base: `origin/main` の `0975b66`、gitlink: `a7de5c2`）。

## 関連

[ci-job-split](./ci-job-split.md)・[measurement-record](./measurement-record.md)・[inventory-trace-check](./inventory-trace-check.md)・[wgpu-baseline-record](./wgpu-baseline-record.md)・[test-frame](./test-frame.md)・[baseline-wgpu](./baseline-wgpu.md)・[workspace-boundary](./workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)・[procedure](./contract/procedure.md)・[security](../.claude/rules/security.md)
