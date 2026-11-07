# feature ゲートのコンパイル失敗の試験（TASK-233）の所在と本リポの現状

TASK-233（親 Issue #460）の試験の所在と、本リポのコードで確かめられる現状を案内する。
spec の本文・内部判断は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

- 正は private spec 側にある（ポインタ: `docs/spec/05-tasks.md` の TASK-233。関連: `docs/spec/04-behavior/build-profiles.md` の BUILD-11・BUILD-12・BUILD-13）
- 進み具合・前提の扱いは private spec 側の記録に従う（ポインタのみ）

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature の宣言 | `native-blocking-wait`・`ext-interop` は `crates/contract/Cargo.toml` に宣言済み。`default` には入らず、他の feature の有効化リストにも現れない | `cargo test -p fandhe-silicon-contract --test feature_gate_scope` |
| コンパイル失敗の仕組み | 外部 crate を足さず、rustdoc の doctest の fence を `cfg_attr` で切り替える（feature なしは `compile_fail`、ありは通る）。置き場は `crates/contract/src/cfg_gate_doctest.rs`（doctest の収集のため `pub mod` にし、`#[doc(hidden)]` で文書の表示だけを隠している。外部 crate から参照はできる） | `cargo test -p fandhe-silicon-contract --doc` と `--features backend-metal` の両方 |
| 仕組みの対照 | 実在の feature ゲート `backend-metal` の `backend_metal` モジュールの対（`BackendMetalGate`）と、`crates/core` の実物のアイテムを使う対。`DownloadBlockingGate`（`download_blocking`）・`WaitBlockingGate`（`wait_blocking`）・`ExternalInteropGate`（`ExternalInterop`）が feature なしで `compile_fail`、ありで通り、`NonBlockingReadbackGate` は常に通る。差し替えの経緯は「アイテムへの差し替え（5/7）」「アイテムへの差し替え（6/7）」の節 | 同上と `--features native-blocking-wait`・`--features ext-interop` |
| 有効側の試験 | feature を有効にした構成で、`download_blocking`・`wait_blocking`・`ExternalInterop` を `&dyn` 越しに参照して型検査が通ること。置き場は `crates/contract/tests/feature_gate_enabled.rs` | `cargo test -p fandhe-silicon-contract --all-features --test feature_gate_enabled` |
| 流れる場所 | feature なし側は `scripts/build-profiles.sh` の組、ありの側は `make verify`（`--all-features`）と、`scripts/build-profiles.sh` の `native-blocking-wait` 単独・`ext-interop` 単独の組（build / clippy / test）。どちらも `verify` ジョブが呼ぶ | `make build-profiles`、`make verify` |
| wasm32 の組 | `cargo check` のみで doctest は流れない | `scripts/build-profiles.sh` |

## 段

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/4 | #3928 | 試験の枠（本書の仕組みと feature の範囲の試験） |
| 2/4 | #3929 | 有効側の試験 |
| 3/4 | #3930 | CI への組み込み |
| 4/4 | #3931 | 完了の条件の確かめ |
| 5/7 | #7558 | アイテムへの差し替え（native-blocking-wait） |
| 6/7 | #7559 | アイテムへの差し替え（ext-interop） |
| 7/7 | #7560 | 完了の条件の確かめ |

## CI への組み込み（3/4）

- `scripts/build-profiles.sh` の組に `native-blocking-wait` 単独と `ext-interop` 単独を足し、feature ごとに build / clippy（`-D warnings`）/ test（doctest を含む）を流す。新しいジョブ・ターゲットは足さず、`verify` ジョブの `make build-profiles` が呼ぶ
- 単独の組の出力の見え方: `native-blocking-wait` 単独では `native_blocking_wait_gate_is_visible` と `NativeBlockingWaitGate` の doctest が通り、他の 2 つの doctest は `compile fail ... ok`。`ext-interop` 単独では `ext_interop_gate_is_visible` と `ExtInteropGate` が通り、他の 2 つが `compile fail ... ok`
- 手元の確かめ: `cargo test -p fandhe-silicon-contract --no-default-features --features native-blocking-wait`（`ext-interop` も同様）、全体は `make build-profiles`

