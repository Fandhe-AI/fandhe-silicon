# CUDA の背後実装の `CpuOnly`・`Graphics` の要求と鍵の一覧の外の機能 ID の返し（TASK-169）の所在と本リポの現状

TASK-169 の作業のうち、CUDA の背後実装の `CpuOnly`・`Graphics` の要求の拒否と、鍵の一覧の外の機能 ID の返しの試験の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-169。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の `check_request` と `impl Capabilities for CudaDevice`（`cuda_capability_of`） | `crates/contract/src/backend_cuda.rs` |
| 試験 | `crates/contract/tests/backend_cuda_profiles.rs`（device に触らない）と、`backend_cuda.rs` の `mod tests` の `ids_outside_the_key_list_are_unlisted` | 下の「確かめ方」 |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 関連 | device の取得は [cuda-device-request.md](./cuda-device-request.md)（TASK-160）、能力の問合せは [cuda-capability-query.md](./cuda-capability-query.md)（TASK-175） | 各文書 |

## プロファイルと所有形態の組の拒否と一覧の外の機能 ID（1/3）

`request` は、`CpuOnly`・`Graphics` を所有形態によらず `InvalidCombination` で返す。

| プロファイル | 所有形態 | 返す値 | 試験 |
| ------------ | -------- | ------ | ---- |
| `CpuOnly` | `ProductIndependent`・`Shared`・`Imported` | `InvalidCombination` | `cpu_only_is_an_invalid_combination_for_every_ownership` |
| `Graphics` | `ProductIndependent`・`Shared`・`Imported` | `InvalidCombination` | `graphics_is_an_invalid_combination_for_every_ownership` |

- `--features backend-cuda` と `--features backend-cuda,graphics` のどちらのビルドでも同じ値

鍵の一覧（`FEATURE_QUERY_KEYS`）に無い 5 つの ID（`feature:shader_f16`・`feature:SHADER_F16 `〔末尾に空白〕・`feature:SUBGRUOP`・`feature:not-a-feature`・空の文字列）を `cuda_capability_of` に当てると、compute capability `(5,3)`・`(7,4)`・`(7,5)`・`(9,0)` と属性 `None`・`Some(false)`・`Some(true)` のどの組でも core の `UNLISTED_FEATURE_QUERY`（`Unsupported`・`Some(UnknownFeature)`・`None`）と等しい値を返す。

### 確かめ方

```bash
cargo test -p fandhe-silicon-contract --features backend-cuda --test backend_cuda_profiles
cargo test -p fandhe-silicon-contract --features backend-cuda,graphics --test backend_cuda_profiles
cargo test -p fandhe-silicon-contract --features backend-cuda --lib ids_outside_the_key_list_are_unlisted
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/3 | プロファイルと所有形態の組の拒否と一覧の外の機能 ID の試験を足す | #3742 |
| 2/3 | device の要る試験を ignore 付きで足す | #3743 |
| 3/3 | 完了の条件の確かめ | #3744 |
