# 引数の宣言の C1・C3 の検査（TASK-420）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-420。

## 所在と現状の振る舞い（1/4）

| 項目 | 内容 |
| ---- | ---- |
| 実装 | `crates/core/src/execution/declared_check.rs` |
| 公開のパス | `fandhe_silicon_core::execution::validate_declared_operands` |
| feature の条件 | なし |

| 名前 | 役割 |
| ---- | ---- |
| `validate_declared_operands` | 宣言の件数と次元数を照らす純粋関数。`(declared, layout)` を受け、`Result<(), ContractError>` を返す |
| `ContractError::InvalidArgument` | 違反の返し方。文言は英語で、先頭は `C1:`・`C3:` のいずれか |

現状の振る舞い。

- C1 を先に判定する。`declared` の件数が `layout.slots` の件数と違えば拒否する
- C1 を通ったら C3 を判定する。`declared` を先頭から見て、`shape` と `stride` の次元数が違う最初の宣言で拒否する。文言にその宣言の `slot` と 2 つの次元数が入る
- `stride` が空でも比べる。`shape` が空でなければ C3 で拒否する
- `slot` の値、`slot` の重複、`dtype`、`alignment`、`shape`・`stride` の値は見ない
- 背後実装・ファイル・時刻には触れない。確保が起きるのは拒否の文言だけで、panic しない

単体テストの名前は `declared_check_` で始まる。

```text
cargo test -p fandhe-silicon-core --lib declared_check
make verify
```

引数の宣言の C4〜C8 は [dispatch-operand-check.md](dispatch-operand-check.md)。
