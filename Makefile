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

.PHONY: help doctor setup check verify build-profiles deny measure-verify wasm-verify deps-check test-deps-check inventory-trace-check test-inventory-trace-check test-session-record-check test-perf-record-check test-metal-session test-record-summary ci-frame-check test-ci-frame-check public-api-check test-public-api-check public-name-check test-public-name-check test-public-api-diff test-hooks

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

build-profiles: ## feature の組ごとの build / clippy / test と wasm32 の check（ソースは変更しない）。直接: scripts/build-profiles.sh
	@scripts/build-profiles.sh

deny: ## cargo deny check（advisories / bans / licenses / sources）。直接: scripts/deny.sh
	@scripts/deny.sh

measure-verify: ## 測定専用の別 workspace（measure/）の fmt --check / clippy（-D warnings）/ test / 依存の検査（cargo + jq。ソースは変更しない・ネットワーク不要・CI の deny ジョブも実行する）。直接: scripts/measure-verify.sh
	@scripts/measure-verify.sh

wasm-verify: ## wasm32 向けの別 workspace（wasm/）の fmt --check / clippy（ホストと wasm32、-D warnings）/ test / wasm32 の build / 依存の検査（cargo + jq。ソースは変更しない・初回は依存の取得が要る・CI の deny ジョブも実行する）。直接: scripts/wasm-verify.sh
	@scripts/wasm-verify.sh

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

test-public-name-check: ## public-name-check の回帰テスト（ダミーの語の一覧の合成 JSON・合成ソースと、一時 workspace の実物の rustdoc の JSON。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-public-name-check.sh
	@scripts/test-public-name-check.sh

test-public-api-diff: ## public-api-diff の回帰テスト（合成 JSON だけ。bash + python3・cargo もネットワークも不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。本体は比べる JSON の引数が要るため make ターゲットを持たない。直接: scripts/test-public-api-diff.sh / scripts/public-api-diff.sh <基準> <新>
	@scripts/test-public-api-diff.sh

test-public-api-check: ## public-api-check の回帰テスト（合成 JSON と、実行時に作る一時 workspace の実物の rustdoc の JSON。bash + python3 + cargo + git・ネットワーク不要・リポジトリのファイルは変更しない・CI の deny ジョブも実行する）。直接: scripts/test-public-api-check.sh
	@scripts/test-public-api-check.sh

test-hooks: ## scripts/hooks/ のフックスクリプト自体の回帰テスト（bash + git のみ）。直接: scripts/hooks/test-hooks.sh
	@scripts/hooks/test-hooks.sh
