# 結果の一致を確かめる共通の道具（TASK-24）の現状・前提・形

TASK-24（親 Issue #100）について、着手時点の現状と前提、3/8〜5/8（#103〜#105）で入れたコードの現状の形、6/8（#106）で整理した閾値と範囲の扱いを、本リポで公開済みの事実とポインタだけで記録する。
spec の本文・閾値・比べ方は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

「現状の形」はコードと公開テストから観測できる事実。置き場・feature・形は確定している。判断の記録の正は private spec 側（`docs/spec/05-tasks.md` の TASK-24 の状態。ポインタのみ）。
値・比べ方の本文は書かず、コードと公開テストを指す。

## 前提タスクの状態

| 前提 | 状態 | 根拠 |
| ---- | ---- | ---- |
| TASK-4（workspace と crate の骨格） | 済 | PR #3 で骨格を追加済み。`cargo metadata --format-version 1 --all-features --locked` で member 4・パッケージ 4・外部依存 0 件。依存の向きは contract → core、exec → core、upper → core・exec。`make verify`・`make deny` が通る |

## 着手時点の骨格の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| workspace | member 4（core / contract / exec / upper）。外部依存 0 | `cargo metadata` の集計 |
| 公開面 | 4 crate とも crate 冒頭の doc コメントのみ（`contract` は cfg ゲート付きの空の backend モジュール宣言 `backend_metal`・`backend_vulkan`・`backend_cuda` を持つ）。`core`・`exec`・`upper` は `#![forbid(unsafe_code)]` | `crates/*/src/lib.rs` |
| contract の feature | `cpu-ops`・`test-support` などは宣言のみで中身は無い | `crates/contract/Cargo.toml` |
| 既存のテスト | `crates/contract/tests/backend_cfg_gate.rs` のみ | `find crates -name '*.rs'` |
| 結果を比べる道具 | コード・テスト・文書とも 0 件 | リポ内検索 |

- `make verify` は `--all-features` で clippy と test を流すため、feature の裏に置くコードも検査の対象になる

## 現状の形

本節はコードと公開テストから観測できる事実だけを書く。`parity` は feature `test-support` の裏にだけ公開される。

