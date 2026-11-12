# Metal の成果物の検査の口（TASK-492）の所在と本リポの現状

TASK-492 の作業のうち、feature `artifact-inspection` の裏にある成果物の検査の口（core の trait と、Metal の背後実装の上書き）の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-492。ポインタのみ）。private spec 側の記録に従う（ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| core の feature | `crates/core/Cargo.toml` の `artifact-inspection = []`。`[[test]]` の `device_artifact_hooks` が `required-features = ["artifact-inspection"]` を持つ | `grep -n 'artifact-inspection' crates/core/Cargo.toml` |
| `artifact` module と trait | `crates/core/src/lib.rs` の `pub mod artifact;`（feature の裏）。`crates/core/src/artifact.rs` の trait `ArtifactHooks` は 3 メソッドで、既定の本体を持たない | `grep -n 'artifact' crates/core/src/lib.rs`、`grep -n 'fn ' crates/core/src/artifact.rs` |
| `Device::artifact_hooks` | `Option<&dyn ArtifactHooks>` を返す。既定の本体は `None`。feature の裏にある | `grep -n 'fn artifact_hooks' -B2 crates/core/src/device.rs` |
| 契約の crate の feature | `artifact-inspection = ["backend-metal", "fandhe-silicon-core/artifact-inspection", "objc2-metal?/MTLBinaryArchive"]` | `grep -n 'artifact-inspection =' crates/contract/Cargo.toml` |
| Metal の上書き | `imp`（macOS だけ）の中の `MetalDevice::artifact_hooks` が `Some(self)` を返す | `grep -n 'fn artifact_hooks' crates/contract/src/backend_metal.rs` |
| 3 メソッドの実装 | `imp` の中の `impl ArtifactHooks for MetalDevice`。`compile_and_write_binary_archive`・`create_pipeline_from_archive_path`・`create_pipeline_checked_from_metallib` | `grep -n 'impl ArtifactHooks for MetalDevice\|fn compile_and_write_binary_archive\|fn create_pipeline_from_archive_path\|fn create_pipeline_checked_from_metallib' crates/contract/src/backend_metal.rs` |
| パスとファイルの部品 | `mod imp` の外の `ARTIFACT_READ_LIMIT`・`artifact_path_str`・`read_written_artifact`（feature の裏。どの OS でもビルドされる） | `grep -n 'const ARTIFACT_READ_LIMIT\|fn artifact_path_str\|fn read_written_artifact' crates/contract/src/backend_metal.rs` |
| core の試験 | `crates/core/tests/device_artifact_hooks.rs`（3 本。device にもファイルにも触れない） | `grep -n 'fn ' crates/core/tests/device_artifact_hooks.rs` |
| Linux の試験の module | `artifact_path_tests`（`#[cfg(all(test, feature = "artifact-inspection"))]`。8 本） | `grep -n 'mod artifact_path_tests' crates/contract/src/backend_metal.rs` |
| device の要る試験の module | `imp::device_tests::artifact_hooks_reach`・`imp::device_tests::dispatch::metallib`・その下の `parity_gemm_relu`・`parity_topk`・`parity_coopmat`（補助は `parity_common`） | `grep -n 'mod artifact_hooks_reach\|mod metallib\|mod parity_' crates/contract/src/backend_metal.rs` |
| 依存と cfg | `objc2-metal` の feature `MTLBinaryArchive` が `artifact-inspection` の裏で加わる。crate と版は変わらない。Metal の上書きは `imp`（macOS だけ）の中 | `grep -n 'objc2-metal' crates/contract/Cargo.toml` |

## 本リポの現状の事実

- `artifact-inspection` は `default` にも本番の組にも入らない。
- feature を付けないビルドでは `artifact` module も `Device::artifact_hooks` も無い。
- feature を付けたビルドで、上書きしない device（試験用の device を含む）の `artifact_hooks()` は `None` を返す。
- Metal の device は `Some` を返す。
- `Cargo.lock`・`deps-approved.toml`・`scripts/`・`.github/` は、1/8 の親のコミット `c2abac4` から変わっていない。
- 1/8 の親から今までの `crates/` の差分に、`unsafe` を含む追加行は 0 件。

## 共通の crate の口（1/8）

