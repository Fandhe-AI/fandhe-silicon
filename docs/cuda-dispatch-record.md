# CUDA の背後実装の dispatch の記録と投入（TASK-805）の所在と本リポの現状

TASK-805 の作業のうち、CUDA の背後実装の `CommandQueue::dispatch_ex`（簡易版の `dispatch` を含む）が記録の時点に照らして記録先に積む形と、投入（`submit`・`submit_many`）が dispatch の記録を扱う形の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-805。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/contract/src/backend_cuda.rs` の `impl CommandQueue for CudaDevice` の `dispatch_ex`。`dispatch` は上書きしない | `git grep -n "fn dispatch_ex(\|fn dispatch(" -- crates/contract/src/backend_cuda.rs` |
| 記録の照らし | 同じファイルの `check_dispatch_record`。記録の型は `DispatchRecord` と `RecordedArg` | `git grep -n "fn check_dispatch_record\|struct DispatchRecord\|enum RecordedArg" -- crates/contract/src/backend_cuda.rs` |
| 投入の照らし | 同じファイルの `plan_transfers` と `check_dispatch_gate`。門は `DISPATCH_EXECUTION_ENABLED` | `git grep -n "fn plan_transfers\|fn check_dispatch_gate\|const DISPATCH_EXECUTION_ENABLED" -- crates/contract/src/backend_cuda.rs` |
| 試験 | 同じファイルの `dispatch_record_tests` module。device に触らない | 下の「確かめ方」 |

## 記録の時点の照らしと投入の門（1/4）

`dispatch_ex` は、slot・定数・共有メモリを `check_dispatch_record` で照らし、通れば記録を記録先に積む。

`check_dispatch_record` が返す値と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| 束縛と定数が配置の slot をちょうど 1 回ずつ持つ | 束縛と定数を slot の昇順に並べた記録。定数の bytes は写し、共有メモリは各 `bytes` の合計で、`index` は使わない | `record_orders_buffers_and_constants_by_slot_and_sums_shared_memory` |
| `offset` が `u64::MAX` | 長さと比べずにそのまま記録する | `record_keeps_the_offset_without_an_upper_check` |
| 空の定数 | `dispatch_ex: constant slot <slot> is empty` | `record_rejects_an_empty_constant_with_its_slot` |
| 同じ slot が 2 回 | `dispatch_ex: duplicate slot in dispatch: <slot>` | `record_rejects_duplicate_unknown_and_missing_slots_in_this_order` |
| 配置に無い slot | `dispatch_ex: slot <slot> is not in the binding layout` | 同上 |
| 配置にあって渡されない slot | `dispatch_ex: slot <slot> of the binding layout is not provided` | 同上 |
| 共有メモリの合計が `u32` をあふれる | `dispatch_ex: threadgroup memory total overflows` | `record_rejects_a_shared_memory_total_that_overflows` |

slot は束縛・定数の順に 1 つずつ、重なり・配置に無いの順で見て、その後で配置にあって渡されない slot を見る。その後で空の定数を見て、最後に共有メモリの合計を見る。

投入は、記録したバッファの解放済みの確かめ（`plan_transfers`）を全記録について先に行う。dispatch の束縛の解放済みは `a bound buffer was freed before submit (binding slot <slot>)`、コピー・クリアは `a copied or cleared buffer was freed before submit` で、どちらも `InvalidArgument`。その後で `check_dispatch_gate` が、門が閉じていれば dispatch の件を含む列を `Unsupported { op: "submit", backend: Cuda }` で拒む。コピー・クリアだけの列は門に関わらず通す。

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| 束縛のバッファが表に無い・世代が違う | slot を載せた `InvalidArgument`。前の記録の範囲が外れていても先に返す | `plan_rejects_a_freed_bound_buffer_with_its_slot` |
| クリアと dispatch の列 | 記録の順の件。閉じた門は `Unsupported`、開いた門は通す。クリアだけの列は閉じた門でも通す | `gate_rejects_lists_with_a_dispatch_only_when_closed` |

## 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::record_
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::plan_
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::gate_
```

