# compute の dispatch に渡す公開の型と簡易版 dispatch の既定の本体（TASK-769）の所在と本リポの現状

TASK-769 の作業の所在と、本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
対応づけは TASK-415、COMP-25、COMP-26・COMP-27・COMP-30・COMP-34、BIND-1・BIND-2・BIND-3 のポインタだけにする。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-769。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 公開に足した名前

型は `crates/core/src/execution/compute.rs`（`fandhe_silicon_core::execution`。feature の条件なし）。

| 名前 | 種類 |
| ---- | ---- |
| `PipelineHandle` | 構造体（欄は非公開。`Debug` のみで `Clone`・`Copy` なし。`__backend_new`・`__backend_id`・`__backend_device_id` は `#[doc(hidden)]` の背後実装向けの口） |
| `Binding<'a>` | 構造体（`#[non_exhaustive]`。`slot`・`buffer`・`offset` が `pub`。`new` で作る。`Debug`・`Clone`・`Copy`） |
| `ConstantBinding<'a>` | 構造体（`#[non_exhaustive]`。`slot`・`bytes` が `pub`。`new` で作る。`Debug`・`Clone`・`Copy`） |
| `ThreadgroupMemory` | 構造体（`#[non_exhaustive]`。`index`・`bytes` が `pub`。`new` で作る。`Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`） |
| `BindingLayout` | 構造体（`#[non_exhaustive]`。`slots: Vec<u32>` が `pub`。`new` で作り、検査をせず重複や並びもそのまま持つ。`Debug`・`Clone`・`PartialEq`・`Eq`） |
| `ComputeOps` | trait（必須のメソッドは `create_pipeline(&self, PipelineInput<'_>, &BindingLayout) -> Result<PipelineHandle, ContractError>` の 1 本。既定の本体を持つメソッドは無い。本リポに実装は無い） |

`CommandQueue`（`crates/core/src/execution/submit.rs`）に次の 2 つを足した。

| メソッド | 事実 |
| -------- | ---- |
| `dispatch_ex` | 既定の本体を持たない必須のメソッド。引数は記録先・パイプライン・バッファの束縛・定数の束縛・起動の大きさ・スレッドグループの大きさ・共有メモリの指定 |
| `dispatch` | 既定の本体で、空の定数・`[1, 1, 1]`・空の共有メモリを渡して `dispatch_ex` を呼び、結果をそのまま返す。自分では検査しない |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 既定の委譲 | `dispatch` は `dispatch_ex` をちょうど 1 回呼び、束縛（slot・offset・バッファ）・起動の大きさ・パイプラインをそのまま渡す | `crates/core/tests/dispatch_default_impl.rs` |
| 補う値 | 定数は空、スレッドグループの大きさは `[1, 1, 1]`、共有メモリは空 | 同上 |
| 結果 | `dispatch_ex` の `Ok(())` と `Err(..)` がそのまま返る。束縛が空でも 2 件以上でも同じ | 同上 |
| 構築の口 | 外の crate は 4 つの型を構造体式で作れず、`new`（`PipelineHandle` は `__backend_new`）を使う | `compute.rs` の rustdoc（`compile_fail,E0639`・`E0451`） |
| 必須のメソッド | `dispatch_ex` を持たない `CommandQueue` の実装は拒まれる。持てば通る | `submit.rs` の rustdoc（`compile_fail,E0046`） |
| 検査 | 4 つの型の作成は検査をせず、渡された値をそのまま写す | `dispatch_default_impl.rs` |
| `BindingLayout` | `new` で渡した `slots` をそのまま持つ。空・重複を含む並びも同じ。`Clone` と比較が使える | `crates/core/tests/compute_ops_shape.rs` |
| `ComputeOps` | 実装は渡された `PipelineInput` と `BindingLayout` をそのまま受け取り、返した `Ok(PipelineHandle)`・`Err(..)` がそのまま呼び出し側へ戻る。`&dyn ComputeOps` として呼べる | 同上 |
| 構築の口（`BindingLayout`） | 外の crate は構造体式で作れず、`new` を使う | `compute.rs` の rustdoc（`compile_fail,E0639`） |
| 必須のメソッド（`ComputeOps`） | `create_pipeline` を持たない実装は拒まれる。持てば通る | `compute.rs` の rustdoc（`compile_fail,E0046`） |
| 外部依存 | 足していない。`unsafe` は無い | `make deps-check` |

## 対応するテスト

