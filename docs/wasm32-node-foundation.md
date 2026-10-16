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
| CI のジョブ | `check`・`test-hooks`・`verify`・`deny`・`wasm-macos`・`pr-title`・`ci-complete` | `grep -n -E '^  [a-z-]+:$' .github/workflows/ci.yml` |
| CI の runner | `runs-on` は `ubuntu-latest` と `test-hooks` の matrix（`ubuntu-latest`・`macos-latest`）、`wasm-macos`（`macos-latest`。arm64）。`ci-frame-check.sh` の許可ラベルは `ubuntu-latest`・`macos-latest` | `grep -n runs-on .github/workflows/ci.yml`、`grep -n ALLOWED_LABELS scripts/ci-frame-check.sh` |
| CI の Node | `wasm-macos` ジョブが版固定・SHA256 検証済みの Node と wasm-bindgen の CLI を入れ、`make wasm-verify`・`make wasm-size`・`make wasm-node-check` を実物で流す。`deny` ジョブの `make test-wasm-size`・`make test-wasm-node-check` は偽のコマンドで流れる。`make build-profiles` は wasm32 の check（`verify` ジョブ） | `grep -n -i -e node -e wasm .github/workflows/ci.yml` |

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

## glue 込みの量の測定（`scripts/wasm-size.sh`）

TASK-22 の 3/6。測り方・閾値の根拠は private spec 側（ポインタ: BUILD-42・T-12）。本リポにはコードとテストで示せる事実だけを置く。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | 本体 `scripts/wasm-size.sh`（`make wasm-size`）、回帰テスト `scripts/test-wasm-size.sh`（`make test-wasm-size`）。bash から `python3 -I -`（標準ライブラリだけ・3.8 以上）に値の扱いを任せる | `make help` |
| 組 | profile（`release`・`wasm-size`）× 命令（`scalar`・`simd128`）× 出力先（`web`・`bundler`）の 8 組。置き方は `<dir>/<profile>-<命令>/<出力先>/` | `grep -n -e '^PROFILES' -e '^ISAS' -e '^TARGETS' scripts/wasm-size.sh` |
| 数えるファイル | `web` は `wasm_glue.js`・`wasm_glue_bg.wasm`、`bundler` はそれに `wasm_glue_bg.js`。`.d.ts`・`package.json` などは数えない。ファイルごとに gzip（level 9・mtime 0）した長さを組ごとに合計する | `grep -n -A4 '^TARGETS' scripts/wasm-size.sh` |
| 閾値 | `LIMIT_BYTES` の 1 か所だけで宣言する。合計が閾値以下なら `ok`、超えたら `ng`。引数・環境変数で変える口は無い | `grep -n LIMIT_BYTES scripts/wasm-size.sh` |
| 未実施 | 組のディレクトリ・数えるファイルが無い、数えるファイルが空、ビルドまたは glue の生成の失敗は `not-run`（reason 付き）。0 バイトや合格にしない | `scripts/test-wasm-size.sh` |
| 終了コード | 0 全部の組が `ok`、1 `ng` または `not-run` の組がある、2 誤用・前提の不足・読めない入力 | `sed -n 1,25p scripts/wasm-size.sh` |
| ビルドする経路の前提 | `wasm-bindgen` の CLI が `0.2.128` と厳密に一致（`wasm/Cargo.lock` の版とも突き合わせる）、wasm32 の target、依存が手元にある（`make wasm-verify` で取得。ビルドは `--locked --offline`）。外れたら cargo を呼ぶ前に終了コード 2。CLI の入手は試みない | `scripts/test-wasm-size.sh` |
| 子プロセスの環境 | `RUSTFLAGS` は渡さず、`CARGO_ENCODED_RUSTFLAGS` で命令の指定だけを渡す。命令ごとに target ディレクトリを分け、cargo の `target_directory` 直下の専用ディレクトリ（`glue-size`）に出す | `grep -n -e CARGO_ENCODED_RUSTFLAGS -e WORK_SUBDIR scripts/wasm-size.sh` |
| CI | `deny` ジョブは `make test-wasm-size`（偽のコマンド）を呼ぶ。実物の `make wasm-size` は CLI を入れた `wasm-macos` ジョブが呼ぶ | `grep -n wasm-size .github/workflows/ci.yml` |

流し方:

- `make wasm-verify` で依存を取得したあと `make wasm-size`。CLI が無ければ終了コード 2 と `error:` が出る
- 生成済みの出力だけを測るなら `scripts/wasm-size.sh --from-dir <dir>`（cargo も CLI も呼ばない）

振る舞いと回帰テスト（`scripts/test-wasm-size.sh`。判定は終了コード + 出力の部分一致）の対応:

