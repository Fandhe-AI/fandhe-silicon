# 部分 Top-k の -0.0 の扱いの確かめ（TASK-2451）

## scalar 版と SIMD 版に -0.0 の入力を当てる試験（1/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2451。親 Issue は #1070、この節（1/4）は #6948。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_topk_reference.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`。既にある設定） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::topk_partial_scalar`・`CpuOps::topk_partial_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1352 のポインタ） |
| 入力 | `CaseId::E4`（長さ 16・k=8。TASK-977 のポインタ）と `SIGNED_ZERO_INPUTS` の 4 件（TASK-976 のポインタ）の計 5 件 |
| 関連の頁 | 並びの規則の試験は `docs/cpu-ops-topk-order.md`（TASK-2450 のポインタ）、バイトの比べの試験は `docs/cpu-ops-topk-simd-bytes.md`（TASK-2452 のポインタ） |

### 試験

| 試験 | 確かめる値 |
| ---- | ---------- |
| `topk_case_e4_matches_case_reference` | 2 型 × 2 メソッドの 4 組で、`CaseId::E4` の出力が `CaseId::E4.reference()` と `compare_topk` で一致する。スロット番号が `5..13`、スコアのビットがすべて `1.0_f32.to_bits()` |
| `topk_case_e4_order_rule_holds_directly` | 4 組の出力に `assert_order_rule` を当てる |
| `topk_positive_zero_precedes_negative_zero` | `SIGNED_ZERO_INPUTS` の各入力と 4 組で `assert_order_rule` を当てる。出力に `+0.0` のビットの件が 1 件以上あり、`-0.0` のビットの件があるときは、最後の `+0.0` の位置が最初の `-0.0` の位置より前 |
| `topk_simd_signed_zero_total_order_matches_scalar` | 5 件それぞれで、入力に `-0.0` のビットの語がある。`DefaultCpuOps` の scalar 版の出力と、`DefaultCpuOps`・`ReferenceCpuOps` の SIMD 版の出力の `total_order_mismatches` が `Some(0)`。比べた位置の数の合計が `2 * (8 + 2 + 4 + 2 + 4)` |
| `topk_signed_zero_inputs_differ_from_ieee_order` | `SIGNED_ZERO_INPUTS` が 4 件。4 件の順に、`DefaultCpuOps` の scalar 版の出力と `ieee_ordered_topk` の `total_order_mismatches` が `Some(2)`・`Some(2)`・`Some(1)`・`Some(4)` |
| `topk_total_order_compare_detects_divergence` | `[(1, 0.0), (0, -0.0)]` について、自分自身との比べが `Some(0)`、先頭のスコアだけ `-0.0` にした写しとの比べが `Some(1)`、2 件を入れ替えた写しとの比べが `Some(2)`、1 件を落とした列との比べが `None` |

補助の関数は次のとおり。

