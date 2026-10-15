# wasm32 のビルドと Node での照合の土台（TASK-22）の所在と本リポの現状

TASK-22（親 Issue #368）の記録の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 記録の正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-22）。本リポには内容・理由・経緯を置かない
- 前提のタスク（ポインタのみ）と、対応する本リポの物

| 前提 | 本リポの物 |
| ---- | ---------- |
| TASK-4 | ルート `Cargo.toml` の `members`・`exclude` と [workspace-boundary](workspace-boundary.md) |
| TASK-5 | `crates/contract/Cargo.toml` の `[features]`、`crates/contract/src/lib.rs` の `cfg`、`scripts/build-profiles.sh` |
| TASK-112 | [ci-device-free-jobs](ci-device-free-jobs.md)、`scripts/ci-frame-check.sh` |

- `docs/spec` は submodule（`update = none`）。指している版は `git ls-tree origin/main docs/spec` で確かめる

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| wasm32 の target | `rust-toolchain.toml` の `targets` は `wasm32-unknown-unknown` | `grep -n targets rust-toolchain.toml` |
| wasm32 の check | `scripts/build-profiles.sh` の `WASM_SETS` は 4 組。`fandhe-silicon-contract` を `cargo check --target wasm32-unknown-unknown --no-default-features` で確かめるだけ（build・link・test はしない）。target が無ければ skip せず失敗する | `grep -n -e WASM_SETS -e 'cargo check' -e 'not installed' scripts/build-profiles.sh` |
| wasm32 の check の呼び出し元 | `make build-profiles`、CI の `verify` ジョブ | `grep -n build-profiles Makefile .github/workflows/ci.yml` |
| wasm32 での背後実装 | Vulkan・CUDA のモジュールは `not(target_arch = "wasm32")` で外れる。Metal は feature のみでゲートされる | `grep -n wasm32 crates/contract/src/lib.rs` |
| CPU 部分 | `cpu-isa`・`cpu-ops` の feature は `crates/contract/Cargo.toml` にあり `default` に入るが、それでゲートされるモジュール・コードは無い。`crates/contract/src` は `lib.rs` と `backend_*` だけ | `ls crates/contract/src`、`grep -n 'pub mod' crates/contract/src/lib.rs`（背後実装の 3 つだけ）、`git grep -n -e 'feature = "cpu-' -- crates`（0 件）、`grep -rn -i cpu crates/contract/src`（0 件） |
| cdylib | `crate-type` を持つ manifest は `wasm/crates/glue/Cargo.toml` の 1 つだけ（`["cdylib"]`）。製品の 4 crate には無い | `git grep -n crate-type -- '*.toml'` |
| 製品の workspace | `members` は 4 crate、`exclude` は `baseline-wgpu`・`measure`・`wasm` | `grep -n -e members -e exclude Cargo.toml` |
| 別 workspace の先例 | `measure/`・`baseline-wgpu/`・`wasm/` は自身の `Cargo.toml`・`Cargo.lock`・`deps-approved.toml` を持つ（`baseline-wgpu/` は自身の `deny.toml` も持つ） | `ls measure baseline-wgpu wasm`、[measurement-tool](measurement-tool.md)・[baseline-wgpu](baseline-wgpu.md) |
| path の検査 | `scripts/deny.sh` の `ALLOWED_ROOT` は `crates/`（製品の workspace の path が外を指さないことの検査） | `grep -n ALLOWED_ROOT scripts/deny.sh` |
| 別 workspace の依存の検査 | `scripts/deny.sh` が `measure/`・`baseline-wgpu/`・`wasm/` に cargo-deny をかけ、`baseline-wgpu/`・`wasm/` の deps-check も呼ぶ。`measure/` と `wasm/` の deps-check は、それぞれ `scripts/measure-verify.sh`・`scripts/wasm-verify.sh` も呼ぶ | `grep -n -e 'cargo deny' -e deps-check scripts/deny.sh scripts/measure-verify.sh scripts/wasm-verify.sh` |
| deps-check の対象 | `scripts/deps-check.sh` は manifest のパスを引数で受け、その workspace ルート直下の `deps-approved.toml` と突き合わせる。`--allow-path-root <dir>` を付けたときだけ、`<dir>` 配下への path 依存を免除する（`wasm/` の検査だけが `crates/` を渡す） | `sed -n 1,20p scripts/deps-check.sh` |
| deps-approved.toml の件数 | ルート 0 件・`measure/` 0 件・`baseline-wgpu/` 1 件・`wasm/` 1 件 | `grep -c '^\[\[approval\]\]' deps-approved.toml measure/deps-approved.toml baseline-wgpu/deps-approved.toml wasm/deps-approved.toml` |
| CI のジョブ | `check`・`test-hooks`・`verify`・`deny`・`pr-title`・`ci-complete` | `grep -n -E '^  [a-z-]+:$' .github/workflows/ci.yml` |
| CI の runner | `runs-on` は `ubuntu-latest` と `test-hooks` の matrix（`ubuntu-latest`・`macos-latest`）。`ci-frame-check.sh` の許可ラベルも同じ 2 つ | `grep -n runs-on .github/workflows/ci.yml`、`grep -n ALLOWED_LABELS scripts/ci-frame-check.sh` |
| CI の Node | `ci.yml` に Node の導入は無い。wasm32 に触れるのは `make build-profiles`・`make wasm-verify` と、偽の node で差し替える回帰テスト `make test-wasm-node-check` の 3 ステップ（実物の Node は流さない） | `grep -n -i -e node -e wasm .github/workflows/ci.yml` |

