# 結果の一致を確かめる共通の道具（TASK-24）の現状・前提・形（案）

TASK-24（親 Issue #100）について、着手時点の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・閾値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

「形の設計（案）」以降は案であり、オーナーの確認を経るまで確定扱いにしない。
値・比べ方の本文は書かない（コードと公開テストと同じ PR で入る）。

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | PR #3 で骨格を追加済み。`cargo metadata --format-version 1 --all-features --locked` で member 4・パッケージ 4・外部依存 0 件。依存の向きは contract → core、exec → core、upper → core・exec。`make verify`・`make deny` が通る |

## 着手時点の骨格の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | member 4（core / contract / exec / upper）。外部依存 0 | `cargo metadata` の集計 |
| 公開面 | 4 crate とも crate 冒頭の doc コメントのみ（`contract` は cfg ゲート付きの空の backend モジュール宣言 `backend_metal`・`backend_vulkan`・`backend_cuda` を持つ）。`core`・`exec`・`upper` は `#![forbid(unsafe_code)]` | `crates/*/src/lib.rs` |
| contract の feature | `cpu-ops`・`test-support` などは宣言のみで中身は無い | `crates/contract/Cargo.toml` |
| 既存のテスト | `crates/contract/tests/backend_cfg_gate.rs` のみ | `find crates -name '*.rs'` |
| 結果を比べる道具 | コード・テスト・文書とも 0 件 | リポ内検索 |

- `make verify` は `--all-features` で clippy と test を流すため、feature の裏に置くコードも検査の対象になる

## 関連タスクの状態と引き継ぎ

事実のみを書く。向きは決めない。

| 関連 | Issue | 状態 | 引き継ぎ |
| ---- | ----- | ---- | -------- |
| TASK-23 | #91 | 未完了（並行） | 本リポに共通の試験の枠やラベルの出力はまだ無い。あるものとして扱わない |
| TASK-25 | #109 | 未完了 | TASK-24 を前提に引く下流。TASK-24 が終わるまで着手できない |
| TASK-53 | #196 | 未完了 | 適用の範囲の判断はこのタスクに委ね、TASK-24 の側で広げも狭めもしない |
| TASK-6 | #35 | 済（クローズ） | 決定の記録は spec リポの main にある（本リポの submodule の固定 SHA はこの記録より前）。持ち込みは [poc-carry-over](../.claude/rules/poc-carry-over.md) と [workspace-boundary](workspace-boundary.md) に従う |

