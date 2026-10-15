# 性能を測る共通の道具（TASK-27）の所在と本リポの現状

TASK-27 の決定の記録の所在と、本リポのコードで確かめられる現状を案内する（親 Issue は #121）。
2/8（#123）で「形の設計」の節を足した（決定の記録のポインタ・道具が乗る既存のコードの事実・守る本リポの規約）。
spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 決定の記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-27）
- 前提のタスクはポインタのみ（TASK-4・TASK-28・TASK-19）。対応する公開側の文書は次のとおり
    - TASK-4: ルートの `Cargo.toml` の `members` と [workspace-boundary](workspace-boundary.md)
    - TASK-28: [measurement-record](measurement-record.md)
    - TASK-19: [metal-run-procedure](metal-run-procedure.md)
- 本リポの `docs/spec` の gitlink は `0696aed`（`update = none`）
    - 確かめ方: `git ls-tree origin/main docs/spec`
    - gitlink の更新は `update-external.yml` の PR 経由で、本作業では進めない

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 製品の workspace | `members` は 4 crate、`exclude` は `baseline-wgpu`。外部依存の承認の記録は 0 件 | `grep -n -e members -e exclude Cargo.toml`、`grep -c '^\[\[' deps-approved.toml` |
| 性能を測る道具 | 時間を測る標準ライブラリの型の使用が `crates/`・`baseline-wgpu/` に無い | `grep -rn -e Instant -e SystemTime -e 'std::time' crates baseline-wgpu`（0 件） |

既にある関連の部品（名前とリンクのみ）:

- `crates/core` の `test_frame` と記録の集計 `scripts/record-summary.sh`: [test-frame](test-frame.md)
- 回ごとの記録の形の検査と手順のスクリプト: [metal-run-procedure](metal-run-procedure.md)
- 比較基準の wgpu の別 workspace: [baseline-wgpu](baseline-wgpu.md)
- 機材の要らない CI の枠: [ci-device-free-jobs](ci-device-free-jobs.md)

## 形の設計（2/8）

本段の範囲は `docs/spec/05-tasks.md` の TASK-27 の内容の前半。
具体（置き場の名前・型・欄・値・統計の定義・出力の先）は 3/8（#124）以降のコードと公開テストが示し、本節には先に書かない。
本節に書くのは、決定の記録のポインタ、道具が乗る既存のコードの事実、どの形でも守る本リポの規約。

### 決定の記録に従う点

