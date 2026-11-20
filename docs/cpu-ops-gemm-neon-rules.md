# 行列積の aarch64 の neon 版の和の順と k の端数の確かめ（TASK-2446）

## 和の順と k の端数の試験（1/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-2446。親 Issue は #1067、この節（1/2）は #6936。

### 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験のファイル | `crates/contract/tests/cpu_ops_gemm_neon_rules.rs`（`Cargo.toml` の `[[test]]` に `required-features = ["cpu-ops", "test-support"]`） |
| 呼ぶ口 | `fandhe_silicon_contract::cpu_ops` の `CpuOps::gemm_simd`・`CpuOps::gemm_scalar`。型は `DefaultCpuOps`。所在の頁は `docs/cpu-ops.md`（TASK-1351 のポインタ） |
| 入力 | 64³（生成器の位置 0）・256³（位置 `2 * 64 * 64`）・`GemmGateRow::Odd37x41x53`（37×41×53。`a_offset()` は `2 * 64 * 64 + 2 * 256 * 256`。`GemmGateRow` の所在は TASK-1005 のポインタ） |
| 4 レーンの参照 | 試験の中の `lane_reference`。`Form::Lanes` は、k を 4 つずつ 4 レーンに `mul_add` で積み、`(l0 + l1) + (l2 + l3)` で足し、端数の k を `mul_add` で足す。`Form::SplitLaneProduct` はレーンの積と和を別の演算にし、`Form::SerialLaneSum` は 4 レーンを `((l0 + l1) + l2) + l3` で足し、`Form::SplitTailProduct` は端数の積と和を別の演算にする |
| 比べ | `compare_f32`（`T1`。TASK-24 のポインタ） |
| 番兵 | 出力を `0x7fc0_beef` のビットの値で埋めてから呼ぶ |
| 関連の頁 | `docs/cpu-ops-gemm-scalar-rules.md`（TASK-2445 のポインタ）・`docs/cpu-ops-gemm-wasm-rules.md`（TASK-2447 のポインタ） |

### 試験

| 試験 | `cfg` | 期待 |
| ---- | ----- | ---- |
| `three_inputs_have_the_expected_shapes` | なし | (m, k, n, k を 4 で割った余り) が `(64, 64, 64, 0)`・`(256, 256, 256, 0)`・`(37, 53, 41, 1)`。`a`・`b` の長さが m×k・k×n で、値が有限 |
| `lane_reference_differs_from_scalar_and_other_forms` | なし | 3 つの入力で、`Form::Lanes` の参照が `gemm_scalar` と `compare_f32(.., T1)` で `compared()` が m×n・`is_match()` で、ビットが違う。`Form::SplitLaneProduct`・`Form::SerialLaneSum` の式とビットが違う。`Form::SplitTailProduct` の式とは、37×41×53 でビットが違い、64³・256³ で同じ |
| `gemm_simd_writes_every_element_of_three_inputs` | なし | 3 つの入力で、`gemm_simd` の出力の長さが m×n で、番兵のビットが残らず、全要素が有限 |
| `neon_gemm_simd_matches_scalar_by_t1` | `all(target_arch = "aarch64", target_feature = "neon")` | 3 つの入力で、`gemm_simd` が `gemm_scalar` と `compare_f32(.., T1)` で `compared()` が m×n・`is_match()` で、ビットが違う |
| `neon_gemm_simd_equals_lane_reference_bits` | `all(target_arch = "aarch64", target_feature = "neon")` | 3 つの入力で、`gemm_simd` が `Form::Lanes` の参照とビットまで一致 |

### 確かめ方

```sh
cargo test -p fandhe-silicon-contract --features test-support --test cpu_ops_gemm_neon_rules
```

x86_64 のビルドで観測した結果は `3 passed; 0 failed; 0 ignored`（`cfg` を持つ 2 本はコンパイルされない）。

## 完了の条件の確かめ（2/2）

private spec 側の記録に従う（ポインタのみ）。`docs/spec` の TASK-2446。この節（2/2）は #6937。

時点のコミットは `a05f0dc119b8538969477cef5e2c9764e01568a6`（1/2 の squash のコミット）。`docs/spec` の gitlink は次のとおり。

