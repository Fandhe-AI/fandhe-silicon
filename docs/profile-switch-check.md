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
| (c) 組の関係 | `scripts/build-profiles.sh` の 注記（`# spec BUILD-` に 1〜3 の番号が付いたもの）で終わる行が番号ごとにちょうど 1 行。1 つ目の組は契約の crate の `default` と一致する。2 つ目は 1 つ目に `backend-` で始まる feature を 1 つ足したもので `graphics` を含まない。3 つ目は 1 つ目に `backend-` で始まる feature を 1 つ以上と `graphics` を足したもので、それ以外を含まない。3 つの組のどの名前も契約の crate の `[features]` に宣言されている | `grep -n '# spec BUILD-[123]$' scripts/build-profiles.sh` |
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

## プロファイルごとの試験の本数の検査（2/4）（TASK-227）

検査は `scripts/profile-test-count-check.sh`、回帰テストは `scripts/test-profile-test-count-check.sh`。
契約の crate の試験 `crates/contract/tests/stub_device_profiles.rs`（TASK-137）を、プロファイルの 3 つの組に `test-support` を足した組で流して本数を照らす。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 組の取り方 | `scripts/build-profiles.sh` の 注記（`# spec BUILD-` に 1〜3 の番号が付いたもの）で終わる行が番号ごとにちょうど 1 行。その行の最初の引用符の中身を組とし、スクリプトの中で末尾に `,test-support` を足す（注記の行そのものには足さない）。`build-profiles.sh` は読むだけで実行しない。名前は順に `CpuOnly`・`ComputeOnly`・`Graphics` | `grep -n '# spec BUILD-[123]$' scripts/build-profiles.sh` |
| 流すコマンド | 組ごとに `cargo test -p fandhe-silicon-contract --no-default-features --features "<組>,test-support" --test stub_device_profiles`（`-q` なし）。`FANDHE_SILICON_TEST_RECORD_DIR` は外す | `grep -n 'cargo test' scripts/profile-test-count-check.sh` |
| 期待の定数 | 通った本数は `CpuOnly` が 6・`ComputeOnly` が 6・`Graphics` が 9。`graphics_` で始まる試験の本数は 0・0・3。スクリプトの `expected_passed`・`expected_graphics` の 1 か所にある。`stub_device_profiles.rs` の `#[test]` を足す・消すときは、同じ変更で定数も直す | `grep -n -A8 '^expected_' scripts/profile-test-count-check.sh` |
| 合格の条件 | cargo の終了コードが 0。`test result:` の行がちょうど 1 行で `ok. N passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` の形、N が期待どおり。`test <名前> ... ok` の行が N 本で、`ok` でない `test` の行が 0。`graphics_` で始まる行が期待どおり。どれか 1 つでも外れた組は標準エラーに `FAILED: profile=<名前> reason=<語>` を出し、3 組とも判定してから終了コード 1 | `bash scripts/test-profile-test-count-check.sh` |
| 出力の行 | 組ごとに標準出力へ `record: profile=<名前> features=<組>,test-support passed=<N> graphics=<数>`。全組合格なら最後に `profile-test-count-check: all profiles passed` | 下の実行の記録 |
| 終了コード | 0 は 3 組とも合格、1 は不合格の組がある、2 は誤用・cargo が無い・読めない入力（注記の行が 0 行か 2 行以上・引用符が無い・組に使えない文字がある・`--build-profiles` のパスが読めない） | `bash scripts/test-profile-test-count-check.sh` |
| 引数 | 既定は引数なし。`--build-profiles <path>` だけが読む `build-profiles.sh` の差し替え（回帰テスト用） | `grep -n -e '--build-profiles' scripts/profile-test-count-check.sh` |

### 対応するテスト

`make test-profile-test-count-check` が、合成の `build-profiles.sh`（ダミーの feature 名）と PATH の先頭に置く偽の cargo で次を確かめる。実物の cargo は流さない。

| 場合 | 内容 |
| ---- | ---- |
| `good`・`good-args` | 6・6・9 と `graphics_` の 3 本で終了コード 0、`record:` の 3 行と最終行。偽の cargo に渡った引数が上の形で `-q` が無い |
| `record-dir-unset` | 環境に置いた `FANDHE_SILICON_TEST_RECORD_DIR` が cargo に渡らない |
| `count-differs`・`count-low` | 本数が期待と違う |
| `failed-nonzero`・`ignored-nonzero`・`filtered-out-nonzero` | `failed`・`ignored`・`filtered out` が 0 でない |
| `result-zero-lines`・`result-two-lines` | `test result:` の行が 0 行・2 行 |
| `cargo-exit` | cargo の終了コードが 0 でない |
| `cpu-has-graphics`・`graphics-two` | CpuOnly に `graphics_` が出る、Graphics の `graphics_` が 2 本 |
| `non-ok-line` | `ok` でない `test` の行がある |
| `note-zero`・`note-two`・`no-quote`・`bad-char` | 注記の行が 0 行・2 行、引用符が無い、組に不正な文字。cargo は呼ばれない |
| `unknown-arg`・`missing-value`・`missing-path`・`no-cargo` | 未知の引数、`--build-profiles` の値なし・パスが無い、cargo が PATH に無い |

```text
test-profile-test-count-check: 27 passed, 0 failed
```

### 手元の実行の記録

`make profile-test-count-check`（終了コード 0）の標準出力:

```text
record: profile=CpuOnly features=contract-core,cpu-isa,cpu-ops,test-support passed=6 graphics=0
record: profile=ComputeOnly features=contract-core,cpu-isa,cpu-ops,backend-metal,test-support passed=6 graphics=0
record: profile=Graphics features=contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics,test-support passed=9 graphics=3
profile-test-count-check: all profiles passed
```

