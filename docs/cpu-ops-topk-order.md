# 部分 Top-k の並びの規則の確かめ（TASK-2450）

## 並びの規則を出力に直に当てる試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2450。親 Issue は #1069、この節（1/2）は #6944。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_topk_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`。既にある設定） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::topk_partial_scalar`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1352 のポインタ） |
| 入力 | 同じファイルの 20,000 件・k=64、同点ありの 200 件・k=10、非有限ありの 200 件・k=10 と `SIGNED_ZERO_INPUTS`（TASK-976 のポインタ）。生成は `SplitMix64`・`GEMM_INPUT_SEED`（TASK-1005 のポインタ） |

### 試験

期待の出力を CPU 参照から作らず、製品の出力を入力に照らして確かめる。

| 試験 | 確かめる値 |
| ---- | ---------- |
| `topk_order_rule_holds_directly` | 20,000 件・同点あり・`SIGNED_ZERO_INPUTS` の各入力で、出力が `assert_order_rule` を満たす |
| `topk_ties_cross_k_boundary_in_slot_order` | 同点ありの入力で、k 件目と同じスコアのスロットが、出力の内側ではスロット番号の昇順に並び、外側のどれよりも小さい |
| `topk_non_finite_slots_absent_directly` | 非有限ありの入力で、出力のスロットのスコアがすべて有限で、`assert_order_rule` を満たす |
| `topk_first_k_are_input_pairs` | 3 つの入力で、出力が k 件で、各件のスロット番号が番兵でなく、スコアが入力の同じ位置のスコアとビットまで一致する |

補助の関数 `assert_order_rule` は、出力が k 件であること、各件が入力の（添字, スコア）の組で有限であり添字が重複しないこと、隣り合う 2 件が `total_cmp` の降順（同点はスロット番号の昇順）であること、出力に入らなかった有限のスコアのどれもが最後の 1 件より前に並ばないことを確かめる。

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
```

## 完了の条件の確かめ（2/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2450。親 Issue は #1069、この節（2/2）は #6945。

実行環境は GPU なしの Linux。確かめた base のコミットは `3d8bd76`。コード・試験・スクリプトは変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 出力が `total_cmp` の降順で、同点はスロット番号の昇順に並び、出力に入らなかった有限のスコアが最後の件より前に来ない（20,000 件・k=64、同点ありの 200 件・k=10、`SIGNED_ZERO_INPUTS`） | `cpu_ops_topk_reference` の `topk_order_rule_holds_directly` | `ok` |
| (2) | 同点が k 件目の前後にまたがる入力で、出力に入る側と入らない側がスロット番号で分かれる | `topk_ties_cross_k_boundary_in_slot_order` | `ok` |
| (3) | 非有限のスロットが出力に 0 件 | `topk_non_finite_slots_absent_directly` と既存の `topk_excludes_non_finite` | 2 本とも `ok` |
| (4) | 先頭 k 件が入力の（スロット番号、スコア）の組で埋まる | `topk_first_k_are_input_pairs` と既存の `topk_fills_first_k` | 2 本とも `ok` |
| (5) | 既存の CPU 参照との比べの試験も合わせて通る | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` | `10 passed; 0 failed; 0 ignored` |
| (6) | wasm32 の構成で試験が lint を通る | `make wasm-verify`（契約の crate の試験の simd128 の構成の clippy の段を含む） | 終了コード 0 |
| (7) | 製品のコード・依存に差分が無い | `git diff --stat 650d7f3..HEAD -- crates/contract/src crates/contract/Cargo.toml Cargo.lock deps-approved.toml` | 出力なし（差分 0） |
| (8) | CI で流れる場所 | `make verify` と、`main` 上の 1/2 のコミット `6776e6e` の CI の run（`gh run view <id> --json conclusion`） | `make verify` は終了コード 0。run の結論は `success` |

今の値の補い（base 8c52335）: 上の (8) の run は、2/2 のマージ（#7905）の `main` 上のコミット `ad056b0` の `ci.yml` の run 41647730709（`gh run view 41647730709 --json conclusion`）で、結論は `success`。

### 1/2 の対応

| 段 | PR | コミット |
| -- | -- | -------- |
| 1/2 | #7901 | `6776e6e` |

### 追加で流したコマンド

```text
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference   # 10 passed
cargo test -p fandhe-silicon-contract --test cpu_ops_build_selection                          # 7 passed
make build-profiles                                                                           # 終了コード 0
make wasm-verify                                                                              # 終了コード 0
make verify                                                                                   # 終了コード 0
```

### 再実行の契機

`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/*.rs`・`crates/contract/tests/cpu_ops_topk_reference.rs` を変えたときに、この表を取り直す。

## 今の値の補い（base 25d8dc2）

上の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（2/2）の (5)・追加で流したコマンドの `# 10 passed` | `cpu_ops_topk_reference` は `10 passed` → `12 passed; 0 failed; 0 ignored`。足された 2 本は `topk_simd_scalar_byte_sequences_match` と `topk_byte_count_detects_divergence` で、所在は [cpu-ops-topk-simd-bytes.md](cpu-ops-topk-simd-bytes.md) | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` |
| 完了の条件の確かめ（2/2）の (7) | `git diff --stat 650d7f3..HEAD -- crates/contract/src crates/contract/Cargo.toml Cargo.lock deps-approved.toml` は出力なし → `650d7f3..25d8dc2` で `crates/contract/Cargo.toml` が +8 行、`crates/contract/src/cfg_gate_doctest.rs` が +28 行・-3 行（`2 files changed, 36 insertions(+), 3 deletions(-)`）。`crates/contract/src/cpu_ops.rs`・`crates/contract/src/cpu_ops/` の差分は 0。`Cargo.lock`・`deps-approved.toml` は差分なし | 同じ `git diff --stat`、`git diff --stat 650d7f3..HEAD -- crates/contract/src/cpu_ops.rs crates/contract/src/cpu_ops`、`git log --format='%h %s' 650d7f3..HEAD -- crates/contract/src crates/contract/Cargo.toml` |

## 今の値の補い（base e416527）

上の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「今の値の補い（base 25d8dc2）」の `cpu_ops_topk_reference` の本数（12 本） | 12 本 → 18 本（`18 passed; 0 failed; 0 ignored`）。足された 6 本は -0.0 を含む入力を扱う試験で、所在は [cpu-ops-topk-signed-zero.md](cpu-ops-topk-signed-zero.md) | `cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference` |

## 今の値の補い（base 0f8b01c）

記録時点の表と既存の節は書き換えない。以下は base `0f8b01c` で観測した今の値（実行環境は GPU なしの Linux x86_64。node・wasm-bindgen の CLI は入れていないので `make wasm-node-check` の実物の実行は流していない）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 完了の条件の確かめ（2/2）の (7)（`:80`） | `650d7f3..25d8dc2` の `Cargo.toml` は +8 行 → `650d7f3..0f8b01c` で +12 行。`cfg_gate_doctest.rs` は +28 行・-3 行、`cpu_ops.rs` は +7 行・-1 行（`cpu_ops/` の差分は 0）。`Cargo.lock`・`deps-approved.toml` は差分なし | `git diff --numstat 650d7f3..0f8b01c -- crates/contract/src crates/contract/Cargo.toml Cargo.lock deps-approved.toml` |
