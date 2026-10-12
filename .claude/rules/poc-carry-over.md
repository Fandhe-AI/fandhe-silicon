# PoC の隔離 workspace からの持ち込みの守り方

## 目的・適用範囲

private spec の TASK-6（`docs/spec/05-tasks.md`。関連: AP-4・AC-2）を前提に、PoC の隔離 workspace のコードや考え方を本リポへ持ち込む作業で適用する。
持ち込む・除くの区分とその理由の正は、private spec 側の TASK-6 の決定の記録とする。本リポには区分の一覧・理由・経緯を書かない（ポインタのみ）。
区分は TASK-6 の決定の記録に従う。決定の記録に無いものは持ち込まない（[workspace-boundary](../../docs/workspace-boundary.md)）。

## 依存グラフに足場を入れない（AP-4・AC-2）

- 製品の workspace の依存グラフ（全 feature・全 target）に、持ち込まないと決めた足場を 1 件も入れない
- 現在の機械的な守りは `make deny`

| 守り | 設定 |
| ---- | ---- |
| 全 feature を対象にする | `deny.toml` の `[graph] all-features = true` |
| wgpu 系・naga の禁止 | `deny.toml` の `[bans] deny` |
| git 依存の禁止 | `deny.toml` の `[sources] unknown-git = "deny"` |

- target は指定していないので、全 target が対象になる
- 追加で要る検査の方法（実装は別 Issue。ここでは名前だけ挙げる）:
    - `cargo metadata --format-version 1 --all-features` で、workspace メンバーが `crates/` 配下だけであることを確かめる
    - path 依存が workspace の外（`docs/spec` を含む）を指していないことを確かめる

## PoC の記録を写さない・書き換えない

- PoC の記録（コード・記録用の bin・JSON などの生成物を含む）を本リポへ複製しない
- `docs/spec` 配下を書き換えない（[contract-shaping-escalation](./contract-shaping-escalation.md) の共通原則に従う）
- 持ち込むコードは、本リポの規約に沿って書き直す
    - unsafe・`// SAFETY:`・依存の承認と `=x.y.z` 固定（[coding-rust](./coding-rust.md)・[dependency-policy](./dependency-policy.md)）
    - コメント（[code-comment-style](./code-comment-style.md)）

## 測定用のコードの置き方の向き

- 製品の crate は、測定専用の crate・ハーネスに依存しない。向きは「測定 → 製品の公開 API」の一方向だけにする（TASK-15 の検査のポインタ）
- 比較用の wgpu を使うコードは基盤外の別 workspace に置く（`deny.toml` に記載済みの方針。TASK-7）
- 測定専用の crate の置き場と、上の層の試作の扱いは、TASK-6 の決定の記録に従う（本規約には内容を書かない。関連: TASK-15・TASK-1050・TASK-1022・TASK-1354・TASK-1356）

## 対象の外

製品として作る上の層（`crates/upper`）の実装は、持ち込む・除くの対象ではない（UPPER 系タスクのポインタ）。

## 区分に迷うとき

決定の記録から区分が一意に決まらないものは、自動運転では決めない。推奨案を既定として先行確定せず、[contract-shaping-escalation](./contract-shaping-escalation.md) の経路でオーナーに伺う（伺いの情報は作業の summary に残し、PR 本文には書かない）。

## 関連規約

- [spec-confidentiality](./spec-confidentiality.md)・[contract-shaping-escalation](./contract-shaping-escalation.md)・[dependency-policy](./dependency-policy.md)・[coding-rust](./coding-rust.md)・[out-of-scope-tracking](./out-of-scope-tracking.md)
