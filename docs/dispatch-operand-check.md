# 引数の宣言の C8 の検査

TASK-421 の 1/4〜4/4（BIND-35〜BIND-37、ART-67・ART-68・ART-85 のポインタ）。private spec の本文は写さない。

## 所在

`crates/core/src/execution/dispatch_check.rs`。公開のパスは `fandhe_silicon_core::execution::` の下で、feature の条件はない。

| 名前 | 役割 |
| ---- | ---- |
| `validate_dispatch_operands` | 宣言を先頭から見て C8 を判定する。最初の違反で `Err`、全部通れば `Ok(())` |
| `required_bytes` | 宣言 1 件の必要バイト数。`u64` であふれたら `None` |
| `OperandCheckError` | 暫定のエラー型（`#[non_exhaustive]`、`InvalidArgument(String)`）。契約のエラー型そのものではない |
| `OperandBinding`・`OperandConstant` | 検査の入力のための形。背後実装へ渡す束縛の型ではない |

## 現状の振る舞い

- C8 を判定する。束縛・定数・容量を返す関数の引数は受けるが読まない
- 判定の順は、`shape` に 0 の次元がある、必要バイト数が `u64` をあふれる、必要バイト数が `max_buffer_bytes` より大きい。上限ちょうどは通す
- 違反は `InvalidArgument` で、文言の先頭は `C8:`（本文は英語）
- `shape` が空なら次元の積は 1 で、1 要素分のバイト数になる
- 必要バイト数は `checked_mul` で計算する。panic・添字アクセス・バッファの確保はない

## 対応するテスト

- `crates/core/src/execution/dispatch_check.rs` の単体テスト（`dispatch_check_` で始まる名前）
- `crates/core/tests/execution_dispatch_check.rs`: 外の crate から公開のパスで呼ぶ統合テスト（`c8_` で始まる名前）。長さ 0 の次元（GEMM の宣言を含む）・`u64` のあふれ・上限超過（1 バイト足りなければ拒否、ちょうどなら通る。巨大な宣言でも確保せずに判定する）を確かめる

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --lib dispatch_check
cargo test -p fandhe-silicon-core --test execution_dispatch_check
make verify
```

## 完了の条件の確かめ（4/4）

4/4（#4744）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-421。関連: BIND-35〜BIND-37）。GPU の実機なしで、末尾の base で実行した値。件数は実測。完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 単体テスト | `cargo test -p fandhe-silicon-core --lib dispatch_check` | 6 passed / 0 failed / 0 ignored |
| A2 | 統合テスト | `cargo test -p fandhe-silicon-core --test execution_dispatch_check` | 5 passed / 0 failed / 0 ignored |
| A3 | 統合テストの関数の数 | `grep -c -E '^fn c8_' crates/core/tests/execution_dispatch_check.rs` | 5 |
| A4 | 確保・panic がない | 本体（`#[cfg(test)]` より上）への grep は `checked_mul` だけ。`crates/core/src/lib.rs` は `#![forbid(unsafe_code)]` | 一致 |
| A5 | feature の条件がない | feature なしの `cargo test -p fandhe-silicon-core`、`make public-api-check` | 全 passed、終了コード 0 |
| A6 | base の CI | `gh run list`（base の push の run、ID 39149201126） | 記録時点で未完了（queued）。結果は未確認 |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |

ビヘイビア ID と試験の対応（関数名）:

| ID | 試験 |
| -- | ---- |
| BIND-35 | `c8_zero_length_shape_is_rejected`・`c8_gemm_zero_dimension_is_rejected` |
| BIND-36 | `c8_u64_overflow_in_byte_calculation_is_rejected` |
| BIND-37 | `c8_exceeds_max_buffer_bytes_is_rejected_without_allocating`・`c8_within_max_buffer_bytes_is_accepted` |

試験が照合するのは `InvalidArgument` の variant と、文言の先頭 `C8` まで。

範囲の外: 先頭以外の次元が 0 の入力、C4〜C7、背後実装の入口を通した確かめは本書の範囲の外（ポインタ: TASK-477・TASK-422・TASK-423・TASK-494・TASK-495）。

### 再実行の契機

- `dispatch_check.rs`・`execution_dispatch_check.rs`・`execution.rs` の `OperandSpec`／`DType::byte_size` を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `7c962ec88bccc285f63d81e46a1f4381f8490082`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。
