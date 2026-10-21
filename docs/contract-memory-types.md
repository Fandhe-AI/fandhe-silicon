# バッファ・メモリ操作の公開の型（TASK-308）の所在と本リポの現状

TASK-308 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-308。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 文書 |
| ---------------- | ---------- | ---- |
| TASK-4 | workspace と 4 crate の骨格（ルート `Cargo.toml` の `members`） | [workspace-boundary](workspace-boundary.md) |
| TASK-10 | `scripts/public-api-check.sh`（`make public-api-check`。CI の `deny` ジョブが呼ぶ） | [public-api-wgpu-check](public-api-wgpu-check.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | [test-frame](test-frame.md) |
| TASK-136 | `crates/core/src/capability.rs` | [contract-device-types](contract-device-types.md) |
| TASK-269 | `crates/core/src/error.rs` | [contract-error-types](contract-error-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| メモリの置き場 | `crates/core/src/memory.rs` はあるが、中身はバッファの引数検査の純粋関数だけ。持ち主は TASK-309（[buffer-range-check](buffer-range-check.md)） | `cat crates/core/src/memory.rs` |
| 足りない型 | バッファの用途・ハンドル・メモリ操作のトレイトに当たる公開の型は `crates/`・`wasm/`・`measure/`・`baseline-wgpu/` に無い | `git grep -n -E 'pub (trait\|struct\|enum) [A-Za-z]*(Buffer\|Memory)' -- crates wasm measure baseline-wgpu`（0 件） |
| 本書の対象の言及 | TASK-308 の言及は本書、`CLAUDE.md`、`README.md` のほかは `crates/core/src/execution/dispatch_check.rs` のポインタ（背後実装へ渡す束縛の型の持ち分）だけ | `git grep -n TASK-308 -- . ':!docs/spec' ':!.agents'` |
| エラー側の型 | `ContractError`（`InvalidArgument` を含む）は `crates/core/src/error.rs` にある。TASK-269 の持ち分 | `grep -n 'InvalidArgument' crates/core/src/error.rs` |
| device 側 | device の識別子・世代を表す専用の型は無い（`DeviceLost` にフィールドがあるだけ）。`limits()` も無い（TASK-157 の持ち分） | `grep -n 'limits' crates/core/src/capability.rs` |
| `contract` の公開面 | 公開モジュールは feature の裏の `backend_*` と `doc(hidden)` の `cfg_gate_doctest` だけ。契約の共通の公開型は無い | `grep -n 'pub mod' crates/contract/src/lib.rs` |
| `compile_fail` の既存の形 | `crates/contract/src/cfg_gate_doctest.rs`・`crates/contract/tests/feature_gate_enabled.rs`・`crates/core/src/parity/tolerance.rs`・`pixel.rs` に前例がある。コメントのとおり stable の rustdoc は失敗の理由を照合しないので、通る例を並べて import の正しさを担保している | `git grep -ln compile_fail -- crates` |
| `core` の形 | 外部依存 0。`#![forbid(unsafe_code)]`。feature は `test-support`・`fault-injection`。wasm32 でもビルドされる | `crates/core/Cargo.toml`、`scripts/build-profiles.sh`、`scripts/wasm-verify.sh` |
| `exec`・`upper` の現状 | `Cargo.toml` と `src/lib.rs` だけの骨格 | `git ls-files crates/exec crates/upper` |
| 公開 API の検査 | `scripts/public-api-check.sh` は workspace の lib crate を全部、2 組（production / all-features）で走査する。どの crate に型を足しても対象に入る | `scripts/public-api-check.sh` |
| 依存の承認 | `deps-approved.toml` の承認は 0 件 | `grep -c '^\[\[' deps-approved.toml` |
| 走る場所 | `make verify` は workspace の全 feature で試験を流すので doctest も走る。GPU の実機は要らない | `scripts/verify.sh` |

## 段の表

| 段 | イシュー | 内容 |
| -- | -------- | ---- |
| 1/6 | #4283 | 現状と前提の確認（本書） |
| 2/6 | #4284 | 実装 |
| 3/6 | #4285 | テスト |
| 4/6 | #4286 | テスト |
| 5/6 | #4287 | テスト |
| 6/6 | #4288 | 確かめ（本書の「完了の条件の確かめ（6/6）」） |

## 現状の形（2/6）

型は `crates/core` に置く（外部依存 0・`forbid(unsafe_code)` のまま）。置き場と形は TASK-308 の決定の記録に従う（ポインタのみ）。
1/6 の「本リポの現状」の表の「足りない型」「`core` の形」の行は、この節の時点では次のとおり変わった。

| 型 | 置き場 | 値・フィールド | `doc(hidden)` の口 |
| -- | ------ | -------------- | ------------------ |
| `BufferUsage` | `crates/core/src/memory/buffer.rs` | 非公開の `u32`。定数 7 つ（`STORAGE`・`UPLOAD`・`DOWNLOAD`・`VERTEX`・`INDEX`・`UNIFORM`・`EXPORTABLE`）、`empty`・`is_empty`・`bits`・`contains`、`BitOr`・`BitOrAssign`。`Debug`・`Clone`・`Copy`・`PartialEq`・`Eq`・`Hash` | なし |
| `BufferHandle` | 同上 | 非公開の 3 フィールド（世代・識別子・device の識別子）。`Debug` のみ（`Clone`・`Copy` なし） | `__backend_new`・`__backend_generation`・`__backend_id`・`__backend_device_id` |
| `MemoryOps`（トレイト） | `crates/core/src/memory/ops.rs` | `allocate`・`upload`・`download_blocking`・`download_async`・`free`・既定の実装つきの `pending_release_count`。オブジェクト安全 | なし |

- core に feature `native-blocking-wait` を足し、`contract` の同名の feature から転送する。`download_blocking` はこの feature の裏にある
- `download_async` は wasm32 とそれ以外で、コールバックの `Send` の境界だけが違う
- 外部の crate は足していない
- 対応するテスト: `crates/core/tests/memory_types.rs`（構築と呼び出しだけを見る最小の試験）
- 確かめ方: `cargo test -p fandhe-silicon-core --test memory_types`（`--features native-blocking-wait` 付きでも流す）
- 残る段: 3/6〜6/6

## 試験（3/6）

本体のコード（`src/`）は変えず、試験だけを足した。

| 確かめること | 事実 |
| ------------ | ---- |
| ビット値 | `STORAGE`・`UPLOAD`・`DOWNLOAD`・`VERTEX`・`INDEX`・`UNIFORM`・`EXPORTABLE` の `bits()` が順に `1 << 0` から `1 << 6` |
| 互いに重ならない | 各値は 1 ビットだけで、7 値の和は `0x7F` |
| 組み合わせ | 空でない 127 通りで、`bits` と `contains` が合う |
| `allocate` の受け付け | 試験用の最小の実装を `&dyn MemoryOps` 越しに呼ぶと、127 通りと 7 値の単独のすべてで `Ok` が返り、受け取った `usage` が渡した値と等しい |
| 陽性対照 | 期待を 1 つずらした表を、照合が検出する |

- 対応するテスト: `crates/core/tests/memory_usage_values.rs`（`usage_constants_have_defined_bits`・`usage_constants_are_single_disjoint_bits`・`usage_all_combinations_round_trip`・`allocate_accepts_every_combination_through_dyn`・`allocate_accepts_each_single_usage`・`mismatch_check_detects_wrong_expectation`）
- 確かめ方: `cargo test -p fandhe-silicon-core --test memory_usage_values`（`--features native-blocking-wait` 付きでも流せる）
- 背後実装（Metal / Vulkan / CUDA）の `allocate` が実際に受け付けることは、この試験の範囲の外
- 残る段: 4/6〜6/6

## 試験（4/6）

`crates/core` の `src/` は変えず、`crates/contract/src/` に doctest だけのモジュール `memory_free_doctest`（`doc(hidden)`・feature `test-support` の裏）を足した。
1/6 の表の「`contract` の公開面」の行は、この節の時点で `doc(hidden)` のモジュールが 1 つ増えた。

| 確かめること | 事実 |
| ------------ | ---- |
| 2 回目の `free` | 同じ `BufferHandle` の変数に `MemoryOps::free` を 2 回呼ぶコードは、値の move の後の使用（E0382）でコンパイルできない（`compile_fail`） |
| 通る対照 | 2 回目の `free` の有無だけが違う、1 回だけ呼ぶ例は通る |
| `Clone`・`Copy` | `BufferHandle` に `clone()` を呼ぶ例（E0599）と `Copy` の境界を課す例（E0277）はコンパイルできない |

- 実装は書かず、`&dyn MemoryOps` を受ける関数で確かめる。背後実装（Metal / Vulkan / CUDA）には依らない
- stable の rustdoc は `compile_fail` のエラーコードを照合しないため、通る対照を対にしている
- 対応するテスト: `crates/contract/src/memory_free_doctest.rs` の doctest
- 確かめ方: `cargo test -p fandhe-silicon-contract --no-default-features --features test-support --doc`
- 実行時の二重解放の検査は、この試験の範囲の外
- 残る段: 5/6〜6/6

## 試験（5/6）

`crates/core` の `src/` は変えず、試験だけを足した。

| 確かめること | 事実 |
| ------------ | ---- |
| 既定の値 | `pending_release_count` を上書きしない最小の実装は、直接・`&dyn MemoryOps` 越し・総称の関数越しのどれでも `None` を返す |
| 操作の前後 | `allocate`・`free` の前後でも `None` のまま |
| `Some(0)` との区別 | `Some(0)` を返すよう上書きした実装と比べると、値が違い、分類も「観測できない」と「0 個」で分かれる |
| 陽性対照 | 分類の関数が `Some(0)` を「観測できない」と見なさない |

- 対応するテスト: `crates/core/tests/memory_pending_release.rs`（`default_impl_returns_none_direct`・`default_impl_returns_none_through_dyn`・`default_impl_returns_none_through_generic`・`default_impl_stays_none_after_allocate_and_free`・`default_none_is_distinct_from_some_zero`・`override_reports_some_values`・`classify_detects_wrong_expectation`）
- 確かめ方: `cargo test -p fandhe-silicon-core --test memory_pending_release`（`--features native-blocking-wait` 付きでも流せる）
- 背後実装（Metal / Vulkan / CUDA）が返す実際の値と、実行時の解放の振る舞いは、この試験の範囲の外
- 残る段: 6/6

## 完了の条件の確かめ（6/6）

6/6（#4288）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-308）。結果は GPU の実機なしで実行した値で、件数は実測。本体のコード（`src/`）と試験は変えていない。

### 成果物と置き場の対応

| 成果物 | 置き場 | 本書の節 |
| ------ | ------ | -------- |
| 型の実装 | `crates/core/src/memory.rs`・`crates/core/src/memory/buffer.rs`・`crates/core/src/memory/ops.rs` | 現状の形（2/6） |
| 型の試験 | `crates/core/tests/memory_types.rs`・`memory_usage_values.rs`・`memory_pending_release.rs` | 現状の形（2/6）・試験（3/6）・試験（5/6） |
| `compile_fail` の doctest | `crates/contract/src/memory_free_doctest.rs`（`doc(hidden)`・`test-support` の裏） | 試験（4/6） |

- 置き場が `core` であることは TASK-308 の決定の記録に従う（ポインタのみ）

### 確かめ

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| B1 | feature なしの統合テスト | `cargo test -p fandhe-silicon-core --test memory_types --test memory_usage_values --test memory_pending_release` | `memory_types` 3 件・`memory_usage_values` 6 件・`memory_pending_release` 7 件が passed。0 failed・0 ignored |
| B2 | `native-blocking-wait` 付き | B1 に `--features native-blocking-wait` を付ける | 同じ 3・6・7 件が passed |
| B3 | `--all-features` | B1 に `--all-features` を付ける | 同じ 3・6・7 件が passed |
| B4 | `required-features` の宣言 | `crates/core/Cargo.toml` の `[[test]]` | 3 本とも `[[test]]` の宣言が無く、feature なしでも走る |
| B5 | `compile_fail` の doctest | `cargo test -p fandhe-silicon-contract --no-default-features --features test-support --doc memory_free_doctest` | 4 件（通る例 1・`compile_fail` 3）が passed。0 failed |
| B6 | doctest を feature の組み合わせで流す | `--features test-support,native-blocking-wait` と `--all-features` で B5 を流す | どちらも同じ 4 件が passed |
| B7 | `test-support` なしの陰性対照 | `cargo test -p fandhe-silicon-contract --no-default-features --doc memory_free_doctest` | 集まる件数が 0（モジュールが `cfg(feature = "test-support")` の裏にある） |
| C1 | 置き場と制約 | `crates/core/src/lib.rs`・`cargo tree -p fandhe-silicon-core -e normal --depth 1`・`deps-approved.toml` | `pub mod memory;` に feature の条件が無い。`#![forbid(unsafe_code)]`。外部依存 0。承認の記録 0 件 |
| C2 | 製品の経路からの参照 | `git grep -E 'BufferUsage\|BufferHandle\|MemoryOps' -- crates/contract crates/exec crates/upper crates/wasm-abi wasm measure baseline-wgpu` | 当たりは `crates/contract/src/memory_free_doctest.rs` だけ |
| C3 | TASK-308 の言及の今の一覧 | `git grep -l TASK-308 -- . ':!docs/spec' ':!.agents'` | 本書・`CLAUDE.md`・`README.md`・`crates/core/src/memory.rs`・`memory/buffer.rs`・`memory/ops.rs`・`crates/core/src/execution/dispatch_check.rs`・`crates/contract/src/memory_free_doctest.rs`・試験 3 本の 11 ファイル。「本リポの現状」の表の行より増えている（その表は書き換えず、ここを今の事実とする） |
| C4 | wasm32 と本番の組 | `cargo check -p fandhe-silicon-core --target wasm32-unknown-unknown --test memory_types --test memory_usage_values --test memory_pending_release`（`--features native-blocking-wait` 付きも）・`make build-profiles`・`make wasm-verify` | すべて終了コード 0 |
| C5 | 公開 API の検査 | `make public-api-check`・`make public-name-check`・`make public-api-sdk-check` | 3 つとも終了コード 0 |
| C6 | 対象外の事実 | 各「試験」の節の対象外 | 実行時の二重解放の検査、背後実装の `allocate` が実際に受け付けること、背後実装が返す `pending_release_count` の値は範囲の外（背後実装ごとのタスクのポインタ） |
| E1 | base の CI | `gh run list --branch main --workflow ci.yml` | base の run 39522744374 は completed・success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | 4 つとも終了コード 0 |

- C4 で `--tests` により全部の試験を一度に検査する形は使っていない。別件の `test_frame_decide` が wasm32 でコンパイルできないため（本件では直さない）
- 実機の run は含めない。`make inventory-trace-check` は `docs/spec` の取得が要るので含めない

### 再実行の契機

- `crates/core/src/memory/buffer.rs`・`ops.rs` の型・定数・derive・メソッド・既定の実装・`cfg` を変えたとき
- 上の 3 本の試験か `memory_free_doctest.rs` を変えたとき
- `core`・`contract` の feature（`test-support`・`native-blocking-wait`）の宣言か転送を変えたとき
- 背後実装の `MemoryOps` が入ったとき（TASK-314・TASK-327・TASK-340 など、背後実装ごとのタスクのポインタ）
- `docs/spec` の gitlink が進んだとき

完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-308）。確かめた base は `e87f4edb1e8c15f95c06ee23adf6d6763e3e4258`、`docs/spec` の gitlink は `0a802d7081610a36991a94b1ef220075d0616a28` を指す。

## 本リポでの反映の方針

- 本書には現状の事実だけを置く。具体の形（型・値・メソッド・置き場）は、受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す
- 置き場（`core` か `contract` か）と形は、TASK-308 の決定の記録に従う（関係: TASK-297・TASK-418。ポインタのみ）。`core` に足す場合は、外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ
- `docs/contract/` の項目ファイルは、対応するコードと公開テストを入れる段で作る（[docs/contract/README.md](contract/README.md)）
- 守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)

## 関連

- [contract-device-types](contract-device-types.md)
- [contract-error-types](contract-error-types.md)
- [buffer-range-check](buffer-range-check.md)
- [dispatch-operand-check](dispatch-operand-check.md)
- [public-api-wgpu-check](public-api-wgpu-check.md)
- [test-frame](test-frame.md)
- [workspace-boundary](workspace-boundary.md)
- [docs/contract/README.md](contract/README.md)