## 完了の条件の確かめ（4/4）

4/4（#3931）で記録した。本リポで観測できる事実だけを書く（ポインタ: `docs/spec/05-tasks.md` の TASK-233）。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| A1 | feature の範囲 | `cargo test -p fandhe-silicon-contract --test feature_gate_scope` | 終了コード 0。3 passed / 0 failed（宣言済み・`default` に無い・他の feature から含意されない） |
| A2 | 無効側の失敗 | `cargo test -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops --doc` | 終了コード 0。3 つの門の doctest がすべて `compile fail ... ok` |
| A3 | `native-blocking-wait` 単独 | `cargo test -p fandhe-silicon-contract --no-default-features --features native-blocking-wait` | 終了コード 0。`native_blocking_wait_gate_is_visible` と `NativeBlockingWaitGate` の doctest が通り、他の 2 つの doctest は `compile fail ... ok` |
| A4 | `ext-interop` 単独 | `cargo test -p fandhe-silicon-contract --no-default-features --features ext-interop` | 終了コード 0。`ext_interop_gate_is_visible` と `ExtInteropGate` の doctest が通り、他の 2 つの doctest は `compile fail ... ok` |
| A5 | 全 feature | `cargo test -p fandhe-silicon-contract --all-features --doc`、`--all-features --test feature_gate_enabled` | 終了コード 0。doctest 3 件と有効側の試験 2 件がすべて通常の成功 |
| B1 | 流れる場所 | `scripts/build-profiles.sh` の組（`native-blocking-wait` 単独・`ext-interop` 単独の行と、core の `--all-features` の行） | 単独の 2 組は feature ごとに build / clippy / test を流す。`verify` ジョブの `make build-profiles` が呼ぶ |
| B2 | `ext-interop` を有効にする組 | `grep -n ext-interop scripts/build-profiles.sh` | 現れるのは BUILD-4 の組と単独の組だけ。`default` と含意に無いことは A1 が担保 |
| C1 | 依存 | `grep -c trybuild Cargo.lock`、`[[approval]]` の件数 | 0 件のまま。外部 crate は足していない |
| E1 | main の CI | main の push の run（`6950c28`。run `39425952330`）の `make verify` ジョブ | `make verify` の step と `make build-profiles` の step がともに success |
| D | 品質ゲート | `make check`・`make verify`・`make deny` | 本 PR の作成時にいずれも終了コード 0 |

- B2 は記録の時点の値。TASK-231 の 2/4 で `backend-vulkan,ext-interop`・`backend-cuda,ext-interop` の組が加わった（[ext-interop-build-check](ext-interop-build-check.md)）
- 4/4 の時点で確かめたのは門の目印の対による枠の範囲だった。対象のアイテム自体を使う snippet への差し替えは「アイテムへの差し替え（5/7）」「アイテムへの差し替え（6/7）」の節に記録した
- wasm32 の組は doctest が流れないので含めない
- 完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-233）

## 本リポでの反映の方針

- 対象の機能を使うコードの snippet は、そのコードを持つ PR で `cfg_gate_doctest.rs` に同じ形で足す。`native-blocking-wait`・`ext-interop` の 3 アイテムは差し替え済み（「アイテムへの差し替え（5/7）」「アイテムへの差し替え（6/7）」の節）
- 手順は [contract/procedure](contract/procedure.md) に従う

## 関連

- [spec-confidentiality](../.claude/rules/spec-confidentiality.md)

## 依存の承認の件数の現状

上の確かめの表と本書の記録の、依存の承認の件数（0 件）は記録時点の値で、書き換えない。今の値は次のとおり。

- `core` の外部依存は 0 のまま（`cargo tree -p fandhe-silicon-core -e normal --depth 1` が自身だけを出す）
- `grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml` は 0 が返る（`core` を `used_by` に持つ記録は無い）
- `grep -c '^\[\[approval\]\]' deps-approved.toml` は 4 が返る。4 件は `crates/contract` を `used_by` に持つ macOS 向けの記録（`objc2` 系）

## アイテムへの差し替え（5/7）

