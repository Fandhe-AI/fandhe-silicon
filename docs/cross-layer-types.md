# 層をまたぐ型の受け渡しの試験の所在（TASK-260）

## 記録の所在

private spec 側の記録に従う（ポインタのみ）。`docs/spec/05-tasks.md` の TASK-260。

## 所在

| 項目 | 内容 |
| ---- | ---- |
| 試験 | `crates/upper/tests/cross_layer_types.rs` |
| feature の条件 | `crates/upper/Cargo.toml` の `[[test]]` の `required-features = ["native-blocking-wait"]` |
| 使う公開のパス | `fandhe_silicon_exec::{Exec, OptIn, Outcome, OpId, F16FmaOperands, SubgroupSumU32Operands, SubgroupSumF32Operands, AtomicAddOperands}`（`Exec` の所在は TASK-919）、`fandhe_silicon_upper::{UpperDevice, Buffer}`（TASK-2386）、`fandhe_silicon_core::stub_device::StubDevice`（TASK-156） |

## 層をまたぐ受け渡しの試験（1/2）

試験用の device 1 つを、下-2 の `Exec` と上の層の `UpperDevice` の両方に `&dyn Device` で渡す。
buffer は 16 バイト・用途 `STORAGE` で、`Buffer::from_handle` で包み、`handle_mut()` の値を下-2 の device の形に渡す。

| 試験 | 入力 | 期待 |
| ---- | ---- | ---- |
| `upper_buffers_pass_to_exec_device_operands` | 既定が「許さない」の `Exec`、上書き `None`、4 つの入口を device の形で呼ぶ（`n` は 8、`atomic_add` の `m` は 1） | どれも `Ok(Outcome::Unavailable { report })` で `report.route` は `None`、`report.op` は入口の操作。呼んだ後の各 buffer の中身は埋めた値のまま |
| `override_call_returns_contract_error_type` | 上書き `Some(OptIn { allow_gpu_emulated: false, allow_cpu: true })` で 4 つの入口を呼ぶ | 戻りが `Result<Outcome, ContractError>` で、`Ok(Outcome::Unavailable { .. })` ではない。呼んだ後の各 `Buffer` は大きさ 16・用途 `STORAGE`、読み出しの長さは 16 |
| `exec_report_level_matches_device_query` | 4 つの操作と能力の鍵（`FEATURE_SHADER_F16`・`FEATURE_SUBGROUP`・`FEATURE_SHADER_FLOAT32_ATOMIC`）の組、`query_route(op, None)` | `observed_level` が `query_ex(鍵).level`、`native_reason` が `CapabilityUnsupported(query_ex(鍵).unsupported_reason)`。`f16_fma` の入口の `Unavailable` の `report` が `query_route` の報告と等しい |
| `upper_submit_and_wait_on_the_same_device` | 下-2 の入口を呼んだ後、同じ device から作った `UpperDevice` の `queue()` で空の `CommandBuffer` を `submit` し `wait_blocking(id, None)` | `submit` が `Ok(SubmitId)`、`wait_blocking` が `Ok(())`。下-2 の値も続けて使える |

### 確かめ方

```text
cargo test -p fandhe-silicon-upper --features native-blocking-wait --test cross_layer_types
cargo clippy -p fandhe-silicon-upper --all-targets -- -D warnings
cargo clippy -p fandhe-silicon-upper --all-targets --features native-blocking-wait -- -D warnings
```

## 完了の条件の確かめ（2/2）

2/2（#3995）で、層をまたぐ受け渡しの試験について本リポで観測できる事実だけを記録する。GPU の実機の無い Linux で実行した。件数は実測で、コードと試験は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 試験の所在と feature | `grep -n -A2 'cross_layer_types' crates/upper/Cargo.toml`、`cargo test -p fandhe-silicon-upper`（feature なし） | `Cargo.toml` の 31 行に `name = "cross_layer_types"`、32 行に `required-features = ["native-blocking-wait"]`。feature なしの出力に `Running tests/cross_layer_types.rs` の行は無い（`upper_compute_path` の 7 passed、doctest の 3 passed と 7 passed） |
| (2) | 同じ device を下-2 の値と上の層の device に渡す | `cargo test -p fandhe-silicon-upper --features native-blocking-wait --test cross_layer_types` | 4 本とも `ok`、`4 passed; 0 failed; 0 ignored` |
| (3) | 上の層の buffer を下-2 の device の形に渡し、経路の無い呼び出しの後も中身が変わらない | 同上の `upper_buffers_pass_to_exec_device_operands` | `ok` |
| (4) | 失敗の型と能力の値 | 同上の `override_call_returns_contract_error_type`・`exec_report_level_matches_device_query` | 2 本とも `ok` |
| (5) | 同じ device の上の投入と待ち | 同上の `upper_submit_and_wait_on_the_same_device` | `ok` |
| (6) | 依存 0・unsafe 0・ソースの変更 0 | 1/2 のコミット `0e29bc0` の `Cargo.lock`・`deps-approved.toml`・ルートの `Cargo.toml`・`crates/upper/src`・`crates/exec`・`crates/core` の差分の行数、同コミットの `crates/upper/Cargo.toml` の差分、`make upper-layer-check`、`make exec-deps-check`、`make deps-check` | 指定パスの差分は 0 件。`crates/upper/Cargo.toml` の差分はコメント 1 行と `[[test]]` の 3 行（空行を含めて 5 行）。`record:` の行は `rs_files=9 direct_deps=3 forbid=yes unsafe_total=0 chip_deps=0 chip_paths=0 other_deps=0`、`ok:` で終了。`make exec-deps-check` は `ok:`（`other_deps=0 dep_source=0 tree_forbidden=0 source_hits=0`）、`make deps-check` は `ok` |
| (7) | CI で流れる場所 | `make verify`、`make build-profiles`、`make wasm-verify` | `make verify` は終了コード 0 で `cross_layer_types` は `4 passed; 0 failed; 0 ignored`（全体は `test result:` 218 行、2221 passed・0 failed・7 ignored）。`make build-profiles` は終了コード 0 で、`native-blocking-wait` の upper の test に `cross_layer_types` の `4 passed; 0 failed`。`make wasm-verify` は終了コード 0 で、upper の試験の wasm32 の `cargo check` が feature なしと `native-blocking-wait` の 2 組とも通る |

