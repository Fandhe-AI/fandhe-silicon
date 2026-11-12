# Metal の dispatch の記録（TASK-775）の所在と本リポの現状

TASK-775 の作業のうち、Metal の背後実装が dispatch を記録し、投入の時に compute encoder へ積む部分の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-775。ポインタのみ）。private spec 側の記録に従う（ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の 1 ファイル | `crates/contract/src/backend_metal.rs` |
| 引数の検査 | 非公開の `check_dispatch_args`（device に触らない純粋関数）と、入力の組 `DispatchArgs` | `grep -n 'fn check_dispatch_args' crates/contract/src/backend_metal.rs` |
| 記録先ごとの記録の表 | `DispatchRecord`・`RecordedBinding` と、表への `record`・取り出しの `claim` | `grep -n 'fn record\b\|fn claim\b' crates/contract/src/backend_metal.rs` |
| 投入の時の束縛の確かめ | `check_recorded_buffers`（device に触らない純粋関数） | `grep -n 'fn check_recorded_buffers' crates/contract/src/backend_metal.rs` |
| 積み方の計画と積み | `prepare_groups`（計画）・`encode_run`・`commit_records`（`imp` の中） | `grep -n 'fn prepare_groups\|fn encode_run\|fn commit_records' crates/contract/src/backend_metal.rs` |
| 解放の保留 | `PendingReleases` と `Submissions` の帳簿（`pending_release_tests` が確かめる） | `grep -n 'struct PendingReleases' crates/contract/src/backend_metal.rs` |
| 入口 | `CommandQueue::dispatch_ex` と `submit`・`submit_many`。`Device::queue()` から届く（device の取得は TASK-158、compute の口は TASK-773、パイプラインの生成は TASK-774 のポインタ） | `grep -n 'fn dispatch_ex' crates/contract/src/backend_metal.rs` |
| cfg | feature `backend-metal`。`imp` は macOS だけ。純粋な部品と `precheck_tests`・`pending_release_tests` はどの OS でも流れる | `crates/contract/src/lib.rs` |

## 記録の前の検査（1/5）

`check_dispatch_args` は、最初に当たった項目を `InvalidArgument` で返し、副作用を持たない。検査の順:

| 順 | 項目 | 拒否の入力 |
| -- | ---- | ---------- |
| 1 | `dispatch_dims` | 0 を含む。上限を超える |
| 2 | `threads_per_threadgroup` | 0 を含む。device の上限を超える。積がパイプラインの上限を超える（積のあふれも拒否） |
| 3 | slot | 上限を超える。束縛と定数で重複する。配置に無い。配置にあるのに渡されていない |
| 4 | 束縛の offset | バッファの長さを超える（長さちょうどは受理する）。4 の倍数でない |
| 5 | 定数 | 長さ 0。長さの上限を超える |
| 6 | 共有メモリ | index が重複する。整列に合わない |

- 拒否の文言は英語で、variant と slot の番号を含む（例: `binding slot {N} offset exceeds the buffer length`）。
- `dispatch_ex` は、記録先・パイプライン・束縛のバッファを照らす。別の device のものは `DeviceMismatch`、表に無いものは `InvalidArgument`。
- 拒否したときは記録が増えない。

| 試験 | 入力と期待 |
| ---- | ---------- |
| `dispatch_args_baseline_is_accepted` | 基準の入力は受理する |
| `dispatch_dims_zero_and_over_limit_are_rejected` | 0 と上限の超過を拒否する |
| `slot_limit_duplicates_and_layout_mismatch_are_rejected` | slot の上限・重複・配置との不一致を拒否する |
| `binding_offset_accepts_the_length_and_rejects_one_more` | 長さちょうどの offset は受理し、1 つ超えると拒否する |
| `binding_offset_not_multiple_of_four_is_rejected` | 4 の倍数でない offset を拒否する |
| `constant_length_bounds_and_empty_are_rejected` | 定数の長さ 0 と上限の超過を拒否する |
| `threadgroup_memory_index_alignment_duplicates_and_total_are_checked` | 共有メモリの index・整列・重複を拒否する |

