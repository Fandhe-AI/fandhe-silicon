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

## device の要る試験（3/4）

device に触る試験で、どれも `#[ignore]` を付けてあり、既定では流れない（実行の結果の `ignored` の件数に数えられる）。

lib の `backend_metal::imp::teardown_device_tests`（直下の 5 本は `backend-metal` だけで動く）:

| 試験 | 準備 | 確かめること |
| ---- | ---- | ------------ |
| `dropping_with_an_open_encoder_submits_nothing_and_empties_the_table` | 記録先を 1 つ開いたまま | 落とすと投入は増えず、記録先の表が空になる |
| `dropping_right_after_submit_waits_for_the_submission` | 空の記録先を投入 | 落とした後、コマンドバッファが全部完了している |
| `dropping_right_after_on_complete_calls_it_once_with_complete` | 投入と callback の登録 | 落とすと callback が `Complete` で 1 回だけ呼ばれる |
| `destroying_right_after_submit_waits_for_the_submission` | 空の記録先を投入 | 明示に破棄した後、コマンドバッファが全部完了している |
| `destroying_right_after_on_complete_calls_it_once` | 投入と callback の登録 | 明示に破棄すると callback が 1 回だけ、`Pending` 以外で呼ばれる |

同じ module の中の `lost_notify`（`fault-injection` のビルドだけ）:

| 試験 | 確かめること |
| ---- | ------------ |
| `destroy_notifies_destroyed_once` | 明示の破棄でロストの通知が `Destroyed` で 1 回 |
| `destroy_after_a_simulated_loss_does_not_notify_again` | 模擬のロストの後の破棄は通知を重ねない |
| `drop_does_not_notify` | `Drop` は通知を呼ばない |

統合試験 `backend_metal_destroy`（公開の口だけを使う。確保の量は同じプロセスで共有するので錠で 1 つずつ流す）:

| 試験 | 準備 | 確かめること |
| ---- | ---- | ------------ |
| `dropping_the_device_returns_its_allocations` | 4 MiB のバッファを 4 本確保して 1 本を解放し、空の記録先を投入 | 落とした後の確保の量が、前の量に 4 MiB を足した値より小さい |
| `destroying_the_device_returns_its_allocations` | 同じ | 明示に破棄した後も同じ |

```bash
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait,fault-injection --lib backend_metal::imp::teardown_device_tests
cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_destroy
```

上の 2 本は、実行の結果の `ignored` の件数を見る（macOS 以外では統合試験は 0 件）。

## 今の値の補い（device の要る試験の準備）

3/4 の表の、投入を含む 4 本の試験（`dropping_with_an_open_encoder_...` 以外の `dropping_right_after_*` 2 本と `destroying_right_after_*` 2 本）は、試験用の補助 `submit_one` で投入する。`submit_one` は先に `hold_commits` で確定を遅らせてから空の記録先を投入し、戻った時点の `poll` が `Pending` であることを確かめる。溜めた確定は破棄の側（`Drop`・`destroy`）が流して待つ。

- `destroying_right_after_on_complete_calls_it_once` の callback に渡る値は `Lost` で、試験は `Lost` との一致を確かめる（表の「`Pending` 以外」より強い）
- `dropping_right_after_on_complete_calls_it_once_with_complete` の値は `Complete` のまま

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-189。ポインタのみ）。

時点のコミット: `ea724aef`。`docs/spec` の gitlink は `3f9168084bba265e4d9044c513a30e7f2a84243d`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | `Drop` の部品 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::teardown_tests` | 7 passed; 0 failed; 0 ignored、終了コード 0 |
| 2 | 明示の破棄の順の部品 | `cargo test -p fandhe-silicon-contract --features backend-metal,fault-injection --lib backend_metal::teardown_tests` | 7 passed; 0 failed; 0 ignored、終了コード 0 |
| 3 | device の要る試験の宣言 | `grep -c '^\s*#\[ignore' crates/contract/src/backend_metal.rs`、同 `crates/contract/tests/backend_metal_destroy.rs` | `backend_metal.rs` は 96、`backend_metal_destroy.rs` は 2 |
| 4 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --all-features -- -D warnings` | 終了コード 0 |
| 5 | 依存 0・unsafe 0 | `git diff --stat 152a6b20..HEAD -- Cargo.lock deps-approved.toml crates/core crates/contract/src/lib.rs crates/contract/tests/backend_metal_unsafe_count.rs`、`cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count`、`make deps-check` | diff は出力なし、1 passed; 0 failed、`make deps-check` は終了コード 0 |
| 6 | 公開の型の検査 | `make public-api-sdk-check`、`make public-api-webgpu-check` | `sdk_type_refs_total=0`、`ok: 2 public API document(s) checked, no blocking-wait names, pipeline input ok, ext-interop ok, submit token ops ok`、終了コード 0 |
| 7 | CI | `gh run view 43628048020 --json conclusion,headSha,jobs`（`ea724aef` の main の push の run） | conclusion `success`、headSha `ea724aef4a7afbf46e1fe6343f676c883841c58e`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success` |

(5) の `152a6b20` は、1/4 の 1 つ前のコミット。

(1) の出力。

```text
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 160 filtered out; finished in 0.00s
```

(2) の出力。

```text
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 172 filtered out; finished in 0.00s
```

(5) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

(6) の出力。

```text
record: commit=ea724aef4a7afbf46e1fe6343f676c883841c58e documents=10 public_items_total=14168 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
ok: 2 public API document(s) checked, no blocking-wait names, pipeline input ok, ext-interop ok, submit token ops ok
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | `Drop` の待ちと記録先の破棄 | #3842 |
| 2/4 | 明示の破棄 | #3843 |
| 3/4 | device の要る試験 | #3844 |
| 4/4 | 完了の条件の確かめ | #3845 |

## 今の値の補い（base 075fb6da）

記録時点の表と既存の節は書き換えない。以下は base `075fb6da` に対するこの変更の後の今の値。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「`Drop` の待ちと記録先の破棄（1/4）」の順の表 | 1 から始まる → feature `fault-injection` のビルドでは、1 の前に積んだ試験用の門を全部開ける（所在は [metal-fault-gate](metal-fault-gate.md)） | `grep -n 'impl Drop for MetalDevice' -A12 crates/contract/src/backend_metal.rs` |
| 「明示の破棄（2/4）」の順の表の 2 | 投入済みの列を全部待つ → feature `fault-injection` のビルドでは、積んだ試験用の門を全部開けてから、投入済みの列を全部待つ | `grep -n 'let wait = |d: &Self|' -A4 crates/contract/src/backend_metal.rs` |
