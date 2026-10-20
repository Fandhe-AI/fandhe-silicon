# 引数の宣言の C4〜C8 の検査

TASK-421 の 1/4〜4/4（BIND-35〜BIND-37、ART-67・ART-68・ART-85 のポインタ）、TASK-423 の 1/4〜4/4（BIND-42〜BIND-44、ART-84・ART-64・ART-65 のポインタ）、TASK-425 の 1/4〜4/4（BIND-58・BIND-59 のポインタ）、TASK-2952 の 1/2・2/2（BIND-38・BIND-39・BIND-41 のポインタ）。private spec の本文は写さない。

## 所在

`crates/core/src/execution/dispatch_check.rs`。公開のパスは `fandhe_silicon_core::execution::` の下で、feature の条件はない。

| 名前 | 役割 |
| ---- | ---- |
| `validate_dispatch_operands` | C8 → C7 → C4 → C5 → C6 の順に判定する。最初の違反で `Err`、全部通れば `Ok(())` |
| `required_bytes` | 宣言 1 件の必要バイト数。`u64` であふれたら `None` |
| `OperandCheckError` | 暫定のエラー型（`#[non_exhaustive]`、`InvalidArgument(String)`）。契約のエラー型そのものではない |
| `OperandBinding`・`OperandConstant` | 検査の入力のための形。背後実装へ渡す束縛の型ではない |

## 現状の振る舞い

- C8 を宣言ごとに先頭から判定し、続けて C7（宣言に無い `slot`・`slot` の重複）・C4（残容量）・C5（offset の境界）・C6（定数の長さ）を段ごとに全件見る。C4・C5・C6 は宣言の無い `slot` を見ない（C7 が先に拒否する）
- C7: `bindings` を先に、次に `consts` を先頭から見る。`declared` に同じ `slot` の宣言が無ければ拒否（未宣言）。前の `bindings` に同じ `slot` があれば拒否、`consts` では前の `consts` か `bindings` のどれかにあれば拒否（重複。束縛と定数の重なりも重複）。`declared` 内の同じ `slot` の重複は拒否しない。配列を持つ宣言（束縛の配列）の数え方は TASK-422・TASK-1146 のポインタ
- C4: 容量が分かる束縛だけを、必要バイト数と「容量から `offset` を引いた残り（引き切れなければ 0）」とで比べる。容量が `None` の束縛は通す
- C5: 宣言の `alignment` が 0 でないときだけ、`offset` が `alignment` の倍数かを調べる
- C6: 定数のバイト列の長さが必要バイト数と違えば（短くても長くても）拒否する
- 判定の順は、`shape` に 0 の次元がある、必要バイト数が `u64` をあふれる、必要バイト数が `max_buffer_bytes` より大きい。上限ちょうどは通す
- 違反は `InvalidArgument` で、文言の先頭は `C4:`・`C5:`・`C6:`・`C7:`・`C8:` のいずれか（本文は英語）
- `shape` が空なら次元の積は 1 で、1 要素分のバイト数になる
- 算術は `checked_mul`・`saturating_sub`・0 を除いた剰余だけで行う。C7 は前方の要素との比較だけで確保しない。panic・添字アクセス・バッファの確保はない

## 対応するテスト