確かめ方:

```text
cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::precheck_tests
```

## 最後に使った投入と解放の保留（2/5）

- `free` は、そのバッファを最後に使った投入が終わっていなければ、ゼロ化も pool への回しもせず、解放の保留へ置く。
- 投入が終わった（完了または失敗）後、駆動する 6 つの呼び出しの中で回収し、ゼロ化して pool へ回す。
- 使用中のバッファへの `upload`・`download_blocking`・`download_async` は `InvalidArgument` で、内容を変えない。完了の後は通る。

| 試験（`pending_release_tests`） | 期待 |
| ------------------------------- | ---- |
| `an_empty_list_takes_nothing` | 空の一覧からは何も取らない |
| `pushed_items_are_counted` | 置いた分が数えられる |
| `only_finished_items_are_taken_in_pushed_order_and_the_rest_keep_their_order` | 終わったものだけを置いた順に取り、残りは順を保つ |
| `nothing_is_taken_when_none_is_finished_and_all_when_all_are` | 終わったものが無ければ取らず、全部終わっていれば全部取る |
| `a_second_take_does_not_return_items_taken_before` | 取ったものは次の取り出しに出ない |
| `is_finished_is_false_for_running_and_true_for_completed_and_failed_groups` | 実行中は終了していない、完了と失敗は終了と判定する |
| `is_finished_is_true_at_or_below_the_watermark_and_for_the_failed_set_without_reclaiming` | 水位以下と失敗の集合は、回収せずに終了と判定する |

確かめ方:

```text
cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::pending_release_tests
```

## 記録と積み（3/5）

- `dispatch_ex` は、検査に通った引数を記録先ごとの表に積む。
- `submit`・`submit_many` は、記録の束縛のバッファを全件確かめる。1 件でも外れたら `InvalidArgument` で、何も積まず通し番号も進めない。
- 通れば 1 つのコマンドバッファに、記録のある記録先ごとに compute encoder を開き、記録の順に積む。束縛した各バッファの最後に使った投入は、その投入の番号になる。
- 共有メモリの index が欠けた記録では、encoder の run を区切り、パイプラインを設定し直す。
- 製品のビルドでは `DISPATCH_EXECUTION_ENABLED`（`cfg!(test)`）が閉じている。記録のある投入は束縛を確かめた後に `Unsupported { op: "submit", backend: Metal }` を返し、何も積まない。積み込みを通すのは、この crate の lib の単体試験のビルドだけ。
- compute encoder へ設定する `unsafe` は `set_bindings`・`set_constants`・`set_threadgroup_memory` の 3 つの関数にある。個数は `crates/contract/tests/backend_metal_unsafe_count.rs` の表が持つ（[unsafe-policy](unsafe-policy.md)）。
- `objc2-metal` の feature に `MTLCommandEncoder`・`MTLComputeCommandEncoder`・`MTLTypes` を含む。

| 試験（`precheck_tests`） | 期待 |
| ------------------------ | ---- |
| `record_appends_to_an_open_encoder_in_order` | 開いた記録先に記録の順で積む |
| `record_rejects_an_unopened_id_without_creating_it` | 開いていない記録先への記録は拒否し、表を作らない |
| `claim_returns_the_records_in_the_given_order_and_removes_them` | 渡した順に記録を返し、表から取り除く |
| `claim_rejects_id_zero_unknown_ids_and_duplicates_without_changing_the_set` | 番号 0・表に無い番号・重複を拒否し、表を変えない |
| `recorded_buffers_that_are_all_alive_are_accepted` | 束縛のバッファがすべて生きていれば受理する |
| `a_missing_stale_or_old_generation_buffer_is_rejected_with_its_slot` | 無い・古い・世代違いのバッファを slot つきで拒否する |
| `a_failure_in_a_later_group_rejects_the_whole_set` | 後の記録先で外れても全部を拒否する |
| `empty_groups_make_no_run_and_groups_do_not_share_a_run` | 空の記録先は run を作らず、記録先どうしは run を共有しない |
| `a_missing_threadgroup_index_ends_the_run_and_sets_the_pipeline_again` | 共有メモリの index が欠けると run を区切り、パイプラインを設定し直す |
| `every_record_comes_out_once_in_order` | すべての記録が順に 1 度ずつ出る |