`native-blocking-wait` の門の目印の対を外し、core の 2 アイテムを使う対に差し替えた（TASK-233・BUILD-11・BUILD-12 のポインタ）。`ext-interop` は 6/7 の範囲で、目印の対のまま。

### 差し替え前の所在

- `MemoryOps::download_blocking`（`crates/core/src/memory/ops.rs`）と `CommandQueue::wait_blocking`（`crates/core/src/execution/submit.rs`）は、どちらも `#[cfg(feature = "native-blocking-wait")]` の裏にある
- `download_async` と `on_complete` は feature に依らず存在する（native では callback に `Send` が要る）
- `crates/contract/Cargo.toml` の `native-blocking-wait = ["fandhe-silicon-core/native-blocking-wait"]` が core の同名 feature へ転送する
- 目印は `NativeBlockingWaitEnabled` と、それを `use` する `NativeBlockingWaitGate` の doctest だった

### 差し替え後の所在

- `crates/contract/src/cfg_gate_doctest.rs`
    - `DownloadBlockingGate`: `download_blocking` を呼ぶ snippet
    - `WaitBlockingGate`: `wait_blocking` を呼ぶ snippet（片方だけ門の外に出ても気づけるよう、別の snippet にしてある）
    - `NonBlockingReadbackGate`: `download_async` と `on_complete` を呼ぶ snippet（fence の切り替えなし。すべての組で通常の成功）
- `crates/contract/tests/feature_gate_enabled.rs` の `native_blocking_wait_gate_is_visible`（名前は変えず、中身を core の 2 アイテムの参照にした）

### 組ごとの見え方

| 組 | `DownloadBlockingGate`・`WaitBlockingGate` | `NonBlockingReadbackGate` |
| -- | -- | -- |
| 既定の組・`ext-interop` 単独 | `compile fail ... ok` | `ok` |
| `native-blocking-wait` 単独・`--all-features` | `ok` | `ok` |

### 記録の時点の値

上の表と 4/4 の表のうち、目印の名前（`NativeBlockingWaitGate`・`NativeBlockingWaitEnabled`）を挙げる行と、A2・A5 の doctest の件数は記録の時点の値で、書き換えない。今の値は次のとおり。

- `crates/` 配下に `NativeBlockingWaitEnabled`・`NativeBlockingWaitGate` の名前は残っていない
- `cfg_gate_doctest.rs` の doctest は 9 件（`BackendMetalGate`・`DownloadBlockingGate`・`WaitBlockingGate`・`NonBlockingReadbackGate`・`ExternalInteropGate`・`FaultInjectionGate`・`DeviceFaultHooksGate`・`GraphicsGate`・`DeviceGraphicsGate`。`GraphicsGate`・`DeviceGraphicsGate` は [contract-device-stub-check](contract-device-stub-check.md) の「描画の口の契約テスト（10/10）」）

## アイテムへの差し替え（6/7）

`ext-interop` の門の目印の対を外し、core の `ExternalInterop` を使う対に差し替えた（TASK-233・BUILD-13 のポインタ）。

### 差し替え前の所在

- `ExternalInterop`（`crates/core/src/interop.rs`）は、`crates/core/src/lib.rs` の `#[cfg(feature = "ext-interop")] pub mod interop;` の裏にある
- `crates/contract/Cargo.toml` の `ext-interop = ["fandhe-silicon-core/ext-interop"]` が core の同名 feature へ転送する
- 目印は `ExtInteropEnabled` と、それを `use` する `ExtInteropGate` の doctest だった

### 差し替え後の所在

- `crates/contract/src/cfg_gate_doctest.rs` の `ExternalInteropGate`: `use fandhe_silicon_core::interop::ExternalInterop;` と `fn probe(_: &dyn ExternalInterop) {}` の snippet
- `crates/contract/tests/feature_gate_enabled.rs` の `ext_interop_gate_is_visible`（名前は変えず、中身を `ExternalInterop` の参照にした）

### 組ごとの見え方

