# 試験の枠が取った device を照らす口（TASK-2945）の所在と、本リポの現状の事実

共通の crate の試験の枠（`test_frame`）にある、試験の側が取った device を走らせる組み合わせと照らす口の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-2945。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/core/src/test_frame/device_check.rs` の `on_acquired_device`。`fandhe_silicon_core::test_frame::on_acquired_device` として再公開される | `crates/core/src/test_frame.rs` |
| 試験 | `crates/core/tests/test_frame_device_check.rs`（7 本） | `cargo test -p fandhe-silicon-core --features test-support --test test_frame_device_check` |
| 有効になる条件 | feature `test-support`（`test_frame` と同じ。feature なしのビルドには現れない） | `cargo build -p fandhe-silicon-core` |
| unsafe | 無い（`crates/core` は `#![forbid(unsafe_code)]`） | `crates/core/src/lib.rs` |
| 依存 | 足していない。`[[test]]` に `required-features = ["test-support"]` を付けただけ | `crates/core/Cargo.toml`・`make deps-check` |

## 照らす順と返す値（1/3）

`on_acquired_device(combination, acquire, machine_of, body)` は、試験の本体の閉包の中から呼ぶ。`acquire` は device を取る関数、`machine_of` は device の診断の値を機材の呼び名に写す関数で、どちらも試験の側が渡す。この口は device を取らず、写しの規則を持たない。

| 順 | 当たる条件 | 返す値 | `machine_of` | `body` |
| -- | ---------- | ------ | ------------ | ------ |
| 1 | `acquire(combination)` が `Err` | `RunResult::Indeterminate(IndeterminateReason::MachineUnreachable)` | 呼ばない | 呼ばない |
| 2 | 取れた device の `backend()` が組み合わせの背後実装（`Metal`・`Vulkan`・`Cuda` に当たる `BackendKind`）と違う | `RunResult::Indeterminate(IndeterminateReason::DeviceMismatch)` | 呼ばない | 呼ばない |
| 3 | `machine_of(&device.diagnostics())` が `Some(combination.machine())` でない | `RunResult::Indeterminate(IndeterminateReason::DeviceMismatch)` | 1 回呼ぶ | 呼ばない |
| 4 | 上のどれにも当たらない | `body(combination, &device)` の戻り値 | 1 回呼ぶ | 1 回呼ぶ |

- 1 は `ContractError` の種類を見ない
- `judge` が組み合わせを走らせないと決めた経路では本体が呼ばれないので、`acquire` も呼ばれない

### 試験との対応

| 動き | 試験 |
| ---- | ---- |
| 1 | `an_acquire_failure_is_machine_unreachable_and_neither_the_map_nor_the_body_is_called` |
| 3（`None`・別の機材） | `an_unknown_device_is_device_mismatch_and_the_body_is_not_called`・`another_machine_is_device_mismatch` |
| 2 | `another_backend_is_device_mismatch_even_when_the_map_agrees` |
| 4 | `a_matching_device_passes_the_combination_and_the_device_to_the_body_once` |
| `judge` を通した走らせない経路で device を取らない | `through_judge_the_not_run_path_does_not_acquire` |
| `judge` を通した各経路のラベル | `through_judge_the_paths_give_their_labels` |

試験は `StubDeviceProvider` の device と、試験の中の写しの閉包を使う。device の要る経路は流さない。

```sh
cargo test -p fandhe-silicon-core --features test-support --test test_frame_device_check
```

## Metal の宣言の試験（2/3）

`crates/contract/tests/test_support_test_frame.rs` の `metal_test_declares_on_the_frame`（feature `backend-metal`）は、Metal の組み合わせを宣言し、枠が走らせると決めた経路で `on_acquired_device` を通す。

- 取る: `MetalDeviceProvider::request` で device を取る（macOS のビルドだけ。macOS 以外のビルドは `DeviceUnavailable` を返すので、機材に届かない判定不能になる）。
- 照らす: Metal の device の診断の値が、試験のファイルの写しに当たるときだけ、走らせる組み合わせの機材に当たる。当たらなければ本体を呼ばず判定不能になる。
- 往復の本体: 64 バイトのバッファを確保し、書いたバイト列を `download_async` で読み戻して比べる（待ちは 10 秒まで）。確保に失敗した、読み戻せなかった、打ち切られた、内容が違うときは不一致になる。バッファは最後に解放する。
- 走らせる経路では部品を 1 件書く。走らせない経路では宣言した組み合わせごとに 1 件書く。
- 確かめるのは `fails_test` が偽であることだけで、`Label::Ok` は求めない。`#[ignore]` は付いていない。組み合わせの指定が無い環境（CI を含む）では device を取らない。

Vulkan・CUDA の宣言の試験は、走らせない経路でだけ部品を書く形のまま。`a_run_path_writes_no_record_and_a_not_run_path_writes_one_per_declared_combination` の背後実装の列は Vulkan・CUDA の 2 つ。

## 食い違いと取れない device の試験（2/3 の続き）

同じファイルの補助 `export_with_device` と写し `metal_machine` を、本物の試験と同じ形で通す試験。device には触らない。

| 試験 | feature・ビルド | 入力 | 結果 |
| ---- | --------------- | ---- | ---- |
| `the_device_path_writes_one_indeterminate_part_when_the_device_does_not_match_or_is_unreachable` | `backend-metal` | 組み合わせの指定は `metal-mac-mini`。(a) 試験用の device、(b) `DeviceUnavailable` | `ran_on` は `MetalMacMini`、一時ディレクトリに 1 件。`device_path_check.metal-mac-mini.json` が `"label":"indeterminate"`・`"label_reason":"device-mismatch"`（a）・`"machine-unreachable"`（b） |
| `the_device_path_does_not_acquire_when_the_frame_does_not_run` | 条件なし | 組み合わせの指定なし。取る関数・写し・本体はどれも呼ばれたら落ちる | `ran_on` は `None`。部品は Metal の組み合わせの数だけ書かれる |
| `an_injected_metal_request_failure_is_machine_unreachable` | `backend-metal`・`fault-injection`・macOS のビルドだけ | `MetalDefaultDeviceNoneProvider::request` を取る関数にする | ラベルは `Label::Indeterminate(IndeterminateReason::MachineUnreachable)` |

どの試験も記録のディレクトリの環境変数を読まず、自分の一時ディレクトリにだけ書く。

## 段の表

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 試験の枠に取った device を走らせる組み合わせと照らす口 | #7097 |
| 2/3 | Metal の宣言の試験を取った device に触る試験にし、判定不能の試験を足す | #7098 |
