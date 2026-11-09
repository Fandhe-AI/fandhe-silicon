# CUDA の背後実装の投入（TASK-604）の所在と本リポの現状

TASK-604 の作業のうち、CUDA の背後実装の `CommandQueue` の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-604。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 口 | `CudaDevice` の `CommandQueue` の `create_encoder`・`submit` | `grep -n 'fn submit\|fn create_encoder' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |

## 記録先の集合と submit（1/5）

### create_encoder

- 記録先の識別子を払い出し、device の開いている集合に入れる。何も投入しない
- 投入せずに破棄した記録先は、集合から外れる
- 識別子が尽きたときは id 0 の記録先を返し、集合には入れない

### submit

検査の順と返す値:

| 順 | 条件 | 返す値 |
| -- | ---- | ------ |
| 1 | 投入済みの表の回収 | 返す値なし |
| 2 | 空でない `wait` | `Unsupported { op: "submit(wait)", backend: Cuda }` |
| 3 | この device で開いていない記録先（別 device の記録先・投入済みの記録先・id 0） | `InvalidArgument` |
| 4 | 通し番号が尽きた | `BackendFailure { op: "submit" }` |
| 5 | event の記録の失敗 | `BackendFailure { op: "record_event" }` |
| - | 上のどれにも当たらない | 印 |

- 拒んだ・失敗した呼び出しは通し番号を消費しない
- 通し番号は device ごとに 1 から払い出す。印の世代は device の世代
- `submit` は完了を待たずに返す

### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `the_first_sequence_is_one_and_a_failed_issue_does_not_consume_a_number` | 最初の番号は 1。失敗した払い出しは番号を消費しない |
| `an_exhausted_sequence_does_not_call_make` | 番号が尽きると記録を呼ばずに `BackendFailure` |
| `reclaim_stops_at_a_pending_head_and_moves_failures_aside` | 未完了の先頭で回収が止まり、失敗は別の集合へ移る |
| `claim_rejects_unknown_zero_and_duplicate_ids_without_changing_the_set` | 開いていない・0・重複は `InvalidArgument` で集合を変えない |
| `close_removes_an_open_id_and_ignores_unknown_ones` | 破棄で集合から外れ、無い id は何もしない |
| `a_nonempty_wait_is_unsupported` | 空の `wait` は通り、空でなければ `Unsupported` |
| `state_ex_distinguishes_pending_complete_failed_and_unissued` | 未完了・完了・失敗・払い出していない番号を区別する |
| `take_ready_keeps_pending_callbacks_and_releases_terminal_ones` | 終端の callback だけを外へ出し、未完了は残す |

### 完了の問合せ

- `poll`・`poll_ex` は投入済みの表を回収して答える。別 device・世代違いの印は `Lost`
- 失敗した列は `poll` で `Lost`、`poll_ex` で `RuntimeError`。払い出していない番号（0 を含む）は `poll` で `Lost`、`poll_ex` で `Unknown`
- `on_complete` は終端ならその場で、未完了なら登録して `submit`・`poll`・`poll_ex`・`on_complete` の中で終端と分かった時に、錠の外で 1 回呼ぶ

## 投入の完了の問い合わせ（2/5）

`CommandQueue::poll` は投入済みの表から 3 値で答える。待たない。

| 状況 | `poll` の値 |
| ---- | ----------- |
| 自分の印で未完了 | `Pending` |
| 自分の印で完了 | `Complete` |
| 回収済み | `Complete` |
| event の問い合わせの失敗 | `Lost` |
| 別 device の印・世代違いの印 | `Lost` |
| 払い出していない番号（0 を含む） | `Lost` |

- `poll_ex` はこの背後実装で上書きせず、共通の crate の既定の本体が `poll` の 3 値を写す
- 投入済みの表の回収は `submit`・`poll`・`on_complete` の中で行う
- `poll` は錠を放してから、終端になった登録済みの callback を呼ぶ
- 印の持ち主の照合は `check_token_owner` で行い、別 device は `DeviceMismatch`、世代違いは `DeviceLost`（印の世代）を返す

| 試験 | 確かめること |
| ---- | ------------ |
| `states_cover_pending_complete_failed_reclaimed_and_unissued` | 未完了・完了・失敗・回収済み・払い出していない番号の値 |
| `the_owner_check_rejects_foreign_and_stale_tokens` | 別 device と世代違いの印の照合の結果 |
| `a_stalled_head_stays_pending_even_if_the_group_reads_complete_afterwards` | 止まった先頭が `Pending` のままであること |

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda`