| 組 | `ExternalInteropGate` |
| -- | -- |
| 既定の組・`native-blocking-wait` 単独 | `compile fail ... ok` |
| `ext-interop` 単独・BUILD-4 の組・`backend-vulkan,ext-interop`・`backend-cuda,ext-interop`・`backend-metal,ext-interop`・`--all-features` | `ok` |

### 本番の組の試験

`crates/contract/tests/feature_gate_scope.rs` が `scripts/build-profiles.sh` の `# spec BUILD-1`〜`BUILD-4` の注記の 4 組を読み、feature の含意をたどって `ext-interop` に届くのが BUILD-4 の組だけであることを確かめる。

- 実物: `ext_interop_is_reached_only_from_the_interop_production_set`
- 陽性対照: `ext_interop_check_catches_a_wrong_production_set`（組の書き換え・組の数の不一致）、`ext_interop_check_follows_implications`（含意の経路）
- 確かめ方: `cargo test -p fandhe-silicon-contract --test feature_gate_scope`
- 4/4 の表の B2（grep による記録）は書き換えず、同じ点を試験でも確かめるようになった

### 記録の時点の値

3/4・4/4・5/7 の表のうち、目印の名前（`ExtInteropGate`・`ExtInteropEnabled`）を挙げる行と、A1・A4・A5 の件数、5/7 の doctest の一覧は記録の時点の値で、書き換えない。今の値は次のとおり。

- `crates/` 配下に `ExtInteropEnabled`・`ExtInteropGate` の名前は残っていない
- `cfg_gate_doctest.rs` の doctest は 9 件（`BackendMetalGate`・`DownloadBlockingGate`・`WaitBlockingGate`・`NonBlockingReadbackGate`・`ExternalInteropGate`・`FaultInjectionGate`・`DeviceFaultHooksGate`・`GraphicsGate`・`DeviceGraphicsGate`。`GraphicsGate`・`DeviceGraphicsGate` は [contract-device-stub-check](contract-device-stub-check.md) の「描画の口の契約テスト（10/10）」）
- `feature_gate_scope` の試験は 6 件

## 完了の条件の確かめ（7/7）

