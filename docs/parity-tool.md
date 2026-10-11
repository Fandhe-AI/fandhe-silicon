# 結果の一致を確かめる共通の道具（TASK-24）の現状・前提・形（案）

TASK-24（親 Issue #100）について、着手時点の現状と前提、および 3/8（#103）で入れた置き場の骨格、4/8（#104）で入れた要素ごとの比べ方、5/8（#105）で入れた Top-k の参照と T-2 の比べ方を、本リポで公開済みの事実とポインタだけで記録する。
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

## 置き場の骨格（3/8。未承認の仮置き）

置き場・feature・範囲は、次節の Q1〜Q4 が未回答のため、推奨を未承認の仮置きとして採った。確定ではない。
本節はコードと公開テストから観測できる事実だけを書く。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity.rs` と `crates/core/src/parity/`。`parity` は feature `test-support` の裏にだけ公開される | `crates/core/src/lib.rs` |
| feature | `core` に `test-support` を新設。`contract` の `test-support` は `fandhe-silicon-core/test-support` へ転送する。どの crate の `default` にも入れない | 各 `Cargo.toml`、`cargo metadata` |
| CPU 参照 | `parity::cpu_reference::reference_gemm_f32`（f32 の行列積）と `ReferenceError`。振る舞いは rustdoc に記す | `crates/core/src/parity/cpu_reference.rs` |
| 公開テスト | `crates/core/tests/parity_cpu_reference.rs`、`crates/contract/tests/test_support_forwarding.rs`（いずれも `required-features = ["test-support"]`） | 各 `Cargo.toml` |
| 依存 | 外部依存は 0 のまま。`contract` から `core` への feature の転送だけを足した | `Cargo.lock` に差分なし |
| 続きの足し方 | 比べ方は `parity/` にファイルを足して `parity.rs` に `pub mod` を 1 行足す | `crates/core/src/parity.rs` |

- 別案に決まったときに変わる箇所: Q1 (b) は `parity` を `crates/contract/src/` へ移し、`core` の feature と転送を直す。Q1 (c) は同じファイルを新しい crate へ移す（member の増減は TASK-1・TASK-6 の記録の変更が要る）。Q2 (b) は `cpu_reference.rs` を `cpu-ops` の裏へ移す。Q3 (b) は feature 名の置換。Q4 (b) は関数の追加だけ
- 3/8 では、Top-k の参照・要素ごとの演算の参照・比べ方・閾値の設定を入れない。`exec`・`upper` からの利用口も足していない（#107 で判断する）

## 要素ごとの比べ方と閾値の設定（4/8。未承認の仮置き）

置き場・feature は 3/8 と同じ未承認の仮置きを続ける。コードと公開テストから観測できる事実だけを書く。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity/tolerance.rs`（`parity.rs` に `pub mod tolerance;` を 1 行足した） | `crates/core/src/parity.rs` |
| 公開アイテム | `Tolerance`・`T1`・`ElementVerdict`・`judge_element`・`Comparison`・`CompareError`・`compare_f32`。閾値は `T1` の getter で読め、任意の値を作る口は無い | rustdoc |
| 計算 | 閾値と誤差は `f64` で持ち、比較は厳密な `<`。絶対誤差か相対誤差（参照の絶対値に対する比）のどちらかが内なら `Within` | `tolerance.rs`、テスト |
| 有限でない値 | 参照か実測が有限でなければ `NonFinite`。不一致として数える | テスト |
| 列の集計 | 件数・最初に外れた添字を返す。1 要素でも外れれば `is_match()` は偽。長さ 0 は `EmptyInput`、長さ違いは `LengthMismatch` | テスト |
| 公開テスト | `crates/core/tests/parity_tolerance.rs`（`required-features = ["test-support"]`） | `crates/core/Cargo.toml` |
| 依存 | 外部依存は 0 のまま | `Cargo.lock` に差分なし |

## Top-k の参照と T-2 の比べ方（5/8。未承認の仮置き）

