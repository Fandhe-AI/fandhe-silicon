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

## 検査が通る 6 通りの入力の試験（3/6）

検査をすべて通る 6 通りの入力で、`validate_declared_operands`・`validate_dispatch_operands`・`validate_operands` がどれも `Ok(())` を返すことを、外の crate から公開のパスで確かめる。feature なしで動く（TASK-426）。

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/core/tests/execution_operand_check.rs` |

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `sequential_checks_accept_matching_counts` | F32 `[4]` の宣言 2 件・束縛 2 件・容量 `None` | 3 つの関数がどれも `Ok(())` |
| `sequential_checks_accept_exact_capacity` | F32 `[4]`（16 バイト）・容量 `Some(16)` | 同上 |
| `sequential_checks_accept_aligned_offset` | `alignment` 16・offset 16・容量 `Some(1024)` | 同上 |
| `sequential_checks_accept_constant_length` | U32 `[3]` の宣言 1 件・12 バイトの定数・束縛なし | 同上 |
| `sequential_checks_accept_exact_limit` | F32 `[1000]`（4,000 バイト）・束縛なし・上限 4,000 | 同上 |
| `sequential_checks_accept_gemm` | M=37・K=53・N=41 の A・B・C と次元の定数・容量 `Some(1 << 20)` | 同上 |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --test execution_operand_check
make verify
```

## 同じ slot の宣言が複数あるときの C4（4/6）

`crates/core/src/execution/dispatch_check.rs` の `check_capacity` は、`declared` に同じ `slot` の宣言が複数あるとき、C4 を最後の宣言で判定する。C5・C6 は先頭の宣言のままで、C8・C7 はすべての宣言を見る。公開の入口は `validate_dispatch_operands` で、`validate_operands` も同じ動きになる（TASK-426）。

単体テスト `dispatch_check_duplicate_slot_uses_last_declaration_for_c4` の入力は、slot 0 の宣言 2 件（先頭 F32 `[1]`・alignment 16、後ろ F32 `[16]`・alignment 0）。

| 段 | 入力 | 期待 |
| -- | ---- | ---- |
| C4 | offset 0・容量 `Some(4)` | `C4` で拒否 |
| C4 | offset 0・容量 `Some(64)` | `Ok(())` |
| C6 | 4 バイトの定数 | `Ok(())` |
| C5 | offset 4 | `C5` で拒否 |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --lib dispatch_check
make verify
```

## 複数の違反の順と重複した宣言の試験（5/6）

C4〜C8 の違反が複数ある入力での返り方と、同じ `slot` の宣言が複数あるときの扱いを、外の crate から公開のパスで `validate_dispatch_operands` を直接呼んで確かめる。上限は `u64::MAX` で、feature なしで動く（TASK-426）。

| 項目 | 値 |
| ---- | -- |
| ファイル | `crates/core/tests/execution_operand_check.rs` |

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `dispatch_checks_return_first_violation_in_order` | C8・C7・C4・C5・C6 のすべてに当たる入力から、違反を 1 つずつ外していく | `C8`、`C7`、`C4`、`C5`、`C6`、`Ok(())` の順 |
| `dispatch_checks_return_earlier_of_two_violations` | 隣り合う 2 つの検査（C8 と C7、C7 と C4、C4 と C5、C5 と C6）に同時に当たる入力 | 前の検査の拒否 |
| `duplicate_declarations_c8_checks_every_declaration` | slot 0 の宣言 2 件の一方の `shape` に 0（前後どちらでも） | どちらも `C8` |
| `duplicate_declarations_c7_checks_every_declaration` | slot 0・slot 0・slot 1 の宣言。束縛は slot 1 / slot 5 / slot 0 を 2 回 | `Ok(())` / `C7` / `C7` |
| `duplicate_declarations_c4_uses_last_declaration` | F32 `[1]` と F32 `[16]` の宣言。束縛 offset 0 | 容量 `Some(4)` で `C4`、`Some(64)` で `Ok(())`、前後を入れ替えると `Some(4)` で `Ok(())` |
| `duplicate_declarations_c5_uses_first_declaration` | `alignment` 16 と 0 の宣言。束縛 offset 4・容量 `None` | `C5`、前後を入れ替えると `Ok(())` |
| `duplicate_declarations_c6_uses_first_declaration` | F32 `[1]` と F32 `[16]` の宣言。定数のみ | 先頭が `[1]` なら 4 バイトで `Ok(())`・64 バイトで `C6`、前後を入れ替えると逆 |

### 確かめ方

```bash
cargo test -p fandhe-silicon-core --test execution_operand_check
make verify
```

## 完了の条件の確かめ（6/6）（TASK-426。以下は当時の base の記録）

6/6（#4762）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-426）。GPU の実機は使わず、下の base で実行した値を書く。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 関数が共通の crate に feature の条件なしで置かれ、公開のパスで呼べる | `grep -n operand_check crates/core/src/execution.rs`・`cargo check -p fandhe-silicon-core`（feature なし） | 12 行目に `mod operand_check;`、27 行目に `pub use operand_check::validate_operands;`。`execution.rs` の先頭 30 行に `cfg` は 0 件。`cargo check` の終了コードは 0 |
| 2 | C1・C3 に当たる入力でその拒否を返し、容量の問い合わせが 0 回 | `cargo test -p fandhe-silicon-core --test execution_operand_check` の `validate_operands_returns_c` | `validate_operands_returns_c1_without_dispatch_checks`・`validate_operands_returns_c3_without_dispatch_checks` が ok。対照の `validate_operands_runs_dispatch_checks_after_declared_checks`・`validate_operands_accepts_input_that_passes_every_check` も ok |
| 3 | 検査が通る 6 通りの入力で 3 つの関数がどれも `Ok(())` | 同上の `sequential_checks_accept_` | 6 件が ok |
| 4 | 複数の違反で C8 → C7 → C4 → C5 → C6 の順に先頭の 1 件 | 同上の `dispatch_checks_return_first_violation_in_order`・`dispatch_checks_return_earlier_of_two_violations` | 2 件が ok |
| 5 | 同じ slot の宣言が複数あるときの C8・C7・C4・C5・C6 | 同上の `duplicate_declarations_` と、`cargo test -p fandhe-silicon-core --lib dispatch_check_duplicate` | 統合テスト 5 件と単体テスト 1 件（`dispatch_check_duplicate_slot_uses_last_declaration_for_c4`）が ok |
| 6 | 背後実装・ファイル・時刻に触れない | `grep -nE 'std::(fs\|time\|env\|net\|process\|thread)\|SystemTime\|Instant' crates/core/src/execution/operand_check.rs crates/core/src/execution/dispatch_check.rs`・`make core-deps-check`・`make wasm-verify` | grep は 0 件。`core-deps-check` は `unexpected=0 missing=0 forward_violations=0` で終了コード 0。`wasm-verify` の終了コードは 0 |
| 7 | 依存 0・unsafe 0 | 1/6 の親（`25f1b96`）から base までの `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` の差分の統計・`grep -c unsafe crates/core/src/execution/operand_check.rs` | 差分は空。unsafe は 0 件 |
| 8 | CI で流れる場所 | `make verify`（`cargo test --workspace --all-features`）・`gh run list --branch main --workflow ci.yml` | `make verify` の終了コードは 0 で、`execution_operand_check` の 18 件、`--lib operand_check` の 6 件、`--lib dispatch_check` の 21 件が ok。base の push の run（`43154445889`）は記録時点で queued |
| 9 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-426）。確かめた base は `459e26f`、`docs/spec` の gitlink は `5f6d802c905938bba1ed963340029d61fe3651a0`。
