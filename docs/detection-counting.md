# 変異体の検出の結果を数える道具（TASK-1082）の所在と現状の事実

検出の結果を数える道具の現状について、本リポのコードと公開テストから説明できる事実を記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1082。ポインタのみ）。spec の本文・値・規則は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 変異体ごとの分類（1/4）

### 所在

| 物 | パス |
| -- | ---- |
| crate（package `measure-detect`・lib `measure_detect`） | `measure/crates/detect/` |
| 分類の型と関数 | `measure/crates/detect/src/classify.rs` |
| 失敗の型 `DetectError` | `measure/crates/detect/src/lib.rs` |

### 入力と分類

`classify::classify` は `MutantInput` 1 件から `MutantClass` を返す純粋関数である。

| 入力 | 分類 |
| ---- | ---- |
| `TypeRejected` | `TypeRejected` |
| `Equivalent` | `Equivalent` |
| `NotRun { reason }` | `NotRun { reason }`（理由はそのまま写る） |
| `Ran`: どれかの組が `Positive` | `Detected` |
| `Ran`: `Positive` が無く、どれかの組が `NotCompleted` | `Indeterminate` |
| `Ran`: すべての組が `Negative` | `Missed` |

`MutantClass::in_denominator` は `Detected`・`Missed`・`Indeterminate` で真、ほかの 3 つで偽を返す。

### Err になる入力

`Ran` の入力は次の順に検査し、最初に当たったものを `DetectError` で返す。

| 入力 | variant |
| ---- | ------- |
| 数える組が空 | `EmptyCounted` |
| 数える組に重複 | `DuplicateCounted` |
| 数える組に無い組の結果 | `UnexpectedResult` |
| 同じ組の結果が重複 | `DuplicateResult` |
| 結果が無い組がある | `MissingResult` |

### 依存と unsafe

- 外部依存は 0 件（`measure/crates/detect/Cargo.toml` の `[dependencies]` が空）
- unsafe は 0 件（`grep -rn unsafe measure/crates/detect` と、`measure/Cargo.toml` の `unsafe_code = "forbid"`）

### 確かめ方

- `cargo test --manifest-path measure/Cargo.toml --locked -p measure-detect --lib classify`
- `make measure-verify`

## 下限・上限とセルのラベル（2/4）

### 所在

| 物 | パス |
| -- | ---- |
| 下限・上限・ラベル・セルのラベルの型と関数・定数 | `measure/crates/detect/src/rate.rs` |
| 追加した `DetectError` の variant（`ZeroDenominator`・`Overflow`） | `measure/crates/detect/src/lib.rs` |

### 定数

| 名前 | 値 |
| ---- | -- |
| `DETECTION_THRESHOLD` | 9/10 |
| `MIN_CELL_DENOMINATOR` | 10 |

どちらも関数の引数にせず、環境変数・ファイル・setter などの差し替えの口は無い。判定の関数はこの定数を読む。

### 関数

| 関数 | 入力 | 返し |
| ---- | ---- | ---- |
| `bounds` | 検出・見逃し・判定不能の件数 | 下限 = 検出 / 分母、上限 = （検出 + 判定不能）/ 分母（約分しない。分母は 3 つの和） |
| `label` | `Bounds` | 下限が閾値以上なら `Ok`、そうでなく上限が閾値未満なら `Ng`、ほかは `Indeterminate { reason: REASON_BETWEEN_BOUNDS }` |
| `judge_cell` | `MutantClass` の列と接続の有無 | `CellResult`（各件数・理由の昇順の `not_run`・分母が 0 より大きいときだけ `Some` の `bounds`・ラベル） |

- 比較は浮動小数を使わず、整数の交差の積で行う。上限が閾値ちょうどのときは `Ng` にならない
- `judge_cell` の判定の順は、未接続 → 分母が 10 件未満 → 下限・上限。分母が 10 件未満なら下限が閾値以上でも `Ok` にならない
- 型で拒否・等価・流していないは分母に入れず、件数だけ `CellResult` に出る

### 判定不能の理由の定数

| 名前 | 文字列 |
| ---- | ------ |
| `REASON_FEWER_THAN_MIN` | `fewer than 10 mutants in the denominator` |
| `REASON_NOT_CONNECTED` | `device not connected` |
| `REASON_BETWEEN_BOUNDS` | `bounds straddle the threshold` |

### Err になる入力

| 入力 | variant |
| ---- | ------- |
| `bounds`: 3 つの件数の和が 0 | `ZeroDenominator` |
| `label`: 分母が 0 の `Bounds` | `ZeroDenominator` |
| 件数の和・積があふれる | `Overflow` |

### 確かめ方

- `cargo test --manifest-path measure/Cargo.toml --locked -p measure-detect --lib rate`
- `make measure-verify`
