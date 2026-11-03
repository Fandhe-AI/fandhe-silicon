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
