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
