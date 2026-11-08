# 成果物のキャッシュキーの口（TASK-501）の所在と本リポの現状

`ComputeOps` が成果物のキャッシュキーを返す口の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-501。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 口 | `crates/core/src/execution/compute.rs` の `ComputeOps::pipeline_cache_key`（TASK-769 のポインタ） | `grep -n 'pipeline_cache_key' crates/core/src/execution/compute.rs` |
| キーの型 | `crates/core/src/execution.rs` の `CacheKey`（TASK-470 のポインタ） | `grep -n 'pub struct CacheKey' crates/core/src/execution.rs` |
| 試験 | `crates/core/tests/compute_ops_shape.rs` の `default_pipeline_cache_key_is_none` と、`compute.rs` の doctest | 下の「キャッシュキーの口の既定（1/4）」 |

## キャッシュキーの口の既定（1/4）

- シグネチャは `fn pipeline_cache_key(&self, _input: &PipelineInput<'_>, _bounds_checked: bool) -> Option<CacheKey>`。
- 既定の本体は入力を読まずに `None` を返す。パイプラインは作らない。
- 上書きしない実装は `None` を返す。今は試験用の device の compute の口がそうである。
- 試験 `default_pipeline_cache_key_is_none` は、`Msl` と `Wgsl` の 2 入力・`bounds_checked` の 2 値で `None` になることと、`create_pipeline` が呼ばれないことを確かめる。
- 確かめ方: `cargo test -p fandhe-silicon-core --test compute_ops_shape`・`cargo test -p fandhe-silicon-core --doc execution::compute`

## Metal のキャッシュキー（2/4）

`crates/contract/src/backend_metal.rs` の `MetalDevice` が `pipeline_cache_key` を上書きする。MSL の 4 つの入力で `Some` を返し、ほかの入力と、知らない値を含む入力では `None` を返す。パイプラインは作らない。欄は、device に触らない非公開の関数 `metal_cache_key` が組み立てる。

| 入力 | `options_hash` の材料 | `specialization_hash` の材料 |
| ---- | -------------------- | --------------------------- |
| `Msl` | `fast-math(default)` | `entry=<入口名>` |
| `MslSpecialized` | `fast-math(default)` | `entry=<入口名>` に定数ごとの `;idx=<番号>:U32(<値>)`／`;idx=<番号>:Bool(<値>)` |
| `MslWithOptions` | `math_mode=<Safe\|Fast>` | `entry=<入口名>` |
| `MslSpecializedWithOptions` | `math_mode=<Safe\|Fast>;float_functions=<Fast\|Precise>` | `MslSpecialized` と同じ |

- 材料の文字列の `sha256_u64` が各欄に入る。
- 4 つの入力に共通の欄: `source_hash` はソースの `sha256_u64`、`compiler_id` は `"metal"`、`target` は `"metal-msl"`、`bounds_checked` は引数の値。
- `compiler_version` と `driver_version`（`Some`）は、OS の版の文字列と device 名を NUL でつないだ文字列の `sha256_u64` で、2 欄とも同じ値。呼ぶたびに `NSProcessInfo` の `operatingSystemVersionString` と `MTLDevice::name` を読む。
- `crates/contract/Cargo.toml` の `objc2-foundation` の features に `NSProcessInfo` を足した（版は変えていない）。
- 試験: `precheck_tests` の `cache_key_is_the_same_for_the_same_input`・`changing_one_input_changes_only_its_field`・`cache_key_fields_for_each_msl_input`・`specialized_with_options_keys_do_not_mix_with_other_inputs`・`non_msl_inputs_have_no_cache_key`・`cache_key_version_joins_os_version_and_device_name_with_nul`。macOS でだけコンパイルされる `imp::tests` の `os_version_string_for_the_cache_key_is_not_empty_and_stable`。
- 確かめ方: `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::precheck_tests`

## device の要る試験（3/4）

