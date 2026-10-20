# 実装に含める範囲の決定の記録（TASK-43）の所在と本リポの現状

実装に含める範囲の決定の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文・候補の一覧・項目 ID の範囲や件数・理由は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-43）
- 本リポには一覧・理由・経緯を置かない（[契約の文書の分担](contract/README.md)）

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 製品 workspace の member | `crates/core`・`crates/contract`・`crates/exec`・`crates/upper`・`crates/wasm-abi` の 5 つ | ルート `Cargo.toml` の `members` |
| 背後実装の feature | `fandhe-silicon-contract` に `backend-metal` / `backend-vulkan` / `backend-cuda` があり、対応するモジュールは `src/lib.rs` で feature によりゲートされる | `crates/contract/Cargo.toml` の `[features]`、`crates/contract/src/lib.rs` |
| 中身の無い置き場 | `backend-dx12-compute` は中身の無い仮の置き場。GL（GLES）は feature を足す位置のコメントだけがあり、feature 自体は無い。どちらもコメントに受け持つタスクのポインタが付いている | `crates/contract/Cargo.toml` |
| crate の公開 | 全 crate が `publish = false` | ルート `Cargo.toml` の `[workspace.package]` |
| 領域ごとの契約の文書 | `docs/contract/` には `README.md`・`TEMPLATE.md`・`procedure.md` だけがあり、領域ごとの文書は無い | `ls docs/contract` |
| wgpu 系の禁止 | wgpu 系・naga は依存グラフに入れない | `deny.toml` の `[bans] deny`、`make deny` |

## 関連

- [workspace-boundary](workspace-boundary.md)・[contract/procedure](contract/procedure.md)・[dx12-compute-check](dx12-compute-check.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md)
