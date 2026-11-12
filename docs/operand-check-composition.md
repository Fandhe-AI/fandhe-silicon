# 引数の宣言の C1・C3 と C4〜C8 を続けて判定する純粋関数（TASK-426）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-426。

## 所在と現状の振る舞い（1/6）

### 所在

| 項目 | 値 |
| ---- | -- |
| ソース | `crates/core/src/execution/operand_check.rs` |
| 公開のパス | `fandhe_silicon_core::execution::validate_operands` |
| feature | 条件なし |

### 名前と役割

| 名前 | 役割 |
| ---- | ---- |
| `validate_operands` | `validate_declared_operands`（C1・C3）の後に `validate_dispatch_operands`（C4〜C8）を呼ぶ純粋関数 |

### 現状の振る舞い

- `validate_declared_operands` を先に呼び、`Err` ならそのまま返す。このとき `validate_dispatch_operands` は呼ばず、`binding_capacity` も呼ばない
- 通れば `validate_dispatch_operands` の結果をそのまま返す
- `Err` は値のまま返し、文言の先頭は `C1:`・`C3:`・`C4:`〜`C8:` のいずれか
- 背後実装・ファイル・時刻に触れない。panic しない

### 単体テスト

名前は `operand_check_` で始まる。

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --lib operand_check
make verify
```