1 つ目は 5 件、2 つ目と 3 つ目は 1 件ずつが通る。

## カーネルの起動と仮引数の照らし（2/4）

門 `DISPATCH_EXECUTION_ENABLED` は `cfg!(test)` で、単体の試験のビルドだけで開く。ほかのビルドでは、dispatch の記録を含む投入は解放済みの確かめの後に `Unsupported` で拒まれ、何も積まれない。門の字句は `crates/contract/tests/backend_cuda_dispatch_gate.rs` がソースの文字列で守る。

門が開いていれば、投入はコピー・クリア・カーネルを stream に積む前に、dispatch の全件について次を済ませる（`prepare_launches`）。

1. パイプラインの関数を表から引く。
2. 関数の仮引数の数と各仮引数の大きさを、記録した引数（束縛のポインタは 8 バイト、定数は bytes の長さ）と照らす（`check_kernel_params`）。
3. 束縛のポインタを解決して `offset` を足す。和がバッファの長さを超えるかは照らさない。

その後で、記録の順にコピー・クリア・カーネルを積む。

`check_kernel_params` が返す値と試験:

| 入力 | 返す値 | 試験 |
| ---- | ------ | ---- |
| 数と大きさがちょうど同じ | 通る。問う回数は引数の数 + 1 | `kernel_params_pass_when_count_and_sizes_match` |
| 大きさが違う | `submit: kernel parameter <i> takes <size> bytes but <len> were recorded` | `kernel_params_reject_size_and_count_mismatches` |
| 仮引数が少ない | `submit: the kernel takes <n> parameters but <m> were recorded` | 同上 |
| 仮引数が多い | `submit: the kernel takes more than the <m> recorded parameters` | 同上 |
| 問い合わせの失敗 | そのまま返す | 同上 |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests::kernel_params_
cargo test -p fandhe-silicon-contract --test backend_cuda_dispatch_gate
cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count
```

1 つ目は 2 件、2 つ目と 3 つ目は 1 件ずつが通る。

## device の要る試験（3/4）

device に触る試験は `crates/contract/src/backend_cuda.rs` の `dispatch_device_tests` module にあり、どれも `#[ignore = "requires a CUDA device"]` を付けている。どの試験も本体を別のスレッドで流し、30 秒以内に終わらなければ失敗とする。カーネルは試験が CUDA C で渡し、`create_pipeline` が NVRTC で PTX にする。

