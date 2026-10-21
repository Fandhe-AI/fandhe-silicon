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
| TASK-136 | `crates/core/src/device.rs`・`capability.rs` | 同上（`gh issue view 411 --json state`） | [contract-device-types](contract-device-types.md) |

## 本リポの現状

| 項目 | 事実 | 確かめ方 |
| ---- | ---- | -------- |
| 既にある型 | device の要求・能力問合せの型は `crates/core` にある。定義と確かめは [contract-device-types](contract-device-types.md) を参照し、ここでは繰り返さない | `crates/core/src/device.rs`、`crates/core/src/capability.rs` |
| 公開のトレイト | `crates/`・`wasm/` の `pub trait` は 4 件だけ（`Capabilities`、`FaultHooks`〔`fault-injection` の裏〕、`MemoryOps`〔`crates/core/src/memory/ops.rs`〕、`Check`〔`test-support` の裏〕） | `git grep -nE 'pub trait' -- crates wasm` |
| device まわりの口 | device のトレイト、device を返す入口、投入の列のトレイト、compute の口、試験用のスタブの device に当たる物は `crates/`・`wasm/` に無い（`pub trait` は上の 4 件だけ）。メモリの操作のトレイト `MemoryOps` とバッファ型（`BufferHandle`・`BufferUsage`）は `crates/core/src/memory/` に既にある。名前に `Device` を含む型は要求と生ハンドルの型、`Stub` を含む型は故障の差し込み用の `FaultStub` だけ。`crates/core/src/memory.rs` はそれらの再公開と引数検査の純粋関数（`validate_allocation_size`・`validate_buffer_range`）を持つ | `git grep -nE 'pub (trait\|struct) [A-Za-z]*(Device\|Memory\|Queue\|Stub)' -- crates wasm` |
| 故障の差し込み口 | `FaultHooks` はある。それを返す公開のアクセサを持つ device のトレイトは無い（`crates/core/tests/fault_parts_composition.rs` の同名メソッドは試験内の局所トレイト） | `crates/core/src/fault.rs`、`crates/core/tests/fault_parts_composition.rs` |
| feature の裏のアイテム | `graphics`・`artifact-inspection` は `Cargo.toml` に宣言済みだが、これらで切られたアイテムは 0 件 | `git grep -n 'feature = "graphics"' -- crates`、`git grep -n 'feature = "artifact-inspection"' -- crates` |
| 組ごとのビルド検査 | `scripts/build-profiles.sh` の `NATIVE_SETS`（`graphics` を含む組と含まない組がある）と `WASM_SETS`。`make build-profiles`（CI の `verify` ジョブ） | `grep -n 'NATIVE_SETS\|WASM_SETS' scripts/build-profiles.sh` |
| コンパイル失敗の既存の仕組み | feature の有無でコンパイルが落ちることを確かめる枠が `crates/contract/src/cfg_gate_doctest.rs` にある | [feature-gate-compile-check](feature-gate-compile-check.md) |
| 走る場所 | `make verify`（`--all-features`）と `make build-profiles`。GPU の実機は要らない | `scripts/verify.sh`、`scripts/build-profiles.sh` |
| 依存の承認 | 外部 crate は足さない。`deps-approved.toml` の記録の範囲に収める | `grep -c '^\[\[' deps-approved.toml` |
| 本書の対象の言及 | TASK-137 の言及は本書、`CLAUDE.md`、`README.md` だけ | `git grep -nE 'TASK-137([^0-9]\|$)' -- . ':!docs/spec' ':!.agents'` |

## 段の表

| 段 | Issue | 内容 |
| -- | ----- | ---- |
| 1/6 | #3659 | 現状と前提の確認（本書） |
| 2/6 | #3660 | 形の設計 |
| 3/6 | #3661 | 実装 |
| 4/6 | #3662 | テスト |
| 5/6 | #3663 | CI への組み込み |
| 6/6 | #3664 | 確かめ |

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

- `git grep -nE 'pub trait' -- crates wasm`（4 件だけであること）
- `git grep -nE 'pub (trait|struct) [A-Za-z]*(Device|Queue|Stub)' -- crates wasm`
- `gh issue view 696 --json state`・`gh issue view 784 --json state`・`gh issue view 1100 --json state`

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
| 流れる組 | 既定の組、`NATIVE_SETS` の各組、core を feature なしで流す行、`--all-features`、`WASM_SETS` の `cargo check` | `grep -n '_SETS' scripts/build-profiles.sh` |
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
- プロファイルごとの本数の照合: TASK-227（#1149）

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

## 本リポでの反映の方針

本書には現状の事実と、決定の記録のポインタ・どの形でも守る規約を置く。具体の形（型・名前・置き場・試験の名前）は受け持つ段の PR のコードと公開テストが示し、その PR で本書に節を足す。
置き場と形は TASK-137 の記録に従う（ポインタのみ）。`core` に足す場合は外部依存 0・`forbid(unsafe_code)`・wasm32 でも通る形を保つ。

守る規約: [poc-carry-over](../.claude/rules/poc-carry-over.md)・[dependency-policy](../.claude/rules/dependency-policy.md)・[coding-rust](../.claude/rules/coding-rust.md)・[spec-confidentiality](../.claude/rules/spec-confidentiality.md)・[code-comment-style](../.claude/rules/code-comment-style.md)。

## 関連

[contract-device-types](contract-device-types.md)・[contract-feature-check](contract-feature-check.md)・[feature-gate-compile-check](feature-gate-compile-check.md)・[contract-fault-injection](contract-fault-injection.md)・[test-frame](test-frame.md)・[docs/contract/README.md](contract/README.md)