| 点 | 扱い |
| -- | ---- |
| 道具の形 | `docs/spec/05-tasks.md` の TASK-27 と、そこから辿る決定の記録（T27-Q1〜Q6）に従う。本書には内容を書かない |
| 記録の形と置き場 | TASK-28 のポインタ（[measurement-record](measurement-record.md)） |
| 専有の確かめの手順 | TASK-19 のポインタ（[metal-run-procedure](metal-run-procedure.md)） |
| 測定専用のコードの置き場 | TASK-6 のポインタ（[workspace-boundary](workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)） |
| 試験の結果の部品との分担 | TASK-23 のポインタ（[test-frame](test-frame.md)） |
| 範囲の外 | TASK-1354・TASK-1356・TASK-53 のポインタ。本節では扱わない |
| 決定の記録から一意に決まらない点 | [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路で伺う。推奨案を既定として確定しない |

### 道具が乗る既存のコード・検査の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験の結果の部品 | `crates/core` の `test_frame::record` が `RecordPart` と `BODY_KIND` を持つ（`test-support` の裏）。run の欄を足して 1 件に組むのは受け取る側 | `crates/core/src/test_frame/record.rs` の `//!` |
| 部品の書き出し | `test_frame::export` が `RECORD_DIR_ENV` で指定されたディレクトリへ書く（指定が無ければ書かない） | `crates/core/src/test_frame/export.rs` |
| 回ごとの記録の形の検査 | `scripts/session-record-check.sh`。make ターゲットは無い | ファイル冒頭のコメント |
| 環境の集計値 | `scripts/metal-session.sh probe` が JSON を標準出力へ出す（macOS だけ） | ファイル冒頭のコメント |
| 部品の集計 | `scripts/record-summary.sh` | [test-frame](test-frame.md) |
| path の検査 | `scripts/deny.sh` の `ALLOWED_ROOT` は `crates/`。測定専用の crate の置き場が決まったら見直す、とコメントにある | `scripts/deny.sh` |
| 別 workspace の前例 | `baseline-wgpu/` はルートの `exclude`。依存の検査は手動で、`make`・CI の対象外 | [baseline-wgpu](baseline-wgpu.md) |
| CI の `deny` ジョブ | `make deny`、`make deps-check`、各回帰テストの順に呼ぶ | `.github/workflows/ci.yml` |
| 外部依存 | 製品の workspace の承認の記録は 0 件 | `deps-approved.toml` |

### どの形でも守る本リポの規約

| # | 点 | 守ること | 根拠 |
| - | -- | -------- | ---- |
| M1 | 製品の workspace | 製品の crate は道具に依存しない。向きは「測定 → 製品の公開 API」の一方向。製品の `members`・依存グラフ・`Cargo.lock` に足場を入れない | [poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |
| M2 | 依存 | 標準ライブラリだけで書く。依存が要る場合は承認の経路と `=x.y.z` 固定 | [dependency-policy](../.claude/rules/dependency-policy.md) |
| M3 | unsafe | 道具のコードは `unsafe` を持たない | [coding-rust](../.claude/rules/coding-rust.md)、[unsafe-policy](unsafe-policy.md) |
| M4 | エラー | ライブラリの部分は `Result` を返し panic しない。取得できない値を暗黙に埋めない | [coding-rust](../.claude/rules/coding-rust.md) |
| M5 | 数値 | 既存の規約に従う（具体は実装と公開テストの PR で書く） | [security](../.claude/rules/security.md) |
| M6 | 機械可読の出力 | 既存の先例に従う（具体は実装と公開テストの PR で書く） | [test-frame](test-frame.md) |
| M7 | 出力に含めない情報 | 既存の規約と TASK-28 のポインタに従う（具体は実装と公開テストの PR で書く） | [security](../.claude/rules/security.md)、TASK-28 のポインタ |
| M8 | 信頼しない入力 | 既存の規約と検査に従う（具体は実装と公開テストの PR で書く） | [security](../.claude/rules/security.md)、`scripts/session-record-check.sh` |
| M9 | 検査は fail-closed | 形が合わない・前提が足りないときは成功扱いにしない。終了コードは既存の検査に揃える | 既存の `scripts/*-check.sh` |
| M10 | 純粋な計算と I/O | 計算は純粋な関数に寄せ、書き出しと分ける。テストはプロセスの環境と作業ツリーを書き換えない | `test_frame` の先例 |
| M11 | CI | ジョブと必須チェックを増やさず、既存のジョブに手順を足す。CI 用のツールは版固定・ハッシュで検証する | [github-actions](../.claude/rules/github-actions.md)、[ci-job-split](ci-job-split.md) |
| M12 | 測らなかった結果 | skip・ignore・アサーションの弱体化で CI を通さない。取得できなかった値を成功にも失敗にも読み替えない | [coding-rust](../.claude/rules/coding-rust.md) |

### 3/8（#124）への引き継ぎ

- 着手時に、gitlink の版の spec の TASK-27 とそこから辿る決定の記録を読み直す。一意に導けない点は推奨案で確定せず伺う
- 決定の記録のポインタに従い、置き場のディレクトリ名・crate 名などは 3/8 がコードで決め、選んだ形を PR 本文に書く
- 置き場を足す段では、同じ PR で次を揃える
    - ルートの `Cargo.toml` の `exclude`
    - `docs/workspace-boundary.md`・`docs/baseline-wgpu.md` の関係の記述
    - `scripts/deny.sh` の `ALLOWED_ROOT` のコメント
    - `README.md`・`CLAUDE.md` のツリー
    - 本書の「本リポの現状」の表
- 公開アイテム・スクリプトを足したら、本書に「現状の形（N/8）」を足し、具体はその PR のコードとテストに合わせて書く

### 段の表

| Issue | 段 |
| ----- | -- |
| #122 | 1/8 現状と前提の確認 |
| #123 | 2/8 形の設計（本節） |
| #124 | 3/8 実装 |
| #125 | 4/8 実装 |
| #126 | 5/8 実装 |
| #127 | 6/8 実装 |
| #128 | 7/8 テスト |
| #129 | 8/8 確かめ |

## 本リポでの反映の方針

- 道具の置き場・記録の欄・統計・検査は、受け持つ段の PR のコードと公開テストが示す（親 #121）
- 本書には、その PR で入った振る舞いを書き足す

## 関連

- [measurement-record](measurement-record.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