- `crates/core/src/execution/dispatch_check.rs` の単体テスト（`dispatch_check_` で始まる名前。C8 に加え、`dispatch_check_c4_`・`dispatch_check_c5_`・`dispatch_check_c6_`、`dispatch_check_c7_`、C8 → C7 → C4 → C5 → C6 の順、`declared` 内の重複は C7 にならず先頭の宣言を使うことを確かめる）
- `crates/core/tests/execution_dispatch_check.rs`: 外の crate から公開のパスで呼ぶ統合テスト（`c8_` で始まる名前）。長さ 0 の次元（GEMM の宣言を含む）・`u64` のあふれ・上限超過（1 バイト足りなければ拒否、ちょうどなら通る。巨大な宣言でも確保せずに判定する）を確かめる
- 同じファイルの `c6_` で始まる名前の統合テスト。定数が宣言の必要バイト数より短ければ C6 で拒否、同じ長さなら通る
- 同じファイルの `c4_`・`c5_` で始まる名前の統合テスト。容量 16 のバッファに 400 バイトの宣言は拒否、16 バイトちょうどは通る（C4）。alignment 16 に offset 4 は拒否、offset 16 は通る（C5）
- 同じファイルの `c4_` で始まる名前のうち、容量が不明の束縛（C4 を判定しない）と、offset 0・4・16・1,000・2,000 の束縛（容量から offset を引いた残り。容量以上の offset では 0）を流す試験（TASK-425 の 2/4）
- 同じファイルの `c5_zero_alignment_` で始まる名前の統合テスト。alignment 0 の宣言は offset 4・16・1,000・2,000 でも C5 を判定せずに通り（対照の alignment 16 は offset 4・1,000 で `C5`）、alignment 0 でも C4・C6 は判定される（TASK-425 の 3/4）
- 同じファイルの `c7_` で始まる名前の統合テスト。宣言に無い束縛の `slot`・束縛の `slot` の重複・定数の `slot` の重複（束縛は空）が `C7` で拒否され、1 回だけの対照は通る（TASK-2952 の 2/2）

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --lib dispatch_check
cargo test -p fandhe-silicon-core --test execution_dispatch_check
make verify
```

## 完了の条件の確かめ（4/4）（TASK-421。以下は当時の base の記録）

4/4（#4744）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-421。関連: BIND-35〜BIND-37）。GPU の実機なしで、末尾の base で実行した値。件数は実測。完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 単体テスト | `cargo test -p fandhe-silicon-core --lib dispatch_check` | 6 passed / 0 failed / 0 ignored |
| A2 | 統合テスト | `cargo test -p fandhe-silicon-core --test execution_dispatch_check` | 5 passed / 0 failed / 0 ignored |
| A3 | 統合テストの関数の数 | `grep -c -E '^fn c8_' crates/core/tests/execution_dispatch_check.rs` | 5 |
| A4 | 宣言された大きさの確保・panic がない（拒否の文言の生成を除く） | 本体（`#[cfg(test)]` より上）への grep は `checked_mul` だけ。`crates/core/src/lib.rs` は `#![forbid(unsafe_code)]` | 一致 |
| A5 | feature の条件がない | feature なしの `cargo test -p fandhe-silicon-core`、`make public-api-check` | 全 passed、終了コード 0 |
| A6 | base の CI | `gh run list`（base の push の run、ID 39149201126） | 成功（success、完了済み） |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |

ビヘイビア ID と試験の対応（関数名）:

| ID | 試験 |
| -- | ---- |
| BIND-35 | `c8_zero_length_shape_is_rejected`・`c8_gemm_zero_dimension_is_rejected` |
| BIND-36 | `c8_u64_overflow_in_byte_calculation_is_rejected` |
| BIND-37 | `c8_exceeds_max_buffer_bytes_is_rejected_without_allocating`・`c8_within_max_buffer_bytes_is_accepted` |

試験が照合するのは `InvalidArgument` の variant と、文言の先頭 `C8` まで。

範囲の外: 先頭以外の次元が 0 の入力、C7、背後実装の入口を通した確かめは本書の範囲の外（ポインタ: TASK-477・TASK-422・TASK-423・TASK-494・TASK-495）。

### 再実行の契機

- `dispatch_check.rs`・`execution_dispatch_check.rs`・`execution.rs` の `OperandSpec`／`DType::byte_size` を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `7c962ec88bccc285f63d81e46a1f4381f8490082`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## 完了の条件の確かめ（4/4）（TASK-423。以下は当時の base の記録）

4/4（#4750）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-423。関連: BIND-42〜BIND-44）。GPU の実機なしで、末尾の base で実行した値。件数は実測。完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| B1 | 単体テスト | `cargo test -p fandhe-silicon-core --lib dispatch_check` | 15 passed / 0 failed / 0 ignored |
| B2 | 統合テスト | `cargo test -p fandhe-silicon-core --test execution_dispatch_check` | 11 passed / 0 failed / 0 ignored |
| B3 | 統合テストの関数の数 | `grep -c -E '^fn c4_'`・`'^fn c5_'`・`'^fn c6_'`（`crates/core/tests/execution_dispatch_check.rs`） | 2・2・2 |
| B4 | panic・確保がない（拒否の文言の生成を除く） | 本体（`#[cfg(test)]` より上）への grep は `checked_mul`（`required_bytes`）・`saturating_sub`（C4）・`u64::try_from`（C6）・`%`（C5。alignment が 0 の入力は先に抜ける）だけ。`unwrap`・`expect`・確保は 0 件。`crates/core/src/lib.rs` は `#![forbid(unsafe_code)]` | 一致 |
| B5 | feature の条件がない | feature なしの `cargo test -p fandhe-silicon-core`、`make public-api-check` | 全 passed、終了コード 0 |
| B6 | base の CI | `gh run list`（base の push の run、ID 39161164298） | 成功（success、完了済み） |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |

