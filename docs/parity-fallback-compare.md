# 代わりの実行の結果の比べ方（TASK-928）の所在と本リポの現状

TASK-928 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-928。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-24 | `crates/core/src/parity/tolerance.rs`・`cpu_reference.rs` | [parity-tool](parity-tool.md) |
| TASK-25 | `crates/core/src/parity/special_values.rs` | [parity-non-finite](parity-non-finite.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 一致の道具のファイル | `parity` は 14 個の `pub mod`（`binary16`・`case_inputs`・`cpu_reference`・`fallback_compare`・`fallback_inputs`・`fallback_reference`・`gemm_gate`・`gemm_inputs`・`gemm_population`・`pixel`・`special_values`・`tolerance`・`topk`・`topk_inputs`）で束ねる。比べ方の追加は配下にファイルを足して `pub mod` で束ねる流儀 | `ls crates/core/src/parity`、`grep -n 'pub mod' crates/core/src/parity.rs` |
| 有限でない値の比べ方 | `SpecialVerdict`・`judge_element_with_special_values`・`SpecialComparison`・`compare_f32_with_special_values`。入力は f32 で、両方が有限なら `tolerance::judge_element` へ委ねる | `crates/core/src/parity/special_values.rs`、`crates/core/tests/parity_special_values.rs` |
| 有限の比べ方 | `Tolerance`・`ElementVerdict`・`judge_element`・`Comparison`・`CompareError`・`compare_f32` | `crates/core/src/parity/tolerance.rs`、`crates/core/tests/parity_tolerance.rs` |
| binary16 の変換 | `f32_to_binary16_bits`・`binary16_bits_to_f32`・`round_slice_to_binary16`・`widen_binary16_slice`。ビット列は `u16` で持つ | `crates/core/src/parity/binary16.rs`、`crates/core/tests/parity_binary16.rs` |
| 統合テスト | `crates/core/tests` の `parity_*` は 19 本。`crates/core/Cargo.toml` の `required-features` は 33 件 | `ls crates/core/tests \| grep -c '^parity_'`、`grep -c required-features crates/core/Cargo.toml` |
| feature | `parity`・`test_frame`・`rerun` は `test-support` の裏にある。`test-support = []` で、`contract` の `test-support` が `fandhe-silicon-core/test-support` へ転送する | `crates/core/src/lib.rs`、`crates/core/Cargo.toml`、`crates/contract/Cargo.toml` |
| `exec` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格。`#![forbid(unsafe_code)]` | `git ls-files crates/exec` |
| 依存・unsafe | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。`deps-approved.toml` に `fandhe-silicon-core` を `used_by` に持つ記録は無い（記録は `crates/contract` の macOS 向けの 4 件だけ） | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml`（0 が返る） |
| 走る場所 | `core` の `parity_*` は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-928 の言及は本書、`crates/core/src/parity.rs`、`crates/core/src/parity/fallback_compare.rs`、`crates/core/tests/parity_fallback_compare.rs`、`CLAUDE.md`、`README.md` にある | `git grep -n TASK-928 -- . ':!docs/spec'` |

## 所在（2/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | `crates/core/src/parity/fallback_compare.rs`（`test-support` の裏。標準ライブラリだけ、確保も panic もしない） | `grep -n 'pub mod' crates/core/src/parity.rs` |
| 公開名 | `OrderVerdict`・`judge_order_f32`・`judge_order_binary16`・`FallbackVerdict`・`judge_bits_u32`・`judge_bits_f32`・`FallbackComparison`・`compare_bits_u32`・`compare_bits_f32`。エラーは `tolerance::CompareError` を再利用する | `crates/core/src/parity/fallback_compare.rs` |
| 試験 | `crates/core/tests/parity_fallback_compare.rs`（3/8〜7/8 の分を含めて 53 本） | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

## 振る舞い（2/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 順序の前段 | 有限でない値の 3 区分は `special_values::judge_element_with_special_values` の結果をそのまま写す。`special_values` は変えていない | `order_agrees_with_special_values_on_non_finite` |
| NaN | 両方が NaN なら `BothNan`（payload・符号を問わない）。片方だけなら `NonFiniteMismatch` | `order_nan_pairs_are_both_nan`・`order_single_nan_is_a_mismatch` |
| 無限大 | 同じ符号どうしだけ `SameInfinity`。逆の符号や有限との組は `NonFiniteMismatch` | `order_infinity_requires_the_same_sign` |
| 零 | 両方が零なら符号を問わず `BothZero`。零と最小の非正規化数は `Numeric` | `order_signed_zeros_are_both_zero` |
| 数値の条件へ回す組 | 上のどれにも当たらない有限どうしは `Numeric`。`is_decided` は `Numeric` 以外で真 | `order_finite_non_zero_pairs_go_to_numeric` |
| binary16 | `u16` のビット列を `binary16_bits_to_f32` で広げて同じ前段に通す。65504 と +Inf は前段で `NonFiniteMismatch` に決まる | `binary16_order_kinds`・`binary16_overflow_neighbour_is_decided_before_numeric`・`binary16_same_bits_never_mismatch_in_order` |
| ビット同一 | u32 は値が等しいときだけ `BitIdentical`。f32 は前段で決まらなかったときだけ `to_bits` を比べる。−0 と +0 は前段で `BothZero` になる | `bits_u32_identity`・`bits_f32_after_order` |
| 列の比べ | 長さ違いが先、次に長さ 0 で、検査のあとで添字を読む。集計は比べた数・不一致の数・最初の不一致の添字 | `compare_bits_errors_and_summary` |

## 所在（3/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/8 と同じ `fallback_compare.rs` に足す（新しいファイルは足さない） | `crates/core/src/parity/fallback_compare.rs` |
| 公開名 | `BINARY16_MAX_ORDER_DISTANCE`・`binary16_order_distance`・`judge_order_distance_binary16`・`compare_order_distance_binary16`・`count_bit_identical_binary16`。`FallbackVerdict` に `WithinOrderDistance`・`OrderDistanceExceeded` を足す | `grep -n 'pub ' crates/core/src/parity/fallback_compare.rs` |
| 試験 | 同じ `parity_fallback_compare.rs` に 9 本を足す | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

## 振る舞い（3/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 順序の差 | binary16 のビット列を符号つきの整数の鍵に写し、その差を取る。+0 と −0 は同じ鍵で、隣り合う値で 1。両端でもオーバーフローしない | `order_distance_mapping` |
| 一致の条件 | 前段で決まらない組は、差が `BINARY16_MAX_ORDER_DISTANCE`（1）以下なら `WithinOrderDistance`（一致）、超えれば `OrderDistanceExceeded`。ビット同一の組も `WithinOrderDistance` | `order_distance_neighbours`・`order_distance_same_bits_all_patterns` |
| 非正規化数・零をまたぐ組 | 非正規化数も 0 に落とさずこの条件で比べる。零をまたぐ組も鍵で数える | `order_distance_subnormals`・`order_distance_across_zero` |
| 前段が先 | 最大の有限値と +Inf のようにビット列が隣り合う組は、前段で `NonFiniteMismatch` に決まり、差の条件へ進まない | `order_distance_overflow_neighbour_is_not_within`・`order_distance_non_finite_follows_order` |
| 列の比べ | 長さ違いが先、次に長さ 0。集計の形は 2/8 と同じ | `compare_order_distance_errors_and_summary` |
| ビット同一の記録 | `count_bit_identical_binary16` はビット列が等しい要素の数を返す。判定には使わない | `count_bit_identical_records_only` |

## 所在（4/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/8 と同じ `fallback_compare.rs` に足す（新しいファイルは足さない） | `crates/core/src/parity/fallback_compare.rs` |
| 公開名 | `SUM_RELATIVE_BOUND`・`judge_relative_sum_f32`・`relative_sum_margin`・`compare_relative_sum_f32`。`FallbackVerdict` に `WithinRelativeSum`・`RelativeSumExceeded` を足す | `grep -n 'pub ' crates/core/src/parity/fallback_compare.rs` |
| 試験 | 同じ `parity_fallback_compare.rs` に 8 本を足す | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

## 振る舞い（4/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 前段が先 | 前段で決まる組はその区分に写し、このとき `abs_sum` は見ない | `relative_sum_order_first` |
| 一致の条件 | 前段で決まらない組は、誤差（f64）が `SUM_RELATIVE_BOUND`（1e-3）* `abs_sum` 以下なら `WithinRelativeSum`、超えれば `RelativeSumExceeded`。境目は含み、割り算は使わない | `relative_sum_boundary_is_inclusive`・`relative_sum_within_and_exceeded` |
| `abs_sum` が 0 | 誤差が正なら不一致、誤差 0 なら一致 | `relative_sum_zero_abs_sum` |
| `abs_sum` が使えない値 | 負・NaN・無限大は不一致に倒す | `relative_sum_invalid_abs_sum_fails_closed` |
| 余裕 | `relative_sum_margin` は誤差 / `abs_sum`。前段で一致は 0、前段で不一致は +Inf。記録だけに使い、判定は `judge_relative_sum_f32` だけが決める | `relative_sum_margin_values` |
| 列の比べ | 長さの検査の順は参照と実測、`abs_sums`、`margins`、長さ 0。エラーのとき `margins` は書き換えない | `compare_relative_sum_errors` |
| 余裕の全件記録 | 検査が通れば全添字の余裕を `margins` に書く。集計の形は 2/8 と同じ | `compare_relative_sum_summary_and_margins` |

## 所在（5/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/8 と同じ `fallback_compare.rs` に足す（新しいファイルは足さない） | `crates/core/src/parity/fallback_compare.rs` |
| 公開名 | `F32SubnormalHandling`（`Preserve`・`FlushToZero`）・`flush_subnormal_f32`・`SumReference`（`value`・`abs_sum`）・`reference_sum_f32`。`FallbackVerdict` は変えない | `grep -n 'pub ' crates/core/src/parity/fallback_compare.rs` |
| 試験 | 同じ `parity_fallback_compare.rs` に 10 本を足す | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` |

## 振る舞い（5/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 1 値の置き換え | f32 の非正規化数（正負とも）は +0 になる。±0・NaN・±Inf・正規化数はそのまま | `flush_subnormal_boundaries`・`flush_leaves_special_values` |
| 保つ | 入力も結果も置き換えず、f64 で列の順に足して f32 に 1 回丸める。Σ|x| は元の入力で数える | `reference_preserve_is_plain_f64_sum` |
| 0 に落とす（入力） | 各入力を置き換えてから足す。Σ|x| も置き換えの後の入力で数える | `reference_flush_replaces_inputs` |
| 0 に落とす（結果） | 丸めた結果が非正規化数なら +0 にする。入力が正規化数でも起こり、Σ|x| は保つと同じ | `reference_flush_replaces_rounded_result` |
| 空の列 | 値 +0・Σ|x| 0 | `reference_empty_inputs` |
| 入力の渡し方 | 入力は `IntoIterator<Item = f32>`。飛び飛びの列でも、集めた列と同じ結果 | `reference_accepts_strided_inputs` |
| 判定との接続 | 結果は `judge_relative_sum_f32`・`compare_relative_sum_f32` にそのまま渡せ、扱いによって判定が変わる組がある。Σ|x| が 0 になる組も含む | `reference_flush_changes_verdict` |
| NaN・Inf | 置き換えず和に流れ、前段で決まる区分に従う | `reference_non_finite_inputs_follow_order` |
| ビット同一との接続 | 結果は `judge_bits_f32` にも渡せる | `reference_connects_to_bit_identity` |
| binary16 | この置き換えは当てない（3/8 の順序の差の条件のまま） | （f32 の関数だけ） |

## 所在（6/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/8 と同じ `parity_fallback_compare.rs` に足す（新しいファイルは足さない）。本体の `fallback_compare.rs` は変えない | `git diff --stat origin/main -- crates/core` |
| 試験 | 同じ `parity_fallback_compare.rs` に 10 本（名前は `boundary_` で始まる）を足す | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare boundary_` |

## 振る舞い（6/8）

境目に当たる入力を、判定の入口（ビット同一・順序の差・相対の条件・余裕と、それぞれの列の入口）に通し、前の段で決まる組が、後の段の条件が成り立つ値を与えても前の段の区分になることを確かめる。期待値は IEEE 754 のビット列から直接書く。

| 境目 | 事実 | 対応するテスト |
| ---- | ---- | -------------- |
| NaN が先 | NaN と他の値の組は、Σ|x| が巨大でも、ビット列が隣り合う場合（binary16 の最小の NaN と +Inf）でも `NonFiniteMismatch`。NaN どうしは `BothNan` | `boundary_nan_is_decided_first` |
| 無限大が数値の条件より先（f32） | 最大の有限値と Inf の組は `NonFiniteMismatch` で、Σ|x| が巨大でも一致にならない。同符号の Inf は Σ|x| が使えない値でも `SameInfinity` | `boundary_infinity_before_numeric_f32` |
| 無限大が数値の条件より先（binary16） | 最大の有限値と Inf は順序の差が 1 でも `NonFiniteMismatch`。一つ手前の組は `WithinOrderDistance` | `boundary_infinity_before_order_distance_binary16` |
| 零が数値の条件より先 | 符号つきの零どうしは全入口で `BothZero`。零と最小の非正規化数は前段で決まらず、数値の条件へ進む | `boundary_signed_zero_before_numeric` |
| 隣り合うビット列（binary16） | 差 1 は `WithinOrderDistance`、差 2 は `OrderDistanceExceeded`。非正規化数と正規化数の境も同じ規則で、零をまたぐ組は差 2 | `boundary_adjacent_bits_binary16` |
| 隣り合うビット列（f32） | 1 ulp 違いは `BitsDiffer`、同じビット列は `BitIdentical` | `boundary_adjacent_bits_f32` |
| 相対の条件の境目 | 誤差がちょうど閾値 * Σ|x| なら一致、超えれば不一致。Σ|x| が 0 で誤差 0 は一致。Σ|x| が使えない値は前段で決まらない組だけ不一致 | `boundary_relative_sum_edges` |
| 0 に落とす参照の境目 | 最大の非正規化数は +0、最小の正規化数はそのまま。和がちょうど最小の正規化数なら置き換えられず、その少し下は落とす参照で +0・保つ参照で非正規化数。同じ実測で判定が分かれる | `boundary_flush_reference_edges` |
| binary16 には当てない | binary16 の非正規化数は零と区別し、順序の差の条件で比べる | `boundary_binary16_ignores_flush` |
| 列の集計 | 境目の組を混ぜた列で、`compared`・`mismatched`・`first_mismatch`・余裕が手で書いた期待どおり | `boundary_sequence_order_summary` |

## 所在（7/8）

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| ファイル | 2/8 と同じ `parity_fallback_compare.rs` に足す（新しいファイルは足さない）。本体の `fallback_compare.rs` は変えない | `git diff --stat origin/main -- crates/core/src` |
| 試験 | 同じ `parity_fallback_compare.rs` に 4 本（名前は `threshold_` で始まる）を足す | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare threshold_` |

## 振る舞い（7/8）

| 振る舞い | 事実 | 対応するテスト |
| -------- | ---- | -------------- |
| 定数が外から読め、値が固定 | `BINARY16_MAX_ORDER_DISTANCE` と `SUM_RELATIVE_BOUND` は外の crate から読め、値はテストが固定する | `threshold_constants_are_readable_and_fixed` |
| 順序の差の条件に使う値が読める定数と同じ | 差が定数ちょうどなら `WithinOrderDistance`、定数 + 1 なら `OrderDistanceExceeded`。正・負の正規化数と非正規化数で同じで、列の入口でも同じ | `threshold_order_distance_applied_is_the_readable_one` |
| 相対の条件に使う値が読める定数と同じ | 誤差がちょうど定数 * Σ\|x\| なら一致（境目を含む）、1 ulp 上は不一致、1 ulp 下は一致。Σ\|x\| をわずかに下げると不一致。余裕の値も同じ側に出て、列の入口でも同じ | `threshold_relative_sum_applied_is_the_readable_one` |
| 閾値を引数に取らない | 判定・差・余裕・列の入口の関数は閾値を引数に取らない形で、コンパイルで固定する | `threshold_is_not_a_parameter` |

## 閾値と当てる範囲の扱い

- 閾値の置き場は `fallback_compare.rs` の 2 つの定数（1 か所ずつ）。本書には数値を持たない（数値はテストが固定する）
- `tolerance` の `judge_element` と違い、比べる関数は閾値を引数に取らず定数を中で使う。外から差し替える口は無い
- 道具は「どの出力に当てるか」の情報を持たず、入力の列を受けて集計を返すだけ。`parity` は `test-support` の裏にある
- 当てる範囲は本書では決めない。private spec 側の TASK-53・TASK-38 を参照する（ポインタのみ）

## 値・規則を変える変更の扱い

- 対象は 2 つの定数と、判定の向き（境目を含むこと）。定義の正は private spec 側（TASK-928 のポインタ）
- 本リポの都合で値を変えない。変える必要が出たら該当の作業を止め、[contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の差し戻しの手順に従う
- 値と、それを固定する公開テストの期待値は同じ PR でしか変えない。期待値だけを弱めて通さない

## 完了の条件の確かめ（8/8）

8/8（#6040）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-928）。
結果は GPU の実機なしで、末尾の base で実行した値。件数は実測。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | 本書の試験ファイル全体 | `cargo test -p fandhe-silicon-core --features test-support --test parity_fallback_compare` | `53 passed; 0 failed; 0 ignored`。所在（2/8）の件数と一致 |
| A2 | 境目の入力での判定の順序（6/8） | A1 の出力で 6/8 の節の 10 本（`boundary_` で始まる）が `ok` | 10 本とも `ok`。NaN が先・無限大が数値の条件より先（f32 / binary16）・零が数値の条件より先・隣り合うビット列（binary16 / f32）・相対の条件の境目・0 に落とす参照の境目・binary16 には当てない・列の集計の各行に 1 本ずつ対応 |
| A3 | 前段の振る舞いの裏付け（2/8〜5/8） | A1 の出力で `order_nan_pairs_are_both_nan`・`order_infinity_requires_the_same_sign`・`order_signed_zeros_are_both_zero`・`order_distance_neighbours`・`relative_sum_boundary_is_inclusive`・`reference_flush_replaces_rounded_result` が `ok` | 6 本とも `ok` |
| A4 | 閾値の固定（7/8） | A1 の出力で 7/8 の節の 4 本（`threshold_` で始まる）が `ok` | 4 本とも `ok` |
| C1 | 本体の範囲 | `grep -n 'unsafe\|std::fs\|println!\|env::var\|process::Command' crates/core/src/parity/fallback_compare.rs`、`grep -rn fallback_compare crates/contract/src crates/exec/src crates/upper/src` | どちらも 0 件 |
| C2 | 依存 | `grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml`、`crates/core/Cargo.toml` の依存 | `core` を `used_by` に持つ承認は 0 件、`core` の外部依存は 0 |
| E | main の CI | base の push run の `make verify (fmt + clippy + test)` と `make deny (cargo-deny)` のジョブの step | `make verify`・`make deny`・`make deps-check` の conclusion は success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | すべて終了コード 0。`make verify` の出力に `parity_fallback_compare` の 53 本の実行が出る |

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-928）。

### 再実行の契機

- `crates/core/src/parity/fallback_compare.rs`・`special_values.rs`・`tolerance.rs`・`binary16.rs` を変えたとき
- `crates/core/tests/parity_fallback_compare.rs` を変えたとき
- CI の `verify`・`deny` ジョブの手順を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `1648adc`、`docs/spec` の gitlink は `8664148`。

## 段の表

| 段 | Issue |
| -- | ----- |
| 1/8 現状と前提の確認 | #6033 |
| 2/8 | #6034 |
| 3/8 | #6035 |
| 4/8 | #6036 |
| 5/8 | #6037 |
| 6/8 | #6038 |
| 7/8 | #6039 |
| 8/8 | #6040 |

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。値・規則を変える変更で振る舞いが変わるときは、その PR のコードと公開テストが示し、その PR で本書の節を直す
- 守る規約: [coding-rust](../.claude/rules/coding-rust.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[parity-element-rules](parity-element-rules.md)・[test-frame](test-frame.md)・[parity-fallback-inputs](parity-fallback-inputs.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
