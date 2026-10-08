# 委譲ルール（作成・編集フェーズ）

## 原則

コードの作成・編集は担当レイヤの builder Agent へ委譲し、main は計画・レビュー・統合に徹する。

## パスベース委譲マッピング（実装）

| 対象パス・内容 | 委譲先 Agent | model |
| -------------- | ------------ | ----- |
| backend 非依存の共通 API（device・メモリ・実行・同期・能力問合せ・診断の trait / 型 / CPU 実装） | core-builder | sonnet |
| Metal / Vulkan / CUDA の背後実装・FFI バインディング・シェーダ / カーネル | backend-builder | sonnet |
| `scripts/`・`Makefile`・`lefthook.yml`・`.github/workflows/`・`.editorconfig` | core-builder（小規模なら main 直接可） | sonnet |
| テスト実行・失敗解析（`cargo test` / `cargo clippy` / `make check`） | test-runner | sonnet |
| ベンチマーク計測・性能回帰検出 | bench-runner | sonnet |
| コードレビュー | reviewer | sonnet |
| セキュリティ監査（unsafe / FFI 境界・秘密情報・spec 漏えい） | security-auditor | sonnet |
| lint・整形の機械的確認 | linter | haiku |
| README・CLAUDE.md・ドキュメント更新 | docs-writer | haiku |

※ クレート構成は未確定。`crates/` 導入時に「対象パス」列を実際のクレートパスへ更新する。

## 実装フローの標準形

1. 計画（main。必要に応じて explorer / reference-researcher で事前調査）
2. 実装（builder へ委譲。core と backend が独立なら並列可。共通 API の変更が先行する場合は core → backend の順）
3. 検証（test-runner → 失敗があれば builder へ差し戻し。性能影響がある変更は bench-runner）
4. レビュー（reviewer / security-auditor。unsafe・FFI を含む変更は security-auditor 必須）
5. コミット（create-commit スキル。Conventional Commits・`--no-verify` 禁止）

## 注意

- 依存（`Cargo.toml` の dependencies）の追加・更新は builder に委譲せず、必ずユーザー承認を経る（[dependency-policy](./dependency-policy.md)）
- CI にジョブを追加する場合は `ci.yml` の `ci-complete` の `needs` にも必ず追加する
- スコープ外の発見事項は放置せず [out-of-scope-tracking](./out-of-scope-tracking.md) に従い追跡する