ビヘイビア ID と試験の対応（関数名）:

| ID | 試験 |
| -- | ---- |
| BIND-42 | `c4_buffer_capacity_exceeded_is_rejected`・`c4_buffer_capacity_sufficient_is_accepted` |
| BIND-43 | `c5_alignment_violation_is_rejected`・`c5_alignment_satisfied_is_accepted` |
| BIND-44 | `c6_constant_size_mismatch_is_rejected`・`c6_constant_size_match_is_accepted` |

試験が照合するのは `InvalidArgument` の variant と、文言の先頭 `C4`・`C5`・`C6` まで。

範囲の外: C7、同じ `slot` の宣言が複数あるときの扱い、容量が不明・offset が 0 より大きい・alignment が 0 の入力、期待より長い定数・定数の slot の入力、背後実装の入口を通した確かめは本書の範囲の外（ポインタ: TASK-422・TASK-424、TASK-426、TASK-425・TASK-477、TASK-475・TASK-476、TASK-494・TASK-495）。

### 再実行の契機

- `dispatch_check.rs`・`execution_dispatch_check.rs`・`execution.rs` の `OperandSpec`／`DType::byte_size`、`OperandBinding`／`OperandConstant`／`binding_capacity` の形を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `22f552400926555b5f8018ffd6feae1518b2d38b`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## 現状と前提の確認（1/4）（TASK-425）

TASK-425（ポインタ: `docs/spec/05-tasks.md`。関連: BIND-58・BIND-59）の 1/4（#4753）で、本リポのコードと公開テストから観測できる事実だけを記録した。前提・完了の条件の正は private spec 側の記録に従う（ポインタのみ）。コードは変えていない。

### 前提

| 前提 | 本リポの物 | 確かめ方 |
| ---- | ---------- | -------- |
| TASK-423 | `dispatch_check.rs` の C4・C5・C6 と、本書の「完了の条件の確かめ（4/4）（TASK-423）」 | `gh issue view 617 --json state`（CLOSED）・本書の節 |

### 現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| C4 の容量が不明の束縛 | `binding_capacity` が `None` なら判定しない（本書「現状の振る舞い」の C4 の行） | `grep -n -A3 'fn check_capacity' crates/core/src/execution/dispatch_check.rs` |
| C4 の offset | `saturating_sub` で残りを出して必要バイト数と比べる | `grep -n saturating_sub crates/core/src/execution/dispatch_check.rs` |
| C5 の alignment 0 | 判定しない | `grep -n 'alignment == 0' crates/core/src/execution/dispatch_check.rs` |
| 単体テスト | 15 件が通る。この入力の種類に触れるのは `dispatch_check_c4_skips_unknown_capacity`（容量 `None`・offset 0）、`dispatch_check_c4_subtracts_offset`（容量 16 に offset 4・1000）、`dispatch_check_c5_skips_zero_alignment`（alignment 0 に offset 4）、`dispatch_check_duplicate_slot_uses_first_declaration_everywhere` | `cargo test -p fandhe-silicon-core --lib dispatch_check` |
| 統合テスト | 11 件が通る。`c4_` は容量 `Some(16)`・offset 0、`c5_` は alignment 16・容量 `None` の入力だけ | `grep -n -E '^fn c[45]_' crates/core/tests/execution_dispatch_check.rs`・`cargo test -p fandhe-silicon-core --test execution_dispatch_check` |
| 統合テストの補助 `spec()` | 既定の alignment は 4 | `grep -n 'alignment' crates/core/tests/execution_dispatch_check.rs` |
| 公開側の TASK-425 の言及 | 本書の TASK-423 の節の「範囲の外」の 1 行だけだった（本節の追加前） | `git grep -n TASK-425 -- . ':!docs/spec'` |

### 段

| 段 | Issue |
| -- | ----- |
| 1/4 現状と前提の確認 | #4753 |
| 2/4 | #4754 |
| 3/4 | #4755 |
| 4/4 | #4756 |

後の段で足す振る舞い・試験は、その PR のコードと公開テストが示し、その PR で本書に節を足す。守る規約は [coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)。

確かめた base は `938c93e714b755785cb25a714a701792c3f8ce5c`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## C4 の容量不明・offset の試験（2/4）（TASK-425）

TASK-425（関連: BIND-58）の 2/4（#4754）で、`crates/core/tests/execution_dispatch_check.rs` に C4 の統合テストを足した。実装（`dispatch_check.rs`）は変えていない。試験の宣言は alignment 4 で、流す offset（0・4・16・1,000・2,000）はすべて 4 の倍数なので C5 には当たらない。

