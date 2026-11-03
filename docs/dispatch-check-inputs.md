# dispatch の検査の入力の組（TASK-2944）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2944。

## 入力の組と当たる検査（1/2）

引数の宣言の検査の純粋関数（`validate_declared_operands`・`validate_dispatch_operands`）に当てる名前つきの入力の組と、入力から契約の型の値を作る関数を、試験用の feature の裏に置く。

| 項目 | 内容 |
| ---- | ---- |
| module | `crates/core/src/dispatch_inputs.rs` |
| 公開のパス | `fandhe_silicon_core::dispatch_inputs` |
| feature | `test-support` |
| 統合テスト | `crates/core/tests/dispatch_inputs_stub.rs` |

### 入力の一覧

`DISPATCH_INPUTS`（13 件）。stride は shape と同じ次元数の `[1]`（`c3_stride_dims` だけ `[1, 1]`）、alignment は書かれていなければ 4。

| 名前 | 当たる検査 | 宣言（slot・dtype・shape・alignment） | 配置の slot | バッファ（slot・長さ・offset） | 定数（slot・長さ） | dims |
| ---- | ---------- | ------------------------------------- | ----------- | ------------------------------ | ------------------ | ---- |
| `pass_base` | `Pass` | 0・F32・[4] | [0] | 0・16・0 | なし | `Fixed([1, 1, 1])` |
| `pass_aligned_offset` | `Pass` | 0・F32・[4]・16 | [0] | 0・1024・16 | なし | `Fixed([1, 1, 1])` |
| `pass_constant_len` | `Pass` | 3・U32・[3] | [3] | なし | 3・12 | `Fixed([1, 1, 1])` |
| `c1_layout_count` | `C1` | 0・F32・[4] | [0, 1] | 0・16・0 | なし | `Fixed([1, 1, 1])` |
| `c3_stride_dims` | `C3` | 0・F32・[4]（stride [1, 1]） | [0] | 0・16・0 | なし | `Fixed([1, 1, 1])` |
| `c4_capacity` | `C4` | 0・F32・[100] | [0] | 0・16・0 | なし | `Fixed([1, 1, 1])` |
| `c4_offset_past_end` | `C4` | 0・F32・[100] | [0] | 0・4・1000 | なし | `Fixed([1, 1, 1])` |
| `c5_alignment` | `C5` | 0・F32・[4]・16 | [0] | 0・1024・4 | なし | `Fixed([1, 1, 1])` |
| `c6_constant_len` | `C6` | 3・U32・[3] | [3] | なし | 3・8 | `Fixed([1, 1, 1])` |
| `c7_undeclared_slot` | `C7` | 0・F32・[4] | [0] | 5・16・0 | なし | `Fixed([1, 1, 1])` |
| `c8_zero_length` | `C8` | 0・F32・[0] | [0] | 0・4・0 | なし | `Fixed([1, 1, 1])` |
| `zero_dispatch_dims` | `ZeroDispatchDims` | slot 0〜2 が F32・[1]、slot 3 が U32・[1] | [0, 1, 2, 3] | 0〜2 とも 4・0 | 3・4 | `Fixed([0, 1, 1])` |
| `dispatch_dims_over_limit` | `DispatchDimsOverLimit` | 0・F32・[4] | [0] | 0・16・0 | なし | `OverLimit` |

- 最後の 2 入力は C1・C3〜C8 の検査に当たらない（2 関数とも `Ok(())`）
- `dispatch_dims_over_limit(max)` は軸 0 が `max[0] + 1`、軸 1・2 が 1。`max[0]` が `u32::MAX` なら `None`
- `c4_offset_past_end` は C4 に当たる

### 作る関数

