# 公開 API のチップの SDK の検査（TASK-13）の所在と現状の事実

公開 API の型・経路と、共通の crate の依存宣言に、チップの SDK の crate（`objc2-metal`・`ash`・`cudarc`）が出ていないことを数える検査について、本リポのスクリプトとテストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-13。関連: BUILD-51・BUILD-54・U-30。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/public-api-sdk-check.sh`（bash + `python3 -I`、標準ライブラリのみ）。検査の 2 側（rustdoc の JSON の参照の数え上げ・共通の crate の依存宣言）を 1 回で流す | `scripts/public-api-sdk-check.sh` |
| 呼び出し元 | `make public-api-sdk-check`（直接実行も可）。CI の `deny` ジョブが呼ぶ（`scanned:` / `record:` の行がジョブのログに残る） | `grep -n public-api-sdk .github/workflows/ci.yml` |
| rustdoc の JSON | `scripts/public-api-check.sh --emit-json` で作る（抽出の実体を共有する） | 同スクリプトの先頭コメント |
| 依存側の入力 | `cargo metadata --locked --offline --no-deps --format-version 1 --all-features` | 同スクリプトの `METADATA_CMD` |
| 回帰テスト | `scripts/test-public-api-sdk-check.sh`（`make test-public-api-sdk-check`）。合成 rustdoc JSON・合成 metadata・偽のコマンド・PATH を絞った環境・一時 workspace の実物の出力で確かめる。リポジトリのファイルを変更しない・ネットワーク不要。CI の `deny` ジョブも呼ぶ | `make test-public-api-sdk-check` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 使い方と終了コード

| 呼び方 | 動作 | 確かめ方 |
| ------ | ---- | -------- |
| 引数なし | rustdoc の JSON を作って数え、`cargo metadata` も流して両側を検査する | 直接実行 |
| `--json <path>...` | 渡した JSON だけを検査する（依存側は流さない。cargo・git を呼ばない） | 回帰テストの「部分の実行」 |
| `--metadata-json <path>` | 渡した metadata だけを検査する（rustdoc 側は流さない。cargo・git を呼ばない）。1 つだけ受け付ける | 回帰テストの「部分の実行」 |
| 両方 | 渡した 2 つだけを検査する（cargo・git を呼ばない） | 回帰テストの「部分の実行」 |

| コード | 意味 | 確かめ方 |
| ------ | ---- | -------- |
| 0 | 実行した側の参照・依存が 0 件 | 直接実行して `echo $?` |
| 1 | どちらかで 1 件以上（違反） | 回帰テストの違反のケース |
| 2 | 誤用・前提の不足（python3・cargo・git）・HEAD のコミットが取れない・読み切れない入力・JSON を作る段の失敗・共通の crate が一意に定まらない | 回帰テストの誤用のケース |

## 出力の読み方

件数の値は変わるので書かない。出る場所と見方だけを示す。

| 出力 | 場所 | 見方 | 確かめ方 |
| ---- | ---- | ---- | -------- |
| `scanned:` | 標準出力 | JSON ごとに 1 行（参照の件数と、公開 API から解決された外部 crate の一覧）と、依存側の 1 行 | `scripts/public-api-sdk-check.sh \| grep '^scanned:'` |
| `record:` | 標準出力（終了コード 0 と 1 のとき） | 件数の合計とコミット。実行しなかった側は `-`（部分の実行が全部の合格に読めない） | `scripts/public-api-sdk-check.sh \| grep '^record:'` |
| `ok:` | 標準出力 | 合格時の 1 行 | 直接実行 |
| `violation:` / `FAILED:` | 標準エラー | 違反した crate・アイテム・種別（type / other）・参照先、または共通の crate の依存の名前・種別・target。違反の合計 | 回帰テストの違反のケース |
| `error:` | 標準エラー | 終了コード 2 の理由 | 回帰テストの誤用のケース |

## 数え方の定義の所在

数え方は、スクリプトの Python 部の先頭の「数え方の定義」ブロックにだけ置く。値は本書に写さず、ブロックを正とする。

| 軸 | 定義の定数 | 確かめ方 |
| -- | ---------- | -------- |
| 対象の crate 名（小文字・`-` を `_` にそろえて完全一致。接頭辞には広げない） | `CHIP_SDK` | `grep -n CHIP_SDK scripts/public-api-sdk-check.sh` |
| 共通の crate の manifest | `COMMON_MANIFEST_REL` | `grep -n COMMON_MANIFEST_REL scripts/public-api-sdk-check.sh` |
| 数える依存の種類 | `DEP_KINDS` | `grep -n DEP_KINDS scripts/public-api-sdk-check.sh` |

## 回帰テストが確かめること

| 区分 | 内容 |
| ---- | ---- |
| 合格 | 参照なし・内部の参照・チップでない外部 crate（一覧は重複なしの昇順）。完全一致の陰性（`ash_window`・`my_ash`・`objc2`・`cudarc_sys`・`objc2-metal-ext` などは数えない） |
| 違反 | 3 つの名前それぞれと大小文字・`-`/`_` の揺れ。型は type、トレイトオブジェクト・再公開・index の外部アイテムは other。依存側は通常・build・dev、target 付き、optional |
| 誤用 | 壊れた JSON・metadata、公開アイテム 0 件、共通の crate が 0 件または 2 件、数えない kind、`--metadata-json` の二重指定、cargo・git・python3 の不足 |
| 一時 workspace の実物 | 型・再公開・feature の裏の漏れ・共通の crate の通常・dev・target 付き build・optional の依存。共通の crate 以外の依存は検査しない範囲 |
