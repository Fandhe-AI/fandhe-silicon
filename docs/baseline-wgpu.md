# 比較基準の wgpu の置き場（TASK-7）の所在と現状の事実

比較基準の wgpu を基盤の外の別 workspace に置く仕組みについて、本リポのファイルと検査から説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-7。関連: AC-2・BUILD-50・D-18。ポインタのみ）。spec の本文・値は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
版の値は、本リポの `baseline-wgpu/` の manifest・`Cargo.lock` から取ったものだけを書く。

## 製品 workspace を守る仕組み

| 仕組み | 事実 | 確かめ方 |
| ------ | ---- | -------- |
| wgpu 系・naga の禁止 | `deny.toml` の `[bans] deny` が `wgpu`・`wgpu-core`・`wgpu-hal`・`wgpu-types`・`naga` を拒否する | `deny.toml`、`make deny` |
| 全 feature の監査 | `deny.toml` の `[graph] all-features = true`。target は指定しないので全 target が対象 | `deny.toml` |
| 別 workspace の切り離し | ルートの `Cargo.toml` の `exclude = ["baseline-wgpu", "measure", "wasm"]`（`baseline-wgpu` が本書の対象。`measure` は測定専用の別 workspace、`wasm` は wasm32 向けの別 workspace）。製品の workspace の member は `crates/` の 4 crate | ルート `Cargo.toml` |
| 別 workspace の cargo-deny | `make deny` は `measure/` にルートの `deny.toml`、`baseline-wgpu/` に `baseline-wgpu/deny.toml` を `--config` で明示して `advisories bans licenses sources` をかける | `scripts/deny.sh` |
| 依存グラフの wgpu 系の検査 | `make deny` は、製品の workspace の依存グラフの wgpu 系 crate を狭い定義と広い定義で数える検査を呼ぶ（`baseline-wgpu/` には当てない）。検査のスクリプトには `--manifest-path` で本 workspace に当てる口がある | `scripts/deny.sh`、`scripts/dep-graph-wgpu-check.sh` |
| path の検査 | `make deny` は、workspace の依存グラフ（全 feature・全 target）の path が `crates/` の外を指していないことを検査する | `scripts/deny.sh` |
| 製品の依存グラフ | パッケージ 4・wgpu 系 0 件 | `cargo metadata --format-version 1 --all-features --locked` |

境界の案内は [workspace-boundary](workspace-boundary.md) を参照。

## 別 workspace の配置と骨格

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | 本リポ内の非 member ディレクトリ `baseline-wgpu/`。自身の `[workspace]` と `Cargo.lock` を持つ | `baseline-wgpu/Cargo.toml` |
| member | `crates/smoke`（`baseline-wgpu-smoke`）と `crates/public-api`（`baseline-wgpu-public-api`）の 2 つ | `baseline-wgpu/Cargo.toml` |
| 外部依存 | `wgpu = "=30.0.1"`（完全固定）のみ | 両 member の `Cargo.toml` |
| lints | `unsafe_code = "forbid"`・`missing_docs = "warn"` | `baseline-wgpu/Cargo.toml` |
| 公開 API の lib | `baseline-wgpu-public-api` は戻り値に `wgpu::Backends` を出す関数を 1 つ公開する。製品の公開 API の検査を `scripts/public-api-check.sh --manifest-path baseline-wgpu/Cargo.toml` で当てる入力 | `baseline-wgpu/crates/public-api/src/lib.rs` |
| smoke bin | `wgpu::Instance` で全 backend のアダプタを列挙し、名前・backend・device type を標準出力へ出す。アダプタが 0 件のときは `0 adapter(s)` を出して正常終了する | `baseline-wgpu/crates/smoke/src/main.rs` |
| 検査の範囲 | `make verify`・`make check` はルートの workspace が対象で、本 workspace は範囲外。`make deny` は本 workspace に cargo-deny（設定は `baseline-wgpu/deny.toml`。ルートの `deny.toml` と違うのは wgpu 系の禁止を外した点だけ）と依存の承認の記録の検査をかけ、CI の `deny` ジョブも流す。ビルド・実行は範囲外 | `baseline-wgpu/deny.toml`、`scripts/deny.sh` |
| 依存の承認の記録 | `baseline-wgpu/deps-approved.toml`（ルートの `deps-approved.toml` には入れない）。検査は `make deny` が `scripts/deps-check.sh baseline-wgpu/Cargo.toml` を呼ぶ | `baseline-wgpu/deps-approved.toml` |
| 実行 | `cargo run --locked --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-smoke` | [baseline-wgpu/README.md](../baseline-wgpu/README.md) |

依存の追加・更新には、改めてユーザー承認が要る（[dependency-policy](../.claude/rules/dependency-policy.md)）。

## 版の記録

[wgpu-baseline-record](wgpu-baseline-record.md) の書式に沿った、本リポの実物から取った値（`basis` は run の設定から取る欄で、版の置き場には run が無いため空欄）。

| 欄 | 値 | 取り方 |
| -- | -- | ------ |
| `wgpu_baseline.version` | `30.0.1` | `baseline-wgpu/Cargo.lock` の `wgpu` の `version` |
| `wgpu_baseline.source` | `crates-io` | 同 `source` が crates.io の registry |
| `wgpu_baseline.lockfile_sha256` | `2b42260a29815b8911273102b65a09b93d49e69d9af59b2add487dea2437f896` | `sha256sum baseline-wgpu/Cargo.lock` |
| `wgpu_baseline.basis` | 未記入 | run の設定から取る欄 |

- `source` が `crates-io` のため、`git_repository`・`git_tag`・`git_commit` の欄は使わない
- `Cargo.lock` を更新したら `lockfile_sha256` の値も直す

## 確かめ方

worktree のルートで実行する。

- `make deny`: wgpu 系の禁止、path の検査、製品の依存グラフの wgpu 系の検査
- `cargo metadata --format-version 1 --all-features --locked`: 製品の依存グラフのパッケージ数と wgpu 系の有無
- `scripts/deps-check.sh baseline-wgpu/Cargo.toml`: 別 workspace の依存の固定と承認の記録の一致
- `sha256sum baseline-wgpu/Cargo.lock`: 版の記録の値との一致
- `cargo run --locked --manifest-path baseline-wgpu/Cargo.toml -p baseline-wgpu-smoke`: 別 workspace の bin のビルドと実行（ネットワークと実行環境が要る）
- `make verify`・`make check`・`make deps-check`: 製品側の品質ゲート

## 関連

[workspace-boundary](workspace-boundary.md)・[wgpu-baseline-record](wgpu-baseline-record.md)・[measurement-record](measurement-record.md)・[poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)
