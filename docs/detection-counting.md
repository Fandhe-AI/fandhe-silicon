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

## コマンドと境界の入力の試験（3/4）

### 所在

| 物 | パス |
| -- | ---- |
| 件数から下限・上限とラベルを出すコマンド | `measure/crates/detect/src/bin/detect-bounds.rs` |
| 分類からセルのラベルまでの境界の入力の試験 | `measure/crates/detect/tests/counting.rs` |
| コマンドを起動して確かめる試験 | `measure/crates/detect/tests/detect_bounds_cmd.rs` |

### コマンド

使い方は `detect-bounds <detected> <missed> <indeterminate>`（10 進の非負整数 3 つ）。`rate::bounds` と `rate::label` を呼び、標準出力に 1 行の JSON を出す。セルの規則（最小件数・未接続）は当てない。ファイル・環境変数・時計・ネットワークは使わず、ファイルを作らない。

| キー | 中身 |
| ---- | ---- |
| `detected`・`missed`・`indeterminate` | 引数の件数 |
| `denominator` | 3 つの和 |
| `lower`・`upper` | `numerator` と `denominator`（約分しない） |
| `threshold` | `DETECTION_THRESHOLD` の分子・分母 |
| `label` | `"ok"`・`"ng"`・`"indeterminate"` |
| `reason` | 判定不能のときだけ理由の定数の文字列、ほかは `null` |

小数・丸めの値は出さない。

| 終了コード | 意味 |
| ---------- | ---- |
| 0 | 出力した（`--help` を含む） |
| 1 | 入力の違反（分母 0・あふれ）。JSON は出さない |
| 2 | 誤用（引数の数が 3 でない・非負整数でない・`-` で始まる引数） |

標準エラーには固定の英語の文言と引数の番号だけを出し、入力の値は出さない。先頭の 0 は受け、`+`・空・小数・英字・`u64` を超える値は誤用になる。

### 試験

`counting.rs`（値は合成）:

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `ten_with_nine_detected_is_ok` | 検出 9・見逃し 1 | 下限・上限 9/10、`Ok` |
| `eight_detected_two_indeterminate_is_indeterminate` | 検出 8・判定不能 2 | 下限 8/10・上限 10/10、`REASON_BETWEEN_BOUNDS` |
| `two_or_more_missed_is_ng` | 10 件で見逃し 2・3・10 | `Ng` |
| `fewer_than_ten_is_indeterminate` | 検出 9。型で拒否・等価・未実施を足した 10 件 | `REASON_FEWER_THAN_MIN`。足しても分母 10 のまま `Ok` |
| `not_connected_is_indeterminate` | 検出 10、未接続 | `REASON_NOT_CONNECTED` |
| `excluded_classes_are_counted_only` | 分母 10 に型で拒否・等価・未実施 | 件数と理由の昇順の件数にだけ現れる |
| `classify_then_judge` | `Ran` を 10 件、数える組 2〜3 | 分類を通した件数とラベル |
| `classify_rejects_inconsistent_pairs` | 空・重複・欠け・余分・結果の重複 | それぞれ対応する `DetectError` |
| `threshold_constants` | 定数 | 9/10 と 10 |

`detect_bounds_cmd.rs`: `9 1 0`（`ok`）・`8 0 2`（`indeterminate`）・`8 2 0`（`ng`）の出力と終了コード 0、`0 0 0` と `u64` の最大値を含む組の終了コード 1、引数の数の違い・`-1`・`1.5`・`abc`・`+1`・`u64` 超え・空の引数の終了コード 2、標準エラーに入力の値が出ないこと。

### 確かめ方

- `cargo test --manifest-path measure/Cargo.toml --locked -p measure-detect --test counting`
- `cargo test --manifest-path measure/Cargo.toml --locked -p measure-detect --test detect_bounds_cmd`
- `cargo run --manifest-path measure/Cargo.toml --locked -p measure-detect --bin detect-bounds -- 9 1 0`
- `make measure-verify`

## 完了の条件の確かめ（4/4）

4/4 で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-1082）。
GPU の実機なしで、末尾の base で実行した値。件数は実測。