確かめた時点は main の `5bf9d3f293dc31ea42bc72c80a042806971985c2`（`docs/spec` の gitlink は `c7887a1a7f75903d4016ba42e0595ae6cbb2cd0b`）。

追加で流したコマンドと結果。

```text
make check                       終了コード 0
make deny                        終了コード 0
make upper-source-check          ok: upper source scan passed
make profile-test-count-check    profile-test-count-check: all profiles passed
```

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-260）。

## 段の表

| 段 | Issue | 題 |
| -- | ----- | -- |
| 1/2 | #3994 | test(upper): TASK-260 の 1/2 上の層の buffer と device を下-2 に渡す試験 |
| 2/2 | #3995 | docs: TASK-260 の 2/2 完了の条件の確かめ |

## 今の値の補い（base 2f36d3d）

上の節の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 所在の「使う公開のパス」 | 表に載る公開のパスは `Exec` ほか 8 つ・`UpperDevice`・`Buffer`・`StubDevice` → 試験の `use` には表に無い公開のパスもある。`fandhe_silicon_exec::UnavailableReason`（試験の 15 行目）、`fandhe_silicon_upper::SubmitId`（17 行目）、`fandhe_silicon_core::memory::BufferHandle`（11 行目。下の handle の照らしが使う） | `sed -n 5,17p crates/upper/tests/cross_layer_types.rs` |
| 層をまたぐ受け渡しの試験（1/2）の `upper_buffers_pass_to_exec_device_operands` | 期待の欄に handle の番地の比べは無い → 下-2 の入口を呼ぶ前に、`BUFFERS`（10）個の buffer ぶんの `handle()` の番地と `handle_mut()` の番地を集め、順に等しいことを `assert_eq!` で確かめる（108〜113 行目）。確かめるのは `Buffer::handle_mut()` が `handle()` と同じ実体を指すことまで。試験は 4 本のまま | `grep -n 'owned\|lent' crates/upper/tests/cross_layer_types.rs`、`cargo test -p fandhe-silicon-upper --features native-blocking-wait --test cross_layer_types` |
| 完了の条件の確かめ（2/2）の (2) | 4 本とも `ok`、`4 passed; 0 failed; 0 ignored` → 同じ（4 passed）。この試験が確かめるのは、同じ device を下-2 の値と上の層の device（`UpperDevice::new`）に渡して作れることまで。2 つが同じ device を持つことは比べていない。上の層の device は作るだけである（`upper_buffers_pass_to_exec_device_operands` は `_upper` に束ねるだけで使わない。`upper_submit_and_wait_on_the_same_device` は上の層の queue で投入と待ちをするが、下-2 の値の device との同一は比べない） | `grep -n 'UpperDevice::new\|queue()' crates/upper/tests/cross_layer_types.rs`（104・212・216 行目） |
| 完了の条件の確かめ（2/2）の (7) | `make verify` の `test result:` は 218 行、2221 passed・0 failed・7 ignored → 221 行、2234 passed・0 failed・7 ignored。`cross_layer_types` は 4 passed; 0 failed; 0 ignored のまま | `make verify > <ログ> 2>&1`、`grep -c '^test result:' <ログ>`、`grep '^test result:' <ログ> \| awk '{p+=$4; f+=$6; i+=$8} END{print p,f,i}'` |
| 完了の条件の確かめ（2/2）の (1) と (6) | `Cargo.toml` の 31・32 行目、feature なしで `Running tests/cross_layer_types.rs` が出ない、`record:` の `rs_files=9 direct_deps=3` → どれも同じ | `grep -n -A2 'cross_layer_types' crates/upper/Cargo.toml`、`cargo test -p fandhe-silicon-upper`、`make upper-layer-check` |