| 試験 | 呼ぶ口 | 照らす値 |
| ---- | ------ | -------- |
| `dispatch_ex_passes_bindings_and_constants_and_launches_the_grid` | 束縛 2 つ・定数 3 つを slot の順と違う並びで渡す `dispatch_ex`（グリッド `[2, 1, 1]`・ブロック `[32, 1, 1]`）と `submit` | 出力の 50 要素が `inp * 3 + 0.5` とビットで同じ。残りの要素は 0 のまま |
| `dispatch_ex_launches_with_the_sum_of_threadgroup_memory_as_dynamic_shared_memory` | 動的な共有メモリを使うカーネルに、同じ `index` の 128 バイトを 2 つ渡す `dispatch_ex` | 2 つのブロックそれぞれで、64 要素が逆順に書き戻される |
| `dispatch_launches_blocks_of_one_thread` | 簡易版の `dispatch`（`[4, 1, 1]`） | 各スレッドが書くブロックの番号は 0〜3、ブロックの大きさは 1 |
| `dispatches_run_in_record_order_and_see_earlier_writes` | 同じ記録先に dispatch・クリア・dispatch を記録して 1 回の `submit` | 後の dispatch の出力が前の dispatch の出力から計算した値と同じ。クリアは前の dispatch の後に効く |
| `dispatch_ex_adds_the_offset_to_the_pointer_without_an_upper_check` | 同じバッファを `offset` 0 と 16・4096 で束縛する `dispatch_ex` | カーネルが書く 2 つのポインタの差が `offset` と同じ（長さ 64 のバッファに 4096 を足しても拒まない。カーネルはポインタの先を読まない） |
| `dispatch_ex_rejects_an_empty_constant_before_recording` | 空の定数を含む `dispatch_ex` | `dispatch_ex: constant slot 3 is empty`。記録先の記録は 0 件のまま |
| `dispatch_ex_rejects_an_omitted_slot_before_recording` | 1 回目は全 slot、2 回目は slot 1 を省いた `dispatch_ex` | 2 回目は `dispatch_ex: slot 1 of the binding layout is not provided`。記録は 1 件のまま |
| `submit_rejects_a_bound_buffer_freed_before_submit_and_enqueues_nothing` | クリアと dispatch を記録し、束縛のバッファを `free` してから `submit` | `a bound buffer was freed before submit (binding slot 1)`。先に記録したクリアも効いていない |
| `submit_rejects_a_parameter_count_mismatch_and_enqueues_nothing` | 仮引数 2 つのカーネルに 3 つの引数を、仮引数 3 つのカーネルに 2 つの引数を記録して、それぞれ `submit` | `submit: the kernel takes 2 parameters but 3 were recorded` と `submit: the kernel takes more than the 2 recorded parameters`。どちらも先に記録したクリアが効いていない |
| `dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_before_recording` | 別の device で作ったパイプラインを渡す `dispatch` と `dispatch_ex` | どちらも `DeviceMismatch`。記録先の記録は 0 件のまま |

| 項目 | 値 |
| ---- | -- |
| module | `crates/contract/src/backend_cuda.rs` の `dispatch_device_tests` |
| feature | `backend-cuda`・`native-blocking-wait` |
| `#[ignore]` | `requires a CUDA device` |

