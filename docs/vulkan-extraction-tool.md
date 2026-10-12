# Vulkan の抽出の道具（TASK-98）の現状と前提

TASK-98（親 Issue #387）について、着手時点（1/6、#3530）の現状と前提を、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・判定のラベル・PoC の手順や結果は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

タスクの中身は `docs/spec/05-tasks.md` の TASK-98 と、`docs/spec/03-poc/kernel-artifacts-and-parity/` の記録を参照する（ポインタのみ）。
取り出しのコードの現状の形（4/6。未承認の仮置き）は [vulkan-operand-extraction](vulkan-operand-extraction.md) の「現状の形」に書く。本書は現状と前提だけを持つ。

## 前提タスクの状態

前提タスクの有無は `docs/spec/05-tasks.md` の TASK-98 を参照する。本書では断定しない。

## 着手時点の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | member 4・パッケージ 4・外部依存 0。`Cargo.lock` の `[[package]]` も 4。`deps-approved.toml` の記録は 0 件 | `cargo metadata --format-version 1 --all-features --locked` の集計、`Cargo.lock` |
| Vulkan の背後実装 | `crates/contract/src/backend_vulkan.rs` はモジュールのドキュメンテーションコメントだけの骨格。項目なし。チップのバインディング crate への依存なし | 同ファイル、`crates/contract/Cargo.toml` |
| ゲート | `lib.rs` で `cfg(all(feature = "backend-vulkan", not(target_arch = "wasm32")))`。feature は `backend-vulkan = ["contract-core"]` | `crates/contract/src/lib.rs`、`crates/contract/Cargo.toml` |
| `unsafe` の個数 | `crates/contract/tests/backend_vulkan_unsafe_count.rs` の期待値は `UnsafeCounts::ZERO`。増やす変更は [unsafe-policy](unsafe-policy.md) の「承認」に従う | 同テスト |
| 共通型 | `core`・`exec`・`upper` の公開面はほぼ空（`core` は `test-support` の裏の `parity` だけ）。宣言を表す共通型はまだ無い | 各 `src/lib.rs` |
| Vulkan の成果物を読む・取り出すコード・テスト | 0 件。Vulkan に触れるのは骨格のモジュール、`backend_cfg_gate.rs`、個数テスト、`scripts/build-profiles.sh` の feature の組だけ | `crates`・`docs`（`docs/spec` を除く）・`scripts`・`README.md`・`CLAUDE.md` を `vulkan` で grep |
| 検証用の道具の先例 | `crates/core/src/parity/` が `core` の `test-support` の裏にあり、`contract` の `test-support` が転送する（[parity-tool](parity-tool.md)） | 各 `Cargo.toml` |
| `build-profiles` | `backend-vulkan` を含む組は native 4・wasm32 1 | `scripts/build-profiles.sh` の `NATIVE_SETS`・`WASM_SETS` |
| 外部の道具の導入 | `scripts/doctor.sh`・`scripts/setup.sh`・`ci.yml` に Vulkan 関連の道具を入れる・要求する記述は 0 件 | 各ファイルを `vulkan` で grep |
| 比較基準の wgpu | 製品 workspace の外（`baseline-wgpu/`。ルート `Cargo.toml` の `exclude`） | ルート `Cargo.toml`、[baseline-wgpu](baseline-wgpu.md) |
| `docs/spec` | gitlink で `update = none`。`ci.yml` は取得しない | `.gitmodules`、`ci.yml` |

## 範囲の記述の確かめ（1/6）

`docs/spec/05-tasks.md` の TASK-98 の範囲の記述について、spec 側の判断待ちがある（ポインタのみ。内容は転記しない）。

- 対象: `docs/spec/05-tasks.md` の TASK-98・TASK-448・TASK-513
- 扱い: どちらにも寄せない。**spec 側の判断待ち**（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)）
- 止まりうる段: 2/6（#3531）・3/6（#3532）。本書の他の節の事実の記録は、この判断に依らない

## 関連タスクの状態と引き継ぎ

事実のみを書く。向きは決めない。

| 関連 | Issue | 状態 | 引き継ぎ |
| ---- | ----- | ---- | -------- |
| TASK-6 | #35 | CLOSED | 持ち込む・除くの区分は決定の記録（private spec）に従う。[poc-carry-over](../.claude/rules/poc-carry-over.md)・[workspace-boundary](workspace-boundary.md) を参照 |
| TASK-8 | #45 | CLOSED | 依存の追加は承認制（[dependency-policy](../.claude/rules/dependency-policy.md)） |
| TASK-9 | #50 | CLOSED | `unsafe` の個数が変わるときは [unsafe-policy](unsafe-policy.md) に従う |
| TASK-24 | #100 | CLOSED | 検証用の道具の置き場の先例（[parity-tool](parity-tool.md)） |
| TASK-23 | #91 | OPEN | 共通の試験の枠は本リポにまだ無い。あるものとして扱わない |
| TASK-99 | #388 | OPEN | 並びのタスク。分担の境界は 2/6 で扱う |
| TASK-418 | #612 | OPEN | 分担の境界は 2/6 で扱う |
| TASK-448 | #633 | OPEN | 分担の境界は 2/6 で扱う |
| TASK-513 | #669 | OPEN | 分担の境界は 2/6 で扱う |

## 方針・形で決める論点（列挙のみ）

推奨は書かない。得ていない承認は書かない。2/6（方針）・3/6（形）で決める。

- 道具の置き場（背後実装のモジュールの中・`test-support` の裏・製品 workspace の外・`scripts/`。TASK-6 の決定の記録のポインタ）
- 入力の受け方と、外部の CLI 道具・依存の要否（依存は承認制。外部の道具は `doctor`・CI への導入の要否が論点）
- 実機の GPU の要否と、CI（GitHub ホステッド）で回る範囲
- 取り出す対象の範囲と、取り出せないものがある場合の扱い（上の「範囲の記述の確かめ」の判断待ちに依る）
- 突き合わせの相手になる宣言の型が本リポにまだ無いこと（出力の型と置き場）
- 関連タスクとの分担の境界
- `unsafe` を足す場合の個数テストの期待値と承認
- PoC からの持ち込みか書き直しか（PoC のコード・記録・入力は複製しない）
- テストの入力の置き方

## 後続の分担

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 2/6 | #3531 | 方針の整理 |
| 3/6 | #3532 | 形の設計 |
| 4/6 | #3533 | 実装 |
| 5/6 | #3534 | テスト |
| 6/6 | #3535 | 確かめ |

## 完了の条件の確かめ

1/6 では未実施。6/6 で、本リポで観測できる事実により埋める。条件の本文は転記しない。
親 #387 の進み具合の spec への記録は spec リポ側の作業で、本リポからは書き換えない。

## 確かめた日付

2026-10-12（base: `6cca009`）。
