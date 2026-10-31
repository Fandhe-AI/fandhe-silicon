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

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --test dispatch_default_impl
cargo test -p fandhe-silicon-core --test compute_ops_shape
cargo test -p fandhe-silicon-core --doc
make verify && make check
make public-api-check && make public-name-check && make public-api-webgpu-check && make public-api-sdk-check && make core-deps-check
```
