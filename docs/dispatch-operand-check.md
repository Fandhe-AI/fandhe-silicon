# 引数の宣言の C8 の検査

TASK-421 の 1/4〜3/4（BIND-35〜BIND-37、ART-67・ART-68・ART-85 のポインタ）。private spec の本文は写さない。

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

完了の条件の確かめは後続の段で扱う。

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --lib dispatch_check
cargo test -p fandhe-silicon-core --test execution_dispatch_check
make verify
```
