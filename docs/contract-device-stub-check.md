# スタブの実装の契約テスト（TASK-137）の所在と本リポの現状

TASK-137 の作業を進めるにあたり、記録の所在と本リポの現状の事実を置く。spec の本文は転記しない（[spec-confidentiality](../.claude/rules/spec-confidentiality.md)）。

## 記録の所在

記録の正は private spec 側にある（`docs/spec/05-tasks.md` の TASK-137。ポインタのみ）。内容・理由・経緯は本書に置かない。
`docs/spec` は submodule（`update = none`）で、指している版は `git ls-tree origin/main docs/spec` で確かめる。

前提のタスクと、それに当たる本リポの物は次のとおり。

| 前提（ポインタ） | 本リポの物 | 確かめ方 | 文書 |
| ---------------- | ---------- | -------- | ---- |
| TASK-5 | `crates/contract/Cargo.toml` の `[features]`、`crates/contract/src/lib.rs` の `cfg`、`scripts/build-profiles.sh` | 親の Issue の状態が CLOSED（`gh issue view 30 --json state`） | [contract-feature-check](contract-feature-check.md) |
| TASK-23 | `crates/core/src/test_frame*`（`test-support` の裏） | 同上（`gh issue view 91 --json state`） | [test-frame](test-frame.md) |
| TASK-136 | `crates/core/src/device.rs`・`capability.rs` | 親の Issue #411 の状態は OPEN（`gh issue view 411 --json state`）。型は `crates/core` にあり、文書の列のテストで確かめる | [contract-device-types](contract-device-types.md) | [contract-device-types](contract-device-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 既にある型 | device の要求・能力問合せの型は `crates/core` にある。定義と確かめは [contract-device-types](contract-device-types.md) を参照し、ここでは繰り返さない | `crates/core/src/device.rs`、`crates/core/src/capability.rs` |
| 公開のトレイト | `crates/`・`wasm/` の `pub trait` は 14 件（`Capabilities`、`Device`・`DeviceProvider`〔`crates/core/src/device.rs`〕、`GraphicsOps`〔`crates/core/src/graphics.rs`。feature `graphics` の裏〕、`FaultHooks`〔`fault-injection` の裏〕、`MemoryOps`〔`crates/core/src/memory/ops.rs`〕、`Check`〔`test-support` の裏〕、`ValidationIntake`〔`crates/core/src/validation.rs`〕、`DeviceLostNotify`、`ComputeOps`、`CommandQueue`、`ExternalInterop`、内部用の `__BackendEncoderCloser`・`__BackendSharedEncoderCloser`） | `git grep -nE 'pub trait' -- crates wasm` |
| device まわりの口 | device のトレイト `Device` と device を返す入口 `DeviceProvider` は `crates/core/src/device.rs` にある。投入の列のトレイト `CommandQueue`（`crates/core/src/execution/submit.rs`）、compute の口 `ComputeOps`（`crates/core/src/execution/compute.rs`。それを返す公開のアクセサ `Device::compute()` は `crates/core/src/device.rs` にあり、既定は `None`）、試験用のスタブの device `StubDevice` と provider `StubDeviceProvider`（`crates/core/src/stub_device.rs`、feature `test-support` の裏。[stub-device](stub-device.md)）が `crates/core` にある。メモリの操作のトレイト `MemoryOps` とバッファ型（`BufferHandle`・`BufferUsage`）は `crates/core/src/memory/` に既にある。名前に `Device` を含む型は要求・生ハンドル・診断の値の型と上の 2 つのトレイト、および `crates/contract/src/backend_metal.rs`（macOS だけ）の Metal の provider 3 件（`MetalDeviceProvider`、feature `fault-injection` の裏の `MetalDefaultDeviceNoneProvider`・`MetalCommandQueueNoneProvider`）、`Stub` を含む型は故障の差し込み用の `FaultStub` と上のスタブの device・provider。`crates/core/src/memory.rs` はそれらの再公開と引数検査の純粋関数（`validate_allocation_size`・`validate_buffer_range`）を持つ | `git grep -nE 'pub (trait\|struct) [A-Za-z]*(Device\|Memory\|Queue\|Stub)' -- crates wasm` |
| 故障の差し込み口 | `FaultHooks` はある。それを返す公開のアクセサ `Device::fault_hooks()` が `crates/core/src/device.rs` にある（feature `fault-injection` の裏。既定は `None`）。`crates/core/tests/fault_parts_composition.rs` の同名メソッドは試験内の局所トレイト | `crates/core/src/fault.rs`、`crates/core/src/device.rs`、`crates/core/tests/fault_parts_composition.rs` |
| feature の裏のアイテム | `graphics`・`artifact-inspection` は `crates/contract/Cargo.toml` に宣言済み。`graphics` の `cfg` は `crates/contract` の 4 ファイル（`src/backend_metal.rs` と `tests/backend_metal_fault.rs`・`backend_metal_request.rs` の `cfg(not(feature = "graphics"))` の 3 件、`tests/stub_device_profiles.rs` の 3 件）と `crates/core` の 4 ファイル（`src/lib.rs`・`src/device.rs`・`src/stub_device.rs` の 12 件・`tests/device_trait_shape.rs`）で、計 8 ファイル 21 行、`artifact-inspection` で切られたアイテムは 0 件 | `git grep -n 'feature = "graphics"' -- crates`、`git grep -n 'feature = "artifact-inspection"' -- crates` |
| 組ごとのビルド検査 | `scripts/build-profiles.sh` の `NATIVE_SETS`（`graphics` を含む組と含まない組がある）と `WASM_SETS`。`make build-profiles`（CI の `verify` ジョブ） | `grep -n 'NATIVE_SETS\|WASM_SETS' scripts/build-profiles.sh` |
| コンパイル失敗の既存の仕組み | feature の有無でコンパイルが落ちることを確かめる枠が `crates/contract/src/cfg_gate_doctest.rs` にある | [feature-gate-compile-check](feature-gate-compile-check.md) |
| 走る場所 | `make verify`（`--all-features`）と `make build-profiles`。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |
| 依存の承認 | 外部 crate は足さない。`deps-approved.toml` の記録の範囲に収める（`core` を `used_by` に持つ記録は無い） | `grep -c '^used_by = .*fandhe-silicon-core' deps-approved.toml`（0 が返る） |
| 本書の対象の言及 | TASK-137 の言及は本書、`CLAUDE.md`、`README.md`、`crates/contract/tests/stub_device_profiles.rs` だけ | `git grep -nE 'TASK-137([^0-9]\|$)' -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/6 | #3659 | 現状と前提の確認（本書） |
| 2/6 | #3660 | 形の設計 |
| 3/6 | #3661 | 実装 |
| 4/6 | #3662 | テスト |
| 5/6 | #3663 | CI への組み込み |
| 6/6 | #3664 | 確かめ |
| 7/8 | #7576 | 契約テストの実装 |
| 8/8 | #7577 | 完了の条件の確かめ |
| 9/9 | #7593・#7596 | compute の口の契約テスト |
| 10/10 | #7612 | 描画の口の契約テスト |

## 形の設計（2/6）

本段の範囲は `docs/spec/05-tasks.md` の TASK-137 の内容の 1 文目（ポインタのみ）。
具体の形（型・名前・置き場・試験の名前・本数の数え方）は 3/6（#3661）以降のコードと公開テストが示し、本節には先に書かない。

### 決定の記録に従う点

形は TASK-137 と `docs/spec/04-behavior/api-device-capability.md` の DEV-15・DEV-59・DEV-60・DEV-63・DEV-66・DEV-80 から辿る決定の記録に従う（ポインタのみ。内容は書かない）。
関連のポインタ: アクセサの共通の形は TASK-134、試験用の device の置き場は TASK-126、投入の列のトレイトは TASK-588、compute の型は TASK-769、故障の差し込み口は TASK-271、プロファイルごとの本数の照合は TASK-227。
決定の記録から一意に決まらない点は [contract-shaping-escalation](../.claude/rules/contract-shaping-escalation.md) の経路で扱う。

### 形が乗る既存のコード・検査の事実

「本リポの現状」の表と重なる行は、そちらを参照する。

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| feature | `crates/core` の feature は `test-support`・`fault-injection`・`native-blocking-wait` だけで、`graphics` は `crates/contract` にしか無い | 両 crate の `Cargo.toml` の `[features]` |
| `artifact-inspection` | contract では `backend-metal` を要求する。core に同名の feature は無い | `crates/contract/Cargo.toml` |
| `fault-injection` | contract から core へ転送される | `crates/contract/tests/fault_injection_forwarding.rs` |
| 試験と feature | core の試験はファイルごとに `required-features` で feature に結び付けている | `crates/core/Cargo.toml` の `[[test]]` |
| 組ごとのビルド | `NATIVE_SETS`・`WASM_SETS`（`make build-profiles`、CI の `verify` ジョブ） | `scripts/build-profiles.sh` |
| コンパイル失敗の枠 | `crates/contract/src/cfg_gate_doctest.rs`（[feature-gate-compile-check](feature-gate-compile-check.md)） | 同左 |

### 前提の部品と持ち主

| 部品 | 本リポ | 持ち主（ポインタ） | 公開側の Issue |
| ---- | ------ | ------------------ | -------------- |
| 投入の列のトレイト | 無い | TASK-588 | #696 |
| compute の口 | 無い | TASK-769 | #784 |
| 試験用の device の置き場 | 無い | TASK-126 | #1100 |
| device のトレイト・device を返す入口 | 無い | TASK-137 の記録に従う | なし |
| メモリの操作のトレイト | ある（`crates/core/src/memory/ops.rs`） | なし | なし |

確かめ方: `git grep -nE 'pub trait' -- crates wasm`、`gh issue view <番号> --json state`。

上の表の「無い」は記録時点の値で、書き換えない。今の値は、投入の列のトレイト `CommandQueue`・compute の口 `ComputeOps`・試験用の device の置き場 `crates/core/src/stub_device.rs`（`StubDevice`・`StubDeviceProvider`。feature `test-support` の裏。[stub-device](stub-device.md)）がいずれも `crates/core` にある。

### どの形でも守る本リポの規約

| 項目 | 規約 | 所在 |
| ---- | ---- | ---- |
| 依存 | 外部 crate 0。`deps-approved.toml` の範囲 | [dependency-policy](../.claude/rules/dependency-policy.md) |
| unsafe | core・exec・upper は `forbid(unsafe_code)`。スタブは unsafe を持たない | [coding-rust](../.claude/rules/coding-rust.md)、[unsafe-policy](unsafe-policy.md) |
| wasm32 | core に足す物は wasm32 でも通す | `WASM_SETS` |
| 試験用の物 | 本番の feature の組に入れない（`test-support`・`fault-injection` の前例） | `crates/core/Cargo.toml` |
| 本数の観測 | プロファイルごとの成功の本数は TASK-227 が照合する。試験は feature の組ごとに本数が数えられる置き方にする | 同左 |
| PoC からの持ち込み | 複製せず、本リポの規約に沿って書き直す | [poc-carry-over](../.claude/rules/poc-carry-over.md) |
| spec との対応 | ID のポインタだけ | [spec-confidentiality](../.claude/rules/spec-confidentiality.md) |

### 3/6 以降で扱うこと

3/6 実装・4/6 テスト・5/6 CI への組み込み・6/6 確かめ（段の表のとおり）。
上の表で「無い」の部品が本リポに入る前に、3/6 で持ち主の TASK の型を先取りして定義しない。

## 実装（3/6）

本段の範囲は `docs/spec/05-tasks.md` の TASK-137 の成果物の 1 つ目（ポインタのみ）。
本段ではコードも試験も足さない。契約テストが前提にする部品が `crates/`・`wasm/` に無く、試験の中だけで局所のトレイトを定義する回避や、持ち主の TASK の型を先に定義することはしない（上の「3/6 以降で扱うこと」のとおり）。

### 契約テストが要る部品の所在

| 部品 | 本リポ | 持ち主（ポインタ） | 公開側の Issue |
| ---- | ------ | ------------------ | -------------- |
| device のトレイト・device を返す入口 | 無い | TASK-137 の記録に従う（ポインタのみ） | なし |
| 投入の列のトレイト | 無い | TASK-588 | #696 |
| compute の口 | 無い | TASK-769 | #784 |
| 試験用の device の置き場 | 無い | TASK-126 の記録に従う（ポインタのみ） | #1100 |
| メモリの操作のトレイト | ある（`crates/core/src/memory/ops.rs` の `MemoryOps`） | なし | なし |

確かめ方:

- `git grep -nE 'pub trait' -- crates wasm`（5 件だけであること）
- `git grep -nE 'pub (trait|struct) [A-Za-z]*(Device|Queue|Stub)' -- crates wasm`
- `gh issue view 696 --json state`・`gh issue view 784 --json state`・`gh issue view 1100 --json state`

上の表の「無い」と「5 件だけ」は記録時点の値で、書き換えない。今の値は末尾の「現状の公開のトレイトと背後実装の公開の項目」に書いた。

### 再開の条件

上の表で「無い」の部品が main に入ったとき、契約テストのコードを足す。4/6（#3662）・5/6（#3663）・6/6（#3664）も同じ条件に従う。段の表は変えない。
どの形でも守る規約は「形の設計（2/6）」の表のとおりで、ここでは繰り返さない。

## テスト（4/6）

本段の範囲は `docs/spec/05-tasks.md` の TASK-137 の内容の 2 文目（ポインタのみ）。
本段でもコードも試験も足さない。「実装（3/6）」の「契約テストが要る部品の所在」の表の部品が、今も `crates/`・`wasm/` に無いため（表は繰り返さない）。

### 本リポの事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 背後実装のモジュール | `crates/contract/src/backend_metal.rs`・`backend_cuda.rs` に `pub` の項目は無い。`backend_vulkan.rs` は `pub mod spirv_operands` だけで、device の実装に当たる公開の項目は無い | `git grep -nE '^\s*pub ' -- crates/contract/src/backend_metal.rs crates/contract/src/backend_vulkan.rs crates/contract/src/backend_cuda.rs` |
| 公開のトレイト | 4 件のまま（「本リポの現状」の表と同じ） | `git grep -nE 'pub trait' -- crates wasm` |
| 前提の部品の Issue | #696・#784・#1100 は OPEN | `gh issue view <番号> --json state` |

### 関連のポインタ

TASK-137 を前提に引く背後実装側の TASK: TASK-195（#1126）・TASK-196（#447）・TASK-197（#448）・TASK-720（#1305）。中身は書かない。

### 再開の条件

「実装（3/6）」の「再開の条件」に従う。段の表は変えない。残る段: 5/6〜6/6。

## CI への組み込み（5/6）

本段の範囲は `docs/spec/05-tasks.md` の TASK-137 の成果物の 2 つ目（ポインタのみ）。
本段でも CI の設定・スクリプト・コード・試験を変えない（専用のジョブ・step・make ターゲット・組は足していない）。「実装（3/6）」の「契約テストが要る部品の所在」の表の部品が、今も `crates/`・`wasm/` に無いため（表は繰り返さない）。

### CI での走り方の事実

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 組ごとの検査の step | CI の `verify` ジョブで、`make verify`（`--all-features`）の後に `make build-profiles` が流れる | `grep -n 'run: make' .github/workflows/ci.yml` |
| 流れる組 | 既定の組、`NATIVE_SETS` の各組、core を feature なしで流す行と `test-support` だけで流す行、`--all-features`、`WASM_SETS` の `cargo check` | `grep -n '_SETS' scripts/build-profiles.sh` |
| 試験の拾われ方 | `crates/contract/tests/` の試験は、各組の `cargo test -p fandhe-silicon-contract` で自動で流れる（前例: [contract-feature-check](contract-feature-check.md) の「CI への組み込み（3/3）」）。core の試験は `crates/core/Cargo.toml` の `[[test]]` の `required-features` で feature に結び付く | `scripts/build-profiles.sh`、`crates/core/Cargo.toml` |
| コンパイル失敗の枠 | `crates/contract/src/cfg_gate_doctest.rs` の doctest は組ごとの test で流れる（[feature-gate-compile-check](feature-gate-compile-check.md) の「CI への組み込み（3/4）」） | 同左 |
| feature の裏のアイテム | `graphics`・`artifact-inspection` で切られたアイテムは今も 0 件 | `git grep -n 'feature = "graphics"' -- crates`、`git grep -n 'feature = "artifact-inspection"' -- crates` |
| 公開のトレイト | 4 件のまま（「本リポの現状」の表と同じ） | `git grep -nE 'pub trait' -- crates wasm` |
| 前提の部品の Issue | #696・#784・#1100 は OPEN | `gh issue view <番号> --json state` |

### 再開の条件

「実装（3/6）」の「再開の条件」に従う。部品が main に入って契約テストを足す PR で、要る組と CI での流れ方をその PR のコードと公開テストで示し、本書に書き足す。段の表は変えない。残る段: 6/6。

## 確かめ（6/6）

6/6（#3664）で記録した。範囲は `docs/spec/05-tasks.md` の TASK-137 の内容の 3〜4 文目（ポインタのみ）。結果は GPU の実機なしで観測した値で、件数は実測。コード・試験・CI は変えていない。

### 成果物と本リポの物の対応

| 成果物 | 本リポ | 本書の節 |
| ------ | ------ | -------- |
| スタブの実装の契約テスト | 無い（前提の部品が無いため） | 「実装（3/6）」の「契約テストが要る部品の所在」 |
| feature の組ごとのビルド検査 | 組ごとのビルドの土台は `scripts/build-profiles.sh`（`NATIVE_SETS`・`WASM_SETS`）にあり、CI の `verify` ジョブの `make build-profiles` が流す。契約テストを組ごとに流す部分は、契約テストと一緒に入る | 「CI への組み込み（5/6）」 |

### 持ち分のポインタ

中身は書かない（ポインタのみ）。

- 背後実装の実体の振る舞いの試験: TASK-195（#1126）・TASK-196（#447）・TASK-197（#448）・TASK-720（#1305）
- プロファイルごとの本数の照合: TASK-227（#1149。[profile-switch-check](./profile-switch-check.md)）

### 確かめの表

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| F1 | 公開のトレイト | `git grep -nE 'pub trait' -- crates wasm` | 4 件（`Capabilities`・`FaultHooks`・`MemoryOps`・`Check`） |
| F2 | device まわりの口 | `git grep -nE 'pub (trait\|struct) [A-Za-z]*(Device\|Queue\|Stub)' -- crates wasm` | `DeviceRequest`・`NativeDeviceHandle`・`FaultStub` の 3 件だけ |
| F3 | feature の裏のアイテム | `git grep -n 'feature = "graphics"' -- crates`・`git grep -n 'feature = "artifact-inspection"' -- crates` | いずれも 0 件 |
| F4 | 背後実装のモジュールの公開の項目 | `git grep -nE '^\s*pub ' -- crates/contract/src/backend_metal.rs crates/contract/src/backend_vulkan.rs crates/contract/src/backend_cuda.rs` | `backend_vulkan.rs` の `pub mod spirv_operands` だけ（4/6 の記録と同じ） |
| F5 | CI での組ごとの検査 | `grep -n 'run: make' .github/workflows/ci.yml` | `verify` ジョブで `make verify` の後に `make build-profiles` |
| F6 | 前提と持ち分の Issue | `gh issue view <番号> --json state`（#696・#784・#1100・#1126・#447・#448・#1305・#1149） | 8 件とも OPEN |
| F7 | 言及の範囲 | `git grep -nE 'TASK-137([^0-9]\|$)' -- . ':!docs/spec' ':!.agents'` | 本書・`CLAUDE.md`・`README.md` だけ |
| E1 | base の直前の CI | `gh run list --branch main --workflow ci.yml --limit 5` | 確かめた base のコミットの run は取得時点で実行中。1 つ前の run `39527629926` は completed / success |
| D | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check` | いずれも終了コード 0 |

