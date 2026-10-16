# 依存グラフの wgpu 系 crate の検査（TASK-11）の所在と現状の事実

製品の workspace の依存グラフに載る crate のうち、wgpu プロジェクトの crate を「狭い定義」と「広い定義」の 2 通りで数える検査について、本リポのスクリプトから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-11。数え方の決定は TASK-82。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/dep-graph-wgpu-check.sh`（bash + `python3 -I`、標準ライブラリのみ） | `scripts/dep-graph-wgpu-check.sh` |
| 対象 | 製品の workspace（ルートの `Cargo.toml`）だけ。`baseline-wgpu/`・`measure/`・`wasm/` には当てない | 同スクリプトの先頭コメント |
| 入力 | `cargo metadata` と `cargo tree`。または `--metadata-json` と `--tree-text` で渡した入力 | 同スクリプトの先頭コメント |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 使い方と終了コード

| 呼び方 | 動作 | 確かめ方 |
| ------ | ---- | -------- |
| 引数なし | cargo を流して数える | 直接実行 |
| `--metadata-json <path> [--tree-text <path>]` | 渡した入力だけを数える（cargo を呼ばない）。`--tree-text` は `--metadata-json` と組でだけ受け付ける | 直接実行 |

| コード | 意味 | 確かめ方 |
| ------ | ---- | -------- |
| 0 | 2 つの定義とも 0 件 | 直接実行して `echo $?` |
| 1 | どちらかの定義で 1 件以上（違反） | 同スクリプトの先頭コメント |
| 2 | 誤用・前提の不足・読めない入力・metadata と tree の不一致・数えた crate が 0 件。`dep_kinds` が欠けた・形の崩れた入力もここ | 同スクリプトの先頭コメント |

## 出力の読み方

件数の値は変わるので書かない。出る場所と見方だけを示す。

| 出力 | 場所 | 見方 | 確かめ方 |
| ---- | ---- | ---- | -------- |
| `scanned:` | 標準出力 | 読んだ入力ごとに 1 行（`reading=metadata` と `reading=tree`） | `scripts/dep-graph-wgpu-check.sh \| grep '^scanned:'` |
| `record:` | 標準出力（終了コード 0 と 1 のとき） | `commit`（cargo を流す経路では HEAD のコミット、変更があれば末尾に `-dirty`。入力を渡す経路では `-`）、`crates_total`、`workspace_members`、`narrow_total`、`wide_total`、`no_repository` | `scripts/dep-graph-wgpu-check.sh \| grep '^record:'` |
| `no-repository:` | 標準出力 | repository が無い crate を 1 件ごとに出す。違反ではない | 同スクリプトの先頭コメント |
| `ok:` | 標準出力 | 合格時の 1 行 | 直接実行 |
| `violation:` / `FAILED:` | 標準エラー | 違反した定義と crate、違反した crate の件数 | 同スクリプトの先頭コメント |
| `error:` | 標準エラー | 終了コード 2 の理由 | 同スクリプトの先頭コメント |

## 数え方の定義の所在

数え方は、スクリプトの Python 部の先頭の「数え方の定義」ブロックにだけ置く。値は本書に写さず、ブロックを正とする。コマンドの組み立て・走査・判定は、このブロックの値から作る。

| 軸 | 定義の定数 | 確かめ方 |
| -- | ---------- | -------- |
| 数える依存の種類 | `DEP_KINDS` | `grep -n DEP_KINDS scripts/dep-graph-wgpu-check.sh` |
| 数える target | `TARGET` | `grep -n TARGET scripts/dep-graph-wgpu-check.sh` |
| 数える feature | `FEATURE_ARGS` | `grep -n FEATURE_ARGS scripts/dep-graph-wgpu-check.sh` |
| 狭い定義の名前と折りたたみ | `NARROW_NAMES`・`NARROW_NAME_REPLACE` | `grep -n NARROW_ scripts/dep-graph-wgpu-check.sh` |
| 広い定義の接頭辞と repository の正規化 | `WIDE_REPOSITORY_PREFIX`・`REPOSITORY_STRIP_PREFIXES`・`REPOSITORY_SCHEMES`・`REPOSITORY_STRIP_SUFFIXES` | `grep -n REPOSITORY_ scripts/dep-graph-wgpu-check.sh` |

- 狭い定義の名前は、`deny.toml` の `[bans] deny` と対応する
- `cargo metadata` 側は `FEATURE_ARGS` と `TARGET`（`all` 以外のとき）で作り、`cargo tree` 側は `FEATURE_ARGS`・`TARGET`・`DEP_KINDS` で作る
- metadata の走査は、`dep_kinds` の kind（null は通常の依存）が `DEP_KINDS` に入る辺だけをたどる

## 数え方を変えるときの手順

| 手順 | 確かめ方 |
| ---- | -------- |
| 変える内容の決定は private spec 側の記録に従う（TASK-82。ポインタのみ） | - |
| 定義ブロックの定数だけを変える。コマンドの組み立て・走査・判定は変えない | `git diff` の変更が定義ブロックの中に収まっている |
| 名前の一覧を変えるときは、`deny.toml` の `[bans] deny` も同じ PR で変える | 両方から名前を取り出して比べる |
| 製品の workspace で直接実行し、終了コード 2 にならない（metadata と tree が食い違わない）ことと `record:` の件数を見る | `scripts/dep-graph-wgpu-check.sh` |
| `scripts/` の回帰テストがあれば、それも通す | `make` ターゲットの一覧（`make help`） |
| 品質ゲートを通す | `make check`・`make verify`・`make deny` |

## 検査しないもの

- `www.` 付きのホスト
- `git://`・`ssh://`・`git@github.com:` の形
- github.com 以外のホスト
- 組織の URL そのもの（`github.com/gfx-rs` で終わるもの）
- 別 workspace
- 件数の増減

## 確かめ方

worktree のルートで `scripts/dep-graph-wgpu-check.sh` を実行すると、`scanned:` が 2 行、`record:`、`ok:` が出て終了コードが 0 になる。書式は `make check` で確かめる。

## 関連

- [public-api-wgpu-check.md](./public-api-wgpu-check.md)
- [baseline-wgpu.md](./baseline-wgpu.md)
- [workspace-boundary.md](./workspace-boundary.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)