`crates/contract/tests/backend_metal_no_recording.rs` の試験（ソースの文字列を見る。どの OS でも流れる）:

| 試験 | 確かめる中身 |
| ---- | ------------ |
| `backend_metal_code_has_no_words_beyond_compute_dispatch` | コード本体に、コピー・クリア・描画・イベント・完了ハンドラに当たる語が無い |
| `objc2_metal_features_do_not_include_other_encoder_headers` | `objc2-metal` の feature に、ほかの encoder の header が無い |
| `backend_metal_contents_safety_comments_state_the_completed_submission_rationale` | `contents()` 越しの読み書きの `// SAFETY:` が、最後に使った投入の完了の確かめを述べている |
| `backend_metal_encoder_safety_comments_state_the_cpu_side_preconditions` | encoder への設定の `// SAFETY:` が、CPU 側で確かめた前提を述べている |
| `backend_metal_constant_safety_comment_keeps_its_rationale` | function constant の設定の `// SAFETY:` の語句が保たれている |
| `backend_metal_dispatch_execution_gate_stays_closed_outside_unit_tests` | 製品のビルドで dispatch を含む投入を拒む門が残っている |

確かめ方:

```text
cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count --test backend_metal_no_recording
```

## device の要る試験（4/5）

`#[ignore = "touches the Metal device; run only in a dedicated device session"]` 付き。行頭の `#[ignore` の属性の行の数を、TASK-775 の分についてファイルごとに示す。

| ファイル | TASK-775 の分の件数 | 数えた範囲 |
| -------- | ------------------- | ---------- |
| `crates/contract/src/backend_metal.rs` | 16 | `imp::device_tests::dispatch` |
| `crates/contract/tests/backend_metal_device.rs` | 2 | 下の 2 本 |

`imp::device_tests::dispatch`（入力と期待）:

| 試験 | 入力と期待 |
| ---- | ---------- |
| `dispatch_reads_and_writes_at_a_nonzero_binding_offset` | 0 でない offset の束縛を読み書きし、結果が参照と一致する |
| `dispatch_passes_a_constant_by_slot` | slot で渡した定数が結果に反映される |
| `dispatch_runs_the_given_threadgroups_with_threadgroup_memory` | 指定した数の threadgroup が共有メモリつきで走る |
| `dispatch_with_one_thread_per_threadgroup_marks_every_group` | threadgroup あたり 1 スレッドでも全 threadgroup が走る |
| `three_dispatches_in_one_encoder_run_in_recorded_order` | 1 つの記録先の 3 つの dispatch が記録の順に走る |
| `repeating_and_switching_pipelines_matches_the_reference` | 同じパイプラインの繰り返しと切り替えの結果が参照と一致する |
| `simple_dispatch_matches_dispatch_ex_with_the_defaults` | 簡易版の dispatch が既定値の `dispatch_ex` と同じ結果になる |
| `omitting_a_previously_bound_slot_is_rejected_and_not_recorded` | 前に束縛した slot を省くと拒否し、記録が増えない |
| `freeing_a_bound_buffer_before_submit_is_rejected_without_submitting` | 投入の前に束縛したバッファを `free` すると投入は拒否され、何も積まれない |
| `submit_many_with_one_freed_buffer_submits_nothing` | `submit_many` はどれか 1 つの記録先が外れれば何も積まない |
| `an_offset_past_the_length_is_rejected_and_the_exact_length_runs` | 長さを超える offset は拒否し、長さちょうどは走る |
| `freeing_a_buffer_of_a_held_submission_keeps_it_until_completion` | 投入中のバッファの `free` は完了まで解放の保留に置かれる |
| `reads_and_writes_of_an_in_use_buffer_are_rejected_until_completion` | 使用中のバッファの読み書きは完了まで拒否される |
| `download_blocking_of_an_in_use_buffer_is_rejected_until_completion` | 使用中のバッファの `download_blocking` は完了まで拒否される |
| `submit_many_with_two_encoders_completes_both_with_one_token` | 2 つの記録先を `submit_many` で投入し、1 つの印で両方が完了する |
| `record_time_rejections_leave_the_encoder_records_unchanged` | 記録の時点の拒否は記録先の記録を変えない |

