# PTX の命令の検索の道具の所在（TASK-100）

PTX のテキストを関数ごとに分け、命令の opcode に名前つきのパターンを当てて件数を数える、試験用の純粋関数の所在と、本リポの現状の事実をまとめる。
決定の記録は private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-100（親の Issue は #389）。

## 本リポの現状の事実

記録時点の commit は `a2c63a0`（`origin/main`）。

| 項目 | 事実 | コマンド |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/ptx_search.rs` | `ls crates/core/src/ptx_search.rs` |
| 有効になる条件 | feature `test-support` の裏にだけ公開される | `grep -n -B1 'pub mod ptx_search' crates/core/src/lib.rs` |
| 依存 | core の中の `std` だけ | `grep -n '^use' crates/core/src/ptx_search.rs` |
| unsafe | なし（core は `#![forbid(unsafe_code)]`） | `grep -n forbid crates/core/src/lib.rs` |

## 検索の関数（1/4）

### 公開の項目

| 項目 | 内容 |
| ---- | ---- |
| `OpcodeRule` | opcode に当てる規則（`root`・`all_parts`・`any_parts`） |
| `InstructionPattern` | 名前（`id`）と規則の列。どれか 1 つの規則に当たる命令を 1 件と数える |
| `F16_INSTRUCTIONS` / `SUBGROUP_INSTRUCTIONS` / `F32_ATOMIC_ADD_INSTRUCTIONS` / `MATRIX_MMA_INSTRUCTIONS` | 組み込みのパターン 4 つ |
| `BUILTIN_PATTERNS` | 上の 4 つを並べた配列 |
| `UnitKind` / `PatternCount` / `UnitReport` / `SearchReport` | 結果の型。`SearchReport::total(id)` は全関数の和（無い id・あふれは `None`） |
| `PtxSearchError` | `Err` の種類。`as_str()` と `Display` は固定の英語の文言 |
| `search` | `search(ptx, patterns)` が `Result<SearchReport, PtxSearchError>` を返す |

### 読み方

- `//` から行末まで、`/* … */`、`"…"` を字ごとに空白へ置き換える（改行は残す）。
- 関数の外は、`;`・`{`・`}` までを 1 つの文として読み、`.entry`・`.func` の語を探す。`{` が続けば対応する `}` までを本文とし、`;` が続けば宣言として名前だけ控える。
- 本文は `;` で文に分け、文の頭の `{`・`}`・ラベル・述語（`@…`）・`.loc`・`.file`（行末まで）を外す。残りの最初の語が opcode で、空または `.` で始まる語は命令に数えない。
- opcode は `.` で分け、最初を根、残りを部分とする。部分の一致は語ごとの完全一致。

### 組み込みのパターン

| id | 当たる例 | 当たらない例 |
| -- | -------- | ------------ |
| `f16` | `fma.rn.f16`・`cvt.f32.f16`・`add.f16x2` | `cvt.rn.bf16.f32` |
| `subgroup` | `shfl.sync.bfly.b32`・`redux.sync.add.u32`・`vote.sync.ballot.b32` | `red.global.add.f32` |
| `f32-atomic-add` | `atom.global.add.f32`・`red.global.add.f32` | `atom.global.add.u32`・`redux.sync.add.u32` |
| `matrix-mma` | `mma.sync.aligned.m16n8k16.row.col.f32.f16.f16.f32`・`wmma.mma.sync.aligned.row.row.m16n16k16.f32.f32` | `wmma.load.a.sync.aligned.row.m16n16k16.global.f16` |

### `Err` になる入力

| 入力 | 種類 |
| ---- | ---- |
| パターンの列が空 | `EmptyPatterns` |
| パターンの id の重複 | `DuplicatePatternId` |
| 閉じない `/*` | `UnterminatedBlockComment` |
| 閉じない `"` | `UnterminatedString` |
| 閉じない `{`、関数の外の余分な `}`、関数の頭の後に `{` も `;` も無い入力 | `UnbalancedBraces` |
| 名前の無い `.entry`・`.func` | `MissingFunctionName` |
| 本文のある関数の名前の重複 | `DuplicateFunctionName` |
| 本文のある `.entry` が無い入力（空を含む） | `NoEntry` |
| 件数の加算のあふれ | `CountOverflow` |

### 確かめ方

```text
cargo test -p fandhe-silicon-core --features test-support --lib ptx_search
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 検索の関数 | #3542 |
