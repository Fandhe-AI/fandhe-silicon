# CUDA の背後実装の upload・download_blocking の用途と範囲の検査（TASK-342）の所在と本リポの現状

TASK-342 の作業のうち、CUDA の背後実装の `MemoryOps::upload`・`download_blocking`・`download_async` が行う用途と範囲の検査の所在と、本リポで観測できる事実を置く。
spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-342。ポインタのみ）。内容・理由・経緯は本書に置かない。

## 所在

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 実装 | `crates/contract/src/backend_cuda.rs` の 1 ファイル | `crates/contract/src/backend_cuda.rs` |
| 関数 | `access_range`・`CudaDevice::range`・`CudaDevice::read` | `grep -n 'fn access_range\|fn range\|fn read' crates/contract/src/backend_cuda.rs` |
| cfg | feature `backend-cuda` かつ `not(target_arch = "wasm32")` | `crates/contract/src/lib.rs` |
| 試験 | 同じファイルの `tests` module の `access_range_checks_the_usage_before_the_range`。device に触らない | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` |
| device の取得・確保 | [cuda-device-request.md](./cuda-device-request.md)（TASK-160） | 同頁 |
| 用途と操作の対応 | [buffer-usage-check.md](./buffer-usage-check.md)（TASK-1796） | 同頁 |

## 用途の検査（1/4）

操作ごとの役割と、確保のときの用途に要る値:

| 操作 | 役割 | 要る用途 |
| ---- | ---- | -------- |
| `upload` | `UploadDestination` | `UPLOAD` |
| `download_blocking`・`download_async` | `ReadbackSource` | `DOWNLOAD` |

`range` が照らす順:

| 順 | 照らすもの | 拒否の値 |
| -- | ---------- | -------- |
| 1 | 別の device のハンドル | `DeviceMismatch` |
| 2 | 払い出しの世代 | `InvalidArgument` |
| 3 | 表にあるか | `InvalidArgument` |
| 4 | 表に記録した用途が役割を満たすか（`access_range`） | `InvalidArgument` |
| 5 | 範囲（`validate_buffer_range`）と `usize` への変換 | `InvalidArgument` |

- 用途の拒否は、範囲を見る前に返り、下位の API（`memcpy_htod`・`memcpy_dtoh`）を呼ばない。
- 文字列は `<操作の名前>: ` の後に共通の型の文字列が続く形。
- 長さ 0 の `upload` と読み戻しも、用途が足りなければ拒む。
- 判定に使うのは表に記録した用途で、ハンドルの `usage()` は使わない。
- device の要る既存の試験は、確保の用途を読み書きに足りる値へ替えた。`pool_device_tests` は `STORAGE | UPLOAD | DOWNLOAD`（`a_different_usage_is_not_reused` の 2 本目は `UPLOAD | DOWNLOAD`）、`teardown_device_tests::prepared()` は `STORAGE | UPLOAD`。

device に触らない試験:

| 試験 | 確かめること |
| ---- | ------------ |
| `access_range_checks_the_usage_before_the_range` | 用途が足りず範囲も外れる入力が用途の拒否になること、空の用途の長さ 0 の読み戻しが拒まれること、過申告の用途で範囲内なら通ること、`u64` があふれる入力が拒まれること |

## 境界の入力の試験（2/4）

`access_range` に device を使わずに値を入れる試験。確保長は 16 を使う。`crates/contract/src/backend_cuda.rs` の `tests` module にあり、前節の表の試験に加えて流れる。

| 試験 | 確かめること |
| ---- | ------------ |
| `access_range_accepts_the_required_bit_and_rejects_every_other_single_bit` | 2 つの役割それぞれで、要る 1 ビットだけと 13 値の和が通ること、空と要るビットを除く 12 値の 1 ビットだけが用途の拒否になること |
| `access_range_range_boundaries_with_a_sufficient_usage` | 用途が足りるとき、終端ちょうど・長さ 0 の終端・終端の 1 つ先・`u64` があふれる入力の通る・拒むが 2 つの役割で同じになること |
| `access_range_reports_the_usage_rejection_before_the_range` | 用途が足りず範囲も外れる入力が、2 つの役割とも操作の名前で始まる用途の拒否になること |
| `access_range_decides_by_the_recorded_usage_only` | ほかの引数を変えず記録した用途だけを替えると、結果が拒否と通るで替わること |

確かめ方:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda
```

## device の要る試験（3/4）

`crates/contract/src/backend_cuda.rs` の `transfer_device_tests` module にある（cfg は `test` かつ feature `native-blocking-wait`）。各試験に `#[ignore]` を付けており、既定では流れない。

