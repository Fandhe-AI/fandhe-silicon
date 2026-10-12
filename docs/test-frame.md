# 共通の試験の枠（TASK-23）の現状・前提・形の設計・現状の形

TASK-23（親 Issue #91）について、着手時点（1/8・#92）の現状と前提、2/8（#93）の形の設計、3/8（#94）で入れたコードの現状の形、4/8（#95）の形の設計、5/8（#96）で入れたコードの現状の形、6/8（#97）で入れたコードの現状の形を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文と決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

| 項目 | 所在 |
| ---- | ---- |
| タスクの中身と決定の記録 | private spec 側（`docs/spec/05-tasks.md` の TASK-23 と、そこから辿る決定の記録）。本リポには決定の内容・理由・経緯を置かない |
| 本リポの `docs/spec` の gitlink | `a7de5c2`（`update = none`。4/8 の着手時も、spec の main の先端と同じことを確かめた） |
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
組み合わせの値は完全一致だけを受ける。許可の値は未設定と空文字を「許可なし」とし、不正な要素・64 バイトを超える要素・16 件を超える件数・1039 バイト（16 件 × 64 バイトと区切りの 15 バイト）を超える値・UTF-8 でない値は「不正」とする。
テストはプロセスの環境を書き換えず、値を `from_lookup` へ渡す。

### 入れていないもの

- 結果のラベルの型・出力・集計、機械可読の記録、閾値の ID と承認の表（後続の段）
- ファイル・標準出力・標準エラーへの書き出し
- `parity` との接続（`parity` の公開面・値・規則は変えていない）
- 実機の device を取って宣言と照合する経路（`core` からは device を取れない。`contract` 側の受け口も足していない）
- 外部依存、`unsafe`、新しい feature・make ターゲット・CI ジョブ・`build-profiles` の組

### 公開テストから到達させていない経路

- `RunSetting::from_env` が読む実際の環境変数の値（プロセスの環境を書き換えないため。呼び出しが panic しないことだけを確かめている）

## 形の設計（4/8）

4/8（#95）の範囲は `docs/spec/05-tasks.md` の TASK-23 の内容の (2)。
具体（名前・型・値・欄・出力の先）は 5/8（#96）のコードと公開テストが示し、本節には先に書かない。
本節に書くのは、決定の記録のポインタ、範囲 (2) が乗る既存のコードの事実、どの形でも守る本リポの規約である。

### 決定の記録に従う点（ポインタ）

