# 開発用 Makefile。変更時は .claude/rules/makefile.md に従う。
#
# Make は scripts/*.sh を呼ぶだけの薄い入口であり、処理の定義は scripts/ 側にのみ置く
# （各レシピは 1 行の呼び出しに限定し、ここでチェック内容を二重定義しない）。
# Make を使わない場合の同等コマンドは scripts/help.sh を参照。
#
# GNU Make 3.81（macOS 標準）で動作確認済み。3.82+ 専用機能（.ONESHELL・.RECIPEPREFIX 等）は使わない。
#
# 使い方: `make help`（既定ターゲット）でターゲット一覧を表示する。
# 一覧は下記の `## ` コメントから生成されるため、`## ` の無いターゲットは表示されない。
# ターゲットを追加・改名・削除したら scripts/help.sh も同じ変更で更新すること。
#
# Rust の品質ゲートは verify（fmt --check / clippy / test）と deny（cargo-deny）に分ける。
# 中身の無いターゲット（`@true` 等）は呼び出し元が終了コードを信用してしまうため置かない。

.DEFAULT_GOAL := help

.PHONY: help public-api-webgpu-check test-public-api-webgpu-check doctor setup check verify build-profiles deny baseline-positive-control test-baseline-positive-control upper-layer-check test-upper-layer-check dep-graph-wgpu-check profile-dep-graph-check test-profile-dep-graph-check profile-dep-graph-positive-control test-profile-dep-graph-positive-control measure-verify wasm-verify wasm-size test-wasm-size wasm-build-check test-wasm-build-check wasm-backend-check test-wasm-backend-check deps-check test-deps-check inventory-trace-check test-inventory-trace-check test-session-record-check test-perf-record-check test-metal-session test-record-summary ci-frame-check test-ci-frame-check public-api-check test-public-api-check public-api-sdk-check test-public-api-sdk-check core-deps-check test-core-deps-check test-dep-graph-wgpu-check public-name-check test-public-name-check test-public-api-diff test-rerun-compare wasm-node-check test-wasm-node-check test-hooks

help: ## このヘルプを表示する（Make 非依存の一覧: scripts/help.sh）
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z0-9_-]+:.*## /{printf "  \033[36mmake %-10s\033[0m %s\n", $$1, $$2}' $(firstword $(MAKEFILE_LIST))

doctor: ## 開発環境を診断する（読み取りのみ・何も導入しない）。直接: scripts/doctor.sh
	@scripts/doctor.sh

setup: ## Git hooks を有効化し .env を雛形から作成する（再実行可）。直接: scripts/setup.sh
	@scripts/setup.sh

check: ## editorconfig-checker + shellcheck（ソースは変更しない）。直接: scripts/check.sh
	@scripts/check.sh

verify: ## cargo fmt --check + clippy（-D warnings）+ test（ソースは変更しない。試験の枠の記録から集計の表を標準出力に出す）。直接: scripts/verify.sh
	@scripts/verify.sh

build-profiles: ## feature の組ごとの build / clippy / test と wasm32 の check・wasm-abi の 2 版の release ビルド（ソースは変更しない）。直接: scripts/build-profiles.sh
	@scripts/build-profiles.sh

deny: ## cargo deny check（advisories / bans / licenses / sources）+ path 検査 + 依存グラフの wgpu 系の検査（dep-graph-wgpu-check。python3・git も要る）+ 契約の crate のプロファイルごとの依存グラフの検査と陽性対照（profile-dep-graph-check・profile-dep-graph-positive-control）+ 別 workspace の監査。直接: scripts/deny.sh
	@scripts/deny.sh

measure-verify: ## 測定専用の別 workspace（measure/）の fmt --check / clippy（-D warnings）/ test / 依存の検査（cargo + jq。ソースは変更しない・ネットワーク不要・CI の deny ジョブも実行する）。直接: scripts/measure-verify.sh
	@scripts/measure-verify.sh

