# 画素の一致を確かめる共通の道具（TASK-26）の現状・前提・形（案）

TASK-26（親 Issue #114）について、着手時点（1/6、#115）の現状と前提、および形の設計（2/6、#116）の案を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・閾値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-26 と、`docs/spec/02-poc-plan.md` の閾値の表の T-3 を参照する（ポインタのみ）。
画素の比べ方のコードと公開テストは、まだ本リポに無い。このため道具の振る舞いは本書に書かない。
形の設計（案）とオーナー判断待ちは、オーナーの確認を経るまで確定扱いにしない。
後続（3/6〜6/6）は、本書の該当節へ追記する。

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | PR #3 で骨格を追加済み。`cargo metadata --format-version 1 --all-features --locked` で member 4・パッケージ 4・外部依存 0 件。依存の向きは contract → core、exec → core、upper → core・exec。`make verify`・`make deny` が通る |

## 着手時点の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | member 4（core / contract / exec / upper）・パッケージ 4・外部依存 0。`Cargo.lock` の `[[package]]` も 4 | `cargo metadata` の集計、`Cargo.lock` |
| 既存の比べ方の道具 | `crates/core/src/parity.rs` と `parity/`（`cpu_reference.rs`・`tolerance.rs`・`topk.rs`）。`core` の feature `test-support` の裏にあり、`contract` の `test-support` が転送する。どの crate の `default` にも入らない。TASK-24 の仮置き（[parity-tool](parity-tool.md)） | 各 `Cargo.toml`、`crates/core/src/lib.rs` |
| 置き場・feature の承認状態 | 未承認の仮置き。[parity-tool](parity-tool.md) の Q1・Q3・Q11 は未回答 | [parity-tool](parity-tool.md) |
| `core` の `unsafe` | `#![forbid(unsafe_code)]` の 1 行だけ | `crates/core/src` を `unsafe` で grep |
| 画像・画素を比べるコード・テスト・文書 | 0 件 | `crates`・`docs`（`docs/spec` を除く）・`README.md`・`CLAUDE.md` を `pixel`・`画素`・`rgba`・`image` で grep |
| `test-support` を含む feature の組 | 4 組（背後実装なし・Metal だけ・Vulkan だけ・CUDA だけ） | `scripts/build-profiles.sh` の `NATIVE_SETS` |
| 比較基準の wgpu | 製品 workspace の外（`baseline-wgpu/`。ルート `Cargo.toml` の `exclude`）。製品の crate から依存できない | ルート `Cargo.toml`、[baseline-wgpu](baseline-wgpu.md) |
| `docs/spec` | gitlink で `update = none`。`ci.yml` は取得しない | `.gitmodules`、`.github/workflows/ci.yml` |

- `make verify` は `--all-features` で clippy と test を流すため、feature の裏に置くコードも検査の対象になる

## 関連タスクの状態と引き継ぎ

事実のみを書く。向きは決めない。

| 関連 | Issue | 状態 | 引き継ぎ |
| ---- | ----- | ---- | -------- |
| TASK-24 | #100 | OPEN（sub-issue #101〜#108 は全てクローズ。完了は宣言されていない） | `parity` の置き場・feature は未承認の仮置き。TASK-26 の側で確定扱いにしない |
| TASK-23 | #91 | OPEN | 共通の試験の枠は本リポにまだ無い。あるものとして扱わない |
| TASK-53 | #196 | OPEN | 閾値を当てる範囲の判断はこのタスクに委ね、TASK-26 の側で広げも狭めもしない |
| TASK-25 | #109 | OPEN（sub-issue は未着手） | `parity` 配下に規則を足す並行のタスク。同じモジュールに触れうる |
| TASK-27 | #121 | OPEN | 並びのタスク |
| TASK-6 | #35 | CLOSED | 持ち込む・除くの区分は決定の記録（private spec）に従う。[poc-carry-over](../.claude/rules/poc-carry-over.md)・[workspace-boundary](workspace-boundary.md) を参照。本リポの submodule の固定 SHA は、この記録より前（[parity-tool](parity-tool.md) に既出の公開の事実） |
| TASK-7 | — | — | 比較基準の wgpu は製品 workspace の外。製品の crate から依存できない |