| 関数 | 流す入力 | 結果 |
| ---- | -------- | ---- |
| `c4_unknown_capacity_is_not_checked` | 容量 `None`、必要 400 バイト、offset 0・4・16・1,000・2,000 | すべて通る。対照の容量 16 は `C4` で拒否 |
| `c4_unknown_capacity_binding_is_skipped_per_binding` | slot 0 は容量 `None`（必要 400 バイト）、slot 1 は容量 16 | slot 1 が必要 16 バイトなら通り、20 バイトなら `C4` |
| `c4_offset_reduces_remaining_capacity` | 容量 16、offset 0・4・16・1,000・2,000 | 残り 16・12・0・0・0。残りちょうどは通り、1 要素多いと `C4`。残り 0 では F32 `[1]`・Bool `[]` とも `C4` |
| `c4_large_offset_subtracts_from_known_capacity` | 容量 4,096、offset 1,000・2,000 | 残り 3,096・2,096。残りちょうど（F32 `[774]`・`[524]`）は通り、1 要素多いと `C4` |

実行の結果: `cargo test -p fandhe-silicon-core --test execution_dispatch_check` は 15 件が通る（`c4_` で始まる関数は 2 から 6 になった）。対応: BIND-58（ポインタのみ）。

範囲の外: alignment 0 の入力は 3/4（#4755）、完了の条件の確かめは 4/4（#4756）。

確かめた base は `13fb87a`（`git rev-parse origin/main`）、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## alignment 0 の C5 の試験（3/4）（TASK-425）

TASK-425（関連: BIND-59）の 3/4（#4755）で、`crates/core/tests/execution_dispatch_check.rs` に alignment 0 の統合テストを足した。実装（`dispatch_check.rs`）は変えていない。本節は 4/4（#4756）で記録した。

| 関数 | 流す入力 | 結果 |
| ---- | -------- | ---- |
| `c5_zero_alignment_with_nonzero_offset_is_accepted` | alignment 0、offset 4・16・1,000・2,000 | すべて通る。対照の alignment 16 は offset 4・1,000 で `C5` |
| `c5_zero_alignment_still_applies_other_checks` | alignment 0・F32 `[4]`・offset 4 で容量 16 と 20。U32 `[3]` の定数 8 バイトと 12 バイト | 容量 16 は `C4` で拒否、容量 20 は通る。定数は 8 バイトなら `C6` で拒否、12 バイトなら通る |

対応: BIND-59（ポインタのみ）。確かめた base は `9a634f0`。

## 完了の条件の確かめ（4/4）（TASK-425。以下は当時の base の記録）

4/4（#4756）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-425。関連: BIND-58・BIND-59）。GPU の実機なしで、末尾の base で実行した値。件数は実測。完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| E1 | 単体テスト | `cargo test -p fandhe-silicon-core --lib dispatch_check` | 15 passed / 0 failed / 0 ignored |
| E2 | 統合テスト | `cargo test -p fandhe-silicon-core --test execution_dispatch_check` | 17 passed / 0 failed / 0 ignored |
| E3 | 統合テストの関数の数 | `grep -c -E '^fn c4_'`・`'^fn c5_'`（`crates/core/tests/execution_dispatch_check.rs`） | 6・4 |
| E4 | TASK-425 で実装を変えていない | 1/4 の base `938c93e` から本 base までの `crates/core/src` の差分 | 空 |
| E5 | feature の条件がない | feature なしの `cargo test -p fandhe-silicon-core`、`make public-api-check` | 全 passed、終了コード 0 |
| E6 | base の CI | `gh run list`（base の push の run、ID 39166382577） | 記録時点は in_progress。直前の 3/4 の commit `9a634f0` の run（ID 39166321519）は success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |

ビヘイビア ID と試験の対応（関数名）:

| ID | 試験 |
| -- | ---- |
| BIND-58 | `c4_unknown_capacity_is_not_checked`・`c4_unknown_capacity_binding_is_skipped_per_binding`・`c4_offset_reduces_remaining_capacity`・`c4_large_offset_subtracts_from_known_capacity` |
| BIND-59 | `c5_zero_alignment_with_nonzero_offset_is_accepted`・`c5_zero_alignment_still_applies_other_checks` |

試験が照合するのは `Ok(())` と、`InvalidArgument` の variant・文言の先頭 `C4`・`C5`・`C6` まで。

範囲の外: C7、同じ `slot` の宣言が複数あるときの扱い、alignment が 4・8・64 の C5、背後実装の入口を通した確かめは本書の範囲の外（ポインタ: TASK-424、TASK-426、TASK-475・TASK-476・TASK-477、TASK-494・TASK-495）。

