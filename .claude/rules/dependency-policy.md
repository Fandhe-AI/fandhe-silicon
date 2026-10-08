# 依存管理規約（リポ固有）

## 原則

- **依存最小方針**: 本リポは上位（fandhe-ai・vector-db・fandhe-3d）の共通基盤であり、依存は推移的に全上位へ波及する。外部クレートへの依存は可能な限り避ける
- **wgpu 非依存**: wgpu・wgpu-core・wgpu-hal・wgpu-types・naga を依存グラフに入れない（spec D-18。`deny.toml` の `[bans] deny` で CI が検出する）
- **共通型 crate**: `crates/core` の外部依存は spec BUILD-54 の範囲に限る
- **完全固定**: 採用する依存は `Cargo.toml` で `=x.y.z` の完全固定（exact pin）で管理する（`^`・`~`・範囲指定は禁止）
- **ユーザー承認制**: 依存の追加・更新・削除は必ずユーザーの明示承認を経てから行う

## 承認を求める際に提示する情報

1. クレート名・バージョン（`=x.y.z`）・目的（なぜ自作でなく依存か）
2. ライセンス（MIT / Apache-2.0 互換であること）
3. メンテナンス状況（最終リリース日・リポジトリの活動状況）
4. 推移的依存の概要（大量の間接依存・wgpu 系を引き込まないか）
5. 対応プラットフォーム（macOS / Linux / Windows のどこでビルドが必要か）

## subagent への適用

- builder Agent（core-builder / backend-builder）は依存の追加・更新を行わない。必要と判断した場合は「承認事項」として main へ報告し、main がユーザーに確認する
- reference-researcher は候補情報の収集までを行い、採否を判断しない