- 実機の run と `make inventory-trace-check`（`docs/spec` の取得が要る）は含めない。
- 完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-137）。

### 再開の条件

「実装（3/6）」の「再開の条件」に従う。部品が main に入って契約テストを足す PR で、そのコードと公開テストに照らして本書の各段の記録を書き足す。段の表は変えない。残る段: なし。

- 確かめた base: `229803f`、`docs/spec` の gitlink は `0a802d7081610a36991a94b1ef220075d0616a28`

## 契約テストの実装（7/8）

7/8（#7576）で記録した。範囲は `docs/spec/05-tasks.md` の TASK-137 の記録に従う（ポインタのみ）。DEV-15・DEV-59・DEV-66 のポインタ。結果は GPU の実機なしで観測した値。

### 置き場

- 試験のファイル: `crates/contract/tests/stub_device_profiles.rs`
- `crates/contract/Cargo.toml` の `[[test]]`（`name = "stub_device_profiles"`・`required-features = ["test-support"]`。contract の `test-support` は core の同名 feature へ転送される）
- 使う core のパス: `stub_device::StubDeviceProvider`・`device`・`memory`・`execution::CommandQueue`・`capability`・`error::CompletionState`
- 要求する `backend` は `BackendKind::Metal` に固定している

### 確かめの表

