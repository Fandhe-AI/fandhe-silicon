# Metal のパイプラインの reflection の取り出し（TASK-493）の所在と本リポの現状

TASK-493 の作業のうち、Metal の背後実装がパイプラインの reflection から引数の宣言を取り出す部分の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-493。ポインタのみ）。private spec 側の記録に従う（ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 値の型と対応表 | 非公開の `ReflectedDataType`・`ReflectedSize`・`ReflectedBuffer`・`PipelineReflection` と、`expected_data_type`・`classify_reflected_buffer`（device に触らない。どの OS でもビルドされる） | `grep -n 'fn expected_data_type\|fn classify_reflected_buffer' crates/contract/src/backend_metal.rs` |
| Linux の試験の module | `reflection_value_tests`（どの OS でも流れる） | `grep -n 'mod reflection_value_tests' crates/contract/src/backend_metal.rs` |
| 関数の生成 | `imp`（macOS だけ）の中の `MetalDevice::compile_function`。`create_pipeline` の入力の分類・検査・コンパイル・特殊化と同じ手順 | `grep -n 'fn compile_function' crates/contract/src/backend_metal.rs` |
| 表への登録 | `imp` の中の `MetalDevice::register_pipeline`。パイプライン状態と `layout` の写しを表に置き、handle を返す | `grep -n 'fn register_pipeline' crates/contract/src/backend_metal.rs` |
| reflection の取り出し | `imp` の中の `MetalDevice::reflect_function`。表には置かない | `grep -n 'fn reflect_function' crates/contract/src/backend_metal.rs` |
| reflection の読み出し | `imp` の中の自由関数 `read_reflection` | `grep -n 'fn read_reflection' crates/contract/src/backend_metal.rs` |
| 型の写し | `imp` の中の自由関数 `metal_data_type` | `grep -n 'fn metal_data_type' crates/contract/src/backend_metal.rs` |
| macOS の試験の module | `imp::reflection_type_tests`（device に触らない） | `grep -n 'mod reflection_type_tests' crates/contract/src/backend_metal.rs` |
| device の要る試験 | `imp::device_tests::reflection` の `#[ignore]` 付きの 6 本 | `grep -n -A1 '#\[ignore' crates/contract/src/backend_metal.rs \| grep -E 'fn (reflect_function\|msl_and_safe_options_reflect)'` |
| 依存 | `objc2-metal` の feature に `MTLArgument` が加わった。crate と版は変わらない | `grep -n 'objc2-metal' crates/contract/Cargo.toml` |
| cfg | feature `backend-metal`。取り出しは `imp`（macOS だけ）の中。値の型・2 つの関数・`reflection_value_tests` はどの OS でも流れる | `crates/contract/src/lib.rs` |

## 値の型と対応表（1/4）

- `ReflectedDataType` は `Float`・`Half`・`UInt`・`Int`・`Bool`・`Struct`・`Other(u64)`。
- `ReflectedSize` は `Bytes(u64)` と `Unchecked`。
- `ReflectedBuffer` は `slot`・`size`・`data_type`。
- `PipelineReflection` は `argument_count` と `buffers`。
- `expected_data_type` は `F32`→`Float`・`F16`→`Half`・`U32`→`UInt`・`I32`→`Int`・`Bool`→`Bool`。表に無い `DType` は `None`。
- `classify_reflected_buffer` は index を `u32`、バイト長を `u64` に変換し、収まらなければ `BackendFailure`（`op` は `create_pipeline`）。バイト長 0 は `Unchecked`、それ以外は `Bytes`。型は変えずに持つ。

確かめ方は次のとおり。

```text
cargo test -p fandhe-silicon-contract --features backend-metal --lib reflection_value_tests
```

| 試験 | 入力と期待 |
| ---- | ---------- |
| `expected_data_type_maps_five_dtypes` | 5 つの `DType` が上の対応に写る |
| `mapped_types_pair_with_byte_size_and_exclude_struct_other` | 対応表の値と `DType::byte_size` の組を並べ、`Struct` と `Other` はどれにも当たらない |
| `classify_keeps_slot_size_and_type` | index 3・バイト長 4・`Float` は slot 3・`Bytes(4)`・`Float` |
| `classify_zero_size_is_unchecked` | バイト長 0 は `Unchecked`、型はそのまま |
| `classify_struct_keeps_size` | `Struct` とバイト長 12 は `Bytes(12)` |
| `classify_other_type_is_kept` | `Other(7)` は値のまま持つ |
| `classify_index_over_u32_fails` | index が `u32` に収まらないと `BackendFailure`（64 ビットだけ） |
| `classify_index_u32_max_passes` | index が `u32::MAX` ちょうどなら通る |
| `pipeline_reflection_holds_count_and_buffers` | `PipelineReflection` を組み、引数の数とバッファの並びを読める |

