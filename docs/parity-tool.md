# 結果の一致を確かめる共通の道具（TASK-24）の所在と現状の事実

背後実装の出力を CPU の参照と比べる共通の道具（`parity`）について、本リポのコードと公開テストから説明できる振る舞いを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-24。ポインタのみ）。spec の本文・閾値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity.rs` と `crates/core/src/parity/`（`cpu_reference.rs`・`tolerance.rs`・`topk.rs`・`topk_inputs.rs`・`pixel.rs`・`special_values.rs`・`binary16.rs`・`fallback_compare.rs`・`gemm_inputs.rs`・`gemm_population.rs`・`gemm_gate.rs`・`case_inputs.rs`）。`case_inputs` は [parity-case-inputs](parity-case-inputs.md)、`topk_inputs` は [parity-signed-zero-topk](parity-signed-zero-topk.md)、`pixel` は [pixel-parity-tool](pixel-parity-tool.md)、`special_values` は [parity-non-finite](parity-non-finite.md)、`binary16`・`gemm_inputs`・`gemm_population`・`gemm_gate` は [gemm-test-inputs](gemm-test-inputs.md) | `crates/core/src/parity.rs` |
| feature | `core` の `test-support` の裏にだけ公開される。`contract` の `test-support` は `fandhe-silicon-core/test-support` へ転送する。どの crate の `default` にも入っていない | 各 `Cargo.toml`、`contract_test_support_enables_core_parity_tool`（`crates/contract/tests/test_support_forwarding.rs`） |
| 依存 | 外部依存 0。`unsafe` を使わない（`core` は `#![forbid(unsafe_code)]`） | `Cargo.lock`、`crates/core/src/lib.rs` |
| 背後実装での分岐 | `parity` に背後実装の `cfg` は無い。`core` の `Cargo.toml` に背後実装の feature は無い | `grep -rn "cfg(" crates/core/src/parity.rs crates/core/src/parity`（0 件） |
| 利用側 | `crates/core/tests/`・`crates/contract/tests/test_support_*.rs`・`crates/core/src/test_frame`（[test-frame](test-frame.md)）。`contract`（`src`）・`exec`・`upper` は参照しない。`exec`・`upper` に `test-support` の転送は無い | `grep -rl parity crates` |
| 走る場所 | `core` の `parity_*` の統合テストは `make verify`（`cargo test --workspace --all-features`）で走る。`contract` の `test_support_*` のテストは `make verify` に加え、`make build-profiles`（feature の組の対象は `fandhe-silicon-contract` だけ）の `test-support` を含む 4 組（背後実装なし・Metal だけ・Vulkan だけ・CUDA だけ）でも走る。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` の `PKG`・`NATIVE_SETS` |

## 振る舞い

表の「テスト」列は、`crates/core/tests/` の統合テストの関数名。

### CPU 参照

テストは `crates/core/tests/parity_cpu_reference.rs`。

| 公開アイテム | 振る舞い | テスト |
| ------------ | -------- | ------ |
| `reference_gemm_f32`・`ReferenceError` | f32 の行列積。小さな整数行列で手計算と一致。k の昇順に融合積和で累積する | `small_integer_matrices_match_hand_computed_result`、`accumulates_with_fused_multiply_add_in_ascending_k` |
| 入力検査 | 長さの不一致は入力を名指しするエラー。次元の積のオーバーフローは確保せずにエラー | `length_mismatch_names_the_offending_input`、`dimension_overflow_is_an_error_without_allocation`、`every_dimension_product_is_checked_for_overflow` |
| 確保の失敗 | 出力が大きすぎるときは panic せず `AllocationFailed` | `oversized_output_is_an_allocation_error` |
| 次元 0 | 次元の一つが 0 のとき、`k == 0` は `+0.0` で埋め、`m == 0`・`n == 0` は空の `Ok` | `zero_dimensions_give_zeros_or_empty` |
| エラー表示 | 空でなく ASCII | `error_display_is_non_empty_ascii` |

有限でない入力を渡したときの出力の振る舞いは、公開テストで固定していない。

### 要素ごとの比べ方と閾値の設定

テストは `crates/core/tests/parity_tolerance.rs`。

| 公開アイテム | 振る舞い | テスト |
| ------------ | -------- | ------ |
| `Tolerance`・`T1` | 閾値は `T1` の getter（`relative()`・`absolute()`）で読める | `threshold_is_readable` |
| 同上 | getter で読んだ値が実際に判定へ適用されている（境界の前後で内外が切り替わる） | `readable_thresholds_are_the_ones_applied` |
| 同上 | 外の crate は `T1` 以外の `Tolerance` を作れない（構造体の式がコンパイルできない） | `Tolerance` の rustdoc の doctest（`compile_fail` 2 件と通る例 1 件。`--features test-support --doc`） |
| `judge_element` | 相対誤差は参照に対する比（引数を入れ替えると結果が変わる） | `relative_error_is_measured_against_the_reference` |
| 同上 | 有限で差が大きいだけの組は `NonFinite` でなく `Exceeded` | `large_finite_difference_is_exceeded_not_non_finite` |
| `compare_f32` | 外れと有限でない値が混ざる列で件数を別々に数える | `mixed_mismatches_are_counted_separately` |
| 同上 | 片側だけ空は `EmptyInput` でなく `LengthMismatch` | `one_empty_side_is_a_length_mismatch` |
| `ElementVerdict`・`judge_element` | 相対誤差（参照の絶対値に対する比）の境界は厳密な `<` | `relative_boundary_is_strict` |
| 同上 | 絶対誤差が内なら小さな値でも `Within` | `absolute_rescues_small_values` |
| 同上 | 参照が 0 のときは絶対誤差だけで判定 | `zero_reference_uses_absolute_only` |
| 同上 | 絶対誤差の境界は `f64` で計算 | `absolute_boundary_is_computed_in_f64` |
| 同上 | 参照か実測が有限でなければ `NonFinite`。不一致として数える | `non_finite_values_are_flagged` |
| `Comparison`・`CompareError`・`compare_f32` | 件数・最初に外れた添字を返し、1 要素でも外れれば `is_match()` は偽 | `sequence_summary` |
| 同上 | 長さ 0 は `EmptyInput`、長さ違いは `LengthMismatch` | `errors` |
| CPU 参照との接続 | CPU 参照の出力をそのまま比べに渡せる | `works_with_cpu_reference_output` |

### Top-k の参照と T-2 の比べ方

テストは `crates/core/tests/parity_topk.rs`。

| 公開アイテム | 振る舞い | テスト |
| ------------ | -------- | ------ |
| `TopKEntry`・`reference_topk_f32` | スコアの降順（`f32::total_cmp`）で並べる。`TopKEntry` は `PartialEq` を持たない | `descending_order_matches_hand_computed_result` |
| 同上 | 同点はスロット番号の昇順 | `ties_are_ordered_by_ascending_slot` |
| 同上 | `+0.0` は `-0.0` より前 | `positive_zero_precedes_negative_zero` |
| 同上 | 有限でないスコアは出力に入らない。有限の件数が k に満たなければ有限の件数だけ返す | `non_finite_scores_never_appear`、`fewer_finite_than_k_returns_only_finite_count` |
| 同上 | `k == 0`・空の列は空の `Ok` | `zero_k_and_empty_scores_give_empty_ok` |
| 同上 | `k` が入力より大きくても（`usize::MAX`）有限の件数だけ降順で返る | `k_larger_than_input_returns_all_finite_entries` |
| `TopKReferenceError` | 表示は空でなく ASCII | `reference_error_display_is_non_empty_ascii` |
| `TopKComparison`・`compare_topk` | 位置ごとにスロット番号とスコアのビット列がともに等しいときだけ一致。許容誤差は無い | `identical_lists_match`、`slot_difference_and_one_ulp_difference_are_mismatches` |
| 同上 | `-0.0` と `+0.0` は不一致。有限でない値も同じビット比較 | `signed_zero_difference_is_caught`、`non_finite_entries_follow_the_same_bitwise_rule` |
| 同上 | 長さ 0 は `EmptyInput`、長さ違いは `LengthMismatch`（`CompareError` を再利用） | `length_mismatch_and_empty_are_errors` |
| 同上 | T-1 の内の差（1 ulp）でも不一致 | `difference_within_t1_is_still_a_topk_mismatch` |
| CPU 参照との接続 | 行列積の出力から Top-k を作って比べられる | `gemm_output_feeds_topk_and_compares_equal` |

公開テストから到達させていない経路（表示のテストのみ）:

- `TopKReferenceError::SlotOverflow`: 添字が `u32` に収まらないほど大きな入力が要る
- `TopKReferenceError::AllocationFailed`: 確保の成否が実行環境の空きに依存し、テストで確実に起こせない

### 別 crate からの確かめ

テストは `crates/contract/tests/test_support_forwarding.rs`。

| 振る舞い | テスト |
| -------- | ------ |
| `core` の外の crate から `T1` の getter が読め、有限で正 | `threshold_setting_is_readable_from_outside_core` |
| 行列積 → 比べ → Top-k → T-2 の比べが、背後実装の feature の組に依らず固定の期待値になる（`contract` のテストなので、`make build-profiles` の各組で走る） | `parity_pipeline_gives_fixed_results_under_any_backend_feature_set` |

## 閾値の設定と当てる範囲の扱い

| 点 | 事実 | 確かめ方 |
| -- | ---- | -------- |
| 閾値の置き場 | 値の定義は `crates/core/src/parity/tolerance.rs` の `T1` の 1 か所。本書には数値を持たない | `T1` の定義 |
| 閾値の作り方 | `Tolerance` のフィールドは非公開で、コンストラクタが無い。外の crate が得られる値は `T1` だけ | `Tolerance` の rustdoc の doctest（`compile_fail` 2 件）。stable の rustdoc は `compile_fail` のエラーコードを照合せず、何らかの理由でコンパイルに失敗することしか確かめないので、通る例を同じ rustdoc に置いて import の正しさを担保している |
| 比べる関数との関係 | `judge_element`・`compare_f32` は閾値を引数で受ける。道具の内側に既定値の暗黙の適用は無い | 各関数のシグネチャ |
| T-2 | `compare_topk` は閾値の引数を持たず、位置ごとのビット比較だけ | `slot_difference_and_one_ulp_difference_are_mismatches`、`signed_zero_difference_is_caught` |
| 当てる範囲 | 道具は「どの出力に当てるか」の情報を持たず、入力の列を受けて集計を返すだけ | 各関数のシグネチャ |
| 範囲を決める場所 | 本書では決めない。private spec 側の TASK-53 を参照する（ポインタのみ） | — |

## 値・規則を変える変更の扱い

- 対象: `T1` の値、`judge_element` の比較の向き、`reference_topk_f32` の並べ方、`compare_topk` の一致の規則、有限でない値の数え方
- 定義の正は private spec 側（TASK-24 のポインタ）。本リポの都合で値・規則を変えない。変える必要があると判断したら、該当の作業だけを止め、spec 側の課題としてユーザーへ報告する（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の差し戻しの手順）
- 値・規則と、それを固定する公開テストの期待値は同じ PR でしか変えない。期待値だけを弱めて通さない（[coding-rust](../.claude/rules/coding-rust.md)）

## 確かめ方

worktree のルートで実行する。

- `cargo test -p fandhe-silicon-core --features test-support`: 道具の統合テストと doctest
- `cargo test -p fandhe-silicon-contract --features test-support --test test_support_forwarding`: 別 crate からの確かめ
- `make build-profiles`: `contract` の feature の組ごとのビルド・clippy・test（`test-support` を含む組で `test_support_*` が走る）
- `make verify`: `--all-features` の fmt・clippy・test
- `make deny`・`make deps-check`: 依存が増えていないこと
- `make check`: editorconfig-checker と shellcheck

## 関連

[pixel-parity-tool](pixel-parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[parity-signed-zero-topk](parity-signed-zero-topk.md)・[test-frame](test-frame.md)・[gemm-test-inputs](gemm-test-inputs.md)・[parity-case-inputs](parity-case-inputs.md)・[parity-fallback-compare](parity-fallback-compare.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[perf-gate-rule](perf-gate-rule.md)