## CI への組み込み（3/4）（TASK-227）

足したのは既存の `verify`・`verify-macos` ジョブの step で、ジョブ・make ターゲット・スクリプトは足していない（`ci-complete` の `needs` と check 名は変わらない）。どの step にも `continue-on-error`・`|| true`・`if:` は付けていない。

| ジョブ（check 名） | step（make ターゲット） | 流すもの |
| ------------------ | ----------------------- | -------- |
| `verify`（`make verify (fmt + clippy + test)`） | `make profile-test-count-check` | 3 組の `cargo test ... --test stub_device_profiles` と本数の照合（`make build-profiles` の後） |
| `verify` | `make test-profile-test-count-check` | 回帰テスト（合成の入力と偽の cargo） |
| `verify-macos`（`make verify + build-profiles (macos-latest)`） | `make profile-test-count-check` | 同じ照合を macOS で（ComputeOnly の組が Metal の依存を実際にビルドする） |

確かめ方は `grep -n 'profile-test-count-check' .github/workflows/ci.yml` と `make ci-frame-check`。

手元（Linux）の実行の記録（作業のもとの HEAD は `ef5fcfc`）。`make profile-test-count-check` の標準出力:

```text
record: profile=CpuOnly features=contract-core,cpu-isa,cpu-ops,test-support passed=6 graphics=0
record: profile=ComputeOnly features=contract-core,cpu-isa,cpu-ops,backend-metal,test-support passed=6 graphics=0
record: profile=Graphics features=contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics,test-support passed=9 graphics=3
profile-test-count-check: all profiles passed
```

## 完了の条件の確かめ（4/4）（TASK-227）

時点のコミットは `f28db54`。CI の run は 3/4 のマージの main の push の run（ID `41405921122`、`headSha` は `c2cbc0e`）。

| 確かめ | コマンド | 結果 |
| ------ | -------- | ---- |
| (1) 構成の試験 | `cargo test -p fandhe-silicon-contract --test profile_feature_sets` | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| (2) 既存の feature の試験（TASK-226） | `cargo test -p fandhe-silicon-contract --test feature_default_scope` | `test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| (3) 本数の検査 | `make profile-test-count-check` | 下の「(3) の出力」 |
| (4) feature ゲートの doctest（TASK-137・TASK-837） | 3 組それぞれで `cargo test -p fandhe-silicon-contract --no-default-features --features "<組>,test-support" --doc` のうち `GraphicsGate` を含む行 | 下の「(4) の出力」 |
| (5) CI の run | `gh run view 41405921122 --json conclusion,headSha,jobs` と、ログの `record:` の行と最終行 | run の `conclusion` は `success`。`make verify (fmt + clippy + test)` と `make verify + build-profiles (macos-latest)` の `conclusion` はどちらも `success`。ログの行は下の「(5) の出力」 |
| (6) 1/4 の試験・組の注記・`Cargo.toml` の差 | `git diff --stat 2ea8f74..HEAD -- crates/contract/tests/stub_device_profiles.rs scripts/build-profiles.sh crates/contract/Cargo.toml`（`2ea8f74` は 1/4 の PR のコミットの親） | （出力なし） |
| (7) 依存と unsafe の差 | `git diff --stat 2ea8f74..HEAD -- Cargo.lock deps-approved.toml`、`git diff 2ea8f74..HEAD -- crates/ \| grep -c '^+.*unsafe'` | 前者は（出力なし）、後者は `0` |

(3) の出力:

```text
record: profile=CpuOnly features=contract-core,cpu-isa,cpu-ops,test-support passed=6 graphics=0
record: profile=ComputeOnly features=contract-core,cpu-isa,cpu-ops,backend-metal,test-support passed=6 graphics=0
record: profile=Graphics features=contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics,test-support passed=9 graphics=3
profile-test-count-check: all profiles passed
```

(4) の出力（組は `scripts/build-profiles.sh` の 3 組の順）:

```text
## contract-core,cpu-isa,cpu-ops
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::GraphicsGate (line 99) - compile fail ... ok
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::DeviceGraphicsGate (line 109) - compile fail ... ok
## contract-core,cpu-isa,cpu-ops,backend-metal
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::GraphicsGate (line 99) - compile fail ... ok
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::DeviceGraphicsGate (line 109) - compile fail ... ok
## contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::DeviceGraphicsGate (line 109) ... ok
test crates/contract/src/cfg_gate_doctest.rs - cfg_gate_doctest::GraphicsGate (line 99) ... ok
```

(5) の出力（ログから `record:` の行と最終行だけ。上が `make verify (fmt + clippy + test)`、下が `make verify + build-profiles (macos-latest)`）:

```text
record: profile=CpuOnly features=contract-core,cpu-isa,cpu-ops,test-support passed=6 graphics=0
record: profile=ComputeOnly features=contract-core,cpu-isa,cpu-ops,backend-metal,test-support passed=6 graphics=0
record: profile=Graphics features=contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics,test-support passed=9 graphics=3
profile-test-count-check: all profiles passed
record: profile=CpuOnly features=contract-core,cpu-isa,cpu-ops,test-support passed=6 graphics=0
record: profile=ComputeOnly features=contract-core,cpu-isa,cpu-ops,backend-metal,test-support passed=6 graphics=0
record: profile=Graphics features=contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics,test-support passed=9 graphics=3
profile-test-count-check: all profiles passed
```
