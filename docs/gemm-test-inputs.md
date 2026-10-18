# GEMM の試験の入力と参照（TASK-1005）の所在と本リポの現状

TASK-1005 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-1005。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-24 | `crates/core/src/parity/cpu_reference.rs`・`tolerance.rs` | [parity-tool](parity-tool.md) |
| TASK-19 | `scripts/metal-session.sh`・`scripts/test-metal-session.sh` | [metal-run-procedure](metal-run-procedure.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| CPU 参照（f32） | `reference_gemm_f32` は f32・行優先の行列積。各要素を `0.0` から `k` の昇順に `f32::mul_add` で積み上げる。長さの不一致・次元の積の溢れ・確保の失敗は `ReferenceError` で返す | `crates/core/src/parity/cpu_reference.rs` の rustdoc、`crates/core/tests/parity_cpu_reference.rs` |
| 比べ方 | `T1` は `Tolerance` の定数で、値は `relative()`・`absolute()` で読む。`compare_f32` は `Comparison`（`compared`・`exceeded`・`non_finite`・`first_mismatch`・`is_match`）を返す。数値は本書に持たない | `crates/core/src/parity/tolerance.rs`、`crates/core/tests/parity_tolerance.rs` |
| ハッシュ | `core` の `hash` は feature の条件なしで公開され、`sha256_bytes`・`sha256_u64` を持つ | `crates/core/src/hash.rs`、`crates/core/tests/hash_known_values.rs`・`hash_shasum.rs` |
| 試験の枠・流し直し | `test_frame`・`rerun` は `test-support` の裏にある | [test-frame](test-frame.md)、[rerun-frame](rerun-frame.md) |
| feature の転送 | `contract` の `test-support` は `fandhe-silicon-core/test-support` へ転送する | `crates/contract/Cargo.toml`、`crates/contract/tests/test_support_forwarding.rs` |
| Metal の背後実装 | `crates/contract/src/backend_metal.rs` は 6 行で、`//!` の説明だけを持つ骨格。チップのバインディング crate への依存は無い | `wc -l crates/contract/src/backend_metal.rs` |
| 実機の手順のスクリプト | `scripts/metal-session.sh` の `run` は、試験の枠が読む環境変数と開発者ディレクトリを設定して `--` 以降のコマンドを exec する。ほかに `probe`・`redact`・`scan` を持つ | `scripts/metal-session.sh` の冒頭、[metal-run-procedure](metal-run-procedure.md) |
| 依存・unsafe | `core` の外部依存は 0。`#![forbid(unsafe_code)]`。`deps-approved.toml` の承認は 0 件 | `crates/core/Cargo.toml`、`crates/core/src/lib.rs`、`deps-approved.toml` |
| 走る場所 | `core` の `parity_*` は `make verify`（`--all-features`）で走る。`contract` の `test_support_*` は `make build-profiles` の `test-support` を含む組でも走る。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |

## 入力と参照（2/6）

門の run に渡す入力と CPU 参照を、`test-support` の裏の `parity` に置く。生成物はファイルに置かず、その場で決定的に作る。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/parity/binary16.rs`（変換）・`gemm_inputs.rs`（行・入力・行単位の参照）。`reference_gemm_f16acc` は `cpu_reference.rs` | 各ファイルの `//!` |
| 行 | `GemmGateRow::ALL` は 5 行（`sq512`・`sq2048`・`sq4096`・`rect4096x1024x4096`・`odd37x41x53`）。`name`・`m`・`n`・`k`・`a_len`・`b_len`・`a_offset`・`b_offset` を持つ | `crates/core/tests/parity_gemm_inputs.rs` の `row_table_is_consistent` |
| 乱数 | `SplitMix64`（固定 seed `GEMM_INPUT_SEED`）。`skip(n)` は `n` 回の `next_u64` と同じ状態へ進む。値は `[-1, 1)` の 2^-23 の倍数で `-0.0` は出ない | `splitmix64_matches_published_reference_values`・`skip_equals_repeated_next`・`small_rows_are_fixed` |
| 入力の dtype | `GemmInputDtype::F32` は A・B とも f32。`F16Acc` は f32 の入力から binary16 へ 1 回だけ最近接偶数で丸めた値（累積は f32） | `small_rows_are_fixed`（要素ごとの 1 回丸めと sha256） |
| binary16 の変換 | `f32_to_binary16_bits` は最近接偶数・非正規化数・零の符号・桁あふれ（無限大）・NaN を扱う。`binary16_bits_to_f32` は正確な拡大。スライス版は確保の失敗を `ReferenceError::AllocationFailed` で返す | `crates/core/tests/parity_binary16.rs` |
| CPU 参照 | `reference_gemm_f16acc` は広げてから `reference_gemm_f32` に通す。`reference_for` が行・dtype から参照を返し、`rounded_to_binary16` で参照を 1 回だけ丸める。参照は `compare_f32`（`T1`）にそのまま渡せる。内部でキャッシュはしないので、行ごとに 1 回作って持ち回るのは呼び出し側 | `crates/core/tests/parity_gemm_reference.rs` |
| 大きな行 | `sq2048` 以上の入力の固定は無視つきの試験。参照を実際に作る流れは本段の外 | 下の流し方 |

### 走る場所と流し方

- 既定の試験（`odd37x41x53`・`sq512`）は `make verify` で走る
- 大きな行の試験は `#[ignore]` で、手元で `cargo test -p fandhe-silicon-core --features test-support --release --test parity_gemm_inputs -- --ignored` を流す
- sha256 は、本リポのコードの出力を既知の値として固定している。生成の式・位置・丸めを変えたら試験の値も直す

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 値・規則を変える変更の扱いは [parity-tool](parity-tool.md) に従う
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 関連

[parity-tool](parity-tool.md)・[metal-run-procedure](metal-run-procedure.md)・[test-frame](test-frame.md)・[rerun-frame](rerun-frame.md)
