# DX12 の compute に関する点検の記録（TASK-236）の所在と本リポの現状

TASK-236（親 Issue #461）の点検の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文・点検の結果は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 点検の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-236。関連: `docs/spec/04-behavior/build-profiles.md` の BUILD-18・BUILD-64）
- 本リポには点検の記録・結果を置かない（[契約の文書の分担](contract/README.md)）

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature | `backend-dx12-compute` は `contract-core` だけを有効にする仮の置き場。`default` には入っていない | `grep -n -E "^default|dx12" crates/contract/Cargo.toml` |
| モジュール | `crates/contract/src` に DX12 のモジュールは無い（Metal・Vulkan・CUDA の 3 つだけ） | `ls crates/contract/src`、`grep -rni dx12 crates --include='*.rs'` |
| 公開面 | `crates/contract/src/lib.rs` の `pub mod` は `backend_metal`・`backend_vulkan`・`backend_cuda` の feature ゲート付き 3 つ。`crates/exec`・`crates/upper` の `lib.rs` に `pub mod` は無い。`crates/core` は `parity` のみ | `grep -n "pub mod" crates/*/src/lib.rs` |
| build-profiles | `scripts/build-profiles.sh` の feature の組にこの feature は現れない。`--all-features` の実行でだけ有効になる | `grep -n -i dx12 scripts/build-profiles.sh`、`grep -n "all-features" scripts/build-profiles.sh` |

## 本リポでの反映の方針

- DX12 の compute の背後実装を足すかどうかは spec 側の決定に従い、本リポの側では決めない
- 足す場合は、受け持つタスクの PR で feature・モジュール・契約の文書を実装と公開テストとともに入れる（[procedure](contract/procedure.md)）

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [implementation-scope](implementation-scope.md)