## reflection の取り出し（2/4）

- `create_pipeline` は `compile_function` → `newComputePipelineStateWithFunction_error` → `register_pipeline` の 3 段。
- `reflect_function` は関数から `newComputePipelineStateWithDescriptor_options_reflection_error`（`MTLPipelineOption::BindingInfo`）でパイプライン状態を作り、`PipelineReflection` とあわせて返す。表の項目の数は変わらない。
- `read_reflection` は `arguments()` を読み、`argument_count` は引数の件数、`buffers` は種別 `Buffer` の引数を slot の昇順に並べる。
- `metal_data_type` は `Float`・`Half`・`UInt`・`Int`・`Bool`・`Struct` の 6 つを写し、ほかは `Other` に値を持つ。
- `imp::reflection_type_tests` は device に触らない 2 本。`the_six_mapped_types_are_copied` は 6 つの写しを、`a_type_outside_the_table_keeps_its_raw_value` は表の外の型が `Other` に値のまま入ることを確かめる。

## device の要る試験（2/4・3/4）

どれも `#[ignore = "touches the Metal device; run only in a dedicated device session"]` 付き。

| 試験 | カーネルの引数 | 期待 |
| ---- | -------------- | ---- |
| `reflect_function_reads_one_float_buffer_and_leaves_the_table_alone` | `device float* a [[buffer(0)]]` | 引数 1・`(0, Bytes(4), Float)`。`reflect_function` の前後で表の数は同じ、`register_pipeline` で 1 増え、handle の device の識別子が一致する |
| `reflect_function_reads_the_gemm_kernel_arguments` | GEMM のカーネル | 引数 4。`(0, 4, Float)`・`(1, 4, Float)`・`(2, 4, Float)`・`(3, 12, Struct)` |
| `reflect_function_reads_the_relu_kernel_arguments` | Relu のカーネル | 引数 3。`(0, 4, Float)`・`(1, 4, Float)`・`(2, 4, UInt)` |
| `reflect_function_reads_the_topk_kernel_arguments` | 部分 Top-k のカーネル | 引数 4。`(0, 4, Float)`・`(1, 4, UInt)`・`(2, 4, Float)`・`(3, 8, Struct)` |
| `msl_and_safe_options_reflect_the_same_arguments_and_create_pipeline` | 上の 3 つのカーネルを `Msl` と `MslWithOptions { Safe }` で | どちらの取り出しも同じ値、どちらの `create_pipeline` も `Ok` |
| `reflect_function_maps_five_dtypes_by_slot_order` | 宣言の順と slot をずらした 6 引数 | slot の昇順に並ぶ。slot 0〜4 の型とバイト長が `expected_data_type`・`DType::byte_size` と一致し、slot 5 は `Float`・4 |

## 完了の条件の確かめ（4/4）

4/4（#4940）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-493）。GPU を使わない Linux で、下の base で実行した値を書く。

