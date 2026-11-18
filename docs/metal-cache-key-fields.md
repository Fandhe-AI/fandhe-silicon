# Metal のキャッシュキーの function constant の値と版の欄の試験（TASK-502）の所在と本リポの現状

TASK-502 の作業のうち、Metal の `pipeline_cache_key` の function constant の値と版の欄の試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-502。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| device に触らない試験の module | `crates/contract/src/backend_metal.rs` の末尾の `cache_key_specialization_tests` | 同左 |
| device の要る試験のファイル | `crates/contract/tests/backend_metal_cache_key_fields.rs` | 同左 |
| `[[test]]` の feature | `backend-metal` | `crates/contract/Cargo.toml` |
| cfg | 試験のファイルは `target_os = "macos"`。macOS 以外では 0 件 | 下の「確かめ方」 |
| キーを組み立てる関数 | `docs/pipeline-cache-key.md` の「Metal のキャッシュキー（2/4）」の節 | 同左 |

## キャッシュキーの function constant の値と版の欄の試験（1/2）

### device に触らない試験

module `cache_key_specialization_tests` の 1 本 `changing_only_a_function_constant_value_changes_only_the_specialization_hash` で、どの OS でも流れる。
入力は `MslSpecialized` と `MslSpecializedWithOptions`（`Safe`・`Precise`）で、index 0 の定数の値だけを `U32(4)` から `U32(5)`、`Bool(true)` から `Bool(false)` に変えた 2 つのキーを `metal_cache_key` で組み立てる。
照らす値は、2 つのキーの `specialization_hash` が違うことと、`specialization_hash` を揃えるとキー全体が等しいこと。

### device の要る試験

2 本とも `#[ignore]` が付く。入口は `MetalDeviceProvider::request`（`Metal`・`ComputeOnly`・`ProductIndependent`）で作った device の `Device::compute()` の `pipeline_cache_key`（パイプラインは作らず、投入もしない）。

| 試験 | 入力 | 照らす値 |
| ---- | ---- | -------- |
| `device_key_changes_only_the_specialization_hash_when_a_constant_value_changes` | `[[function_constant(0)]]` の `uint` を持つ MSL の `MslSpecialized` と `MslSpecializedWithOptions`（`Safe`・`Precise`）。定数の値だけ `U32(4)` から `U32(5)` | 2 つのキーの `specialization_hash` が違う。`specialization_hash` を揃えるとキー全体が等しい |
| `version_fields_are_stable_and_come_from_the_os_version_and_the_device_name` | MSL の 4 つの入力（`Msl`・`MslSpecialized`・`MslWithOptions`・`MslSpecializedWithOptions`） | 入力ごとに 2 回呼んだキーが等しい。`compiler_version` が期待の値、`driver_version` が `Some(期待の値)` |

期待の値は試験の中で作る。`NSProcessInfo` の `operatingSystemVersionString` と `MTLCreateSystemDefaultDevice` の `name()` を NUL でつないだ文字列の `sha256_u64`。