| 関数 | 動き |
| ---- | ---- |
| `ExpectedCheck::prefix` | C1・C3〜C8 は `"C1:"` などの接頭辞、ほかは `None` |
| `DispatchInput::layout` | `layout_slots` から配置を作る |
| `DispatchInput::allocate_buffers` | 渡されたメモリ操作の口で `buffers` の並びに確保する。途中で失敗したら確保済みを解放して `Err` を返す |
| `DispatchInput::bindings` | 確保したバッファから束縛の組を作る |
| `DispatchInput::constants` | 定数の束縛の組を作る |
| `DispatchInput::operand_bindings` / `operand_constants` | 検査の関数に渡す束縛と定数を作る |
| `DispatchInput::dispatch_dims` | `Fixed` はその値、`OverLimit` は `dispatch_dims_over_limit` の値 |

### 試験

| 試験 | 確かめること |
| ---- | ------------ |
| `c_inputs_hit_their_prefix` | C1〜C8 の 8 入力が期待の接頭辞で拒否される |
| `pass_inputs_return_ok` | `Pass` の 3 入力で 2 関数とも `Ok(())` |
| `dims_inputs_pass_the_c_checks` | dims の 2 入力で 2 関数とも `Ok(())`。dims の値が 0 を含む・上限を超える |
| `inputs_build_contract_values` | 全 13 入力で束縛・定数・配置が入力の欄と一致する |
| module 内の単体テスト | 名前の重複なし・全期待値の網羅・接頭辞・`dispatch_dims_over_limit` の値 |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features test-support --test dispatch_inputs_stub
cargo test -p fandhe-silicon-core --features test-support --lib dispatch_inputs
make verify
```

## 完了の条件の確かめ（2/2）（TASK-2944。以下は当時の base の記録）

2/2（#6931）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-2944）。GPU の実機は使わず、下の base で実行した値を書く。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 入力の一覧と当たる検査の対応が頁の表にある | `grep -c 'name: "' crates/core/src/dispatch_inputs.rs`・本書の「入力の一覧」の表のデータ行の数・`--lib dispatch_inputs` の `dispatch_inputs_names_are_unique`・`dispatch_inputs_cover_every_expected_check` | module の入力は 13 件、表のデータ行は 13 行。2 件が ok |
| 2 | C1・C3〜C8 の 8 入力が対応する接頭辞で拒否される | `cargo test -p fandhe-silicon-core --features test-support --test dispatch_inputs_stub` の `c_inputs_hit_their_prefix` | ok |
| 3 | `Pass` の 3 入力で 2 関数とも `Ok(())` | 同上の `pass_inputs_return_ok` | ok |
| 4 | dims の 2 入力で 2 関数とも `Ok(())` | 同上の `dims_inputs_pass_the_c_checks` | ok。統合テストは 4 passed、0 failed |
| 5 | 入力を作る関数が試験用の device で契約の型の値を作る | 同上の `inputs_build_contract_values`・`cargo test -p fandhe-silicon-core --features test-support --lib dispatch_inputs` | `inputs_build_contract_values` が ok。単体テストは 4 passed、0 failed |
| 6 | 依存 0・unsafe 0 | 1/2 の親（`d180582`）から 1/2 のコミット（`6cdfdc8`）までの `crates/core/Cargo.toml`・`Cargo.lock`・`deps-approved.toml`・`deny.toml` の差分の統計・`grep -c unsafe crates/core/src/dispatch_inputs.rs crates/core/tests/dispatch_inputs_stub.rs`・`make core-deps-check`・`make deps-check` | 差分は `crates/core/Cargo.toml` の 4 行の追加だけ（`[[test]]` の 1 組）。unsafe は各 0 件。`core-deps-check` は `unexpected=0 missing=0 forward_violations=0` で終了コード 0。`deps-check` の終了コードは 0 |
| 7 | CI で流れる場所 | `make verify`（`cargo test --workspace --all-features`）・`gh run list --workflow ci.yml --branch main` の 1/2 のコミットの run | `make verify` の終了コードは 0。`dispatch_inputs_stub` の 4 件と単体テストの 4 件が ok。CI の run `41652555921` は観測時の status が queued |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-2944）。確かめた base は `6cdfdc8`、`docs/spec` の gitlink は `c7887a1a7f75903d4016ba42e0595ae6cbb2cd0b`。
