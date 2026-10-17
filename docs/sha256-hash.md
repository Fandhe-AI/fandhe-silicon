# SHA-256 の関数（TASK-471）の所在と現状の事実

本リポのコードと公開テストから説明できる振る舞いを記録する。記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-471・`04-behavior/` の ART-28。ポインタのみ）。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 置き場 | `crates/core/src/hash.rs` | `crates/core/src/lib.rs` |
| feature | 条件なし（feature の裏ではない） | `crates/core/Cargo.toml` |
| 依存 | 外部依存 0。`unsafe` を使わない（`core` は `#![forbid(unsafe_code)]`） | `Cargo.lock` |
| 用途 | 同一性の識別。秘密の比較・MAC・パスワードの用途は対象外で、定数時間の性質は保証しない | `crates/core/src/hash.rs` の `//!` |

## 振る舞い

公開名は `sha256_bytes`（32 バイトの要約を返す）と `sha256_u64`（要約の先頭 8 バイトを big-endian で読んだ値）。

## 対応するテスト

| 振る舞い | テスト |
| -------- | ------ |
| 固定の既知の値（空・1 バイト・NIST の例・ブロック境界の長さ・長い入力）と一致 | `crates/core/tests/hash_known_values.rs` の `known_values_match` |
| `shasum -a 256` の出力と一致（小さい入力・ブロック境界・任意のバイト列の掃引・長い入力） | `crates/core/tests/hash_shasum.rs` の `small_inputs_match_shasum`・`block_boundaries_match_shasum`・`arbitrary_byte_sweep_matches_shasum`・`long_inputs_match_shasum` |

`hash_shasum.rs` は `shasum` を PATH から引いて直接起動し、入力を stdin で渡す。`shasum` が無い・異常終了した・出力の形が違うときは、飛ばさずに失敗する。

## 確かめ方

```bash
cargo test -p fandhe-silicon-core --test hash_known_values --test hash_shasum
```

`make verify` の `cargo test` でも走る。`shasum`（perl に付属）が PATH に要る。
