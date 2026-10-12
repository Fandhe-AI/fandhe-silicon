# Vulkan の成果物からオペランドの宣言を取り出す道具（TASK-98）の現状・前提・方針・形（案）

TASK-98（親 Issue #387）について、現状と前提（1/6、#3530）・方針（2/6、#3531）・形（3/6、#3532）を、本リポで公開済みの事実とポインタだけで記録する。
5/6（#3534）で、別 crate からの統合テストと境界のテストを足した（「公開テスト（5/6）」節）。
6/6（#3535）で、本リポで観測できる事実による完了の条件の確かめを記録した（「完了の条件の確かめ（6/6）」節）。TASK-98 の完了は本書では宣言しない。
spec の本文・PoC の方式と結果は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-98 と `docs/spec/03-poc/kernel-artifacts-and-parity/` を参照する（ポインタのみ）。
道具の主題は「Vulkan の成果物（SPIR-V）からオペランドの宣言を取り出す道具」まで。
方針と形は案であり、オーナーの確認を経るまで確定扱いにしない。識別子・ファイル名はすべて仮。

## 現状と前提

1/6（#3530）の記録は [vulkan-extraction-tool](vulkan-extraction-tool.md) を参照する。

## 方針の整理（2/6。案）

### 方針が拠る本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| Vulkan の背後実装のモジュール | `crates/contract/src/backend_vulkan.rs` は doc コメントだけの骨格。`backend_vulkan/` ディレクトリは無い | ファイルの中身、`ls crates/contract/src` |
| cfg ゲート | `backend-vulkan` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| `unsafe` の個数テスト | `backend_vulkan/` 配下に期待値の表に無い `.rs` があると失敗する。個数 0 の行の追加はレビューで足りる変更で、個数の増加はオーナーの明示の承認が要る | [unsafe-policy](unsafe-policy.md)、`crates/contract/tests/backend_vulkan_unsafe_count.rs` |
| テスト・診断専用の feature | `contract` に `test-support`（`core` の同名 feature へ転送）・`fault-injection`・`artifact-inspection`（定義は `["backend-metal"]`）。どれも `default` に入らない | `crates/contract/Cargo.toml` |
| feature の組 | `scripts/build-profiles.sh` の native の組に、`backend-vulkan` を含む組・`test-support` を含む組・両方を含む組がある。`artifact-inspection` の組は 1 つで Metal を含意する。wasm32 の組では Vulkan のモジュールはコンパイル対象外 | `scripts/build-profiles.sh`、`crates/contract/src/lib.rs` |
| `make verify` | `--all-features` で clippy と test を流す（feature の裏のコードも対象） | `scripts/verify.sh` |
| 外部依存 | 0 件 | `cargo metadata --format-version 1 --all-features --locked`、`Cargo.lock` |
| CI が導入する道具 | editorconfig-checker・shellcheck・cargo-deny と Rust toolchain。SPIR-V のコンパイラ・逆アセンブラは導入していない | `.github/workflows/ci.yml` |
| SPIR-V を扱うコード・入力 | 0 件 | `crates`・`docs`（`docs/spec` を除く）・`scripts` を `spirv`・`spir-v`・`glsl` で grep（大文字小文字を区別しない） |
| オペランドの宣言の型 | `core` の公開面は `test-support` の裏の `parity` だけ。宣言を表す型は本リポにまだ無い（受け持つタスクのポインタ: TASK-470） | `crates/core/src/lib.rs` |

### 既存の決定・規約から導ける方針

