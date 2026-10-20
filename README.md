# fandhe-silicon

fandhe-ai・vector-db・fandhe-3d で共用する、独立した低レイヤーの CPU／GPU 基盤
（device・メモリ・実行・同期・能力問合せ・診断）の実装リポジトリです。
Metal・Vulkan・CUDA を背後実装とし、wgpu を依存関係から外せる状態を目指します。

開発基盤は [Fandhe-AI/template-dev](https://github.com/Fandhe-AI/template-dev) から作成しています。
実装言語は Rust（edition 2024・stable）で、現在は workspace と crate の骨格（空の公開面・依存なし）のみです。

## 仕様（`docs/spec`）

仕様・フェーズ文書は private リポジトリ `Fandhe-AI/fandhe-silicon-spec` を `docs/spec` に
submodule として置いています（閲覧には権限が必要です）。

認証の無い環境での clone が失敗しないよう `.gitmodules` で `update = none` を指定しているため、
`git clone --recurse-submodules` や `git submodule update --init` では取得されません。
spec リポジトリへの読み取り権限がある環境で、次のように明示的に取得します。

```bash
git -c submodule.docs/spec.update=checkout submodule update --init docs/spec
```

参照の更新は `update-external.yml` が日次で PR を作成します（後述の「CI」）。

## ライセンス

MIT OR Apache-2.0 のデュアルライセンスです（[LICENSE-MIT](./LICENSE-MIT) / [LICENSE-APACHE](./LICENSE-APACHE)）。

## crate 構成

3 段（下-1・下-2・上）と、3 段で受け渡す型を置く共通 crate の 4 crate 構成です。
公開名・crates.io への公開は未定のため、全 crate を `publish = false` としています。

| パス | crate | 役割 |
|---|---|---|
| `crates/core/` | `fandhe-silicon-core` | 共通の型（device・メモリ・実行・同期・能力問合せ・診断）。外部 crate に依存しない。SHA-256 の関数（`hash`）とパイプライン生成の入力の型と引数の宣言の型とキャッシュキーの型（`execution`）とバッファの引数検査の純粋関数（`memory`）を条件なしで公開する。feature `test-support` の裏に検証用の道具（`parity`）と試験の枠（`test_frame`。宣言・実行の可否・結果のラベルの決定・組み合わせごとの集計・試験ごとの記録の部品と書き出し）と流し直しの照合の型と実行の口（`rerun`）がある |
| `crates/contract/` | `fandhe-silicon-contract` | 下-1: チップごとの薄い呼び出し。Metal・Vulkan・CUDA の背後実装を `backend-*` feature で持つ |
| `crates/exec/` | `fandhe-silicon-exec` | 下-2: 共通の操作と代わりの実行。`core` のみに依存する |
| `crates/upper/` | `fandhe-silicon-upper` | 上: wgpu 風の層。`#![forbid(unsafe_code)]` |

依存の向きは `contract → core` / `exec → core` / `upper → core, exec` です。
`unsafe` と FFI は `contract` の backend モジュールに閉じ込め、それ以外の crate は `#![forbid(unsafe_code)]` です。形は [docs/unsafe-policy.md](docs/unsafe-policy.md) にあります。

`fandhe-silicon-contract` の feature:

| feature | 既定 | 内容 |
|---|---|---|
| `contract-core`・`cpu-isa`・`cpu-ops` | ✓ | CPU のみの構成 |
| `backend-metal`・`backend-vulkan`・`backend-cuda` | | 各 GPU の背後実装。いずれも `contract-core` を有効にする（`--no-default-features` でも共通契約の入口は含まれる）。`backend-vulkan`・`backend-cuda` は `wasm32` では有効にならない |
| `backend-dx12-compute` | | DX12 の compute の背後実装の置き場（名前は仮）。現時点で中身はなく、既定では無効。`contract-core` を有効にする |
| `graphics`・`ext-interop`・`native-blocking-wait` | | 描画・外部 API との相互運用・ブロッキング待機 |
| `test-support`・`fault-injection`・`artifact-inspection` | | テスト・診断専用（本番ビルドに含めない）。`test-support` は `core` の同名 feature（検証用の道具）へ、`fault-injection` は `core` の同名 feature（故障の差し込み口）へ転送する。`artifact-inspection` は `backend-metal`（したがって `contract-core`）も有効にするため、`--no-default-features` でこの feature だけを選んでも Metal の背後実装と共通契約の入口が含まれる |

wgpu 系（`wgpu`・`wgpu-core`・`wgpu-hal`・`wgpu-types`）と `naga` は `deny.toml` で依存グラフへの混入を禁止しています。

## 構成

| パス | 役割 |
|---|---|
| `wasm/` | wasm32 向けの別 workspace（TASK-22）。cdylib `wasm-glue`（`wasm/crates/glue/`）が `crates/contract` に path で依存する（向きは wasm/ → 製品だけ）。製品の依存グラフに入らない（ルート `exclude`）。外部依存は wasm-bindgen の 1 件（wasm32 向けにだけ宣言）。`make wasm-verify` が検査し、CI の `deny` ジョブも呼ぶ |
| `measure/` | 測定専用の別 workspace（TASK-6・TASK-27）。共有 crate `measure-perf`（`measure/crates/perf/`）に標本・run ごとの統計・run 間の集計・run ごとの記録の書き出し・記録を読んで run 間の集計を出すコマンド `across-runs`・記録のパスを引数で受けてラベルを再現するコマンド `variation-replay` を置く。製品の依存グラフに入らない（ルート `exclude`）。外部依存は 0 件（試験だけが `crates/core` を dev-dependency で使う）。`make measure-verify` が検査し、CI の `deny` ジョブも呼ぶ |
| `baseline-wgpu/` | 比較基準の wgpu を置く別 workspace（TASK-7）。基盤の依存グラフに入らず、`make deny` が cargo-deny（`baseline-wgpu/deny.toml`）と依存の承認の記録を検査する（ビルドはしない）。外部依存は wgpu のみ（完全固定）で、アダプタ列挙の smoke bin のみ |
| `crates/` | Rust の crate（前述の「crate 構成」） |
| `Cargo.toml`・`Cargo.lock` | workspace 定義（edition・ライセンス・lint を一元管理）とロックファイル |
| `rust-toolchain.toml` | ツールチェーンの単一真実源（stable + rustfmt / clippy） |
| `deny.toml` | cargo-deny の設定（アドバイザリ・ライセンス・取得元・wgpu 系の禁止）。`baseline-wgpu/deny.toml` は wgpu 系の禁止だけを外した別 workspace 用 |
| `deps-approved.toml` | 依存の承認の記録（`make deps-check` が宣言済みの外部依存と突き合わせる。書式は同ファイル先頭） |
| `CLAUDE.md`・`.claude/agents/`・`.claude/rules/`・`.claude/settings.json` | Claude Code 用のリポジトリ案内・subagent 定義・規約・hooks |
| `.claude/workflows/` | `implement-issue-tree` の workflow（スキル内スクリプトへの symlink） |
| `.agents/skills/`・`.claude/skills/` | エージェント用スキル（`skills-lock.json` で管理する外部取得物。`.claude/skills/` は `.agents/skills/` への symlink） |
| `Makefile` | `scripts/` を呼ぶだけの薄い入口（`make help` で一覧） |
| `scripts/` | 処理の実体（`help` / `doctor` / `setup` / `check` / `verify` / `build-profiles` / `deny` / `deps-check` / `test-deps-check` / `inventory-trace-check` / `test-inventory-trace-check` / `session-record-check`（make ターゲットなし・直接実行）/ `test-session-record-check` / `perf-record-check`（make ターゲットなし・直接実行）/ `test-perf-record-check` / `record-summary`（make ターゲットなし・直接実行）/ `test-record-summary` / `metal-session`（make ターゲットなし・直接実行）/ `test-metal-session` / `hooks/test-hooks`）。Make なしでも直接実行できる |
| `scripts/hooks/` | lefthook から呼ばれる Git hooks の実体 |
| `lefthook.yml` | Git hooks 定義（pre-commit / commit-msg）。編集方針は `.claude/rules/git-hooks.md`（フックの時間予算と CI との役割分担） |
| `.editorconfig`・`.editorconfig-checker.json` | 文字コード・改行・インデントの宣言と、その検査の除外設定（ライセンス本文は除外） |
| `.shellcheckrc` | shellcheck が `source` 先（`scripts/lib.sh`）を追って検査するための設定 |
| `.envrc`・`.env.example` | direnv による `.env` の読み込みと、その雛形 |
| `.mcp.json` | Claude Code のプロジェクト共有 MCP サーバー定義（ベースは空） |
| `docs/contract/` | 契約の明文のまとめ（置き場と書式は同ディレクトリの README）と、形を決める作業の手順書（`procedure.md`） |
| `docs/workspace-boundary.md` | 製品 workspace の境界（members・依存から外すもの・外部 workspace からの持ち込みの扱い） |
| `docs/wgpu-baseline-record.md` | 比較用の run に残す wgpu の版の記録の書式 |
| `docs/baseline-wgpu.md` | 比較基準の wgpu の置き場（TASK-7）の所在・製品 workspace を守る仕組み・別 workspace の配置・版の記録・確かめ方 |
| `docs/parity-tool.md` | 結果の一致を確かめる共通の道具（TASK-24）の所在・振る舞い・対応するテスト・閾値と範囲の扱い・確かめ方 |
| `docs/pixel-parity-tool.md` | 画素の一致を確かめる共通の道具（TASK-26）の所在・振る舞い・対応するテスト・確かめ方 |
| `docs/parity-non-finite.md` | 有限でない値の規則（TASK-25）の所在・振る舞い・対応するテスト・確かめ方 |
| `docs/parity-signed-zero-topk.md` | ±0 の扱いと Top-k の規則（TASK-976）の所在と本リポの現状の事実・完了の条件の確かめ（6/6） |
| `docs/parity-element-rules.md` | T-1 の要素ごとの規則の確かめ（TASK-975）の所在と、本リポの現状の事実・完了の条件の確かめ（6/6） |
| `docs/parity-fallback-compare.md` | 代わりの実行の結果の比べ方の所在・振る舞い・対応するテスト・本リポの現状の事実・完了の条件の確かめ（8/8）（TASK-928） |
| `docs/gemm-test-inputs.md` | GEMM の試験の入力と参照・母数の表・累積の順の対照・門の run の枠・完了の条件の確かめ（6/6）（TASK-1005）の所在・振る舞い・対応するテスト・本リポの現状の事実 |
| `docs/parity-case-inputs.md` | 結果の一致の試験の共通の入力の所在・17 ケースの入力と参照の出力・追加の入力の振る舞い・対応するテスト・本リポの現状の事実・別 crate からの確かめ・完了の条件の確かめ（4/4）（TASK-977） |
| `docs/parity-fallback-inputs.md` | 代わりの実行の試験の入力と CPU の参照の所在・`f16-fma`・`subgroup-sum`・`atomic-add` の入力と CPU の参照の振る舞い・出力の形の規則の試験・対応するテスト・本リポの現状の事実・完了の条件の確かめ（6/6）（TASK-929） |
| `docs/artifact-input-types.md` | 成果物の受け付けに使う公開の型（TASK-470）の所在・現状の型・対応するテスト・確かめ方・完了の条件の確かめ（6/6） |
| `docs/contract-error-types.md` | 契約のエラー・完了状態の型（TASK-269）の所在と、本リポの現状の事実・現状の形（2/8〜7/8）・完了の条件の確かめ（8/8） |
| `docs/contract-device-types.md` | 契約の公開の型（TASK-136）の所在と、本リポの現状の事実・現状の形（2/6〜4/6）・確かめ（5/6）・完了の条件の確かめ（6/6） |
| `docs/contract-fault-injection.md` | 故障の差し込みの部品（TASK-271）の所在と、本リポの現状の事実・現状の形（2/8〜6/8）・試験（7/8）・使い分けの手順（8/8）・完了の条件の確かめ（8/8） |
| `docs/capability-query-rules.md` | 能力の問合せの共通の規則（TASK-273）の所在と、本リポの現状の事実・現状の形（2/6〜5/6）・完了の条件の確かめ（6/6） |
| `docs/artifact-cache-check.md` | 成果物キャッシュの照合（TASK-483）の所在と、本リポの現状の事実・現状の形（2/8〜4/8）・試験（5/8・6/8）・確かめ（7/8）・完了の条件の確かめ（8/8） |
| `docs/dispatch-operand-check.md` | 引数の宣言の C4〜C8 を判定する純粋関数（TASK-421・TASK-423）の所在・現状の振る舞い・対応するテスト・確かめ方・完了の条件の確かめ（4/4）・現状と前提の確認（1/4）・C4 の容量不明・offset の試験（2/4）・alignment 0 の C5 の試験（3/4）・完了の条件の確かめ（4/4）（TASK-425）・C7 の実装（1/2）・統合テストと完了の条件の確かめ（2/2）（TASK-2952）・定数の C7 の試験（1/2）・完了の条件の確かめ（2/2）（TASK-424） |
| `docs/buffer-range-check.md` | バッファの引数検査の純粋関数（TASK-309 の 1/4）の所在・現状の振る舞い・対応するテスト・確かめ方・境界の入力の試験（2/4）・拒否のときの内容の不変の試験（3/4）・完了の条件の確かめ（4/4） |
| `docs/unsafe-policy.md` | `unsafe` の局所化・不変条件の書式・個数テストの枠の形・個数が変わるときの扱い（TASK-9） |
| `docs/inventory-trace-check.md` | 一覧と項目の対応を確かめる検査（TASK-16）の所在・検査のスクリプト・対応するテスト・確かめ方・追随分の取り込みの手順（TASK-17） |
| `docs/implementation-scope.md` | 実装に含める範囲の決定の記録の所在と、本リポの現状の事実（TASK-43） |
| `docs/vulkan-operand-extraction.md` | Vulkan の成果物からオペランドの宣言を取り出す道具の所在と、本リポの現状の事実（TASK-98） |
| `docs/dx12-compute-check.md` | DX12 の compute に関する点検の記録の所在と、本リポの現状（feature・モジュール・公開面）の事実（TASK-236） |
| `docs/ci-job-split.md` | CI の分け方の決定の記録の所在と、本リポの CI の現状（ジョブ・runner・呼ぶ make）の事実（TASK-18） |
| `docs/ci-device-free-jobs.md` | 機材の要らないジョブの枠の所在と、本リポの現状の事実（どのジョブに足すか・失敗の扱い・`ci.yml` が呼ぶ検査と make ターゲットの対応・`ci-frame-check` の規則とテストの対応・足す手順）（TASK-112） |
| `docs/wasm32-node-foundation.md` | wasm32 のビルドと Node での照合の土台の所在と、本リポの現状の事実・完了の条件の確かめ（6/6）（TASK-22）・量の測定の記録（TASK-247 の 3/4）・完了の条件の確かめ（TASK-247 の 4/4） |
| `docs/wasm32-build.md` | wasm32 向けの cdylib（`crates/wasm-abi`）の所在と scalar 版・simd128 版のビルド・2 版の判別と型・cfg の確かめ・現状の事実・CI への組み込み（TASK-237 の 1/4〜4/4） |
| `docs/test-frame.md` | 共通の試験の枠の所在と、本リポの現状の事実（TASK-23） |
| `docs/rerun-frame.md` | 流し直しの枠と前の記録と比べる道具の所在・振る舞い・対応するテスト・確かめ方・完了の条件の確かめ（8/8）（TASK-29） |
| `docs/public-name-check.md` | 公開 API の名前の検査の決定の記録の所在と、本リポの現状（スクリプト・設定・公開面）の事実（TASK-14・TASK-113） |
| `docs/core-deps-check.md` | 共通の crate の外部依存の一覧の完全一致と feature の転送の検査の所在・照合の規則・対応するテスト・TASK-13 の検査との突き合わせ・確かめ方（TASK-258） |
| `docs/public-api-sdk-check.md` | 公開 API と共通の crate の依存にチップの SDK の crate が無いことの検査の所在・振る舞い・対応するテスト・確かめ方（TASK-13） |
| `docs/public-api-wgpu-check.md` | 公開 API に wgpu の型が無いことの検査の所在・振る舞い・対応するテスト・確かめ方（TASK-10） |
| `docs/public-api-contract-check.md` | 契約の crate の実物に当てた公開 API の検査の確かめの所在と、本リポの現状の事実・本番の feature の組と --all-features での wgpu 系の参照（1/4）・外部 crate の一覧とチップの SDK の型（2/4）・公開 API の検査の実行の記録（3/4）・外部 crate の一覧のまとめと完了の条件の確かめ（4/4）（TASK-253） |
| `docs/public-api-webgpu-check.md` | 公開 API の検査（TASK-241）の所在と、本リポの現状の事実・現状と前提の確認（1/6）・(1) の検査の実装（2/6）・(1) の検査の試験（3/6）・(2) の検査の実装と試験（4/6）・(3) の検査の実装と試験（5/6）・CI への組み込みと完了の条件の確かめ（6/6） |
| `docs/dep-graph-wgpu-check.md` | 依存グラフの wgpu 系 crate の検査の所在・出力の読み方・数え方の定義の所在と変える手順・確かめ方（TASK-11） |
| `docs/contract-feature-check.md` | `default` feature とテスト・診断専用 feature の検査（TASK-226）の所在と、本リポの現状の事実・現状と前提の確認（1/3）・実装（2/3）・CI への組み込み（3/3） |
| `docs/ext-interop-build-check.md` | `ext-interop` と片方の背後実装の組のビルドの確かめ（TASK-231）の所在と、本リポの現状の事実・現状と前提の確認（1/4）・実装（2/4）・テスト（3/4）・完了の条件の確かめと 3 構成の結果の記録（4/4） |
| `docs/profile-dep-graph-check.md` | 依存グラフのプロファイルごとの検査の所在・出力の読み方・数え方の定義の所在・陽性対照・確かめ方・完了の条件の確かめ（5/6）・プロファイルごとの件数の記録（6/6）（TASK-228）・現状と前提の確認（1/6）（TASK-229）・Graphics（Vulkan）の組の検査（2/6）・Graphics と相互運用の組の検査（3/6）・選んでいない SDK と比較基準の差の検査（4/6）・試験（5/6）・完了の条件の確かめと件数・差の記録（6/6）（TASK-229） |
| `docs/upper-layer-check.md` | 上の層の検査の所在・出力の読み方・振る舞いと対応するテスト・確かめ方（TASK-15） |
| `docs/baseline-positive-control.md` | 比較基準への陽性対照の所在・出力の読み方・振る舞いと対応するテスト・確かめ方（TASK-12） |
| `docs/public-api-diff.md` | 公開 API の差分の道具の所在・2 つの JSON の作り方・流し方・振る舞いと対応するテスト・確かめ方・完了の条件の確かめ（5/5）（TASK-17） |
| `docs/measurement-record.md` | 測定の記録の形式と置き場の決定の記録の所在と、本リポの現状（コード・検査・既存の書式の文書）の事実（TASK-28） |
| `docs/measurement-tool.md` | 性能を測る共通の道具の所在・本リポの現状の事実・形の設計（決定の記録のポインタと守る規約）・現状の形（3/8〜7/8・9/11・10/11）・完了の条件の確かめ（8/8）（TASK-27） |
| `docs/perf-gate-rule.md` | 性能の値を採る前の門の規則の所在と、本リポの現状の事実・門の規則（2/4）・試験（3/4）・完了の条件の確かめ（4/4）（TASK-1026） |
| `docs/perf-variation-rule.md` | 性能の判定の変動の幅の規則の所在と、本リポの現状の事実・形の設計（決定の記録のポインタと守る規約）・変動の幅の規則（3/6）・測り直しと判定不能の扱い（4/6）・再現の試験（5/6）・完了の条件の確かめ（6/6）（TASK-1027） |
| `docs/metal-run-procedure.md` | Metal の実機での試験の実行手順（TASK-19）。手順を助けるスクリプト `scripts/metal-session.sh` の振る舞いと、回帰テストとの対応 |
| `docs/session-record.example.json` | 回ごとの記録のダミーの記入例（値はすべてダミー） |
| `docs/run-record.example.json` | run ごとの記録のダミーの記入例（値はすべてダミー。書き出しの実物と一致） |
| `docs/spec/` | 仕様・フェーズ文書（private の submodule。前述の「仕様」） |
| `.gitmodules` | submodule 定義（`docs/spec` を `update = none` で登録） |
| `LICENSE-MIT`・`LICENSE-APACHE` | ライセンス（前述の「ライセンス」） |
| `.github/workflows/` | CI・AI PR レビュー・外部ソース自動追従（後述の「CI」） |

## 必要なツール

| ツール | 用途 | 必須 |
|---|---|---|
| [GNU Make](https://www.gnu.org/software/make/) 3.81+ | `make` 入口 | ✓ |
| [rustup](https://rustup.rs/) | Rust ツールチェーン（`rust-toolchain.toml` に従い自動導入） | ✓ |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) 0.20.2 | 依存の監査（`make deny`。CI と同じ版を推奨） | 任意 |
| [jq](https://jqlang.org/) | 依存の版指定の検査（`make deps-check` / `make test-deps-check`） | 任意 |
| [Python](https://www.python.org/) 3.11 以上 | TOML 解析（`make deps-check` / `make test-deps-check`。標準ライブラリの `tomllib`。`make inventory-trace-check` / `make test-inventory-trace-check` も使う。`make test-session-record-check`・`make test-metal-session` / `make test-record-summary` / `make ci-frame-check` / `make test-ci-frame-check` / `make public-api-check` / `make test-public-api-check` / `make test-public-api-sdk-check` / `make dep-graph-wgpu-check` / `make test-dep-graph-wgpu-check` / `make baseline-positive-control` / `make test-baseline-positive-control` / `make upper-layer-check` / `make test-upper-layer-check` / `make deny` / `make public-name-check` / `make public-api-sdk-check` / `make test-public-api-diff` / `make test-rerun-compare` / `make wasm-size` / `make test-wasm-size` / `make test-wasm-build-check` / `make wasm-backend-check` / `make test-wasm-backend-check` と `scripts/session-record-check.sh`・`scripts/metal-session.sh`・`scripts/public-api-diff.sh`・`scripts/rerun-compare.sh` は 3.8 以上で動く） | 任意 |
| [Python](https://www.python.org/) 3.8 以上 | `make verify` が試験の枠の記録から集計の表を作る（`scripts/record-summary.sh`。標準ライブラリだけ）。`make doctor` は無いとき・3.8 未満のときに失敗として知らせる | ✓ |
| [lefthook](https://lefthook.dev/) | Git hooks | ✓ |
| [editorconfig-checker](https://github.com/editorconfig-checker/editorconfig-checker) | `.editorconfig` 準拠チェック | ✓ |
| [ShellCheck](https://www.shellcheck.net/) | シェルスクリプトの lint | ✓ |
| [direnv](https://direnv.net/) | `.env` の自動読み込み | 任意 |

macOS（Homebrew）の例:

```bash
brew install lefthook editorconfig-checker shellcheck direnv rustup
rustup-init          # 初回のみ。以降は rust-toolchain.toml の版が自動で使われる
cargo install --locked cargo-deny@0.20.2   # 任意
```

`make doctor` で導入状況を確認できます（読み取りのみで、何も導入・変更しません）。

## セットアップ

```bash
make doctor   # 必要なツールが揃っているか確認する
make setup    # Git hooks を有効化し、.env が無ければ .env.example から作成する
direnv allow  # direnv を使う場合のみ。.envrc の読み込みを許可する
```

`make setup` は何度実行しても安全です（既存の `.env` は上書きしません）。

ビルド成果物（`target/`）は肥大化しやすいため、ルート `Cargo.toml` の `[profile.dev.package."*"]` で依存クレート
（workspace 外）のデバッグ情報を行番号のみにしています（`dev`・`test` に効き、本リポの crate は full debuginfo のままです。
現在は外部依存が無いため実効は無く、依存を追加した時点から効きます）。溜まった古い成果物は `cargo install cargo-sweep` のうえ
`cargo sweep --time 14`（14 日以上使われていない成果物を削除）で消せます。

## コマンド

| コマンド | 直接実行 | 内容 |
|---|---|---|
| `make help` | `scripts/help.sh` | 操作の一覧を表示する |
| `make doctor` | `scripts/doctor.sh` | 開発環境を診断する |
| `make setup` | `scripts/setup.sh` | Git hooks 有効化と `.env` 雛形の配置 |
| `make check` | `scripts/check.sh` | editorconfig-checker + shellcheck（ソースは変更しない） |
| `make verify` | `scripts/verify.sh` | `cargo fmt --check` + `cargo clippy`（`-D warnings`）+ `cargo test`（ソースは変更しない）。`cargo test` の 1 回だけ試験の枠の記録を新しい一時ディレクトリへ書かせ、試験の後に集計の表（機材の組み合わせ × ラベルの件数）を標準出力に出して一時ディレクトリを消す。呼び出し側の環境に `FANDHE_SILICON_TEST_RECORD_DIR` が既にあるときは上書きせず、集計も片づけもしない |
| `make build-profiles` | `scripts/build-profiles.sh` | contract crate の feature の組ごとに build / clippy / test、wasm32 は check と `crates/wasm-abi` の 2 版の release ビルド、core crate は feature なしで clippy / test（target は `rust-toolchain.toml` で導入。未導入なら失敗） |
| `make deny` | `scripts/deny.sh` | `cargo deny check`（advisories / bans / licenses / sources）+ workspace の path 依存が `crates/` の外を指さないことの検査 + `measure/`・`baseline-wgpu/`・`wasm/` の cargo-deny と `baseline-wgpu/`・`wasm/` の依存の承認の記録の検査 + 製品の依存グラフの wgpu 系 crate の検査 + 契約の crate のプロファイルごとの依存グラフの検査と陽性対照（jq・python3・git が要る） |
| `make deps-check` | `scripts/deps-check.sh` | 依存の版指定が `=x.y.z` の完全固定で、承認の記録（`deps-approved.toml`）と一致するかを検査（`cargo metadata` + `jq`。ソースは変更しない） |
| `make test-deps-check` | `scripts/test-deps-check.sh` | `deps-check` 自体の回帰テスト（一時 workspace の fixture で版指定・取得元・`[patch]`・承認の記録の各検査を検証。cargo + jq + python3。ネットワーク不要） |
| `make inventory-trace-check` | `scripts/inventory-trace-check.sh` | 一覧の各行の鍵が項目ファイルのどれかに現れるかを検査し（既定はファイル単位。項目の行への帰属は直接実行で `--item-rows` を付けたときだけ）、`--list` で対応の一覧を標準出力へ出す（手元だけ。`docs/spec` の取得が要り、無ければ終了コード 2。CI では実行しない。ソースは変更しない） |
| `make test-inventory-trace-check` | `scripts/test-inventory-trace-check.sh` | `inventory-trace-check` 自体の回帰テスト（合成 fixture で終了コード・標準出力・標準エラーの文言・違反の件数・鍵の秘匿・入力を変更しないことを検証。bash + python3。ネットワーク不要・`docs/spec` の取得は不要。CI の `deny` ジョブも実行する） |
| `make ci-frame-check` | `scripts/ci-frame-check.sh` | `.github/workflows/ci.yml` が CI の枠の事実（集約ジョブの網羅・実行場所・失敗の握りつぶしが無いこと）を保っているかの検査（bash + python3。ソースは変更しない・ネットワーク不要。CI の `deny` ジョブも実行する） |
| `make test-ci-frame-check` | `scripts/test-ci-frame-check.sh` | `ci-frame-check` 自体の回帰テスト（合成 fixture。bash + python3。ネットワーク不要。CI の `deny` ジョブも実行する） |
| `make public-api-check` | `scripts/public-api-check.sh` | 4 crate の公開 API に wgpu 系の crate の型・経路が出ていないことの検査（rustdoc の JSON を数え、走査した公開アイテムの数を標準出力に出す。bash + python3 + cargo + git。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要。CI の `deny` ジョブも実行する） |
| `make measure-verify` | `scripts/measure-verify.sh` | 測定専用の別 workspace（`measure/`）の fmt --check / clippy（-D warnings）/ test / 依存の検査（cargo + jq。ソースは変更しない・ネットワーク不要。CI の deny ジョブも実行する） |
| `make wasm-verify` | `scripts/wasm-verify.sh` | wasm32 向けの別 workspace（`wasm/`）の fmt --check / clippy（ホストと wasm32、-D warnings）/ test / wasm32 の build / 依存の検査（cargo + jq。ソースは変更しない・初回は依存の取得が要る。CI の deny ジョブも実行する） |
| `make wasm-size` | `scripts/wasm-size.sh` | wasm-bindgen の glue を含めた成果物の量（gzip 後）を、ビルド設定 4 つ × 出力先 2 つの 8 組で測って閾値と比べる（bash + python3 + cargo + wasm-bindgen の CLI 0.2.128。欠けた組は not-run で終了コード 1・前提の不足は 2。ネットワーク不要。CI は `wasm-macos` ジョブが呼び、`deny` ジョブは呼ばない）。`scripts/wasm-size.sh --from-dir <dir>` なら生成済みの出力だけを測る |
| `make public-api-webgpu-check` | `scripts/public-api-webgpu-check.sh` | feature `native-blocking-wait` を外した構成で、契約のコア（`crates/core`・`crates/contract`）の公開面にブロッキング待ちの名前の公開アイテムが 0 件であること（内容の (1)）と、パイプライン生成の入力の形（`PipelineInput` の定義・`Wgsl` の variant・生成の関数の入力。内容の (2)）と、拡張 trait の feature `ext-interop` への閉じ込めと submit 系の `wait` 引数の形（内容の (3)）の検査（TASK-241 のポインタ。語は `public-api-webgpu-check.conf`。rustdoc の JSON は `public-api-check.sh --emit-json --without-feature` で作る。bash + python3 + cargo + git。CI の deny ジョブも実行する） |
| `make public-name-check` | `scripts/public-name-check.sh` | 製品の crate の公開アイテムの名前に語の一覧（`public-name-check.conf`）の語が出ていないことの検査（rustdoc の JSON を数え、`--source` でソースの単語・呼び出しの形も数える。bash + python3 + cargo + git。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要。CI の deny ジョブも実行する） |
| `make public-api-sdk-check` | `scripts/public-api-sdk-check.sh` | 製品の 4 crate の公開 API と共通の crate の依存宣言にチップの SDK の crate が無いことの検査（rustdoc の JSON と cargo metadata を数える。`--json` / `--metadata-json` で一部だけの実行も可。JSON を作る段は `public-api-check.sh --emit-json` を共有。bash + python3 + cargo + git。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要。CI の deny ジョブも実行する） |
| `make test-public-api-check` | `scripts/test-public-api-check.sh` | `make public-api-check` の回帰テスト（合成 JSON と、実行時に作る一時 workspace の実物の rustdoc の JSON。bash + python3 + cargo + git。リポジトリのファイルは変更しない・ネットワーク不要。CI の `deny` ジョブも実行する） |
| `make test-public-api-sdk-check` | `scripts/test-public-api-sdk-check.sh` | `scripts/public-api-sdk-check.sh`（`make public-api-sdk-check`。公開 API と共通の crate の依存にチップの SDK の crate が無いことの検査）の回帰テスト（合成 JSON・合成 metadata・偽のコマンドと、実行時に作る一時 workspace の実物の rustdoc の JSON と cargo metadata。bash + python3 + cargo + git。リポジトリのファイルは変更しない・ネットワーク不要。CI の deny ジョブも実行する） |
| `make core-deps-check` | `scripts/core-deps-check.sh` | 共通の crate（`crates/core`）が宣言する外部の crate の一覧が決められた一覧と完全に一致することと、その feature が契約の crate から転送されていることの検査（cargo metadata を読む。`--metadata-json` で差し替え可。bash + python3 + cargo + git。ソースは変更しない・ネットワーク不要。CI の deny ジョブも実行する） |
| `make test-core-deps-check` | `scripts/test-core-deps-check.sh` | `scripts/core-deps-check.sh` の回帰テスト（合成 metadata・偽のコマンド・実行時に作る一時 workspace の実物・公開 API の検査〔`make public-api-sdk-check`〕との突き合わせ。bash + python3 + cargo + git。リポジトリのファイルは変更しない・ネットワーク不要。CI の deny ジョブも実行する） |
| `make dep-graph-wgpu-check` | `scripts/dep-graph-wgpu-check.sh` | 製品の workspace の依存グラフの wgpu 系 crate を狭い定義と広い定義で数える検査。件数を標準出力の `record:` の行に出す（bash + python3 + cargo + git。ソースは変更しない・ネットワーク不要。`make deny` も呼ぶ） |
| `make baseline-positive-control` | `scripts/baseline-positive-control.sh` | 比較基準の別 workspace に依存グラフの検査と公開 API の検査を当てる陽性対照（狭い定義・広い定義・公開 API のそれぞれが 1 件以上で合格・どれか 0 件なら失敗。件数は `record:` の行。bash + python3 + cargo + git。`make deny` には含めず、CI の `deny` ジョブが `make deny` の後に呼ぶ） |
| `make test-baseline-positive-control` | `scripts/test-baseline-positive-control.sh` | `make baseline-positive-control` の回帰テスト（偽の検査で定義ごとの判定を、一時 workspace の実物の出力でも確かめる。python3 + cargo + git・ネットワーク不要。CI の `deny` ジョブも実行する） |
| `make upper-layer-check` | `scripts/upper-layer-check.sh` | 上の層（`crates/upper`）と測定の workspace に、unsafe の不在・チップの crate の不在・直接依存の許可リスト・測定のビルドで検証用の feature が有効でないことの 4 つの検査を当てる（件数は `record:` の行。bash + python3 + cargo + git・ネットワーク不要。`make deny` には含めず、CI の `deny` ジョブが呼ぶ） |
| `make test-upper-layer-check` | `scripts/test-upper-layer-check.sh` | `make upper-layer-check` の回帰テスト（合成の入力・偽のコマンド・一時 workspace の実物の cargo metadata・実物のリポジトリの合格。python3 + cargo + git・ネットワーク不要。CI の `deny` ジョブも実行する） |
| `make test-dep-graph-wgpu-check` | `scripts/test-dep-graph-wgpu-check.sh` | `scripts/dep-graph-wgpu-check.sh` の回帰テスト（合成 JSON・合成 tree と、実行時に作る一時 workspace の実物の cargo metadata / cargo tree。bash + python3 + cargo + git。リポジトリのファイルは変更しない・ネットワーク不要。CI の `deny` ジョブも実行する） |
| `make test-public-api-webgpu-check` | `scripts/test-public-api-webgpu-check.sh` | `make public-api-webgpu-check` の回帰テスト（ダミーの語の合成 JSON と、一時 workspace の実物の rustdoc の JSON。リポジトリを変更しない・ネットワーク不要。bash + python3 + cargo + git。CI の deny ジョブも実行する） |
| `make test-public-name-check` | `scripts/test-public-name-check.sh` | `make public-name-check` の回帰テスト（ダミーの語の合成 JSON・合成ソースと、実行時に作る一時 workspace の実物の rustdoc の JSON。bash + python3 + cargo + git。リポジトリのファイルは変更しない・ネットワーク不要。CI の deny ジョブも実行する） |
| `make test-public-api-diff` | `scripts/test-public-api-diff.sh` | `scripts/public-api-diff.sh`（同じ crate の 2 つの版の rustdoc の JSON から公開 API の差分を出す）の回帰テスト（合成 JSON だけ。bash + python3。cargo・ネットワーク不要。CI の `deny` ジョブも実行する）。本体は比べる JSON のパスが要るため make ターゲットを持たず、`scripts/public-api-diff.sh <基準の JSON> <新しい版の JSON>` を直接実行する（終了コード 0 差分なし・1 差分あり・2 誤用または前提の不足） |
| `make test-rerun-compare` | `scripts/test-rerun-compare.sh` | `scripts/rerun-compare.sh`（試験の枠の記録の部品の 2 つのディレクトリ、前の記録と新しい記録を比べる）の回帰テスト（合成の記録の部品だけ。bash + python3。cargo・ネットワーク不要。CI の `deny` ジョブも実行する）。本体は比べる記録のパスが要るため make ターゲットを持たず、`scripts/rerun-compare.sh <前の記録のディレクトリ> <新しい記録のディレクトリ>` を直接実行する（終了コード 0 差分なし・1 差分あり・2 誤用・前提の不足・読めない記録） |
| `make test-wasm-size` | `scripts/test-wasm-size.sh` | `make wasm-size` の回帰テスト（合成の入力と偽の cargo / wasm-bindgen だけ。bash + python3。CLI・ネットワーク不要。リポジトリのファイルは変更しない。CI の `deny` ジョブも実行する） |
| `make wasm-build-check` | `scripts/wasm-build-check.sh` | wasm32 の cdylib の 2 版の成果物の判別・公開の型の存在・simd128 の `cfg` を行ごとに match / mismatch / not-run で出す（bash + python3 + cargo + wasm32 の target。CI からは呼ばない） |
| `make wasm-backend-check` | `scripts/wasm-backend-check.sh` | 背後実装の feature を付けた wasm32 の cargo check・Vulkan と CUDA のモジュールの除外と Metal のモジュールが空であること・wasm32 の依存グラフのチップの SDK の件数を行ごとに出す（bash + python3 + cargo + wasm32 の target。CI の `deny` ジョブも実行する） |
| `make test-wasm-backend-check` | `scripts/test-wasm-backend-check.sh` | `make wasm-backend-check` の回帰テスト（合成の rustdoc JSON・tree と偽の cargo、一時 workspace の実物の検査と陽性対照。ネットワーク不要。リポジトリのファイルは変更しない。CI の `deny` ジョブも実行する） |
| `make test-wasm-build-check` | `scripts/test-wasm-build-check.sh` | `make wasm-build-check` の回帰テスト（合成の入力・偽の cargo と一時 workspace のダミーの crate。ネットワーク不要。リポジトリのファイルは変更しない。CI の `deny` ジョブも実行する） |
| `make test-session-record-check` | `scripts/test-session-record-check.sh` | `session-record-check` 自体の回帰テスト（合成 fixture。bash + python3。ネットワーク不要）。検査本体は引数が要るため make ターゲットを持たず、`scripts/session-record-check.sh <path>` を直接実行する |
| `make test-perf-record-check` | `scripts/test-perf-record-check.sh` | `perf-record-check` 自体の回帰テスト（合成 fixture。bash + python3。cargo・ネットワーク不要）。検査本体は引数が要るため make ターゲットを持たず、`scripts/perf-record-check.sh <path>` または `--session <回の記録> [<path>...]` を直接実行する |
| `make test-record-summary` | `scripts/test-record-summary.sh` | `record-summary` と `verify.sh` の記録の扱いの回帰テスト（合成の部品と偽の cargo。bash + python3。ネットワーク不要）。集計の本体は引数（部品のディレクトリ）が要るため make ターゲットを持たず、`scripts/record-summary.sh <dir>` を直接実行する。CI の `deny` ジョブが実行し、`test-hooks` ジョブの macOS では標準の bash 3.2 でも直接実行する |
| `make test-metal-session` | `scripts/test-metal-session.sh` | `metal-session` 自体の回帰テスト（macOS のコマンドは偽のコマンドで差し替えるので Linux でも流れる。bash + python3 + git。ネットワーク不要）。本体はサブコマンド（`probe` / `run` / `redact` / `scan`）と引数が要るため make ターゲットを持たず、`scripts/metal-session.sh` を直接実行する。CI の `deny` ジョブと `test-hooks` ジョブ（macOS を含む）も実行する |
| `make test-hooks` | `scripts/hooks/test-hooks.sh` | フックスクリプト自体の回帰テスト（一時 git リポジトリで検出・許可ケースを検証。bash + git のみ） |

整形は `cargo fmt --all` で行います（`make verify` は差分の検出のみ）。

## Git hooks

`make setup` 後、以下が自動実行されます。

- **pre-commit**
  - 簡易シークレット検知（`.env` 系ファイルの追加、トークン形式・秘密鍵・認証情報入り URL・ハードコード値）。
    保守的なヒューリスティックであり、網羅的なスキャナの代替ではありません
  - staged ファイルの editorconfig-checker
  - staged の `*.sh` に対する shellcheck
  - staged の `*.rs` に対する `rustfmt --check`
- **commit-msg**: [Conventional Commits](https://www.conventionalcommits.org/ja/) 形式の検証
  （`<type>[(<scope>)][!]: <要約>`、type は `feat` `fix` `docs` `style` `refactor` `perf` `test` `build` `ci` `chore` `revert`）

hooks に引っかかった場合は原因を修正してから再コミットしてください。`--no-verify` によるバイパスは行いません。

開発速度を落とさないため、hooks には staged ファイル限定で一瞬で終わる検査と、コミット単位でしか意味がない検査
（シークレット検知・コミットメッセージ）だけを置きます。clippy・test・cargo-deny など重い処理や CI と重複する検査は
CI（`.github/workflows/ci.yml`）で行います。手元で CI 相当を確認する場合は `make check` / `make verify` / `make deny` / `make deps-check` / `make test-deps-check` / `make test-inventory-trace-check` / `make test-session-record-check` / `make test-perf-record-check` / `make test-metal-session` / `make test-record-summary` / `make ci-frame-check` / `make test-ci-frame-check` を実行してください。
時間予算（pre-commit 合計 1 秒未満）と判断手順は `.claude/rules/git-hooks.md` に定義しています。

フックスクリプト（`scripts/hooks/*.sh`）自体のテストもフックには置かず、CI の `test-hooks` ジョブ（`make test-hooks`）で
Linux・macOS（標準の bash 3.2 を含む）について実行します。`scripts/hooks/` を変更したらローカルでも `make test-hooks` を実行してください。

## CI

| ワークフロー | 内容 |
|---|---|
| `ci.yml` | `check`: ローカル・hooks と同じ `make check` を実行する（editorconfig-checker / shellcheck はバージョン固定 + SHA256 検証で導入）。`verify` / `deny`: ローカルと同じ `make verify` / `make deny` を実行する（`verify` は試験の枠の記録から集計の表をジョブのログに出し〔記録のファイルは残さず、artifact にも PR の Summary にも出さない〕、続けて `make build-profiles` を、`deny` は続けて `make deps-check` / `make test-deps-check` / `make test-inventory-trace-check` / `make test-session-record-check` / `make test-metal-session` / `make test-record-summary` / `make ci-frame-check` / `make test-ci-frame-check` を実行する。jq・python3 は runner 同梱。toolchain は `rust-toolchain.toml`、cargo-deny はバージョン固定 + SHA256 検証で導入）。`wasm-macos`: macOS arm64 で `make wasm-verify` / `make wasm-size` / `make wasm-node-check` を実行する（Node と wasm-bindgen の CLI はバージョン固定 + SHA256 検証で導入。キャッシュなし）。`pr-title`: PR タイトルを commit-msg フックと同じスクリプトで検証する（squash merge でコミット件名になるため）。`test-hooks`: フックスクリプト自体の回帰テスト（`make test-hooks`。ubuntu-latest・macos-latest。あわせて `make test-session-record-check` / `make test-metal-session` も流し、macOS では標準の bash 3.2 でも直接実行する。macOS ではあわせて `scripts/test-record-summary.sh` も標準の bash 3.2 で直接実行する）。`ci-complete`: 全ジョブ結果の集約 |
| `ai-review.yml` | Fandhe-AI/actions の ai-review（codex）による PR 自動レビュー。Actions variable `CODEX_HOME_DIR` が未設定の間は skip される |
| `update-external.yml` | エージェントスキル（`skills-lock.json`）と submodule（`.gitmodules`）の日次自動追従 PR。secrets は org の `SUBMODULE_PAT` を使う（`SKILLS_PAT` 未登録時は共通側が `SUBMODULE_PAT` へフォールバック）。作成する PR には `dependencies` / `automated` ラベルが付く |

- `main` には ruleset `main-protection` を適用している（`setup-repo-guards` スキルの手順で導入）
  - 削除・force push の禁止、PR 必須（承認数 0・レビュースレッドの resolve 必須・squash merge のみ）
  - required status checks（`strict` は無効。`Cursor Bugbot` は cursor App、それ以外は GitHub Actions の発行に束縛）:
    `ci-complete`・`codex / preflight`・`codex / review`・`codex / post_feedback`・`Cursor Bugbot` と、
    `ci.yml` の個別ジョブ（`check` / `verify` / `deny` / `test-hooks`（ubuntu-latest・macos-latest）/ `pr-title` / `wasm-macos`〔check 名 `wasm32 build + size + node (macos-latest)`〕）の check 名。
    個別ジョブも登録するのは、implement-issue-tree の自動マージ前ゲートが PR 上の全 check を必須チェックに含めることを要求するため
    （`ci-complete` の集約は従来どおり維持する）。codex のレビュー基準は `AGENTS.md`
- マージ設定は squash のみ（コミット件名 = PR タイトル、本文 = PR 本文）・マージ後のブランチ自動削除
- CI にジョブを追加したら `ci-complete` の `needs` にも必ず追加する

## MCP サーバー

`.mcp.json` はベースとして空（`"mcpServers": {}`）にしてある。派生リポジトリで必要なサーバーを追加する。
API キー等は値を直接書かず `"${EXAMPLE_API_KEY:-}"` のように環境変数参照とし、実値は `.env`（direnv 経由）に置く。

## 環境変数

- 雛形は `.env.example`（キー名と空値・ダミー値のみ）、実値は `.env`（git 管理外）に置きます
- direnv を導入している場合、`.envrc` が `.env` をシェルへ読み込みます
- 個人用の追加設定は `.envrc.local`（git 管理外）に置けます

## 派生リポジトリでの拡張

1. **言語固有のコマンド**: `scripts/` に処理（例: `scripts/verify.sh`）を追加し、`Makefile` に 1 行で呼ぶターゲットと `## ` コメントを足し、`scripts/help.sh` の一覧も更新する（中身の無いターゲット `@true` 等は置かない）
2. **`make check` の拡張**: `scripts/check.sh` に fmt チェック・lint 等を追記する（自動修正は別ターゲットに分ける）
3. **hooks の追加**: `.claude/rules/git-hooks.md` の時間予算と判断手順に従う。staged 限定で 1 秒未満の検査のみ `lefthook.yml` に job を追加し（実体が長くなる場合は `scripts/hooks/` へ切り出す）、テスト・ビルド等は `ci.yml` にジョブを追加して `ci-complete` の `needs` にも加える
4. **EditorConfig**: 言語ごとのセクション（例: `[*.rs] indent_size = 4`）を `.editorconfig` に追加する
5. **`.gitignore`**: ビルド成果物（`/target`・`/dist`・`node_modules/` 等）を追加する
