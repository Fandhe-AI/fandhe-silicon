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