- `ArtifactHooks` の 3 メソッドは、`compile_and_write_binary_archive`（ソース・入口名・数学モード・パスを受け、書き出したバイト列を返す）、`create_pipeline_from_archive_path`（パスの binary archive を使ってパイプラインを作る。`fail_on_archive_miss` が `true` なら archive に無いときに失敗する）、`create_pipeline_checked_from_metallib`（metallib のファイルと入口名と宣言の列を受け、パイプラインを作る）。
- `Device::artifact_hooks` の既定は `None`。
- `crates/core/tests/device_artifact_hooks.rs` は次の 3 本。

| 試験 | 確かめること |
| ---- | ------------ |
| `default_artifact_hooks_is_none_through_dyn_device` | 上書きしない device は `dyn Device` 越しに `None` を返す |
| `overridden_artifact_hooks_return_dummy_values` | 上書きした device の 3 メソッドが、ダミーの返す値をそのまま返す |
| `overriding_device_can_still_report_none` | 上書きした device も `None` を返せる |

```text
cargo test -p fandhe-silicon-core --features artifact-inspection --test device_artifact_hooks
```

## パスとファイルの部品（2/8）

- `artifact_path_str` は絶対・UTF-8・空でないパスを `&str` にして返す。空・相対・UTF-8 でないパスは、どれも `InvalidArgument`。
- `read_written_artifact` は通常のファイル（シンボリックリンクでない）で、長さが 1 以上 `limit` 以下のときだけ中身を返す。外れれば `ArtifactRejected { kind: "binary-archive" }`。
- `ARTIFACT_READ_LIMIT` は `1 << 28`。

| 試験（`artifact_path_tests`） | 入力と期待 |
| ------------------------------ | ---------- |
| `path_str_accepts_absolute_utf8` | 絶対・UTF-8 のパスは通る |
| `path_str_rejects_relative_and_empty` | 相対と空は `InvalidArgument` |
| `path_str_rejects_non_utf8` | UTF-8 でないパスは `InvalidArgument` |
| `read_returns_bytes_with_default_limit` | 既定の上限でファイルの中身が返る |
| `read_enforces_limit_boundary` | 長さが上限ちょうどは通り、上限を 1 超えると拒否される |
| `read_rejects_empty_missing_and_directory` | 空のファイル・存在しないパス・ディレクトリは拒否される |
| `read_open_does_not_follow_symlink` | 読み取りで開くときにシンボリックリンクをたどらないフラグを付けると、リンクは開けず、リンク先の通常のファイルは開ける |
| `read_rejects_symlink` | シンボリックリンクは拒否され、リンク先の通常のファイルは中身が返る |

```text
cargo test -p fandhe-silicon-contract --features artifact-inspection --lib artifact_path_tests
```

## Metal の上書き（3/8）

- `MetalDevice::artifact_hooks` は feature `artifact-inspection` のビルドで `Some(self)` を返す。
- `compile_and_write_binary_archive` はソースを数学モードつきでコンパイルし、絶対パスへ binary archive を書き出して、書き出したバイト列を返す。パイプライン状態は作らず、表にも置かない。失敗は `ArtifactRejected { kind: "binary-archive" }`。
- `create_pipeline_from_archive_path` は archive から作ったパイプラインを、束縛の無い layout で表に置く。
- `create_pipeline_checked_from_metallib` は metallib のファイルから宣言の slot の layout でパイプラインを作り、表に置く。`reflect_function` を呼ぶ。
- 3 メソッドとも、パスの検査は `artifact_path_str` を通る。

## device の要る試験（3/8〜7/8）

どれも `#[ignore = "touches the Metal device; run only in a dedicated device session"]` 付き。`backend_metal.rs` の行頭の `#[ignore` は 85 件で、このうち 17 件が TASK-492 の試験。

