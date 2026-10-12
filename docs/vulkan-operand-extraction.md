# Vulkan の成果物からオペランドの宣言を取り出す道具（TASK-98）の現状・前提・方針（案）

TASK-98（親 Issue #387）について、現状と前提（1/6、#3530）と方針（2/6、#3531）を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・PoC の方式と結果は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-98 と `docs/spec/03-poc/kernel-artifacts-and-parity/` を参照する（ポインタのみ）。
道具の主題は「Vulkan の成果物（SPIR-V）からオペランドの宣言を取り出す道具」まで。
方針は案であり、オーナーの確認を経るまで確定扱いにしない。

## 現状と前提

1/6（#3530）で記録する。

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

## オーナー判断待ち

下の推奨は案であり、確定ではなく、承認を得たものでもない。3/6 以降が推奨を採る場合は「未承認の仮置き」と明記する
（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）。

| # | 論点 | 選択肢 | 推奨 | 止まるもの |
| - | ---- | ------ | ---- | ---------- |
| Q1 | 道具の形 | (a) メモリ上の SPIR-V のモジュールを読む、標準ライブラリだけの関数 (b) 外部の逆アセンブラの出力を読む道具（手元だけで走らせる） | (a) | #3532〜#3534 |
| Q2 | 置き場と feature | (a) `crates/contract/src/backend_vulkan/` 配下で `backend-vulkan` だけでゲート (b) 同じ置き場で `backend-vulkan` かつ `test-support` でゲート (c) `crates/core` の `test-support` の裏（`parity` と並べる） (d) 製品 workspace の外 (e) 同じ置き場で `backend-vulkan` かつ既存の `artifact-inspection` でゲート | (b) | #3532・#3533 |
| Q3 | 宣言との照合の示し方（宣言の型が本リポにまだ無い） | (a) 道具は取り出した結果の型だけを持ち、照合は道具のテストの中の期待値との比べで示す (b) 宣言の型が入るのを待つ (c) 道具の側に宣言を表す仮の型を足す | (a) | #3532・#3534・#3535 |
| Q4 | テストの入力の作り方 | (a) テストコードの中で組み立てた最小のモジュール（SPIR-V の公開の仕様に沿って手で書いた語の列） (b) 本リポで新しく書いたシェーダーのソースとコンパイル済みの成果物をコミットする (c) CI でコンパイルする | (a) | #3534・#3535 |

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
- Q4 共通: どの入力で確かめれば完了の条件を満たすかは、6/6（#3535）で TASK-98 と照らす（ポインタのみ）。(a) で足りないと分かったら、spec 側の判断待ちとして報告する

回答の形: 「Q1〜Q4 とも推奨で」または「Qn は (x)」。

## 現状の形（4/6。未承認の仮置き）

本節はコードと本リポのテストから観測できる事実だけを書く。Q1〜Q4 は本リポでは回答を確認できていないので、入力の型・置き場・対象の範囲・エラーの形は推奨案を採った未承認の仮置きで、判断が出るまでマージしない。識別子はすべて仮。

- 置き場: `crates/contract/src/backend_vulkan/spirv_operands.rs`。`backend_vulkan.rs` の `#[cfg(feature = "test-support")] pub mod spirv_operands;` で公開する。有効になる条件は `backend-vulkan` かつ `test-support` で wasm32 でない。`test-support` なしのビルドには入らない。
- 入口: `extract_storage_buffer_operands(&[u32]) -> Result<Vec<ExtractedOperand>, SpirvExtractError>`。語の列を借用で受け、ファイル IO・外部コマンドの起動はしない。並びは（set, binding）の昇順。
- 取り出す内容: set・binding・要素の型（`ScalarKind`）・要素 1 個分のバイト数・`ArrayStride` の値・全体のバイト長。固定長の配列は `ByteLength::Fixed`、runtime array は `ByteLength::Unsized`（0 や推測で埋めない）。
- 対象: storage class が `StorageBuffer` の変数で、`Block` 装飾の struct がメンバー 1 つ（配列）だけを持つ形。モジュール全体の変数が対象で、エントリポイントでの絞り込みと push constant は含めない。
- 対象外の形は `SpirvExtractError::Unsupported` で返す（bool・vector・matrix・struct の要素、block の配列、`Uniform` + `BufferBlock`、メンバーの並びが違う block、8 の倍数でないビット幅、定数でない配列の長さ、`ArrayStride` が 0）。
- 入力の拒否: ヘッダーに満たない・マジック違い・逆のバイト順のマジック・語数 0 の命令・残りを超える命令・オペランド不足・未定義の id・装飾の欠落・定義の重複・（set, binding）の重複・大きさの溢れ。panic しない。
- `unsafe`: 0。`#![forbid(unsafe_code)]` を置き、`crates/contract/tests/backend_vulkan_unsafe_count.rs` の期待値の表に個数 0 の行を足した。
- 走るテスト: 同じファイルの単体テスト（語の列をテストコードの中で組み立てる。バイナリ・外部の道具に依らない）。`make verify` と `make build-profiles` の `backend-vulkan,test-support` の組で走る。境界の網羅と統合テストは 5/6（#3534）。

## 後続の分担

| 段 | Issue | 引き継ぐこと |
| -- | ----- | ------------ |
| 3/6 形の設計 | #3532 | D1〜D7 を前提に、Q1〜Q4 の回答（無ければ未承認の仮置き）で形を決める |
| 4/6 実装 | #3533 | Q2 が (a)・(b)・(e) なら `unsafe` の個数テストの表に行を足す。入力の検証を security-auditor がレビューする |
| 5/6 テスト | #3534 | D4・D5 の境界（切れた入力・不正な語数・決まらない量）を公開テストで固定する |
| 6/6 確かめ | #3535 | 本リポで観測できる事実による確かめの記録。完了の条件との対応づけの正は private spec 側 |

## 関連

- [unsafe-policy](unsafe-policy.md)・[workspace-boundary](workspace-boundary.md)・[parity-tool](parity-tool.md)（`test-support` の前例）
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[security](../.claude/rules/security.md)
