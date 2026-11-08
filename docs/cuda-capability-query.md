# CUDA の背後実装の能力の問合せ（TASK-175）の所在と本リポの現状

TASK-175 の作業のうち、CUDA の背後実装の `Capabilities::query`・`query_ex` が答える能力の表と問合せの所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-175。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 部品 | `CapabilitySnapshot`・`CudaRule`・`MIN_COMPUTE_CAPABILITY`・`CUDA_FEATURE_TABLE`・`cuda_capability_of` | `grep -n 'CapabilitySnapshot\|CUDA_FEATURE_TABLE\|cuda_capability_of' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |

## 能力の表と問合せ（1/4）

device を作る時点（`request`）で、compute capability とホストメモリのマップの可否の属性（`CU_DEVICE_ATTRIBUTE_CAN_MAP_HOST_MEMORY`）を 1 回だけ捕捉する。
`query`・`query_ex` は捕捉した値と能力の表だけから答え、下位の API を呼ばない。属性の読み取りの失敗は `request` を失敗させない。

表は次の 6 行:

| 鍵 | 規則 | `source` |
| -- | ---- | -------- |
| `FEATURE_EXPERIMENTAL_COOPERATIVE_MATRIX` | compute capability 7.0 以上 | CUDA C++ Programming Guide: Warp Matrix Functions |
| `FEATURE_SHADER_F16` | 5.3 以上 | CUDA C++ Programming Guide: Feature Support per Compute Capability |
| `FEATURE_SUBGROUP` | 5.0 以上 | CUDA C++ Programming Guide: Warp Shuffle Functions |
| `FEATURE_SUBGROUP_BARRIER` | 7.0 以上 | CUDA C++ Programming Guide: Independent Thread Scheduling |
| `FEATURE_SHADER_FLOAT32_ATOMIC` | 2.0 以上 | CUDA C++ Programming Guide: atomicAdd() |
| `FEATURE_MAPPABLE_PRIMARY_BUFFERS` | 属性の値 | CUDA Driver API: CU_DEVICE_ATTRIBUTE_CAN_MAP_HOST_MEMORY |

返す値:

| 入力 | level | 理由 | 根拠 |
| ---- | ----- | ---- | ---- |
| compute capability が `MIN_COMPUTE_CAPABILITY`（7.5）より低い（どの行も。属性の読み取りの結果によらない） | `Unsupported` | `NotSupported` | `RuntimeApi` |
| 閾値の行で、下限以上 | `S2` | なし | `RuntimeApi` |
| 閾値の行で、下限未満 | `Unsupported` | `NotSupported` | `RuntimeApi` |
| 属性の値が 1 | `S2` | なし | `RuntimeApi` |
| 属性の値が 0 | `Unsupported` | `NotSupported` | `RuntimeApi` |
| 属性の値が 1・0 以外、または読み取りの失敗 | `S0` | なし | なし |
| 表に行の無い鍵 | `Unsupported` | `UnknownFeature` | なし（core の `UNLISTED_FEATURE_QUERY`） |

`S3` 以上を返す分岐は無い。

試験（device に触らない）:

| 試験 | 確かめること |
| ---- | ------------ |
| `threshold_rows_follow_the_compute_capability` | 閾値の 5 行が 7.5・9.0 で `S2`、7.4 で `Unsupported` |
| `a_rule_below_its_threshold_is_not_supported` | 8.0 以上の規則が 7.5 で `Unsupported`、8.0 で `S2` |
| `mappable_follows_the_attribute_and_is_s0_when_unread` | 属性の 1・0・読み取りの失敗・ほかの値の写しと、7.4 では属性が不明でも `Unsupported` |

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda`

## 能力の表の共通の規則の検査（2/4）

device に触らない試験で、同じファイルの `tests` module にある。

| 試験 | 入力 | 確かめること |
| ---- | ---- | ------------ |
| `no_key_returns_s3_or_above` | 鍵の一覧のすべての鍵 × compute capability 1.0・5.3・7.4・7.5・9.0・99.9 × 属性（不明・偽・真） | level が `S0`・`S2`・`Unsupported` のどれか |
| `keys_without_a_row_are_unlisted` | 表に行の無い鍵 | `UNLISTED_FEATURE_QUERY` と等しい |
| `table_passes_the_common_checks` | 表と鍵の一覧。5.3・7.4・7.5・9.0 × 属性 3 つ | 表の鍵が一覧にあり重ならない（`check_table_keys`）。一覧のすべての鍵の値が共通の規則を通る（`check_table_queries`） |

`table_passes_the_common_checks` は feature `test-support` を付けたときだけコンパイル・実行される。付けないときは 2 本が走る。

確かめ方: `cargo test -p fandhe-silicon-contract --features backend-cuda,test-support --lib backend_cuda`

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 能力の判定の材料を捕捉して能力の表から問合せに答える | #3762 |
| 2/4 | 能力の表を共通の規則の検査に通す試験を足す | #3763 |
| 3/4 | device の要る能力の問合せの試験を ignore 付きで足す | #3764 |
| 4/4 | 完了の条件の確かめ | #3765 |