置き場・feature は 3/8 と同じ未承認の仮置きを続ける。コードと公開テストから観測できる事実だけを書く。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity/topk.rs`（`parity.rs` に `pub mod topk;` を 1 行足した） | `crates/core/src/parity.rs` |
| 公開アイテム | `TopKEntry`・`TopKReferenceError`・`reference_topk_f32`・`TopKComparison`・`compare_topk`。`TopKEntry` は `PartialEq` を持たない | rustdoc |
| 計算 | 有限でないスコアを外し、スコアの降順（`f32::total_cmp`）、同点はスロット番号の昇順で並べて先頭 `min(k, 有限の件数)` 件を返す。`k == 0`・空の列は空の `Ok` | `topk.rs`、テスト |
| 比べ | 位置ごとにスロット番号とスコアのビット列がともに等しいときだけ一致。許容誤差は無い。`-0.0` と `+0.0` は不一致 | テスト |
| 列の集計 | 件数・不一致の件数・最初の不一致の位置を返す。長さ 0 は `EmptyInput`、長さ違いは `LengthMismatch`（`tolerance` の `CompareError` を再利用） | テスト |
| 公開テスト | `crates/core/tests/parity_topk.rs`（`required-features = ["test-support"]`） | `crates/core/Cargo.toml` |
| 依存 | 外部依存は 0 のまま | `Cargo.lock` に差分なし |

- 別案に決まったときに変わる箇所: Q8 (b) は番兵の定数の追加と埋める処理、Q9 (b) はエラーの variant の追加、Q10 (b) は定数の追加。Q2 (b) のときは `reference_topk_f32` と `TopKReferenceError` を `cpu_reference.rs` と一緒に移す

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
| D3 | 背後実装への非依存 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | TASK-24（ポインタのみ） |
| D4 | 閾値の設定 | `crates/core/src/parity/tolerance.rs` の rustdoc とテストを参照（4/8 で実装） | TASK-24 の完了の条件、`docs/spec/02-poc-plan.md` の T-1・T-2（ポインタのみ） |
| D5 | 適用の範囲 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | TASK-53（#196）（ポインタのみ） |
| D6 | TASK-23 の枠との接続 | 本書では記述しない。実装と公開テストが入る PR でコードから記述する（spec 側の判断待ち） | TASK-23（#91）（ポインタのみ） |
| D7 | T-1 の要素の分類 | 4/8 では有限でない値を `NonFinite` として不一致に数えるだけ。それ以外の規則は TASK-25（#109）で扱う | TASK-25（#109）、PARITY-2〜PARITY-5（ポインタのみ） |
| D8 | T-1 の比べの形 | `crates/core/src/parity/tolerance.rs` の rustdoc とテストを参照（4/8 で実装） | PARITY-1（ポインタのみ） |
| D9 | T-2 の比べの形 | `crates/core/src/parity/topk.rs` の rustdoc とテストを参照（5/8 で実装） | PARITY-6、`docs/spec/02-poc-plan.md` の T-2（ポインタのみ） |
| D10 | CPU 参照の形 | `crates/core/src/parity/cpu_reference.rs` の rustdoc とテストを参照（3/8 で実装） | TASK-1350・TASK-1378・CPU-27（ポインタのみ） |
| D11 | 結果とエラー | `crates/core/src/parity/cpu_reference.rs` の rustdoc とテストを参照（3/8 で実装） | [coding-rust](../.claude/rules/coding-rust.md)、[security](../.claude/rules/security.md)、[japanese-style](../.claude/rules/japanese-style.md) |
| D12 | 走る場所 | `make verify` の `cargo test`。GPU の実機は要らず GitHub ホステッドの CI で走る。新しい make ターゲット・CI ジョブは足さない | [unsafe-policy](unsafe-policy.md) と同じ考え方 |

## オーナー判断待ち

下表の推奨は案であり、確定ではなく、オーナーの承認を得たものでもない。
回答の形は [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) に従い、「推奨で」と答えられる形にする。

| # | 論点 | 選択肢 | 推奨（未承認） | 止まるもの |
| - | ---- | ------ | -------------- | ---------- |
| Q1 | 比べ方と閾値の設定の置き場 | (a) `crates/core` のモジュール。テスト専用の feature の裏 (b) `crates/contract` のモジュール。既存の `test-support` の裏 (c) 検証専用の新しい member crate | (a) | #103〜#105 の置き場、#107 |
| Q2 | CPU 参照の置き場 | (a) Q1 と同じ場所（検証専用） (b) `crates/contract` の `cpu-ops` の裏 | (a) | #103、TASK-1350 |
| Q3 | Q1 が (a)・(b) の場合の feature | (a) 既存の `test-support` と同名にする（core に置く場合は contract から転送） (b) 道具専用の feature を新しく作る | (a) | #103 の `Cargo.toml` の変更 |
| Q4 | CPU 参照が持つ演算の範囲 | (a) 最小の参照だけ (b) 要素ごとの演算の参照も含める | (a)。増やす分は TASK-1350 で決める | #103 の範囲 |
| Q5 | 有限でない値を含む要素の数え方 | (a) 不一致に数え専用の区分で区別する (b) 比べの対象から外して件数だけ出す | (a)。規則の追加は TASK-25（#109） | #109 の足し方 |
| Q6 | 閾値の設定の形 | (a) 固定の定数だけを公開し任意の値を作る口は持たない (b) 検査付きのコンストラクタも公開する | (a) | #105・#107 の使い方 |
| Q7 | 長さ 0 の入力 | (a) エラー (b) 0 件を比べて一致 | (a) | #107 |
| Q8 | 有限のスコアが k 件に満たないときの参照の出力 | (a) `min(k, 有限の件数)` 件だけ返す（番兵の定数を持たない） (b) k 件に揃え、足りない位置を公開の番兵の定数で埋める | (a)。番兵の値を公開面に固定しない。埋められた出力と比べると長さ違いのエラーで止まる | #107 の使い方、TASK-1350 |
| Q9 | `k == 0`・スコア列が空のときの参照 | (a) 空の結果を `Ok` で返す（比べの側が長さ 0 をエラーにする） (b) 参照の側でエラーにする | (a) | #107 |
| Q10 | T-2 の設定の見せ方 | (a) 専用の定数を持たず、規則は関数の振る舞いと公開テストで確かめる (b) 規則を表す公開の定数を足す | (a)。足すのは後からでも互換 | #108 |

各案の影響（本リポの公開済みの事実とポインタだけ）:

- Q1 (a): 既存の依存の向きのまま 4 crate すべてのテストから届く。`core` に feature と公開アイテムが増える（BUILD-54・BUILD-15 のポインタ）。測定のビルドとの関係は TASK-15 のポインタ
- Q1 (b): `exec` は `contract` に依存しないので、今の依存の向きでは `exec` のテストから届かない。`test-support` を本番経路で必須にしない規約（`AGENTS.md` 2 節、BUILD-65 のポインタ）には沿う
- Q1 (c): 本番の feature と公開面に触れない。member の増減は TASK-1・TASK-6 の決定の記録の変更に当たり（[workspace-boundary](workspace-boundary.md)）、依存の向きの規約（`AGENTS.md` 2 節、TASK-15 のポインタ）の改定も要る
- Q2 (b): 本番の既定 feature の公開面に検証用の参照が入り、TASK-1350 が決める範囲に踏み込む（CPU-25・CPU-27 のポインタ）

Q1〜Q10 は 5/8 の時点でも未回答。#103〜#105 は推奨を未承認の仮置きで実装した。回答が出るまで、その PR は自動でマージしない。

## 後続の分担

| 作業 | 担当 |
| ---- | ---- |
| 形の設計（本節群） | #102（2/8） |
| 置き場の骨格と CPU 参照（Q1〜Q4 は未承認の仮置きで実装済み） | #103（3/8。済） |
| 許容誤差つきの要素ごとの比べ方と閾値の設定（未承認の仮置きで実装済み） | #104（4/8。済） |
| T-2 に関わる作業（Top-k の参照と比べ。未承認の仮置きで実装済み） | #105（5/8。済） |
| 整理の作業 | #106（6/8） |
| テスト。Q1 が `core` の feature の裏に決まった場合は、`make build-profiles` の対象に入っているかを確かめる（`build-profiles.sh` は今 contract だけを回す）。`exec`・`upper` からの利用口の要否も判断する | #107（7/8） |
| 完了の条件の確かめ | #108（8/8） |

## 完了の条件の確かめ

1/8 では未実施。8/8（#108）で、本リポで観測できる事実により埋める。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。

## 確かめた日付

2026-10-11（base: 5aa3371）
