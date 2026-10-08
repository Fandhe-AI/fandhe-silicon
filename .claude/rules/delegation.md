# 委譲ルール（調査・設計フェーズ）

## 原則

main セッションはオーケストレーションに徹し、コンテキスト消費の大きい作業
（ファイルの大量読み込み・横断検索・外部仕様調査）は subagent へ委譲する。
main が直接ファイルを読むのは、委譲結果の確認や小さなピンポイント参照に限る。

## パスベース切り替え表（調査）

| 対象パス・内容 | 委譲先 Agent | model |
| -------------- | ------------ | ----- |
| `crates/`（core / contract / exec / upper）のコード調査・構造把握・影響範囲 | explorer | sonnet |
| `scripts/`・`Makefile`・`lefthook.yml`・`.github/workflows/` の構造把握 | explorer | sonnet |
| `docs/spec/`（private submodule）のタスク・ビヘイビア参照 | explorer（ポインタ表記で報告） | sonnet |
| Metal / Vulkan / CUDA / MSL / PTX 等の外部 API・仕様 | reference-researcher | sonnet |
| wgpu の挙動・置き換え対象 API の調査 | reference-researcher | sonnet |
| 依存候補クレート（FFI バインディング等）の調査 | reference-researcher | sonnet |
| lint・フォーマット状況の確認 | linter | haiku |

※ crate 構成は spec D-33 の 3 段 + 共通 crate に対応する暫定構成（公開名・下-1 のチップ別分割は未決）。構成を変えたら本表も更新する。

## 参照スキル

外部仕様の一次情報は、Web 検索より先に導入済みのリファレンススキルを当たる
（`apple-silicon`・`apple-graphics`・`nvidia-cuda`・`dgx-spark`・`windows-graphics-media`・`rust`・
`fandhe-ai`・`fandhe-db`）。reference-researcher への指示にも参照すべきスキル名を含める。

## model 配分

| 用途 | model |
| ---- | ----- |
| 複雑な横断判断・アーキテクチャ設計（backend 抽象の境界設計等） | opus または fable（fable は特に大規模設計・横断判断の最上位 tier） |
| 調査・生成・実装・レビュー | sonnet |
| 機械的集計・lint・ドキュメント更新 | haiku |

## 注意

- 調査結果に `docs/spec` の本文を含めない（[spec-confidentiality](./spec-confidentiality.md)）
- 複数の独立した調査は 1 メッセージで並列に委譲する