device の上でキーを確かめる試験で、すべて `#[ignore]` 付き（通常の `cargo test` では流れない）。パイプラインは作らない。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `imp::device_tests::pipeline_cache_key_of_safe_msl_with_options_is_stable` | `MslWithOptions`（`Safe`）・`bounds_checked = true` を 2 回 | どちらも `Some` で等しい。`compiler_version` は device の OS の版の文字列と device 名から決まる値、`driver_version` は同じ値の `Some`。パイプラインの表は空 |
| `imp::device_tests::pipeline_cache_key_changes_only_the_changed_field` | 基準に対し、数学モード（`Safe`→`Fast`）・ソース・`bounds_checked` を 1 つずつ変える | 変えた欄だけが変わる |
| `imp::device_tests::pipeline_cache_key_of_each_msl_input_matches_the_pure_key` | MSL の 4 入力 × `bounds_checked` の 2 値。`Wgsl` | 4 入力は `metal_cache_key` の値と等しい。`Wgsl` は `None`。パイプラインの表は空 |
| `tests/backend_metal_device.rs` の `pipeline_cache_key_reaches_through_the_public_device` | 公開の `Device` の `compute()` から `MslWithOptions`（`Safe`）を 2 回 | `Some` で等しい。`compiler_id` は `"metal"`、`target` は `"metal-msl"` |

device の上で流すときのコマンド:

```text
cargo test -p fandhe-silicon-contract --features backend-metal --lib -- --ignored pipeline_cache_key_
cargo test -p fandhe-silicon-contract --features backend-metal,native-blocking-wait --test backend_metal_device -- --ignored pipeline_cache_key_reaches_through_the_public_device
```

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-501。ポインタのみ）。

時点のコミット: `6a3bf94`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 口の既定 | `cargo test -p fandhe-silicon-core --test compute_ops_shape`、`cargo test -p fandhe-silicon-core --doc execution::compute`、`grep -n 'fn pipeline_cache_key' crates/core/src/execution/compute.rs crates/contract/src/backend_metal.rs` | 6 passed（`default_pipeline_cache_key_is_none` は `ok`）、doctest は 7 passed。`fn pipeline_cache_key` は `compute.rs:256` と `backend_metal.rs:2562` |
| 2 | 組み立ての純粋な試験 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::precheck_tests` | 90 passed。`cache_key_is_the_same_for_the_same_input`・`changing_one_input_changes_only_its_field`・`cache_key_fields_for_each_msl_input`・`specialized_with_options_keys_do_not_mix_with_other_inputs`・`non_msl_inputs_have_no_cache_key`・`cache_key_version_joins_os_version_and_device_name_with_nul` の 6 本が `ok` |
| 3 | 材料の字句 | `grep -n 'fast-math(default)\|math_mode=\|float_functions=\|entry=\|;idx=' crates/contract/src/backend_metal.rs` | 本体は `backend_metal.rs:223`〜`:290`。ほかは試験の行 |
| 4 | macOS の試験 | `gh run view 42453808324 --log --job 128648430834 \| grep os_version_string_for_the_cache_key` | `imp::tests::os_version_string_for_the_cache_key_is_not_empty_and_stable ... ok`。一致した行は 7 |
| 5 | device の要る試験の宣言 | `grep -n -A1 '#\[ignore = "touches the Metal device' crates/contract/src/backend_metal.rs \| grep -E 'fn pipeline_cache_key_'`、`crates/contract/tests/backend_metal_device.rs` に同じ形のコマンド | `backend_metal.rs` に 3 本、`backend_metal_device.rs` に 1 本 |
| 6 | unsafe・依存 | `cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count`、`git diff --stat 00f3e89 HEAD -- Cargo.toml Cargo.lock deps-approved.toml crates/contract/tests/backend_metal_unsafe_count.rs`、`git show --stat --format= <sha> -- Cargo.toml Cargo.lock deps-approved.toml 'crates/*/Cargo.toml' crates/contract/tests/backend_metal_unsafe_count.rs`、`make deps-check` | 1 passed。`git diff --stat` は空。`a60b3d8`・`6a9d1df` は空、`0370696` は `crates/contract/Cargo.toml` の 1 行（`objc2-foundation` の features に `"NSProcessInfo"`、版 `=0.3.2` は同じ）。`make deps-check` は終了コード 0 |
| 7 | CI | `gh run view 42453808324 --json conclusion,headSha,jobs`（main の push の run） | conclusion `success`、headSha `6a9d1df74ea7f62bdc315359f8c272930679b80a`。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success` |

