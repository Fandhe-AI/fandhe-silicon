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
| 数えるもの | 公開の関数・メソッド・再公開のうち、名前を `_` で区切った語が語の一覧の語に等しいもの（部分一致では当てない） | スクリプトの先頭コメント |
| 語の一覧 | `public-api-webgpu-check.conf` の節 `[blocking-words]` | 同ファイルの先頭 |
| 終了コード | 0 一致なし・1 一致あり・2 誤用または前提の不足または読み切れない入力 | スクリプトの先頭コメント |
| 検査しないもの | 語を含まない名前、非ブロッキングの形の有無、exec・upper、他の feature の組 | スクリプトの先頭コメント |
| 現状 | 手元で `make public-api-webgpu-check` が終了コード 0（件数の値は時点で変わるので書かない） | `make public-api-webgpu-check` |

振る舞いとテストの対応の表は、テストを足す段で本書に足す。