| 成果物 | 本書の節 | 試験 |
| ------ | -------- | ---- |
| `classify` | 変異体ごとの分類（1/4） | `measure/crates/detect/src/classify.rs` の単体試験と `tests/counting.rs` |
| `rate` | 下限・上限とセルのラベル（2/4） | `measure/crates/detect/src/rate.rs` の単体試験と `tests/counting.rs` |
| `detect-bounds` | コマンドと境界の入力の試験（3/4） | `measure/crates/detect/tests/detect_bounds_cmd.rs` |

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | crate が `measure/` の member で、製品の workspace の外にある | `grep -n members measure/Cargo.toml`、`grep -n exclude Cargo.toml`、`make upper-layer-check` | `members = ["crates/detect", "crates/perf"]`、`exclude = ["baseline-wgpu", "measure", "wasm"]`、`measure_pkgs=2 measure_contract_pkgs=0 measure_forbidden=0` |
| 2 | 分類の試験が通る | `cargo test --manifest-path measure/Cargo.toml --locked -p measure-detect --lib classify`、同じ形に `--test counting` | `--lib classify` は 11 passed; 0 failed; 0 ignored。`--test counting` の `classify_then_judge`・`classify_rejects_inconsistent_pairs`・`excluded_classes_are_counted_only` は ok |
| 3 | 下限・上限とセルのラベルの境界の試験が通る | 同じ形に `--lib rate`、`--test counting` | `--lib rate` は 13 passed; 0 failed; 0 ignored。`--test counting` は 9 passed; 0 failed; 0 ignored（`ten_with_nine_detected_is_ok`・`eight_detected_two_indeterminate_is_indeterminate`・`two_or_more_missed_is_ng`・`fewer_than_ten_is_indeterminate`・`not_connected_is_indeterminate`・`threshold_constants` を含む） |
| 4 | 閾値と最小の件数が定数で、差し替える口が無い | `grep -n 'pub const DETECTION_THRESHOLD\|pub const MIN_CELL_DENOMINATOR' measure/crates/detect/src/rate.rs`、`grep -rn 'env::var\|std::env' measure/crates/detect/src` | 前者は `rate.rs` の 28 行目と 34 行目。後者のヒットは `src/bin/detect-bounds.rs` の `std::env::args_os()` の 1 件だけ |
| 5 | コマンドの試験が通り、合成の引数の出力が得られる | 同じ形に `--test detect_bounds_cmd`、`cargo run --manifest-path measure/Cargo.toml --locked -p measure-detect --bin detect-bounds -- 9 1 0` | 9 passed; 0 failed; 0 ignored。出力は `{"detected":9,"missed":1,"indeterminate":0,"denominator":10,"lower":{"numerator":9,"denominator":10},"upper":{"numerator":9,"denominator":10},"threshold":{"numerator":9,"denominator":10},"label":"ok","reason":null}`、終了コード 0 |
| 6 | 依存 0・unsafe 0 | `measure/crates/detect/Cargo.toml` の `[dependencies]`、`grep -rc unsafe measure/crates/detect`、`make deps-check`、`git diff 1ad4cd1 origin/main -- measure/Cargo.lock` | `[dependencies]` は空。`unsafe` の件数はすべてのファイルで 0。`make deps-check` の終了コードは 0。`measure/Cargo.lock` の差分は `measure-detect`（version 0.0.0、依存なし）の package 1 ブロックの追加だけ（`1ad4cd1` は 1/4 の直前のコミット） |
| 7 | CI で流れる場所 | `make measure-verify`（CI の `deny` ジョブが呼ぶ） | `measure-detect` の lib 24 passed、`tests/counting.rs` 9 passed、`tests/detect_bounds_cmd.rs` 9 passed（いずれも 0 failed; 0 ignored）。終了コードは 0 |
| 8 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make measure-verify`・`make upper-layer-check`・`make build-profiles` | すべて終了コード 0 |

### 再実行の契機

- `measure/crates/detect/src/` の各ファイルまたは `measure/crates/detect/tests/` を変えたとき
- `measure/Cargo.toml` または `measure/Cargo.lock` を変えたとき
- `docs/spec` の gitlink を進めたとき

確かめた base は `cd05d0a`、`docs/spec` の gitlink は `d15921d`。
完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-1082）。