- `case_e4`: `CaseId::E4.input()` のスコアの列と k を返す。
- `signed_zero_cases`: `CaseId::E4` と `SIGNED_ZERO_INPUTS` の 4 件の名前・スコアの列・k を返す。
- `total_order_mismatches`: 2 つの出力の長さが違えば `None`、同じなら同じ位置のスロット番号が違うか `f32::total_cmp` が `Equal` でない位置の数を返す。
- `ieee_ordered_topk`: 有限のスコアを `partial_cmp` の降順（同点はスロット番号の昇順）に並べた先頭 k 件を返す。試験の中の比べの相手にだけ使う。

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_topk_reference
```

## -0.0 を含む 5 ケースの入力とネイティブの参照（2/4）

private spec 側の記録に従う（ポインタのみ）。この節（2/4）は #6949。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 道具のファイル | `wasm/crates/parity-host/src/lib.rs`（表 `SIGNED_ZERO_CASES`・`build_signed_zero_case`・`build_signed_zero_cases`）、試験は `wasm/crates/parity-host/tests/gen.rs` |
| 枠の頁 | `docs/wasm32-native-parity.md`（TASK-994・TASK-996 のポインタ） |
| 入力の取り方 | core の `parity::case_inputs` の `CaseId::E4.input()` と `additional_topk_inputs()` から取る。値は道具に写さない |

### ケース

| ケース | n | k | 入力の出どころ |
| ------ | - | - | -------------- |
| `topk-e-4` | 16 | 8 | `CaseId::E4` |
| `topk-signed-zero-adjacent` | 2 | 2 | `signed_zero_adjacent` |
| `topk-signed-zero-separated` | 4 | 4 | `signed_zero_separated` |
| `topk-signed-zero-cut-at-k` | 3 | 2 | `signed_zero_cut_at_k` |
| `topk-signed-zero-repeated` | 4 | 4 | `signed_zero_repeated` |

### 書くファイル

`wasm-parity-host gen <dir>` は、5 ケースそれぞれに `<ケース>.scores.bin` と `<ケース>.native-scalar.bin` を書き、`manifest.json` に `kind` が `topk` の要素を、既存の 8 要素の後ろに足す（計 13 要素）。参照は `DefaultCpuOps` の `topk_partial_scalar` の出力。

### 試験

| 試験 | 確かめる値 |
| ---- | ---------- |
| `signed_zero_cases_match_core_inputs` | ケース名が 5 個の字句に順まで等しい。スコアのビット列と k が core の同じ入力と等しい |
| `signed_zero_inputs_hold_negative_zero_before_positive_zero` | 書いた `.scores.bin` の語に、5 ケースとも `0x8000_0000` と `0x0000_0000` があり、最初の `0x8000_0000` が最初の `0x0000_0000` より前 |
| `signed_zero_native_matches_core` | `.native-scalar.bin` の記録が、`topk-e-4` は `CaseId::E4.reference()`、ほかは `TopKInput::expected()` と `compare_topk` で一致する。比べた件数が k |
| `signed_zero_native_differs_from_ieee_order` | `topk-signed-zero-*` の 4 ケースで、`partial_cmp` の降順（同点はスロット番号の昇順）の先頭 k 件との違う件の数が順に 2・2・1・4 |
| `signed_zero_native_has_no_non_finite_slots` | 5 ケースの記録の件数が k で、スコアがすべて有限 |
| `signed_zero_manifest_entries_match_files` | 5 要素の鍵の組、`kind`、`n`、`k`、ファイルの長さ・中身・sha256 が合う |
| `gen_refuses_existing_signed_zero_file` | `topk-e-4.scores.bin` が先にあると `GenError::FileExists`。中身は変わらない |
| `signed_zero_case_rejects_unknown_input_name` | 存在しない名前の行が `GenError::InputFailed` |

### 確かめ方

```sh
cargo test --manifest-path wasm/Cargo.toml --workspace --locked
grep -n SIGNED_ZERO_CASES wasm/crates/parity-host/src/lib.rs
```

## 照合の 20 行（3/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2451。親 Issue は #1070、この節（3/4）は #6950。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 入力の行 | `wasm/node/cases.json` の `rows` の末尾に 20 行 |
| 回帰テスト | `scripts/test-wasm-node-check.sh` の項目 22。部分 Top-k の行が 30 組にちょうど等しいことを照らす |
| 枠の頁 | `docs/wasm32-native-parity.md`（TASK-994・TASK-996 のポインタ） |

### 20 行

5 ケースそれぞれに 4 組（scalar 版と simd128 版のビルド × `topk_scalar`・`topk_simd`）。

| id | ビルド | export | ケース | 参照の出どころ |
| -- | ------ | ------ | ------ | -------------- |
| `topk-e-4-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-e-4` | `native-scalar` |
| `topk-e-4-scalar-topk-simd` | `scalar` | `topk_simd` | `topk-e-4` | `native-scalar` |
| `topk-e-4-simd128-topk-scalar` | `simd128` | `topk_scalar` | `topk-e-4` | `native-scalar` |
| `topk-e-4-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-e-4` | `native-scalar` |
| `topk-signed-zero-adjacent-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-signed-zero-adjacent` | `native-scalar` |
| `topk-signed-zero-adjacent-scalar-topk-simd` | `scalar` | `topk_simd` | `topk-signed-zero-adjacent` | `native-scalar` |
| `topk-signed-zero-adjacent-simd128-topk-scalar` | `simd128` | `topk_scalar` | `topk-signed-zero-adjacent` | `native-scalar` |
| `topk-signed-zero-adjacent-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-signed-zero-adjacent` | `native-scalar` |
| `topk-signed-zero-separated-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-signed-zero-separated` | `native-scalar` |
| `topk-signed-zero-separated-scalar-topk-simd` | `scalar` | `topk_simd` | `topk-signed-zero-separated` | `native-scalar` |
| `topk-signed-zero-separated-simd128-topk-scalar` | `simd128` | `topk_scalar` | `topk-signed-zero-separated` | `native-scalar` |
| `topk-signed-zero-separated-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-signed-zero-separated` | `native-scalar` |
| `topk-signed-zero-cut-at-k-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-signed-zero-cut-at-k` | `native-scalar` |
| `topk-signed-zero-cut-at-k-scalar-topk-simd` | `scalar` | `topk_simd` | `topk-signed-zero-cut-at-k` | `native-scalar` |
| `topk-signed-zero-cut-at-k-simd128-topk-scalar` | `simd128` | `topk_scalar` | `topk-signed-zero-cut-at-k` | `native-scalar` |
| `topk-signed-zero-cut-at-k-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-signed-zero-cut-at-k` | `native-scalar` |
| `topk-signed-zero-repeated-scalar-topk-scalar` | `scalar` | `topk_scalar` | `topk-signed-zero-repeated` | `native-scalar` |
| `topk-signed-zero-repeated-scalar-topk-simd` | `scalar` | `topk_simd` | `topk-signed-zero-repeated` | `native-scalar` |
| `topk-signed-zero-repeated-simd128-topk-scalar` | `simd128` | `topk_scalar` | `topk-signed-zero-repeated` | `native-scalar` |
| `topk-signed-zero-repeated-simd128-topk-simd` | `simd128` | `topk_simd` | `topk-signed-zero-repeated` | `native-scalar` |

### 確かめ方

```sh
grep -c '"case": "topk-e-4"' wasm/node/cases.json
grep -c '"case": "topk-signed-zero-' wasm/node/cases.json
make test-wasm-node-check 2>&1 | tail -n 1
```

## 完了の条件の確かめ（4/4）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2451。親 Issue は #1070、この節（4/4）は #6951。

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-2451）。

時点のコミット: `9cd7da5`（この変更の土台）。(1)・(2)・(4)〜(10) は、このコミットの main への push で流れた CI の run `42133968984` から取る（event `push`・conclusion `success`・headSha は `9cd7da53eff7faf218b4ae1f3bd2af1cc1f0ee90`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | ネイティブの試験（ubuntu） | `gh run view 42133968984 --job 127617292597 --log` の `make verify` の段 | ジョブ `make verify (fmt + clippy + test)` は `success`。`running 18 tests`・6 本の `ok`・`test result:` の行（下のコードブロック） |
| (2) | ネイティブの試験（macOS arm64） | `gh run view 42133968984 --job 127617292880 --log` の `make verify` の段 | ジョブ `make verify + build-profiles (macos-latest)` は `success`。`running 18 tests`・6 本の `ok`・`test result:` の行（下のコードブロック） |
| (3) | 5 ケースの入力とネイティブの参照の作り方 | `cargo test --manifest-path wasm/Cargo.toml --workspace --locked`（`-- --list` を含む） | 試験の名前 8 行と `running`・`test result:` の行（下のコードブロック） |
| (4) | 20 行の比べ | `gh run view 42133968984 --job 127617292897 --log` の `make wasm-node-check` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。`node:`・`wasm-bindgen:` の行・`row:` 20 行・`count:` 20 行・`summary:` の行（下のコードブロック） |
| (5) | 非有限の件数 | 同じログの `make wasm-node-check` の段 | `nonfinite:` 5 行（下のコードブロック） |
| (6) | 保存した出力からの数え直し | 同じログの `make wasm-node-check` の段 | `recount:` 20 行と `recount-summary:` の行（下のコードブロック） |
| (7) | 出力の sha256 | 同じログの `make wasm-node-check` の段 | `sha256:` 30 行（出力 20・ケースのファイル 10。下のコードブロック） |
| (8) | `wasm-macos` ジョブの環境の行 | 同じジョブの `Set up job`・`Install pinned Node and wasm-bindgen CLI`・`Show tool versions` の段 | 下のコードブロック |
| (9) | 回帰テスト | `gh run view 42133968984 --job 127617293075 --log` の `make test-wasm-node-check` の段 | ジョブ `make deny (cargo-deny)` は `success`。項目 22 の topk の `ok:` の行と最終行 `passed: 542, failed: 0` |
| (10) | wasm32 の lint | 同じ run の `wasm-macos` ジョブの `make wasm-verify` の段 | 段の見出しの行 2 行（下のコードブロック） |
| (11) | unsafe・外部の依存の追加 0 | `git diff --stat` と `git grep -n unsafe`（下のコードブロック） | stat は空、`grep` は 0 件 |

ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、下のコマンドは段の名前で絞って時刻までを落とす。

(1) の出力。`make verify` の段のうち、試験の名前を含む `running` の行から `test result:` までの塊。

```console
$ gh run view 42133968984 --job 127617292597 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{r=$0;b="";k=0} /^test (topk_case_e4_matches_case_reference|topk_case_e4_order_rule_holds_directly|topk_positive_zero_precedes_negative_zero|topk_simd_signed_zero_total_order_matches_scalar|topk_signed_zero_inputs_differ_from_ieee_order|topk_total_order_compare_detects_divergence) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s\n%s%s\n",r,b,$0;exit}}'
running 18 tests
test topk_case_e4_order_rule_holds_directly ... ok
test topk_case_e4_matches_case_reference ... ok
test topk_positive_zero_precedes_negative_zero ... ok
test topk_signed_zero_inputs_differ_from_ieee_order ... ok
test topk_simd_signed_zero_total_order_matches_scalar ... ok
test topk_total_order_compare_detects_divergence ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

