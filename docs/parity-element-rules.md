# T-1 の要素ごとの規則の確かめ（TASK-975）の所在と本リポの現状

TASK-975 の確かめを進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-975。ポインタのみ）。内容・理由・経緯は本書に置かない。
対象の項目は TASK-975 の記録に従う（ポインタのみ）。`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-24 | `crates/core/src/parity/tolerance.rs`・`cpu_reference.rs`・`topk.rs` | [parity-tool](parity-tool.md) |
| TASK-25 | `crates/core/src/parity/special_values.rs` | [parity-non-finite](parity-non-finite.md) |
| TASK-23 | `crates/core/src/test_frame` | [test-frame](test-frame.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 試験用の部品の feature | `core` の `lib.rs` で `parity`・`test_frame`・`rerun` が `#[cfg(feature = "test-support")]` の下にある。`hash` は条件なし。`[features]` は `test-support = []` | `crates/core/src/lib.rs`、`crates/core/Cargo.toml` |
| 一致の道具のファイル | `crates/core/src/parity` に 5 ファイル（`cpu_reference`・`pixel`・`special_values`・`tolerance`・`topk`） | `ls crates/core/src/parity` |
| 一致の道具の統合テスト | `core` は `crates/core/tests/parity_*.rs` の 6 本。`contract` は `test_support_forwarding.rs`・`test_support_pixel.rs`・`test_support_special_values.rs` | `ls crates/core/tests crates/contract/tests` |
| `required-features` の件数 | 20 | `grep -c required-features crates/core/Cargo.toml` |
| 依存 | `core` の外部依存は 0。`#![forbid(unsafe_code)]` | `crates/core/Cargo.toml`、`crates/core/src/lib.rs` |
| 走る場所 | `make verify`（`--all-features`）と、`make build-profiles` の `test-support` を含む組。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |

## 既にある関連の部品

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。形の具体は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 値・規則を変える変更の扱いは [parity-tool](parity-tool.md) の「値・規則を変える変更の扱い」に従う
- 守る規約: [spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[coding-rust](../.claude/rules/coding-rust.md)・[dependency-policy](../.claude/rules/dependency-policy.md)

## 関連

[parity-tool](parity-tool.md)・[parity-non-finite](parity-non-finite.md)・[test-frame](test-frame.md)