各試験は `StubDeviceProvider::request` で得た `Box<dyn Device>` について、次を確かめる。`ownership()` が要求と一致する（`Imported` は中の値を読める）。`memory()`・`queue()` を `&dyn MemoryOps`・`&dyn CommandQueue` で受けられる。確保・書き込み・`download_async`・`free` と、`create_encoder`・`submit`・`poll`（`Complete`）が通る。`query`・`limits` を呼べる。

| 試験の名前 | プロファイル | 所有形態 | 置き方 |
| ---------- | ------------ | -------- | ------ |
| `cpu_only_product_independent` | `CpuOnly` | `ProductIndependent` | 条件なし |
| `cpu_only_shared` | `CpuOnly` | `Shared` | 条件なし |
| `cpu_only_imported` | `CpuOnly` | `Imported` | 条件なし |
| `compute_only_product_independent` | `ComputeOnly` | `ProductIndependent` | 条件なし |
| `compute_only_shared` | `ComputeOnly` | `Shared` | 条件なし |
| `compute_only_imported` | `ComputeOnly` | `Imported` | 条件なし |
| `graphics_product_independent` | `Graphics` | `ProductIndependent` | feature `graphics` の裏 |
| `graphics_shared` | `Graphics` | `Shared` | feature `graphics` の裏 |
| `graphics_imported` | `Graphics` | `Imported` | feature `graphics` の裏 |

