# 行列積の scalar 版の積和の順と特殊値の確かめ（TASK-2445）

## 積和の順と特殊値の入力の試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2445。親 Issue は #1066、この節（1/2）は #6932。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_gemm_scalar_rules.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::gemm_scalar`。型は `DefaultCpuOps`・`ReferenceCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 参照 | `reference_gemm_f32`・`reference_gemm_f32_descending_k`（TASK-24 のポインタ） |
| 有限の入力 | 64³（生成器の位置 0）・256³（位置 `2 * 64 * 64`）・`GemmGateRow::Odd37x41x53`（37×41×53。`a_offset()` は `2 * 64 * 64 + 2 * 256 * 256`。`GemmGateRow` の所在は TASK-1005 のポインタ） |
| 特殊値の入力 | `CaseId::G01`〜`G13`（TASK-977 のポインタ）。NaN・+Inf・-Inf・+0・-0 の件数の和は `[3, 6, 3, 59, 60]` |
| 比べ | `compare_f32`・`compare_f32_with_special_values`（`T1`。TASK-25 のポインタ） |
| 番兵 | 出力を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |
| 関連の頁 | `docs/cpu-ops-elementwise-special.md`（TASK-2456 のポインタ） |

### 試験

| 試験 | 入力 | 型 | 期待 |
| ---- | ---- | -- | ---- |
| `gemm_scalar_follows_ascending_k` | 有限の 3 つの入力 | 2 型の `gemm_scalar` | `reference_gemm_f32` と `compare_f32(.., T1)` で `compared()` が m×n・`is_match()`、ビットまで一致。`reference_gemm_f32_descending_k` とはビットが違う |
| `special_inputs_hold_special_values` | `G01`〜`G13` の `a`・`b` | なし | NaN・+Inf・-Inf・+0・-0 の件数の和が `[3, 6, 3, 59, 60]` |
| `gemm_scalar_output_classes_match_design` | `G01`〜`G13` | 2 型の `gemm_scalar` | 出力の長さが `design()` の長さ、番兵のビットが残らない、各要素の `classify` が `design()` と同じ |
| `gemm_scalar_single_element_classes` | `G01`〜`G07`・`G13` | 2 型の `gemm_scalar` | 出力 1 要素の区分が試験の中の表（NaN・+Inf・-Inf・-Inf・NaN・NaN・NaN・NaN）と同じ |
| `gemm_scalar_matches_case_reference_by_rule` | `G01`〜`G13` | 2 型の `gemm_scalar` | `compare_f32_with_special_values(id.reference(), 出力, T1)` で `compared()` が m×n、`exceeded()`・`non_finite_mismatch()` が 0、`is_match()` |
| `rule_detects_replaced_non_finite` | `G05`・`G02` | `DefaultCpuOps` の `gemm_scalar` | 先頭の出力を `0.0`（`G05`）・-Inf（`G02`）に置き換えた列は `non_finite_mismatch()` が 1、`is_match()` が偽、`classify` が `design()` と違う |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_gemm_scalar_rules
```

観測した結果は `6 passed; 0 failed; 0 ignored`。

## 完了の条件の確かめ（2/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2445。親 Issue は #1066、この節（2/2）は #6933。

本リポで観測できる事実を、コマンドと出力だけで記録する（TASK-2445）。

時点のコミット: `ed6f04f`（1/2 のマージ）。(1)〜(5) は、このコミットの main への push で流れた CI の run `42452409637` から取る（event `push`・conclusion `success`・headSha は `ed6f04f5a5c1a265182ee12cc7e0515b7336adc5`）。

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| (1) | ネイティブの試験（ubuntu） | `gh run view 42452409637 --job 128644242408 --log` の `make verify` の段 | ジョブ `make verify (fmt + clippy + test)` は `success`。`running 6 tests`・6 本の `ok`・`test result:` の行（下のコードブロック） |
| (2) | ネイティブの試験（macOS arm64） | `gh run view 42452409637 --job 128644242269 --log` の `make verify` の段 | ジョブ `make verify + build-profiles (macos-latest)` は `success`。`running 6 tests`・6 本の `ok`・`test result:` の行（下のコードブロック） |
| (3) | 既存の試験の行 | (1)(2) と同じ 2 つのログの `make verify` の段の `cpu_ops_reference.rs` の塊 | ubuntu は `running 13 tests`・macOS は `running 14 tests`。どちらも `gemm_scalar_matches_cpu_reference` と `reference_ops_follow_scalar_path` が `ok`（下のコードブロック） |
| (4) | wasm32 の lint | `gh run view 42452409637 --job 128644242454 --log` の `make wasm-verify` の段 | ジョブ `wasm32 build + size + node (macos-latest)` は `success`。段の見出しの行 2 行（下のコードブロック） |
| (5) | ジョブの環境 | `gh run view 42452409637 --job 128644242269 --log` の `Set up job` の段 | 下のコードブロック |
| (6) | unsafe・外部の依存の追加 0 | `git diff --stat`・`git show`・`grep -c`（下のコードブロック） | stat は空、`[[test]]` の 4 行、`grep -c` は `0` |

ログの行は `ジョブ名<TAB>段の名前<TAB>時刻 本文` の形で、下のコマンドは段の名前で絞って時刻までを落とす。

(1) の出力。`make verify` の段のうち、試験の名前を含む `running` の行から `test result:` までの塊。

```console
$ gh run view 42452409637 --job 128644242408 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{r=$0;b="";k=0} /^test (gemm_scalar_follows_ascending_k|special_inputs_hold_special_values|gemm_scalar_output_classes_match_design|gemm_scalar_single_element_classes|gemm_scalar_matches_case_reference_by_rule|rule_detects_replaced_non_finite) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s\n%s%s\n",r,b,$0;exit}}'
running 6 tests
test gemm_scalar_single_element_classes ... ok
test rule_detects_replaced_non_finite ... ok
test special_inputs_hold_special_values ... ok
test gemm_scalar_output_classes_match_design ... ok
test gemm_scalar_matches_case_reference_by_rule ... ok
test gemm_scalar_follows_ascending_k ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
```

(2) の出力。(1) と同じ式を `--job 128644242269` のログに当てる。

```console
$ gh run view 42452409637 --job 128644242269 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^running [0-9]+ tests?$/{r=$0;b="";k=0} /^test (gemm_scalar_follows_ascending_k|special_inputs_hold_special_values|gemm_scalar_output_classes_match_design|gemm_scalar_single_element_classes|gemm_scalar_matches_case_reference_by_rule|rule_detects_replaced_non_finite) \.\.\. /{b=b $0 "\n";k=1} /^test result:/{if(k){printf "%s\n%s%s\n",r,b,$0;exit}}'
running 6 tests
test gemm_scalar_output_classes_match_design ... ok
test gemm_scalar_single_element_classes ... ok
test rule_detects_replaced_non_finite ... ok
test gemm_scalar_matches_case_reference_by_rule ... ok
test special_inputs_hold_special_values ... ok
test gemm_scalar_follows_ascending_k ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s
```

(3) の出力。`Running tests/cpu_ops_reference.rs` の行以降の、`running` の行・2 本の `ok`・その塊の `test result:` の行だけを抜く。塊にはほかの試験もあり、`running` の件数と `test result:` の passed の件数は同じ塊のものである。

```console
$ gh run view 42452409637 --job 128644242408 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk -v re='^test (gemm_scalar_matches_cpu_reference|reference_ops_follow_scalar_path) [.][.][.] ' '/Running tests\/cpu_ops_reference\.rs/{k=1;next} k&&/^running [0-9]+ tests?$/{print;next} k&&($0 ~ re){print;next} k&&/^test result:/{print;exit}'
running 13 tests
test gemm_scalar_matches_cpu_reference ... ok
test reference_ops_follow_scalar_path ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.32s
$ gh run view 42452409637 --job 128644242269 --log | awk -F'\t' '$2=="make verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk -v re='^test (gemm_scalar_matches_cpu_reference|reference_ops_follow_scalar_path) [.][.][.] ' '/Running tests\/cpu_ops_reference\.rs/{k=1;next} k&&/^running [0-9]+ tests?$/{print;next} k&&($0 ~ re){print;next} k&&/^test result:/{print;exit}'
running 14 tests
test gemm_scalar_matches_cpu_reference ... ok
test reference_ops_follow_scalar_path ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.81s
```

(4) の出力。

```console
$ gh run view 42452409637 --job 128644242454 --log | awk -F'\t' '$2=="make wasm-verify"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | grep -E 'cargo clippy \(crates/contract tests, wasm32-unknown-unknown, (simd128|scalar)\) -D warnings'
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, simd128) -D warnings
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, scalar) -D warnings
```

(5) の出力。`Set up job` の段の `Operating System` と `Runner Image` の group の行。

```console
$ gh run view 42452409637 --job 128644242269 --log | awk -F'\t' '$2=="Set up job"' | sed -E 's/^[^\t]*\t[^\t]*\t[^ ]+ //' | awk '/^##\[group\]Operating System$/{p=1} /^##\[group\]Runner Image$/{p=1} p{print} /^##\[endgroup\]/{p=0}'
##[group]Operating System
macOS
26.7.2
25G332
##[endgroup]
##[group]Runner Image
Image: macos-26-arm64
Version: 20261102.0473.1
Included Software: https://github.com/actions/runner-images/blob/macos-26-arm64/20261102.0473/images/macos/macos-26-arm64-Readme.md
Image Release: https://github.com/actions/runner-images/releases/tag/macos-26-arm64%2F20261102.0473
##[endgroup]
```

(6) の出力。base は 1/2 の直前のコミット `a60b3d8`。

```console
$ git diff --stat a60b3d8..ed6f04f -- deps-approved.toml wasm/deps-approved.toml Cargo.toml Cargo.lock wasm crates/contract/src crates/core scripts .github
$ git show ed6f04f -- crates/contract/Cargo.toml | grep '^+' | grep -v '^+++'
+[[test]]
+name = "cpu_ops_gemm_scalar_rules"
+required-features = ["cpu-ops", "test-support"]
+
$ grep -c unsafe crates/contract/tests/cpu_ops_gemm_scalar_rules.rs
0
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 積和の順と特殊値の入力の試験 | #6932 |
| 2/2 | 完了の条件の確かめ | #6933 |
