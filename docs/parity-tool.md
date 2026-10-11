# 結果の一致を確かめる共通の道具（TASK-24）の現状と前提

TASK-24（親 Issue #100）について、着手時点の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・閾値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | PR #3 で骨格を追加済み。`cargo metadata --format-version 1 --all-features --locked` で member 4・パッケージ 4・外部依存 0 件。依存の向きは contract → core、exec → core、upper → core・exec。`make verify`・`make deny` が通る |

## 着手時点の骨格の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | member 4（core / contract / exec / upper）。外部依存 0 | `cargo metadata` の集計 |
| 公開面 | 4 crate とも crate 冒頭の doc コメントのみ。`core`・`exec`・`upper` は `#![forbid(unsafe_code)]` | `crates/*/src/lib.rs` |
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
| TASK-6 | #35 | 親が未完了 | PoC の隔離 workspace から持ち込む場合は決定の記録と [poc-carry-over](../.claude/rules/poc-carry-over.md) に従う。決定の記録に無いものは持ち込まない |

- 依存: 外部依存は 0。追加は未承認で、必要なら [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに乗せる
- `cpu-ops` に関わる下流のポインタ: TASK-1350・TASK-1378

## 形の設計（#102）で決める論点 / オーナー判断待ち

推奨案は書かない。既存の決定から一意に導けないものは [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路でオーナーに伺う。

- 置き場（どの crate か、feature の裏か、テスト専用か）
- 公開面（製品の公開 API に含めるか、検証用に留めるか）
- TASK-23 の枠との接続の仕方
- TASK-25 が足す部分の受け口
- PoC からの持ち込みか書き直しか（TASK-6 の確定に依存）

## 後続の分担

後続は本文書の該当節へ追記する。

| 作業 | 担当 |
| ---- | ---- |
| 形の設計 | #102（2/8） |
| 実装 | #103〜#105（3/8〜5/8） |
| 方針の整理 | #106（6/8） |
| テスト | #107（7/8） |
| 確かめ | #108（8/8） |

## 完了の条件の確かめ

1/8 では未実施。8/8（#108）で、本リポで観測できる事実により埋める。
spec の完了の条件との対応づけはオーナーが確認する。条件の本文は転記しない。

## 確かめた日付

2026-10-11（base: 098303e）