### 組ごとの見え方

`scripts/build-profiles.sh` の `NATIVE_SETS` のうち `test-support` を含む 4 組と、全 feature の行（`make verify` の `--all-features` も同じ）で流れる。手元で観測した `test result: ok. N passed` の N は次のとおり。

| 実行 | N |
| ---- | -- |
| `cargo test -p fandhe-silicon-contract --no-default-features --features test-support --test stub_device_profiles` | 6 |
| 同 `--features backend-metal,test-support` | 6 |
| 同 `--features backend-vulkan,test-support` | 6 |
| 同 `--features backend-cuda,test-support` | 6 |
| `cargo test -p fandhe-silicon-contract --all-features --test stub_device_profiles` | 9 |

`test-support` を含まない組と既定の組では、この試験はビルドされない（`cargo test -p fandhe-silicon-contract` の出力に `stub_device_profiles` が出ない）。CI では `verify` ジョブの `make verify`・`make build-profiles` で流れる。wasm32 では contract の試験は既定の feature の型検査だけなので、この試験は流れない。

### 本リポに無い部品

| 部品 | 確かめ方 |
| ---- | -------- |
| `Device` から `ComputeOps` へ届く読み出しは無い | `crates/core/src/device.rs` の `Device` の method の一覧。`ComputeOps` を返す method が無い |
| 描画の操作の trait と、それへ届く `Device` の読み出しは無い | `git grep -n 'feature = "graphics"' -- crates/core` が 0 件。描画の型は TASK-837 のポインタ |
| `artifact-inspection` の裏の `Device` の読み出しは無い | `git grep -n 'artifact-inspection' -- crates/core` が 0 件 |
| core の `[features]` に `graphics`・`artifact-inspection` は無い | `crates/core/Cargo.toml` |

