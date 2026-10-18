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
| 実機の手順のスクリプト | `scripts/metal-session.sh` の `run` は、試験の枠が読む環境変数と開発者ディレクトリ（付けたときだけ検証レイヤーの環境変数と門のセルの識別名）を設定して `--` 以降のコマンドを exec する。ほかに `probe`・`redact`・`scan` を持つ | `scripts/metal-session.sh` の冒頭、[metal-run-procedure](metal-run-procedure.md) |
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

## 母数の表（3/6）

門の run のセル（腕 × 行 × dtype）と、母数に入る件数を `test-support` の裏の `parity` に置く。根拠は TASK-1005・TASK-774・TASK-1008〜TASK-1011 のポインタ（腕の中身は各腕のタスクが持つ）。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/parity/gemm_population.rs`（`GemmGateArm`・`GemmGateCell`・`gate_cells`・`population_cells`・`GEMM_GATE_POPULATION`） | 同ファイルの `//!` |
| 腕と dtype | `g1`・`g2`・`g5`・`g7` は f32 のみ。`g3`・`g3-prime`・`g6`・`g9`・`mps` は f32 と f16acc。母数に入らないのは `mps` だけ | `arms_per_dtype`、`mps_is_outside_population` |
| 件数 | 母数 60（f32 が 40、f16acc が 20）。門の run のセルは 70（`mps` の 10 を含む）。`mps` は数えない | `population_is_sixty`、`population_split_by_dtype`、`mps_is_outside_population` |
| セル | 腕・dtype ごとに 5 行が 1 回ずつ。識別名は `<腕>-<行>-<dtype>`（例 `g6-sq512-f32`）で `TestId` を通る。順は 腕 → 行 → dtype で固定 | `each_arm_covers_all_rows`、`cells_are_unique`、`ids_are_test_ids`、`order_is_fixed` |
| 範囲の外 | 採否の規則と照合の集計は含まない（件数を数えるだけ） | - |

試験は `crates/core/tests/parity_gemm_population.rs`。既定で走り（`make verify`）、GPU の実機は要らない。

## 累積の順の対照（4/6）

同じ入力で、累積の順だけを `k` の降順にした CPU の行列積を作り、昇順の参照と `compare_f32`（`T1`）で比べる。判定には使わない対照で、根拠は TASK-1005 のポインタ。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/parity/cpu_reference.rs` の `reference_gemm_f32_descending_k`、`gemm_inputs.rs` の `reference_for_descending_k` | 各ファイルの `//!`・`///` |
| 振る舞い | `0.0` から `k` の降順に `f32::mul_add` で積み上げる。昇順の参照との違いは累積の順だけ。長さの検査・溢れ・確保の失敗は昇順と同じ順・同じ `ReferenceError`。`reference_gemm_f32` は変えていない | `descending_hand_computed`、`descending_differs_from_ascending_when_order_matters`、`descending_errors_match_ascending` |
| 既定の行 | `odd37x41x53`・`sq512` × `f32`・`f16acc` の 4 セルで、`compare_f32`（`T1`）の外れ 0 件・有限でない値 0 件 | `descending_is_within_t1_on_default_rows` |
| 併記 | 上の 4 セルでも、降順の値はビット列が昇順と違う要素を持つ。T-1 で一致しても、同じ順で積み上げた証拠にはならない | `t1_match_does_not_imply_same_order` |
| 大きな行 | `sq2048`・`sq4096`・`rect4096x1024x4096` × 2 dtype を、行 × dtype ごとの `#[ignore]` の試験にした。外れの件数を出力して記録し、外れ 0 件は要求しない（有限でない値 0 件と比べた件数だけを確かめる） | 下の流し方 |

### 大きな行の実測（本段で流した範囲）

- `sq2048` の `f32` は外れ 3 件、`f16acc` は外れ 397 件だった（`T1` で比べた降順の値。ビット列の違う要素は 4,000,000 余り）。つまり大きな行では、累積の順の違いだけで `T1` の外れが出うる
- `sq4096`・`rect4096x1024x4096` は、素朴な単一スレッドの参照が 1 回の実行（600 秒）に収まらず、本段では流していない。結果は未取得で、合格とは書かない

### 走る場所と流し方

- 既定の試験は `make verify` で走り、GPU の実機は要らない
- 大きな行は `cargo test -p fandhe-silicon-core --features test-support --release --test parity_gemm_k_order -- --ignored --nocapture <試験名>` で 1 つずつ流す（例 `sq2048_f32`）
- 再実行の契機: `cpu_reference.rs`・`gemm_inputs.rs`・`binary16.rs`・`tolerance.rs`・`parity_gemm_k_order.rs` を変えたとき、`docs/spec` の gitlink を進めたとき

### 確かめ（4/6）

| 方法 | 結果 |
| ---- | ---- |
| `cargo test -p fandhe-silicon-core --features test-support --test parity_gemm_k_order` | 5 passed; 0 failed; 6 ignored |
| 同 `--release -- --ignored sq2048` | 2 passed（外れ件数は上のとおり） |

