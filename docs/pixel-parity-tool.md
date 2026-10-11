# 画素の一致を確かめる共通の道具（TASK-26）の現状と前提

TASK-26（親 Issue #114）について、着手時点（1/6、#115）の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・閾値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-26 と、`docs/spec/02-poc-plan.md` の閾値の表の T-3 を参照する（ポインタのみ）。
画素の比べ方のコードと公開テストは、まだ本リポに無い。このため道具の振る舞いは本書に書かない。
後続（2/6〜6/6）は、本書の該当節へ追記する。

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

## 形の設計（#116）で決める論点とオーナー判断待ち

本節は論点の列挙だけで、推奨案は書かない。既存の決定から一意に導けないものは、[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路でオーナーに伺う。得ていない承認は書かない。

- 置き場と feature: 既存の `parity` 配下に足すか、別にするか。[parity-tool](parity-tool.md) の Q1・Q3 が未回答であることを引き継ぐ
- 画像の入力の表現: 幅・高さ・チャネル・並びを型でどう受けるか
- 「同じ成果物・同じ背後実装」と「背後実装の間」の 2 つの比べを、入口としてどう分けるか
- 閾値の設定の見せ方: 外から確かめられる形にするか。[parity-tool](parity-tool.md) の Q6 と同じ向きにするか
- 結果とエラーの形: 既存の `CompareError` を再利用するか
- 製品 workspace の外にある比較基準の出力を、道具へどう渡すか
- TASK-23 の枠との接続
- PoC からの持ち込みか書き直しか（TASK-6 の決定の記録のポインタのみ）

## 後続の分担

| 作業 | 担当 |
| ---- | ---- |
| 現状と前提の確認（本書） | #115（1/6） |
| 形の設計 | #116（2/6） |
| 実装 | #117・#118（3/6・4/6） |
| テスト | #119（5/6） |
| 完了の条件の確かめ | #120（6/6） |

## 完了の条件の確かめ

1/6 では未実施。6/6（#120）で、本リポで観測できる事実により埋める。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。

## 確かめた日付

2026-10-11（base: c77021c）
