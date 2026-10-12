# 共通の試験の枠（TASK-23）の現状・前提・形の設計・現状の形

TASK-23（親 Issue #91）について、着手時点（1/8・#92）の現状と前提、2/8（#93）の形の設計、3/8（#94）で入れたコードの現状の形を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-23 と、そこから辿る決定の記録）。本リポには決定の内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | `a7de5c2`（`update = none`） |
| gitlink と記録の前後 | 3/8 の着手時に、gitlink と spec の main の先端が同じことを確かめた（確かめ方: `git ls-tree HEAD docs/spec` と、読み取りで取得した spec の `git rev-parse HEAD`。取得物は本リポの worktree に置かず、作業の後に削除した）。後続の段も同じ方法で辿る |
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

## 現状の形（3/8）

本節はコードと公開テストから観測できる事実だけを書く。範囲は `docs/spec/05-tasks.md` の TASK-23 の内容の (1)（ポインタのみ）。
`test_frame` は feature `test-support` の裏にだけ公開され、どの crate の `default` にも入れていない。

### 置き場

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| モジュール | `crates/core/src/test_frame.rs` と配下 4 ファイル（`combination`・`declaration`・`setting`・`decision`） | `crates/core/src/test_frame/` |
| feature | `core` の `test-support`。`contract` の `test-support` が転送する。背後実装の feature は `core` に無い | 各 `Cargo.toml` |
| テスト | `crates/core/tests/` の `test_frame_types.rs`・`test_frame_decide.rs`、`crates/contract/tests/test_support_test_frame.rs`（いずれも `[[test]]` に `required-features = ["test-support"]`） | 各 `Cargo.toml` |
| 走る場所 | `make verify`（`--all-features`）と `make build-profiles` の `test-support` を含む組。GPU の実機は要らない | `scripts/verify.sh`・`scripts/build-profiles.sh` |

### 公開アイテムと観測できる振る舞い

| アイテム | 観測できる振る舞い | 確かめ方（テスト名） |
| -------- | ------------------ | -------------------- |
| `MachineCombination` | `ALL` は 7 値で重複なし。`name()` は ASCII の小文字・数字・ハイフンだけ。`from_name` は完全一致のみ（空白・大文字違いは `None`）。`backend()`・`machine()` は値ごとに固定 | `all_combinations_are_distinct`・`name_round_trips_for_every_combination`・`names_use_only_lowercase_digits_and_hyphen`・`backend_and_machine_are_fixed_per_combination`・`from_name_is_exact_match_only` |
| `Backend`・`Machine` | 名前だけの列挙。機材の呼び名は `Machine::name()` が返すものだけ | 同上 |
| `Permission` | `[a-z0-9-]`・1〜64 バイト。外れは `PermissionError`（種類だけで入力の値を含まない） | `permission_accepts_valid_names`・`permission_rejects_invalid_names`・`error_messages_are_ascii_and_do_not_echo_input` |
| `Declaration` | 組み合わせが空なら `Err`。重複は取り除かれ、結果は重複の有無に依らない。組み合わせ 64 件・許可 16 件を超えると `Err` | `declaration_requires_a_combination`・`declaration_limits_counts`・`declaration_drops_duplicates`・`from_parts_reports_permission_errors` |
| `declaration!` | `Declaration::from_parts` を呼ぶだけ。マクロ無しでも同じ宣言を書ける | `macro_matches_function_api` |
| `RunSetting` | `from_lookup` が検査と分類を行い、生成は失敗しない。`from_env` は `from_lookup` へ `std::env::var_os` を渡すだけ | `decide` の各テスト |
| 環境変数の名前 | `COMBINATION_ENV`（組み合わせの名前 1 つ）・`PERMISSIONS_ENV`（許可の名前のカンマ区切り）。綴りはコードの定数が示す | `environment_names_are_fixed_constants` |
| `decide` | 下の順で先に当たった条件が勝つ。入力以外の状態を読まず、panic しない | `earlier_conditions_win_over_later_ones`・`every_combination_runs_only_when_declared` ほか |
| `NotRunReason`・`Decision` | 理由は列挙で、`as_str()`・`Display` は英語の ASCII。入力の値を含まない | `reasons_and_decisions_have_ascii_text_without_input_values` |
| contract からの到達 | `contract` の `test-support` だけで届き、背後実装の feature に依らず `decide` の結果が同じ | `contract_test_support_enables_core_test_frame`・`decisions_are_fixed_under_any_backend_feature_set` |

