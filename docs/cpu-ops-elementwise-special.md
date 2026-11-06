# 要素ごとの演算の NaN・±Inf・±0 の確かめ（TASK-2456）

## NaN・±Inf・±0 の入力を当てる試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2456。親 Issue は #1073、この節（1/2）は #6958。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_elementwise_special.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::elementwise_scalar`・`elementwise_simd`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 入力 | `CaseId::E2.input()`（TASK-977 のポインタ）。長さ 37。NaN 7・+Inf 6・-Inf 6・+0 6・-0 6 |
| 比べ | `compare_f32_with_special_values(.., T1)`（TASK-25 のポインタ） |
| 番兵 | `out` を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |
| 関連の頁 | `docs/cpu-ops-elementwise-check.md`（TASK-2455 のポインタ） |

### 試験

| 試験 | 入力 | 型とメソッド | 期待 |
| ---- | ---- | ------------ | ---- |
| `input_holds_special_values` | `CaseId::E2` | なし | 長さ 37、NaN・+Inf・-Inf・+0・-0 の件数が `[7, 6, 6, 6, 6]` |
| `relu_scalar_maps_nan_to_positive_zero` | `CaseId::E2` | 2 型の `elementwise_scalar`、`Relu` | 入力が NaN の位置の出力が `0.0_f32` のビット、出力に NaN が 0 件 |
| `relu_matches_case_reference_by_rule` | `CaseId::E2` | 2 型 × 2 メソッドの 4 通り、`Relu` | `CaseId::E2.reference()` と規則で比べ、`compared()` が 37、`exceeded()`・`non_finite_mismatch()` が 0 |
| `simd_matches_scalar_by_rule` | `CaseId::E2` の長さ 1〜37 の接頭辞 | 2 型の `elementwise_simd` と `DefaultCpuOps` の `elementwise_scalar`、`Relu`・`AddScalar(0.5)` | 規則で比べ、`compared()` が接頭辞の長さ、`exceeded()`・`non_finite_mismatch()` が 0 |
| `rule_detects_nan_passed_through` | `CaseId::E2` | `DefaultCpuOps` の `elementwise_scalar`、`Relu` | NaN の位置に入力の NaN を戻した列は `non_finite_mismatch()` が NaN の件数と等しい。零の符号だけを裏返した列は `is_match()` |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_elementwise_special
```

観測した結果は `5 passed; 0 failed; 0 ignored`。

## 完了の条件の確かめ（2/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2456。親 Issue は #1073、この節（2/2）は #6959。

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-2456）。

時点のコミット: `c8a923f`（1/2 のマージ）。(1)〜(9) は、このコミットの main への push で流れた CI の run `42164060534` から取る（event `push`・conclusion `success`・headSha は `c8a923fee080bcb94de0657d9c3026ebefa0dda4`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | ネイティブの試験（ubuntu） | `gh run view 42164060534 --job 127716261743 --log` の `make verify` の段 | ジョブ `make verify (fmt + clippy + test)` は `success`。`running 5 tests`・5 本の `ok`・`test result:` の行（下のコードブロック） |
| (2) | ネイティブの試験（macOS arm64） | `gh run view 42164060534 --job 127716261544 --log` の `make verify` の段 | ジョブ `make verify + build-profiles (macos-latest)` は `success`。`running 5 tests`・5 本の `ok`・`test result:` の行（下のコードブロック） |
| (3) | 8 行の比べ | `gh run view 42164060534 --job 127716261676 --log` の `make wasm-node-check` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。`node:`・`wasm-bindgen:` の行・`row:` 8 行・`count:` 8 行・`cpu-count:` 8 行・`summary:` の行（下のコードブロック） |
| (4) | 入力の件数 | 同じログの `make wasm-node-check` の段 | `input-special:` 2 行（下のコードブロック） |
| (5) | 語の比べの値 | 同じログの `make wasm-node-check` の段 | `t1:` 8 行（下のコードブロック） |
| (6) | 保存した出力からの数え直し | 同じログの `make wasm-node-check` の段 | `recount:` 8 行・`recount-cpu:` 8 行と `recount-summary:` の行（下のコードブロック） |
| (7) | 出力の sha256 | 同じログの `make wasm-node-check` の段 | `sha256:` 14 行（出力 8・ケースの行 6。下のコードブロック） |
| (8) | `wasm-macos` ジョブの環境の行 | 同じジョブの `Set up job`・`Install pinned Node and wasm-bindgen CLI`・`Show tool versions` の段 | 下のコードブロック |
| (9) | wasm32 の lint | 同じジョブの `make wasm-verify` の段 | 段の見出しの行 2 行（下のコードブロック） |
| (10) | unsafe・外部の依存の追加 0 | `git diff --stat`・`git show`・`grep -c`（下のコードブロック） | stat は空、`[[test]]` の 4 行、`grep -c` は `0` |

ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、下のコマンドは段の名前で絞って時刻までを落とす。

(1) の出力。`make verify` の段のうち、試験の名前を含む `running` の行から `test result:` までの塊。

```console
$ gh run view 42164060534 --job 127716261743 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{r=$0;b="";k=0} /^test (input_holds_special_values|relu_matches_case_reference_by_rule|relu_scalar_maps_nan_to_positive_zero|rule_detects_nan_passed_through|simd_matches_scalar_by_rule) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s\n%s%s\n",r,b,$0;exit}}'
running 5 tests
test input_holds_special_values ... ok
test relu_scalar_maps_nan_to_positive_zero ... ok
test rule_detects_nan_passed_through ... ok
test simd_matches_scalar_by_rule ... ok
test relu_matches_case_reference_by_rule ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(2) の出力。(1) と同じ式を `--job 127716261544` のログに当てる。