(2) の出力。(1) と同じ式を `--job 127617292880` のログに当てる。

```console
running 18 tests
test topk_case_e4_order_rule_holds_directly ... ok
test topk_case_e4_matches_case_reference ... ok
test topk_positive_zero_precedes_negative_zero ... ok
test topk_signed_zero_inputs_differ_from_ieee_order ... ok
test topk_simd_signed_zero_total_order_matches_scalar ... ok
test topk_total_order_compare_detects_divergence ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

(3) の出力。

```console
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked -- --list 2>&1 | grep -E '^(signed_zero_cases_match_core_inputs|signed_zero_inputs_hold_negative_zero_before_positive_zero|signed_zero_native_matches_core|signed_zero_native_differs_from_ieee_order|signed_zero_native_has_no_non_finite_slots|signed_zero_manifest_entries_match_files|gen_refuses_existing_signed_zero_file|signed_zero_case_rejects_unknown_input_name): test$'
gen_refuses_existing_signed_zero_file: test
signed_zero_case_rejects_unknown_input_name: test
signed_zero_cases_match_core_inputs: test
signed_zero_inputs_hold_negative_zero_before_positive_zero: test
signed_zero_manifest_entries_match_files: test
signed_zero_native_differs_from_ieee_order: test
signed_zero_native_has_no_non_finite_slots: test
signed_zero_native_matches_core: test
$ cargo test --manifest-path wasm/Cargo.toml --workspace --locked 2>&1 | grep -E '^running [0-9]+ tests?$|^test result:'
running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 36 tests
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.07s
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.72s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(4) の出力。

