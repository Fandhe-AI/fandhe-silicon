# 公開 API の差分の道具（TASK-17）の所在と現状の事実

同じ crate の 2 つの版（比較基準の版と新しい版）の rustdoc の JSON から、公開 API の差分（足された項目・消えた項目）を機械で出す道具について、本リポのスクリプトと回帰テストから説明できる振る舞いと、比べる 2 つの JSON の作り方を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-17。ポインタのみ）。spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
道具の出力を spec 側でどう使うかは、private spec 側の記録に従う（ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 道具 | `scripts/public-api-diff.sh`（bash + `python3 -I`、Python 3.8 以上の標準ライブラリのみ）。比べる JSON のパスの引数が要るので make ターゲットは持たない | `scripts/public-api-diff.sh` |
| 回帰テスト | `scripts/test-public-api-diff.sh`。入口は `make test-public-api-diff`。合成 JSON（ダミーの名前）だけで動き、cargo・ネットワーク・`docs/spec` の取得は要らない | `scripts/test-public-api-diff.sh` |
| CI | `ci.yml` の `deny` ジョブが回帰テストを呼ぶ。道具の本体は呼ばない | `.github/workflows/ci.yml` |
| 入力の扱い | 読むだけ。ファイルを作らず、ネットワークを使わない | ケース `creates_no_files` |
| 依存 | 外部依存を足していない | `Cargo.lock` |

## 道具の流し方

```bash
scripts/public-api-diff.sh <比較基準の JSON> <新しい版の JSON>
```

引数の順を逆にすると、added と removed が入れ替わる。

終了コード:

| コード | 意味 |
| ------ | ---- |
| 0 | 差分なし |
| 1 | 差分あり（足された項目か消えた項目が 1 件以上） |
| 2 | 誤用・前提の不足・JSON を読み切れない・想定外の例外（成功扱いにしない） |

出力:

- 標準出力（終了コード 0 と 1）: 鍵で並べた決定的な行。`removed:` → `added:` → `summary:` の順。0 のときは続けて `ok:`。行の形の例（ダミーの名前）は `added: kind=function path=dummy::g`
- 標準エラー: 終了コード 1 で `DIFFERENT:`、2 で `error:`（誤用は `usage:`）
- 突き合わせの鍵は (kind, path)。id は版をまたいで安定しないので比べない。同じ path で kind が変わった項目は「消えた + 足された」
- path・kind の想定外の文字は `?` に置き換えて出す。引数のファイルパス・ホストの絶対パス・環境変数の値は出さない
- 入力の値はシェルに渡さず、`python3 -I` の中だけで扱う

数える項目と前提の規則は、スクリプトの先頭コメントの D1〜D6 を正とする（本書には写さない）。

### 比べないもの

- シグネチャ・型の変化
- impl の下にだけ在るメソッド・フィールド・関連定数
- 再公開（use）の別名
- doc の文言
- target 違いの API（wasm32 など）

## 比べる 2 つの JSON の作り方

2 本とも、worktree のルートから `--manifest-path` で `cargo rustdoc` を流す。

共通の前提:

- ルートから流す。rustup は作業ディレクトリの `rust-toolchain.toml` で toolchain を選ぶので、リポの外で流すと別の toolchain になりうる。toolchain が食い違うと `format_version` が食い違い、道具は終了コード 2 で止まる（ケース `format_version_mismatch`）
- `RUSTC_BOOTSTRAP=1` は、その 1 つの `cargo rustdoc` の呼び出しにだけ付ける（`export` しない）
- `--target-dir` は `mktemp -d` の一時ディレクトリにし、製品の `target/` や `baseline-wgpu/target/` と共有しない
- `--document-private-items` を付けない（付けると道具は終了コード 2 で止まる。ケース `includes_private_true`）
- 2 本の feature の組は揃える。食い違いは誤りではなく added / removed として出るので、道具では気付けない。比較基準は `baseline-wgpu/crates/smoke/Cargo.toml` の宣言どおり wgpu の既定の feature
- target は native だけ

比較基準の版（`baseline-wgpu/Cargo.lock` のまま作る。版の記録は [baseline-wgpu](baseline-wgpu.md)）:

```bash
RUSTC_BOOTSTRAP=1 cargo rustdoc --locked --manifest-path baseline-wgpu/Cargo.toml \
  -p wgpu --lib --target-dir "<一時ディレクトリ A>" \
  -- -Z unstable-options --output-format json
```