### 再実行の契機

- `dispatch_check.rs`・`execution_dispatch_check.rs`・`execution.rs` の `OperandSpec`／`DType::byte_size`、`OperandBinding`／`binding_capacity` の形を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `2e105b5163b0becd759f4d1e5388c30b1083b11a`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## C7 の実装（1/2）（TASK-2952）

TASK-2952（ポインタ: `docs/spec/05-tasks.md`。関連: BIND-38・BIND-39・BIND-41）の 1/2 で、`dispatch_check.rs` に C7 を足した。公開面は増やしていない。

| 関数 | 流す入力 | 結果 |
| ---- | -------- | ---- |
| `dispatch_check_c7_rejects_undeclared_slots` | 宣言に無い `slot` の束縛（宣言済みなら C4・C5 に当たる値）、同じく定数 | どちらも `C7` |
| `dispatch_check_c7_rejects_undeclared_binding_slot` | 宣言 slot 0 に対し束縛 slot 0 と 5 | 0 は通り、5 は `C7` |
| `dispatch_check_c7_rejects_duplicate_binding_slot` | 束縛 slot 0 を 2 回 | `C7` |
| `dispatch_check_c7_rejects_duplicate_const_slot` | 定数 slot 0 を 1 回と 2 回 | 1 回は通り、2 回は `C7` |
| `dispatch_check_c7_rejects_const_overlapping_binding` | 束縛 slot 0 と定数 slot 0 | `C7` |
| `dispatch_check_c7_checks_bindings_before_consts` | 束縛の重複と定数の未宣言 | 文言は束縛側 |
| `dispatch_check_checks_in_order_c8_c7_c4_c5_c6` | 各段に同時に当たる入力 | C8、C7、C4、C5 の順に先に当たる |

| 段 | Issue |
| -- | ----- |
| 1/2 実装 | #7329 |
| 2/2 統合テストと完了の条件の確かめ | #7330 |

統合テストと完了の条件の確かめは下の 2/2 の節に記録した。

確かめた base は `1b70c78`。

## 完了の条件の確かめ（2/2）（TASK-2952。以下は当時の base の記録）

2/2（#7330）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-2952。関連: BIND-38・BIND-39・BIND-41）。GPU の実機なしで、末尾の base で実行した値。件数は実測。完了の条件の正は private spec 側の記録に従う（ポインタのみ）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| F1 | 単体テスト | `cargo test -p fandhe-silicon-core --lib dispatch_check` | 20 passed / 0 failed / 0 ignored |
| F2 | 統合テスト | `cargo test -p fandhe-silicon-core --test execution_dispatch_check` | 20 passed / 0 failed / 0 ignored |
| F3 | 統合テストの関数の数 | `grep -c -E '^fn c7_' crates/core/tests/execution_dispatch_check.rs` | 3 |
| F4 | 2/2 で実装を変えていない | 1/2 の merge commit `f11c2c2` から `crates/core/src` の差分 | 空 |
| F5 | feature の条件がない | feature なしの `cargo test -p fandhe-silicon-core`、`make public-api-check` | 全 passed、終了コード 0 |
| F6 | base の CI | `gh run list`（base の push の run、ID 39451518781） | 記録時点は in_progress。直前の commit `c68234f` の run（ID 39451182182）は success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0 |

ビヘイビア ID と試験の対応（関数名）:

| ID | 試験 |
| -- | ---- |
| BIND-38 | `c7_undeclared_slot_in_bindings_is_rejected` |
| BIND-39 | `c7_duplicate_slot_in_bindings_is_rejected` |
| BIND-41 | `c7_duplicate_slot_in_consts_is_rejected` |

試験が照合するのは `Ok(())` と、`InvalidArgument` の variant・文言の先頭 `C7` まで。

範囲の外: 定数の未宣言・定数と束縛の重なり（TASK-424）、配列を持つ宣言の数え方（TASK-422・TASK-1146）、`declared` 内の `slot` の重複の扱い（TASK-426）、背後実装の入口を通した確かめ（TASK-494・TASK-495）は本書の範囲の外。

### 再実行の契機

- `dispatch_check.rs`・`execution_dispatch_check.rs`・`execution.rs` の `OperandSpec`、`OperandBinding`／`OperandConstant` の形を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `f11c2c23e7b1ed66ffdd9c407ecec54c514c354b`、`docs/spec` の gitlink は `f603cba40283866ca89338a66a85a9bde085a0d9`。