`fault-injection` がない構成で `fault_hooks()` が無いことは、`crates/contract/src/cfg_gate_doctest.rs` の `DeviceFaultHooksGate`（TASK-271 のポインタ）が確かめている。

上の表の部品が main に入ったら、同じ試験に確かめを足す。1 行目の `ComputeOps` への読み出しは今は `Device::compute()` として main にあり、その確かめは「compute の口の契約テスト（9/9）」に足した（この段の記録時点の値は書き換えない）。

## 完了の条件の確かめ（8/8）

8/8（#7577）で記録した。範囲は `docs/spec/05-tasks.md` の TASK-137（ポインタのみ）。結果は GPU の実機なしで、隔離した作業ツリー（base `4d0e20f`）で実際に流した出力による。コード・試験・CI は変えていない。

| # | 確かめる項目 | 方法 | 結果 |
| - | ------------ | ---- | ---- |
| (1) | 所有形態 3 × プロファイル 3 の組で `ownership()` が要求と一致（DEV-15 のポインタ） | `cargo test -p fandhe-silicon-contract --no-default-features --features test-support --test stub_device_profiles`、同 `--all-features --test stub_device_profiles` | 終了コード 0。前者は `test result: ok. 6 passed`、後者は `ok. 9 passed`。試験の名前は「契約テストの実装（7/8）」の確かめの表 |
| (2) | `memory()`・`queue()` を `&dyn MemoryOps`・`&dyn CommandQueue` で受け、`CpuOnly` でも確保・投入・`poll` が通る（DEV-59・DEV-66 のポインタ） | `cargo test -p fandhe-silicon-contract --features test-support --test stub_device_profiles cpu_only`、`grep -n 'let m: &dyn MemoryOps\|let q: &dyn CommandQueue' crates/contract/tests/stub_device_profiles.rs` | `cpu_only_imported`・`cpu_only_product_independent`・`cpu_only_shared` の 3 件が `ok`（`3 passed`）。grep は 52・53 行目の 2 行 |
| (3) | `query`・`limits` を呼べる（3 つの所有形態で） | `grep -n 'device.query\|device.limits' crates/contract/tests/stub_device_profiles.rs`、(1) の実行 | 76・77 行目の 2 行が共通の `check()` にあり、(1) の全試験が通る |
| (4) | `fault-injection` がない構成で `fault_hooks()` が無い（DEV-80・TASK-271 のポインタ） | `cargo test -p fandhe-silicon-contract --no-default-features --features test-support --doc`、同 `--all-features --doc` | どちらも終了コード 0。前者に `cfg_gate_doctest::DeviceFaultHooksGate (line 87) - compile fail ... ok`、後者に `cfg_gate_doctest::DeviceFaultHooksGate (line 87) ... ok` |
| (5) | `Device` から `ComputeOps`・描画の操作・`artifact-inspection` の口へ届く読み出しは本リポに無い | 「契約テストの実装（7/8）」の「本リポに無い部品」の確かめ方 | `git grep -n 'feature = "graphics"' -- crates/core` 0 件、`git grep -n 'artifact-inspection' -- crates/core` 0 件、`crates/core/Cargo.toml` に `graphics`・`artifact-inspection` の行 0 件。`Device` の method に `ComputeOps` を返すものは無い |
| (6) | 組ごとの見え方と流れる場所、main の CI | `grep -n 'run: make' .github/workflows/ci.yml`、`grep -n 'test-support' scripts/build-profiles.sh`、`gh run view 41274870749 --json status,conclusion,headSha,jobs` | `make build-profiles` は `verify` ジョブ（181 行目）と `verify-macos` ジョブ（468 行目）が呼ぶ。`NATIVE_SETS` の `test-support` を含む組は 4 つ（39〜42 行目）。7/8 のマージ（`4d0e20f`）の push の run `41274870749` は取得時点で実行中。`make verify (fmt + clippy + test)`・`make verify + build-profiles (macos-latest)` を含む 6 つのジョブが completed / success、`make deny (cargo-deny)` は in_progress |
| (7) | 依存 0・unsafe の追加 0 | `git diff --stat 2e08604 4d0e20f -- Cargo.lock deps-approved.toml`、`grep -c unsafe crates/contract/tests/stub_device_profiles.rs`、`grep -c '^\[\[approval\]\]' deps-approved.toml` | 差分なし、0、4 |
| (8) | 品質ゲート | `make check`・`make verify`・`make deny`・`make deps-check`・`make build-profiles` | いずれも終了コード 0 |

