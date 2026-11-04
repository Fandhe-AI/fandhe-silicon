# 経路の突き合わせの道具の所在（TASK-922）

返した経路と、呼び出しの後に読んだ値を突き合わせて 1 回ごとに一致・不一致を出す、試験用の純粋関数の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-922（親の Issue は #866）。

## 本リポの現状の事実

記録時点の commit は `5e75fcc`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/route_evidence.rs` | `ls crates/core/src/route_evidence.rs` |
| 有効になる条件 | feature `test-support` の裏にだけ公開される | `grep -n -B1 'pub mod route_evidence' crates/core/src/lib.rs` |
| 依存 | core の中の `hash`（`sha256_bytes`）だけ。exec・device・buffer の型を受けない | `grep -n '^use' crates/core/src/route_evidence.rs` |
| unsafe | なし（core は `#![forbid(unsafe_code)]`） | `grep -n forbid crates/core/src/lib.rs` |

## 値の型と 1 回の呼び出しの照合（1/4）

### 公開の項目

| 項目 | 内容 |
| ---- | ---- |
| `MARKER_NATIVE` / `MARKER_GPU_EMULATED` / `MARKER_CLEAR` | 印の値の期待。それぞれ `0x4E`・`0x47`・`0` |
| `SENTINEL_BYTE` | 出力の列に埋めるバイト `0xA5` |
| `ReportedRoute` | `Native`・`GpuEmulated`・`Cpu`。経路なしは `Option<ReportedRoute>` の `None` |
| `RunCounts` | 呼び出しの前または後に読んだ回数の組（`native`・`gpu_emulated`・`cpu`・`probe`） |
| `ArtifactDigests` | `Native` と `GpuEmulated` の成果物の SHA-256 の組。`new`・`from_artifacts`・getter 2 つ |
| `CallEvidence` | 1 回の呼び出しの後に読んだ値（`marker`・`before`・`after`・`output`・`artifact`） |
| `CallCheck` | 照合の結果。`marker()`・`counts()`・`output()`・`artifact()`・`probe_runs()`・`is_match()` |
| `EvidenceError` | `EmptyOutput`・`CountsDecreased`・`SameArtifactDigest`。`Display` は固定の英語の文言 |
| `fill_sentinel` / `is_untouched` | 出力の列を `0xA5` で埋める / 全バイトが `0xA5` なら真（空の列は真） |
| `check_call` | 返した経路と `CallEvidence` を突き合わせて `Result<CallCheck, EvidenceError>` を返す |

### 経路ごとの期待

| 返した経路 | 印の値 | 回数の差（`native`, `gpu_emulated`, `cpu`） | 出力の列 | 成果物（`artifacts` が `Some` のとき） |
| ---------- | ------ | ------------------------------------------- | -------- | -------------------------------------- |
| `Some(Native)` | `MARKER_NATIVE` | (1, 0, 0) | `is_untouched` が偽 | `Some(native の SHA-256)` |
| `Some(GpuEmulated)` | `MARKER_GPU_EMULATED` | (0, 1, 0) | `is_untouched` が偽 | `Some(gpu_emulated の SHA-256)` |
| `Some(Cpu)` | `MARKER_CLEAR` | (0, 0, 1) | `is_untouched` が偽 | `None` |
| `None` | `MARKER_CLEAR` | (0, 0, 0) | `is_untouched` が真 | `None` |

- `probe` の差は `probe_runs()` に入り、`counts()` の判定には入らない
- `artifacts` が `None` なら `artifact()` は `None`（比べない）で、`is_match()` は成果物を除いた 3 つで決まる
- 値の不一致は `Ok` の `CallCheck` の `false` で返り、`Err` にならない

### `Err` になる入力

| 入力 | 戻り値 |
| ---- | ------ |
| `output` が空 | `Err(EmptyOutput)` |
| `after` の欄（`probe` を含む）のどれかが `before` より小さい | `Err(CountsDecreased)` |
| `ArtifactDigests::new` / `from_artifacts` の 2 つの SHA-256 が等しい | `Err(SameArtifactDigest)` |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features test-support --lib route_evidence
```

## 呼び出しの数え上げと試験の枠への受け渡し（2/4）

`CallCheck` を呼び出しの列として数える型 `EvidenceTally` を足した。この節の後の module は core の中の `test_frame`（`RunResult`・`Verdict`・`IndeterminateReason`）も使う（`grep -n '^use' crates/core/src/route_evidence.rs`）。

### 公開の項目

| 項目 | 内容 |
| ---- | ---- |
| `EvidenceTally` | 数え上げ。`new`・`record`・`record_call`・getter 9 個・`run_result` |
| `EvidenceError::TallyOverflow` | 数え上げの欄があふれたときの `Err`。`Display` は固定の英語の文言 |

### 数える欄

| 欄 | 1 増えるとき |
| -- | ------------ |
| `calls` | `record` ごとに必ず |
| `matched` / `mismatched` | `is_match()` が真 / 偽 |
| `marker_mismatches` / `count_mismatches` / `output_mismatches` | `marker()` / `counts()` / `output()` が偽 |
| `artifact_mismatches` | `artifact()` が `Some(false)` |
| `artifact_not_compared` | `artifact()` が `None`（`matched` にも数える） |
| `probe_runs` | `probe_runs()` の値を足す（一致・不一致の判定には入らない） |

### `run_result` の結果

| 数え上げ | 結果 |
| -------- | ---- |
| `calls` が 0 | `RunResult::Indeterminate(IndeterminateReason::EvidenceMissing)` |
| `mismatched` が 1 以上 | `RunResult::Verdict(Verdict::Mismatch)` |
| それ以外 | `RunResult::Verdict(Verdict::Match)` |

### `Err` になる入力

| 入力 | 戻り値 |
| ---- | ------ |
| 欄のどれかがあふれる | `Err(TallyOverflow)`。数え上げは変わらない |
| `record_call` に渡した値が `check_call` の `Err` になる | その `Err` をそのまま返す。数え上げは変わらない |

### 対応するテスト（`crates/core/tests/route_evidence_check.rs`）

| 試験 | 確かめる動き |
| ---- | ------------ |
| `all_routes_match` | 4 つの経路の一致を数えると 4 件の一致で `Verdict(Match)` |
| `one_injected_mismatch_is_detected` | 経路 × 値の全組で 1 件の食い違いが、その値の欄にだけ数えられ `Verdict(Mismatch)` |
| `probe_runs_are_counted_apart` | 試しの実行の回数は合計にだけ入り、一致のまま |
| `artifact_not_compared_is_counted` | 成果物を比べない呼び出しは一致と比べなかった欄に数える |
| `input_errors_are_not_counted` | 空の出力・回数の減少・同じ SHA-256 で `Err`、数え上げは不変 |
| `empty_tally_is_indeterminate` | 0 件は `Indeterminate(EvidenceMissing)` |
| `tally_overflow_keeps_counts` | あふれで `Err(TallyOverflow)`、数え上げは不変 |
| `labels_through_the_frame` | `judge` の本体から返して 0 件は判定不能、一致のみは `Label::Ok`、不一致 1 件で `Label::Ng` |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features test-support --lib route_evidence
cargo test -p fandhe-silicon-core --features test-support --test route_evidence_check
```
