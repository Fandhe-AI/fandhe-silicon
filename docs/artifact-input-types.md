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

振る舞いを持つ関数は `DType::byte_size`（1 要素のバイト数を返す）だけ。必要バイト数の計算と宣言の C4〜C8 の検査は同じ `execution` の別の関数にあり、[dispatch-operand-check.md](dispatch-operand-check.md) に書く。

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

## 完了の条件の確かめ（6/6）

6/6（#4894）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-470）。
GPU の実機なしで、末尾の base で実行した値。件数は実測。

| 成果物 | 本書の節 | 試験 |
| ------ | -------- | ---- |
| 型の実装 | 現状の型 | `crates/core/src/execution.rs` |
| 型の試験 | 対応するテスト | `execution_pipeline_input`・`execution_operand_spec`・`execution_cache_key`・`execution_type_coverage`・`--lib execution` |

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 列挙型の網羅（`_` なしの match） | `cargo test -p fandhe-silicon-core --lib execution` | 3 passed; 0 failed; 0 ignored（`execution_pipeline_input_has_nine_variants`・`execution_small_enums_have_expected_variants`・`execution_dtype_has_five_variants`） |
| A2 | `#[non_exhaustive]` の保持と struct のフィールドの集合 | 同 `--test execution_type_coverage` | 3 passed; 0 failed; 0 ignored |
| A3 | 各 variant の組み立てと読み出し | 同 `--test execution_pipeline_input` | 4 passed; 0 failed; 0 ignored |
| A4 | `DType` のバイト数 | 同 `--test execution_operand_spec` | 4 passed; 0 failed; 0 ignored |
| A5 | `CacheKey` の等価比較とハッシュ | 同 `--test execution_cache_key` | 4 passed; 0 failed; 0 ignored |
| A6 | feature なしでの試験 | `cargo test -p fandhe-silicon-core`・`make build-profiles` | 型の試験 4 本と単体がすべて ok、`build-profiles` は終了コード 0 |
| A7 | 公開 API に wgpu の型が無いこと | `make public-api-check` | `core` の `features=production`・`features=all-features` の 2 行がともに `wgpu_type_refs=0 wgpu_other_refs=0`、終了コード 0 |
| A8 | base の CI | main の base の commit を指定した push の run の一覧 | run 39121355696 が success（`make verify`・`make deny`・`make check`・`ci-complete` とも success） |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles` | いずれも終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-470）。

### 再実行の契機

- `crates/core/src/execution.rs` を変えたとき
- 上の 4 本の試験ファイルを変えたとき
- `scripts/public-api-check.sh`・`scripts/build-profiles.sh` を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `a8e36ad13b12ef9998d8d4ca6ed6215d1207764f`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。