`decide` の順（上から先に当たったもの）: 組み合わせの設定なし → 組み合わせの設定が不正 → 許可の設定が不正 → 宣言に無い組み合わせ → 要る許可が足りない → 走らせる。
不正な設定は宣言に関わらず走らせない側に倒し、「設定なし」と区別できる理由を返す。
組み合わせの値は完全一致だけを受ける。許可の値は未設定と空文字を「許可なし」とし、不正な要素・64 バイトを超える要素・16 件を超える件数・1024 バイトを超える値・UTF-8 でない値は「不正」とする。
テストはプロセスの環境を書き換えず、値を `from_lookup` へ渡す。

### 入れていないもの

- 結果のラベルの型・出力・集計、機械可読の記録、閾値の ID と承認の表（後続の段）
- ファイル・標準出力・標準エラーへの書き出し
- `parity` との接続（`parity` の公開面・値・規則は変えていない）
- 実機の device を取って宣言と照合する経路（`core` からは device を取れない。`contract` 側の受け口も足していない）
- 外部依存、`unsafe`、新しい feature・make ターゲット・CI ジョブ・`build-profiles` の組

### 公開テストから到達させていない経路

- `RunSetting::from_env` が読む実際の環境変数の値（プロセスの環境を書き換えないため。呼び出しが panic しないことだけを確かめている）

## 4/8（#95）への引き継ぎ

- 範囲 (2) が乗る型は `Decision`・`NotRunReason`（`crates/core/src/test_frame/decision.rs`）。`Declaration` はフィールドが非公開で、後続で変えられる
- 環境変数の名前（`COMBINATION_ENV`・`PERMISSIONS_ENV`）と許可の名前の形は、実装で決めた綴りである

## 出力の先の記録の所在

- 範囲 (2) の出力の先: `docs/spec/05-tasks.md` の TASK-28 と、そこから辿る決定の記録（ポインタのみ。本書には書かない）

## 後続の分担

| Issue | 段 |
| ----- | -- |
| #93 | 2/8 形の設計（本書の「形の設計（2/8）」） |
| #94 | 3/8 実装（済。本書の「現状の形（3/8）」） |
| #95 | 4/8 形の設計 |
| #96 | 5/8 実装 |
| #97 | 6/8 実装 |
| #98 | 7/8 テスト |
| #99 | 8/8 確かめ |

- [parity-tool](./parity-tool.md) と [pixel-parity-tool](./pixel-parity-tool.md) の枠に関する記述は、置き場と範囲 (1) が入った段で現状に直した。接続の記述は、接続が入る PR で更新する。
- 完了の条件の確かめは未実施。#99 で、本リポで観測できる事実により埋める（条件の本文は転記しない）。
- 親 #91 の「進み具合を spec に記録する」は spec リポ側の作業。

## 確かめた日付

2026-10-12（1/8。base: `origin/main` の `3d2ef33`、gitlink: `c1cb4c9`）。

2026-10-12（2/8。base: `origin/main` の `048db9d`、gitlink: `0621b49`）。

2026-10-12（3/8。base: `origin/main` の `e671bc6`、gitlink: `a7de5c2`）。

## 関連

[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)・[ci-job-split](./ci-job-split.md)・[workspace-boundary](./workspace-boundary.md)・[contract/README](./contract/README.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)
