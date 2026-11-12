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

## 直接の呼び出しの試験（2/6）

`validate_operands` を外の crate から公開のパスだけで直接呼ぶ統合テストを足した。feature なしで動く（TASK-426）。

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/core/tests/execution_operand_check.rs` |

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `validate_operands_returns_c1_without_dispatch_checks` | 宣言 1 件（`shape` `[0]`）・`slots` 2 件・容量 `Some(0)`・上限 0 | 先頭 `C1`。容量の閉包の呼び出しは 0 回 |
| `validate_operands_returns_c3_without_dispatch_checks` | 宣言 1 件（`shape` 2 次元・`stride` 1 次元）・容量 `Some(0)`・上限 0 | 先頭 `C3`。容量の閉包の呼び出しは 0 回 |
| `validate_operands_runs_dispatch_checks_after_declared_checks` | C1・C3 を通る入力で C8・C7・C4・C5・C6 の拒否を 1 つずつと全部通る入力 | `validate_dispatch_operands` を単独で呼んだ結果（先頭 `C8`・`C7`・`C4`・`C5`・`C6`、または `Ok(())`）と等しい |
| `validate_operands_accepts_input_that_passes_every_check` | F32 `[4]`・`alignment` 16・offset 16・容量 `Some(32)`・上限 4,000 | `Ok(())`。容量の閉包は 1 回以上呼ばれる |
| `validate_operands_return_type_is_pinned` | 空の入力 | 署名が固定され `Ok(())` |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --test execution_operand_check
make verify
```
