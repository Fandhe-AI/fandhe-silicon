# 公開 API に wgpu の型が無いことの検査（TASK-10）の所在と現状の事実

製品の workspace の各 crate の公開 API に wgpu 系の型・経路が出ていないことを rustdoc の JSON から機械で確かめる検査について、本リポのスクリプトと回帰テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-10。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査 | `scripts/public-api-check.sh`（bash + `python3 -I`、標準ライブラリのみ）。入口は `make public-api-check` | `scripts/public-api-check.sh` |
| 回帰テスト | `scripts/test-public-api-check.sh`。入口は `make test-public-api-check`。合成 JSON の節と、一時 workspace で実物の rustdoc JSON を作る節がある | `scripts/test-public-api-check.sh` |
| CI | `ci.yml` の `deny` ジョブが検査と回帰テストの両方を呼ぶ。`scanned:` / `record:` の行はジョブのログに残る | `.github/workflows/ci.yml` |
| 出力先 | `cargo metadata` の `target_directory` 配下の専用ディレクトリ。ソースを変更せず、ネットワークを使わない | `scripts/public-api-check.sh` の先頭コメント |
| toolchain | `rust-toolchain.toml` のまま。`RUSTC_BOOTSTRAP` は検査の rustdoc 呼び出しとその事前確認の子プロセスにだけ渡す | 同スクリプトの先頭コメント |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 検査のスクリプト

使い方:

| 呼び方 | 動作 |
| ------ | ---- |
| 引数なし | workspace の lib crate ごとに 2 組（production / all-features）の rustdoc JSON を作って検査する |
| `--json <path>...` | 渡した JSON だけを検査する（cargo を呼ばない） |
| `--manifest-path <path>` | `<path>`（リポジトリ内の `Cargo.toml`）の workspace の lib crate に同じ検査を当てる（例: `baseline-wgpu/Cargo.toml`）。`--json`・`--emit-json` とは併用できない。`make public-api-check` が呼ぶのは製品の workspace だけ |

検査の経路（引数なし・`--manifest-path`）だけ、rustdoc に `--document-hidden-items` を渡して `#[doc(hidden)]` の公開アイテムも走査する（既定の JSON には入らないため。TASK-136 のポインタ）。`--emit-json` の経路は渡さず、emit を共有する他の検査の入力は変えない。対応するケースは `real_hidden_*`・`build_hidden_flag`・`emit_no_hidden_flag`・`real_emit_json_no_hidden`。

production の組で contract に渡す feature 列は、スクリプトの `CONTRACT_PRODUCTION` を正とする（本書には列挙しない）。

`--emit-json <dir> --without-feature <名前>` は、契約の production の feature 列から名前を除いた組を lib crate ごとに 1 組だけ作る opt-in の口（ラベル `without-<名前>`。誤用は終了コード 2）。使うのは [public-api-webgpu-check](public-api-webgpu-check.md)。対応する回帰テストのケースは `scripts/test-public-api-check.sh` の `without_*`・`real_without_no_contract`。

終了コード:

| コード | 意味 |
| ------ | ---- |
| 0 | wgpu 系の参照が 0 件 |
| 1 | 参照が 1 件以上（違反） |
| 2 | 誤用・前提の不足・JSON を読み切れない（成功扱いにしない） |

出力:

- 標準出力: 組ごとの `scanned:`、全部の文書を検査し終えたとき（終了コード 0 と 1）だけ最後に 1 回出す `record:`、合格時の `ok:`
- 標準エラー: `violation:`・`FAILED:`・`error:`
- ホストの絶対パス・環境変数の値は出さない

数える段の規則はスクリプトの先頭コメントの V1〜V7 を正とする（本書には写さない）。件数は出すだけで合否に使わない。公開アイテムが 0 件のときは終了コード 2。

### 検査しないもの

- 件数の増減
- wasm32 の公開 API
- 依存グラフ（`make deny` の役割）
- 陽性対照
- wgpu 系以外の名前

## 振る舞いとテストの対応

テストは `make test-public-api-check` のケース名（`scripts/test-public-api-check.sh`）。判定は終了コード + 標準出力と標準エラーの部分一致。