```console
$ gh run view 42133968984 --job 127617292897 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(node|wasm-bindgen|summary): |^(row|count): topk-(e-4|signed-zero-)'
node: v24.13.0
wasm-bindgen: 0.2.128
row: topk-e-4-scalar-topk-scalar match
row: topk-e-4-scalar-topk-simd match
row: topk-e-4-simd128-topk-scalar match
row: topk-e-4-simd128-topk-simd match
row: topk-signed-zero-adjacent-scalar-topk-scalar match
row: topk-signed-zero-adjacent-scalar-topk-simd match
row: topk-signed-zero-adjacent-simd128-topk-scalar match
row: topk-signed-zero-adjacent-simd128-topk-simd match
row: topk-signed-zero-separated-scalar-topk-scalar match
row: topk-signed-zero-separated-scalar-topk-simd match
row: topk-signed-zero-separated-simd128-topk-scalar match
row: topk-signed-zero-separated-simd128-topk-simd match
row: topk-signed-zero-cut-at-k-scalar-topk-scalar match
row: topk-signed-zero-cut-at-k-scalar-topk-simd match
row: topk-signed-zero-cut-at-k-simd128-topk-scalar match
row: topk-signed-zero-cut-at-k-simd128-topk-simd match
row: topk-signed-zero-repeated-scalar-topk-scalar match
row: topk-signed-zero-repeated-scalar-topk-simd match
row: topk-signed-zero-repeated-simd128-topk-scalar match
row: topk-signed-zero-repeated-simd128-topk-simd match
count: topk-e-4-scalar-topk-scalar mismatches=0 n=8
count: topk-e-4-scalar-topk-simd mismatches=0 n=8
count: topk-e-4-simd128-topk-scalar mismatches=0 n=8
count: topk-e-4-simd128-topk-simd mismatches=0 n=8
count: topk-signed-zero-adjacent-scalar-topk-scalar mismatches=0 n=2
count: topk-signed-zero-adjacent-scalar-topk-simd mismatches=0 n=2
count: topk-signed-zero-adjacent-simd128-topk-scalar mismatches=0 n=2
count: topk-signed-zero-adjacent-simd128-topk-simd mismatches=0 n=2
count: topk-signed-zero-separated-scalar-topk-scalar mismatches=0 n=4
count: topk-signed-zero-separated-scalar-topk-simd mismatches=0 n=4
count: topk-signed-zero-separated-simd128-topk-scalar mismatches=0 n=4
count: topk-signed-zero-separated-simd128-topk-simd mismatches=0 n=4
count: topk-signed-zero-cut-at-k-scalar-topk-scalar mismatches=0 n=2
count: topk-signed-zero-cut-at-k-scalar-topk-simd mismatches=0 n=2
count: topk-signed-zero-cut-at-k-simd128-topk-scalar mismatches=0 n=2
count: topk-signed-zero-cut-at-k-simd128-topk-simd mismatches=0 n=2
count: topk-signed-zero-repeated-scalar-topk-scalar mismatches=0 n=4
count: topk-signed-zero-repeated-scalar-topk-simd mismatches=0 n=4
count: topk-signed-zero-repeated-simd128-topk-scalar mismatches=0 n=4
count: topk-signed-zero-repeated-simd128-topk-simd mismatches=0 n=4
summary: rows=55 match=55 mismatch=0 not-run=0
```