時点のコミットは `da4f3d1`、`docs/spec` の gitlink は `5f6d802c905938bba1ed963340029d61fe3651a0`。1/4 の親のコミットは `ff18807`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 取り出しの関数と対応表がある | `grep -n 'fn reflect_function\|fn read_reflection\|fn expected_data_type\|fn metal_data_type' crates/contract/src/backend_metal.rs` | 本体の 4 件に加えて、名前が前方一致する試験の `fn` の行。下の (1) の出力 |
| 2 | 対応表と値の分け方の試験（Linux） | `cargo test -p fandhe-silicon-contract --features backend-metal --lib reflection_value_tests` | `running 9 tests`、`test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 151 filtered out` |
| 3 | macOS の型の写しの試験 | `gh run view 43213722739 --log --job 131167546139 \| grep reflection_type_tests`（2/4 のマージの後の main の push の run の `make verify + build-profiles (macos-latest)`） | run の conclusion は `success`。`the_six_mapped_types_are_copied` と `a_type_outside_the_table_keeps_its_raw_value` が、どちらも `ok` の行で出る（ログの行は 14 行で、各 7 行） |
| 4 | device の要る試験の宣言 | `grep -cE '^\s*#\[ignore' crates/contract/src/backend_metal.rs`・`grep -n -A1 '#\[ignore' crates/contract/src/backend_metal.rs \| grep -E 'fn (reflect_function\|msl_and_safe_options_reflect)'` | `backend_metal.rs` は 68 件（`ff18807` では 62）。名前に `reflect` を含む `fn` は 6 件で、名前は上の表のとおり。下の (4) の出力 |
| 5 | `create_pipeline` の生成の呼び出しと reflection の呼び出し | `grep -n 'newComputePipelineStateWithFunction_error\|newComputePipelineStateWithDescriptor_options_reflection_error' crates/contract/src/backend_metal.rs` | 2 件。前者は `create_pipeline`、後者は `reflect_function` の中。下の (5) の出力 |
| 6 | unsafe 0・依存の crate 0 | `cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count --test backend_metal_no_recording`・`git diff --stat ff18807 origin/main -- Cargo.lock deps-approved.toml`・`git diff ff18807 origin/main -- crates/contract/Cargo.toml`・`git diff ff18807 origin/main -- crates/contract/src \| grep -c '^+.*unsafe'` | `running 6 tests` と `running 1 test` が、どちらも `test result: ok`（6 passed、1 passed）。`Cargo.lock`・`deps-approved.toml` の差分は空。`crates/contract/Cargo.toml` の差分は `objc2-metal` の行の `"MTLArgument"` の 1 語。`unsafe` を含む追加行は 0 件 |

(1) の出力（`fn` の行）。

```text
1835:fn expected_data_type(dtype: DType) -> Option<ReflectedDataType> {
3477:    fn read_reflection(
3499:    fn metal_data_type(t: MTLDataType) -> ReflectedDataType {
3598:        fn reflect_function(
3992:            fn reflect_function_reads_one_float_buffer_and_leaves_the_table_alone() {
9498:    fn expected_data_type_maps_five_dtypes() {
```

(3) の出力（重複を除いた `ok` の行）。

```text
test backend_metal::imp::reflection_type_tests::a_type_outside_the_table_keeps_its_raw_value ... ok
test backend_metal::imp::reflection_type_tests::the_six_mapped_types_are_copied ... ok
```

(4) の出力（`fn` の行）。

```text
3992-            fn reflect_function_reads_one_float_buffer_and_leaves_the_table_alone() {
4117-            fn reflect_function_reads_the_gemm_kernel_arguments() {
4138-            fn reflect_function_reads_the_relu_kernel_arguments() {
4158-            fn reflect_function_reads_the_topk_kernel_arguments() {
4179-            fn msl_and_safe_options_reflect_the_same_arguments_and_create_pipeline() {
4207-            fn reflect_function_maps_five_dtypes_by_slot_order() {
```

(5) の出力（呼び出しの行）。

```text
3613:                .newComputePipelineStateWithDescriptor_options_reflection_error(
3656:                .newComputePipelineStateWithFunction_error(&function)
```

## 段

| 段 | 内容 | Issue | PR |
| -- | ---- | ----- | -- |
| 1/4 | reflection の値の型と引数の型の対応表を足す | #4937 | #8245 |
| 2/4 | パイプラインの生成の reflection から引数の宣言を取り出す | #4938 | #8246 |
| 3/4 | 3 演算と型の対応表の reflection の device の要る試験を `#[ignore]` 付きで足す | #4939 | #8247 |
| 4/4 | 完了の条件の確かめ | #4940 | 本頁 |

## 今の値の補い（base 3cd4a0c）

記録時点の表と既存の節は書き換えない。以下は base `3cd4a0c` で観測した今の値（実行環境は GPU なしの Linux）。

- feature `artifact-inspection` のビルドの `create_pipeline_checked_from_metallib`（metallib のファイルから作る経路）が `reflect_function` を呼ぶ。確かめ方: `grep -n 'self.reflect_function' crates/contract/src/backend_metal.rs`
- 確かめ表の (4) の行頭の `#[ignore` の件数: 68 件 → 85 件（`grep -cE '^\s*#\[ignore' crates/contract/src/backend_metal.rs`）。名前に `reflect` を含む `fn` は 6 件のまま。
- metallib から作る経路の所在と試験は [metal-artifact-hooks](metal-artifact-hooks.md) に置く（TASK-492）。