| テスト | 内容 |
| ------ | ---- |
| `crates/core/tests/dispatch_default_impl.rs` | `dispatch` の既定の委譲、補う値、結果のそのままの返却、各型の値の保持 |
| `crates/core/tests/compute_ops_shape.rs` | `BindingLayout` の保持、`ComputeOps` の引数と結果のそのままの受け渡し、object safe |
| `compute.rs` の rustdoc | 各型（`BindingLayout` を含む）の構造体式が拒まれること、`create_pipeline` を持たない `ComputeOps` の実装が拒まれること。`new` / `__backend_new` で作れる陽性対照つき |
| `submit.rs` の rustdoc | `dispatch_ex` を持たない実装が拒まれること。持てば通る陽性対照つき |
| `submit.rs` の rustdoc（`dispatch_ex`、`compile_fail,E0382`） | 投入した記録先を `dispatch` へ渡すと所有権の移動のエラーになること（TASK-772） |

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --test dispatch_default_impl
cargo test -p fandhe-silicon-core --test compute_ops_shape
cargo test -p fandhe-silicon-core --doc
make verify && make check
make public-api-check && make public-name-check && make public-api-webgpu-check && make public-api-sdk-check && make core-deps-check
```

## 今の値

「公開に足した名前」の `ComputeOps` の行の「本リポに実装は無い」は記録時点の値で、書き換えない。今は Metal の device（`crates/contract/src/backend_metal.rs`）と試験用の device（`crates/core/src/stub_device.rs` の非公開の型）が実装する。`Device` から届く読み出しは `Device::compute()`（[device-compute](device-compute.md)）。

## 今の値の補い（base dd1a116）

記録時点の表と既存の節は書き換えない。以下は base `dd1a116` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「公開に足した名前」の `ComputeOps` の行（既定の本体を持つメソッドは無い） | 既定の本体を持つメソッドは無い → `ComputeOps::pipeline_cache_key` が既定の本体（`None`）を持つ。必須のメソッドは `create_pipeline` のまま | `grep -n 'fn pipeline_cache_key' -A8 crates/core/src/execution/compute.rs`、`cargo test -p fandhe-silicon-core --test compute_ops_shape default_pipeline_cache_key_is_none` |
| 「今の値」の節の実装の一覧（Metal の device と試験用の device） | Metal の device と試験用の device が実装する → `crates/contract/src/backend_cuda.rs` の `CudaDevice` も `ComputeOps` を実装する | `grep -n 'impl ComputeOps for' crates/contract/src/backend_cuda.rs crates/contract/src/backend_metal.rs crates/core/src/stub_device.rs` |
| 同じ行（`pipeline_cache_key` の後） | `ComputeOps::create_pipeline_checked(&self, PipelineInput<'_>, &BindingLayout, &[OperandSpec<'_>]) -> Result<PipelineHandle, ContractError>` も既定の本体を持つ。宣言を `validate_declared_operands` で照らし、通れば `create_pipeline` を 1 回呼び、拒否のときは呼ばない（[cuda-create-pipeline-checked](cuda-create-pipeline-checked.md)） | `grep -n 'fn create_pipeline_checked' -A8 crates/core/src/execution/compute.rs`、`cargo test -p fandhe-silicon-core --test compute_create_pipeline_checked` |

## 今の値の補い（base 70ab958）

記録時点の表と既存の節は書き換えない。以下は base `70ab958` で観測した今の値（実行環境は GPU なしの Linux）。

訂正: 「今の値の補い（base dd1a116）」の表のうち `create_pipeline_checked` の行は、base `dd1a116` の値ではない。`dd1a116` の `crates/core/src/execution/compute.rs` に `create_pipeline_checked` は無く、その行の値は TASK-457 の 1/4 の後（コミット `cb8e391`）の値である。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「今の値の補い（base dd1a116）」の `create_pipeline_checked` の行の時点 | base `dd1a116` → TASK-457 の 1/4（`cb8e391`）の後。今も `create_pipeline_checked` は既定の本体を持ち、`validate_declared_operands` を通ってから `create_pipeline` を呼ぶ | `git show dd1a116:crates/core/src/execution/compute.rs \| grep -c create_pipeline_checked`（0）、`git log --oneline -S'fn create_pipeline_checked' -- crates/core/src/execution/compute.rs` |
| 同じ表の `ComputeOps` の既定の本体を持つメソッド | `pipeline_cache_key` → `create_pipeline_checked` と `pipeline_cache_key` の 2 つ（必須は `create_pipeline` のまま） | `grep -n '^    fn ' crates/core/src/execution/compute.rs` |

## 今の値の補い（base 2847d3c）

記録時点の表と既存の節は書き換えない。以下は base `2847d3c` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「公開に足した名前」の `CommandQueue` の表（`dispatch_ex` は必須、`dispatch` は既定の本体） | 2 つの行のとおり（変わらない）。`CommandQueue`（`crates/core/src/execution/submit.rs`）の必須のメソッドは、`dispatch_ex` のほかに `create_encoder`・`submit`・`submit_many`・`poll`・`on_complete`・`max_buffer_bytes`（feature `native-blocking-wait` の裏に `wait_blocking`）。既定の本体を持つメソッドは、`dispatch` のほかに `dispatch_checked`・`poll_ex`・`submit_ex`・`gpu_timing`（feature `ext-interop` の裏に `submit_exportable`）。`dispatch_checked` は宣言を照らしてから `dispatch_ex` を呼ぶ（[dispatch-checked-default.md](dispatch-checked-default.md)） | `awk 'NR>=277 && /^    fn / {print NR": "$0}' crates/core/src/execution/submit.rs`、`sed -n '/fn dispatch_ex/,/;$/p' crates/core/src/execution/submit.rs`、`grep -n 'fn max_buffer_bytes' crates/core/src/execution/submit.rs` |
| 「本リポの現状」の「必須のメソッド」の行（`dispatch_ex` を持たない実装は拒まれる） | `dispatch_ex` を持たない実装が拒まれる点は同じ。`submit.rs` の rustdoc の陽性対照の実装は `max_buffer_bytes` も持つ | `sed -n 380,440p crates/core/src/execution/submit.rs`、`cargo test -p fandhe-silicon-core --doc execution::submit` |
