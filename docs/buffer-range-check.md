# バッファの引数検査

TASK-309 の 1/4（MEM-70・MEM-74〜MEM-76 のポインタ）。private spec の本文は写さない。

## 所在

`crates/core/src/memory/buffer_check.rs`。公開のパスは `fandhe_silicon_core::memory::` の下で、feature の条件はない。

| 名前 | 役割 |
| ---- | ---- |
| `validate_buffer_range` | 部分範囲 `[offset, offset + len)` が確保長に収まるかを判定する。`upload` の書き込み先にも `download` の読み出し元にも使える |
| `validate_allocation_size` | 確保のバイト数が `max_buffer_bytes` を超えないかを判定する |

## 現状の振る舞い

- どちらも `Result<(), ContractError>` を返し、違反は `InvalidArgument`（文言は英語）
- `validate_buffer_range`: 終端を `checked_add` で求める。`u64` であふれたら拒否、終端が確保長を超えたら拒否（判定の順はあふれ → 超過）。終端が確保長ちょうどなら通る。`len` が 0 でも `offset` が確保長を超えれば拒否する
- `validate_allocation_size`: 上限だけを見る。上限ちょうどは通す
- 算術は `checked_add` と比較だけ。背後実装の API は呼ばず、確保もしない（エラーの文字列を除く）。`usize` は受けない

## 対応するテスト

`crates/core/src/memory/buffer_check.rs` の単体テスト（`buffer_check_` で始まる名前）。範囲内・終端ちょうど・1 バイト超過・終端のあふれ・上限ちょうどと 1 超過を確かめる。

## 確かめ方

```bash
cargo test -p fandhe-silicon-core buffer_check_
make verify
```
