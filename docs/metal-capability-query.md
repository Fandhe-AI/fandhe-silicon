# Metal の背後実装の能力の問合せ（TASK-173）の所在と本リポの現状

TASK-173 の作業のうち、Metal の背後実装の `Capabilities::query`・`query_ex` が答える能力の表と問合せの所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。書式は [cuda-capability-query](cuda-capability-query.md)（TASK-175）に合わせる。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-173。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_metal.rs` の 1 ファイル | `crates/contract/src/backend_metal.rs` |
| 規則の型 | `MetalCapabilitySnapshot`・`NotMet`・`MetalRule` | `grep -n 'struct MetalCapabilitySnapshot\|enum NotMet\|enum MetalRule' crates/contract/src/backend_metal.rs` |
| 表 | `METAL_FEATURE_TABLE`（8 行） | `grep -n 'METAL_FEATURE_TABLE' crates/contract/src/backend_metal.rs` |
| 純粋な関数 | `metal_runtime_decided`・`not_met`・`metal_evaluate`・`metal_capability_of`（macOS 以外でもビルドされる） | `grep -n 'fn metal_runtime_decided\|fn not_met\|fn metal_evaluate\|fn metal_capability_of' crates/contract/src/backend_metal.rs` |
| 捕捉の関数 | `imp` module の `capture_capabilities`（`MetalDevice::new` が 1 回呼ぶ） | `grep -n 'fn capture_capabilities' crates/contract/src/backend_metal.rs` |
| 差し込みの欄 | `imp` module の `Faults::capability_capture_fails`（非公開） | `grep -n 'capability_capture_fails' crates/contract/src/backend_metal.rs` |
| cfg | feature `backend-metal`。捕捉は macOS だけ | `crates/contract/src/lib.rs` |
| 試験 | `backend_metal::capability_tests`。device に触らない | `cargo test -p fandhe-silicon-contract --all-features --lib backend_metal::capability_tests` |

## 能力の表と問合せ（1/3）

device を作るときに、Apple の GPU family（`MTLGPUFamily::Apple9` から `Apple1` の順に `supportsFamily` を問い、最初に真になった世代）を 1 回だけ捕捉する。
`query`・`query_ex` は捕捉した値と能力の表だけから答え、下位の API を呼ばない。

表は次の 8 行:

| 鍵 | `min` | `below` | `without_apple` |
| -- | ----- | ------- | --------------- |
| `FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX` | 7 | `NotSupported` | `Unlisted` |
| `FEATURE_SHADER_F16` | 1 | `Unlisted` | `Unlisted` |
| `FEATURE_SUBGROUP` | 7 | `Unlisted` | `Unlisted` |
| `FEATURE_SUBGROUP_BARRIER` | 4 | `Unlisted` | `Unlisted` |
| `FEATURE_SHADER_FLOAT32_ATOMIC` | 7 | `Unlisted` | `Unlisted` |
| `FEATURE_MAPPABLE_PRIMARY_BUFFERS` | 1 | `NotSupported` | `NotSupported` |
| `FEATURE_TEXTURE_COMPRESSION_ASTC_HDR` | 6 | `NotSupported` | `Unlisted` |
| `FEATURE_VULKAN_EXTERNAL_MEMORY_FD` | `NoApi`（捕捉した値を読まない。根拠 `RuntimeApi`） | | |

`NotSupported` は `Unsupported`・`NotSupported`・`RuntimeApi` の値、`Unlisted` は core の `UNLISTED_FEATURE_QUERY` と同じ値を返す。

返す値:

| 入力 | level | 理由 | 根拠 |
| ---- | ----- | ---- | ---- |
| family の行で、世代が `min` 以上 | `S2` | なし | `RuntimeApi` |
| family の行で、Apple の family を持ち世代が `min` 未満 | `below` の値 | `below` の値 | `below` の値 |
| family の行で、Apple の family を持たない | `without_apple` の値 | `without_apple` の値 | `without_apple` の値 |
| family の行で、捕捉の失敗 | `S0` | なし | なし |
| `NoApi` の行（捕捉の失敗を含むどの入力でも） | `Unsupported` | `NoApi` | `RuntimeApi` |
| 表に行の無い鍵（捕捉の失敗を含むどの入力でも） | `Unsupported` | `UnknownFeature` | なし（`UNLISTED_FEATURE_QUERY`） |

`S3` 以上を返す分岐は無い。

試験（device に触らない）:

| 試験 | 確かめること |
| ---- | ------------ |
| `family_rows_follow_the_family_and_are_s0_when_not_captured` | family の各行が、世代 9 と `min` で `S2`、`min` 未満で `below`、Apple の family なしで `without_apple`、捕捉の失敗で `S0` |
| `no_api_row_returns_no_api` | `NoApi` の行が、5 つの捕捉の値のどれでも `Unsupported`・`NoApi`・`RuntimeApi` |
| `no_key_returns_s3_or_above` | 全鍵 × 5 つの捕捉の値で、段階が `S0`・`S2`・`Unsupported` のどれか |
| `keys_without_a_row_are_unlisted` | 行の無い全鍵が、5 つの捕捉の値のどれでも `UNLISTED_FEATURE_QUERY` |
| `three_keys_have_no_row` | `FEATURE_EXPERIMENTAL_MESH_SHADER`・`FEATURE_TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`・`FEATURE_MULTI_DRAW_INDIRECT_COUNT` に行が無く、世代 9 で `UNLISTED_FEATURE_QUERY` |
| `table_passes_the_common_checks`（feature `test-support`） | 全鍵 × 4 つの捕捉の値（失敗・family なし・世代 1・世代 9）が core の `check_query` を通る |

確かめ方: `cargo test -p fandhe-silicon-contract --all-features --lib backend_metal::capability_tests`

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 377 filtered out
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | 能力の判定の材料を捕捉して能力の表から問合せに答える | #3746 |
