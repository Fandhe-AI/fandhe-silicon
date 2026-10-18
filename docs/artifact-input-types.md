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
| `CacheKey` | 公開フィールド 8 つ（`source_hash: u64`・`compiler_id: &'static str`・`compiler_version: u64`・`target: &'static str`・`options_hash: u64`・`bounds_checked: bool`・`specialization_hash: u64`・`driver_version: Option<u64>`）の struct。`Copy`・`PartialEq`・`Eq`・`Hash`。`#[non_exhaustive]` ではない |

振る舞いを持つ関数は `DType::byte_size`（1 要素のバイト数を返す）だけ。必要バイト数の計算や宣言の検査はここにない。

## 対応するテスト

| 振る舞い | テスト |
| -------- | ------ |
| 各 variant を外の crate から組み立て、渡した値が読み出せる | `crates/core/tests/execution_pipeline_input.rs` の `execution_source_variants_keep_fields`・`execution_msl_option_variants_keep_fields` |
| `MslMathMode`・`MslFloatFunctions` の `Copy`・等価比較 | 同 `execution_mode_enums_are_copy_and_eq` |
| function constant の件数と中身が保たれる | 同 `execution_function_constants_keep_count_and_values` |
| `DType::byte_size` の値、`OperandSpec` の組み立てと読み出し・`Copy` | `crates/core/tests/execution_operand_spec.rs` の `execution_dtype_byte_sizes`・`execution_dtype_is_copy_and_eq`・`execution_operand_spec_keeps_fields`・`execution_operand_spec_accepts_empty_dims` |
| `CacheKey` の組み立てと読み出し・`Copy`、全フィールドの等価比較、1 フィールドの違いが別のキーになること、等しいキーのハッシュの一致 | `crates/core/tests/execution_cache_key.rs` の `execution_cache_key_keeps_fields`・`execution_cache_key_equal_when_all_fields_equal`・`execution_cache_key_differs_when_one_field_differs`・`execution_cache_key_hash_follows_equality` |
| 各 enum の全 variant の網羅（crate 内で `_` なしの `match`。`PipelineInput` 9・`MslMathMode` 2・`MslFloatFunctions` 2・`FunctionConstantValue` 2・`DType` 5） | `crates/core/src/execution.rs` の `tests` の `execution_pipeline_input_has_nine_variants`・`execution_small_enums_have_expected_variants`・`execution_dtype_has_five_variants` |
| `#[non_exhaustive]` が外れたら落ちること（外の crate で `_` の腕が到達不能になる）と、struct のフィールドの集合（`..` なしの分解） | `crates/core/tests/execution_type_coverage.rs` の `execution_enums_stay_non_exhaustive`・`execution_pipeline_input_stays_non_exhaustive`・`execution_structs_keep_field_sets` |

## CI での走り方

| ジョブ | 呼ぶ make ターゲット | 流れるもの | 確かめ方 |
| ------ | -------------------- | ---------- | -------- |
| `verify` | `make verify` | `--all-features` での `execution_*` の統合テストと `execution` の単体テスト | `scripts/verify.sh` |
| `verify` | `make build-profiles` | `core` を feature なしで clippy（`-D warnings`）と test。`test-support` の要る試験は含まれず、型の試験はそれなしで通る | `grep -n CORE_PKG scripts/build-profiles.sh` |
| `deny` | `make public-api-check` | `core` を production と all-features の 2 組で走査し、wgpu の型の参照が 0 件で合格する | `scripts/public-api-check.sh`、`.github/workflows/ci.yml` |

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --test execution_pipeline_input
cargo test -p fandhe-silicon-core --test execution_operand_spec
cargo test -p fandhe-silicon-core --test execution_cache_key
cargo test -p fandhe-silicon-core --test execution_type_coverage
cargo test -p fandhe-silicon-core --lib execution
cargo test -p fandhe-silicon-core
make public-api-check
make build-profiles
```

`make verify` が流す。GPU の実機は要らない。