出力は `<一時ディレクトリ A>/doc/wgpu.json`。

新しい版:

1. `mktemp -d` で使い捨ての workspace を作る。`[workspace]` を持つ `Cargo.toml` 1 つと空の `src/lib.rs` で、依存は `wgpu = "=<新しい版>"`（feature は比較基準と同じ既定）
2. 使い捨ての workspace にはロックファイルが無く、`--locked` 付きの `cargo rustdoc` は生成を拒否される。先に `cargo generate-lockfile --manifest-path <使い捨ての workspace>/Cargo.toml` で作る（ロックファイルは使い捨ての workspace の中にだけできる）
3. 比較基準と同じ呼び出し（`--locked` 付き）を、`--manifest-path <使い捨ての workspace>/Cargo.toml` と別の一時ディレクトリ B で流す。出力は `<一時ディレクトリ B>/doc/wgpu.json`
4. 新しい版の取得（手順 2 のロックファイルの生成）だけがネットワークを使う
5. `git status` で、製品の workspace・`baseline-wgpu/`・どの `Cargo.lock` も変わっていないことを確かめる

比較基準の版の固定（`wgpu = "=30.0.1"`・`baseline-wgpu/Cargo.lock`）を変えるのは、依存の承認の流れ（[dependency-policy](../.claude/rules/dependency-policy.md)）を経るときだけで、この手順では変えない。
新しい版の値は本書に書かない。使い終えた一時ディレクトリと使い捨ての workspace は消す。

## 振る舞いとテストの対応

テストは `make test-public-api-diff` のケース名（`scripts/test-public-api-diff.sh`）。判定は終了コード + 標準出力と標準エラーの部分一致で、全ケース共通で引数のパスが出力に出ないことも確かめる。
前節の JSON の作り方は回帰テストを持たない手順なので、この表には含めない。

| 振る舞い | ケース |
| -------- | ------ |
| id を付け替えても・並びを入れ替えても差分なし | `same`・`renumbered`・`renumbered_rev` |
| 非公開・restricted・外部 crate・index に無い id は数えない | `private_added`・`restricted_added`・`external_added`・`path_unindexed_added` |
| 足された項目・消えた項目があれば 1 で終わる | `added`・`removed`・`added_and_removed` |
| kind の変化は「消えた + 足された」 | `kind_changed` |
| 引数の順を逆にすると added と removed が入れ替わる | `added_reversed` |
| 出力の順が決定的 | `output_order` |
| 制御文字は `?` に置き換える | `sanitized_path` |
| format_version・root の crate 名の食い違いは 2 で終わる | `format_version_mismatch`・`format_version_mismatch_rev`・`root_name_mismatch` |
| includes_private が true なら 2 で終わる | `includes_private_true`・`includes_private_true_first` |
| 形の崩れた JSON は 2 で終わる | `format_version_string`・`not_object`・`missing_index`・`missing_root`・`malformed_paths_entry`・`malformed_paths_path`・`malformed_paths_crate_id`・`root_without_name`・`root_foreign_crate` |
| 公開項目が 0 件なら 2 で終わる | `no_public_items`・`no_public_items_first` |
| 読めない入力は 2 で終わる | `notjson`・`notjson_first`・`badutf8`・`deep_nesting`・`missing_file`・`directory_as_input` |
| 誤用は 2 で終わる（未知のオプションの語を出さない） | `args_none`・`args_one`・`args_three`・`unknown_option` |
| ファイルを作らない | `creates_no_files` |

## 確かめ方

worktree のルートで実行する。

| 確かめること | コマンド |
| ------------ | -------- |
| 回帰テストが通る | `make test-public-api-diff` |
| CI で回帰テストが通る | `deny` ジョブのログで `make test-public-api-diff` の step を見る |
| 2 本の JSON での実行 | 前節の手順で作った 2 本を `scripts/public-api-diff.sh` に渡し、終了コードと行を見る |
| 文書と編集ファイルの書式 | `make check` |

## 関連

- [public-api-wgpu-check](public-api-wgpu-check.md)・[ci-device-free-jobs](ci-device-free-jobs.md)
- [baseline-wgpu](baseline-wgpu.md)・[wgpu-baseline-record](wgpu-baseline-record.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)