確かめた base は `94408323bccf28bce9f79462aae5c74486d89bd9`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## 門の run の枠（5/6）

門の run の 1 セル（腕 × 行 × dtype）を 1 プロセスで 1 つだけ流す共通の枠。腕の中身（TASK-774・TASK-1008〜TASK-1011）は含まず、本リポの試験は偽の腕（CPU 参照そのもの・値を崩した腕）で枠だけを確かめる。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 所在 | `crates/core/src/parity/gemm_gate.rs`（feature `test-support` の裏）。セルの識別名の解決は `gemm_population.rs` の `GemmGateCell::from_id` | 各ファイルの `//!`・`///` |
| セルの選び方 | 環境変数 `FANDHE_SILICON_GEMM_GATE_CELL` の値を、セルの識別名との完全一致だけで 1 セルへ解決する。未設定・不正（空・UTF-8 でない・64 バイト超・知らない識別名・大文字・前後の空白）は腕を呼ばずに失敗で返す。値はエラー・表示に出さない | `selection_parses_exact_ids`、`selection_errors_do_not_echo_value`、`unselected_does_not_call_arm` |
| 1 プロセス 1 回 | 選択つきの入口は同じプロセスで 2 回目を `AlreadyRanInProcess` で拒否し、腕を呼ばない。この入口を呼ぶ試験はテストのバイナリにつき 1 つだけにする | `second_run_in_process_is_rejected` |
| 投入は 1 回 | 入力と参照は 1 回の実行で 1 回ずつ作り、腕へは `FnOnce` で 1 回だけ渡す。`F32` のセルは f32 の入力、`F16Acc` のセルは binary16 へ 1 回だけ丸めた入力 | `submit_is_called_once`、`input_kind_follows_dtype` |
| 比べる相手 | `T1` の `compare_f32` で比べる。f32 の出力は参照そのもの、binary16 の出力は「参照を binary16 へ 1 回だけ丸めた値」と比べる。`F16Acc` のセルは f32 の出力も受け付ける。`F32` のセルの binary16 の出力・長さが M×N と違う出力は失敗にする | `reference_arm_matches_f32`、`binary16_arm_matches_rounded_reference`、`f32_output_on_f16acc_cell_is_accepted`、`binary16_on_f32_cell_is_error`、`wrong_length_is_error`、`perturbed_arm_is_detected` |
| 結果の欄 | `Comparison`、丸めた参照とビット列が違う要素の数、有限な要素どうしの最大の絶対差（f64）、出力の sha256（要素をリトルエンディアンで並べたバイト列。f32 は 4 バイト、binary16 は 2 バイト）、検証レイヤーの状態、セル。`Display` は 1 行 | `sha256_is_little_endian_bytes`、`display_is_one_line` |
| 検証レイヤー | `MTL_DEBUG_LAYER` と `MTL_SHADER_VALIDATION` が両方ちょうど `"1"` のときだけ有効として結果に残す。それ以外はすべて無効。枠は環境変数を設定しない（起動時に設定されている必要がある） | `validation_setting_requires_both_exact_one`、`validation_state_is_recorded_in_result` |
| 手順のスクリプト | `scripts/metal-session.sh run` の `--validation-layer` が上の 2 つを `1` にし、`--gate-cell <id>` が識別名を渡す。付けないときは 3 つとも取り除く。セルを回すループは呼び出し側が持つ | `scripts/test-metal-session.sh`（`run: validation layer ...`・`run: gate cell ...`・`sync: ...`）、[metal-run-procedure](metal-run-procedure.md) |
| 範囲の外 | 腕の実装、機材の組み合わせの判定（各腕の試験が試験の枠で持つ）、採否の集計、最大の相対差、実機（Mac mini）での実行 | - |

試験は `crates/core/tests/parity_gemm_gate.rs`。既定で走り（`make verify`）、既定の行は `odd37x41x53`・`sq512` だけ。GPU の実機は要らない。大きな行の CPU 参照は単一スレッドで 600 秒を超えうる（4/6 の実測）。

### 確かめ（5/6）

| 方法 | 結果 |
| ---- | ---- |
| `cargo test -p fandhe-silicon-core --features test-support --test parity_gemm_gate` | 20 passed; 0 failed |
| `make test-metal-session` | passed: 292, failed: 0 |
| 実機（Mac mini）での 1 セルの実行 | 未実施（device の経路が無い。合格とは書かない） |

確かめた base は `ca04e4b22f55837d724372ed08017f59ebe8d0b6`、`docs/spec` の gitlink は `86641485a76d52552027eada422e38a5a9ca411d`。

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 値・規則を変える変更の扱いは [parity-tool](parity-tool.md) に従う
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 関連

[parity-tool](parity-tool.md)・[metal-run-procedure](metal-run-procedure.md)・[test-frame](test-frame.md)・[rerun-frame](rerun-frame.md)
