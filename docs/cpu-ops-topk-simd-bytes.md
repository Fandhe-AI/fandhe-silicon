# 部分 Top-k の SIMD 版と scalar 版の出力の一致の確かめ（TASK-2452）

## SIMD 版と scalar 版の出力をバイトの列で比べる試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2452。親 Issue は #1071、この節（1/2）は #6952。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_topk_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`。既にある設定） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::topk_partial_scalar`・`CpuOps::topk_partial_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1352 のポインタ） |
| 入力 | 同じファイルの 20,000 件・k=64、同点ありの 200 件・k=10、非有限ありの 200 件・k=10。生成は `SplitMix64`・`GEMM_INPUT_SEED`（TASK-1005 のポインタ）。並びの規則の試験の頁は `docs/cpu-ops-topk-order.md`（TASK-2450 のポインタ） |

### 試験

| 試験 | 確かめる値 |
| ---- | ---------- |
| `topk_simd_scalar_byte_sequences_match` | 2 型のそれぞれ、3 つの入力のそれぞれで、scalar 版と SIMD 版を `out` の前詰めの値を別々にして呼ぶ。どちらの出力も前詰めの値を残さず、バイトの列の長さが `k` 件分で、違うバイトが 0。比べたバイトの数の合計が、3 つの入力の k の合計から求めた値と等しい |
| `topk_byte_count_detects_divergence` | 3 つの入力で、`DefaultCpuOps` の scalar 版の出力のバイトの列について、自分自身との比べが 0、最後の件のスコアのビットの最下位を反転した写しとの比べが 1、先頭 2 件を入れ替えた写しとの比べが 0 より大きい値、最後の 1 件を落とした写しとの比べが `None` |

補助の関数は次のとおり。

- `out_bytes`: 出力の各件のスロット番号とスコアのビットを、この順に並べたバイトの列にする。
- `differing_bytes`: 2 つのバイトの列の長さが違えば `None`、同じなら同じ位置の値が違うバイトの数を返す。
- `assert_overwritten`: 出力のどの件も、前詰めの値のスロット番号・スコアのビットを残していないことを確かめる。

同じファイルの既存の `topk_simd_matches_scalar_bytes` も、同じ 2 つのメソッドを（スロット番号、スコアのビット）の組で比べる。

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
```

## 完了の条件の確かめ（2/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2452。親 Issue は #1071、この節（2/2）は #6953。

実行環境は GPU なしの Linux。確かめた base のコミットは `24eaeca`。コード・試験・スクリプトは変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 3 つの入力（20,000 件・k=64、同点ありの 200 件・k=10、非有限ありの 200 件・k=10）で、2 型のそれぞれの `topk_partial_simd` と `topk_partial_scalar` の出力のバイトの列が一致し、違うバイトが 0、比べたバイトの数が入力の件数から求めた合計と等しく、どちらの出力も前詰めの値を残さない | `cpu_ops_topk_reference` の `topk_simd_scalar_byte_sequences_match` | `ok` |
| (2) | 比べの関数が 1 ビットの違い・件の入れ替え・件数の違いを数える | `topk_byte_count_detects_divergence` | `ok` |
| (3) | 既存の組の比べの試験も通る | 既存の `topk_simd_matches_scalar_bytes` | `ok` |
| (4) | 同じファイルの試験が合わせて通る | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` | `12 passed; 0 failed; 0 ignored` |
| (5) | wasm32 の構成で試験が lint を通る | `make wasm-verify`（契約の crate の試験の wasm32 の clippy の段〈simd128 なし・あり〉を含む） | 終了コード 0 |
| (6) | 製品のコード・依存に差分が無い | `git diff --stat bba66a5..HEAD -- crates/contract/src crates/contract/Cargo.toml Cargo.lock deps-approved.toml` | 出力なし（差分 0） |
| (7) | CI で流れる場所 | `main` 上の 1/2 のコミット `24eaeca` の `ci.yml` の run 41670055160（`gh run view 41670055160 --json conclusion`） | 結論は `success` |
| (8) | CI のジョブ `verify`（表示名 `make verify (fmt + clippy + test)`、`runs-on: ubuntu-latest`）のログ | `gh run view 41670055160 --json jobs -q '.jobs[] \| "\(.databaseId) \(.name)"'` でジョブ 126035691449 を引き、`gh run view 41670055160 --log --job 126035691449` を読む | `make verify` の段の `cpu_ops_topk_reference` は `12 passed; 0 failed; 0 ignored`。`make build-profiles` の段の同じ試験の行も `12 passed; 0 failed; 0 ignored` |
| (9) | CI のジョブ `verify-macos`（表示名 `make verify + build-profiles (macos-latest)`、`runs-on: macos-latest`）のログ | 同じ jobs の問合せでジョブ 126035691524 を引き、`gh run view 41670055160 --log --job 126035691524` を読む | `make verify` の段の `cpu_ops_topk_reference` は `12 passed; 0 failed; 0 ignored`。`make build-profiles` の段の同じ試験の行も `12 passed; 0 failed; 0 ignored` |