| 振る舞い | ケース |
| -------- | ------ |
| 参照が 0 件なら 0 で終わる。内部・wgpu 系でない外部の参照は数えない | `ok_noref`・`ok_paths_internal`・`ok_index_internal`・`ok_ext_nonwgpu`・`ok_index_ext_nonwgpu`・`ok_use_null`・`ok_seq_internal`・`ok_tuple_null` |
| 名前の判定は区切りを見る（`my_wgpu`・`nagano` は当たらない） | `ok_name_my_wgpu`・`ok_name_nagano` |
| 公開アイテムとして数えるのは公開と既定の可視性だけ | `count_visibility` |
| 型の参照で 1 で終わる | `bad_type` |
| trait・use・index・並びの参照で 1 で終わる | `bad_trait`・`bad_use`・`bad_index_crate`・`bad_unref_index`・`bad_seq_wgpu` |
| wgpu 系の名前の各形で 1 で終わる | `bad_name_0` から `bad_name_5` |
| 読めない・形の崩れた JSON、解決できない参照は 2 で終わる（成功扱いにしない） | `err_*` の各ケース・`notjson`・`missing_path`・`second_unreadable` |
| 公開アイテムが 0 件なら 2 で終わる | `err_no_public` |
| 誤用は 2 で終わる | `unknown_arg`・`json_without_path` |
| 複数本を渡すと組ごとに `scanned:` を出し、片方が違反なら全体は 1 | `two_files_one_bad` |
| `record:` は全部の文書を検査し終えたときだけ、最後の `scanned:` の後に 1 回出る。合計の値が合う | `record_last_on_violation`・`record_before_ok`・`two_files_totals` |
| feature の組ごとに JSON を取り違えない（片方の組だけの違反を見逃さない） | `build_prod_only_bad`・`build_all_only_bad`・`build_both_ok`・`build_counts_per_set` |
| コミットが取れなければ JSON を作る前に 2 で止まる。変更が在れば `-dirty` が付く | `commit_fail-rev`・`commit_fail-status`・`commit_garbage`・`build_dirty` |
| 実物の rustdoc JSON で参照 0 件なら通る | `real_clean`・`real_plain_dep` |
| 実物の rustdoc JSON で 1 件（feature の裏だけを含む）あれば落ちる | `real_type_leak_feature`・`real_type_leak_feature_production_clean`・`real_type_leak` |
| 再エクスポートも落ちる | `real_reexport` |
| `#[doc(hidden)]` の公開アイテム（アイテム・モジュール）の参照も走査し、1 件あれば落ちる。参照が無ければ通る | `real_hidden_erased_clean`・`real_hidden_leak`・`real_hidden_module_leak` |
| 隠したアイテムの走査は検査の経路だけ（rustdoc の引数と `--emit-json` の JSON で確かめる） | `build_hidden_flag`・`emit_no_hidden_flag`・`real_emit_json_no_hidden` |

回帰テストは一時 workspace の合成 crate で落ちることを確かめる。製品の workspace に陽性対照は置かない（TASK-12 のポインタ）。

## 確かめ方

worktree のルートで実行する。

| 確かめること | コマンド |
| ------------ | -------- |
| crate ごとに 2 組の `scanned:` の行と、`record:`・`ok:` の行が出る | `make public-api-check` |
| 回帰テストが通る | `make test-public-api-check` |
| CI のログに走査の行が残る | `deny` ジョブのログで `scanned:` / `record:` を探す（例: `gh run view <id> --log \| grep -E 'scanned: crate\|record: commit'`） |
| 文書と編集ファイルの書式 | `make check` |

件数（`public_items` の値）は変わるので本書には書かない。出る場所と見方だけを示す。

## 関連

- [public-api-contract-check](public-api-contract-check.md)・[public-name-check](public-name-check.md)・[ci-device-free-jobs](ci-device-free-jobs.md)・[ci-job-split](ci-job-split.md)
- [baseline-wgpu](baseline-wgpu.md)・[baseline-positive-control](baseline-positive-control.md)・[workspace-boundary](workspace-boundary.md)・[measurement-record](measurement-record.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)