`tests/backend_metal_device.rs`（公開の `Device` 越し）:

| 試験 | 入力と期待 |
| ---- | ---------- |
| `dispatch_through_the_public_device_is_recorded_and_submit_is_unsupported` | dispatch を記録して `submit` すると `Unsupported` が返り、書き込み先のバッファは 0 のまま。次の空の投入の通し番号は 1 |
| `record_time_rejections_through_the_public_device` | 別 device の記録先・パイプラインは `DeviceMismatch`、slot の不一致は slot の番号を含む `InvalidArgument`。拒否だけを受けた記録先の `submit` は成功する |

件数の数え方:

```text
grep -cE '^\s*#\[ignore' crates/contract/src/backend_metal.rs
grep -cE '^\s*#\[ignore' crates/contract/tests/backend_metal_device.rs
```

## 完了の条件の確かめ（5/5）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-775。ポインタのみ）。

時点のコミット: `ad1e625`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 検査・記録・積み方の計画の純粋な試験 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::precheck_tests` | 110 passed; 0 failed、終了コード 0 |
| 2 | 解放の保留の帳簿の試験 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::pending_release_tests` | 7 passed; 0 failed、終了コード 0 |
| 3 | unsafe の個数と記録の操作の範囲 | `cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count --test backend_metal_no_recording` | 6 passed と 1 passed、終了コード 0 |
| 4 | aarch64-apple-darwin の clippy | `cargo clippy -p fandhe-silicon-contract --all-targets --target aarch64-apple-darwin --features <組> -- -D warnings`（`backend-metal`・`backend-metal,native-blocking-wait`・`backend-metal,native-blocking-wait,fault-injection` の 3 組） | 3 組とも終了コード 0 |
| 5 | device の要る試験の宣言 | `grep -cE '^\s*#\[ignore' <ファイル>`（上の節の 2 ファイル） | `backend_metal.rs` の `imp::device_tests::dispatch` に 16 本、`backend_metal_device.rs` に 2 本 |
| 6 | 依存 | `make deps-check`、`git diff --stat 7ab8ea6 ad1e625 -- Cargo.lock deps-approved.toml` | 終了コード 0、差分 0 行 |
| 7 | CI | `gh run view 43154445889 --json conclusion,headSha`（3/5 のマージの後の main の push の run） | conclusion `success`、headSha `459e26f4c350d5314bd9ebf4e0092d5ae93dbb70` |

(1) の出力（`running` と `test result:` の行）。

```text
running 110 tests
test result: ok. 110 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.01s
```

(2) の出力。

```text
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 123 filtered out; finished in 0.00s
```

(3) の出力。

```text
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

## 段

| 段 | 内容 | Issue | PR |
| -- | ---- | ----- | -- |
| 1/5 | dispatch の引数を記録の前に検査する | #5635 | #8216 |
| 2/5 | バッファの最後に使った投入の帳簿と解放の保留 | #5636 | #8218 |
| 3/5 | dispatch を記録して投入の時に compute encoder に積む | #5637 | #8222 |
| 4/5 | dispatch の device の要る試験を `#[ignore]` 付きで足す | #5638 | #8225 |
| 5/5 | 完了の条件の確かめ | #5639 | 本頁 |