| module | 本数 | 試験 |
| ------ | ---- | ---- |
| `imp::device_tests::artifact_hooks_reach` | 1 | `metallib_route_rejects_bad_inputs_without_touching_the_table` |
| `imp::device_tests::dispatch::metallib` | 8 | `gemm_topk_and_relu_build_from_their_metallibs`・`a_metallib_builds_after_its_source_and_air_are_removed`・`an_entry_point_missing_from_the_metallib_is_rejected`・`mismatched_shape_and_stride_dimensions_are_rejected_by_c3`・`the_written_binary_archive_matches_the_file`・`a_pipeline_builds_from_the_archive_with_fail_on_miss`・`an_entry_missing_from_the_archive_fails_only_with_fail_on_miss`・`an_archive_pipeline_rejects_a_binding_without_recording` |
| `metallib::parity_gemm_relu` | 2 | `gemm_from_the_metallib_matches_the_cpu_reference_for_14_cases`・`relu_from_the_metallib_matches_the_cpu_reference` |
| `metallib::parity_topk` | 1 | `topk_from_the_metallib_matches_the_cpu_reference_for_2_cases` |
| `metallib::parity_coopmat` | 5 | `coopmat_gemm_from_the_metallib_matches_the_cpu_reference_for_sq512`・`..._for_sq2048`・`..._for_sq4096`・`coopmat_gemm_from_the_metallib_matches_the_cpu_for_rect4096x1024x4096`・`coopmat_gemm_from_the_metallib_matches_the_cpu_reference_for_odd37x41x53` |

- `artifact_hooks_reach` と `metallib`（`parity_*` を除く）は feature `artifact-inspection` の裏。
- `parity_gemm_relu`・`parity_topk`・`parity_coopmat` は `artifact-inspection` と `test-support` の両方の裏。

`--ignored` を付けて device の上で流すときのコマンドの形は次のとおり。この PR では流していない。

```text
cargo test --release -p fandhe-silicon-contract --features backend-metal,artifact-inspection,test-support --lib metallib -- --ignored
```

## 完了の条件の確かめ（8/8）

8/8（#4936）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-492）。GPU を使わない Linux で、下の base で実行した値を書く。

時点のコミットは `3cd4a0c`、`docs/spec` の gitlink は `5f6d802c905938bba1ed963340029d61fe3651a0`。1/8 の親のコミットは `c2abac4`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | core に feature と口がある | `grep -n 'artifact-inspection' crates/core/Cargo.toml`・`grep -n 'fn artifact_hooks' crates/core/src/device.rs` | `Cargo.toml` の 28 行目と 267 行目。`device.rs` の 199 行目。下の (1) の出力 |
| 2 | 契約の crate の feature が core へ転送し `MTLBinaryArchive` を有効にする | `grep -n 'artifact-inspection =' crates/contract/Cargo.toml` | 54 行目に 3 つの値。下の (2) の出力 |
| 3 | Metal の上書きと 3 メソッド | `grep -n 'impl ArtifactHooks for MetalDevice\|fn artifact_hooks\|fn create_pipeline_checked_from_metallib' crates/contract/src/backend_metal.rs` | 下の (3) の出力 |
| 4 | core の試験 | `cargo test -p fandhe-silicon-core --features artifact-inspection --test device_artifact_hooks` | `running 3 tests`、`test result: ok. 3 passed; 0 failed; 0 ignored` |
| 5 | パスとファイルの部品の試験（Linux） | `cargo test -p fandhe-silicon-contract --features artifact-inspection --lib artifact_path_tests` | `running 8 tests`、`test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 160 filtered out` |
| 6 | device の要る試験の宣言 | `grep -cE '^\s*#\[ignore' crates/contract/src/backend_metal.rs` | 85 件（`c2abac4` では 68）。差の 17 件は上の表の 1+8+2+1+5 |
| 7 | 構造・unsafe・feature の範囲の守りの試験 | `cargo test -p fandhe-silicon-contract --all-features --test backend_metal_unsafe_count --test backend_metal_no_recording --test feature_default_scope` | `running 6 tests`・`running 1 test`・`running 15 tests` が、どれも `test result: ok`（6 passed、1 passed、15 passed） |
| 8 | unsafe 0・依存の crate 0 | `git diff --stat c2abac4 HEAD -- Cargo.lock deps-approved.toml scripts .github`・`git diff c2abac4 HEAD -- crates/ \| grep -c '^+.*unsafe'` | 差分は空。`unsafe` を含む追加行は 0 件 |
| 9 | macOS の型検査と試験の一覧 | CI の `make verify + build-profiles (macos-latest)`（下の「macOS の CI」） | job の conclusion は `success`。下の「macOS の CI」 |