(1) の出力。

```text
running 6 tests
test default_pipeline_cache_key_is_none ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 66 filtered out; finished in 0.00s
crates/core/src/execution/compute.rs:256:    fn pipeline_cache_key(
crates/contract/src/backend_metal.rs:2562:        fn pipeline_cache_key(
```

(2) の出力。

```text
test backend_metal::precheck_tests::cache_key_fields_for_each_msl_input ... ok
test backend_metal::precheck_tests::cache_key_is_the_same_for_the_same_input ... ok
test backend_metal::precheck_tests::cache_key_version_joins_os_version_and_device_name_with_nul ... ok
test backend_metal::precheck_tests::non_msl_inputs_have_no_cache_key ... ok
test backend_metal::precheck_tests::changing_one_input_changes_only_its_field ... ok
test backend_metal::precheck_tests::specialized_with_options_keys_do_not_mix_with_other_inputs ... ok
running 90 tests
test result: ok. 90 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.01s
```

(3) の出力（本体の行）。

```text
223:/// `entry=<入口名>` に、function constant ごとに `;idx=<番号>:U32(<値>)` か `;idx=<番号>:Bool(<値>)` を
227:    let mut tag = format!("entry={entry_point}");
234:        tag.push_str(&format!(";idx={}:{value}", c.index));
244:/// `options_hash` は、`Msl`・`MslSpecialized` が `fast-math(default)`、`MslWithOptions` が `math_mode=<語>`、
245:/// `MslSpecializedWithOptions` が `math_mode=<語>;float_functions=<語>` の `sha256_u64`。
260:            "fast-math(default)".to_string(),
269:            "fast-math(default)".to_string(),
278:            format!("math_mode={}", math_mode_word(*math_mode)?),
290:                "math_mode={};float_functions={}",
```

(4) の出力（`make verify` のステップの行）。

```text
test backend_metal::imp::tests::os_version_string_for_the_cache_key_is_not_empty_and_stable ... ok
```

(5) の出力。

```text
3782:        fn pipeline_cache_key_of_safe_msl_with_options_is_stable() {
3800:        fn pipeline_cache_key_changes_only_the_changed_field() {
3849:        fn pipeline_cache_key_of_each_msl_input_matches_the_pure_key() {
---
1320:fn pipeline_cache_key_reaches_through_the_public_device() {
```

(6) の出力。

```text
test backend_metal_unsafe_count_matches_expected ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`0370696` の `crates/contract/Cargo.toml` の差分の行。

```text
-objc2-foundation = { version = "=0.3.2", optional = true, default-features = false, features = ["std", "NSString"] }
+objc2-foundation = { version = "=0.3.2", optional = true, default-features = false, features = ["std", "NSProcessInfo", "NSString"] }
```

(7) の出力。

```text
{"conclusion":"success","headSha":"6a9d1df74ea7f62bdc315359f8c272930679b80a","jobs":[{"conclusion":"success","name":"make deny (cargo-deny)"},{"conclusion":"success","name":"make verify + build-profiles (macos-latest)"},{"conclusion":"success","name":"make verify (fmt + clippy + test)"}]}
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | `ComputeOps` に `pipeline_cache_key` の口を足す | #4957 |
| 2/4 | Metal の `pipeline_cache_key` を実装する | #4958 |
| 3/4 | キャッシュキーの device の試験を ignore 付きで足す | #4959 |
| 4/4 | 完了の条件の確かめ | #4960 |
