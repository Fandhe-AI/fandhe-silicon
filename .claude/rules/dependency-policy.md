# 依存管理規約（リポ固有）

## 原則

- **依存最小方針**: 本リポは上位（fandhe-ai・vector-db・fandhe-3d）の共通基盤であり、依存は推移的に全上位へ波及する。外部クレートへの依存は可能な限り避ける
- **wgpu 非依存**: wgpu・wgpu-core・wgpu-hal・wgpu-types・naga を依存グラフに入れない（spec D-18。`deny.toml` の `[bans] deny` で CI が検出する）
- **共通型 crate**: `crates/core` の外部依存は spec BUILD-54 の範囲に限る
- **完全固定**: 採用する依存は `Cargo.toml` で `=x.y.z` の完全固定（exact pin）で管理する（`^`・`~`・範囲指定は禁止）。`make deps-check`（`scripts/deps-check.sh`）が機械で検出する（spec TASK-8）
- **ユーザー承認制**: 依存の追加・更新・削除は必ずユーザーの明示承認を経てから行う

## 承認を求める際に提示する情報

1. クレート名・バージョン（`=x.y.z`）・目的（なぜ自作でなく依存か）
2. ライセンス（MIT / Apache-2.0 互換であること）
3. メンテナンス状況（最終リリース日・リポジトリの活動状況）
4. 推移的依存の概要（大量の間接依存・wgpu 系を引き込まないか）
5. 対応プラットフォーム（macOS / Linux / Windows のどこでビルドが必要か）

## 承認の記録（`deps-approved.toml`）

承認の記録を workspace ルート直下の `deps-approved.toml` に機械可読で持ち、`make deps-check` が
記録の無い外部依存の宣言を失敗させる（spec TASK-8・AP-8 のポインタ）。書式は同ファイル先頭に書く（案。オーナー確認待ち）。

- 手順: 承認事項の提示 → ユーザーの明示承認 → 依存の追加と**同じ PR** で記録を追加 → `make deps-check`。版の更新・削除でも記録を直す
- 検出する違反: 記録なし・版の不一致・利用元（`used_by`）の不一致・使われていない記録・記録の書式違反・記録ファイルの欠落／破損
- builder Agent は記録を自分で書かず、承認事項として main へ報告する。得ていない承認を記録・コミット・PR 本文に書かない
- `purpose` に private spec の本文や内部判断を書かない

限界（検査が見ないもの）:

- 記録の有無と一致だけを見る。記録が本物の承認に基づくかは判定できないため、`approval_ref` の先に明示の承認があるかを PR レビューで確かめる
- 依存の追加と記録の追加は同じ PR でできる。記録ファイルの変更を所有者レビュー必須にする設定は別途（リポジトリ設定。オーナー判断）
- 推移的依存（`Cargo.lock` のみに現れる package）は対象外。1 crate につき記録は 1 件で、member ごとに別の版を承認する形は表せない
- どの crate からも継承されない `[workspace.dependencies]` の項目は見えない

## subagent への適用

- builder Agent（core-builder / backend-builder）は依存の追加・更新を行わない。必要と判断した場合は「承認事項」として main へ報告し、main がユーザーに確認する
- reference-researcher は候補情報の収集までを行い、採否を判断しない