- 依存: 外部依存は 0。追加は未承認で、必要なら [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに乗せる
- `cpu-ops` に関わる下流のポインタ: TASK-1350・TASK-1378

## 形の設計（案）

既存の決定から一意に導ける点だけを、根拠のポインタ付きで記録する。導けない点は次節に送る。

| # | 点 | 形（案） | 根拠 |
| - | -- | -------- | ---- |
| D1 | PoC からの持ち込みか書き直しか | PoC のファイルは複製せず、本リポの規約で新しく書く。記録・入力・生成物を写さない | TASK-6 の決定の記録、[poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |
| D2 | 依存・unsafe | 標準ライブラリだけで書く。外部依存は 0（承認フローに乗る依存は無い）。`unsafe` を使わない | [dependency-policy](../.claude/rules/dependency-policy.md)、[coding-rust](../.claude/rules/coding-rust.md) |
| D3 | 背後実装への非依存 | 入力はホスト側に読み戻した値のスライスだけ。背後実装の型・ハンドル・device を引数に取らない。`backend-*` feature や target の `cfg` で分けない（wasm32 でもビルドできる書き方） | TASK-24 の完了の条件、`scripts/build-profiles.sh` |
| D4 | 閾値の設定 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | TASK-24 の完了の条件、`docs/spec/02-poc-plan.md` の T-1・T-2（ポインタのみ） |
| D5 | 適用の範囲 | 道具は比べるだけで、どの範囲に当てるか・承認の有無を表す API を持たない | TASK-53（#196） |
| D6 | TASK-23 の枠との接続 | 道具は背後実装に依らない素の結果の型を返す。試験のラベルの語彙は TASK-23 の持ち物で、道具はその名前を定義も公開もしない。素の結果からラベルへの対応づけは TASK-23 の側が行う。依存の向きは「枠・試験 → 道具」の一方向 | TASK-23（#91） |
| D7 | T-1 の要素の分類 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | TASK-25（#109）、PARITY-2〜PARITY-5（ポインタのみ） |
| D8 | T-1 の比べの形 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | PARITY-1（ポインタのみ） |
| D9 | T-2 の比べの形 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | PARITY-6、`docs/spec/02-poc-plan.md` の T-2（ポインタのみ） |
| D10 | CPU 参照の形 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | TASK-1350・TASK-1378・CPU-27（ポインタのみ） |
| D11 | 結果とエラー | 結果の型は素の結果（全体が一致したか、不一致の件数、判定しなかった要素の件数、最初の不一致の位置、誤差の最大値）を持つ。これは試験のラベルではない（D6）。入力の不備は `Result` の `Err`（panic しない。メッセージは英語）。要素数の計算は `checked_*` | [coding-rust](../.claude/rules/coding-rust.md)、[security](../.claude/rules/security.md)、[japanese-style](../.claude/rules/japanese-style.md) |
| D12 | 走る場所 | `make verify` の `cargo test`。GPU の実機は要らず GitHub ホステッドの CI で走る。新しい make ターゲット・CI ジョブは足さない | [unsafe-policy](unsafe-policy.md) と同じ考え方 |

モジュールの構成（名前は仮）は、閾値の設定 / 数値の一致の比べ / 順位付きの結果の比べ / CPU 参照 の 4 つに分ける。
置き場が決まればそのまま移せるよう、置き場に依らない純粋関数にする。

## オーナー判断待ち

下表の推奨は案であり、確定ではなく、オーナーの承認を得たものでもない。
回答の形は [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) に従い、「推奨で」と答えられる形にする。

| # | 論点 | 選択肢 | 推奨（未承認） | 止まるもの |
| - | ---- | ------ | -------------- | ---------- |
| Q1 | 比べ方と閾値の設定の置き場 | (a) `crates/core` のモジュール。テスト専用の feature の裏 (b) `crates/contract` のモジュール。既存の `test-support` の裏 (c) 検証専用の新しい member crate | (a) | #103〜#105 の置き場、#107 |
| Q2 | CPU 参照の置き場 | (a) Q1 と同じ場所（検証専用） (b) `crates/contract` の `cpu-ops` の裏 | (a) | #103、TASK-1350 |
| Q3 | Q1 が (a)・(b) の場合の feature | (a) 既存の `test-support` と同名にする（core に置く場合は contract から転送） (b) 道具専用の feature を新しく作る | (a) | #103 の `Cargo.toml` の変更 |
| Q4 | CPU 参照が持つ演算の範囲 | (a) T-1・T-2 の比べに要る最小の参照だけ (b) 要素ごとの演算の参照も含める | (a)。増やす分は TASK-1350 で決める | #103 の範囲 |

各案の影響（本リポの公開済みの事実とポインタだけ）:

- Q1 (a): 既存の依存の向きのまま 4 crate すべてのテストから届く。`core` に feature と公開アイテムが増える（BUILD-54・BUILD-15 のポインタ）。測定のビルドとの関係は TASK-15 のポインタ。feature の裏に置くので、閾値を外から確かめる要件（D4）は、その feature を有効にしたビルドでだけ満たせる
- Q1 (b): `exec` は `contract` に依存しないので、今の依存の向きでは `exec` のテストから届かない。`test-support` を本番経路で必須にしない規約（`AGENTS.md` 2 節、BUILD-65 のポインタ）には沿う。D4 は feature を有効にしたビルドでだけ満たせる
- Q1 (c): 本番の feature と公開面に触れない。member の増減は TASK-1・TASK-6 の決定の記録の変更に当たり（[workspace-boundary](workspace-boundary.md)）、依存の向きの規約（`AGENTS.md` 2 節、TASK-15 のポインタ）の改定も要る。D4 は feature の有無に左右されず満たせる
- Q2 (b): 本番の既定 feature の公開面に検証用の参照が入り、TASK-1350 が決める範囲に踏み込む（CPU-25・CPU-27 のポインタ）

## 後続の分担

| 作業 | 担当 |
| ---- | ---- |
| 形の設計（本節群） | #102（2/8） |
| 置き場の骨格（Q1〜Q3 の回答に従う）、閾値の設定の型、T-1 の比べに要る CPU 参照 | #103（3/8） |
| 数値の一致（T-1）の比べ方と、要素の分類（D7） | #104（4/8） |
| 順位付きの結果の一致（T-2）の比べ方と、その CPU 参照（D10） | #105（5/8） |
| 閾値を変えないこと・適用の範囲を TASK-53 に委ねることの整理、設定を外から確かめる方法の整理 | #106（6/8） |
| テスト（背後実装に依らず動くこと、設定が外から読めること、入力の不備が `Err` になること）。Q1 が `core` の feature の裏に決まった場合は、wasm32 のビルドが `make build-profiles` の対象に入っているかを確かめる（`build-profiles.sh` は今 contract だけを回す） | #107（7/8） |
| 完了の条件の確かめ | #108（8/8） |

## 完了の条件の確かめ

1/8 では未実施。8/8（#108）で、本リポで観測できる事実により埋める。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。

## 確かめた日付

2026-10-11（base: 6d105dc）