wasm-verify: ## wasm32 向けの別 workspace（wasm/）の fmt --check / clippy（ホストと wasm32、-D warnings）/ test / wasm32 の build / 依存の検査（cargo + jq。ソースは変更しない・初回は依存の取得が要る・CI の deny ジョブも実行する）。直接: scripts/wasm-verify.sh
	@scripts/wasm-verify.sh

wasm-size: ## wasm-bindgen の glue を含めた成果物の量（gzip 後）を 8 組で測って閾値と比べる（bash + python3 + cargo + wasm-bindgen の CLI 0.2.128。欠けた組は not-run で終了コード 1・前提の不足は 2・ネットワーク不要。手元と CI の wasm-macos ジョブ用で CI の deny ジョブは呼ばない）。直接: scripts/wasm-size.sh [--from-dir <dir>]
	@scripts/wasm-size.sh

deps-check: ## 依存の版指定が =x.y.z の完全固定で、承認の記録（deps-approved.toml）と一致するかを検査する（cargo metadata + jq。ソースは変更しない）。直接: scripts/deps-check.sh
	@scripts/deps-check.sh

test-deps-check: ## scripts/deps-check.sh 自体の回帰テスト（一時 workspace の fixture。cargo + jq + python3。ネットワーク不要）。直接: scripts/test-deps-check.sh
	@scripts/test-deps-check.sh

inventory-trace-check: ## 一覧の鍵が項目ファイルに現れるかを検査する（ファイル単位。行単位は直接実行で --item-rows。要 private submodule の取得。CI では実行しない。ソースは変更しない）。直接: scripts/inventory-trace-check.sh
	@scripts/inventory-trace-check.sh

test-inventory-trace-check: ## scripts/inventory-trace-check.sh 自体の回帰テスト（合成 fixture。bash + python3。ネットワーク不要・docs/spec の取得は不要）。直接: scripts/test-inventory-trace-check.sh
	@scripts/test-inventory-trace-check.sh

test-session-record-check: ## scripts/session-record-check.sh 自体の回帰テスト（合成 fixture。bash + python3。ネットワーク不要）。検査本体は引数が要るので make ターゲットを持たない（直接: scripts/session-record-check.sh <path>）。直接: scripts/test-session-record-check.sh
	@scripts/test-session-record-check.sh

test-perf-record-check: ## scripts/perf-record-check.sh 自体の回帰テスト（合成 fixture。bash + python3。cargo・ネットワーク不要）。検査本体は引数が要るので make ターゲットを持たない（直接: scripts/perf-record-check.sh <path>）。直接: scripts/test-perf-record-check.sh
	@scripts/test-perf-record-check.sh

test-metal-session: ## scripts/metal-session.sh 自体の回帰テスト（macOS のコマンドは偽のコマンドで差し替える。bash + python3 + git。ネットワーク不要・Linux でも流れる）。本体はサブコマンドと引数が要るので make ターゲットを持たない（直接: scripts/metal-session.sh <probe|run|redact|scan>）。直接: scripts/test-metal-session.sh
	@scripts/test-metal-session.sh

wasm-node-check: ## wasm32 の .wasm を wasm-bindgen の glue 越しに Node で呼び、結果を参照と照合して行ごとに match / mismatch / not-run を出す土台（Node v24.13.0 と wasm-bindgen の CLI が要る実物の実行。CI の wasm-macos ジョブが呼ぶ）。直接: scripts/wasm-node-check.sh [--wasm <path>] [--cases <path>] [--record-dir <dir> --record-combination <name>]（書き出しは選んだときだけ。選ばなければ出力と終了コードは不変）
	@scripts/wasm-node-check.sh

test-wasm-node-check: ## scripts/wasm-node-check.sh 自体の回帰テスト（偽の node・wasm-bindgen・cargo と合成の結果。run-glue.cjs は本物の node で偽の glue を読ませる。bash + python3 + node。ネットワーク不要）。直接: scripts/test-wasm-node-check.sh
	@scripts/test-wasm-node-check.sh