今の値の補い:

- 1/5 の節の `poll_ex` の行（失敗した列は `RuntimeError`、払い出していない番号は `Unknown`）は、この段のあとは `poll` と同じく `Lost`
- 1/5 の節の試験の表の `state_ex_distinguishes_pending_complete_failed_and_unissued` は、この段で `states_cover_pending_complete_failed_reclaimed_and_unissued` に替わった

## 一括投入（3/5）

`submit_many` は複数の記録先を 1 回で投入し、印を 1 つ返す。検査の順と返す値は次のとおり。どれも何も投入せず、番号を消費しない。

| 順 | 入力 | 返す値 |
| -- | ---- | ------ |
| 1 | `after` の別 device の印 | `DeviceMismatch` |
| 1 | `after` の世代違いの印 | `DeviceLost`（印の世代） |
| 2 | 空の列 | `InvalidArgument` |
| 3 | 空でない `wait` | `Unsupported` |
| 4 | 開いていない記録先・重複を含む列 | `InvalidArgument`（開いている集合は変えない） |

- 検査の前に投入済みの表を回収する（回収は `submit`・`submit_many`・`poll`・`on_complete` の中で行う）
- 通れば全記録先を集合から外し、event を 1 つ記録して印を 1 つ返す。番号は 1 つだけ使う
- `after` は照合だけで、待ちは足さない（1 つの stream に順に積む）
- 通れば、錠を放してから終端になった登録済みの callback を呼ぶ（`submit` と同じ）

| 試験 | 確かめること |
| ---- | ------------ |
| `submit_many_checks_after_then_empty_then_wait` | `after`、空の列、`wait` の検査の順と値 |

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda`

## device の要る試験（4/5）

device に触る試験を、すべて `#[ignore = "requires a CUDA device"]` 付きで足した。`--ignored` を付けたときだけ走る。

統合試験 `crates/contract/tests/backend_cuda_submit.rs`（feature `backend-cuda`）:

| 試験 | 確かめること |
| ---- | ------------ |
| `submit_returns_a_token_of_the_device_generation_without_waiting` | 印の世代が device の世代で、直後の `poll` が `Pending` か `Complete`、最後は `Complete` |
| `creating_and_dropping_encoders_submits_nothing` | 記録先を作って捨てたあとの最初の投入の番号が 1 |
| `submit_ex_with_an_empty_wait_returns_a_token_that_completes` | 空の `wait` の `submit_ex` が印を返し `Complete` になる |
| `foreign_encoders_and_a_nonempty_wait_are_rejected_without_consuming_a_sequence` | 別 device の記録先は `InvalidArgument`、空でない `wait` は `Unsupported`、番号を消費しない |
| `submit_many_returns_one_token_for_the_whole_list` | 3 つの記録先の一括投入が印を 1 つ返し、番号が 1 つ進んで `Complete` になる |
| `submit_many_rejects_bad_lists_without_consuming_a_sequence` | 空の列・別 device の `after`・別 device の記録先を含む列を拒み、番号を消費しない |
| `poll_of_a_foreign_or_unissued_token_is_lost` | 別 device の印と払い出していない番号の `poll` が `Lost` |

lib の `device_tests` に足した 2 本（`#[cfg(test)]` の数の口 `test_submitted_count`・`test_open_encoder_count` を使う）:

| 試験 | 確かめること |
| ---- | ------------ |
| `dropping_an_encoder_closes_it_and_submits_nothing` | 記録先を捨てると開いている数が戻り、投入は 0 回 |
| `rejected_submissions_do_not_change_the_submitted_count` | 拒まれた投入が投入の回数と開いている数を変えない |

確かめ方:

- `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_submit` が `0 passed; 0 failed; 7 ignored` で通る
- `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` が `11 ignored` で通る

## 完了の条件の確かめ（5/5）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-604。ポインタのみ）。

