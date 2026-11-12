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

## 段

| 段 | 内容 | Issue |
| -- | ---- | ----- |
| 1/4 | 用途の検査 | #4443 |
| 2/4 | 境界の入力の試験 | #4444 |