test-record-summary: ## scripts/record-summary.sh（試験の枠の記録の部品から集計の表を出す）と scripts/verify.sh の記録の扱いの回帰テスト（合成の部品と偽の cargo。bash + python3。ネットワーク不要）。集計の本体は引数（部品のディレクトリ）が要るので make ターゲットを持たない（直接: scripts/record-summary.sh <dir>）。直接: scripts/test-record-summary.sh
	@scripts/test-record-summary.sh

ci-frame-check: ## .github/workflows/ci.yml が CI の枠の事実（集約・実行場所・失敗の扱い）を保っているかを検査する（bash + python3。ソースは変更しない・ネットワーク不要）。直接: scripts/ci-frame-check.sh [--workflow <path>]
	@scripts/ci-frame-check.sh

test-ci-frame-check: ## scripts/ci-frame-check.sh 自体の回帰テスト（合成 fixture。bash + python3。ネットワーク不要）。直接: scripts/test-ci-frame-check.sh
	@scripts/test-ci-frame-check.sh

public-api-check: ## 4 crate の公開 API に wgpu 系（wgpu*・naga）の crate の型・経路が出ていないことを、rustdoc の JSON から数えて検査し、走査した公開アイテムの数を標準出力に出す（bash + python3 + cargo + git。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要・CI の deny ジョブも実行する）。直接: scripts/public-api-check.sh [--json <path>...]
	@scripts/public-api-check.sh

public-name-check: ## 製品の crate の公開アイテムの名前に語の一覧（public-name-check.conf）の語が出ていないことを、rustdoc の JSON から数えて検査する。--source でソースの単語・呼び出しの形も数える（bash + python3 + cargo + git。JSON は public-api-check.sh --emit-json で作る。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要・CI の deny ジョブも実行する）。直接: scripts/public-name-check.sh --config public-name-check.conf [--json <path>...] [--source <path>...]
	@scripts/public-name-check.sh --config public-name-check.conf

public-api-webgpu-check: ## feature native-blocking-wait を外した構成で、契約のコアの公開面にブロッキング待ちの名前の公開アイテムが無いこと（内容の (1)）と、パイプライン生成の入力の形（PipelineInput の定義・Wgsl の variant・生成の関数の入力。内容の (2)）と、拡張 trait の feature ext-interop への閉じ込めと submit 系の wait 引数の形（内容の (3)）を、rustdoc の JSON から検査する（TASK-241。語は public-api-webgpu-check.conf。bash + python3 + cargo + git。JSON は public-api-check.sh --emit-json --without-feature で外す feature ごとに 2 回作る。CI の deny ジョブも実行する）
	@scripts/public-api-webgpu-check.sh --config public-api-webgpu-check.conf

test-public-api-webgpu-check: ## public-api-webgpu-check の回帰テスト（ダミーの語の一覧の合成 JSON と、一時 workspace の実物の rustdoc の JSON。TASK-241。bash + python3 + cargo + git。CI の deny ジョブも実行する）
	@scripts/test-public-api-webgpu-check.sh

test-public-name-check: ## public-name-check の回帰テスト（ダミーの語の一覧の合成 JSON・合成ソースと、一時 workspace の実物の rustdoc の JSON。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-public-name-check.sh
	@scripts/test-public-name-check.sh

test-public-api-diff: ## public-api-diff の回帰テスト（合成 JSON だけ。bash + python3・cargo もネットワークも不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。本体は比べる JSON の引数が要るため make ターゲットを持たない。直接: scripts/test-public-api-diff.sh / scripts/public-api-diff.sh <基準> <新>
	@scripts/test-public-api-diff.sh

test-rerun-compare: ## rerun-compare の回帰テスト（合成の記録の部品だけ。bash + python3・cargo もネットワークも不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。本体は比べる記録のディレクトリの引数が要るため make ターゲットを持たない。直接: scripts/test-rerun-compare.sh / scripts/rerun-compare.sh <前の記録> <新しい記録>
	@scripts/test-rerun-compare.sh

test-public-api-check: ## public-api-check の回帰テスト（合成 JSON と、実行時に作る一時 workspace の実物の rustdoc の JSON。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-public-api-check.sh
	@scripts/test-public-api-check.sh

