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