### 全体

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/parity.rs` と `crates/core/src/parity/`（`cpu_reference.rs`・`tolerance.rs`・`topk.rs`） | `crates/core/src/lib.rs`、`crates/core/src/parity.rs` |
| feature | `core` に `test-support` を新設。`contract` の `test-support` は `fandhe-silicon-core/test-support` へ転送する。どの crate の `default` にも入れない | 各 `Cargo.toml`、`cargo metadata`、`contract_test_support_enables_core_parity_tool`（`crates/contract/tests/test_support_forwarding.rs`） |
| 依存 | 外部依存は 0 のまま。`contract` から `core` への feature の転送だけを足した | `Cargo.lock` に差分なし |
| 続きの足し方 | 比べ方は `parity/` にファイルを足して `parity.rs` に `pub mod` を 1 行足す | `crates/core/src/parity.rs` |
| 入れていないもの | 要素ごとの演算の参照。`exec`・`upper` からの利用口（使うコード・テストが無く、使われない feature の転送は公開面だけを増やすので足していない） | リポ内検索 |
| 走る場所 | `make verify`（`--all-features`）と、`make build-profiles` の `test-support` を含む 4 組（背後実装なし・Metal だけ・Vulkan だけ・CUDA だけ）。背後実装を選んだ組でも道具のテストは `required-features` で skip されず走る | `scripts/build-profiles.sh` の `NATIVE_SETS`、`make build-profiles` のログ |
| 背後実装の feature での分岐 | `core` に宣言の無い feature 名の `cfg` を書くと `unexpected_cfgs` が `-D warnings` でエラーになる。`core` の `Cargo.toml` に backend の feature を足さない限り `parity` は背後実装で分岐できない | 試行（コミットしていない） |

### CPU 参照

| 公開アイテム | 観測できる振る舞い | 確かめ方（`crates/core/tests/parity_cpu_reference.rs`） |
| ------------ | ------------------ | ------------------------------------------------------ |
| `reference_gemm_f32`・`ReferenceError` | f32 の行列積。小さな整数行列で手計算と一致。k の昇順に融合積和で累積する | `small_integer_matrices_match_hand_computed_result`、`accumulates_with_fused_multiply_add_in_ascending_k` |
| 入力検査 | 長さの不一致は入力を名指しするエラー。次元の積のオーバーフローは確保せずにエラー | `length_mismatch_names_the_offending_input`、`dimension_overflow_is_an_error_without_allocation`、`every_dimension_product_is_checked_for_overflow` |
| 確保の失敗 | 出力が大きすぎるときは panic せず `AllocationFailed` | `oversized_output_is_an_allocation_error` |
| 次元 0 | 次元の一つが 0 のとき、`k == 0` は `+0.0` で埋め、`m == 0`・`n == 0` は空の `Ok` | `zero_dimensions_give_zeros_or_empty` |
| エラー表示 | 空でなく ASCII | `error_display_is_non_empty_ascii` |

### 要素ごとの比べ方と閾値の設定

| 公開アイテム | 観測できる振る舞い | 確かめ方（`crates/core/tests/parity_tolerance.rs`） |
| ------------ | ------------------ | --------------------------------------------------- |
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

| 公開アイテム | 観測できる振る舞い | 確かめ方（`crates/core/tests/parity_topk.rs`） |
| ------------ | ------------------ | ---------------------------------------------- |
| `TopKEntry`・`reference_topk_f32` | スコアの降順（`f32::total_cmp`）で並べる。`TopKEntry` は `PartialEq` を持たない | `descending_order_matches_hand_computed_result` |
| 同上 | 同点はスロット番号の昇順 | `ties_are_ordered_by_ascending_slot` |
| 同上 | `+0.0` は `-0.0` より前 | `positive_zero_precedes_negative_zero` |
| 同上 | 有限でないスコアは出力に入らない。有限の件数が k に満たなければ有限の件数だけ返す | `non_finite_scores_never_appear`、`fewer_finite_than_k_returns_only_finite_count` |
| 同上 | `k == 0`・空の列は空の `Ok` | `zero_k_and_empty_scores_give_empty_ok` |
| `TopKReferenceError` | 表示は空でなく ASCII | `reference_error_display_is_non_empty_ascii` |
| `TopKComparison`・`compare_topk` | 位置ごとにスロット番号とスコアのビット列がともに等しいときだけ一致。許容誤差は無い | `identical_lists_match`、`slot_difference_and_one_ulp_difference_are_mismatches` |
| 同上 | `-0.0` と `+0.0` は不一致。有限でない値も同じビット比較 | `signed_zero_difference_is_caught`、`non_finite_entries_follow_the_same_bitwise_rule` |
| 同上 | 長さ 0 は `EmptyInput`、長さ違いは `LengthMismatch`（`CompareError` を再利用） | `length_mismatch_and_empty_are_errors` |
| CPU 参照との接続 | 行列積の出力から Top-k を作って比べられる | `gemm_output_feeds_topk_and_compares_equal` |
| T-1 と T-2 の違い | T-1 の内の差（1 ulp）でも `compare_topk` は不一致 | `difference_within_t1_is_still_a_topk_mismatch` |
| `reference_topk_f32` | `k` が入力より大きくても（`usize::MAX`）有限の件数だけ降順で返る | `k_larger_than_input_returns_all_finite_entries` |

公開テストから到達させていない経路（表示のテストのみ）:

- `TopKReferenceError::SlotOverflow`: 添字が `u32` に収まらないほど大きな入力が要るため。
- `TopKReferenceError::AllocationFailed`: 有限の要素数に対する `try_reserve_exact` の失敗を変換したもので、`u32` 以下の件数でもメモリ不足なら起こる。確保の成否は実行環境の空きに依存し、テストで確実に起こせないため。

### 別 crate からの確かめ

| 観測できる振る舞い | 確かめ方（`crates/contract/tests/test_support_forwarding.rs`） |
| ------------------ | -------------------------------------------------------------- |
| `core` の外の crate から `T1` の getter が読め、有限で正 | `threshold_setting_is_readable_from_outside_core` |
| 行列積 → 比べ → Top-k → T-2 の比べが、背後実装の feature の組に依らず固定の期待値になる（`make build-profiles` の各組で走る） | `parity_pipeline_gives_fixed_results_under_any_backend_feature_set` |

## 閾値の設定と当てる範囲の扱い（6/8）

コードと既存の規約から言える点だけを書く。範囲そのものは本書で決めない。

| 点 | 事実 | 確かめ方 |
| -- | ---- | -------- |
| 閾値の置き場 | 値の定義は `crates/core/src/parity/tolerance.rs` の `T1` の 1 か所。本書には数値を持たない | `T1` の定義 |
| 閾値の作り方 | `Tolerance` のフィールドは非公開で、コンストラクタが無い。外の crate が得られる値は `T1` だけ | `tolerance.rs` の定義。`Tolerance` の rustdoc の doctest（`compile_fail` 2 件）で確かめる。stable の rustdoc は `compile_fail` のエラーコードを照合せず、何らかの理由でコンパイルに失敗することしか確かめないので、通る例を同じ rustdoc に置いて import の正しさを担保している |
| 比べる関数との関係 | `judge_element`・`compare_f32` は閾値を引数で受ける。道具の内側に既定値の暗黙の適用は無い | 各関数のシグネチャ |
| T-2 | `compare_topk` は閾値の引数を持たず、位置ごとのビット比較だけ | `slot_difference_and_one_ulp_difference_are_mismatches`、`signed_zero_difference_is_caught` |
| 当てる範囲 | 道具は「どの出力に当てるか」の情報を持たず、入力の列を受けて集計を返すだけ。`parity` を使うのは `crates/core/tests/` と `crates/contract/tests/test_support_forwarding.rs` だけで、`src/` からの利用は `core` 内に限る。`exec`・`upper` には feature の転送が無い | `grep -rn parity crates`、各 `Cargo.toml` |
| 範囲を決める場所 | 本書では決めない。TASK-53（#196）のポインタのみ | — |

## 値・規則を変える変更の扱い

- 対象: `T1` の値、`judge_element` の比較の向き、`reference_topk_f32` の並べ方、`compare_topk` の一致の規則、有限でない値の数え方
- 定義の正は spec 側（TASK-24 のポインタ）。本リポの都合で値・規則を変えない。変える必要があると判断したら、該当の作業だけを止め、spec 側の課題としてユーザーへ報告する（[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の差し戻しの手順）
- 値・規則と、それを固定する公開テストの期待値は同じ PR でしか変えない。期待値だけを弱めて通さない（`AGENTS.md` 2 節、[coding-rust](../.claude/rules/coding-rust.md)）
- 値・規則を変える変更を本リポの PR では決めない。上の手順で spec 側の課題として報告し、オーナーに伺う。判断の記録は spec リポ側に置き、本リポからは書き換えない。得ていない承認を書かない

## 関連タスクの状態と引き継ぎ

事実のみを書く。向きは決めない。

| 関連 | Issue | 状態 | 引き継ぎ |
| ---- | ----- | ---- | -------- |
| TASK-23 | #91 | 未完了（並行） | 共通の試験の枠の置き場と範囲 (1) は本リポにある（[test-frame](./test-frame.md)）。範囲 (2) のうち、結果のラベルの決定と `parity` の比較型から判定への写像も本リポにある（`parity` は変えていない）。記録の出力と集計はまだ無い |
| TASK-25 | #109 | Issue はクローズ済み | TASK-24 を前提に引く下流。足した規則は [parity-non-finite](parity-non-finite.md) |
| TASK-53 | #196 | 未完了（Issue は OPEN） | 適用の範囲の判断はこのタスクに委ね、TASK-24 の側で広げも狭めもしない。コード側の見る場所は `T1`・getter・`threshold_is_readable` |
| TASK-6 | #35 | 済（クローズ） | 決定の記録は spec リポの main にある（本リポの submodule の固定 SHA はこの記録より前）。持ち込みは [poc-carry-over](../.claude/rules/poc-carry-over.md) と [workspace-boundary](workspace-boundary.md) に従う |

- 依存: 外部依存は 0。追加が要るときは [dependency-policy](../.claude/rules/dependency-policy.md) の承認フローに乗せる
- `cpu-ops` に関わる下流のポインタ: TASK-1350・TASK-1378

## 形の設計

既存の決定から一意に導ける点を、根拠のポインタ付きで記録する。

| # | 点 | 形 | 根拠 |
| - | -- | -------- | ---- |
| D1 | PoC からの持ち込みか書き直しか | PoC のファイルは複製せず、本リポの規約で新しく書く。記録・入力・生成物を写さない | TASK-6 の決定の記録、[poc-carry-over](../.claude/rules/poc-carry-over.md)、[workspace-boundary](workspace-boundary.md) |
| D2 | 依存・unsafe | 標準ライブラリだけで書く。外部依存は 0（承認フローに乗る依存は無い）。`unsafe` を使わない | [dependency-policy](../.claude/rules/dependency-policy.md)、[coding-rust](../.claude/rules/coding-rust.md) |
| D3 | 背後実装への非依存 | `parity` に `cfg(feature = "backend-*")` は無く、標準ライブラリだけで書く。`core` は `#![forbid(unsafe_code)]`・外部依存 0。GPU なしの CI で走る。contract からは feature の転送だけ | `crates/core/src/parity.rs`、`crates/contract/tests/test_support_forwarding.rs`、TASK-24（ポインタ） |
| D4 | 閾値の設定 | `crates/core/src/parity/tolerance.rs` の rustdoc とテスト、本書「現状の形」を参照 | TASK-24 の完了の条件、`docs/spec/02-poc-plan.md` の T-1・T-2（ポインタのみ） |
| D5 | 適用の範囲 | 本書では記述しない。TASK-53（#196）に委ねる。実装と公開テストが入る PR でコードから記述する | TASK-53（#196）（ポインタのみ） |
| D6 | TASK-23 の枠との接続 | 本書では記述しない。比較型から判定への写像は `test_frame` の側にあり（`parity` は変えていない）、現状は [test-frame](./test-frame.md) の「現状の形（5/8）」 | TASK-23（#91）（ポインタのみ） |
| D7 | T-1 の要素の分類 | 4/8 では有限でない値を `NonFinite` として不一致に数えるだけ。それ以外の規則は TASK-25（#109）で扱う | TASK-25（#109）、PARITY-2〜PARITY-5（ポインタのみ） |
| D8 | T-1 の比べの形 | `crates/core/src/parity/tolerance.rs` の rustdoc とテスト、本書「現状の形」を参照 | PARITY-1（ポインタのみ） |
| D9 | T-2 の比べの形 | `crates/core/src/parity/topk.rs` の rustdoc とテスト、本書「現状の形」を参照 | PARITY-6、`docs/spec/02-poc-plan.md` の T-2（ポインタのみ） |
| D10 | CPU 参照の形 | `crates/core/src/parity/cpu_reference.rs` の rustdoc とテスト、本書「現状の形」を参照 | TASK-1350・TASK-1378・CPU-27（ポインタのみ） |
| D11 | 結果とエラー | `crates/core/src/parity/cpu_reference.rs` の rustdoc とテスト、本書「現状の形」を参照 | [coding-rust](../.claude/rules/coding-rust.md)、[security](../.claude/rules/security.md)、[japanese-style](../.claude/rules/japanese-style.md) |
| D12 | 走る場所 | `make verify` の `cargo test` と、`make build-profiles` の `test-support` を含む 4 組。GPU の実機は要らず GitHub ホステッドの CI で走る。新しい make ターゲット・CI ジョブは足さない | [unsafe-policy](unsafe-policy.md) と同じ考え方 |