| 振る舞い | テストのケース |
| -------- | -------------- |
| 8 組とも小さければ終了コード 0。`gzip_bytes` が Python で数えた値と一致 | `all-ok` |
| 合計がちょうど閾値は `ok`、閾値 + 1 は `ng` | `boundary-eq`・`boundary-over` |
| 1 組だけ超えたら終了コード 1 でその組だけ `ng` | `one-ng` |
| 組のディレクトリ・数えるファイルの欠落、空のファイルは `not-run` で終了コード 1 | `missing-dir`・`missing-file`・`empty-file` |
| `.d.ts`・`package.json` は量に入らない | `extra-files` |
| シンボリックリンク・通常でないファイル・組の一覧にない項目は終了コード 2 | `symlink`・`dir-as-file`・`unknown-entry` |
| 誤用（未知の引数・値の欠落・存在しない／ディレクトリでないパス） は終了コード 2 | `unknown-arg`・`from-dir-missing-value`・`no-such-dir`・`from-dir-is-file`・`from-dir-empty`・`positional` |
| CLI 無し・版違い・読めない版・target 無しは終了コード 2 で cargo を呼ばない | `no-bindgen`・`bad-version`・`no-target` |
| 偽のコマンドで 8 組を組み立て、profile・命令の指定が組ごとに正しく渡り、利用者の `RUSTFLAGS` は渡らない | `fake-all`・`fake-flags`・`fake-bindgen-calls` |
| ある設定のビルドが失敗したら、その 2 組が `not-run`（`build-failed`）で終了コード 1 | `fake-build-failed` |
| 出力に一時ディレクトリの絶対パスが出ない | 全ケース |

## Node での照合の土台

glue を Node で読み込み、最小の export を呼んで結果をネイティブの参照と照合し、行ごとに `match`・`mismatch`・`not-run` を出す枠。照合の中身（再現する行）はここに入れない。`crates/core` の `test_frame`（記録の部品・集計）や `parity`（許容誤差のある比べ方）とは役割を重ねない（比べるのはビット列の完全一致だけ）。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 入口 | `make wasm-node-check`（`scripts/wasm-node-check.sh [--wasm <path>] [--cases <path>]`）。前提の確認、`.wasm` のビルド（`--wasm` なら cargo を呼ばない）、`wasm-bindgen --target nodejs` で glue を一時ディレクトリへ生成、Node の実行、判定の順。実物の実行で、CI は `wasm-macos` ジョブが呼ぶ | `sed -n 1,20p scripts/wasm-node-check.sh` |
| Node のスクリプト | `wasm/node/run-glue.cjs`。glue を呼んで f32 のビット列（8 桁の 16 進）を書き出すだけで、判定は持たない。`require` は `node:` の組み込みと引数の glue だけ。呼べる export は許可表の `roundtrip_f32` だけ。`package.json`・npm パッケージは無い | `grep -n require wasm/node/run-glue.cjs` |
| 入力 | `wasm/node/cases.json`。`roundtrip_f32` の 3 行（空・有限値と `-0.0`・非有限値）。参照は `wasm/crates/glue/src/lib.rs` の単体テストが固定する振る舞いと同じ値。各行は参照の出どころの欄 `reference_source` が必須で、値は `fixed`・`native-neon` の 2 つだけ（欠け・未知の値は入力の不正）。同梱の 3 行はどれも `fixed`。`reference` が無い・`null` の行は照合の相手が無い行 | `cat wasm/node/cases.json` |
| 前提の版 | Node は `scripts/wasm-node-check.sh` 先頭の定数 `v24.13.0` と完全一致。wasm-bindgen の CLI は `wasm/crates/glue/Cargo.toml` の版と `wasm-bindgen --version` の完全一致（宣言点を増やさない）。無い・版違いは終了コード 2。版の宣言は `ci.yml` の `env`（導入用）とスクリプト側（`NODE_EXPECTED` など）の二つ。一致は `wasm-macos` ジョブの中で二重に確かめる（導入した物と `env` を比べ、各 make が自分の定数と比べる）ので、ずれれば終了コード 2 で失敗する | `grep -n NODE_EXPECTED scripts/wasm-node-check.sh` |
| 判定 | 参照があり長さも全要素のビット列も一致なら `match`、違えば（Node 側の呼び出しの失敗を含む）`mismatch`、出どころがこの環境で参照を作れないもの（今は `native-neon` の全行。参照の値があっても比べない）は `not-run (reference-unavailable)`、作れる出どころで参照が無ければ `not-run (reference-missing)`（どちらも一致扱いにしない。出どころを先に見る）。標準出力は `row: <id> <状態>`（`not-run` は括弧で理由）と `summary: rows=N match=N mismatch=N not-run=N` | `bash scripts/test-wasm-node-check.sh`（項目 5〜11） |
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
| 参照の無い行は `not-run (reference-missing)` で、成功にしない | 9・26 |
| 出どころの欄の欠け・未知の値は 2 で、Node の本体を起動しない | 25 |
| 作れない出どころ（`native-neon`）の行は参照の値があっても `not-run (reference-unavailable)`。出どころが先 | 26 |
| 出どころ・理由の綴りが core のソースに在る | 27 |
| 行が 0 件は成功にしない | 10 |
| Node の失敗・結果の欠け・不正・未知の id・重複は 2 | 12・13 |
| 入力の不正・誤用・存在しない `.wasm`・glue が生成されない場合は 2 | 14〜16 |
| 入力の値をシェルに展開しない | 17 |
| Node と CLI に渡す引数の形・既定の経路の cargo の呼び方 | 18・19・`cargo` で始まる項目 |
| 版の宣言点が増えていない | 20 |
| npm パッケージが無く、`require` が組み込みと glue だけ | 21 |
| `run-glue.cjs` がビット列（NaN・`-0.0`・±Inf）を保ち、許可表外の export を拒否する | 23 |

