# 部分 Top-k の -0.0 の扱いの確かめ（TASK-2451）

## scalar 版と SIMD 版に -0.0 の入力を当てる試験（1/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2451。親 Issue は #1070、この節（1/4）は #6948。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_topk_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`。既にある設定） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::topk_partial_scalar`・`CpuOps::topk_partial_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1352 のポインタ） |
| 入力 | `CaseId::E4`（長さ 16・k=8。TASK-977 のポインタ）と `SIGNED_ZERO_INPUTS` の 4 件（TASK-976 のポインタ）の計 5 件 |
| 関連の頁 | 並びの規則の試験は `docs/cpu-ops-topk-order.md`（TASK-2450 のポインタ）、バイトの比べの試験は `docs/cpu-ops-topk-simd-bytes.md`（TASK-2452 のポインタ） |

### 試験

| 試験 | 確かめる値 |
| ---- | ---------- |
| `topk_case_e4_matches_case_reference` | 2 型 × 2 メソッドの 4 組で、`CaseId::E4` の出力が `CaseId::E4.reference()` と `compare_topk` で一致する。スロット番号が `5..13`、スコアのビットがすべて `1.0_f32.to_bits()` |
| `topk_case_e4_order_rule_holds_directly` | 4 組の出力に `assert_order_rule` を当てる |
| `topk_positive_zero_precedes_negative_zero` | `SIGNED_ZERO_INPUTS` の各入力と 4 組で `assert_order_rule` を当てる。出力に `+0.0` のビットの件が 1 件以上あり、`-0.0` のビットの件があるときは、最後の `+0.0` の位置が最初の `-0.0` の位置より前 |
| `topk_simd_signed_zero_total_order_matches_scalar` | 5 件それぞれで、入力に `-0.0` のビットの語がある。`DefaultCpuOps` の scalar 版の出力と、`DefaultCpuOps`・`ReferenceCpuOps` の SIMD 版の出力の `total_order_mismatches` が `Some(0)`。比べた位置の数の合計が `2 * (8 + 2 + 4 + 2 + 4)` |
| `topk_signed_zero_inputs_differ_from_ieee_order` | `SIGNED_ZERO_INPUTS` が 4 件。4 件の順に、`DefaultCpuOps` の scalar 版の出力と `ieee_ordered_topk` の `total_order_mismatches` が `Some(2)`・`Some(2)`・`Some(1)`・`Some(4)` |
| `topk_total_order_compare_detects_divergence` | `[(1, 0.0), (0, -0.0)]` について、自分自身との比べが `Some(0)`、先頭のスコアだけ `-0.0` にした写しとの比べが `Some(1)`、2 件を入れ替えた写しとの比べが `Some(2)`、1 件を落とした列との比べが `None` |

補助の関数は次のとおり。

- `case_e4`: `CaseId::E4.input()` のスコアの列と k を返す。
- `signed_zero_cases`: `CaseId::E4` と `SIGNED_ZERO_INPUTS` の 4 件の名前・スコアの列・k を返す。
- `total_order_mismatches`: 2 つの出力の長さが違えば `None`、同じなら同じ位置のスロット番号が違うか `f32::total_cmp` が `Equal` でない位置の数を返す。
- `ieee_ordered_topk`: 有限のスコアを `partial_cmp` の降順（同点はスロット番号の昇順）に並べた先頭 k 件を返す。試験の中の比べの相手にだけ使う。

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
```

## -0.0 を含む 5 ケースの入力とネイティブの参照（2/4）

private spec 側の記録に従う（ポインタのみ）。この節（2/4）は #6949。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 道具のファイル | `wasm/crates/parity-host/src/lib.rs`（表 `SIGNED_ZERO_CASES`・`build_signed_zero_case`・`build_signed_zero_cases`）、試験は `wasm/crates/parity-host/tests/gen.rs` |
| 枠の頁 | `docs/wasm32-native-parity.md`（TASK-994・TASK-996 のポインタ） |
| 入力の取り方 | core の `parity::case_inputs` の `CaseId::E4.input()` と `additional_topk_inputs()` から取る。値は道具に写さない |

### ケース

| ケース | n | k | 入力の出どころ |
| ------ | - | - | -------------- |
| `topk-e-4` | 16 | 8 | `CaseId::E4` |
| `topk-signed-zero-adjacent` | 2 | 2 | `signed_zero_adjacent` |
| `topk-signed-zero-separated` | 4 | 4 | `signed_zero_separated` |
| `topk-signed-zero-cut-at-k` | 3 | 2 | `signed_zero_cut_at_k` |
| `topk-signed-zero-repeated` | 4 | 4 | `signed_zero_repeated` |

### 書くファイル

`wasm-parity-host gen <dir>` は、5 ケースそれぞれに `<ケース>.scores.bin` と `<ケース>.native-scalar.bin` を書き、`manifest.json` に `kind` が `topk` の要素を、既存の 8 要素の後ろに足す（計 13 要素）。参照は `DefaultCpuOps` の `topk_partial_scalar` の出力。

### 試験

| 試験 | 確かめる値 |
| ---- | ---------- |
| `signed_zero_cases_match_core_inputs` | ケース名が 5 個の字句に順まで等しい。スコアのビット列と k が core の同じ入力と等しい |
| `signed_zero_inputs_hold_negative_zero_before_positive_zero` | 書いた `.scores.bin` の語に、5 ケースとも `0x8000_0000` と `0x0000_0000` があり、最初の `0x8000_0000` が最初の `0x0000_0000` より前 |
| `signed_zero_native_matches_core` | `.native-scalar.bin` の記録が、`topk-e-4` は `CaseId::E4.reference()`、ほかは `TopKInput::expected()` と `compare_topk` で一致する。比べた件数が k |
| `signed_zero_native_differs_from_ieee_order` | `topk-signed-zero-*` の 4 ケースで、`partial_cmp` の降順（同点はスロット番号の昇順）の先頭 k 件との違う件の数が順に 2・2・1・4 |
| `signed_zero_native_has_no_non_finite_slots` | 5 ケースの記録の件数が k で、スコアがすべて有限 |
| `signed_zero_manifest_entries_match_files` | 5 要素の鍵の組、`kind`、`n`、`k`、ファイルの長さ・中身・sha256 が合う |
| `gen_refuses_existing_signed_zero_file` | `topk-e-4.scores.bin` が先にあると `GenError::FileExists`。中身は変わらない |
| `signed_zero_case_rejects_unknown_input_name` | 存在しない名前の行が `GenError::InputFailed` |

### 確かめ方

```sh
cargo test --manifest-path wasm/Cargo.toml --workspace --locked
grep -n SIGNED_ZERO_CASES wasm/crates/parity-host/src/lib.rs
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | scalar 版と SIMD 版に -0.0 の入力を当てる試験 | #6948 |
| 2/4 | -0.0 を含む 5 ケースの入力とネイティブの参照 | #6949 |
