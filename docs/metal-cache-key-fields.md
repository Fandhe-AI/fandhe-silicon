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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | キャッシュキーの function constant の値と版の欄の試験 | #4963 |