public-api-sdk-check: ## 製品の 4 crate の公開 API と共通の crate（crates/core）の依存宣言に、チップの SDK の crate が無いことを rustdoc の JSON と cargo metadata から数えて検査する（bash + python3 + cargo + git。JSON は public-api-check.sh --emit-json で作る。専用の target ディレクトリに出力しソースは変更しない・ネットワーク不要・CI の deny ジョブも実行する）。直接: scripts/public-api-sdk-check.sh [--json <path>...] [--metadata-json <path>]
	@scripts/public-api-sdk-check.sh

test-public-api-sdk-check: ## public-api-sdk-check の回帰テスト（合成 JSON・合成 metadata・偽のコマンドと、実行時に作る一時 workspace の実物の rustdoc の JSON と cargo metadata。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-public-api-sdk-check.sh
	@scripts/test-public-api-sdk-check.sh

core-deps-check: ## 共通の crate（crates/core）が宣言する外部の crate の一覧が決められた一覧と完全に一致することと、その feature が契約の crate（crates/contract）から転送されていることを cargo metadata から検査する（bash + python3 + cargo + git・ソースは変更しない・ネットワーク不要・CI の deny ジョブも実行する）。直接: scripts/core-deps-check.sh [--metadata-json <path>]
	@scripts/core-deps-check.sh

test-core-deps-check: ## core-deps-check の回帰テスト（合成 metadata・偽のコマンドと、実行時に作る一時 workspace の実物の cargo metadata。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-core-deps-check.sh
	@scripts/test-core-deps-check.sh

dep-graph-wgpu-check: ## 製品の workspace の依存グラフの wgpu 系 crate を狭い定義と広い定義で数えて検査し、件数を record: の行に出す（bash + python3 + cargo + git。ソースは変更しない・ネットワーク不要。make deny も呼ぶ）。直接: scripts/dep-graph-wgpu-check.sh
	@scripts/dep-graph-wgpu-check.sh

test-dep-graph-wgpu-check: ## dep-graph-wgpu-check の回帰テスト（合成 JSON・合成 tree と、実行時に作る一時 workspace の実物の cargo metadata / cargo tree。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-dep-graph-wgpu-check.sh
	@scripts/test-dep-graph-wgpu-check.sh

profile-dep-graph-check: ## 契約の crate のプロファイルごとの依存グラフ（CpuOnly・ComputeOnly の Metal・Graphics の Vulkan の組・Graphics と相互運用の組。ComputeOnly の Metal の組は objc2-metal の per-header feature も数え、Graphics の Vulkan の組は ash・libloading だけを許し、Graphics と相互運用の組は ash・cudarc・libloading だけを許す）で判定に使う crate 名を数えて検査し、件数を record: / count: の行に出す（bash + python3 + cargo + git。ソースは変更しない・ネットワーク不要。make deny も呼ぶ）。直接: scripts/profile-dep-graph-check.sh
	@scripts/profile-dep-graph-check.sh

test-profile-dep-graph-check: ## profile-dep-graph-check の回帰テスト（合成 tree・偽の cargo / git と、実行時に作る一時 workspace の実物の cargo tree。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない）。直接: scripts/test-profile-dep-graph-check.sh
	@scripts/test-profile-dep-graph-check.sh

profile-dep-graph-positive-control: ## profile-dep-graph-check の陽性対照。判定の名前の一覧に実物の依存グラフへ載っている名前を 1 件足して流し、全プロファイルで違反が出る（終了コード 1・1 件以上）ことを確かめる（bash + python3 + cargo + git。ソースと Cargo.lock は変更しない・ネットワーク不要。make deny も呼ぶ）。直接: scripts/profile-dep-graph-positive-control.sh
	@scripts/profile-dep-graph-positive-control.sh

test-profile-dep-graph-positive-control: ## profile-dep-graph-positive-control の回帰テスト（偽の内側の検査で判定と fail-closed を、実物のリポジトリでも確かめる。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない）。直接: scripts/test-profile-dep-graph-positive-control.sh
	@scripts/test-profile-dep-graph-positive-control.sh