- wasm32 では contract の試験は既定の feature の型検査だけで、この試験（`test-support` が要る）は流れない。
- 完了の条件の正は private spec 側の記録に従う（ポインタのみ。`docs/spec/05-tasks.md` の TASK-137）。
- 確かめた base: `4d0e20f`

## compute の口の契約テスト（9/9）

範囲は `docs/spec/05-tasks.md` の TASK-137（ポインタのみ。DEV-59・DEV-60 のポインタ）。`Device::compute()`（[device-compute](device-compute.md)）の確かめを、既存の契約テストの共通の `check()` に足した。新しい `#[test]` は足していない。

### 足した確かめ

`crates/contract/tests/stub_device_profiles.rs` の 80〜95 行目（`device.limits()` の後）。`Profile` は `#[non_exhaustive]` で contract は外の crate なので、網羅の `match` ではなく `matches!` で分ける。

| プロファイル | 確かめ |
| ------------ | ------ |
| `CpuOnly` | `device.compute().is_none()` |
| `ComputeOnly`・`Graphics` | `device.compute()` が `Some`。`create_pipeline(PipelineInput::Msl { source: "", entry_point: "main" }, &BindingLayout::new(vec![]))` が `Ok`。返った `PipelineHandle` の `__backend_device_id()` が `device.generation()` と等しい |

先頭の `//!` にも、共通の確かめに `compute()` が入ることを足した。

### どの組で何本流れるか

- `make verify`（`--all-features`）: 9 本（3 プロファイル × 3 所有形態）
- `make build-profiles` の `test-support` を含む各組（`graphics` 無し）: 6 本。`Graphics` の 3 本は feature `graphics` の裏

### 確かめ方