時点のコミット: `371a82d`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 単体 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` | 28 passed; 0 failed; 11 ignored、終了コード 0 |
| 2 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_submit` | 0 passed; 0 failed; 7 ignored、終了コード 0 |
| 3 | 個数テスト | `cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 1 passed、終了コード 0 |
| 4 | 公開 API | `make public-api-sdk-check` | `sdk_type_refs_total=0`、終了コード 0 |
| 5 | 依存の検査 | `make deps-check`、`grep -c '^\[\[approval\]\]' deps-approved.toml` | 終了コード 0、件数 5 |
| 6 | feature の組の clippy | `cargo clippy -p fandhe-silicon-contract --all-targets --features <組> -- -D warnings`（`backend-cuda`、`backend-cuda,native-blocking-wait`、`backend-cuda,fault-injection`） | 3 組とも終了コード 0 |
| 7 | CI | `gh run view 42475946787 --json conclusion,headSha,jobs`（main の push の run） | conclusion success、headSha `371a82d97296979a98969bbc84e0838b81bd7b9c`、`make verify (fmt + clippy + test)` success、`make verify + build-profiles (macos-latest)` success、`make deny (cargo-deny)` success |

(1) の出力。

```text
running 39 tests
test result: ok. 28 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out; finished in 0.01s
```

(2) の出力。

```text
running 7 tests
test result: ok. 0 passed; 0 failed; 7 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(3) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

(4) の出力。

```text
record: commit=371a82d97296979a98969bbc84e0838b81bd7b9c documents=10 public_items_total=13838 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
```

(5) の出力。

```text
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
5
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/5 | 記録先の集合と submit | #5184 |
| 2/5 | 投入の完了の問い合わせ | #5185 |
| 3/5 | 複数の記録先の一括投入 | #5186 |
| 4/5 | 投入の device の要る試験 | #5187 |
| 5/5 | 完了の条件の確かめ | #5188 |

## 今の値の補い（base 788d89a）

記録時点の表と既存の節は書き換えない。以下は base `788d89a` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 完了の callback を呼ぶ呼び出しの列 | `submit`・`poll`・`poll_ex`・`on_complete` → feature `fault-injection` のビルドでは `FaultHooks::simulate_device_lost` の 1 回目も、登録の世代が古い callback を `Lost` で呼ぶ | `grep -n 'take_stale' crates/contract/src/backend_cuda.rs` |
| 登録の callback の値 | 列の状態 → 登録の世代が駆動の読んだ世代より小さければ列の状態に関わらず `Lost` | `cargo test -p fandhe-silicon-contract --no-default-features --features backend-cuda --lib backend_cuda::tests::take_ready` |

## 今の値の補い（base 1417dc5）

記録時点の表と既存の節は書き換えない。以下は base `1417dc5` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「完了の条件の確かめ（5/5）」の (1) の行と出力 | `28 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out` → `50 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out`（`running 61 tests`）。11 ignored は変わらない | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |

## 今の値の補い（base 70ab958）

記録時点の表と既存の節は書き換えない。以下は base `70ab958` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「完了の条件の確かめ（5/5）」の (1) の行と出力 | `50 passed; 0 failed; 11 ignored`（`running 61 tests`）→ `57 passed; 0 failed; 19 ignored; 0 measured; 13 filtered out`（`running 76 tests`） | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |
| 「投入の完了の問い合わせ（2/5）」の節の `on_complete` の行（終端と分かった時に呼ぶ場所の列挙: `submit`・`poll`・`poll_ex`・`on_complete`） | その 4 つ → 登録している完了の callback を呼ぶ場所は、`submit`・`submit_many`・`poll`・`on_complete`（どれも `drive_callbacks` か `on_complete` 自身が呼ぶ）に加えて（`poll_ex` は `CommandQueue` の既定の本体が `poll` を 1 回呼ぶので、`poll` の経路に含まれる）、`Drop`（同期を確かめたら `take_ready` と同じ判定で呼ぶ）と `destroy`（登録している全部を待たずに `Lost` で呼ぶ）の経路にもある。feature `fault-injection` のビルドでは `FaultHooks::simulate_device_lost` の 1 回目も、登録の世代が古い callback を `Lost` で呼ぶ | `grep -n 'run_callbacks(\|drive_callbacks()' crates/contract/src/backend_cuda.rs`（`Drop` は 1302 行目、`destroy` は 1684 行目付近、`simulate_device_lost` は 1773 行目付近） |