| 試験 | 確かめること |
| ---- | ------------ |
| `upload_at_offsets_reads_back_through_download_blocking` | 長さと位置と書く長さの 6 組（終端まで書く組・1 MiB の組を含む）で、書いた内容が全体の読み戻しの該当の範囲に現れ、ほかは 0 のままであること |
| `download_blocking_returns_byte_len_bytes_of_the_written_contents` | 長さ 4097 の確保で、4 組の位置と長さの読み戻しが `byte_len` の長さで書いた内容の該当の範囲と一致すること |
| `upload_past_the_end_or_overflowing_is_rejected_and_leaves_the_contents_unchanged` | 終端ちょうどの upload は通り、終端を越える・`u64` があふれる 4 入力は `InvalidArgument` で、内容が変わらないこと |
| `download_blocking_past_the_end_is_rejected_and_in_range_windows_match` | 終端を越える 3 入力は `InvalidArgument`、範囲内の 4 つの窓（長さ 0 を含む）は書いた内容と一致すること |
| `overflowing_offset_plus_len_is_rejected_by_upload_and_download_blocking` | あふれる入力が upload で 2 つ、download_blocking で 3 つ `InvalidArgument` になり、内容が変わらないこと |
| `missing_usage_is_rejected_and_leaves_the_contents_unchanged` | 読み出しだけ・書き込みだけ・空の用途の確保で、足りない側の操作（長さ 0 を含む）が `InvalidArgument` になり、内容が変わらないこと |
| `the_recorded_usage_decides_even_if_the_handle_claims_more` | 表に記録した用途が `STORAGE` だけなら、読み書きを申告したハンドルの写しでも元のハンドルでも `InvalidArgument` になること |
| `download_async_without_download_usage_calls_back_once_with_invalid_argument` | `DOWNLOAD` の無い確保への `download_async` が、コールバックを 1 回だけ `InvalidArgument` で呼ぶこと |

確かめ方（device の要らない件数の確認。試験は `ignored` と数えられる）:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --lib backend_cuda
```

`33 ignored` の形で通る。`--features backend-cuda` だけの組は `19 ignored` のまま。device の要る試験の流し方:

```sh
cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --lib backend_cuda::transfer_device_tests -- --ignored
```

## 完了の条件の確かめ（4/4）

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-342。ポインタのみ）。

時点のコミット: `d9266d9`

| # | 確かめ | コマンド | 結果 |
| - | ------ | -------- | ---- |
| 1 | 用途と範囲の検査の試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda --lib backend_cuda` | 62 passed; 0 failed; 19 ignored、終了コード 0 |
| 2 | device の要る試験の宣言 | `cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --lib backend_cuda` | 62 passed; 0 failed; 33 ignored、終了コード 0 |
| 3 | 既存の device の要る統合試験 | `cargo test -p fandhe-silicon-contract --features backend-cuda,native-blocking-wait --test backend_cuda_device` | 0 passed; 0 failed; 12 ignored、終了コード 0 |
| 4 | 個数テスト | `cargo test -p fandhe-silicon-contract --test backend_cuda_unsafe_count` | 1 passed、終了コード 0 |
| 5 | 公開 API | `make public-api-sdk-check` | `sdk_type_refs_total=0`、終了コード 0 |
| 6 | 依存の検査 | `make deps-check`、`grep -c '^\[\[approval\]\]' deps-approved.toml` | 終了コード 0、件数 5 |
| 7 | CI | `gh run view 43166167083 --json conclusion,headSha,jobs`（main の push の run） | conclusion success、headSha `d9266d9035a4e79b41917667ad4d23d34e39b283`、`make verify (fmt + clippy + test)` success、`make verify + build-profiles (macos-latest)` success、`make deny (cargo-deny)` success |

(1) の出力。

```text
running 81 tests
test result: ok. 62 passed; 0 failed; 19 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

(2) の出力。

```text
running 95 tests
test result: ok. 62 passed; 0 failed; 33 ignored; 0 measured; 13 filtered out; finished in 0.00s
```

(3) の出力。

```text
running 12 tests
test result: ok. 0 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

(4) の出力。

```text
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

(5) の出力。

```text
record: commit=d9266d9035a4e79b41917667ad4d23d34e39b283 documents=10 public_items_total=13924 sdk_type_refs_total=0 sdk_other_refs_total=0 common_declared_deps=0 common_chip_deps=0
```

(6) の出力。

```text
[deps-check.sh] ok: all dependency requirements are pinned to =x.y.z and match deps-approved.toml
5
```

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 用途の検査 | #4443 |
| 2/4 | 境界の入力の試験 | #4444 |
| 3/4 | device の要る試験 | #4445 |
| 4/4 | 完了の条件の確かめ | #4446 |
