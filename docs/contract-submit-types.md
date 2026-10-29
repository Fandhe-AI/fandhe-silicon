# 投入まわりの公開の型と `CommandQueue`（TASK-588）の所在と本リポの現状

TASK-588 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-588。対象の項目は `docs/spec/04-behavior/api-command-submit.md` の CMD-1・CMD-5・CMD-9・CMD-12・CMD-36・CMD-44・CMD-80。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 公開に足した名前

| 名前 | 場所 | 種類 |
| ---- | ---- | ---- |
| `SubmitToken` | `crates/core/src/execution/submit.rs`（`execution` から公開） | 構造体（`#[non_exhaustive]`。`generation`・`sequence` が公開、device の識別子は非公開） |
| `GpuTiming` | 同上 | enum（`#[non_exhaustive]`。`Eq` は持たない） |
| `CommandEncoder` | 同上 | 構造体（フィールド非公開。`Clone` なし。`Drop` で背後実装の hook の `close`。`Send + Sync`） |
| `CommandQueue` | 同上 | trait |
| `ImportedSemaphoreHandle` | `crates/core/src/execution.rs` | 構造体（非公開の `id` だけ） |
| `__BackendEncoderCloser`・`__BackendSharedEncoderCloser` | `submit.rs`（`#[doc(hidden)]`） | 背後実装向けの hook の trait |

`ImportedSemaphoreHandle` の定義を `execution` の直下に置くのは、公開 API の検査（`scripts/public-api-webgpu-check.sh`）が定義のパスを `execution` の中に限るため。

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `create_encoder` | `Result` でなく `CommandEncoder` を返す。必須のメソッドは `create_encoder`・`submit`・`poll`・`on_complete` | `crates/core/tests/submit_types.rs` |
| `on_complete` | native では callback に `Send` を要求し、`target_arch = "wasm32"` では外す | 同上、wasm32 向けのビルド |
| 既定の本体 | `poll_ex`（`poll` の 3 値を写す）・`submit_ex`・`submit_exportable`（`submit` に委ねる）・`gpu_timing`（`Unavailable`） | 同上（呼べることまで） |
| feature | `wait_blocking` は `native-blocking-wait`、`submit_exportable` は `ext-interop` の裏。`ext-interop` は `core` にも置き、`contract` の同名 feature が転送する | `crates/core/Cargo.toml`、`crates/contract/Cargo.toml`、`make core-deps-check` |
| 範囲の外 | `dispatch` 系・束縛の検査・一括投入・`Device` からの取り出しは本書の範囲に含まれない。`Device` は変えていない | `crates/core/src/device.rs` |
| 外部依存 | 足していない（`Cargo.lock`・`deps-approved.toml` は変更なし） | `make deps-check` |

## 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| `crates/core/tests/submit_types.rs` | ダミーの queue、`dyn CommandQueue`、`CommandEncoder` の `Send + Sync`・`Clone` なし・`Drop` と `__backend_disarm_closer`、`SubmitToken`・`GpuTiming` の derive、feature ごとのメソッド |
| `SubmitToken`・`CommandEncoder` の rustdoc（`compile_fail`） | 外からの構造体式の構築と `Clone` が拒まれること |

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --all-features
cargo test -p fandhe-silicon-core --no-default-features
cargo build -p fandhe-silicon-core --target wasm32-unknown-unknown
make verify && make check && make deny
make public-api-check && make public-name-check && make public-api-webgpu-check && make core-deps-check
```
