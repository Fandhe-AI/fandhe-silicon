# 投入まわりの公開の型と `CommandQueue`（TASK-588）の所在と本リポの現状

TASK-588 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-588。ポインタのみ）。内容・理由・経緯は本書に置かない。

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
| `create_encoder` | `Result` でなく `CommandEncoder` を返す。必須のメソッドは `create_encoder`・`submit`・`submit_many`・`poll`・`on_complete` | `crates/core/tests/submit_types.rs` |
| `on_complete` | native では callback に `Send` を要求し、`target_arch = "wasm32"` では外す | 同上、wasm32 向けのビルド |
| 既定の本体 | `poll_ex`（`poll` の 3 値を写す）・`submit_ex`・`submit_exportable`（`submit` に委ねる）・`gpu_timing`（`Unavailable`） | 同上（呼べることまで） |
| feature | `wait_blocking` は `native-blocking-wait`、`submit_exportable` は `ext-interop` の裏。`ext-interop` は `core` にも置き、`contract` の同名 feature が転送する | `crates/core/Cargo.toml`、`crates/contract/Cargo.toml`、`make core-deps-check` |
| 範囲の外 | `dispatch` 系・束縛の検査は本書の範囲に含まれない。`Device` は `queue(&self) -> &dyn CommandQueue` を持ち、投入の窓口を取り出せる | `crates/core/src/device.rs` |
| 一括投入 | `submit_many(Vec<CommandEncoder>, &[SubmitToken], &[ImportedSemaphoreHandle])` を足した（TASK-592）。既定の本体を持たない必須のメソッドで、共通の crate は宣言だけを持つ | `crates/core/tests/submit_types.rs`（`dyn CommandQueue` 越しに呼べることまで） |
| `Device::queue` | `Device` の必須のメソッドとして `queue(&self) -> &dyn CommandQueue` を足した（TASK-592）。既定の本体は無い | `crates/core/tests/device_trait_shape.rs`（`device_exposes_the_command_queue`） |
| 記録先の識別子の源 | 背後実装向けの隠した口 `__backend_next_encoder_id() -> Option<u64>` を足した（1 から、背後実装・device をまたいで重ならず、0 を返さず、尽きたら `None`） | `crates/core/tests/submit_types.rs`（`backend_encoder_ids_are_unique_and_never_zero_across_threads`） |
| 外部依存 | 足していない（`Cargo.lock`・`deps-approved.toml` は変更なし） | `make deps-check` |

## 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| `crates/core/tests/submit_types.rs` | ダミーの queue、`dyn CommandQueue`、`CommandEncoder` の `Send + Sync`・`Clone` なし・`Drop` と `__backend_disarm_closer`、`SubmitToken`・`GpuTiming` の derive、feature ごとのメソッド |
| `crates/core/tests/poll_ex_default.rs` | `poll_ex` を上書きしない queue で `poll` の `Pending`・`Complete`・`Lost` が対応する `CompletionStateEx` に写り、`RuntimeError`・`Unknown` が出ないこと（ERR-27。TASK-270） |
| `crates/core/tests/submit_default_impls.rs`（`submit_ex`） | `RetryPolicy` の両方の値で、内側の `submit` がちょうど 1 回呼ばれ `Ok`・`Err` をそのまま返すこと（CMD-12。TASK-589。`fault-injection` の裏） |
| `crates/core/tests/submit_default_impls.rs`（`submit_exportable`） | `exportable` の真・偽の両方で、内側の `submit` が 1 回呼ばれ戻り値をそのまま返すこと（CMD-20。TASK-589。`fault-injection` と `ext-interop` の裏） |
| `crates/core/tests/gpu_timing_default.rs` | `gpu_timing` を上書きしない queue がどの印でも `GpuTiming::Unavailable` を返すこと（TASK-589） |
| `SubmitToken`・`CommandEncoder`・`CommandQueue::on_complete` の rustdoc（`compile_fail`） | 外からの構造体式の構築、`Clone`、native で `Send` でない callback が拒まれること |
| `CommandQueue::submit` の rustdoc（`compile_fail,E0382`） | `submit` に渡した `CommandEncoder` の変数を再び使うと所有権の移動のエラーになること（CMD-5。TASK-590） |
| `CommandEncoder` の rustdoc（`compile_fail`） | `Send + Sync` でない型（`Rc<()>`）が同じ境界で拒まれること。`Send + Sync` を確かめる検査が外れると落ちることの陽性対照（CMD-9。TASK-590） |
| `crates/core/tests/encoder_send_sync.rs` | `CommandEncoder` が `Send + Sync` の境界を満たすこと、別スレッドへ移せること、複数スレッドから共有参照できること（CMD-9。TASK-590。スレッドを使う部分は wasm32 を除く） |

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --all-features
cargo test -p fandhe-silicon-core --no-default-features
cargo build -p fandhe-silicon-core --target wasm32-unknown-unknown
make verify && make check && make deny
make public-api-check && make public-name-check && make public-api-webgpu-check && make core-deps-check
```