(5) の出力（同じ前段に `grep -E '^nonfinite: topk-(e-4|signed-zero-)'` をつなぐ）。

```console
nonfinite: topk-e-4 input=3 output=0
nonfinite: topk-signed-zero-adjacent input=0 output=0
nonfinite: topk-signed-zero-separated input=0 output=0
nonfinite: topk-signed-zero-cut-at-k input=0 output=0
nonfinite: topk-signed-zero-repeated input=0 output=0
```

(6) の出力（同じ前段に `grep -E '^recount: topk-(e-4|signed-zero-)|^recount-summary: '` をつなぐ）。

```console
recount: topk-e-4-scalar-topk-scalar mismatches=0 n=8
recount: topk-e-4-scalar-topk-simd mismatches=0 n=8
recount: topk-e-4-simd128-topk-scalar mismatches=0 n=8
recount: topk-e-4-simd128-topk-simd mismatches=0 n=8
recount: topk-signed-zero-adjacent-scalar-topk-scalar mismatches=0 n=2
recount: topk-signed-zero-adjacent-scalar-topk-simd mismatches=0 n=2
recount: topk-signed-zero-adjacent-simd128-topk-scalar mismatches=0 n=2
recount: topk-signed-zero-adjacent-simd128-topk-simd mismatches=0 n=2
recount: topk-signed-zero-separated-scalar-topk-scalar mismatches=0 n=4
recount: topk-signed-zero-separated-scalar-topk-simd mismatches=0 n=4
recount: topk-signed-zero-separated-simd128-topk-scalar mismatches=0 n=4
recount: topk-signed-zero-separated-simd128-topk-simd mismatches=0 n=4
recount: topk-signed-zero-cut-at-k-scalar-topk-scalar mismatches=0 n=2
recount: topk-signed-zero-cut-at-k-scalar-topk-simd mismatches=0 n=2
recount: topk-signed-zero-cut-at-k-simd128-topk-scalar mismatches=0 n=2
recount: topk-signed-zero-cut-at-k-simd128-topk-simd mismatches=0 n=2
recount: topk-signed-zero-repeated-scalar-topk-scalar mismatches=0 n=4
recount: topk-signed-zero-repeated-scalar-topk-simd mismatches=0 n=4
recount: topk-signed-zero-repeated-simd128-topk-scalar mismatches=0 n=4
recount: topk-signed-zero-repeated-simd128-topk-simd mismatches=0 n=4
recount-summary: rows=50 zero=50 nonzero=0 uncompared=0
```

