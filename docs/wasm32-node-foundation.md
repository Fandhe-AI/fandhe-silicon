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
| CI の Node | `ci.yml` に Node の導入・実行は無い。wasm32 に触れるのは `make build-profiles`・`make wasm-verify`・`make test-wasm-size` の 3 ステップだけ（`make test-wasm-size` は偽のコマンドで流れ、wasm32 のビルドも Node も呼ばない） | `grep -n -i -e node -e wasm .github/workflows/ci.yml` |

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
| CI | `deny` ジョブは `make test-wasm-size` だけを呼ぶ。`make wasm-size` は CLI が要るので呼ばない（後続のジョブ用） | `grep -n wasm-size .github/workflows/ci.yml` |

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

## 本リポでの反映の方針

- 本書は現状の事実だけを置く。形の具体は、受け持つ段のコードと公開テストの PR で入れ、その PR で本書に節を足す
- 製品の workspace の外に置く物は [workspace-boundary](workspace-boundary.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)、依存は [dependency-policy](../.claude/rules/dependency-policy.md)、CI に足す扱いは [ci-device-free-jobs](ci-device-free-jobs.md)・[github-actions](../.claude/rules/github-actions.md) に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[implementation-scope](implementation-scope.md)・[ci-job-split](ci-job-split.md)・[test-frame](test-frame.md)
