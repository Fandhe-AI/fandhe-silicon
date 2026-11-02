# プロファイルの切替の検査の所在と本リポの現状の事実（TASK-227）

本リポのコードと公開テストから観測できる事実だけを記録する。
記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-227。ポインタのみ）。本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。
Rust の規約は [coding-rust](../.claude/rules/coding-rust.md) に従う。

## プロファイルの feature と crate の構成の試験（1/4）（TASK-227）

試験は `crates/contract/tests/profile_feature_sets.rs` にある。条件なしの統合テストで、`Cargo.toml` に `[[test]]` の項目は無く、どの feature の組で流しても結果は同じ。
外部 crate もコマンドの起動も使わず、manifest・スクリプト・`lib.rs` の文字列だけを読む。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| (a) crate の構成 | ルートの `members` は 5 件（`crates/core`・`crates/contract`・`crates/exec`・`crates/upper`・`crates/wasm-abi`）で、1 行の配列。各 member の `[package] name` に `cpu-only`・`compute-only`・`cpuonly`・`computeonly`・`profile`（`-` と `_`・大文字小文字を問わない）を含むものは無い。`[[test]]` の `name` は読まない | `grep -n '^members' Cargo.toml` |
| (b) feature の名前 | 5 つの member の `[features]` の名前に (a) と同じ語を含むものは無い。`[features]` を持たない member は空として読む。`graphics` は語に入れない | `grep -n '^\[features\]' crates/*/Cargo.toml` |
| (c) 組の関係 | `scripts/build-profiles.sh` の `# spec BUILD-1`・`# spec BUILD-2`・`# spec BUILD-3` で終わる行がそれぞれちょうど 1 行。1 つ目の組は契約の crate の `default` と一致する。2 つ目は 1 つ目に `backend-` で始まる feature を 1 つ足したもので `graphics` を含まない。3 つ目は 1 つ目に `backend-` で始まる feature を 1 つ以上と `graphics` を足したもので、それ以外を含まない。3 つの組のどの名前も契約の crate の `[features]` に宣言されている | `grep -n '# spec BUILD-[123]$' scripts/build-profiles.sh` |
| (d) モジュールの単位 | 契約の crate の `src/lib.rs` の `pub mod backend_metal;`・`pub mod backend_vulkan;`・`pub mod backend_cuda;` はそれぞれ 1 行で、直前の連続した属性の行に対応する `feature = "backend-<名前>"` を含む `#[cfg(...)]` がある。共通の crate の `src/lib.rs` の `pub mod graphics;` の直前に `#[cfg(feature = "graphics")]` がある | `grep -n 'pub mod backend_' crates/contract/src/lib.rs` と `grep -n 'pub mod graphics;' crates/core/src/lib.rs` |
| 陽性対照 | 実物を 1 か所だけ変えた入力で違反が出る。変更後の入力が元と同じなら試験自体が落ちる。変えるのは、`members` への追加・削除・glob 化、package 名への語の混入、各 member の `[features]` への `profile-cpu-only` の追加（節の無い member は節ごと追加）、注記の組への `graphics` の追加・`backend-` の 2 つ化・`graphics` の除去・宣言の無い名前の追加・注記の行の重複、`#[cfg(...)]` の行の削除、契約の crate の `[features]` の欠落 | 下の「対応するテスト」 |
| `default` などの試験 | `default` の完全一致、既定で無効な feature が `default` から届かないこと、本番の組の写しとスクリプトの一致は `crates/contract/tests/feature_default_scope.rs`（TASK-226、[contract-feature-check](contract-feature-check.md)） | `cargo test -p fandhe-silicon-contract --test feature_default_scope` |

### 対応するテスト

| 関数 | 内容 |
| ---- | ---- |
| `real_inputs_have_no_problems` | 実物の入力で違反が 0 件 |
| `profile_words_exclude_graphics_and_match_normalized_names` | 語の一覧と名前の正規化 |
| `reader_ignores_test_name_and_follows_sections` | `[[test]]` の `name` を読まず、`[target...]` を挟んでも `[features]` を読む |
| `reader_rejects_unreadable_shapes` | 複数行の配列・glob・`members` の欠落を読み取りの失敗にする |
| `control_members_added_removed_or_glob` | `members` の追加・削除・glob 化 |
| `control_package_name_with_profile_word` | package 名への語の混入 |
| `control_profile_feature_in_any_member` | 5 つの member それぞれへの語を含む feature の追加 |
| `control_build_set_notes` | 注記の組の 5 通りの変更 |
| `control_missing_cfg_attributes` | 背後実装 3 つと `graphics` の `#[cfg(...)]` の削除 |
| `control_missing_contract_features_section` | 契約の crate の `[features]` の欠落 |

### 確かめ方

```text
cargo test -p fandhe-silicon-contract --test profile_feature_sets
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

終了コードは 0。`--no-default-features --features contract-core,cpu-isa,cpu-ops`・`--all-features` など、ほかの組でも同じ件数で通る。

### 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | プロファイルの feature と crate の構成の試験 | #7712 |
| 2/4 | プロファイルごとの試験の本数の検査 | #7713 |
| 3/4 | 試験の本数の検査の CI への組み込み | #7714 |
| 4/4 | 完了の条件の確かめ | #7715 |

親の Issue は #1149。