確かめ:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --lib backend_cuda::dispatch_device_tests
```

10 件が `ignored` と数えられる。

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-805。ポインタのみ）。

時点のコミット: `421669af`。`docs/spec` の gitlink は `ac56f928301ffca0c15d857b56302916a3cce0fb`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `421669af` の `crates/contract/src/backend_cuda.rs` の 2 つの module に `grep -c` を当てる | 出力 (1) |
| (2) | 受け手の試験の名前 | `git grep -n` で 19 本の名前を 1 つずつ引く | 出力 (2) |
| (3) | 口の所在 | `git grep -n` で `check_dispatch_record`・`check_dispatch_gate`・`check_kernel_params`・`kernel_param_size`・`prepare_launches`・`launch_kernel`・`DISPATCH_EXECUTION_ENABLED` を 1 つずつ引く | 出力 (3) |
| (4) | 門の試験 | `cargo test -p fandhe-silicon-contract --test backend_cuda_dispatch_gate` | 出力 (4) |
| (5) | device に触らない試験 | `cargo test -p fandhe-silicon-contract` の 2 つの実行 | 出力 (5) |
| (6) | 依存の差 | `68959521^ 421669af` の差と `make deps-check` | 出力 (6) |
| (7) | CI | 3/4 のマージの後の main の push の run | 出力 (7) |

`68959521^` は 1/4 の squash `68959521` の親。(4)(5) の `cargo test` と (6) の `make deps-check` は作業木で流した。

出力 (1):

```text
$ git show 421669af:crates/contract/src/backend_cuda.rs | awk '/^mod dispatch_record_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
9
$ git show 421669af:crates/contract/src/backend_cuda.rs | awk '/^mod dispatch_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[test\]'
10
$ git show 421669af:crates/contract/src/backend_cuda.rs | awk '/^mod dispatch_device_tests \{$/{f=1} f{print} f&&/^}$/{exit}' | grep -c '^[[:space:]]*#\[ignore'
10
```

出力 (2):

```text
$ git grep -n "fn record_orders_buffers_and_constants_by_slot_and_sums_shared_memory(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9531:    fn record_orders_buffers_and_constants_by_slot_and_sums_shared_memory() {
$ git grep -n "fn record_keeps_the_offset_without_an_upper_check(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9577:    fn record_keeps_the_offset_without_an_upper_check() {
$ git grep -n "fn record_rejects_an_empty_constant_with_its_slot(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9601:    fn record_rejects_an_empty_constant_with_its_slot() {
$ git grep -n "fn record_rejects_duplicate_unknown_and_missing_slots_in_this_order(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9633:    fn record_rejects_duplicate_unknown_and_missing_slots_in_this_order() {
$ git grep -n "fn record_rejects_a_shared_memory_total_that_overflows(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9693:    fn record_rejects_a_shared_memory_total_that_overflows() {
$ git grep -n "fn plan_rejects_a_freed_bound_buffer_with_its_slot(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9738:    fn plan_rejects_a_freed_bound_buffer_with_its_slot() {
$ git grep -n "fn gate_rejects_lists_with_a_dispatch_only_when_closed(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9761:    fn gate_rejects_lists_with_a_dispatch_only_when_closed() {
$ git grep -n "fn kernel_params_pass_when_count_and_sizes_match(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9797:    fn kernel_params_pass_when_count_and_sizes_match() {
$ git grep -n "fn kernel_params_reject_size_and_count_mismatches(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9811:    fn kernel_params_reject_size_and_count_mismatches() {
$ git grep -n "fn dispatch_ex_passes_bindings_and_constants_and_launches_the_grid(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9941:    fn dispatch_ex_passes_bindings_and_constants_and_launches_the_grid() {
$ git grep -n "fn dispatch_ex_launches_with_the_sum_of_threadgroup_memory_as_dynamic_shared_memory(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:9981:    fn dispatch_ex_launches_with_the_sum_of_threadgroup_memory_as_dynamic_shared_memory() {
$ git grep -n "fn dispatch_launches_blocks_of_one_thread(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10015:    fn dispatch_launches_blocks_of_one_thread() {
$ git grep -n "fn dispatches_run_in_record_order_and_see_earlier_writes(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10040:    fn dispatches_run_in_record_order_and_see_earlier_writes() {
$ git grep -n "fn dispatch_ex_adds_the_offset_to_the_pointer_without_an_upper_check(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10100:    fn dispatch_ex_adds_the_offset_to_the_pointer_without_an_upper_check() {
$ git grep -n "fn dispatch_ex_rejects_an_empty_constant_before_recording(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10133:    fn dispatch_ex_rejects_an_empty_constant_before_recording() {
$ git grep -n "fn dispatch_ex_rejects_an_omitted_slot_before_recording(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10167:    fn dispatch_ex_rejects_an_omitted_slot_before_recording() {
$ git grep -n "fn submit_rejects_a_bound_buffer_freed_before_submit_and_enqueues_nothing(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10209:    fn submit_rejects_a_bound_buffer_freed_before_submit_and_enqueues_nothing() {
$ git grep -n "fn submit_rejects_a_parameter_count_mismatch_and_enqueues_nothing(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10245:    fn submit_rejects_a_parameter_count_mismatch_and_enqueues_nothing() {
$ git grep -n "fn dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_before_recording(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:10305:    fn dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_before_recording() {
```

出力 (3):

```text
$ git grep -n "fn check_dispatch_record(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:1466:fn check_dispatch_record(
$ git grep -n "fn check_dispatch_gate(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:1731:fn check_dispatch_gate(jobs: &[TransferJob], enabled: bool) -> Result<(), ContractError> {
$ git grep -n "fn check_kernel_params(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:1749:fn check_kernel_params(
$ git grep -n "fn kernel_param_size(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:1782:fn kernel_param_size(
$ git grep -n "fn prepare_launches<" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:2721:    fn prepare_launches<'a>(
$ git grep -n "fn launch_kernel(" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:2816:    fn launch_kernel(&self, launch: &mut PreparedLaunch<'_>) -> Result<(), ContractError> {
$ git grep -n "const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);" 421669af -- crates/contract/src/backend_cuda.rs
421669af:crates/contract/src/backend_cuda.rs:1726:const DISPATCH_EXECUTION_ENABLED: bool = cfg!(test);
```

出力 (4):

```text
$ cargo test -p fandhe-silicon-contract --test backend_cuda_dispatch_gate 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

出力 (5):

```text
$ cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda::dispatch_record_tests 2>&1 | grep -E '^(running|test result:)'
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 137 filtered out; finished in 0.00s
$ cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count 2>&1 | grep -E '^(running|test result:)'
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

出力 (6):

```text
$ git diff --stat 68959521^ 421669af -- Cargo.lock deps-approved.toml Cargo.toml crates/contract/Cargo.toml crates/core
$ make deps-check; echo exit=$?
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
exit=0
```

出力 (7):

```text
$ gh run list --branch main --event push --commit 421669af2fc2ea85caf2f3402e2f5111c625e3c5 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44852903938
$ gh run view 44852903938 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^make (verify|deny)"))|"\(.name): \(.conclusion)")'
success
421669af2fc2ea85caf2f3402e2f5111c625e3c5
make verify + build-profiles (macos-latest): success
make verify (fmt + clippy + test): success
make deny (cargo-deny): success
$ gh run view 44852903938 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136688199610
$ gh run view 44852903938 --job 136688199610 --log | grep -oE 'test backend_cuda::dispatch_(record|device)_tests::[a-z_]+ \.\.\. (ok|ignored, requires a CUDA device)' | sort | uniq -c
      2 test backend_cuda::dispatch_device_tests::dispatch_and_dispatch_ex_reject_a_pipeline_of_another_device_before_recording ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::dispatch_ex_adds_the_offset_to_the_pointer_without_an_upper_check ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::dispatch_ex_launches_with_the_sum_of_threadgroup_memory_as_dynamic_shared_memory ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::dispatch_ex_passes_bindings_and_constants_and_launches_the_grid ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::dispatch_ex_rejects_an_empty_constant_before_recording ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::dispatch_ex_rejects_an_omitted_slot_before_recording ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::dispatch_launches_blocks_of_one_thread ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::dispatches_run_in_record_order_and_see_earlier_writes ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::submit_rejects_a_bound_buffer_freed_before_submit_and_enqueues_nothing ... ignored, requires a CUDA device
      2 test backend_cuda::dispatch_device_tests::submit_rejects_a_parameter_count_mismatch_and_enqueues_nothing ... ignored, requires a CUDA device
      6 test backend_cuda::dispatch_record_tests::gate_rejects_lists_with_a_dispatch_only_when_closed ... ok
      6 test backend_cuda::dispatch_record_tests::kernel_params_pass_when_count_and_sizes_match ... ok
      6 test backend_cuda::dispatch_record_tests::kernel_params_reject_size_and_count_mismatches ... ok
      6 test backend_cuda::dispatch_record_tests::plan_rejects_a_freed_bound_buffer_with_its_slot ... ok
      6 test backend_cuda::dispatch_record_tests::record_keeps_the_offset_without_an_upper_check ... ok
      6 test backend_cuda::dispatch_record_tests::record_orders_buffers_and_constants_by_slot_and_sums_shared_memory ... ok
      6 test backend_cuda::dispatch_record_tests::record_rejects_a_shared_memory_total_that_overflows ... ok
      6 test backend_cuda::dispatch_record_tests::record_rejects_an_empty_constant_with_its_slot ... ok
      6 test backend_cuda::dispatch_record_tests::record_rejects_duplicate_unknown_and_missing_slots_in_this_order ... ok
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | dispatch の記録の時点の照らしと投入の門を実装する | #5727 |
| 2/4 | dispatch のカーネルの起動と仮引数の照らしを実装する | #5728 |
| 3/4 | dispatch の記録と投入の device の要る試験を ignore 付きで足す | #5729 |
| 4/4 | 完了の条件の確かめ | #5730 |