| 点 | 扱い |
| -- | ---- |
| 範囲 (2) の形 | `docs/spec/05-tasks.md` の TASK-23 と、そこから辿る決定の記録に従う。本書には内容を書かない |
| 出力の先・記録の形 | `docs/spec/05-tasks.md` の TASK-28 と、そこから辿る決定の記録に従う（「出力の先の記録の所在」の節と [measurement-record](./measurement-record.md) も参照） |
| 閾値の適用の範囲 | TASK-53（Issue #196）のポインタ。本節では決めない |
| 範囲 (3)（集計） | 本節では決めない（6/8〈#97〉が扱う） |
| 決定の記録から一意に決まらない点 | [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路で伺う。推奨案を既定として確定しない |

### 範囲 (2) が乗る既存のコードの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実行の可否の型 | `Decision`・`NotRunReason` | `crates/core/src/test_frame/decision.rs` |
| 宣言 | `Declaration` はフィールドが非公開で、後続で項目を足せる | `crates/core/src/test_frame/declaration.rs` |
| 比べの結果 | `parity` の各比較型の `is_match()` で取れる。`parity` は `test_frame` に依存していない | `crates/core/src/parity/` |
| device | `core` は device を取れず、`contract` 側の受け口はまだ無い | 「現状の形（3/8）」の「入れていないもの」 |
| 直列化の依存 | `crates/` に外部依存は 0 | `cargo metadata --format-version 1 --all-features --locked` |

### 範囲 (2) のどの形でも守る本リポの規約

F1〜F14 は引き続き当てる（再掲しない）。範囲 (2) に固有の点を足す。

| # | 点 | 守ること | 根拠 |
| - | -- | -------- | ---- |
| G1 | 結果・理由の型 | 列挙にし、文字列の表現は英語の ASCII。入力の値・自由な文を含めない | 3/8 の `NotRunReason` の先例、[security](../.claude/rules/security.md)、F10 |
| G2 | 機械可読の出力 | 組み立ては標準ライブラリだけで書き、外部依存を足さない。文字列を出力へ入れるときのエスケープと長さの上限をコードで持ち、公開テストで固定する | F4、[dependency-policy](../.claude/rules/dependency-policy.md) |
| G3 | 試験が宣言する識別子 | 信頼しない入力として検査する。文字の種類と長さを限り、外れは `Result` の `Err` にする。エラーは入力の値を写さない | `Permission` の先例、F9、F11 |
| G4 | ファイルへの書き出し | 経路を持つ場合、出力の先は信頼しない入力として扱う。明示の指定が無いときは書き出さない。指定の値をコマンドやパスへ未検証で連結しない。上位ディレクトリへ抜ける指定は拒む。件数とバイト数に上限を持つ | F9、[security](../.claude/rules/security.md) |
| G5 | 出力に含めない情報 | 実ホスト名・アドレス・ユーザー名・環境変数の値・絶対パス | F10 |
| G6 | 結果の決め方 | 純粋な関数に寄せ、入力以外の状態を読まない。書き出しとは分ける。テストはプロセスの環境と作業ツリーを書き換えない | `decide`・`from_lookup` の先例 |
| G7 | `parity` との関係 | 公開面・値・比べの規則を変えない。写像は `test_frame` の側に置き、`parity` を `test_frame` に依存させない | F7、[parity-tool](./parity-tool.md) の D6 |
| G8 | 承認の有無などの表 | コードに持つ場合は 1 か所に置く。表を変える PR は根拠の所在をポインタで引く。得ていない承認を書かない | [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)、[dependency-policy](../.claude/rules/dependency-policy.md) の承認の記録の先例 |
| G9 | 走らせなかった結果 | skip・ignore・アサーションの弱体化で CI を通さない。成功にも失敗にも読み替えない | F8、`decision.rs` のモジュール説明 |
| G10 | device | `core` から device を取る経路を作らない。照合で生じる結果は型に持たせてよいが、作る経路は `contract` 側の受け口が入る段で足す | F3、F6 |
| G11 | 追加物 | 新しい feature・make ターゲット・CI ジョブ・`build-profiles` の組を足さない。足す必要が出たら理由を PR 本文に書く | F12 |

### 5/8（#96）への引き継ぎ

- 出力の先に依らない作業: 結果・理由の型、`Decision`・`NotRunReason` からの写像、`parity` の結果からの写像、試験が宣言する識別子の受け口、承認の有無の表、それぞれの公開テスト
- 出力の先・記録の形に依る作業: 記録の組み立てと書き出し。出力の先の所在は下の「出力の先の記録の所在」のポインタに従う
- spec 側が「実装で決める」とする点は 5/8 がコードで決め、選んだ形を PR 本文に書く
- 着手時に gitlink の版の spec と決定の記録を当て直す。一意に導けない点は推奨案で確定せず伺う
- 公開アイテムを足したら、本書に「現状の形（5/8）」を足し、`README.md`・`CLAUDE.md`・crate 冒頭の `//!` を同じ PR で直す

## 現状の形（5/8）

本節はコードと公開テストから観測できる事実だけを書く。範囲は `docs/spec/05-tasks.md` の TASK-23 の内容の (2) のうち、出力の先に依らない部分。
追加分も feature `test-support` の裏にだけ公開される。綴りの表と上限の値はコードの定数が示す。

### 置き場

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| モジュール | `crates/core/src/test_frame/` に `identifier`・`threshold`・`label`・`outcome` の 4 ファイルを追加。`decision.rs` には `NotRunReason::name()` を足しただけ | 同ディレクトリ |
| テスト | `crates/core/tests/` の `test_frame_identifiers.rs`・`test_frame_label.rs`・`test_frame_outcome.rs`（`[[test]]` に `required-features = ["test-support"]`）。`crates/contract/tests/test_support_test_frame.rs` に到達性のテストを追記 | 各 `Cargo.toml` |
| `parity` | 差分なし。比較型から判定への写像（`From<&比較型> for Verdict`）は `test_frame` の側にある | `git diff origin/main -- crates/core/src/parity` |

### 公開アイテムと観測できる振る舞い

| アイテム | 観測できる振る舞い | 確かめ方（テスト名） |
| -------- | ------------------ | -------------------- |
| `TestId`・`BehaviorId` | 文字種と長さを限って検査し、外れは種類だけの `IdentifierError`（入力の値を含まない） | `test_id_accepts_valid_names`・`test_id_rejects_invalid_names`・`behavior_id_accepts_valid_names`・`behavior_id_rejects_invalid_names`・`identifier_errors_are_ascii_and_do_not_echo_input`・`length_limits_are_fixed` |
| `ThresholdId` | 正規の綴りだけを受ける。`is_approved()` は承認の表（`APPROVED_THRESHOLDS`。1 か所の定数）に載る ID だけ `true`。公開側の表は空で、どの ID も `false`（承認の状態は公開側に持たない） | `threshold_id_accepts_only_the_canonical_spelling`・`approval_table_is_empty_in_public_code`・`only_ids_in_the_table_are_approved` |
| `Label`・`IndeterminateReason` | 5 種のラベル。種類と理由のトークンは `[a-z0-9-]` で重複なし。`fails_test()` は `Ng` のときだけ `true`。表示は英語の ASCII | `kind_tokens_are_distinct_and_well_formed`・`reason_tokens_exist_only_for_kinds_with_a_reason`・`not_run_reason_tokens_cover_every_variant_without_duplicates`・`indeterminate_reason_tokens_cover_every_variant_without_duplicates`・`only_ng_fails_the_test`・`display_is_ascii_english` |
| `Subject` | 名前を受けて検査し、重複を取り除く。件数に上限がある | `subject_validates_and_deduplicates`・`subject_limits_counts`・`subject_errors_are_ascii_and_do_not_echo_input` |
| `judge`・`judge_with_approval` | `judge` は公開側の表だけを見る（表は空なので、閾値を宣言した試験は判定保留になる）。`judge_with_approval` は承認の判定を試験の側から渡す。順（上から先に当たったもの）: 実行の可否が `NotRun` なら未実施（本体は呼ばない）→ 本体が判定不能なら判定不能 → 宣言の閾値に未承認があれば判定保留（宣言の順で最初の ID）→ 一致なら OK、不一致なら NG。入力以外の状態を読まない | `not_run_reasons_pass_through_and_the_body_is_not_called`・`indeterminate_wins_over_pending_and_verdicts`・`public_table_is_empty_so_any_declared_threshold_gives_pending`・`unapproved_threshold_gives_pending_with_the_first_one_in_declared_order`・`approved_thresholds_map_verdicts_to_ok_and_ng`・`only_declared_combinations_run_over_all_seven` |
| `Outcome` | getter だけを公開し、`judge` 以外からは作れない。走らせた組み合わせは本体を呼んだときだけ `Some` | `outcome_getters_report_what_was_declared_and_run` |
| 失敗にするかの判定 | `Termination` は実装しない（走らせなかった結果・判定保留・判定不能が出力も記録もなく成功に見えるのを避ける。ラベルを記録・集計する経路ができるまで、試験の関数の戻り値には使えない）。呼び出し側が `Label::fails_test`（`Ng` のときだけ `true`）で明示的に扱う | `fails_test_is_true_only_for_ng` |
| `Verdict` と `parity` | `parity` の 5 つの比較型から `Verdict` への `From` を持つ（`is_match()` を読むだけ） | `from_match_maps_booleans`・`parity_comparisons_map_to_verdicts` |
| contract からの到達 | `contract` の `test-support` だけで届き、背後実装の feature に依らず同じラベル | `judge_is_reachable_and_fixed_under_any_backend_feature_set` |

### 入れていないもの

- 機械可読の記録の組み立て、ファイル・標準出力・標準エラーへの書き出し、出力の先の指定の受け口
- 組み合わせごとの集計（後続の段）
- 実機の device を取って宣言と照合する経路、`contract` 側の受け口（判定不能の値を作る経路は試験の側に残る）
- 比べの関数が返す `Err`（長さの不一致など）からラベルへの写像（扱いは試験の側に残る）
- 外部依存、`unsafe`、新しい feature・make ターゲット・CI ジョブ・`build-profiles` の組

### 公開テストから到達させていない経路

- `RunSetting::from_env` が読む実際の環境変数の値（3/8 と同じ）
- `judge` に渡した本体の panic（そのまま伝わる。ラベルにはしない）

### 6/8（#97）への引き継ぎ

- 未実施・判定保留・判定不能は終了コードが成功になる（`Ng` だけが失敗）。ラベルが残らない集計は成功を装うため、集計側で 5 種のラベルの件数を必ず拾い、`ok` 以外を見落とさない
- 集計は `Outcome` の getter の上に乗せる。形は決定の記録に従い、一意に決まらない点は推奨案で確定せず伺う
- 公開アイテムを足したら、本書の現状の形・`README.md`・`CLAUDE.md`・crate 冒頭の `//!` を同じ PR で直す

## 現状の形（6/8）

本節はコードと公開テストから観測できる事実だけを書く。範囲は `docs/spec/05-tasks.md` の TASK-23 の内容の (3) のうち、出力の先に依らない部分。
追加分も feature `test-support` の裏にだけ公開される。

### 置き場

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| モジュール | `crates/core/src/test_frame/summary.rs` を追加。`label.rs` には `LabelKind` と `Label::kind()` を足し、`Label::name()` は `kind().name()` を返す | 同ディレクトリ |
| テスト | `crates/core/tests/test_frame_summary.rs`（`[[test]]` に `required-features = ["test-support"]`）。`test_frame_label.rs` に `LabelKind` のテストを追記。`crates/contract/tests/test_support_test_frame.rs` に到達性のテストを追記 | 各 `Cargo.toml` |
| `parity`・依存 | 差分なし。外部依存は 0 のまま | `git diff origin/main -- crates/core/src/parity deps-approved.toml Cargo.lock` |

### 公開アイテムと観測できる振る舞い

| アイテム | 観測できる振る舞い | 確かめ方（テスト名） |
| -------- | ------------------ | -------------------- |
| `LabelKind` | ラベルの種類 5 つ（理由・閾値の ID を持たない）。`ALL` の順は ok・ng・pending・not-run・indeterminate。トークンは `Label::name()` と同じ | `label_kinds_are_five_distinct_tokens`・`kind_matches_name_for_every_label` |
| `Summary`・`LabelCounts` | `Outcome` の getter だけを入力に、`MachineCombination::ALL` の 7 行と全体の件数を数える。行は、走らせた組み合わせにそのラベルの種類を 1 件、宣言したが走らせていない組み合わせに `not-run` を 1 件、宣言していない組み合わせは数えない。全体は `Outcome` 1 件につき 1 件。5 種の件数を常に持つ | `empty_summary_has_seven_rows_of_zero`・`ran_outcomes_count_under_the_combination_they_ran_on`・`not_run_outcomes_count_under_every_declared_combination`・`declared_but_not_run_combinations_count_as_not_run`・`undeclared_combinations_are_not_counted`・`row_total_equals_outcomes_declaring_the_combination`・`totals_count_each_outcome_once_by_its_label` |
| 順序・足し方 | 結果は `Outcome` の順に依らない。`record` を繰り返した結果は `from_outcomes` と一致する | `result_does_not_depend_on_order_and_record_agrees_with_from_outcomes` |
| `has_ng` | `Ng` が 1 件でもあるときだけ `true`（`Label::fails_test` の集計版） | `has_ng_is_true_only_when_an_outcome_is_ng` |
| 表（`Display`） | Markdown の表。固定の見出しと区切り、`MachineCombination::ALL` の順の 7 行。セルは固定のトークンと 10 進の数だけで、試験の ID・ビヘイビアの ID・環境の値は出ない。英語の ASCII | `table_has_fixed_header_and_seven_rows_in_all_order`・`table_shows_counts_and_keeps_non_ok_columns`・`table_is_ascii_and_has_no_identifiers` |
| `SummaryError` | 件数のあふれは `CountOverflow`（`checked_add`。飽和も panic もしない。失敗した `record` は集計を変えない）。表示は英語の ASCII で入力の値を含まない | `summary_error_is_ascii` |
| contract からの到達 | `contract` の `test-support` だけで届き、背後実装の feature に依らず同じ件数 | `summary_is_reachable_and_fixed_under_any_backend_feature_set` |

### 入れていないもの

- 機械可読の記録の組み立て、ファイル・標準出力・標準エラーへの書き出し、出力の先の指定の受け口
- 複数の試験（別プロセスを含む）の結果を 1 つの `Summary` に集める経路、複数の run をまたぐ集計
- 未実施・判定不能の理由ごとの内訳、同じ試験の ID の重複の検出
- `Outcome` への `Termination` の実装（5/8 の扱いのまま）
- 実機の device を取って宣言と照合する経路、`contract` 側の受け口
- 外部依存、`unsafe`、新しい feature・make ターゲット・CI ジョブ・`build-profiles` の組

### 公開テストから到達させていない経路

- `CountOverflow` に至る経路（`usize::MAX` 件の入力が要るため作れない）
- `LabelCounts::total` の飽和の経路（同上）

### 実装で決めた点

- 複数の組み合わせを宣言した試験が 1 つで走ったとき、走らなかった宣言の行に `not-run` を 1 件ずつ積む
- 表の列の並び（`LabelKind::ALL` の順）、`has_ng` の導入

### 7/8（#98）への引き継ぎ

- 集計のテストは公開テストで固定済み。7/8 では不足する経路の洗い出しと追加をする
- 出力の先と形が決まるまで、記録の組み立て・書き出しと複数の試験をまたぐ集計の経路は入らない（spec 側の判断待ち。ポインタ: `docs/spec/05-tasks.md` の TASK-28・TASK-27）

## 出力の先の記録の所在

- 範囲 (2) の出力の先: `docs/spec/05-tasks.md` の TASK-28 と、そこから辿る決定の記録（ポインタのみ。本書には書かない）

## 後続の分担

| Issue | 段 |
| ----- | -- |
| #93 | 2/8 形の設計（本書の「形の設計（2/8）」） |
| #94 | 3/8 実装（済。本書の「現状の形（3/8）」） |
| #95 | 4/8 形の設計（済。本書の「形の設計（4/8）」） |
| #96 | 5/8 実装（済。本書の「現状の形（5/8）」） |
| #97 | 6/8 実装（済。本書の「現状の形（6/8）」） |
| #98 | 7/8 テスト |
| #99 | 8/8 確かめ |

- [parity-tool](./parity-tool.md) と [pixel-parity-tool](./pixel-parity-tool.md) の枠に関する記述は、置き場と範囲 (1) が入った段で現状に直した。接続の記述は、接続が入る PR で更新する。
- 完了の条件の確かめは未実施。#99 で、本リポで観測できる事実により埋める（条件の本文は転記しない）。
- 親 #91 の「進み具合を spec に記録する」は spec リポ側の作業。

## 確かめた日付

2026-10-12（1/8。base: `origin/main` の `3d2ef33`、gitlink: `c1cb4c9`）。

2026-10-12（2/8。base: `origin/main` の `048db9d`、gitlink: `0621b49`）。

2026-10-12（3/8。base: `origin/main` の `e671bc6`、gitlink: `a7de5c2`）。

2026-10-12（4/8。base: `origin/main` の `0975b66`、gitlink: `a7de5c2`）。

2026-10-12（5/8。base: `origin/main` の `065bd04`、gitlink: `a7de5c2`）。

2026-10-12（6/8。base: `origin/main` の `ca396fd`、gitlink: `a7de5c2`）。

## 関連

[parity-tool](./parity-tool.md)・[pixel-parity-tool](./pixel-parity-tool.md)・[ci-job-split](./ci-job-split.md)・[workspace-boundary](./workspace-boundary.md)・[contract/README](./contract/README.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)
