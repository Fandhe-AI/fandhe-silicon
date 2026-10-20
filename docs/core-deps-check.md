# 共通の crate の依存の検査（TASK-258）の所在と現状の事実

共通の crate（`crates/core`）が宣言する外部の crate の一覧が決められた一覧と完全に一致することと、その feature が契約の crate（`crates/contract`）の同じ名前の feature から転送されていることを機械で確かめる検査について、本リポのスクリプトとテストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-258。関連: BUILD-54・U-30。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
隣の検査 [public-api-sdk-check](./public-api-sdk-check.md) は入ってはいけないチップの SDK の crate の名前を数える。本検査は入ってよい名前の一覧との完全一致を見るので、チップの crate の名前の一覧は持たない（チップの crate は期待の一覧に入らないので違反として出る）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/core-deps-check.sh`（bash + `python3 -I`、標準ライブラリのみ） | `scripts/core-deps-check.sh` |
| 呼び出し元 | `make core-deps-check`（直接実行も可）。CI のジョブからはまだ呼ばない | `grep -n core-deps .github/workflows/ci.yml` |
| 入力 | `cargo metadata --locked --offline --no-deps --format-version 1 --all-features` | 同スクリプトの `METADATA_CMD` |
| 回帰テスト | `scripts/test-core-deps-check.sh`（`make test-core-deps-check`）。合成 metadata・偽のコマンド・PATH を絞った環境・一時 workspace の実物の出力で確かめる。リポジトリのファイルを変更しない・ネットワーク不要。CI のジョブからはまだ呼ばない | `make test-core-deps-check` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 使い方と終了コード

| 呼び方 | 動作 | 確かめ方 |
| ------ | ---- | -------- |
| 引数なし | `cargo metadata` を流して検査する。commit は HEAD（変更が在れば末尾に `-dirty`） | 直接実行 |
| `--metadata-json <path>` | 渡した metadata だけを検査する（cargo・git を呼ばない。commit は `-`）。1 つだけ受け付ける | 回帰テストの合成 metadata の節 |

| コード | 意味 | 確かめ方 |
| ------ | ---- | -------- |
| 0 | 違反なし | 直接実行して `echo $?` |
| 1 | 外部の crate の余り・不足、または feature の転送の違反 | 回帰テストの違反のケース |
| 2 | 誤用・前提の不足（python3・cargo・git）・読み切れない入力・対象の crate が一意に定まらない・期待の一覧の定義の自己検査の失敗 | 回帰テストの誤用のケース |

## 照合の規則

| 軸 | 内容 | 確かめ方 |
| -- | ---- | -------- |
| (A) 依存の宣言 | 共通の crate の宣言（package 名。target 付き・optional を含み、重複は除く。workspace の member への依存も数える）を通常・build・dev ごとに期待の一覧と比べ、余り（`unexpected`）も不足（`missing`）も違反にする | 回帰テストの `bad_*` |
| (B) feature の転送 | 共通の crate の feature F ごとに、契約の crate に同じ名前の F があり、その値に `<依存のキー>/F` があること。別名から転送している場合（`renamed`）、共通の crate に `default` がある場合も違反。契約の crate の `backend-*` など対応の無い feature は対象にしない。依存のキーは `rename` があればそれ | 回帰テストの `bad_*`・`ok_rename` |

## 出力の読み方

件数の値は変わるので書かない。出る場所と見方だけを示す。

| 出力 | 場所 | 見方 | 確かめ方 |
| ---- | ---- | ---- | -------- |
| `scanned:` | 標準出力 | 依存側（`check=core-deps`）と feature 側（`check=feature-forward`）の 2 行 | `scripts/core-deps-check.sh \| grep '^scanned:'` |
| `record:` | 標準出力（終了コード 0 と 1 のとき） | commit・期待と宣言の通常の依存の一覧・余りと不足の件数・build と dev の宣言の件数・共通の crate の feature の数と転送の数・feature の違反の件数 | `scripts/core-deps-check.sh \| grep '^record:'` |
| `ok:` | 標準出力 | 合格時の 1 行 | 直接実行 |
| `violation:` / `FAILED:` | 標準エラー | 違反した依存の名前・種別・理由・target、または feature の名前・理由。違反の合計 | 回帰テストの違反のケース |
| `error:` | 標準エラー | 終了コード 2 の理由 | 回帰テストの誤用のケース |

## 数え方の定義の所在

期待の一覧は、スクリプトの Python 部の先頭の「数え方の定義」ブロックにだけ置く。値は本書に写さず、ブロックを正とする。
期待の一覧を変えるのは、依存を足す PR の中だけとする（承認の記録 `deps-approved.toml` と同じ PR。[dependency-policy](../.claude/rules/dependency-policy.md)）。

| 軸 | 定義の定数 | 確かめ方 |
| -- | ---------- | -------- |
| 通常の依存の期待の一覧 | `EXPECTED_NORMAL`（上限は `PERMITTED_NORMAL`。部分集合でなければ終了コード 2） | `grep -n EXPECTED_NORMAL scripts/core-deps-check.sh` |
| build・dev の依存の期待の一覧 | `EXPECTED_BUILD`・`EXPECTED_DEV` | `grep -n EXPECTED_ scripts/core-deps-check.sh` |
| 共通・契約の crate の manifest | `CORE_MANIFEST_REL`・`CONTRACT_MANIFEST_REL` | `grep -n MANIFEST_REL scripts/core-deps-check.sh` |

## 回帰テストが確かめること

| 区分 | 内容 |
| ---- | ---- |
| 合格 | 実物と同じ形の metadata。manifest_path の表記ゆれ。契約の crate の対応の無い feature。契約の crate の依存の `rename`。共通の crate 以外の依存は検査しない範囲 |
| 依存の違反 | 外部の crate を 1 つ足すと落ちる（任意の名前・チップの crate の名前・通常・build・dev・target 付き・member への依存・重複は 1 件） |
| feature の違反 | 契約の crate に同じ名前が無い・転送されない・別名から転送される（`?/` の形を含む）・`default` がある |
| 誤用 | 壊れた JSON・metadata、対象の crate が 0 件または 2 件、契約の crate の共通の crate への依存が 0 件または 2 件、数えない kind、`--metadata-json` の二重指定、cargo・git・python3 の不足 |
| 一時 workspace の実物 | 実物の `cargo metadata` で合格。workspace の外の path crate を共通の crate に足すと落ちる |
| 実物のリポジトリ | 引数なしで合格し `record:` が出る |