(7) の出力（同じ前段に `grep -E '^sha256: topk-(e-4|signed-zero-)'` をつなぐ）。

```console
sha256: topk-e-4-scalar-topk-scalar 4835737d3f12e0eab580fc5b73695c951da10c47d9c74a526d8c2cf7613a0960
sha256: topk-e-4-scalar-topk-simd 4835737d3f12e0eab580fc5b73695c951da10c47d9c74a526d8c2cf7613a0960
sha256: topk-e-4-simd128-topk-scalar 4835737d3f12e0eab580fc5b73695c951da10c47d9c74a526d8c2cf7613a0960
sha256: topk-e-4-simd128-topk-simd 4835737d3f12e0eab580fc5b73695c951da10c47d9c74a526d8c2cf7613a0960
sha256: topk-signed-zero-adjacent-scalar-topk-scalar 8bfd453c13278f5fe3040b806e4234aa6aea3377f3abf6ca19ab74dad908b7ae
sha256: topk-signed-zero-adjacent-scalar-topk-simd 8bfd453c13278f5fe3040b806e4234aa6aea3377f3abf6ca19ab74dad908b7ae
sha256: topk-signed-zero-adjacent-simd128-topk-scalar 8bfd453c13278f5fe3040b806e4234aa6aea3377f3abf6ca19ab74dad908b7ae
sha256: topk-signed-zero-adjacent-simd128-topk-simd 8bfd453c13278f5fe3040b806e4234aa6aea3377f3abf6ca19ab74dad908b7ae
sha256: topk-signed-zero-separated-scalar-topk-scalar f9961f714c0d7c8c50e26734f0ec1f2a92567a590d9a18c9c3e3df26cc5cfc62
sha256: topk-signed-zero-separated-scalar-topk-simd f9961f714c0d7c8c50e26734f0ec1f2a92567a590d9a18c9c3e3df26cc5cfc62
sha256: topk-signed-zero-separated-simd128-topk-scalar f9961f714c0d7c8c50e26734f0ec1f2a92567a590d9a18c9c3e3df26cc5cfc62
sha256: topk-signed-zero-separated-simd128-topk-simd f9961f714c0d7c8c50e26734f0ec1f2a92567a590d9a18c9c3e3df26cc5cfc62
sha256: topk-signed-zero-cut-at-k-scalar-topk-scalar ee210783b34ae3d5ebb03e70ca167d07c36340454f12073ecc1b35a185e596f4
sha256: topk-signed-zero-cut-at-k-scalar-topk-simd ee210783b34ae3d5ebb03e70ca167d07c36340454f12073ecc1b35a185e596f4
sha256: topk-signed-zero-cut-at-k-simd128-topk-scalar ee210783b34ae3d5ebb03e70ca167d07c36340454f12073ecc1b35a185e596f4
sha256: topk-signed-zero-cut-at-k-simd128-topk-simd ee210783b34ae3d5ebb03e70ca167d07c36340454f12073ecc1b35a185e596f4
sha256: topk-signed-zero-repeated-scalar-topk-scalar 984ad52a784ed7231b10a0aef911b89236fd2c6c5fe83e0af3d1b0a5ee717c19
sha256: topk-signed-zero-repeated-scalar-topk-simd 984ad52a784ed7231b10a0aef911b89236fd2c6c5fe83e0af3d1b0a5ee717c19
sha256: topk-signed-zero-repeated-simd128-topk-scalar 984ad52a784ed7231b10a0aef911b89236fd2c6c5fe83e0af3d1b0a5ee717c19
sha256: topk-signed-zero-repeated-simd128-topk-simd 984ad52a784ed7231b10a0aef911b89236fd2c6c5fe83e0af3d1b0a5ee717c19
sha256: topk-e-4.scores 38b632eafbbd18de1ba0f25cf69ab8dfcc0badea38d57876373b9b14a6a27cc2
sha256: topk-e-4.native-scalar 4835737d3f12e0eab580fc5b73695c951da10c47d9c74a526d8c2cf7613a0960
sha256: topk-signed-zero-adjacent.scores a9765c4805658a968e5abdccd437e25907681a1cff6375e363239c53b125fcd4
sha256: topk-signed-zero-adjacent.native-scalar 8bfd453c13278f5fe3040b806e4234aa6aea3377f3abf6ca19ab74dad908b7ae
sha256: topk-signed-zero-separated.scores 835f3258d58ba3891418ae083adfea957ad428854b9f43141fb72520b056e549
sha256: topk-signed-zero-separated.native-scalar f9961f714c0d7c8c50e26734f0ec1f2a92567a590d9a18c9c3e3df26cc5cfc62
sha256: topk-signed-zero-cut-at-k.scores b53bde2bb94d94b207be0421476f23c4a0d90339534921d3b6aa73817d749543
sha256: topk-signed-zero-cut-at-k.native-scalar ee210783b34ae3d5ebb03e70ca167d07c36340454f12073ecc1b35a185e596f4
sha256: topk-signed-zero-repeated.scores 078da96d9151dec1b312083a9042d39ba325179d922dcbd8b6931d3a0df684cd
sha256: topk-signed-zero-repeated.native-scalar 984ad52a784ed7231b10a0aef911b89236fd2c6c5fe83e0af3d1b0a5ee717c19
```

