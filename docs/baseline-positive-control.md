# 比較基準への陽性対照（TASK-12）の所在と現状の事実

比較基準の別 workspace（`baseline-wgpu/`）に、製品向けの 2 つの検査を当てて「wgpu を持つ側で検査が 1 件以上を検出できる」ことを確かめる陽性対照について、本リポのスクリプトと回帰テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-12。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 本体 | `scripts/baseline-positive-control.sh`（bash + `python3 -I`、標準ライブラリのみ。外部依存は足していない）。入口は `make baseline-positive-control` | `scripts/baseline-positive-control.sh` |
| 当てる検査 | `scripts/dep-graph-wgpu-check.sh --manifest-path baseline-wgpu/Cargo.toml --allow-metadata-superset` と `scripts/public-api-check.sh --manifest-path baseline-wgpu/Cargo.toml` | 同スクリプトの先頭コメント |
| CI | `ci.yml` の `deny` ジョブで `make deny` の後のステップとして流す。`make deny` には含めない（`make deny` は製品の workspace だけを見る） | `grep -n baseline-positive-control Makefile .github/workflows/ci.yml` |
| 回帰テスト | `scripts/test-baseline-positive-control.sh`。入口は `make test-baseline-positive-control` | `scripts/test-baseline-positive-control.sh` |

## 使い方と終了コード

引数は取らない（未知の引数は 2）。2 つの検査は、片方が失敗しても止めずに両方流す。

| 終了コード | 意味 | 確かめ方 |
| ---------- | ---- | -------- |
| 0 | 3 つの定義（narrow・wide・public-api）すべてで 1 件以上を検出 | `make baseline-positive-control` |
| 1 | どれかの定義が 0 件 | `all_zero`・`narrow_only` など（下の対応表） |
| 2 | 誤用、前提ツールの不足、内側の終了コードが 0・1 以外、`record:` の行の崩れ、内側の終了コードと件数の食い違い。2 は 1 に勝つ | `error_beats_zero` |

## 出力の読み方

件数（値）は変わるので本書には書かない。出る場所と見方だけを示す。

| 出力 | 場所 | 見方 | 確かめ方 |
| ---- | ---- | ---- | -------- |
| `scanned:`・`record:` | 標準出力 | 内側の 2 検査の行をそのまま流し直したもの。件数を表す `record:` はこの 2 行で、CI の `deny` ジョブのログに残る。鍵の意味は [dep-graph-wgpu-check.md](./dep-graph-wgpu-check.md)・[public-api-wgpu-check.md](./public-api-wgpu-check.md) | `gh run view <id> --log` の出力を `grep 'record: commit'` |
| `violation:`・`FAILED:` | 標準エラー | 内側の検査の違反。陽性対照では出るのが正常 | `make baseline-positive-control` |
| `positive-control: check=… definition=… count=<n> result=detected` | 標準出力 | 定義ごとに 1 行。`<n>` は narrow が `narrow_total`、wide が `wide_total`、public-api が `wgpu_type_refs_total` と `wgpu_other_refs_total` の和 | `make baseline-positive-control` |
| `positive-control-failed: … result=no-detection` | 標準エラー | その定義が 0 件 | `all_zero_three_failures` |
| `positive-control-failed: … result=unreadable-record`・`inconsistent`・`error` | 標準エラー | 読めない入力や想定外の終了（終了コード 2） | `dep_record_<mode>`・`api_error` |
| `ok: positive control detected wgpu in baseline-wgpu (narrow, wide, public-api)` | 標準出力 | 3 定義すべてで検出したときだけ最後に 1 行 | `make baseline-positive-control` |

包み自身は `record:` の行を出さない。件数を保存するファイルも作らない。件数はジョブのログの標準出力に残すだけである。

## 振る舞いとテストの対応

判定は終了コードと出力の部分一致で行う。ケース名は `scripts/test-baseline-positive-control.sh` のもの。

| 振る舞い | ケース |
| -------- | ------ |
| 全定義で検出なら 0。定義ごとに 1 行 | `all_detected`・`all_detected_three_lines` |
| 件数は特定の値と比べない | `counts_not_compared` |
| public-api は 2 つの鍵の和で判定する | `public_api_other_only`・`public_api_type_only` |
| 0 件の定義があれば 1 | `all_zero`・`all_zero_three_failures`・`narrow_only`・`wide_only`・`public_api_zero`・`dep_zero_api_detected` |
| `record:` の行の崩れは 2 | `dep_record_<mode>`・`api_record_<mode>` |
| 内側のエラーや想定外の終了は 2 | `api_error`・`dep_error`・`dep_unexpected` |
| 2 は 1 に勝つ | `error_beats_zero` |
| 内側の終了コードと件数の食い違いは 2 | `inner_one_but_all_zero`・`inner_one_but_api_zero`・`inner_zero_but_count` |
| 誤用とツールの不足は 2 | `unknown_arg`・`no_cargo` |
| 一時 workspace の実物の出力で通す | `real_all_detected`・`real_all_detected_three_lines`・`real_none`・`real_none_three_failures`・`real_wide_only`・`real_not_exposed` |

確かめ方は `make test-baseline-positive-control`。

## 検査しないもの

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 件数の厳密な一致 | 見ない（TASK-255 のポインタのみ） | 同スクリプトの先頭コメントの「限界」 |
| 製品の workspace | 対象外（`make deny` / `make public-api-check` の役割） | `scripts/deny.sh` |
| 件数の保存 | しない（標準出力のみ） | `git status` でファイルが増えないこと |

## 確かめ方

- `make baseline-positive-control`: `positive-control:` が 3 行、続けて `ok:` が出て終了コード 0
- `make test-baseline-positive-control`: 回帰テスト
- `make check`: 書式

## 関連

- [dep-graph-wgpu-check.md](./dep-graph-wgpu-check.md)・[public-api-wgpu-check.md](./public-api-wgpu-check.md)・[baseline-wgpu.md](./baseline-wgpu.md)・[ci-device-free-jobs.md](./ci-device-free-jobs.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)
