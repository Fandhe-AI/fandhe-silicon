# 公開 API に wgpu の型が無いことの検査（TASK-10）の現状と前提

親 Issue #363（TASK-10）の 1/6（#3390）として、着手時点の本リポの現状と前提を確かめた記録。
公開済みの事実とポインタだけを書き、spec の本文・決定の内容は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-10 と、そこから辿る決定の記録）
- `docs/spec` の gitlink は `267f826`。更新は `update-external.yml` の PR 経由で、本作業では進めない
- 完了の条件との対応づけの記録は spec 側が正で、本リポには置かない

## 前提タスクの状態

| 前提 | 状態 | 根拠（本リポ側で観測できるもの） |
| ---- | ---- | -------------------------------- |
| TASK-4 | 済 | `cargo metadata --format-version 1 --all-features --locked` で workspace member 4・パッケージ 4（外部依存 0） |
| TASK-112 | 公開側は済 | #393 は CLOSED。[ci-device-free-jobs](ci-device-free-jobs.md) と `make ci-frame-check` が在る |

## 着手時点の本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | `crates/` 配下の 4 crate。全 crate が `publish = false`（`publish.workspace = true`） | `cargo metadata`、`grep -n publish crates/*/Cargo.toml` |
| 公開面 | `contract` は feature ゲート付きの `pub mod` 3 つ。`core` は `test-support` の裏の `pub mod` 2 つ。`exec`・`upper` に `pub mod`・`pub use` は無い | `grep -n "pub mod\|pub use" crates/*/src/lib.rs` |
| この検査の実体 | `scripts/public-api-check.sh` と `make public-api-check` が在る。CI のステップは無い。走査した公開アイテムの数は標準出力に出る（書式は同スクリプトの先頭コメントと回帰テストが示す）。検査の中身は同スクリプトの先頭コメントと `scripts/test-public-api-check.sh` が示す | `grep -rn "public-api-check" scripts Makefile .github` |
| 既存の言及 | TASK-10 への言及は [public-name-check](public-name-check.md) の 2 行だけ | `git grep -n -w "TASK-10" -- docs .claude README.md CLAUDE.md crates scripts` |
| 依存グラフ側の守り | `deny.toml` の `[bans] deny` と `make deny`（CI の `deny` ジョブ） | `deny.toml` |
| 比較基準 | `baseline-wgpu/` はルートの `exclude` で切り離した別 workspace | ルートの `Cargo.toml`、[baseline-wgpu](baseline-wgpu.md) |
| feature の組 | `scripts/build-profiles.sh` の `NATIVE_SETS`・`WASM_SETS` | 同スクリプト |
| toolchain | `rust-toolchain.toml` が単一の真実源 | 同ファイル |
| CI のジョブ | 一覧は複製しない | [ci-device-free-jobs](ci-device-free-jobs.md)・[ci-job-split](ci-job-split.md) |

## 関連タスクの状態

| 関連 | Issue | 状態 |
| ---- | ----- | ---- |
| TASK-7 | #40 | CLOSED |
| TASK-14 | #56 | CLOSED |
| TASK-112 | #393 | CLOSED |
| TASK-11 | #364 | OPEN |
| TASK-12 | #365 | OPEN |
| TASK-13 | #366 | OPEN |
| TASK-15 | #367 | OPEN |
| TASK-22 | #368 | OPEN |
| TASK-113 | #394 | OPEN |
| TASK-1523 | #2969 | OPEN |

## 後続で扱う論点

名詞だけを挙げる。答えはここに書かない。

- 載せるジョブ
- 失敗の確かめ

## 後続の分担

検査の内容は、受け持つ段の PR のコードとテストが示す。

| Issue | 段 |
| ----- | -- |
| #3391 | 2/6 実装 |
| #3392 | 3/6 実装 |
| #3393 | 4/6 テスト |
| #3394 | 5/6 CI への組み込み |
| #3395 | 6/6 完了の条件の確かめ |

親 #363 の進み具合の spec への記録は spec 側の作業で、本リポからは書き換えない。

## 確かめた日付

2026-10-12。base は `main` の `c4704d8`、gitlink は `267f826`。

## 関連

- [public-name-check](public-name-check.md)・[ci-device-free-jobs](ci-device-free-jobs.md)・[ci-job-split](ci-job-split.md)
- [baseline-wgpu](baseline-wgpu.md)・[workspace-boundary](workspace-boundary.md)・[measurement-record](measurement-record.md)
- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[dependency-policy](../.claude/rules/dependency-policy.md)