```console
$ git ls-tree "a05f0dc119b8538969477cef5e2c9764e01568a6" docs/spec
160000 commit 96476a8254c0c1fba240acf062644b4433123e99	docs/spec
```

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の宣言 | `#[test]`・`cfg`・`#[ignore` の行数。下のコードブロック (1) | `5`・`2`・`0` |
| (2) | 受け手の試験の名前 | 5 本の `fn` の所在と試験のファイルの所在。下のコードブロック (2) | `130`・`149`・`172`・`187`・`202` 行目と `crates/contract/tests/cpu_ops_gemm_neon_rules.rs` |
| (3) | neon 版の口の所在 | `git grep -n -F` の 4 本。下のコードブロック (3) | `neon.rs` の 60・62・64 行目と `cpu_ops.rs` の 113 行目 |
| (4) | CI（ubuntu） | push の run のログ。下のコードブロック (4) | `3 passed; 0 failed; 0 ignored` |
| (5) | CI（macOS arm64） | push の run のログ。下のコードブロック (5) | `5 passed; 0 failed; 0 ignored`・`14 passed; 0 failed; 0 ignored` |
| (6) | wasm32 の lint | push の run のログ。下のコードブロック (6) | `simd128`・`scalar` の 2 行 |
| (7) | ジョブの環境 | `Set up job` の段。下のコードブロック (7) | `Image: macos-26-arm64` |
| (8) | 依存の差と `[[test]]` の 1 節 | `git diff` と `make deps-check` の段。下のコードブロック (8) | 差は空・`[[test]]` の 4 行・`[deps-check.sh] ok` の 1 行 |
| (9) | `crates` の差と `unsafe` の語の数 | `git diff --stat`・`git show`・`make verify` の段。下のコードブロック (9) | 差は空・`0`・`3`・`3`・`ok` の 1 行 |

(1)

```console
$ git show "a05f0dc119b8538969477cef5e2c9764e01568a6":crates/contract/tests/cpu_ops_gemm_neon_rules.rs | grep -c '^[[:space:]]*#\[test\]'
5
$ git show "a05f0dc119b8538969477cef5e2c9764e01568a6":crates/contract/tests/cpu_ops_gemm_neon_rules.rs | grep -c '^#\[cfg(all(target_arch = "aarch64", target_feature = "neon"))\]$'
2
$ git show "a05f0dc119b8538969477cef5e2c9764e01568a6":crates/contract/tests/cpu_ops_gemm_neon_rules.rs | grep -c '#\[ignore'
0
```

(2)

```console
$ git grep -n "fn three_inputs_have_the_expected_shapes" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/tests/cpu_ops_gemm_neon_rules.rs:130:fn three_inputs_have_the_expected_shapes() {
$ git grep -n "fn lane_reference_differs_from_scalar_and_other_forms" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/tests/cpu_ops_gemm_neon_rules.rs:149:fn lane_reference_differs_from_scalar_and_other_forms() {
$ git grep -n "fn gemm_simd_writes_every_element_of_three_inputs" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/tests/cpu_ops_gemm_neon_rules.rs:172:fn gemm_simd_writes_every_element_of_three_inputs() {
$ git grep -n "fn neon_gemm_simd_matches_scalar_by_t1" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/tests/cpu_ops_gemm_neon_rules.rs:187:fn neon_gemm_simd_matches_scalar_by_t1() {
$ git grep -n "fn neon_gemm_simd_equals_lane_reference_bits" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/tests/cpu_ops_gemm_neon_rules.rs:202:fn neon_gemm_simd_equals_lane_reference_bits() {
$ git ls-tree -r --name-only "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates/contract/tests/cpu_ops_gemm_neon_rules.rs
crates/contract/tests/cpu_ops_gemm_neon_rules.rs
```

(3)

```console
$ git grep -n -F 'acc = vfmaq_f32(acc, va, vb);' "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates/contract/src/cpu_ops/neon.rs
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/src/cpu_ops/neon.rs:60:                acc = vfmaq_f32(acc, va, vb);
$ git grep -n -F 'let mut s = vaddvq_f32(acc);' "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates/contract/src/cpu_ops/neon.rs
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/src/cpu_ops/neon.rs:62:            let mut s = vaddvq_f32(acc);
$ git grep -n -F 's = a[i * k + p].mul_add(b[p * n + j], s);' "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates/contract/src/cpu_ops/neon.rs
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/src/cpu_ops/neon.rs:64:                s = a[i * k + p].mul_add(b[p * n + j], s);
$ git grep -n -F 'neon::gemm(a, b, m, k, n, out)' "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates/contract/src/cpu_ops.rs
a05f0dc119b8538969477cef5e2c9764e01568a6:crates/contract/src/cpu_ops.rs:113:    neon::gemm(a, b, m, k, n, out)
```

