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

device に触る試験は `crates/contract/tests/backend_cuda_capability.rs`（feature `backend-cuda`・`test-support`、`#[ignore]` 付き。下の 3/4 の節）。

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

## device の要る試験（3/4）

`crates/contract/tests/backend_cuda_capability.rs` に置く。`required-features` は `backend-cuda` と `test-support`。
どの試験も `#[ignore = "requires a CUDA device"]` 付きで、既定では流れない。

| 試験 | 確かめること |
| ---- | ------------ |
| `rows_follow_the_compute_capability_and_the_attribute` | 閾値の 5 鍵が、`adapter_name` から読んだ compute capability が 7.5 以上なら `S2`、未満なら `Unsupported`。根拠はどれも `RuntimeApi`。`FEATURE_MAPPABLE_PRIMARY_BUFFERS` は「`S2`・理由なし」か「`Unsupported`・`NotSupported`」のどちらかで、根拠は `RuntimeApi`。ホストメモリ属性の取得に失敗した device では、この鍵だけ `S0`（理由・根拠なし）も許容する |
| `every_key_passes_the_common_rules_without_s0` | 鍵の一覧のすべての鍵で `check_query_ex` が通り、`query` と `query_ex` の level が等しく、level が `S0` でない。鍵ごとに 1 行を標準出力に出す |

確かめ方（device を使わない）: `cargo test -p fandhe-silicon-contract --features backend-cuda,test-support --test backend_cuda_capability` が `0 passed; 0 failed; 2 ignored` で終わること。

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-175。ポインタのみ）。

時点のコミット: `49185a2`
`docs/spec` の gitlink: `f50c86bf8919b4ee68e62948491886fb25c9ae12`（`git ls-tree origin/main docs/spec`）

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | `query`・`query_ex` が捕捉した値と表から答える | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda`（`threshold_rows_follow_the_compute_capability`・`a_rule_below_its_threshold_is_not_supported`・`mappable_follows_the_attribute_and_is_s0_when_unread` を含む） | 44 passed; 0 failed; 11 ignored、終了コード 0 |
| 2 | 属性の読み取りの失敗で `S0`、真で `S2`・偽で `Unsupported` | 同じコマンドの `backend_cuda::tests::mappable_follows_the_attribute_and_is_s0_when_unread` | `ok` |
| 3 | `S3` 以上を返さない | 同じコマンドの `backend_cuda::tests::no_key_returns_s3_or_above`、`grep -c 'CapabilityLevel::S[3-6]' crates/contract/src/backend_cuda.rs` | `ok`、grep の件数 0 |
| 4 | 共通の規則の検査を表が通る | `cargo test -p fandhe-silicon-contract --features backend-cuda,test-support --lib backend_cuda`（`table_passes_the_common_checks` を含む） | 45 passed; 0 failed; 11 ignored、終了コード 0 |
| 5 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-cuda,test-support --test backend_cuda_capability` | 0 passed; 0 failed; 2 ignored、終了コード 0 |
| 6 | unsafe・依存 | `cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count`、`git diff 8e32db0 origin/main --stat -- Cargo.toml crates/contract/Cargo.toml Cargo.lock deps-approved.toml`、`make deps-check`、`grep -c '^\[\[approval\]\]' deps-approved.toml` | 1 passed、差分は `crates/contract/Cargo.toml` の 4 行（`[[test]]` の 1 ブロック）だけ、`deps-check` は終了コード 0、件数 5 |
| 7 | 公開 API | `make public-api-check`、`make public-api-sdk-check`、`make public-name-check` | 3 つとも終了コード 0。`wgpu_type_refs_total=0`、`sdk_type_refs_total=0`、`name_hits_total=0` |
| 8 | CI で流れる場所 | `grep -n 'all-features' scripts/verify.sh`、`grep -n 'backend-cuda' scripts/build-profiles.sh`、`gh run view 42495199336 --json conclusion,headSha,jobs`（main の push の run） | `verify.sh` が `cargo test --workspace --all-features` を流す。`build-profiles.sh` に `backend-cuda`・`backend-cuda,test-support` の組がある。conclusion success、headSha `49185a2513f3ac9e2299c3ad9eaa3463b84ba558`、`make verify (fmt + clippy + test)` success、`make verify + build-profiles (macos-latest)` success、`make deny (cargo-deny)` success |

(1)・(2)・(3) の出力。

```text
running 55 tests
test backend_cuda::tests::a_rule_below_its_threshold_is_not_supported ... ok
test backend_cuda::tests::no_key_returns_s3_or_above ... ok
test backend_cuda::tests::mappable_follows_the_attribute_and_is_s0_when_unread ... ok
test backend_cuda::tests::threshold_rows_follow_the_compute_capability ... ok
test result: ok. 44 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

(4) の出力。

```text
running 56 tests
test backend_cuda::tests::no_key_returns_s3_or_above ... ok
test backend_cuda::tests::table_passes_the_common_checks ... ok
test result: ok. 45 passed; 0 failed; 11 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

(5) の出力。

```text
running 2 tests
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(6) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
5
```

(7) の出力。

```text
record: commit=49185a2513f3ac9e2299c3ad9eaa3463b84ba558 documents=10 public_items_total=16071 wgpu_type_refs_total=0 wgpu_other_refs_total=0
record: commit=49185a2513f3ac9e2299c3ad9eaa3463b84ba558 documents=10 public_items_total=13838 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
record: commit=49185a2513f3ac9e2299c3ad9eaa3463b84ba558 documents=10 public_items_total=13838 name_hits_total=0 sources=1 source_hits_total=0 field_specs=1 field_hits_total=0
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 能力の判定の材料を捕捉して能力の表から問合せに答える | #3762 |
| 2/4 | 能力の表を共通の規則の検査に通す試験を足す | #3763 |
| 3/4 | device の要る能力の問合せの試験を ignore 付きで足す | #3764 |
| 4/4 | 完了の条件の確かめ | #3765 |
