# 共通の試験の枠（TASK-23）の現状・前提・形の設計

TASK-23（親 Issue #91）について、着手時点（1/8・#92）の現状と前提、および 2/8（#93）の形の設計を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-23 と、そこから辿る決定の記録）。本リポには決定の内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | `c1cb4c9`（`update = none`） |
| gitlink と記録の前後 | 未検証（2/8 の着手時に gitlink が `0621b49` に変わっていた。本段では spec を取得していない）。後続の段は、gitlink の更新が取り込まれるまで、spec の main を読み取りで取得して辿る（取得物は本リポの worktree に置かず、作業の後に削除する） |
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
| 既存の文書の言及 | 枠への言及は「委ねる」「まだ無い」のポインタだけ（[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)・[parity-non-finite](./parity-non-finite.md)） | `git grep -n "TASK-23" -- docs` |

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

## 形の設計（2/8）

2/8（#93）の範囲は `docs/spec/05-tasks.md` の TASK-23 の内容の (1)。
形のうち spec 側の決定が定める点は本書に書かず、ポインタだけを置く。具体（名前・型・シグネチャ）は 3/8（#94）のコードと公開テストが示す。
本節に書くのは、どの形でも守る本リポの規約と、その根拠である。

### 決定の記録に従う点（ポインタ）

| 点 | 扱い |
| -- | ---- |
| 範囲 (1) の形 | `docs/spec/05-tasks.md` の TASK-23 と、そこから辿る決定の記録に従う。本書には内容を書かない |
| 範囲 (1) の外の形 | 本節では決めない（4/8〈#95〉以降の段が扱う） |
| 機材の呼び名・実機での実行の条件 | spec 側の記録に従う（ポインタ: `docs/spec/05-tasks.md` の TASK-18・TASK-19・TASK-28）。本書には書かない |
| 決定の記録から一意に決まらない点 | 見つけた段で、その点だけを止めて [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路で伺う。推奨案を既定として確定しない |

### どの形でも守る本リポの規約

| # | 点 | 守ること | 根拠 |
| - | -- | -------- | ---- |
| F1 | PoC との関係 | PoC のファイル・記録・入力を複製せず、本リポの規約で新しく書く | [poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](./workspace-boundary.md) |
| F2 | workspace の境界 | workspace の外のコードを member や path 依存として加えない。member の増減は [workspace-boundary](./workspace-boundary.md) の記載（決定の記録のポインタ）に従い、本節では決めない | [workspace-boundary](./workspace-boundary.md)、`make deny` |
| F3 | 依存の向き | contract → core、exec → core、upper → core・exec を変えない。`core` はチップの crate に依存しない | [coding-rust](../.claude/rules/coding-rust.md) |
| F4 | 依存・`unsafe` | 標準ライブラリだけで書く。外部依存は 0 のまま（`deps-approved.toml` に足さない）。`unsafe` を足さない（個数テストの期待値を変えない） | [dependency-policy](../.claude/rules/dependency-policy.md)、[coding-rust](../.claude/rules/coding-rust.md)、[unsafe-policy](./unsafe-policy.md) |
| F5 | 本番のビルドとの分離 | テスト専用のコードを本番のビルドと `default` の feature に入れない。製品の経路（非テストのコード）から参照しない | [parity-tool](./parity-tool.md) の「現状の形」（`test-support` の先例）、各 `Cargo.toml` |
| F6 | 背後実装の選択 | 背後実装の選択は `contract` の feature と `cfg` で行う。`core` に backend の feature を足さない（`core` の中のコードは backend の feature の `cfg` で分岐できない） | [coding-rust](../.claude/rules/coding-rust.md)、[parity-tool](./parity-tool.md) の「現状の形」の「背後実装の feature での分岐」の行 |
| F7 | 既存の `parity` | 公開面・値・比べの規則を変えない。接続の形は本節で決めない | [parity-tool](./parity-tool.md) の「値・規則を変える変更の扱い」と D6、[pixel-parity-tool](./pixel-parity-tool.md) の D8 |
| F8 | 走らせない試験の扱い | skip・ignore・アサーションの弱体化で CI を通す形にしない。能力・機材が無いときに暗黙のフォールバックをしない | [coding-rust](../.claude/rules/coding-rust.md) の「テスト」「エラーハンドリング」 |
| F9 | 入力の検査 | 外から受け取る値は、信頼しない入力として検査する。想定しない値で安全でない側に倒さない（fail-closed）。値をコマンドライン・ソースの組み立てに連結しない | [security](../.claude/rules/security.md) の「OWASP Top 10 観点」 |
| F10 | 出力に書く情報 | 実ホスト名・アドレス・ユーザー名・鍵のパス・資格情報を、コード・テスト・出力に書かない。出力の文字列は英語、コメントと rustdoc は日本語 | [security](../.claude/rules/security.md)、[japanese-style](../.claude/rules/japanese-style.md)、[code-comment-style](../.claude/rules/code-comment-style.md) |
| F11 | エラーと計算 | ライブラリの側は `Result` で返し、panic させない。サイズ・件数の計算は `checked_*` で行う | [coding-rust](../.claude/rules/coding-rust.md)、[security](../.claude/rules/security.md) |
| F12 | 走る場所 | `make verify`（`--all-features`）と `make build-profiles` の既存の組で、GPU の実機なしに走る。make ターゲット・CI ジョブ・feature の組を足す必要が出た場合は、理由を PR 本文に書く。必須チェックの登録は本リポの PR では変えない | [ci-job-split](./ci-job-split.md)、`scripts/verify.sh`、`scripts/build-profiles.sh` |
| F13 | テストの置き場 | 統合テストは crate の `tests/` に置く。feature の裏のコードを使うテストは、`Cargo.toml` の `[[test]]` に `required-features` を書く | `crates/core/Cargo.toml`・`crates/contract/Cargo.toml` の先例 |
| F14 | rustdoc と文書 | 公開アイテムに `///`、モジュールの入口に `//!` を付ける。書くのは、コードと公開テストが示す振る舞いだけ | [spec-confidentiality](../.claude/rules/spec-confidentiality.md)、[code-comment-style](../.claude/rules/code-comment-style.md) |

### 3/8（#94）への引き継ぎ

- 形の具体は、3/8 の PR のコードと公開テストが示す。本書に先に書かない
- 3/8 は、gitlink の更新が取り込まれるまで、spec の main を読み取りで取得して決定の記録を辿る（取得物は本リポの worktree に置かず、作業の後に削除する）
- 3/8 で公開アイテムを足したら、本書に「現状の形」節を足し、公開アイテム・観測できる振る舞い・確かめ方（テスト名）を表で書く（書式は [parity-tool](./parity-tool.md) の「現状の形」に合わせる）
- 公開面が変わる crate の説明（crate 冒頭の `//!`・`README.md`・`CLAUDE.md`）は、公開面が変わる PR で更新する

## オーナー判断待ち

2/8（#93）の時点で、範囲 (1) の形について本リポ側から新たに伺う論点は見つかっていない。後続で見つかった場合だけ、同じ形式で追記する。

## 後続の分担

| Issue | 段 |
| ----- | -- |
| #93 | 2/8 形の設計（本書の「形の設計（2/8）」） |
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

2026-10-12（1/8。base: `origin/main` の `3d2ef33`、gitlink: `c1cb4c9`）。

2026-10-12（2/8。base: `origin/main` の `048db9d`、gitlink: `0621b49`）。

## 関連

[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)・[ci-job-split](./ci-job-split.md)・[workspace-boundary](./workspace-boundary.md)・[contract/README](./contract/README.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)