(4)

```console
$ gh run list --branch main --event push --commit a05f0dc119b8538969477cef5e2c9764e01568a6 --workflow ci.yml --json databaseId --jq '.[].databaseId'
44722531593
$ gh run view 44722531593 --json conclusion,headSha,jobs --jq '.conclusion, .headSha, (.jobs[]|select(.name|test("^(make verify|make deny|wasm32 build)"))|"\(.name): \(.conclusion)")'
success
a05f0dc119b8538969477cef5e2c9764e01568a6
make deny (cargo-deny): success
wasm32 build + size + node (macos-latest): success
make verify (fmt + clippy + test): success
make verify + build-profiles (macos-latest): success
$ gh run view 44722531593 --json jobs --jq '.jobs[]|select(.name=="make verify (fmt + clippy + test)")|.databaseId'
136241642998
$ gh run view 44722531593 --job 136241642998 --log | awk -F'\t' '$2=="make verify"' | grep -oE 'test (three_inputs_have_the_expected_shapes|lane_reference_differs_from_scalar_and_other_forms|gemm_simd_writes_every_element_of_three_inputs|neon_gemm_simd_matches_scalar_by_t1|neon_gemm_simd_equals_lane_reference_bits) \.\.\. ok' | sort | uniq -c
      1 test gemm_simd_writes_every_element_of_three_inputs ... ok
      1 test lane_reference_differs_from_scalar_and_other_forms ... ok
      1 test three_inputs_have_the_expected_shapes ... ok
$ gh run view 44722531593 --job 136241642998 --log | awk -F'\t' '$2=="make verify"' | awk '/test lane_reference_differs_from_scalar_and_other_forms \.\.\. ok/{k=1} k&&/test result:/{print;exit}' | grep -oE 'test result: [^;]+; [0-9]+ failed; [0-9]+ ignored'
test result: ok. 3 passed; 0 failed; 0 ignored
```

(5)

```console
$ gh run view 44722531593 --json jobs --jq '.jobs[]|select(.name=="make verify + build-profiles (macos-latest)")|.databaseId'
136241643000
$ gh run view 44722531593 --job 136241643000 --log | awk -F'\t' '$2=="make verify"' | grep -oE 'test (three_inputs_have_the_expected_shapes|lane_reference_differs_from_scalar_and_other_forms|gemm_simd_writes_every_element_of_three_inputs|neon_gemm_simd_matches_scalar_by_t1|neon_gemm_simd_equals_lane_reference_bits) \.\.\. ok' | sort | uniq -c
      1 test gemm_simd_writes_every_element_of_three_inputs ... ok
      1 test lane_reference_differs_from_scalar_and_other_forms ... ok
      1 test neon_gemm_simd_equals_lane_reference_bits ... ok
      1 test neon_gemm_simd_matches_scalar_by_t1 ... ok
      1 test three_inputs_have_the_expected_shapes ... ok
$ gh run view 44722531593 --job 136241643000 --log | awk -F'\t' '$2=="make verify"' | awk '/test lane_reference_differs_from_scalar_and_other_forms \.\.\. ok/{k=1} k&&/test result:/{print;exit}' | grep -oE 'test result: [^;]+; [0-9]+ failed; [0-9]+ ignored'
test result: ok. 5 passed; 0 failed; 0 ignored
$ gh run view 44722531593 --job 136241643000 --log | awk -F'\t' '$2=="make verify"' | grep -oE 'test gemm_simd_(follows_lane_order|differs_from_scalar)_on_neon \.\.\. ok' | sort | uniq -c
      1 test gemm_simd_differs_from_scalar_on_neon ... ok
      1 test gemm_simd_follows_lane_order_on_neon ... ok
$ gh run view 44722531593 --job 136241643000 --log | awk -F'\t' '$2=="make verify"' | awk '/test gemm_simd_follows_lane_order_on_neon \.\.\. ok/{k=1} k&&/test result:/{print;exit}' | grep -oE 'test result: [^;]+; [0-9]+ failed; [0-9]+ ignored'
test result: ok. 14 passed; 0 failed; 0 ignored
```

