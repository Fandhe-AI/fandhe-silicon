# 公開 API の検査（TASK-241）の所在と現状の事実

本リポのコードと公開テストから観測できる事実だけを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-241。関連: BUILD-29・BUILD-31・BUILD-32。ポインタのみ）。本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 現状と前提の確認（1/6）（TASK-241）

1/6（#3952）で、コードと公開テストから観測できる事実だけを記録した。前提・完了の条件の正は private spec 側の記録に従う（ポインタのみ）。コードは変えていない。

### 前提

| 前提 | 本リポの物 | 確かめ方 |
| ---- | ---------- | -------- |
| TASK-10 | `scripts/public-api-check.sh`（[public-api-wgpu-check](public-api-wgpu-check.md)） | `gh issue view 363 --json state`（CLOSED） |
| TASK-22 | `wasm/`（[wasm32-node-foundation](wasm32-node-foundation.md)） | `gh issue view 368 --json state`（CLOSED） |
| TASK-37 | 公開側に文書は無い。記録は private spec 側（ポインタのみ） | `gh issue view 168 --json state`（CLOSED） |

### 現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 公開 API の抽出の口 | `scripts/public-api-check.sh --emit-json <dir>` は `{lib}.{組}.json` を写すだけで検査しない | スクリプトの先頭コメント |
| CI | `deny` ジョブが `make public-api-check` を呼ぶ | `grep -n 'public-api-check' .github/workflows/ci.yml` |
| `make public-api-check` | 終了コード 0 で、`ok:` の行が出た（件数の値は時点で変わるので書かない） | `make public-api-check` |
| パイプライン生成の入力の型 | `PipelineInput` は `#[non_exhaustive]` の enum（形は [artifact-input-types](artifact-input-types.md)） | `grep -n -B1 'pub enum PipelineInput' crates/core/src/execution.rs` |
| 契約の crate の公開面 | feature で切り替わる backend のモジュールだけ | `grep -n '^pub' crates/contract/src/lib.rs` |
| 契約の crate の feature | `[features]` を正とする（本書に列挙しない） | `sed -n '/\[features\]/,/^$/p' crates/contract/Cargo.toml` |

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/6 | 現状と前提の確認 | #3952 |
| 2/6 | 実装 | #3953 |
| 3/6 | テスト | #3954 |
| 4/6 | 実装 | #3955 |
| 5/6 | 実装 | #3956 |
| 6/6 | CI への組み込み | #3957 |

後の段で足す検査の振る舞い・試験は、その PR のコードと公開テストが示し、その PR で本書に節を足す。守る規約は [coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)。

確かめた base は `b0f3982ea9a037bce40d63b6729a33ecbcc6ea07`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。

## (1) の検査の実装（2/6）（TASK-241）

内容の (1) の機械検査を入れた。コードと公開テストから観測できる事実だけを記録する。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 検査本体 | `scripts/public-api-webgpu-check.sh --config public-api-webgpu-check.conf`（`make public-api-webgpu-check`）。CI にはまだ組み込まない | スクリプトの先頭コメント |
| 抽出 | `public-api-check.sh --emit-json <dir> --without-feature native-blocking-wait` を共有する。対象は core と contract の 2 本で、片方が欠けたら終了コード 2 | `scripts/test-public-api-check.sh` の `without_*` |
| 数えるもの | 公開の関数・メソッド・再公開のうち、名前を `_` で区切った語が語の一覧の語に等しいもの（部分一致では当てない）。参照先が外部 crate または解決できない glob の再公開は、名前を照合できないので終了コード 2 で止める | スクリプトの先頭コメント |
| 語の一覧 | `public-api-webgpu-check.conf` の節 `[blocking-words]` | 同ファイルの先頭 |
| 終了コード | 0 一致なし・1 一致あり・2 誤用または前提の不足または読み切れない入力 | スクリプトの先頭コメント |
| 検査しないもの | 語を含まない名前、非ブロッキングの形の有無、exec・upper、他の feature の組 | スクリプトの先頭コメント |
| 現状 | 手元で `make public-api-webgpu-check` が終了コード 0（件数の値は時点で変わるので書かない） | `make public-api-webgpu-check` |

振る舞いとテストの対応は、次の節（3/6）にまとめた。

## (1) の検査の試験（3/6）（TASK-241）

(1) の検査の回帰テストを入れた。コードと公開テストから観測できる事実だけを記録する。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `scripts/test-public-api-webgpu-check.sh`（`make test-public-api-webgpu-check`）。前提は python3・cargo・git で、無ければ失敗する（skip しない）。CI のジョブからは呼ばず、手元で流す | スクリプトの先頭コメント |
| 構成 | 前半は合成 JSON を `--json` で渡す経路（cargo・git を呼ばない）。後半は package 名が `fandhe-silicon-core`・`fandhe-silicon-contract` の依存なしの一時 workspace に検査本体・`public-api-check.sh`・`lib.sh` を写し、実物の rustdoc の JSON で既定の経路を通す | 同上 |
| 副作用 | リポジトリのファイルを変更しない・ネットワーク不要（一時ディレクトリは終了時に消す） | 同上 |
| 判定 | 終了コード + 標準出力 + 標準エラーの部分一致。終了コード 2 のケースは `ok:`・`record:` が無いことも見る | `check` 関数 |
| 確かめ方 | 件数の値は時点で変わるので書かない | `make test-public-api-webgpu-check` |

### 振る舞いとテストの対応

| 振る舞い | テストのケース |
| -------- | -------------- |
| 語の照合は `_` 区切りの語の等号で、大文字小文字を区別せず、部分一致では当てない | 「大文字小文字を区別しない」「部分一致には当てない」 |
| 数えるのは公開の関数・メソッド・再公開だけ（非公開・他 crate・関数でも再公開でもないものは数えない） | 「非公開・他 crate の関数は数えない」「関数でも再公開でもないアイテムは数えない」「trait の impl のメソッド（default）も当たる」 |
| 再公開は 1 件を 2 回数えず、name と use.name が別の語なら両方数える | 「name と use.name が同じ再公開は 1 回」「name と use.name が別の語なら両方数える」 |
| 違反は終了コード 1 で `violation:`・`FAILED:` を出し、`ok:` を出さない。複数 JSON は合計する | 「自由関数の語の一致」「FAILED の行」「複数 JSON の合計」 |
| 出力の値は無害化する | 「違反の出力の値の無害化」 |
| 設定・JSON・glob の再公開の誤用は終了コード 2 で、締めの行を出さない | 「未知の節」から「解決できない glob の再公開」までの終了コード 2 の各ケース |
| 実物の設定の各語が当たる | 「実物の語が当たる」（語は設定から実行時に作る） |
| feature `native-blocking-wait` を外した構成では、その feature の裏にある語の名前の関数は 0 件 | `real_feature_gated` |
| 同じ関数を feature の裏から出すと終了コード 1 になる（違反を 1 件足すと落ちる）。core・メソッド・再公開も対象 | `real_contract_fn`・`real_core_fn`・`real_method`・`real_reexport` |
| 作業ツリーに変更があれば commit に `-dirty` が付く。ホストの絶対パスを標準出力に出さない | `real_clean_commit`・`real_feature_gated_dirty`・`real` 関数の判定 |
| JSON を作る段の失敗・契約の crate が無い workspace・core と contract がそろわない出力・重複・commit が取れない場合は終了コード 2 | `real_build_fail`・`real_no_contract`・`real_no_emit`・`real_core_only`・`real_duplicate_emit`・`real_emit_exit1`・`real_no_commit` |
| 一時 workspace の準備に失敗したら失敗として数える | `real_fixture_setup` |