## 後続の分担

| 作業 | 担当 |
| ---- | ---- |
| 形の設計（本節群） | #102（2/8。済） |
| 置き場の骨格と CPU 参照 | #103（3/8。済） |
| 許容誤差つきの要素ごとの比べ方と閾値の設定 | #104（4/8。済） |
| Top-k の参照と T-2 の比べ | #105（5/8。済） |
| 方針の整理（本書の構成の整理、閾値と範囲の扱い） | #106（6/8。済） |
| テスト。判断: `build-profiles` に `test-support` を含む 4 組を足した / `Tolerance` を外から作れないことを rustdoc の `compile_fail` で固定した / `exec`・`upper` からの利用口は足していない（利用するコードが無い）/ TASK-23（#91）の枠との接続は触れていない（D6 のまま。#91 は未完了） | #107（7/8。済） |
| 完了の条件の確かめ | #108（8/8。済。本書の「完了の条件の確かめ」に記録） |

## 完了の条件の確かめ

8/8（#108）で、本リポで観測できる事実による確かめを記録する（ポインタ: TASK-24）。
spec の完了の条件との対応づけの記録の正は private spec 側（TASK-24 の状態）。条件の本文は転記しない。

| 確かめる項目 | 方法 | 結果 |
| ------------ | ---- | ---- |
| `parity` に背後実装の feature での分岐が無い（D3） | `crates/core/src/parity.rs` と `crates/core/src/parity/` を `backend`・`cfg(` で grep | 0 件 |
| `unsafe` を使っていない（D2） | `crates/core/src` を `unsafe` で grep | `lib.rs` の `#![forbid(unsafe_code)]` の 1 行だけ |
| 外部依存 0 のまま（D2） | `Cargo.lock` の `[[package]]` の数 | 4（workspace の 4 crate だけ） |
| 製品の経路から `parity` を参照していない | `crates/exec`・`crates/upper`・`crates/contract/src` を `parity` で grep | 0 件 |
| `test-support` が `default` に入っていない | 各 `Cargo.toml` の `[features]` | `core` に `default` なし、`contract` の `default` に含まれない |
| 道具のテストが通る | `cargo test -p fandhe-silicon-core --features test-support` | 通過（統合テスト 3 本が 8・14・15 件、doctest も通過） |
| 背後実装の feature の組に依らず同じ結果になる | `make build-profiles` の `test-support` を含む 4 組で `parity_pipeline_gives_fixed_results_under_any_backend_feature_set` | 4 組すべてで通過 |
| 閾値の設定が `core` の外から読める（D4） | 同 4 組で `threshold_setting_is_readable_from_outside_core` | 4 組すべてで通過 |
| 読める値が判定に当たっている・外から任意の閾値を作れない・T-2 の比べが閾値の引数を持たない | `parity_tolerance`・`parity_topk` の統合テストと `Tolerance` の rustdoc の `compile_fail` | 通過 |
| GPU の実機なしで走る（D12） | 上のテストはいずれも実機なしで実行した。CI の結果は PR で確かめる | ローカルでは通過。CI は PR の結果で確認（未検証） |
| 品質ゲート | `make check`・`make verify`・`make build-profiles`・`make deny`・`make deps-check` | すべて終了コード 0 |
| 適用の範囲・TASK-23 の枠との接続 | 本書 D5・D6 のまま | 該当なし（TASK-53（#196）・TASK-23（#91）に委ねる。本書では触れない） |

- 判定: 本リポで観測できる範囲では、上の項目はすべて確かめられた。完了の条件との対応づけの記録の正は private spec 側（TASK-24 の状態）
- 再実行の契機: `parity` の置き場・feature の変更、`T1` の値・比べの規則の変更、TASK-25（#109）での規則の追加、TASK-53（#196）の範囲の決定、`scripts/build-profiles.sh` の組の変更

## 確かめた日付

2026-10-11（base: f73258b）
