# 要素ごとの演算の出力の全要素の確かめ（TASK-2455）

## 出力の全要素が書かれることの試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2455。親 Issue は #1072、この節（1/2）は #6954。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_elementwise_written.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::elementwise_scalar`・`elementwise_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 入力 | 長さ 1,000,003。生成は `SplitMix64`・`GEMM_INPUT_SEED`（TASK-24 のポインタ）で、`0.0` と等しい値は飛ばす |
| 番兵 | `out` を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |

### 試験

| 試験 | 入力の長さ | 型とメソッド | 期待 |
| ---- | ---------- | ------------ | ---- |
| `input_has_no_special_values` | 1,000,003 | なし | 長さが `4 * 250_000 + 3`、全要素が有限、`0.0` と等しい要素が 0 件 |
| `relu_writes_every_element` | 0〜9、1,000,000〜1,000,003 の接頭辞 | 2 型 × 2 メソッドの 4 通り、`Relu` | 番兵が 0 件、各要素が `x.max(0.0)` とビットまで一致 |
| `add_scalar_writes_every_element` | 同上 | 4 通り、`AddScalar(0.5)` | 番兵が 0 件、各要素が `x + 0.5` とビットまで一致 |
| `last_elements_are_written` | 1,000,003、6、7 | 4 通り × 2 演算 | 長さ 1,000,003 の末尾 3 要素と、長さ 6・7 の接頭辞の末尾の要素が、番兵でなく式とビットまで一致 |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_written
```

観測した結果は `4 passed; 0 failed; 0 ignored`。

## 完了の条件の確かめ（2/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2455。親 Issue は #1072、この節（2/2）は #6955。

実行環境は GPU なしの Linux。確かめた base のコミットは `6cdfdc8`。CI の run は 1/2 のコミット `d180582` の `main` への push。コード・試験・スクリプトは変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | `Relu` で出力の全要素が書かれ、各要素が `x.max(0.0)` とビットまで一致する | `cpu_ops_elementwise_written` の `relu_writes_every_element` | `ok` |
| (2) | `AddScalar(0.5)` で同じことが成り立つ | `add_scalar_writes_every_element` | `ok` |
| (3) | 長さ 1,000,003 の出力の末尾 3 要素と、長さ 6・7 の接頭辞の末尾の要素が書かれる | `last_elements_are_written` | `ok` |
| (4) | 入力の長さと値の範囲 | `input_has_no_special_values` | `ok` |
| (5) | CI で流れる場所 | run `41652139978` の `make verify (fmt + clippy + test)` と `make verify + build-profiles (macos-latest)` のログの `cpu_ops_elementwise_written` の `test result:` の行。手元の `make wasm-verify`（契約の crate の試験の wasm32 の simd128 の構成の clippy の段を含む） | 2 ジョブとも `test result: ok. 4 passed; 0 failed; 0 ignored` が 2 行ずつ。run の結論は `success`。`make wasm-verify` は終了コード 0（clippy の段の終了コード） |
| (6) | 依存 0・unsafe 0 | `git diff --stat d180582^ d180582 -- Cargo.lock deps-approved.toml`、`git show d180582 -- crates/contract/Cargo.toml`、`grep -c unsafe crates/contract/tests/cpu_ops_elementwise_written.rs` | 1 つ目は出力なし。2 つ目の追加は `[[test]]` の 4 行（空行・`[[test]]`・`name`・`required-features`）。3 つ目は `0` |

### 1/2 の対応

| 段 | PR | コミット |
| -- | -- | -------- |
| 1/2 | #7907 | `d180582` |

### 追加で流したコマンド

```text
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_written   # 4 passed
make verify                                                                                        # 終了コード 0
make deps-check                                                                                    # 終了コード 0
make build-profiles                                                                                # 終了コード 0
make wasm-verify                                                                                   # 終了コード 0
```

### 再実行の契機

`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_elementwise_written.rs` を変えたときに、この表を取り直す。

## 今の値の補い（base 25d8dc2）

上の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（2/2）の (5) | wasm32 の clippy の段は simd128 の構成の段 → `scripts/wasm-verify.sh` に simd128 なしの wasm32 の scalar の構成の段も加わった（#7909。`log` は 59 行目が simd128、68 行目が scalar。どちらも `--features contract-core,cpu-isa,cpu-ops,test-support` の `--tests`）。この試験は `required-features = ["cpu-ops", "test-support"]` なので、2 つの段のどちらでも lint される。`make wasm-verify` は終了コード 0。CI の `ci.yml` の run 41659827154（`headSha` は `dc8b673`）は結論 `success` で、ジョブ `make deny (cargo-deny)` と `wasm32 build + size + node (macos-latest)` のログに simd128 の段と scalar の段の `log` の行が 1 行ずつある | `grep -n 'cargo clippy (crates/contract tests' scripts/wasm-verify.sh`、`make wasm-verify`、`gh run view 41659827154 --json conclusion,headSha`、`gh run view 41659827154 --log` から `cargo clippy (crates/contract tests` の行を抜く |
| 再実行の契機 | `crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_elementwise_written.rs` → 契機に `scripts/wasm-verify.sh` の wasm32 の clippy の段（simd128 の段・scalar の段）を変えたときも含まれる | 上の `grep` |
| 確かめ方・完了の条件の確かめ（2/2）の (1)〜(4) | `cpu_ops_elementwise_written` は 4 本 → 今も 4 本（`4 passed; 0 failed; 0 ignored`） | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_written` |

今の値の補いとして、次の試験が加わっている。

| 試験 | 所在 | 本数 |
| ---- | ---- | ---- |
| `elementwise_add_scalar_other_constants`（#7917） | `crates/contract/tests/cpu_ops_reference.rs` の 277 行目。`AddScalar(c)` を 0.5 以外の定数（`-2.5`・`1.0`・`3.75`・`0.3`・`-0.125`・`1.0e6`）で、長さ `0`・`1`・`3`・`4`・`5`・`7`・`8`・`13`・`1024`・`1027`・`ELEMENTWISE_LEN` について、`DefaultCpuOps` と `ReferenceCpuOps` の scalar 版・SIMD 版が `x + c` とビットまで一致することを確かめる | `cpu_ops_reference` は 8 本（`cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_reference` が `8 passed; 0 failed; 0 ignored`）のうちの 1 本 |

## 今の値の補い（base 42731d8）

記録時点の表と既存の節は書き換えない。以下は base `42731d8` で観測した今の値（実行環境は GPU なしの Linux。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「再実行の契機」（`:75`） | `crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_elementwise_written.rs`・`scripts/wasm-verify.sh` の wasm32 の clippy の段 → 契機に、要素ごとの演算の export を通して Node で流れる `make wasm-node-check`（`wasm/node/cases.json` の `elementwise_scalar`・`elementwise_simd` の 8 行）の実行も加わる。回帰テストは `make test-wasm-node-check` が `passed: 505, failed: 0` | `jq -r '.rows[].export' wasm/node/cases.json \| sort \| uniq -c`、`make test-wasm-node-check` |

## 今の値の補い（base 0f8b01c）

記録時点の表と既存の節は書き換えない。以下は base `0f8b01c` で観測した今の値（実行環境は GPU なしの Linux x86_64。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「今の値の補い（base 25d8dc2）」の試験の表（`:78`） | 同じ試験の表に `cpu_ops_elementwise_special`（5 本）が加わった。所在は [cpu-ops-elementwise-special.md](cpu-ops-elementwise-special.md) | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_special` |
| 「今の値の補い（base 42731d8）」の `cases.json` の行（`:90`） | `elementwise_scalar`・`elementwise_simd` の 8 行 → 16 行（各 8 行）。回帰テストは `passed: 505, failed: 0` → `passed: 642, failed: 0` | `jq -r '.rows[].export' wasm/node/cases.json \| sort \| uniq -c`、`make test-wasm-node-check` |
