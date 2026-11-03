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

## C1 の件数の試験（2/4）

試験は `crates/core/tests/execution_declared_check.rs`。公開のパスだけを使い、feature なしで動く（TASK-420）。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `c1_binding_count_mismatch_is_rejected` | 宣言 1 件・`slots` 2 件 | `InvalidArgument`（先頭 `C1`） |
| `c1_binding_count_match_is_accepted` | 宣言 2 件（`stride` の次元数を `shape` にそろえる）・`slots` 2 件 | `Ok(())` |

```text
cargo test -p fandhe-silicon-core --test execution_declared_check
make verify
```

## C3 の次元数の試験（3/4）

試験は `crates/core/tests/execution_declared_check.rs`。公開のパスだけを使い、feature なしで動く（TASK-420）。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `c3_shape_stride_dimension_mismatch_is_rejected` | 宣言 1 件（`shape` `[4, 4]`・`stride` `[1]`）・`slots` 1 件 | `InvalidArgument`（先頭 `C3`） |
| 同上の対照 | 宣言 1 件（`shape` `[4, 4]`・`stride` `[4, 1]`）・`slots` 1 件 | `Ok(())` |
| `declared_check_return_type_is_pinned` | 署名 `fn(&[OperandSpec<'_>], &BindingLayout) -> Result<(), ContractError>` への代入・空の入力 | コンパイルが通り `Ok(())` |

```text
cargo test -p fandhe-silicon-core --test execution_declared_check
make verify
```

## 完了の条件の確かめ（4/4）（TASK-420。以下は当時の base の記録）

4/4（#4740）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-420）。GPU の実機は使わず、下の base で実行した値を書く。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 関数が共通の crate に feature の条件なしで置かれ、公開のパスで呼べる | `grep -n declared_check crates/core/src/execution.rs`・`cargo check -p fandhe-silicon-core`（feature なし） | 9 行目に `mod declared_check;`、18 行目に `pub use declared_check::validate_declared_operands;`。前後に `#[cfg` は 0 件。`cargo check` の終了コードは 0 |
| 2 | 件数が少ない宣言の拒否と、件数が一致する宣言の通過 | `cargo test -p fandhe-silicon-core --test execution_declared_check` の `c1_` | `c1_binding_count_mismatch_is_rejected`・`c1_binding_count_match_is_accepted` の 2 件が ok |
| 3 | `shape` の次元数が `stride` より多い宣言の拒否と、次元数をそろえた対照の通過 | 同上の `c3_shape_stride_dimension_mismatch_is_rejected` | 1 件が ok |
| 4 | 戻り値の型が `Result<(), ContractError>` で、拒否が `InvalidArgument` と先頭の検査名で照合される | `declared_check_return_type_is_pinned`・試験の helper `is_rejected_by`（`execution_declared_check.rs` の 25 行目） | `declared_check_return_type_is_pinned` が ok。helper は `matches!` で `InvalidArgument` と `starts_with` を見る。統合テストは 4 件が ok |
| 5 | 背後実装・ファイル・時刻に触れない | `grep -nE 'std::(fs\|time\|env\|net\|process\|thread)\|SystemTime\|Instant' crates/core/src/execution/declared_check.rs`・`make core-deps-check`・`make wasm-verify` | grep は 0 件。`core-deps-check` は `unexpected=0 missing=0 forward_violations=0` で終了コード 0。`wasm-verify` の終了コードは 0 |
| 6 | 単体テストが通る | `cargo test -p fandhe-silicon-core --lib declared_check` | 8 passed、0 failed |
| 7 | 依存 0・unsafe 0 | 1/4 の親（`52772d6`）から base までの `Cargo.lock`・`deps-approved.toml`・`crates/core/Cargo.toml` の差分の統計・`grep -c unsafe crates/core/src/execution/declared_check.rs` | 差分は空。unsafe は 0 件 |
| 8 | CI で流れる場所 | `make verify`（`cargo test --workspace --all-features`）・`gh run list --branch main --workflow ci.yml` | `make verify` の終了コードは 0 で、`execution_declared_check` の 4 件と単体テストの 8 件が ok。base の push の run（`41543512673`）は記録時点で in_progress |
| 9 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-420）。確かめた base は `6f71964`、`docs/spec` の gitlink は `d15921d873c6ce20039ecc7eb25a799d25e7748a`。

## 現状の値の補い

上の表の項目 8 の `41543512673` は記録した時点の値（in_progress）で、書き換えていない。`gh run view 41543512673 -R Fandhe-AI/fandhe-silicon --json conclusion` の結果はいま `success`。

関連: dispatch の検査の入力の組は [dispatch-check-inputs.md](dispatch-check-inputs.md)（TASK-2944）。
