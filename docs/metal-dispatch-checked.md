# Metal の投入の前の検査の上書き（TASK-436）の所在と本リポの現状

TASK-436 の作業のうち、Metal の背後実装が `dispatch_checked` を上書きし、バッファの実際の長さで引数の宣言を照らす部分の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-436。ポインタのみ）。private spec 側の記録に従う（ポインタのみ）。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 上書き | `crates/contract/src/backend_metal.rs` の `impl CommandQueue for MetalDevice` の `dispatch_checked`。`Device::queue()` から届く | `grep -n 'fn dispatch_checked' crates/contract/src/backend_metal.rs` |
| ハンドルの照合 | 非公開の `check_handle_record`（device に触らない純粋関数） | `grep -n 'fn check_handle_record' crates/contract/src/backend_metal.rs` |
| 宣言の照らし | 非公開の `check_declared_operands`（device に触らない純粋関数） | `grep -n 'fn check_declared_operands' crates/contract/src/backend_metal.rs` |
| Linux の試験の module | `dispatch_checked_tests`（どの OS でも流れる） | `grep -n 'mod dispatch_checked_tests' crates/contract/src/backend_metal.rs` |
| device の要る試験（単体） | `imp::device_tests::dispatch` の `#[ignore]` 付きの 7 本 | `grep -n -A1 '#\[ignore' crates/contract/src/backend_metal.rs \| grep 'fn dispatch_checked'` |
| device の要る試験（公開の口） | `crates/contract/tests/backend_metal_device.rs` の `#[ignore]` 付きの 4 本 | `grep -n -A1 '#\[ignore' crates/contract/tests/backend_metal_device.rs \| grep 'fn dispatch_checked'` |
| cfg | feature `backend-metal`。上書きは `imp`（macOS だけ）の中。2 つの純粋な関数と `dispatch_checked_tests` はどの OS でも流れる | `crates/contract/src/lib.rs` |

## 上書きの振る舞い（1/5）

`dispatch_checked` は、引数の宣言を C8、C7、C4、C5、C6 の順に照らし、通ったら `dispatch_ex` を 1 回呼んでその結果を返す。

- C4 の容量は束縛のバッファの実際の長さ。上限は `limits()` の `max_buffer_bytes`。
- C8・C7 の違反は、ハンドルを表で引かずに返す。
- C4 の段で、束縛のハンドルを並びの順に照らす。別の device のハンドルは `DeviceMismatch`、表に無いハンドルと、世代または大きさが表の項目と違うハンドルは `InvalidArgument` で返す。どちらも `dispatch_ex` は呼ばない。
- `check_handle_record` は、世代、大きさの順に見る。通れば `buffer_len` を返す。
- `check_declared_operands` は、容量の取得が `Err` を返した束縛の `Err` をそのまま返す。照らしが通ったときは、まだ引いていない位置もすべて引いてから `Ok(())` を返す。

## 宣言の照らしの関数の試験（2/5）

`dispatch_checked_tests` の 21 件。device に触らず、確かめ方は次のとおり。

```text
cargo test -p fandhe-silicon-contract --features backend-metal --lib dispatch_checked_tests
```

| 試験 | 入力と期待 |
| ---- | ---------- |
| `zero_length_dimension_is_rejected_before_any_lookup` | 長さ 0 の次元は、容量の取得を呼ばずに拒む |
| `overflowing_declaration_is_rejected_by_c8_without_any_lookup` | あふれる宣言は C8 で拒み、容量の取得を呼ばない |
| `slot_mistakes_are_rejected_by_c7_without_any_lookup` | slot の誤りは C7 で拒み、容量の取得を呼ばない |
| `declaration_larger_than_the_buffer_length_is_rejected_by_c4` | バッファの長さを超える宣言は C4 で拒む |
| `declaration_one_byte_over_the_limit_is_rejected_and_the_limit_passes` | 上限を 1 バイト超えると C8、上限ちょうどは通る |
| `offset_counts_against_the_capacity` | offset は容量に数える |
| `misaligned_offset_is_rejected_by_c5_after_every_binding_is_looked_up` | 整列に合わない offset は、全束縛を引いた後に C5 で拒む |
| `constant_size_mismatch_is_rejected_by_c6` | 定数の大きさの不一致は C6 で拒む |
| `c8_comes_before_a_lookup_error` | C8 は取得の失敗より先に返る |
| `c7_comes_before_a_lookup_error` | C7 は取得の失敗より先に返る |
| `a_lookup_error_wins_over_c5` | 取得の失敗は C5 より先に返る |
| `c4_stops_the_lookups_at_the_first_violation` | C4 の最初の違反で取得を止める |
| `the_first_lookup_error_wins_over_a_later_c4` | 最初の取得の失敗は、後の C4 より先に返る |
| `lookup_errors_are_returned_as_they_are` | 取得の失敗の `Err` をそのまま返す |
| `lookups_follow_the_binding_order_not_the_slot_order` | 取得は束縛の並びの順で、slot の順ではない |
| `every_binding_is_looked_up_once_when_the_declaration_passes` | 宣言が通るとき、各束縛を 1 回ずつ引く |
| `extreme_values_return_a_result_without_panicking` | 極端な値でも panic せず結果を返す |
| `handle_record_returns_the_length_when_generation_and_size_match` | 世代と大きさが合えば長さを返す |
| `handle_record_rejects_a_different_generation` | 世代が違えば拒む |
| `handle_record_rejects_a_larger_size` | ハンドルの大きさが長さより大きければ拒む |
| `handle_record_rejects_a_smaller_size` | ハンドルの大きさが長さより小さければ拒む |