baseline-positive-control: ## 比較基準の別 workspace（baseline-wgpu/）に依存グラフの検査と公開 API の検査を当てる陽性対照。狭い定義・広い定義・公開 API のそれぞれが 1 件以上で合格・どれか 0 件なら失敗・読めない入力や誤用や前提の不足は終了コード 2。件数は record: の行に残る（bash + python3 + cargo + git。ソースは変更しない・依存の取得後に流す。make deny には含めない・CI の deny ジョブが make deny の後に呼ぶ）。直接: scripts/baseline-positive-control.sh
	@scripts/baseline-positive-control.sh

test-baseline-positive-control: ## baseline-positive-control の回帰テスト（偽の検査で定義ごとの判定を、一時 workspace の実物の出力でも確かめる。python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-baseline-positive-control.sh
	@scripts/test-baseline-positive-control.sh

upper-layer-check: ## 上の層（crates/upper）と測定の workspace（measure/）に、unsafe の不在・チップの crate の不在・直接依存の許可リスト・測定のビルドで検証用の feature が有効でないことの 4 つの検査を当てる。合格は終了コード 0 と ok: の行・違反は 1・誤用や前提の不足は 2。件数は record: の行に残る（bash + python3 + cargo + git。ソースは変更しない・ネットワーク不要。make deny には含めない・CI の deny ジョブが呼ぶ）。直接: scripts/upper-layer-check.sh
	@scripts/upper-layer-check.sh

test-upper-layer-check: ## upper-layer-check の回帰テスト（合成の入力・ツールが無いとき・偽のコマンドと、一時 workspace の実物の cargo metadata・実物のリポジトリの合格。python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-upper-layer-check.sh
	@scripts/test-upper-layer-check.sh

wasm-build-check: ## wasm32 の cdylib の 2 版の成果物の判別（target_features）・公開の型の存在・simd128 がビルド時の cfg で決まることを行ごとに match / mismatch / not-run で出す（bash + python3 + cargo + wasm32 の target。終了コード 0 全行 match・1 mismatch か not-run・2 誤用か前提の不足。CI からは呼ばない）。直接: scripts/wasm-build-check.sh [--require-type <path>]...
	@scripts/wasm-build-check.sh

wasm-backend-check: ## 背後実装の feature を付けた wasm32 向けの契約の crate の cargo check・Vulkan と CUDA のモジュールが外れ Metal のモジュールが公開のアイテムを持たないこと・wasm32 の依存グラフのチップの SDK の件数を行ごとに出す（bash + python3 + cargo + wasm32 の target。終了コード 0 全行 match・1 mismatch・2 誤用か前提の不足。CI の deny ジョブも実行する）
	@scripts/wasm-backend-check.sh

test-wasm-backend-check: ## wasm-backend-check の回帰テスト（合成の rustdoc JSON・tree と偽の cargo、一時 workspace の実物の検査と陽性対照。bash + python3 + cargo + git + wasm32 の target・ネットワーク不要・リポジトリのファイルは変更しない。CI の deny ジョブも実行する）。直接: scripts/test-wasm-backend-check.sh
	@scripts/test-wasm-backend-check.sh

test-wasm-build-check: ## wasm-build-check の回帰テスト（合成の wasm・rustdoc JSON・ソースと偽の cargo、一時 workspace の実物のビルド。bash + python3 + cargo + wasm32 の target・ネットワーク不要・リポジトリのファイルは変更しない。CI の deny ジョブも実行する）。直接: scripts/test-wasm-build-check.sh
	@scripts/test-wasm-build-check.sh

test-wasm-size: ## wasm-size の回帰テスト（合成の入力と偽の cargo / wasm-bindgen だけ。bash + python3・CLI もネットワークも不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-wasm-size.sh
	@scripts/test-wasm-size.sh

test-hooks: ## scripts/hooks/ のフックスクリプト自体の回帰テスト（bash + git のみ）。直接: scripts/hooks/test-hooks.sh
	@scripts/hooks/test-hooks.sh