- 依存: 外部依存は 0。追加は未承認で、必要なら [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに乗せる

## 形の設計（案）

既存の決定から一意に導ける点だけを、根拠のポインタ付きで記録する。導けない点は次節に送る。
本節と次節はオーナーの確認を経るまで確定扱いにしない。

| # | 点 | 形（案） | 根拠 |
| - | -- | -------- | ---- |
| D1 | PoC からの持ち込みか書き直しか | PoC のファイル・記録・入力・生成物は複製せず、本リポの規約で新しく書く | TASK-6 の決定の記録（ポインタ）、[poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |
| D2 | 依存・unsafe | 標準ライブラリだけで書く。外部依存は 0 のまま（画像ファイルの読み込み・復号は道具に含めない）。`unsafe` を使わない | [dependency-policy](../.claude/rules/dependency-policy.md)、[coding-rust](../.claude/rules/coding-rust.md)、`crates/core/src/lib.rs` の `#![forbid(unsafe_code)]` |
| D3 | 背後実装の feature との関係 | `core` に置く限り、背後実装の feature による分岐は書けない。この置き場の帰結として GPU の実機なしの CI で走る（Q1 の (a) の場合） | `core` の `Cargo.toml` に backend の feature が無いこと。宣言の無い feature 名の `cfg` は `unexpected_cfgs` でエラーになること（[parity-tool](parity-tool.md) の「現状の形」に既出） |
| D4 | 道具が workspace の外に依存しないこと | 道具の側から `baseline-wgpu/` を参照しない。入力は呼び出し側が渡す | ルート `Cargo.toml` の `exclude`、`make deny` の path の検査、[poc-carry-over](../.claude/rules/poc-carry-over.md)（測定 → 製品の一方向） |
| D5 | 大きさの計算と確保 | 幅・高さなどの積は `checked_*` で計算し、オーバーフローや確保の失敗は panic せずエラーで返す。エラーの表示は英語 | [security](../.claude/rules/security.md)、[coding-rust](../.claude/rules/coding-rust.md)、[japanese-style](../.claude/rules/japanese-style.md) |
| D6 | 閾値の値の扱い | 本書に数値を持たない。値・規則の正は spec 側で、本リポの都合で変えない | `docs/spec/02-poc-plan.md` の T-3（ポインタのみ）、[parity-tool](parity-tool.md) の「値・規則を変える変更の扱い（案）」 |
| D7 | 適用の範囲 | 本書では記述しない | TASK-53（#196）に委ねる（ポインタのみ） |
| D8 | TASK-23 の枠との接続 | 本書では記述しない。枠は本リポにまだ無く、あるものとして扱わない | TASK-23（#91）（ポインタのみ） |
| D9 | 並行する TASK-25 との衝突の避け方 | 追加は新しいファイル 1 つと `pub mod` 1 行に留め、既存の `tolerance.rs`・`topk.rs`・`cpu_reference.rs` の公開アイテムを変えない（Q1 の (a) の場合） | `crates/core/src/parity.rs` の冒頭コメント、TASK-25（#109）（ポインタのみ） |
| D10 | 走る場所 | 既存の `parity` 配下に置く場合、`make verify` と `make build-profiles` の `test-support` を含む 4 組で走る。新しい make ターゲット・CI ジョブは足さない（Q1 の (a) の場合） | `scripts/build-profiles.sh` の `NATIVE_SETS`、`scripts/verify.sh` |

## オーナー判断待ち

下表の推奨は案であり、確定ではなく、オーナーの承認を得たものでもない。
回答の形は [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) に従い、「推奨で」と答えられる形にする。

| # | 論点 | 選択肢 | 推奨（未承認） | 止まるもの |
| - | ---- | ------ | -------------- | ---------- |
| Q1 | 置き場 | (a) 既存の `crates/core/src/parity/` に新しいファイルを足す (b) `parity` とは別のモジュールにする (c) [parity-tool](parity-tool.md) の Q1 の回答が出るまで置かない | (a)。`parity` の置き場が動けば一緒に動く | #117・#118 |
| Q2 | feature | (a) 既存の `test-support` の裏に置く (b) 道具専用の feature を新しく作る | (a)。[parity-tool](parity-tool.md) の Q3 と同じ向き | #117 の `Cargo.toml` |
| Q3 | 画像の入力の受け方 | (a) 画素の列の借用と幅・高さを束ねた専用の借用型を作り、生成時に長さを検査する (b) 比べる関数の引数に画素の列・幅・高さを直接並べる | (a) | #117 |
| Q4 | 行の詰め物（stride）の扱い | (a) 詰め物の無い列だけを受け、取り除くのは呼び出し側 (b) 行の幅（stride）も受ける | (a)。(b) は後から足しても互換 | #117 |
| Q5 | 2 つの比べの入口の分け方 | (a) 比べごとに別の関数にする (b) 1 つの関数に比べの種類を表す列挙を渡す | (a)。既存の `compare_f32`・`compare_topk` が比べごとに別の関数である形に揃える | #117・#118 |
| Q6 | 閾値の設定の見せ方 | (a) 固定の定数だけを公開し、値は getter で読める。任意の値を作る口は持たない (b) 検査付きのコンストラクタも公開する | (a)。[parity-tool](parity-tool.md) の Q6 と同じ向き | #118・#119 |
| Q7 | 結果とエラーの型 | (a) 画素の比べ専用のエラー型と集計の型を新しく作る (b) 既存の `CompareError` に variant を足して再利用する (c) 既存の `CompareError` を今の variant のまま再利用する | (a)。D9 と両立する | #117・#118 |
| Q8 | 大きさ 0 の入力 | (a) エラー (b) 0 件を比べて一致 | (a)。[parity-tool](parity-tool.md) の Q7 と同じ向き | #117・#119 |
| Q9 | 比較基準（workspace の外）の側から道具を呼ぶ経路 | (a) 本タスクでは決めず、道具はメモリ上の入力を受ける口だけを持つ (b) `baseline-wgpu/` から `crates/core` への path 依存を足す | (a)。(b) は `baseline-wgpu/` の構成の変更で、TASK-7 の判断待ちの論点に関わる | 比較基準と突き合わせる作業（#117・#118 は止まらない） |

各案の影響:

- Q1 (a): 既存の依存の向きのまま 4 crate のテストから届く。[parity-tool](parity-tool.md) の Q1・Q3 が未回答であることをそのまま引き継ぐ（置き場は未承認の仮置きになる）
- Q1 (b)・Q2 (b): `core` の公開面と feature が増える。`contract` 側の転送と `scripts/build-profiles.sh` の組の追加が要る
- Q1 (c): #117 以降が止まる
- Q7 (b): 既存の `CompareError` は `#[non_exhaustive]` でないため、variant の追加は網羅的な `match` を壊す。TASK-25（#109）と同じファイルに触れる
- Q9 (b): `baseline-wgpu/` は「外部依存は wgpu のみ」の現状から変わる（[baseline-wgpu](baseline-wgpu.md)）
- Q3: 要素型・チャネル数・並びは本書では具体化しない。実装と公開テストの PR でコードから記述する
- Q5: 2 つの比べ（「同じ成果物・同じ背後実装」と「背後実装の間」）は軸の名前としてだけ使う

Q1〜Q9 への回答は本リポでは確認できていない。#117・#118 が推奨を採る場合は未承認の仮置きとして明示する。

## 別案に決まったときに変わる箇所

| Q | 別案 | 変わる箇所 |
| - | ---- | ---------- |
| Q1 | (b) | 新しいファイルの置き場と `pub mod` の位置 |
| Q2 | (b) | feature 名、`contract` の転送、`scripts/build-profiles.sh` の組 |
| Q3 | (b) | 専用の型を外し、検査を比べる関数の入口へ移す |
| Q4 | (b) | 入力の型に行の幅を足す（追加だけ） |
| Q5 | (b) | 関数を 1 つにまとめ、列挙を足す |
| Q6 | (b) | コンストラクタの追加 |
| Q7 | (b)・(c) | 専用の型を外し、`tolerance.rs` の型を使う |
| Q8 | (b) | エラーの variant を外す |
| Q9 | (b) | `baseline-wgpu/Cargo.toml` の変更（承認の経路は TASK-7 側） |

## 後続の分担

| 作業 | 担当 |
| ---- | ---- |
| 現状と前提の確認 | #115（1/6。済） |
| 形の設計（本書） | #116（2/6） |
| 置き場の骨格・入力の受け口と検査（案） | #117（3/6） |
| 比べの関数と閾値の設定（案） | #118（4/6） |
| テスト（別 crate からの確かめ、feature の組ごとの確かめを含む） | #119（5/6） |
| 完了の条件の確かめ | #120（6/6） |

#117・#118 の切り方は案。Issue 本文の範囲と食い違うと実装側が判断した場合は、実装側の計画を優先する。

## 完了の条件の確かめ

2/6 までは未実施。6/6（#120）で、本リポで観測できる事実により埋める。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。

## 確かめた日付

2026-10-11（base: 40817ac）