### 1/2 の対応

| 段 | PR | コミット |
| -- | -- | -------- |
| 1/2 | #7918 | `24eaeca` |

### 追加で流したコマンド

```text
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference   # 12 passed
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection                          # 7 passed
make build-profiles                                                                           # 終了コード 0
make wasm-verify                                                                              # 終了コード 0
```

### 再実行の契機

`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_topk_reference.rs` を変えたときに、この表を取り直す。

## 今の値の補い（base 25d8dc2）

上の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（2/2）の (6) | `bba66a5..HEAD` の差分は出力なし → `bba66a5..24eaeca` は今も出力なし。`bba66a5..25d8dc2` は `crates/contract/Cargo.toml` が +4 行（#7920 の `cpu_ops_public_surface` の `[[test]]`）、`crates/contract/src/cfg_gate_doctest.rs` が +17 行・-2 行（#7926 の `CpuOpsNonExhaustiveGate`）。`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/` の差分は 0。`Cargo.lock`・`deps-approved.toml` は差分なし | `git diff --stat bba66a5..24eaeca -- crates/contract/src crates/contract/Cargo.toml Cargo.lock deps-approved.toml`、`git diff --stat bba66a5..HEAD -- crates/contract/src crates/contract/Cargo.toml Cargo.lock deps-approved.toml`、`git log --format='%h %s' bba66a5..HEAD -- crates/contract/src crates/contract/Cargo.toml` |
| 完了の条件の確かめ（2/2）の (4) | `cpu_ops_topk_reference` は 12 本 → 今も 12 本（`12 passed; 0 failed; 0 ignored`） | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` |

## 今の値の補い（base e416527）

上の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「今の値の補い（base 25d8dc2）」の `cpu_ops_topk_reference` の本数（12 本） | 12 本 → 18 本（`18 passed; 0 failed; 0 ignored`）。足された 6 本は -0.0 を含む入力を扱う試験で、所在は [cpu-ops-topk-signed-zero.md](cpu-ops-topk-signed-zero.md) | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` |

## 今の値の補い（base 0f8b01c）

記録時点の表と既存の節は書き換えない。以下は base `0f8b01c` で観測した今の値（実行環境は GPU なしの Linux x86_64。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（2/2）の (6)（`:79`） | `bba66a5..25d8dc2` の `crates/contract/Cargo.toml` は +4 行 → `bba66a5..0f8b01c` で +8 行。`cfg_gate_doctest.rs` は +17 行・-2 行、`cpu_ops.rs` は +7 行・-1 行（`cpu_ops/` の差分は 0） | `git diff --numstat bba66a5..0f8b01c -- crates/contract/src crates/contract/Cargo.toml Cargo.lock deps-approved.toml` |

## 今の値の補い（base 2f35b9e）

記録時点の表と既存の節は書き換えない。以下は base `2f35b9e` で観測した今の値（実行環境は GPU なしの Linux x86_64）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「確かめ方」の `cpu_ops_build_selection`（`:64`） | `7 passed` → `17 passed` | `cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection` |