```text
cargo test -p fandhe-silicon-contract --features test-support --test stub_device_profiles
cargo test -p fandhe-silicon-contract --all-features --test stub_device_profiles
sed -n 80,95p crates/contract/tests/stub_device_profiles.rs
```

### 実行の結果

base `e4386fe` の作業ツリーで流した。前者は `test result: ok. 6 passed; 0 failed`、後者は `test result: ok. 9 passed; 0 failed`。

## 描画の口の契約テスト（10/10）

範囲は `docs/spec/05-tasks.md` の TASK-137（ポインタのみ。DEV-59・DEV-60・DEV-63 のポインタ）。`Device::graphics()`（[graphics-types](graphics-types.md)。TASK-837 のポインタ）の確かめを、既存の契約テストの共通の `check()` に足し、feature の門の対を `cfg_gate_doctest.rs` に足した。新しい `#[test]` は足していない。

### 足した確かめ

`crates/contract/tests/stub_device_profiles.rs` の `check()` の中、compute の確かめの後。feature `graphics` のビルドでだけ動く。`Profile` は `#[non_exhaustive]` なので `matches!` で分ける。

| プロファイル | 確かめ |
| ------------ | ------ |
| `CpuOnly`・`ComputeOnly` | `device.graphics().is_none()` |
| `Graphics` | `&dyn GraphicsOps` で受け、`create_render_pipeline` が `Ok`。返った `PipelineHandle` の `__backend_device_id()` が `device.generation()` と等しい |

`crates/contract/src/cfg_gate_doctest.rs` に 2 つの対を足した。`GraphicsGate`（core の `graphics` モジュール）と `DeviceGraphicsGate`（`Device::graphics`）。

| 組 | 見え方 |
| -- | ------ |
| 既定の組、`graphics` を含まない `NATIVE_SETS` の組 | `compile fail ... ok` |
| `NATIVE_SETS` の BUILD-3・BUILD-4 の組、`--all-features` | `ok` |

### どの組で何本流れるか

- `make verify`（`--all-features`）: 9 本。`graphics()` の確かめが流れるのは、契約の crate で `graphics` と `test-support` を両方含む組、つまり今は `--all-features` だけ
- `make build-profiles` の `test-support` を含む各組（`graphics` 無し）: 6 本

### 確かめ方

```text
cargo test -p fandhe-silicon-contract --features test-support --test stub_device_profiles
cargo test -p fandhe-silicon-contract --all-features --test stub_device_profiles
cargo test -p fandhe-silicon-contract --doc cfg_gate
cargo test -p fandhe-silicon-contract --no-default-features --features contract-core,cpu-isa,cpu-ops,backend-vulkan,graphics --doc cfg_gate
```

### 実行の結果

base `760233c` の作業ツリーで流した。

- 1 つ目: `test result: ok. 6 passed; 0 failed`
- 2 つ目: `test result: ok. 9 passed; 0 failed`
- 3 つ目: `GraphicsGate (line 99) - compile fail ... ok`・`DeviceGraphicsGate (line 109) - compile fail ... ok`
- 4 つ目: `GraphicsGate (line 99) ... ok`・`DeviceGraphicsGate (line 109) ... ok`

## 本リポでの反映の方針

本書には現状の事実と、決定の記録のポインタ・どの形でも守る規約を置く。具体の形（型・名前・置き場・試験の名前）は受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す。
置き場と形は TASK-137 の記録に従う（ポインタのみ）。`core` に足す場合は外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ。

守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)。

## 関連

[contract-device-types](contract-device-types.md)・[contract-feature-check](contract-feature-check.md)・[feature-gate-compile-check](feature-gate-compile-check.md)・[contract-fault-injection](contract-fault-injection.md)・[test-frame](test-frame.md)・[docs/contract/README.md](contract/README.md)

## 現状の公開のトレイトと背後実装の公開の項目

「実装（3/6）」の「5 件だけ」、「テスト（4/6）」・「CI への組み込み（5/6）」の「4 件のまま」、「確かめ（6/6）」の F1 の「4 件」・F4 など、各段の公開のトレイトと device の口の件数・有無は記録時点の値で、書き換えない。今の値は次のとおり（冒頭の「本リポの現状」の表と同じ）。

