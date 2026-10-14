# 公開 API の名前の検査（TASK-14・TASK-113）の決定の記録の所在と本リポの現状

TASK-14・TASK-113 の決定の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-14・TASK-113 と、そこから辿る決定の記録。関連: TASK-10・TASK-112）
- 本リポには決定の記録・内容を置かない（[契約の文書の分担](contract/README.md)）

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| スクリプト | この検査のスクリプトは `scripts/` に無い | `ls scripts` |
| 設定ファイル・CI | この検査の設定ファイル・CI のジョブ・Makefile のターゲットは無い | `grep -rn -i -E "name-check\|public-name" scripts .github Makefile lefthook.yml deny.toml` が何も返さない |
| 公開面 | `crates/contract/src/lib.rs` の `pub mod` は `backend_metal`・`backend_vulkan`・`backend_cuda` の feature ゲート付き 3 つ。`crates/core/src/lib.rs` は `test-support` の裏の `pub mod parity` と `pub mod test_frame` のみ。`crates/exec`・`crates/upper` の `lib.rs` に `pub mod`・`pub use` は無い | `grep -n "pub mod\|pub use" crates/*/src/lib.rs` |

## 本リポでの反映の方針

- 検査の実装・設定・CI への組み込みは、それを受け持つタスクの PR で、公開テスト（回帰テスト）とともに入れる（ポインタ: TASK-113。関連: TASK-10・TASK-112）
- 検査の内容は、その PR のコードとテストが示す
- 進め方の追跡は親 Issue #394 に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
- [implementation-scope](implementation-scope.md)