## 別 workspace `wasm/` の所在と検査の入口

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 構成 | `wasm/Cargo.toml`（workspace）・`wasm/crates/glue/`（package `wasm-glue`。`crate-type = ["cdylib"]`）・`wasm/Cargo.lock`・`wasm/deps-approved.toml` | `ls wasm wasm/crates/glue` |
| 依存の向き | `wasm-glue` が `crates/contract` に path で依存する。製品の crate は `wasm/` を参照しない（参照すると `make deny` の path の検査が落ちる） | `grep -n path wasm/crates/glue/Cargo.toml` |
| 製品側の feature | `contract-core`・`cpu-isa`・`cpu-ops` だけを明示する（`default-features = false`）。Vulkan・CUDA は入れない | `grep -n features wasm/crates/glue/Cargo.toml` |
| 外部依存 | `wasm-bindgen = "=0.2.128"` を wasm32 の target 向けにだけ宣言する。承認の記録は `wasm/deps-approved.toml`（`approval_ref` は #368）。製品の 4 crate には無い | `grep -n -B1 wasm-bindgen wasm/crates/glue/Cargo.toml`、`make deps-check` |
| 推移的依存 | wasm32 の依存グラフにある外部の crate は 12 件（`bumpalo`・`cfg-if`・`once_cell`・`proc-macro2`・`quote`・`rustversion`・`syn`・`unicode-ident`・`wasm-bindgen` 本体と `-macro`・`-macro-support`・`-shared`）。ライセンスは `MIT OR Apache-2.0`（`unicode-ident` は加えて `Unicode-3.0`）。wgpu 系・naga は含まない | `cargo tree --manifest-path wasm/Cargo.toml --locked --target wasm32-unknown-unknown --prefix none -e normal,build`、`make deny` |
| export | `roundtrip_f32(&[f32]) -> Vec<f32>` の 1 つ。入力をビット列のまま写して返す。wasm32 のときだけ wasm-bindgen の export になる。手で書く unsafe は無い（workspace の lints は `unsafe_code = "forbid"`） | `grep -n -e roundtrip_f32 -e wasm_bindgen wasm/crates/glue/src/lib.rs` |
| 対応するテスト | `wasm/crates/glue/src/lib.rs` の単体テスト 3 件（空・有限値・非有限値と `-0.0`。ホストで流れる） | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked` |
| 量の測定の profile | `wasm/Cargo.toml` の `[profile.wasm-size]`（`release` を継承。`opt-level = "z"`・`lto = true`・`codegen-units = 1`） | `grep -n -A5 'profile.wasm-size' wasm/Cargo.toml` |
| 検査の入口 | `make wasm-verify`（`scripts/wasm-verify.sh`）。順に fmt --check、clippy（ホスト・wasm32）、test、wasm32 の build と `.wasm` の存在確認、deps-check（`--allow-path-root crates`）。target 未導入は失敗する | `sed -n 1,60p scripts/wasm-verify.sh` |
| make deny での監査 | `scripts/deny.sh` が `wasm/` にルートの `deny.toml` をそのまま当て（wgpu 系・naga の禁止は残る）、deps-check も呼ぶ | `grep -n 'wasm/' scripts/deny.sh` |
| CI | `deny` ジョブが `make wasm-verify` を呼ぶ。ジョブ・check 名は増えない | `grep -n -B1 -A2 wasm-verify .github/workflows/ci.yml` |

## Node での照合の土台

glue を Node で読み込み、最小の export を呼んで結果をネイティブの参照と照合し、行ごとに `match`・`mismatch`・`not-run` を出す枠。照合の中身（再現する行）はここに入れない。`crates/core` の `test_frame`（記録の部品・集計）や `parity`（許容誤差のある比べ方）とは役割を重ねない（比べるのはビット列の完全一致だけ）。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入口 | `make wasm-node-check`（`scripts/wasm-node-check.sh [--wasm <path>] [--cases <path>]`）。前提の確認、`.wasm` のビルド（`--wasm` なら cargo を呼ばない）、`wasm-bindgen --target nodejs` で glue を一時ディレクトリへ生成、Node の実行、判定の順。実物の実行で、CI はまだ呼ばない | `sed -n 1,20p scripts/wasm-node-check.sh` |
| Node のスクリプト | `wasm/node/run-glue.cjs`。glue を呼んで f32 のビット列（8 桁の 16 進）を書き出すだけで、判定は持たない。`require` は `node:` の組み込みと引数の glue だけ。呼べる export は許可表の `roundtrip_f32` だけ。`package.json`・npm パッケージは無い | `grep -n require wasm/node/run-glue.cjs` |
| 入力 | `wasm/node/cases.json`。`roundtrip_f32` の 3 行（空・有限値と `-0.0`・非有限値）。参照は `wasm/crates/glue/src/lib.rs` の単体テストが固定する振る舞いと同じ値。`reference` が無い・`null` の行は照合の相手が無い行 | `cat wasm/node/cases.json` |
| 前提の版 | Node は `scripts/wasm-node-check.sh` 先頭の定数 `v24.13.0` と完全一致。wasm-bindgen の CLI は `wasm/crates/glue/Cargo.toml` の版と `wasm-bindgen --version` の完全一致（宣言点を増やさない）。無い・版違いは終了コード 2。CI へ Node を入れる段で版の宣言が二重になるので、その段で一致の確かめ方を決める | `grep -n NODE_EXPECTED scripts/wasm-node-check.sh` |
| 判定 | 参照があり長さも全要素のビット列も一致なら `match`、違えば（Node 側の呼び出しの失敗を含む）`mismatch`、参照が無ければ `not-run`（一致扱いにしない）。標準出力は `row: <id> <状態>` と `summary: rows=N match=N mismatch=N not-run=N` | `bash scripts/test-wasm-node-check.sh`（項目 5〜11） |
| 終了コード | 0 は 1 行以上あり全行 `match` のときだけ。1 は `mismatch`・`not-run`・行が 0 件。2 は誤用・前提の不足・版違い・入力や結果の不正（未知の id・重複・欠け・形の不正）・Node の実行の失敗 | 同上 |
| 入力の扱い | 入力の値はシェルに展開しない（`python3 -I` の固定のコードにパスだけを引数で渡す）。`id` は `[a-z0-9-]` の文字種、ビット列は 8 桁の小文字 16 進、`export` は許可表のみ。生成物は一時ディレクトリだけに出して終了時に消す | `bash scripts/test-wasm-node-check.sh`（項目 14・17） |
| 回帰テスト | `make test-wasm-node-check`（`scripts/test-wasm-node-check.sh`）。偽の node・wasm-bindgen・cargo と合成の結果で判定の全経路を確かめる。`run-glue.cjs` は本物の node で偽の glue を読ませて確かめる。CI の `deny` ジョブが呼ぶ | `make test-wasm-node-check` |

### 振る舞いとテストの対応

| 振る舞い | 回帰テストの項目 |
| -------- | ---------------- |
| node が無い・版違い（前方一致や rc 付きを含む）は失敗で、Node の本体を起動しない | 1・2 |
| wasm-bindgen が無い・版違いは失敗 | 3・4 |
| 全行一致で 0 | 5・22 |
| ビット列が 1 つでも違えば `mismatch`（`-0.0` と `+0.0`、NaN のペイロード違い、長さ違い、Node 側のエラーを含む） | 6〜8・11 |
| 参照の無い行は `not-run` で、成功にしない | 9 |
| 行が 0 件は成功にしない | 10 |
| Node の失敗・結果の欠け・不正・未知の id・重複は 2 | 12・13 |
| 入力の不正・誤用・存在しない `.wasm`・glue が生成されない場合は 2 | 14〜16 |
| 入力の値をシェルに展開しない | 17 |
| Node と CLI に渡す引数の形・既定の経路の cargo の呼び方 | 18・19・`cargo` で始まる項目 |
| 版の宣言点が増えていない | 20 |
| npm パッケージが無く、`require` が組み込みと glue だけ | 21 |
| `run-glue.cjs` がビット列（NaN・`-0.0`・±Inf）を保ち、許可表外の export を拒否する | 23 |

## 本リポでの反映の方針

- 本書は現状の事実だけを置く。形の具体は、受け持つ段のコードと公開テストの PR で入れ、その PR で本書に節を足す
- 製品の workspace の外に置く物は [workspace-boundary](workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)、依存は [dependency-policy](../.claude/rules/dependency-policy.md)、CI に足す扱いは [ci-device-free-jobs](ci-device-free-jobs.md)・[github-actions](../.claude/rules/github-actions.md) に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[implementation-scope](implementation-scope.md)・[ci-job-split](ci-job-split.md)・[test-frame](test-frame.md)