(8) の出力。`Set up job` の段は `Operating System` と `Runner Image` の group の行、`Install pinned Node and wasm-bindgen CLI` の段は Node の tar の行、`Show tool versions` の段は末尾の 4 行。

```console
##[group]Operating System
macOS
26.6.2
25G83
##[endgroup]
##[group]Runner Image
Image: macos-26-arm64
Version: 20261026.0455.2
Included Software: https://github.com/actions/runner-images/blob/macos-26-arm64/20261026.0455/images/macos/macos-26-arm64-Readme.md
Image Release: https://github.com/actions/runner-images/releases/tag/macos-26-arm64%2F20261026.0455
##[endgroup]
node-v24.13.0-darwin-arm64.tar.gz: OK
node: v24.13.0
wasm-bindgen: wasm-bindgen 0.2.128
jq-1.8.2
Python 3.15.1
```

(9) の出力。

```console
$ gh run view 42133968984 --job 127617293075 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^ok: 22 bundled topk rows are exactly'
ok: 22 bundled topk rows are exactly the thirty (case, export, build) pairs
$ gh run view 42133968984 --job 127617293075 --log | awk -F'\t' '$2=="make test-wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | tail -n 1
passed: 542, failed: 0
```

(10) の出力（`make wasm-verify` の段の見出しの行）。

```console
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, simd128) -D warnings
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, scalar) -D warnings
```

(11) の出力。1/4 の base は `52ec195`。

```console
$ git diff --stat 52ec195..9cd7da5 -- deps-approved.toml wasm/deps-approved.toml Cargo.toml Cargo.lock wasm/Cargo.toml wasm/Cargo.lock crates/contract/src crates/contract/Cargo.toml crates/core wasm/crates/glue scripts/wasm-node-check.sh wasm/node/run-glue.cjs .github
$ git grep -n unsafe 9cd7da5 -- wasm/crates/parity-host/src/lib.rs crates/contract/tests/cpu_ops_topk_reference.rs
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | scalar 版と SIMD 版に -0.0 の入力を当てる試験 | #6948 |
| 2/4 | -0.0 を含む 5 ケースの入力とネイティブの参照 | #6949 |
| 3/4 | 照合の 20 行 | #6950 |
| 4/4 | 完了の条件の確かめ | #6951 |