| # | 点 | 方針 | 根拠 |
| - | -- | ---- | ---- |
| D1 | PoC からの持ち込みか書き直しか | PoC のファイル・記録・入力・コンパイル済みの成果物を複製せず、本リポの規約で新しく書く | TASK-6 の決定の記録（ポインタ）、[poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |
| D2 | 依存・`unsafe` | 標準ライブラリだけで書く。外部依存は 0 のまま。`unsafe` を使わない（個数の期待値を増やさない） | [dependency-policy](../.claude/rules/dependency-policy.md)、[coding-rust](../.claude/rules/coding-rust.md)、[unsafe-policy](unsafe-policy.md) |
| D3 | 契約の経路との関係 | 本タスクが作るのは道具まで。契約の公開の入口・能力の問合せ・エラーの型を足さない・変えない。`docs/contract/` に領域の文書を作らない。関連: TASK-418・TASK-448・TASK-513（ポインタのみ） | [docs/contract/README.md](contract/README.md) |
| D4 | 入力の扱い | モジュールは信頼しない入力として読む。語数・オフセット・長さの計算は `checked_*` で行い、範囲の外を読まない。不正な入力は panic させず `Result` で返す。エラーの表示は英語 | [security](../.claude/rules/security.md)、[coding-rust](../.claude/rules/coding-rust.md)、[japanese-style](../.claude/rules/japanese-style.md) |
| D5 | 静的に決まらない量があるとき | 0 や推測で埋めず、「決まらない」ことを区別できる形で返す。その扱いの決定は本タスクで行わない | [coding-rust](../.claude/rules/coding-rust.md)（暗黙のフォールバックをしない） |
| D6 | CI で走る確かめ | 外部の CLI・GPU の実機・`docs/spec` の取得に依らない。`make verify` と `make build-profiles` の既存の組で走らせ、新しい make ターゲット・CI ジョブを足さない（Q2 の回答で組の追加が要る場合を除く）。実機を通す確かめは本タスクの道具のテストに含めない | [coding-rust](../.claude/rules/coding-rust.md)（テスト）、`.github/workflows/ci.yml` |
| D7 | 文書に書く範囲 | 本書に PoC の方式・結果・値・件数を持たない。4/6 以降は、コードと公開テストから観測できる事実を「現状の形」として書く | [spec-confidentiality](../.claude/rules/spec-confidentiality.md) |

## 形の設計（3/6。案）

形の設計の根拠は、本リポの規約・文書・コードと SPIR-V の公開の仕様（Khronos）に限る。
行は F1〜。Q の回答に依る行は「未承認の仮置き」で、Q1〜Q6 はいずれも本リポでは回答を確認できていない。

| # | 点 | 形（案） | 根拠・備考 |
| - | -- | -------- | ---------- |
| F1 | 置き場 | `crates/contract/src/backend_vulkan/` 配下に新しいファイル 1 つ（名前は仮）。`backend_vulkan.rs` に `#[cfg(feature = "test-support")] pub mod <名前>;` の 1 行。外側のゲート（`backend-vulkan` かつ wasm32 でない）は `lib.rs` の既存のものを使う | 未承認の仮置き（Q2 の推奨 (b)）。`crates/contract/src/lib.rs` |
| F2 | `unsafe` を使わない守り | 新しいファイルの先頭に `#![forbid(unsafe_code)]` を置く案（親の `allow` の内側で `forbid` を重ねる）。4/6 でコンパイルが通ることを確かめ、通らなければ個数テストだけで守る | D2、[unsafe-policy](unsafe-policy.md) |
| F3 | 公開面 | 入口の関数 1 つ・結果の型・エラーの型をそのモジュールの `pub` に置く。`lib.rs` から再輸出しない。契約の入口・能力の問合せ・共通のエラーの型は足さない・変えない | D3、[parity-tool](parity-tool.md)（`test-support` の裏の `pub mod` の先例） |
| F4 | 入力 | メモリ上のモジュールを借用で受ける。語の列（`&[u32]`）かバイトの列（`&[u8]`）かは Q5 | 未承認の仮置き（Q1 の推奨 (a)、Q5） |
| F5 | 読み方の前提 | 先頭の語を検査する（マジック・ヘッダーの語数）。命令の先頭の語から語数と命令の番号を読んで順に進む。語数 0・残りを超える語数はエラー。知らない命令は語数だけ読んで飛ばす。逆のバイト順のマジックはエラー（Q5 が (a) の場合） | SPIR-V の公開の仕様（バイナリ形式の節）。4/6 で節番号を引く |
| F6 | 拾う対象 | モジュール直下の変数の宣言と、その装飾（set・binding など）・指す型の連鎖をたどる。含める資源の種別・量は**保留**（[vulkan-extraction-tool](vulkan-extraction-tool.md) の範囲の記述の確かめにある spec 側の判断待ちに依る）。型は種別・量を後から足せる形にする。エントリポイントで絞るかは Q6 | D5 |
| F7 | 出力の型 | 資源の変数 1 つにつきレコード 1 つの列。並びは決定的（set・binding の昇順、同値は出現順）。レコードは `pub` のフィールドと `Debug`・`Clone`・`PartialEq`・`Eq` を持ち、別 crate のテストから期待値を組んで `==` で比べられる。型の語彙は道具の中の小さな列挙で持ち、`core` に型を足さない | 未承認の仮置き（Q3 の推奨 (a)）。TASK-470（ポインタのみ） |
| F8 | 決まらない・欠けている値 | 0・既定値・`None` で埋めない。理由の種別を持つ専用の列挙の variant で区別する。扱いの決定（拒否するか等）は本タスクで行わない | D5 |
| F9 | エラーの型 | 道具専用の列挙（`#[non_exhaustive]`）。`Display`（英語）と `std::error::Error` を実装する。位置は語の番号で持ち、入力の中身を表示に出さない。`core` の `parity` のエラーを再利用しない。panic させない | D4、[security](../.claude/rules/security.md)、[japanese-style](../.claude/rules/japanese-style.md) |
| F10 | 上限と確保 | ヘッダーの値に比例した確保をしない（id の表は入力の長さで上が決まる map で持つ）。型の連鎖は訪問済みの記録か深さの上限で止める。添字・長さは `checked_*` と `get` で扱う | D4、[security](../.claude/rules/security.md) |
| F11 | 走る場所 | `make verify`（`--all-features`）と、`scripts/build-profiles.sh` の native の組のうち `backend-vulkan,test-support`。`backend-vulkan` だけの組・wasm32 の組ではコンパイル対象外。make ターゲット・CI ジョブ・feature の組を足さない | D6、`scripts/build-profiles.sh`、`scripts/verify.sh` |
| F12 | `unsafe` の個数テスト | 4/6 で `crates/contract/tests/backend_vulkan_unsafe_count.rs` の表に新しいファイルの行（個数 0）を足す。足さないと `not listed in the expected table` で失敗する。個数 0 の行の追加はレビューで足りる変更 | [unsafe-policy](unsafe-policy.md)、`crates/contract/tests/common/mod.rs` |
| F13 | テストの置き場と入力 | `crates/contract/tests/` に新しいテストファイル 1 つ。`Cargo.toml` に `[[test]]` と `required-features = ["backend-vulkan", "test-support"]`。入力はテストコードの中で語の列を組み立て、補助はテスト側に置く。バイナリ・外部の道具・GPU の実機・`docs/spec` に依らない | 未承認の仮置き（Q4 の推奨 (a)）。`test_support_pixel` の `[[test]]` の先例（`crates/contract/Cargo.toml`） |
| F14 | 文書 | 4/6 以降は、コードと公開テストから観測できる事実を本書の「現状の形」節に書く。`docs/contract/` に領域の文書を作らない | D3・D7 |

## オーナー判断待ち

下の推奨は案であり、確定ではなく、承認を得たものでもない。3/6 以降が推奨を採る場合は「未承認の仮置き」と明記する
（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）。

| # | 論点 | 選択肢 | 推奨 | 止まるもの |
| - | ---- | ------ | ---- | ---------- |
| Q1 | 道具の形 | (a) メモリ上の SPIR-V のモジュールを読む、標準ライブラリだけの関数 (b) 外部の逆アセンブラの出力を読む道具（手元だけで走らせる） | (a) | #3532〜#3534 |
| Q2 | 置き場と feature | (a) `crates/contract/src/backend_vulkan/` 配下で `backend-vulkan` だけでゲート (b) 同じ置き場で `backend-vulkan` かつ `test-support` でゲート (c) `crates/core` の `test-support` の裏（`parity` と並べる） (d) 製品 workspace の外 (e) 同じ置き場で `backend-vulkan` かつ既存の `artifact-inspection` でゲート | (b) | #3532・#3533 |
| Q3 | 宣言との照合の示し方（宣言の型が本リポにまだ無い） | (a) 道具は取り出した結果の型だけを持ち、照合は道具のテストの中の期待値との比べで示す (b) 宣言の型が入るのを待つ (c) 道具の側に宣言を表す仮の型を足す | (a) | #3532・#3534・#3535 |
| Q4 | テストの入力の作り方 | (a) テストコードの中で組み立てた最小のモジュール（SPIR-V の公開の仕様に沿って手で書いた語の列） (b) 本リポで新しく書いたシェーダーのソースとコンパイル済みの成果物をコミットする (c) CI でコンパイルする | (a) | #3534・#3535 |
| Q5 | 入力の型（形の設計で追加） | (a) 語の列（`&[u32]`） (b) バイトの列（`&[u8]`。長さが 4 の倍数でない入力のエラーとバイト順の扱いが入る） | (a)。(b) は後から薄い入口として足せる | #3533・#3534 |
| Q6 | エントリポイントでの絞り込み（形の設計で追加） | (a) 絞らず、モジュール直下の対象の変数をすべて返す (b) エントリポイントの名前を受けて絞る | (a)。(b) は後から足せる | #3533・#3534 |

各案の影響（本リポの公開済みの事実とポインタだけで書く）。

- Q1 (a): CI で走る公開テストを持てる。SPIR-V の読み取りを自前で持つ（保守の範囲が増える）
- Q1 (b): 外部の道具の導入と版の固定が要り、いまの CI では走らない。外部コマンドの起動が入り、パス・引数の連結の面ができる
- Q2 (a): Vulkan の背後実装を有効にした本番のビルドの公開面に入る。関連する決定（TASK-418 のポインタ）より先に公開面へ出る
- Q2 (b): 本番のビルドに入らない。走るのは `make verify` と、build-profiles の両方の feature を含む組。`unsafe` の個数テストの表に個数 0 の行を足す。取り込みの段（TASK-513 のポインタ）で cfg を直す
- Q2 (c): 背後実装の feature に依らず `test-support` の組で走る。`core` に特定の背後実装の成果物の読み取りを置くことになり、`core` の責務との整合の確認が要る
- Q2 (d): member の増減・別 workspace は TASK-1・TASK-6 の決定の記録に関わる。`make` の対象外になり CI で走らない
- Q2 (e): 本番のビルドに入らない。ただし `artifact-inspection` はいま `backend-metal` を含意するので、既存の feature の定義の変更と build-profiles の組の追加が要る（BUILD-65 のポインタ）。`unsafe` の個数テストの表の行は (b) と同じ
- Q3 (b): TASK-98 の前提には無い待ちが生じるので、spec 側の課題としての報告が要る（contract-shaping-escalation の差し戻しの経路）
- Q3 (c): 後から入る宣言の型（TASK-470 のポインタ）と二重になる
- Q4 (b): バイナリと外部のコンパイラの版の記録を本リポに持つ（出どころの記録が要る）。(c): CI に道具の導入（版固定・SHA256 の検証）が要る
- Q5 (b): 長さが 4 の倍数でない入力のエラーと、マジックの読み方によるバイト順の判定が公開面に入る
- Q6 (b): 入力にエントリポイントの名前が増え、見つからない場合のエラーが増える
- Q4 共通: どの入力で確かめれば完了の条件を満たすかの判断は、6/6（#3535）でも回答が無く判断待ちのまま（「完了の条件の確かめ（6/6）」節の 項目 17）。(a) で足りないと分かったら、spec 側の判断待ちとして報告する

回答の形: 「Q1〜Q6 とも推奨で」または「Qn は (x)」。

5/6（#3534）は仮置きの形をテストで固定したが、形が決まったとは扱わない。

## 別案に決まったときに変わる箇所

| 論点 | 別案 | 変わる箇所 |
| ---- | ---- | ---------- |
| Q1 | (b) 外部の逆アセンブラ | F4・F5 が出力の文字列の解釈に変わる。F13 の入力と F11 の走る場所（CI では走らない）が変わる |
| Q2 | (a) `backend-vulkan` のみ | F1 の cfg、F11 の組、F13 の `required-features` から `test-support` を外す |
| Q2 | (c) `core` の `test-support` の裏 | F1 の置き場、F12 の行が不要になる、F13 の `[[test]]` の対象 crate |
| Q2 | (d) 製品 workspace の外 | F1・F11・F12・F13 の全体。`make` の対象外になる |
| Q2 | (e) `artifact-inspection` | F1 の cfg、F11 の build-profiles の組の追加、F13 の `required-features` |
| Q3 | (b)・(c) | F7 の型が、宣言の型の到着待ち、または仮の型の追加に変わる |
| Q4 | (b)・(c) | F13 の入力の作り方と、出どころの記録・CI の道具の導入が変わる |
| Q5 | (b) バイト列 | F4・F5（長さ・バイト順の検査）、F9 のエラーの種別 |
| Q6 | (b) 絞り込みあり | F6・F7・F9 |

## 現状の形（4/6。未承認の仮置き）

本節はコードと本リポのテストから観測できる事実だけを書く。Q1〜Q4 は本リポでは回答を確認できていないので、入力の型・置き場・対象の範囲・エラーの形は推奨案を採った未承認の仮置きで、判断が出るまでマージしない。識別子はすべて仮。

- 置き場: `crates/contract/src/backend_vulkan/spirv_operands.rs`。`backend_vulkan.rs` の `#[cfg(feature = "test-support")] pub mod spirv_operands;` で公開する。有効になる条件は `backend-vulkan` かつ `test-support` で wasm32 でない。`test-support` なしのビルドには入らない。
- 入口: `extract_storage_buffer_operands(&[u32]) -> Result<Vec<ExtractedOperand>, SpirvExtractError>`。語の列を借用で受け、ファイル IO・外部コマンドの起動はしない。並びは（set, binding）の昇順。
- 取り出す内容: set・binding・要素の型（`ScalarKind`）・要素 1 個分のバイト数・`ArrayStride` の値・全体のバイト長。固定長の配列は `ByteLength::Fixed`、runtime array は `ByteLength::Unsized`（0 や推測で埋めない）。
- 対象: storage class が `StorageBuffer` の変数で、`Block` 装飾の struct がメンバー 1 つ（配列）だけを持つ形。モジュール全体の変数が対象で、エントリポイントでの絞り込みと push constant は含めない。
- 対象外の形は `SpirvExtractError::Unsupported` で返す（bool・vector・matrix・struct の要素、block の配列、`Uniform` + `BufferBlock`、メンバーの並びが違う block、8 の倍数でないビット幅、定数でない配列の長さ、`ArrayStride` が 0）。
- 入力の拒否: ヘッダーに満たない・マジック違い・逆のバイト順のマジック・語数 0 の命令・残りを超える命令・オペランド不足・未定義の id・装飾の欠落・定義の重複・（set, binding）の重複・大きさの溢れ。panic しない。
- `unsafe`: 0。`#![forbid(unsafe_code)]` を置き、`crates/contract/tests/backend_vulkan_unsafe_count.rs` の期待値の表に個数 0 の行を足した。
- 走るテスト: 同じファイルの単体テストと、`crates/contract/tests/backend_vulkan_spirv_operands.rs` の統合テスト（語の列をテストコードの中で組み立てる。バイナリ・外部の道具に依らない）。統合テストは `required-features = ["backend-vulkan", "test-support"]` で、`make verify`（全 feature）と `make build-profiles` の `backend-vulkan,test-support` の組・全 feature の組で走る。ほかの組では対象外になる。

## 公開テスト（5/6。未承認の仮置き）

別 crate（`tests/` 配下）から公開の入口だけで確かめる。入力はテストコードの中で組み立てた語の列で、命令・装飾・storage class の番号は SPIR-V の公開の仕様のものをテスト側で独立に持つ。仮置きの形をテストで固定したが、形が決まったとは扱わない。

| 観測できる振る舞い | 確かめ方（`crates/contract/tests/backend_vulkan_spirv_operands.rs`） |
| ------------------ | ------------------------------------------------------------------- |
| 別 crate から入口に届き、ヘッダーだけのモジュールは空の列を返す | `tool_is_reachable_from_another_crate` |
| 要素の型・固定長と runtime・stride の有無を混ぜた宣言が、外で組んだ期待値と `==` で一致する | `extracted_operands_equal_expected_declarations` |
| 並びが（set, binding）の昇順になる | `order_is_by_set_then_binding` |
| 道具が読まない命令や対象でない変数が混ざっても、storage buffer だけが返る | `compiler_like_module_yields_only_storage_buffers` |
| 型を共有する変数もそれぞれ返る | `variables_sharing_types_are_each_extracted` |
| 浮動小数点・整数の幅と符号が `ScalarKind` と要素のバイト数に写る | `scalar_kinds_and_element_bytes` |
| runtime array は `Unsized` で、0 にならない | `runtime_array_is_unsized_never_zero` |
| 固定長は長さ × stride。最大の長さと stride でも溢れず `Fixed` で返る | `fixed_length_is_count_times_stride`、`largest_length_and_stride_do_not_overflow` |
| 長さの符号付き・符号なしの境界と 0 | `signed_length_boundaries` |
| 長さが素の定数でない形は `LengthNotConstant`（長さの id が未定義でもこの形） | `length_that_is_not_a_plain_constant_is_rejected` |
| stride の 0・要素より小さい・無いときの扱い | `stride_boundaries` |
| 5 語に満たない入力は長さの検査が先に効く。マジック違いと逆のバイト順 | `inputs_shorter_than_header`、`magic_is_checked_after_length` |
| 全部の接頭辞が panic せず分類される（命令の途中は位置・語数・残りが付く） | `every_prefix_is_classified_without_panic` |
| 語数 0・残りを超える語数 | `zero_word_count_reports_its_offset`、`word_count_beyond_the_module` |
| 道具が読む命令のオペランド不足が位置と opcode の付いたエラーになる。値の要らない装飾と知らない装飾は受理される | `too_few_operands_for_each_instruction_the_tool_reads` |
| 各位置の語を差し替えた入力で panic せず、成功なら（set, binding）が狭義の昇順、失敗なら表示が空でない ASCII | `single_word_corruption_never_panics` |
| 未定義の id・pointer でない型・set か binding の欠落 | `undefined_ids_are_reported`、`variable_type_that_is_not_a_pointer`、`missing_set_or_binding` |
| 装飾・定義の重複、（set, binding）の重複、storage class の食い違い（両方向） | `duplicate_decorations_are_rejected`、`duplicate_binding_reports_set_and_binding`、`storage_class_mismatch_in_both_directions` |
| `UnsupportedShape` の各 variant に外から届く（`non_exhaustive` のため網羅は機械で確かめられず、表は手で保つ） | `unsupported_element_shapes_are_reported`、`unsupported_block_shapes_are_reported`、`unsupported_other_types_are_reported`、`unsupported_legacy_buffer_block_is_reported`、`unsupported_length_and_stride_shapes_are_reported`、`decoration_groups_are_rejected_with_the_group_id` |
| 対象でない storage class の変数（push constant を含む）と `Uniform` + `Block` はエラーにせず外れる | `non_storage_buffer_variables_are_skipped` |
| エラーが `std::error::Error` で、表示が空でない ASCII の 1 行 | `errors_are_std_errors_with_ascii_display` |

テストで固定した現状の事実（判断ではない）。

- 長さと stride の積は `u32` どうしで `u64` に収まるため、あふれのエラーは持たない。最大の積が `Fixed` で返ることは上の表で固定している
- `BadMagic` は入力の語の値を持たず、表示にも出さない
- ビット幅は 8 の倍数なら上限なく受理する

## 後続の分担

| 段 | Issue | 引き継ぐこと |
| -- | ----- | ------------ |
| 3/6 形の設計 | #3532 | 本書の「形の設計」節（F1〜F14。Q1〜Q6 は未回答のため未承認の仮置きを含む） |
| 4/6 実装 | #3533 | F1〜F10・F12 の実装。入力の検証を security-auditor がレビューする。回答が仮置きと違えば下の節に従って直す |
| 5/6 テスト | #3534 | F13。D4・D5 の境界（切れた入力・不正な語数・決まらない量）を公開テストで固定する。結果: `crates/contract/tests/backend_vulkan_spirv_operands.rs`（済） |
| 6/6 確かめ | #3535 | 本リポで観測できる事実による確かめの記録。完了の条件との対応づけの正は private spec 側。結果: 下の「完了の条件の確かめ（6/6）」節（済） |

## 完了の条件の確かめ（6/6）

本リポで観測できる事実だけを書く。コマンドは worktree のルートで実行した。条件の本文は転記しない（`docs/spec/05-tasks.md` の TASK-98 を参照。ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 前段（1/6〜5/6）の状態 | Issue と PR の状態の読み取り | #3530〜#3534 は CLOSED。PR #7036・#7037・#7041・#7043・#7050 は MERGED |
| 2 | 範囲のポインタ（TASK-98） | 本書では spec の内容を確かめない | ポインタのみ。内容の照合は spec 側 |
| 3 | TASK-98 の進み具合・決定の記録 | 本リポからは確かめない | spec 側の判断待ち（ポインタ: TASK-98） |
| 4 | Q1〜Q6 への回答の記録 | #387・#3530〜#3535 のコメント | 無い（コメントは全て 0 件）。形は未承認の仮置きのまま |
| 5 | 別 crate から公開の入口に届く | `tool_is_reachable_from_another_crate` | 通過 |
| 6 | 取り出した宣言が、外で組んだ期待値と `==` で一致する | `extracted_operands_equal_expected_declarations`・`order_is_by_set_then_binding`・`scalar_kinds_and_element_bytes` | 通過 |
| 7 | 決まらない量・対象外の変数が、0 や推測で埋まらず区別して返る | `runtime_array_is_unsized_never_zero`・`non_storage_buffer_variables_are_skipped`・`unsupported_element_shapes_are_reported` ほか | 通過 |
| 8 | 不正な入力で panic しない | `every_prefix_is_classified_without_panic`・`single_word_corruption_never_panics` | 通過 |
| 9 | `unsafe` を使っていない | `spirv_operands.rs` の `#![forbid(unsafe_code)]`、`backend_vulkan_unsafe_count.rs` の表の行（個数 0） | 成り立つ |
| 10 | 外部依存 0 のまま | `Cargo.lock` の `[[package]]` の数、`make deps-check` | 4 件（workspace の member のみ）。`make deps-check` は終了コード 0 |
| 11 | ファイル IO・外部コマンドの起動が無い | `spirv_operands.rs` を `std::fs`・`std::process`・`std::env` で grep、`use` の行 | 該当 0 件。`use` は `std::collections`・`std::fmt` のみ |
| 12 | 製品の経路から参照していない | `crates` を `spirv_operands` で grep | `backend_vulkan.rs` の `test-support` ゲート付きの `pub mod` と、テスト・`Cargo.toml` のみ。`core`・`exec`・`upper` からの参照は無い |
| 13 | `test-support` が `default` に入っておらず、`test-support` なしのビルドに入らない | `crates/contract/Cargo.toml` の `[features]`、`make build-profiles` | `default` は `contract-core`・`cpu-isa`・`cpu-ops`。`make build-profiles` は終了コード 0 |
| 14 | 道具のテストが通る | `cargo test --locked -p fandhe-silicon-contract --features backend-vulkan,test-support` | lib の単体テスト 19 件・統合テスト `backend_vulkan_spirv_operands` 33 件・個数テスト 1 件が通過。失敗 0 |
| 15 | GPU の実機・外部の CLI なしで走る | 上のテストを実機なしで実行、`ci.yml` を `spirv` で grep | ローカルでは通過。`ci.yml` に SPIR-V の道具の導入は 0 件。CI での結果は未検証（PR 作成前） |
| 16 | テストの入力の出どころ | `crates/contract/tests/backend_vulkan_spirv_operands.rs`、リポ内の `.spv`・シェーダーのソースの拡張子のファイル | 入力はテストコードで組み立てた語の列だけ。該当する拡張子のファイルは 0 件 |
| 17 | 手で組み立てた入力で足りるか（Q4） | 本リポからは決めない | **判断待ち**。回答が無く、足りる・足りないのどちらにも結論しない |
| 18 | 足していないもの | `ls docs/contract`、変更の範囲 | feature・make ターゲット・CI ジョブ・`docs/contract/` の領域の文書・契約の入口・能力の問合せは足していない（関連: TASK-418・TASK-448・TASK-513 のポインタ） |
| 19 | 関連タスクの状態 | Issue の状態の読み取り | #387・#388・#612・#633・#669・#91 は OPEN |
| 20 | 品質ゲート | `make check`・`make verify`・`make build-profiles`・`make deny`・`make deps-check` | いずれも終了コード 0 |
| 21 | spec の完了の条件との照合 | 記録の正は private spec 側 | 本書では行っていない |

- 判定: 本リポで確かめられる項目に問題は無い。ただし **TASK-98 の完了は本リポからは確かめられず、宣言しない**。Q1〜Q6 が未回答で、spec 側の判断待ちがあるため。回答に依る部分は保留とする
- Q1〜Q6 は判断待ちで、置き場・feature・入力の型・対象の範囲・エラーの形は未承認の仮置きのまま
- 再実行の契機: Q1〜Q6 への回答、範囲の記述についての spec 側の判断、submodule の参照先の更新（`update-external.yml` の PR）、道具の置き場・feature の変更、`scripts/build-profiles.sh` の組の変更、取り込みの段（TASK-513 のポインタ）
- 引き継ぎ（親 #387 へ）: 残るのは、Q1〜Q6 への回答、spec 側の判断、本表の再実行、親のクローズの判断の順。いずれも本リポの自動運転の対象外
- 持ち越し（5/6 から）: `SizeOverflow` の扱いと `BadMagic` の表示の扱いは、後続の修正で決めた（上の「テストで固定した現状の事実」の通り）

## 確かめた日付

2026-10-12（base: `d8e06a1`）。5/6 は 2026-10-12（base: `23c4e0b`）。6/6 は 2026-10-12（base: `678bd79`）。

## 関連

- [unsafe-policy](unsafe-policy.md)・[workspace-boundary](workspace-boundary.md)・[parity-tool](parity-tool.md)（`test-support` の前例）
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[security](../.claude/rules/security.md)