## device の要る試験（3/5・4/5）

どれも `#[ignore = "touches the Metal device; run only in a dedicated device session"]` 付き。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `dispatch_checked_rejects_a_zero_length_gemm_before_looking_up_handles` | 長さ 0 の次元の宣言で、表に無いハンドルを束縛 | C8 で拒む |
| `dispatch_checked_rejects_a_declaration_larger_than_the_buffer` | バッファの実際の長さを超える宣言 | C4 で拒む |
| `dispatch_checked_rejects_one_byte_over_the_limit_and_runs_at_the_limit` | 上限を 1 バイト超える宣言と、上限ちょうどの宣言 | 前者は C8、後者は通る |
| `dispatch_checked_rejects_handles_that_do_not_match_the_table` | 表の項目と合わないハンドル | 通る宣言でも `dispatch_ex` の前に拒む |
| `dispatch_checked_rejects_c5_c6_c7_and_c8_before_recording` | C5・C6・C7（4 通り）・C8 の入力 | 記録の前に拒み、記録は増えない |
| `dispatch_checked_passes_an_exact_declaration_to_the_dispatch` | ちょうどの宣言 | `dispatch_ex` に渡り、投入した出力が入力と一致する |
| `dispatch_checked_accepts_fresh_and_reused_allocations` | 新しい確保と、解放後に再利用した確保のハンドル | その大きさの宣言で通る |
| `dispatch_checked_through_the_public_device_rejects_one_byte_over_the_limit` | `queue()` 越しで、`max_buffer_bytes` より 1 大きい宣言と、ちょうどの宣言 | 前者は `C8:` で拒み、後者は通る |
| `dispatch_checked_through_the_public_device_uses_the_buffer_length` | `queue()` 越しで、束縛のバッファの長さと、大きさが長さと違うハンドル | 実際の長さで容量を照らし、大きさが違えば拒む |
| `dispatch_checked_through_the_public_device_rejects_a_zero_length_gemm_first` | `queue()` 越しで、行数 0 の GEMM の宣言 | ハンドルを表で引く前に `C8:` で拒む |
| `dispatch_checked_through_the_public_device_rejects_another_devices_buffer` | `queue()` 越しで、別の device のバッファの束縛と、同じ device のバッファの束縛 | 前者は `DeviceMismatch`、後者は通る |

## 完了の条件の確かめ（5/5）

5/5（#4793）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-436）。GPU を使わない Linux で、下の base で実行した値を書く。

時点のコミットは `f433877`、`docs/spec` の gitlink は `5f6d802c905938bba1ed963340029d61fe3651a0`。1/5 の親のコミットは `971b1db`。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| 1 | 上書きがあり、`Device::queue()` から届く | `grep -n 'fn dispatch_checked' crates/contract/src/backend_metal.rs`・1/5 の PR の CI の run `43186522137`（`gh run view 43186522137 --json conclusion,jobs`） | 上書きの宣言は `fn dispatch_checked(` の 1 件。run の conclusion は `success` で、`make verify + build-profiles (macos-latest)` のジョブも `success` |
| 2 | 宣言の照らしとハンドルの照合の振る舞い | `cargo test -p fandhe-silicon-contract --features backend-metal --lib dispatch_checked_tests` | `running 21 tests`、`test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out` |
| 3 | device の要る試験の宣言 | `grep -cE '^\s*#\[ignore' crates/contract/src/backend_metal.rs`・`grep -cE '^\s*#\[ignore' crates/contract/tests/backend_metal_device.rs`・`grep -n -A1 '#\[ignore' <各ファイル> \| grep 'fn dispatch_checked'` | `backend_metal.rs` は 62 件（`971b1db` では 55）、`backend_metal_device.rs` は 59 件（`971b1db` では 55）。`fn dispatch_checked…` は `backend_metal.rs` に 7 件、`backend_metal_device.rs` に 4 件で、名前は上の表のとおり |
| 4 | unsafe 0・依存 0 | `cargo test -p fandhe-silicon-contract --features backend-metal --test backend_metal_unsafe_count --test backend_metal_no_recording`・`Cargo.lock`・`deps-approved.toml`・`crates/contract/Cargo.toml` の `971b1db` から base までの差分の stat・同じ範囲の `crates/contract/src` の追加行のうち `unsafe` を含む行の件数 | `running 6 tests` と `running 1 test` が、どちらも `test result: ok`（6 passed、1 passed）。3 ファイルの差分は空。`unsafe` を含む追加行は 0 件 |
| 5 | CI で流れる場所 | `make verify`（`cargo test --workspace --all-features`） | 終了コードは 0。`dispatch_checked_tests` の 21 件が ok |
| 6 | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |

## 段

| 段 | 内容 | Issue | PR |
| -- | ---- | ----- | -- |
| 1/5 | `dispatch_checked` を上書きして束縛のハンドルとバッファの長さで引数の宣言を照らす | #4789 | #8240 |
| 2/5 | 宣言の照らしの関数に検査ごとの拒否と順の試験を足す | #4790 | #8241 |
| 3/5 | `dispatch_checked` の device の要る試験を `#[ignore]` 付きで足す | #4791 | #8242 |
| 4/5 | 公開の device の口から `dispatch_checked` の device の要る試験を `#[ignore]` 付きで足す | #4792 | #8243 |
| 5/5 | 完了の条件の確かめ | #4793 | 本頁 |