```console
$ gh run view 42164060534 --job 127716261544 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{r=$0;b="";k=0} /^test (input_holds_special_values|relu_matches_case_reference_by_rule|relu_scalar_maps_nan_to_positive_zero|rule_detects_nan_passed_through|simd_matches_scalar_by_rule) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s\n%s%s\n",r,b,$0;exit}}'
running 5 tests
test relu_matches_case_reference_by_rule ... ok
test input_holds_special_values ... ok
test relu_scalar_maps_nan_to_positive_zero ... ok
test rule_detects_nan_passed_through ... ok
test simd_matches_scalar_by_rule ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(3) の出力。

```console
$ gh run view 42164060534 --job 127716261676 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(node|wasm-bindgen|summary): |^(row|count|cpu-count): special-elementwise-'
node: v24.13.0
wasm-bindgen: 0.2.128
row: special-elementwise-relu-e-2-scalar-elementwise-scalar match
row: special-elementwise-relu-e-2-scalar-elementwise-simd match
row: special-elementwise-relu-e-2-simd128-elementwise-scalar match
row: special-elementwise-relu-e-2-simd128-elementwise-simd match
row: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar match
row: special-elementwise-add-scalar-e-2-scalar-elementwise-simd match
row: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar match
row: special-elementwise-add-scalar-e-2-simd128-elementwise-simd match
count: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
count: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
count: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
count: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
count: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
cpu-count: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
summary: rows=123 match=123 mismatch=0 not-run=0
```

(4) の出力。

```console
$ gh run view 42164060534 --job 127716261676 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^input-special: special-elementwise-'
input-special: special-elementwise-relu-e-2 nan=7 inf=12 zero=12
input-special: special-elementwise-add-scalar-e-2 nan=7 inf=12 zero=12
```

(5) の出力。

```console
$ gh run view 42164060534 --job 127716261676 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^t1: special-elementwise-'
t1: special-elementwise-relu-e-2-scalar-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-relu-e-2-scalar-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-relu-e-2-simd128-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-relu-e-2-simd128-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-scalar-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
t1: special-elementwise-add-scalar-e-2-simd128-elementwise-simd max-abs=0.0 max-abs-at=- max-rel=0.0 max-rel-at=- abs-over=0 rel-over=0
```

(6) の出力。

```console
$ gh run view 42164060534 --job 127716261676 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^(recount|recount-cpu): special-elementwise-|^recount-summary: '
recount: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-scalar-elementwise-simd mismatches=0 n=37
recount: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-relu-e-2-simd128-elementwise-simd mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-scalar-elementwise-simd mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar mismatches=0 n=37
recount: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
recount-cpu: special-elementwise-add-scalar-e-2-simd128-elementwise-simd mismatches=0 n=37
recount-summary: rows=118 zero=118 nonzero=0 uncompared=0
```

(7) の出力。

```console
$ gh run view 42164060534 --job 127716261676 --log | awk -F'\t' '$2=="make wasm-node-check"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E '^sha256: special-elementwise-'
sha256: special-elementwise-relu-e-2-scalar-elementwise-scalar 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2-scalar-elementwise-simd 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2-simd128-elementwise-scalar 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2-simd128-elementwise-simd 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-add-scalar-e-2-scalar-elementwise-scalar ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2-scalar-elementwise-simd ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2-simd128-elementwise-scalar ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2-simd128-elementwise-simd ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-relu-e-2.input f3973b98178c51aecfe234068e78dccd4b0cc17e682d3f1176279297c71d21c6
sha256: special-elementwise-relu-e-2.native-scalar 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-relu-e-2.cpu-reference 036c590c92cf29d207fd7fd7a3717f3f250de87f0a7648a58f998f39409ccf44
sha256: special-elementwise-add-scalar-e-2.input f3973b98178c51aecfe234068e78dccd4b0cc17e682d3f1176279297c71d21c6
sha256: special-elementwise-add-scalar-e-2.native-scalar ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
sha256: special-elementwise-add-scalar-e-2.cpu-reference ca3c4c9c481f53fde39f39a569b161c551468080b2d22f570c51b2e222550091
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
$ gh run view 42164060534 --job 127716261676 --log | awk -F'\t' '$2=="make wasm-verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E 'cargo clippy \(crates/contract tests, wasm32-unknown-unknown, (simd128|scalar)\) -D warnings'
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, simd128) -D warnings
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, scalar) -D warnings
```

(10) の出力。

```console
$ git diff --stat 901ee8e..c8a923f -- deps-approved.toml wasm/deps-approved.toml Cargo.toml Cargo.lock wasm crates/contract/src crates/core scripts .github
$ git show c8a923f -- crates/contract/Cargo.toml | grep '^+' | grep -v '^+++'
+[[test]]
+name = "cpu_ops_elementwise_special"
+required-features = ["cpu-ops", "test-support"]
+
$ grep -c unsafe crates/contract/tests/cpu_ops_elementwise_special.rs
0
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | NaN・±Inf・±0 の入力を当てる試験 | #6958 |
| 2/2 | 完了の条件の確かめ | #6959 |

## 今の値の補い（base fbbd70d）

記録時点の表と既存の節は書き換えない。以下は base `fbbd70d` で観測した今の値（実行環境は GPU なしの Linux x86_64）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「所在」の試験のファイル（`:11`） | 試験のファイルが増えた。`crates/contract/tests/cpu_ops_add_scalar_nonfinite.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops"]`）が、`AddScalar` の入力の側と定数の側の NaN・±Inf の答えを、字句で置いた期待と `elementwise_scalar` の出力で比べる。`DefaultCpuOps` と `ReferenceCpuOps` の 2 つに当て、NaN は種類だけ、ほかはビット列で比べる | `sed -n 1,3p crates/contract/tests/cpu_ops_add_scalar_nonfinite.rs` |
| 「確かめ方」の結果（`:34`） | 増えた試験の結果は `2 passed; 0 failed; 0 ignored` | `cargo test -p fandhe-silicon-contract --features cpu-ops --test cpu_ops_add_scalar_nonfinite` |
