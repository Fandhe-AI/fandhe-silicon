# Metal の背後実装の device の破棄（TASK-189）の所在と本リポの現状

TASK-189 の作業のうち、device の `Drop` の順の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-189。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の `impl Drop for MetalDevice` | `crates/contract/src/backend_metal.rs` |
| cfg | feature `backend-metal` かつ `target_os = "macos"`。部品（`finish_all` など）は macOS 以外でもビルドされる | 同上 |
| 試験 | `mod teardown_tests` | 下の「確かめ方」 |

## `Drop` の待ちと記録先の破棄（1/4）

`Drop` は次の順で進む。

| 順 | 動き |
| -- | ---- |
| 1 | 開いている記録先の記録を、投入せずに捨てる（`OpenEncoders::discard_all`） |
| 2 | 投入済みの列を、登録の有無に関わらず全部待つ（`Submissions::wait_pending`） |
| 3 | 残りの完了の callback を、`poll` と同じ判定の値で 1 回ずつ呼ぶ（`finish_all` の中の `finish_table`） |
| 4 | 欄を解放する |

- 記録先の値が device より長く生きても、その記録は投入されない
- ロストの通知は呼ばない
- 待ちに上限は無い。callback の panic は捨てる

| 試験 | 確かめること |
| ---- | ------------ |
| `finish_all_waits_every_group_without_registrations` | 実行中の列が 3 つ・登録が 0 でも、3 つとも完了になる |
| `finish_all_calls_the_registered_callbacks_with_the_polled_state_after_waiting` | 登録の無い列も待ち、登録のある callback は失敗と完了の判定の値で呼ばれ、登録の表は空になる |
| `discard_all_empties_the_open_encoders_and_a_later_close_is_a_no_op` | 捨てたあとの表は空で、あとの `close` は何もしない |

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::teardown_tests
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal -- -D warnings
```

1 本目はどの OS でも流れる。

## 明示の破棄（2/4）

`Device::destroy` を上書きする。順は次のとおり。本体は `destroy_in_order`。

| 順 | 動き |
| -- | ---- |
| 1 | 登録済みの完了の callback を待たずに `Lost` で呼び切る（`Submissions::take_all_lost` → `run_callbacks`） |
| 2 | 投入済みの列を全部待つ（`wait_pending`） |
| 3 | ロストの通知（`fault-injection` のビルドだけ。`Destroyed` で 1 回。先にロストしていれば呼ばない） |
| 4 | 解放（1/4 の `Drop` の流れ） |

callback の最初の panic、無ければ通知先の panic を、解放の後に再送出する。もう一方は捨てる。

| 項目 | `Drop` | `destroy` |
| ---- | ------ | --------- |
| ロストの通知 | 呼ばない | `fault-injection` のビルドで呼ぶ |
| callback の値 | 待った後の `poll` と同じ判定 | `Lost` |
| panic | 捨てる | 解放の後に再送出する |

| 試験 | 確かめること |
| ---- | ------------ |
| `take_all_lost_returns_every_registration_as_lost_and_empties_the_table` | 登録が全部 `Lost` で返り、表が空になる |
| `destroy_steps_run_in_order` | 4 つの段が順に 1 回ずつ呼ばれ、戻りは `None` |
| `destroy_steps_return_the_first_panic_after_release` | 解放のあとに最初の panic の中身を返す |
| `destroy_steps_return_the_notify_panic_when_flush_has_none` | callback 側に無ければ通知先の中身を返す |

```bash
cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::teardown_tests
cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection --lib backend_metal::teardown_tests
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal,fault-injection -- -D warnings
```

上の 2 本はどの OS でも流れる。

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | `Drop` の待ちと記録先の破棄 | #3842 |
| 2/4 | 明示の破棄 | #3843 |
