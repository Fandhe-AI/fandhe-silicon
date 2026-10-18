# 成果物の受け付けに使う公開の型（TASK-470）の所在と現状の事実

本リポのコードと公開テストから説明できる振る舞いを記録する。記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-470。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/execution.rs` | `crates/core/src/lib.rs` |
| feature | 条件なし（feature の裏ではない） | `crates/core/src/lib.rs` |
| 依存 | 外部依存 0。`unsafe` を使わない（`core` は `#![forbid(unsafe_code)]`） | `Cargo.lock` |

## 現状の型

| 型 | 形 |
| -- | -- |
| `PipelineInput<'a>` | `#[non_exhaustive]` の enum。variant は 9 つ（`Msl`・`SpirV`・`CudaC`・`Ptx`・`Dxil`・`Wgsl`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions`）。成果物と入口名は借用で持つ |
| `MslMathMode` | `#[non_exhaustive]` の enum（`Safe`・`Fast`）。`Copy`・`PartialEq`・`Eq` |
| `MslFloatFunctions` | `#[non_exhaustive]` の enum（`Fast`・`Precise`）。`Copy`・`PartialEq`・`Eq` |
| `FunctionConstant` | 公開フィールド `index: u32`・`value: FunctionConstantValue` の struct。`Copy` |
| `FunctionConstantValue` | `#[non_exhaustive]` の enum（`U32`・`Bool`）。`Copy` |
| `DType` | `#[non_exhaustive]` の enum（`F32`・`F16`・`U32`・`I32`・`Bool`）。`Copy`・`PartialEq`・`Eq` |
| `OperandSpec<'a>` | 公開フィールド `slot: u32`・`dtype: DType`・`shape: &[u64]`・`stride: &[i64]`・`alignment: u32` の struct。`Copy`。形と stride は借用 |

振る舞いを持つ関数は `DType::byte_size`（1 要素のバイト数を返す）だけ。必要バイト数の計算や宣言の検査はここにない。

## 対応するテスト

| 振る舞い | テスト |
| -------- | ------ |
| 各 variant を外の crate から組み立て、渡した値が読み出せる | `crates/core/tests/execution_pipeline_input.rs` の `execution_source_variants_keep_fields`・`execution_msl_option_variants_keep_fields` |
| `MslMathMode`・`MslFloatFunctions` の `Copy`・等価比較 | 同 `execution_mode_enums_are_copy_and_eq` |
| function constant の件数と中身が保たれる | 同 `execution_function_constants_keep_count_and_values` |
| `DType::byte_size` の値、`OperandSpec` の組み立てと読み出し・`Copy` | `crates/core/tests/execution_operand_spec.rs` の `execution_dtype_byte_sizes`・`execution_dtype_is_copy_and_eq`・`execution_operand_spec_keeps_fields`・`execution_operand_spec_accepts_empty_dims` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --test execution_pipeline_input
cargo test -p fandhe-silicon-core --test execution_operand_spec
```

`make verify` が流す。GPU の実機は要らない。
