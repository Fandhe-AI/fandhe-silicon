# 成果物キャッシュの照合（TASK-483）の所在と本リポの現状

TASK-483 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-483。対象の項目は `docs/spec/04-behavior/api-shader-kernel-artifacts.md` の ART-52。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-470 | `crates/core/src/execution.rs`（`PipelineInput`・`CacheKey` ほか） | [artifact-input-types](artifact-input-types.md) |
| TASK-471 | `crates/core/src/hash.rs`（`sha256_bytes`・`sha256_u64`） | [sha256-hash](sha256-hash.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-460 | —（本リポに対応する物は無い。private spec 側の記録に従う〔ポインタのみ〕） | — |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| `CacheKey` の形 | 公開フィールド 8 つの構造体。`Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash` を持ち、`#[non_exhaustive]` ではない。値を保持するだけで、キーの組み立てやハッシュの計算はしない | `crates/core/src/execution.rs`、`crates/core/tests/execution_cache_key.rs` |
| `hash` | 公開の関数は `sha256_bytes`（32 バイト）と `sha256_u64` の 2 つ。feature の条件なしで公開 | `grep -n 'pub fn' crates/core/src/hash.rs` |
| `execution` | 型が主で、公開の関数は `DType::byte_size` だけ（`pub const fn`） | `grep -n 'pub const fn\|pub fn' crates/core/src/execution.rs` |
| feature の配置 | `hash`・`execution` は条件なし。`parity`・`test_frame`・`rerun` は `test-support` の裏 | `crates/core/src/lib.rs` |
| 照合・記録の型と関数 | エントリの型 `CacheEntry`（2/8）と、読み込み時の照合（`RejectReason`・`LoadOutcome`・`verify_entry`・入れ物 `CacheStore`。3/8）は `artifact_cache` にある。背後実装へ渡すバイト列の SHA-256 を記録する台帳 `Ledger`（4/8）も `artifact_cache` にある | `git grep -n -i -c cache -- crates` |
| 依存・unsafe | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。`deps-approved.toml` の承認は 0 件 | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`deps-approved.toml` |
| `exec` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec` |
| 依存の向き | contract → core、exec → core、upper → core・exec。`wasm/crates/glue` が `contract` に path で依存する（`core` に足すものは std だけで wasm32 でも通る必要がある） | 各 `Cargo.toml`、`wasm/crates/glue/Cargo.toml` |
| 走る場所 | `core` の試験は `make verify`（`--all-features`）で走る。GPU の実機は要らない | `scripts/verify.sh` |
| 本書の対象の言及 | TASK-483 の言及は本書、`CLAUDE.md`、`README.md` にある。TASK-460 の言及は本書だけ | `git grep -n -e TASK-483 -e TASK-460 -- . ':!docs/spec'` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/8 | #4907 | 現状と前提の確認（本書） |
| 2/8〜4/8 | #4908・#4909・#4910 | 実装 |
| 5/8〜6/8 | #4911・#4912 | テスト |
| 7/8 | #4913 | 確かめ |
| 8/8 | #4914 | 完了の条件の確かめの記録 |

## 現状の形（2/8）

名前は仮。`crates/core/src/artifact_cache.rs`（feature の条件なし）に次を置く。

| 項目 | 事実 |
| ---- | ---- |
| 型 | `CacheEntry`。キー（`CacheKey`）・成果物のバイト列・そのバイト列の SHA-256 を private フィールドで持つ。`Debug`・`Clone`・`PartialEq`・`Eq` |
| 作り方 | `new(key, bytes)` は `hash::sha256_bytes` で SHA-256 を計算する。`from_parts(key, bytes, sha256)` は渡された値をそのまま保持し、再計算も照合もしない（バイト列と一致しない SHA-256 のエントリも表せる） |
| 読み出し | `key()`・`bytes()`・`sha256()`、分解は `into_parts()` |
| 持たないもの | 読み込み時の照合・拒否の理由・台帳 |

対応するテスト: `crates/core/tests/artifact_cache_entry.rs`。確かめ方: `cargo test -p fandhe-silicon-core --test artifact_cache_entry`。

## 現状の形（3/8）

名前は仮。同じ `crates/core/src/artifact_cache.rs`（feature の条件なし）に次を足した。

| 項目 | 事実 |
| ---- | ---- |
| 拒否の理由 | `RejectReason`（`KeyMismatch`・`HashMismatch`）。`#[non_exhaustive]` |
| 読み込みの結果 | `LoadOutcome<'a>`（`Hit(&CacheEntry)`・`Rejected(RejectReason)`）。`#[non_exhaustive]`。`Hit` は借用でバイト列を複製しない |
| 照合 | `verify_entry(&entry, &requested)`。判定の順序は固定: キーが違えば `KeyMismatch`（ハッシュは計算しない）、次にバイト列から SHA-256 を計算し直して保持する値と違えば `HashMismatch`、どちらも合えば `Ok`。計算量はバイト列の長さに比例 |
| 入れ物 | `CacheStore`（`Vec` の線形探索）。`new`・`insert`・`load`。`insert` は照合せず、同じキーがあれば置き換えて前のものを返す。`load` は `&self` で、キーが合うエントリが無ければ `Rejected(KeyMismatch)`、あれば `verify_entry` を通す。拒否したエントリも消さない |
| 持たないもの | 拒否の数え上げ・上限や追い出し（台帳は 4/8 の `Ledger`。入れ物は持たない） |

対応するテスト: `crates/core/tests/artifact_cache_load.rs` と `artifact_cache.rs` 内の網羅の試験。確かめ方: `cargo test -p fandhe-silicon-core --test artifact_cache_load`。

## 現状の形（4/8）

名前は仮。同じ `crates/core/src/artifact_cache.rs`（feature の条件なし）に次を足した。

| 項目 | 事実 |
| ---- | ---- |
| 型 | `Ledger`。SHA-256（32 バイト）の記録を private の `Vec` で持つ。`Debug`・`Clone`・`Default`・`PartialEq`・`Eq` |
| 作り方 | `new()`（`Default` と同じく空） |
| 記録 | `record(&mut self, bytes)` は渡されたバイト列から `hash::sha256_bytes` で SHA-256 を計算して末尾に足し、その値を返す。`CacheEntry::sha256()` の保持値は写さない（保持値と一致しないエントリでも、渡したバイト列の値が残る）。重複は除かない。計算量はバイト列の長さに比例 |
| 問合せ | `len()`・`is_empty()`・`records()`（記録順）・`contains(&sha256)`（線形探索） |
| 持たないもの | バイト列そのもの・背後実装への受け渡し・保存先・上限や削除。`CacheStore` は台帳を持たない |

対応するテスト: `crates/core/tests/artifact_cache_ledger.rs`。確かめ方: `cargo test -p fandhe-silicon-core --test artifact_cache_ledger`。

## 試験（5/8）

本体のコード（`src/`）は変えず、試験だけを足した。入れ物・照合・台帳をつないだ流れで、バイト列が壊れたエントリ（SHA-256 の不一致）の拒否と作り直しを確かめる。

| 確かめること | 事実 |
| ------------ | ---- |
| 件数の比較 | 拒否の前・拒否の後・作り直しの後で台帳の件数を比べる。拒否では増えず、作り直したバイト列を通したときに 1 件増え、最後の記録がその SHA-256 と等しい |
| 全記録との照合 | 壊れたバイト列の SHA-256 と一致する記録を、台帳の全記録を走査して数えると 0 件。作り直した側は 1 件 |
| 範囲 | 反転する位置（先頭・中央・末尾）、拒否の繰り返し、同じ入れ物の中のほかのエントリへの影響なし |
| 陽性対照 | 照合を通さず壊れたバイト列を記録した台帳では、同じ数え方が 1 件を返す |
| 件数だけでは足りない対照 | 件数が等しい 2 つの台帳でも、壊れたバイト列を記録した側は全記録との照合で 1 件になる |

対応するテスト: `crates/core/tests/artifact_cache_ledger_flow.rs`。確かめ方: `cargo test -p fandhe-silicon-core --test artifact_cache_ledger_flow`。

## 試験（6/8）

本体のコード（`src/`）は変えず、試験だけを足した。キーの版の欄（`compiler_version`・`target`・`driver_version`）の 1 つだけが違い、バイト列は同じエントリについて、入れ物・照合・台帳をつないだ流れで確かめる。

| 確かめること | 事実 |
| ------------ | ---- |
| 拒否 | 版の欄を 1 つだけ変えたキーで読み込むと `KeyMismatch`。台帳の件数は増えない |
| 作り直し | 同じバイト列を新しいキーで作ったエントリの SHA-256 は、古いエントリの保持値と等しい。キーが違うので `insert` は前のエントリを返さず、古いエントリも残る |
| 全記録との照合 | 作り直したバイト列を通すと台帳は 1 件増え、古いエントリの保持値と一致する記録は 1 件。5/8 の「0 件」の確かめは当てはまらない |
| 読み出し | 古いキーでも新しいキーでも、返るバイト列は同じ |
| 対照 | バイト列が壊れた流れ（5/8）では壊れた側と作り直した側の SHA-256 が違い、キーの版の書き換えでは等しい |
| 迂回との対照 | 照合を通さず古いバイト列を記録した台帳と、拒否 → 作り直し → 記録の台帳は、記録が等しい。区別は読み込みの `KeyMismatch` で付く |
| 順序 | キーの書き換えとバイト列の破損が重なったエントリは、キーの判定が先で `KeyMismatch`。台帳は変わらない |

対応するテスト: `crates/core/tests/artifact_cache_key_rewrite.rs`。確かめ方: `cargo test -p fandhe-silicon-core --test artifact_cache_key_rewrite`。

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・関数・名前・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場は TASK-460 の決定の記録に従う（ポインタのみ）
- `core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[artifact-input-types](artifact-input-types.md)・[sha256-hash](sha256-hash.md)・[test-frame](test-frame.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