(6)

```console
$ gh run view 44722531593 --json jobs --jq '.jobs[]|select(.name=="wasm32 build + size + node (macos-latest)")|.databaseId'
136241642752
$ gh run view 44722531593 --job 136241642752 --log | awk -F'\t' '$2=="make wasm-verify"' | grep -oE '\[wasm-verify\.sh\] cargo clippy \(crates/contract tests, wasm32-unknown-unknown, (simd128|scalar)\) -D warnings'
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, simd128) -D warnings
[wasm-verify.sh] cargo clippy (crates/contract tests, wasm32-unknown-unknown, scalar) -D warnings
```

(7)

```console
$ gh run view 44722531593 --job 136241643000 --log | awk -F'\t' '$2=="Set up job"' | grep -oE 'Image: [a-z0-9-]+'
Image: macos-26-arm64
```

(8)

```console
$ git diff --stat "939f83f332c362929b6b261a89d904591c5a3206" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- Cargo.toml Cargo.lock deps-approved.toml crates/core/Cargo.toml crates/exec/Cargo.toml crates/upper/Cargo.toml crates/wasm-abi/Cargo.toml wasm measure baseline-wgpu
$ git diff -U0 "939f83f332c362929b6b261a89d904591c5a3206" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates/contract/Cargo.toml | grep '^[-+]' | grep -v -e '^+++ ' -e '^--- '
+[[test]]
+name = "cpu_ops_gemm_neon_rules"
+required-features = ["cpu-ops", "test-support"]
+
$ gh run view 44722531593 --json jobs --jq '.jobs[]|select(.name=="make deny (cargo-deny)")|.databaseId'
136241642615
$ gh run view 44722531593 --job 136241642615 --log | awk -F'\t' '$2=="make deps-check"' | grep -oE '\[deps-check\.sh\] ok: .*'
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
```

(9)

```console
$ git diff --stat "939f83f332c362929b6b261a89d904591c5a3206" "a05f0dc119b8538969477cef5e2c9764e01568a6" -- crates/contract/src crates/core crates/contract/tests/cpu_ops_reference.rs crates/contract/tests/cpu_ops_neon_unsafe_count.rs
$ git show "a05f0dc119b8538969477cef5e2c9764e01568a6":crates/contract/tests/cpu_ops_gemm_neon_rules.rs | grep -c unsafe
0
$ git show "939f83f332c362929b6b261a89d904591c5a3206":crates/contract/src/cpu_ops/neon.rs | grep -c unsafe
3
$ git show "a05f0dc119b8538969477cef5e2c9764e01568a6":crates/contract/src/cpu_ops/neon.rs | grep -c unsafe
3
$ gh run view 44722531593 --job 136241642998 --log | awk -F'\t' '$2=="make verify"' | grep -oE 'test cpu_ops_neon_unsafe_count_matches_expected \.\.\. ok' | sort | uniq -c
      1 test cpu_ops_neon_unsafe_count_matches_expected ... ok
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/2 | 和の順と k の端数の試験 | #6936 |
| 2/2 | 完了の条件の確かめ | #6937 |

## 今の値の補い（base 8a43647a）

記録時点の表と既存の節は書き換えない。以下は base `8a43647a` で確かめた今の値。

「完了の条件の確かめ（2/2）」の節の表 (9) の「`crates` の差」「差は空」が比べた範囲は、コードブロック (9) の 1 本目のコマンドの対象のパス `crates/contract/src`・`crates/core`・`crates/contract/tests/cpu_ops_reference.rs`・`crates/contract/tests/cpu_ops_neon_unsafe_count.rs` の 4 つ。`crates` 全体の差は次のとおりで、`crates/contract/Cargo.toml` と新しい試験のファイルだけ。

```console
$ git diff --stat 939f83f332c362929b6b261a89d904591c5a3206 a05f0dc119b8538969477cef5e2c9764e01568a6 -- crates | cut -c2-
crates/contract/Cargo.toml                       |   4 +
crates/contract/tests/cpu_ops_gemm_neon_rules.rs | 211 +++++++++++++++++++++++
2 files changed, 215 insertions(+)
```
