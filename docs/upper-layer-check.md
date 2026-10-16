# 上の層の検査（TASK-15）の所在と現状の事実

上の層の crate（`crates/upper`）と測定の workspace（`measure/`）に、4 つの検査を当てるスクリプトについて、本リポのスクリプトとテストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-15。関連: BUILD-56・BUILD-57・BUILD-58・TASK-6。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/upper-layer-check.sh`（bash + `python3 -I`、標準ライブラリのみ） | `scripts/upper-layer-check.sh` |
| 呼び出し元 | `make upper-layer-check`（単体）と、CI の `deny` ジョブのステップ。`make deny`（`scripts/deny.sh`）には含めない。違反も前提の不足も失敗にし、握りつぶさない | `grep -n "run: make upper-layer-check" .github/workflows/ci.yml` |
| 対象 | 上の層の crate（`crates/upper`）と、測定の workspace（`measure/`）の member。入れている検査は 4 つ（unsafe の不在・チップの crate の不在・直接依存の許可リスト・測定のビルドで検証用の feature が有効でないこと） | 同スクリプトの先頭コメント |
| 入力 | 引数なしは `cargo metadata --locked --offline --no-deps --all-features`。または `--metadata-json` と `--measure-metadata-json` の組で渡した JSON | 同スクリプトの先頭コメント |
| 回帰テスト | `scripts/test-upper-layer-check.sh`（`make test-upper-layer-check`）。合成の入力、ツールが無いとき、偽の cargo / git、一時 workspace の実物の cargo metadata、実物のリポジトリの合格を確かめる。リポジトリのファイルを変更しない・ネットワーク不要 | `make test-upper-layer-check` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 使い方と終了コード

| 呼び方 | 動作 | 確かめ方 |
| ------ | ---- | -------- |
| 引数なし | cargo を流して数える | 直接実行 |
| `--metadata-json <path> --measure-metadata-json <path>` | 渡した 2 つの JSON だけを読む（cargo・git を呼ばない）。片方だけの指定は誤用 | テストの `args-product-only`・`args-measure-only` |

| コード | 意味 | 確かめ方 |
| ------ | ---- | -------- |
| 0 | 合格 | 直接実行して `echo $?` |
| 1 | 違反あり | 同スクリプトの先頭コメント |
| 2 | 誤用・前提の不足（python3・cargo・git が無い、metadata が JSON でない・形が崩れている、crate が見つからない、`.rs` が 0 件、閉じていないコメント・文字列、シンボリックリンク、想定外の例外など） | 同スクリプトの先頭コメント |

## 出力の読み方

件数の値は変わるので書かない。出る場所と見方だけを示す。

| 出力 | 場所 | 見方 |
| ---- | ---- | ---- |
| `scanned:` | 標準出力 | 読んだ入力ごとに 1 行 |
| `record:` | 標準出力（終了コード 0 と 1 のとき） | `commit`・`rs_files`・`direct_deps`・`forbid`・`unsafe_total`・`chip_deps`・`chip_paths`・`other_deps`・`measure_pkgs`・`measure_contract_pkgs`・`measure_forbidden` |
| `ok:` | 標準出力（合格のとき） | 1 行 |
| `violation: check=<forbid\|unsafe\|chip-dep\|chip-path\|dep-allow\|measure-feature>` | 標準エラー | 違反 1 件ごとに 1 行。パスは crate ディレクトリからの相対 |
| `FAILED:` | 標準エラー | 違反の件数 |
| `error:` | 標準エラー | 誤用・前提の不足の理由 |

## 数え方の定義の所在

対象の位置、チップの crate の名前、許可の名前、禁止の feature、依存の種類、名前の折りたたみは、Python 部の先頭の「数え方の定義」ブロックの 1 か所にだけある。値は本書へ書き写さない。変えるときはそのブロックを直し、`scripts/test-upper-layer-check.sh` の対応するケースを同じ変更で直す。

## 振る舞いとテストの対応

ケース名は `scripts/test-upper-layer-check.sh` に実在するものだけを挙げる。

| 振る舞い | テストの節・ケース |
| -------- | ------------------ |
| unsafe の不在（forbid の宣言・forbid が届かない位置の unsafe） | 節 1 の `real-unsafe`・`real-unsafe-again`・`no-forbid-msg`・`unsafe-in-tests-msg`・`unsafe-in-build-rs-msg`・`unsafe-two-count` |
| チップの crate の不在（依存・ソース） | 節 1 の `chip-dep-*`・`chip-use-*`・`chip-path-*`・`comment-not-chip`・`string-not-chip` |
| 直接依存の許可リスト | 節 1 の `allow-ok`・`allow-extra-normal`・`allow-renamed-extra`・`allow-chip`・`allow-contract-normal`・`allow-contract-dev`・`allow-contract-build`・`allow-contract-with-extra`・`allow-contract-with-chip`・`allow-contract-chip-source`・`allow-renamed-contract-alias`・`allow-fold-contract-ok` |
| 測定の feature | 節 1 の `measure-default`・`measure-test-support`・`measure-fault-injection`・`measure-forward`・`measure-indirect` |
| 対象 0 件の合格 | 節 1 の `measure-no-contract`・`allow-zero-deps-count`・`measure-zero-contract-deps-count` |
| 終了コード 2 | 節 1 の `upper-missing`・`product-not-json`・`no-rs-files`・`symlink-rs`・`unterminated-comment`・`measure-not-json` |
| ツールが無いとき | 節 2 の `no_python3`・`no_cargo`・`no_git`・`unknown_option` |
| 偽のコマンド（引数なしの経路） | 節 3 |
| 一時 workspace の実物の cargo metadata | 節 4 |
| 実物のリポジトリ（`crates/upper` と `measure/`）の合格 | 節 5 |

## 検査しないもの

マクロが生成するパス、`include!`・`#[path]` で crate ディレクトリの外から読むファイル、`target/` と隠しディレクトリ、`package =` で改名した依存のソース側の別名、bin target の root の forbid、`cfg_attr` で包んだ forbid、件数の増減、測定の workspace の member でない path の crate を経由した間接の依存、resolve による feature の統一、`cfg` で絞った target 依存の除外は見ない（同スクリプトの先頭コメント）。ホストの絶対パスと環境変数の値は出力に出さない。ソースを変更しない。ネットワークを使わない。

## 確かめ方

| 確かめること | コマンド |
| ------------ | -------- |
| 検査が通る | `make upper-layer-check`（終了コード 0 と `ok:` の行） |
| 回帰テストが通る | `make test-upper-layer-check`（末尾が `passed=<n> failed=0`） |
| CI の枠を保つ | `make ci-frame-check` と `make test-ci-frame-check` |
| CI が呼んでいる | `grep -n "run: make upper-layer-check\|run: make test-upper-layer-check" .github/workflows/ci.yml` |

## 関連

- [ci-device-free-jobs](./ci-device-free-jobs.md)（`deny` ジョブの検査と make ターゲットの対応）
- [unsafe-policy](./unsafe-policy.md)
- [workspace-boundary](./workspace-boundary.md)
- [makefile](../.claude/rules/makefile.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