## macOS arm64 のジョブ（TASK-22 の 5/6）

`ci.yml` の `wasm-macos`（check 名 `wasm32 build + size + node (macos-latest)`）。中身は make ターゲットを呼ぶだけで、CI 独自の判定は持たない。

| 項目 | 現状 | 確かめ方 |
| ---- | ---- | -------- |
| 流れる物 | `make wasm-verify`、`make wasm-size`、`make wasm-node-check` の順（先頭が依存を取得し、後の 2 つは `--offline` で動く） | `grep -n -A60 '^  wasm-macos:' .github/workflows/ci.yml` |
| 道具の入れ方 | Node と wasm-bindgen の CLI を、版固定のうえ SHA256 を確かめてから入れる。版と SHA256 は `ci.yml` 先頭の `env`。入れたあとに版が宣言と一致しなければ失敗する。arm64 でなければ失敗する | `grep -n -E 'NODE_|WASM_BINDGEN_CLI_' .github/workflows/ci.yml` |
| 作りの制約 | キャッシュを使わない。`continue-on-error` も `\|\| true` も無い。`permissions` は `contents: read`、checkout は `persist-credentials: false` | `grep -n -E 'continue-on-error|cache' .github/workflows/ci.yml`（0 件） |
| 集約 | `ci-complete` の `needs` に入っている。ruleset の required status checks にも登録している（check 名 `wasm32 build + size + node (macos-latest)`） | `sed -n '/^  ci-complete:/,$p' .github/workflows/ci.yml` |
| 枠の検査 | `make ci-frame-check`・`make test-ci-frame-check` が通る | `make ci-frame-check` |

## 完了の条件の確かめ（6/6）

6/6（#3419）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-22）。
結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 別 workspace の検査の入口（2/6） | `make wasm-verify` | 終了コード 0・glue の単体テストは `3 passed; 0 failed` |
| A2 | glue 込みの量の測定の回帰テスト（3/6） | `make test-wasm-size` | 終了コード 0・`36 passed, 0 failed` |
| A3 | Node での照合の土台の回帰テスト（4/6） | `make test-wasm-node-check` | 終了コード 0・`passed: 65, failed: 0` |
| A4 | main の CI の `deny` ジョブ | main の CI（`c801677` の push run）の `make deny (cargo-deny)` ジョブの step | `make deny`・`make wasm-verify`・`make deps-check`・`make test-wasm-size`・`make test-wasm-node-check` とも conclusion は success |
| A5 | main の CI の新しいジョブ（5/6） | 同じ run の `wasm32 build + size + node (macos-latest)` ジョブの step | `Install pinned Node and wasm-bindgen CLI`・`make wasm-verify`・`make wasm-size`・`make wasm-node-check` とも conclusion は success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make ci-frame-check`・`make test-ci-frame-check` | すべて終了コード 0 |

実物の `make wasm-size`・`make wasm-node-check` は、wasm-bindgen の CLI と版固定の Node が要るため手元の表に含めず、A5 の CI のジョブで確かめた。
照合の中身は TASK-237・247・248・252・994〜997 の範囲で、この表に含めない（ID のポインタのみ）。

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-22）。

### 再実行の契機

- `wasm/`・`scripts/wasm-verify.sh`・`scripts/wasm-size.sh`・`scripts/test-wasm-size.sh`・`scripts/wasm-node-check.sh`・`scripts/test-wasm-node-check.sh`・`wasm/node/` を変えたとき
- CI の `deny` ジョブ・`wasm-macos` ジョブの手順を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `c801677`、`docs/spec` の gitlink は `3c83ea3`。

## 本リポでの反映の方針

- 本書は現状の事実だけを置く。形の具体は、受け持つ段のコードと公開テストの PR で入れ、その PR で本書に節を足す
- 製品の workspace の外に置く物は [workspace-boundary](workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)、依存は [dependency-policy](../.claude/rules/dependency-policy.md)、CI に足す扱いは [ci-device-free-jobs](ci-device-free-jobs.md)・[github-actions](../.claude/rules/github-actions.md) に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[implementation-scope](implementation-scope.md)・[ci-job-split](ci-job-split.md)・[test-frame](test-frame.md)