7/7（#7560）で、本リポで観測できる事実だけを記録する（TASK-233 のポインタ。`docs/spec/05-tasks.md`）。結果は隔離した作業ツリーで実際に流した出力による。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 無効の構成で 3 アイテムの門が通らない（BUILD-11・BUILD-12・BUILD-13） | `cargo test -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops --doc` | 終了コード 0。doctest 7 件のうち `DownloadBlockingGate`・`WaitBlockingGate`・`ExternalInteropGate` を含む 6 件が `compile fail ... ok`、`NonBlockingReadbackGate` が `ok` |
| (2a) | `native-blocking-wait` 単独 | `cargo test -p fandhe-silicon-contract --no-default-features --features native-blocking-wait` | 終了コード 0。`DownloadBlockingGate`・`WaitBlockingGate` が `ok`、`ExternalInteropGate` が `compile fail ... ok`。`feature_gate_enabled` は `native_blocking_wait_gate_is_visible` の 1 件が通る |
| (2b) | `ext-interop` 単独 | `cargo test -p fandhe-silicon-contract --no-default-features --features ext-interop` | 終了コード 0。`ExternalInteropGate` が `ok`、`DownloadBlockingGate`・`WaitBlockingGate` が `compile fail ... ok`。`feature_gate_enabled` は `ext_interop_gate_is_visible` の 1 件が通る |
| (2c) | 全 feature | `cargo test -p fandhe-silicon-contract --all-features --doc` と `--all-features --test feature_gate_enabled` | 終了コード 0。doctest 7 件がすべて `ok`、`feature_gate_enabled` は 2 件が通る |
| (3) | 常に通る読み戻しの口 | (1)・(2a)・(2b)・(2c) の `NonBlockingReadbackGate` の行 | どの組でも `ok` |
| (4) | 本番の組と `ext-interop` | `cargo test -p fandhe-silicon-contract --test feature_gate_scope ext_interop` | 3 件が通る（`ext_interop_is_reached_only_from_the_interop_production_set`・`ext_interop_check_catches_a_wrong_production_set`・`ext_interop_check_follows_implications`） |
| (5) | feature の範囲 | `cargo test -p fandhe-silicon-contract --test feature_gate_scope gated_features` と絞りなしの `--test feature_gate_scope` | 絞りありで 3 件、絞りなしで 6 件が通る |
| (6) | 流れる場所と main の CI | `grep -n 'make build-profiles' .github/workflows/ci.yml`、`grep -n 'native-blocking-wait\|ext-interop' scripts/build-profiles.sh`、`gh run view 41266845012` | `ci.yml` の `verify` ジョブと `verify-macos` ジョブが `make build-profiles` を呼び、`build-profiles.sh` に `native-blocking-wait` 単独・`ext-interop` 単独の組がある。main の `6630725` の push の run では、`make verify + build-profiles (macos-latest)` と `make verify (fmt + clippy + test)` の job が success。run `41266845012` は completed / success |
| (7) | 依存 0・unsafe の追加 0 | `grep -c trybuild Cargo.lock`、`grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -c unsafe crates/contract/src/cfg_gate_doctest.rs`、`git diff --stat origin/main -- Cargo.lock deps-approved.toml` | 0 / 4（contract の macOS 向けの記録。先の「依存の承認の件数の現状」と同じ値）/ 0 / 差分なし |
| (8) | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles` | どれも終了コード 0 |

- wasm32 の組は `cargo check` だけで doctest が流れないので、この表に含めない
- 読み戻しの口の名前の側の機械の検査は `make public-api-webgpu-check` の (1) にある（TASK-241 のポインタ）
- 完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-233）

## 今の値の補い（base 25d8dc2）

上の表と文は記録時点の値で、書き換えていない。今の値は次のとおり（実行環境は GPU なしの Linux）。

| 節 | 記録の値 → 今の値 | 測ったコマンド |
| -- | ----------------- | -------------- |
| 「記録の時点の値」（アイテムへの差し替え 5/7）と同（6/7）の `cfg_gate_doctest.rs` の doctest 9 件 | 9 件 → 11 件。9 件の一覧に `CpuOpsGate`（127 行目）と `CpuOpsNonExhaustiveGate`（142 行目）が加わった（[cpu-ops-surface-check](cpu-ops-surface-check.md)） | `grep -c '^pub struct' crates/contract/src/cfg_gate_doctest.rs`（11）、`grep -n '^pub struct' crates/contract/src/cfg_gate_doctest.rs` |
| 完了の条件の確かめ（4/4）の A5 と完了の条件の確かめ（7/7）の (2c) の有効側の試験 | `feature_gate_enabled` は 2 件 → 3 件（`3 passed; 0 failed; 0 ignored`。`cpu_ops_gate_is_visible` が加わった） | `cargo test -p fandhe-silicon-contract --all-features --test feature_gate_enabled` |
| 同じ行の `--all-features --doc` | doctest はすべて通常の成功 → `cfg_gate_doctest.rs` の doctest 11 件はすべて通り、10 件が `ok`、`CpuOpsNonExhaustiveGate (line 131)` だけが `compile fail ... ok`（`compile_fail,E0004`） | `cargo test -p fandhe-silicon-contract --all-features --doc` |
| 完了の条件の確かめ（7/7）の (5) | `feature_gate_scope` は絞りなしで 6 件 → 今も 6 件（`6 passed; 0 failed; 0 ignored`） | `cargo test -p fandhe-silicon-contract --test feature_gate_scope` |

## 今の値の補い（base 3e32680）

記録時点の表と既存の節は書き換えない。以下は base `3e32680` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 「依存の承認の件数の現状」の節の `grep -c '^\[\[approval\]\]'` の行 | 4 が返る（4 件は `crates/contract` を `used_by` に持つ macOS 向けの `objc2` 系）→ 5 が返る（`objc2` 系 4 件に、wasm32 以外向けの `cudarc` の 1 件が加わった。5 件とも `used_by` は `fandhe-silicon-contract`） | `grep -c '^\[\[approval\]\]' deps-approved.toml`、`grep -c '^name = "objc2' deps-approved.toml`（4 が返る）、`grep -n -A3 '^name = "cudarc"' deps-approved.toml` |
