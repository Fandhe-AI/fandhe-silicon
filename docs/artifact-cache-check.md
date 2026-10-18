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
| 照合・記録の型と関数 | 成果物のバイト列とハッシュを組にして照合する型・関数や、背後実装へ渡した成果物を記録する型は `crates/` に無い。cache の語が出るのは `execution.rs` のキーの型とその試験だけ | `git grep -n -i -c cache -- crates` |
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

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・関数・名前・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場は TASK-460 の決定の記録に従う（ポインタのみ）
- `core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

[artifact-input-types](artifact-input-types.md)・[sha256-hash](sha256-hash.md)・[test-frame](test-frame.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)