## 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::cache_key_specialization_tests
cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_cache_key_fields
cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal -- -D warnings
```

1 本目はどの OS でも 1 件。2 本目は macOS 以外では `running 0 tests`、macOS では `ignored` の件数が 2 になる。

## 完了の条件の確かめ（2/2）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-502。ポインタのみ）。

時点のコミット: `ddd019b3`。`docs/spec` の gitlink は `becfefdc2fb47fa610c6ecc914f3bf601910f5f3`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 試験の宣言 | `crates/contract/tests/backend_metal_cache_key_fields.rs` の `grep -c '#\[test\]'`、`grep -c '^[[:space:]]*#\[ignore'`。`cache_key_specialization_tests` の `#[test]`・`#[ignore` の数は `awk` で module を切り出して `grep -c` | ファイルの `#[test]` は 2、`#[ignore` は 2。module の `#[test]` は 1、`#[ignore` は 0 |
| 2 | 受け手の試験の名前 | 下の (2) の `git grep` | 下の (2) の出力（6 行） |
| 3 | device に触らない試験 | `cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::cache_key_specialization_tests`、`cargo test -p fandhe-silicon-contract --features backend-metal --lib backend_metal::precheck_tests::cache_key` | `test result: ok. 1 passed; 0 failed`、`test result: ok. 3 passed; 0 failed` |
| 4 | macOS の型検査 | `cargo clippy -p fandhe-silicon-contract --target aarch64-apple-darwin --all-targets --features backend-metal -- -D warnings`、同じコマンドの `--features backend-metal,native-blocking-wait`、`--all-features` | 3 組とも終了コード 0 |
| 5 | CI の macOS のビルド | `gh run view 44348451506 --log --job 134957180052`（`make verify + build-profiles (macos-latest)`）の `backend_metal_cache_key_fields` の部分。run は `ddd019b3` の main の push | `Running tests/backend_metal_cache_key_fields.rs` の行は 8 か所。`make verify` の部分は `test result: ok. 0 passed; 0 failed; 2 ignored`。run の conclusion `success`、headSha `ddd019b332588d90df0130bfbea65e639947ea3c` |
| 6 | 依存 0・製品のコードの差 0 | `git diff --stat ddd019b3^..HEAD -- Cargo.lock deps-approved.toml crates/core`、`git diff --numstat ddd019b3^..HEAD -- crates/contract/src/backend_metal.rs`、`make deps-check`、`cargo test -p fandhe-silicon-contract --test backend_metal_unsafe_count` | 1 つ目は出力なし、2 つ目は追加 68・削除 0、`make deps-check` は終了コード 0、`test result: ok. 1 passed; 0 failed` |
| 7 | CI | `gh run view 44348451506 --json conclusion,headSha,jobs` | `make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)`・`make deny (cargo-deny)` が `success`、headSha `ddd019b332588d90df0130bfbea65e639947ea3c` |

(6) の `ddd019b3^` は、1/2 の 1 つ前のコミット。

(2) の出力（`git grep -n -e "fn device_key_changes_only_the_specialization_hash_when_a_constant_value_changes" -e "fn changing_only_a_function_constant_value_changes_only_the_specialization_hash" -e "fn version_fields_are_stable_and_come_from_the_os_version_and_the_device_name" -e "fn cache_key_version_joins_os_version_and_device_name_with_nul" -e "fn os_version_string_for_the_cache_key_is_not_empty_and_stable" -e "fn pipeline_cache_key_of_safe_msl_with_options_is_stable" -- crates/contract`）。

```text
crates/contract/src/backend_metal.rs:5928:        fn os_version_string_for_the_cache_key_is_not_empty_and_stable() {
crates/contract/src/backend_metal.rs:9303:        fn pipeline_cache_key_of_safe_msl_with_options_is_stable() {
crates/contract/src/backend_metal.rs:12662:    fn cache_key_version_joins_os_version_and_device_name_with_nul() {
crates/contract/src/backend_metal.rs:14368:    fn changing_only_a_function_constant_value_changes_only_the_specialization_hash() {
crates/contract/tests/backend_metal_cache_key_fields.rs:39:fn device_key_changes_only_the_specialization_hash_when_a_constant_value_changes() {
crates/contract/tests/backend_metal_cache_key_fields.rs:100:fn version_fields_are_stable_and_come_from_the_os_version_and_the_device_name() {
```

(5) の出力（`make verify` の部分）。

```text
Running tests/backend_metal_cache_key_fields.rs (target/debug/build/fandhe-silicon-contract/fbd61bd5d7fad305/out/backend_metal_cache_key_fields-fbd61bd5d7fad305)

running 2 tests
test device_key_changes_only_the_specialization_hash_when_a_constant_value_changes ... ignored, touches the Metal device; run only in a dedicated device session
test version_fields_are_stable_and_come_from_the_os_version_and_the_device_name ... ignored, touches the Metal device; run only in a dedicated device session

test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | キャッシュキーの function constant の値と版の欄の試験 | #4963 |
| 2/2 | 完了の条件の確かめ | #4964 |