- device のトレイトと device を返す入口は `crates/core/src/device.rs` にある（`Device`・`DeviceProvider`）
- 公開のトレイト（`pub trait`）を `crates`・`wasm` から探すと 14 件（`Capabilities`・`Device`・`DeviceProvider`・`GraphicsOps`・`FaultHooks`・`MemoryOps`・`Check`・`ValidationIntake`・`DeviceLostNotify`・`ComputeOps`・`CommandQueue`・`ExternalInterop`・内部用の `__BackendEncoderCloser`・`__BackendSharedEncoderCloser`）
- 投入の列のトレイト `CommandQueue`、compute の口 `ComputeOps`、試験用の device `StubDevice` と provider `StubDeviceProvider`（`crates/core/src/stub_device.rs`。feature `test-support` の裏。試験は `crates/core/tests/stub_device_*.rs`）が `crates/core` にある。詳細は [stub-device](stub-device.md)
- 前提の部品の Issue の今の状態（`gh issue view <番号> --json state`）は、#1100 が CLOSED、#696・#784・#1126・#447・#448・#1305・#1149 が OPEN。「#1100 は OPEN」「8 件とも OPEN」は記録時点の値で、書き換えない
- 「確かめ（6/6）」の F7（言及の範囲）の「本書・`CLAUDE.md`・`README.md` だけ」は記録時点の値で、書き換えない。今の値は上の「本リポの現状」の「本書の対象の言及」の行のとおり
- 「実装（7/8）」の「使う core のパス」に並ぶ `error::CompletionState` は記録時点の値で、書き換えない。`crates/contract/tests/stub_device_profiles.rs` は今 `error::ContractError` も import している
- 「前提の部品の Issue」や「部品が今も `crates/`・`wasm/` に無い」とする各段の記述（「テスト（4/6）」・「CI への組み込み（5/6）」・「確かめ（6/6）」の F2・F6・成果物の対応の表を含む）も記録時点の値で、書き換えない
- `backend_metal.rs` は macOS だけの `pub use` を持つ（`MetalDeviceProvider`、feature `fault-injection` の裏の `MetalCommandQueueNoneProvider`・`MetalDefaultDeviceNoneProvider`）。`backend_vulkan.rs` は `pub mod spirv_operands` だけ。`backend_cuda.rs` に `pub` の項目は無い
- 「契約テストの実装（7/8）」の「本リポに無い部品」の 1 行目と「完了の条件の確かめ（8/8）」の (5) の「`Device` の method に `ComputeOps` を返すものは無い」は記録時点の値で、書き換えない。今は `Device::compute()` が `crates/core/src/device.rs` にある（[device-compute](device-compute.md)）
- `crates/core` の feature は `test-support`・`fault-injection`・`native-blocking-wait`・`ext-interop`・`graphics` の 5 つ（`crates/core/Cargo.toml` の `[features]`）。「形が乗る既存のコード・検査の事実」の feature の行（3 つだけ）は記録時点の値で、書き換えない
- 上の `crates/core` の feature の行と末尾の表の「描画の操作の trait …は無い」「core の `[features]` に `graphics` … は無い」は記録時点の値で、書き換えない。今は core の `[features]` に `graphics` があり（契約の crate の `graphics` から転送）、`crates/core/src/graphics.rs` に `GraphicsOps` がある（[graphics-types](graphics-types.md)）。公開のトレイトは `GraphicsOps` を足して 14 件
- 末尾の表の「描画の操作の trait と、それへ届く `Device` の読み出しは無い」は記録時点の値で、書き換えない。今は `Device::graphics()` が feature `graphics` の裏で `crates/core/src/device.rs` にある（既定は `None`。[graphics-types](graphics-types.md)）
- `Device::graphics()` の確かめは今は「描画の口の契約テスト（10/10）」にある。「契約テストの実装（7/8）」の「本リポに無い部品」の 2 行目と「完了の条件の確かめ（8/8）」の (5) の描画の操作の部分は記録時点の値で、書き換えない

## 今の値の補い（base 6b35dd9）

上の表の `make build-profiles` を呼ぶ行（`verify` ジョブの 181 行目と `verify-macos` ジョブの 468 行目）は記録時点の値で、書き換えない。今の `.github/workflows/ci.yml` では `verify` ジョブが 181 行目、`verify-macos` ジョブが 508 行目である（数えたコマンド: `grep -nE 'run: make (verify|wasm-verify|build-profiles|profile-test-count-check)$' .github/workflows/ci.yml`）。

## 今の値の補い（base 3e32680）

記録時点の表と既存の節は書き換えない。以下は base `3e32680` で観測した今の値（実行環境は GPU なしの Linux）。

| 表の行 | 前の値 → 今の値 | 測ったコマンド |
| ------ | --------------- | -------------- |
| 本リポの現状の事実の「背後実装のモジュール」の行 | `backend_metal.rs`・`backend_cuda.rs` に `pub` の項目は無い → `backend_cuda.rs` に `pub struct CudaDeviceProvider;`（232 行目）の 1 件がある（`backend_cuda.rs` の `pub` の行はこの 1 行だけ）。`backend_vulkan.rs` は `pub mod spirv_operands;`（12 行目）のまま | `git grep -nE '^\s*pub ' -- crates/contract/src/backend_cuda.rs crates/contract/src/backend_vulkan.rs` |
| 「確かめ」の表の F4 の行（背後実装のモジュールの公開の項目） | `backend_vulkan.rs` の `pub mod spirv_operands` だけ（4/6 の記録と同じ）→ `backend_vulkan.rs` の `pub mod spirv_operands` に加えて、`backend_cuda.rs` の `pub struct CudaDeviceProvider;` | `git grep -nE '^\s*pub ' -- crates/contract/src/backend_cuda.rs crates/contract/src/backend_vulkan.rs` |
| 末尾近くの「`backend_cuda.rs` に `pub` の項目は無い」の文 | `backend_cuda.rs` に `pub` の項目は無い → `backend_cuda.rs` に `pub struct CudaDeviceProvider;` が 1 件ある（[cuda-device-request](cuda-device-request.md)） | `git grep -nE '^\s*pub ' -- crates/contract/src/backend_cuda.rs` |