(1) の出力。

```text
crates/core/Cargo.toml:28:artifact-inspection = []
crates/core/Cargo.toml:267:required-features = ["artifact-inspection"]
199:    fn artifact_hooks(&self) -> Option<&dyn crate::artifact::ArtifactHooks> {
```

(2) の出力。

```text
54:artifact-inspection = ["backend-metal", "fandhe-silicon-core/artifact-inspection", "objc2-metal?/MTLBinaryArchive"]
```

(3) の出力。

```text
3063:        fn artifact_hooks(&self) -> Option<&dyn ArtifactHooks> {
3804:    impl ArtifactHooks for MetalDevice {
3900:        fn create_pipeline_checked_from_metallib(
```

### 品質の門

| コマンド | 結果 |
| -------- | ---- |
| `make check` | 終了コード 0 |
| `make verify` | 終了コード 0。`test result:` の行は 255 行で、`2710 passed; 0 failed; 82 ignored` |
| `make deny` | 終了コード 0 |
| `make deps-check` | 終了コード 0 |
| `make build-profiles` | 終了コード 0 |
| `make profile-test-count-check` | 終了コード 0 |
| `make wasm-verify` | 終了コード 0 |
| `make core-deps-check` | 終了コード 0 |
| `make public-api-check` | 終了コード 0 |
| `make public-name-check` | 終了コード 0 |
| `make public-api-webgpu-check` | 終了コード 0 |
| `make public-api-sdk-check` | 終了コード 0 |
| `make wasm-backend-check` | 終了コード 0 |
| `cargo clippy -p fandhe-silicon-contract --all-targets --target aarch64-apple-darwin --no-default-features --features contract-core,cpu-isa,cpu-ops,<組> -- -D warnings` | `<組>` が `backend-metal`・`artifact-inspection`・`artifact-inspection,fault-injection`・`artifact-inspection,test-support`・`artifact-inspection,test-support,fault-injection,native-blocking-wait` の 5 組で、どれも終了コード 0 |
| `cargo clippy -p fandhe-silicon-contract --all-targets --target aarch64-apple-darwin --all-features -- -D warnings` | 終了コード 0 |
| `cargo clippy -p fandhe-silicon-contract --all-targets --target wasm32-unknown-unknown --features backend-metal,artifact-inspection -- -D warnings` | 終了コード 0 |

### macOS の CI

`3cd4a0c`（7/8 のマージ）の push の run `43283514954` の job `make verify + build-profiles (macos-latest)`（job の ID は `131410736297`）の conclusion は `success`。ログは `gh run view 43283514954 --log --job 131410736297` で引く。

| 確かめ | 結果 |
| ------ | ---- |
| `artifact_path_tests` | 8 本が `ok` の行で出る |
| `device_artifact_hooks` | 試験の実行ファイルの行が出る |
| `imp::device_tests::artifact_hooks_reach` と `imp::device_tests::dispatch::metallib`（`parity_*` を含む） | 重複を除いて 17 本が `ignored, touches the Metal device; run only in a dedicated device session` の行で出る。上の表の 1+8+2+1+5 と同じ数 |

## 段

| 段 | 内容 | Issue | PR |
| -- | ---- | ----- | -- |
| 1/8 | 成果物の検査の口の trait と device の読み出しを feature の裏に足す | #4929 | #8249 |
| 2/8 | 成果物の検査の口のパスの変換と書き出したファイルの読み取りを足す | #4930 | #8251 |
| 3/8 | Metal の device で成果物の検査の口を上書きする | #4931 | #8252 |
| 4/8 | metallib と binary archive から作る経路の device の要る試験を `#[ignore]` 付きで足す | #4932 | #8253 |
| 5/8 | metallib から作った GEMM と ReLU を投入して CPU の参照と比べる試験を `#[ignore]` 付きで足す | #4933 | #8254 |
| 6/8 | metallib から作った部分 Top-k を投入して CPU の参照と比べる試験を `#[ignore]` 付きで足す | #4934 | #8255 |
| 7/8 | metallib から作った協調行列の GEMM を投入して CPU の参照と比べる試験を `#[ignore]` 付きで足す | #4935 | #8256 |
| 8/8 | 完了の条件の確かめ | #4936 | 本頁 |
